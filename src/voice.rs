//! The voice call: the microphone is listened to by its level (speech starts when it rises,
//! ends after the pause set in Ajustes, 2 s by default), each stretch of speech goes to
//! whisper.cpp's server (run here as a program, its model loaded once) and its words come back as
//! a message; the summary of what the agent answers is spoken a sentence at a time by Piper
//! through the speaker, or by the system's voice while Piper isn't there. whisper.cpp, Piper and
//! their models are programs and files the installer downloads (install.rs), never the call.
// ponytail: speech is found by level, not by a VAD model (Silero when it misfires); the words
// go over a local HTTP call through curl; Windows and Linux download whisper.cpp's binaries, the
// Mac takes it from Homebrew (brew install whisper-cpp)

use crate::prelude::*;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

pub(crate) const SAMPLE_RATE: u32 = 16_000;
/// 20 ms blocks at 16 kHz.
const BLOCK: usize = 320;
/// whisper.cpp's models: the small one on the CPU, large-v3-turbo (far better Portuguese) on a GPU.
pub(crate) const WHISPER_MODEL: &str = "ggml-small-q5_1.bin";
pub(crate) const WHISPER_TURBO: &str = "ggml-large-v3-turbo-q5_0.bin";
pub(crate) const WHISPER_MODELS_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
pub(crate) const WHISPER_RELEASE: &str = "https://github.com/ggml-org/whisper.cpp/releases/download/b5454";
pub(crate) const PIPER_RELEASE: &str = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2";
const PIPER_VOICES_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main";
/// The voices offered in Ajustes › Voz (Piper's names: locale-speaker-quality).
pub(crate) const VOICES: [&str; 8] = [
    "pt_BR-faber-medium",
    "pt_BR-cadu-medium",
    "pt_BR-jeff-medium",
    "pt_BR-edresson-low",
    "en_US-lessac-medium",
    "en_US-amy-medium",
    "en_US-ryan-high",
    "en_US-hfc_female-medium",
];

/// How the call is set up, from Ajustes: the Piper voice, the language whisper hears ("auto",
/// "pt", "en") and whether to try the GPU build of whisper.cpp.
#[derive(Clone, Debug)]
pub(crate) struct VoiceSetup {
    pub(crate) voice: String,
    pub(crate) language: String,
    pub(crate) gpu: bool,
    /// Seconds of quiet that end what the person says (Ajustes › Voz).
    pub(crate) pause: f32,
}

impl Default for VoiceSetup {
    fn default() -> Self {
        VoiceSetup { voice: VOICES[0].into(), language: "pt".into(), gpu: true, pause: 2.0 }
    }
}

/// Where a Piper voice's files live, from its name ("pt_BR-faber-medium" → pt/pt_BR/faber/medium).
pub(crate) fn piper_voice_url(voice: &str) -> Option<String> {
    let (locale, rest) = voice.split_once('-')?;
    let (speaker, quality) = rest.rsplit_once('-')?;
    let lang = locale.split('_').next()?;
    Some(format!("{PIPER_VOICES_URL}/{lang}/{locale}/{speaker}/{quality}"))
}

/// The GPU this computer has for the models, and whether a whisper.cpp build uses it here: NVIDIA
/// with CUDA on Windows; Metal comes with Homebrew's build on the Mac; on Linux the ready-made
/// build is CPU only.
pub(crate) fn gpu() -> (String, bool) {
    static FOUND: std::sync::OnceLock<(String, bool)> = std::sync::OnceLock::new();
    FOUND
        .get_or_init(|| {
            if cfg!(target_os = "macos") {
                return ("Apple (Metal)".into(), true);
            }
            let nvidia = crate::ide::lsp::which("nvidia-smi").is_some() || Path::new("C:/Windows/System32/nvidia-smi.exe").is_file();
            match (nvidia, cfg!(windows)) {
                (true, true) => ("NVIDIA (CUDA)".into(), true),
                // ponytail: Linux would need the CUDA/Vulkan build compiled here; upgrade: build it once
                (true, false) => ("NVIDIA (sem build CUDA pronto: CPU)".into(), false),
                _ => ("Nenhuma (CPU)".into(), false),
            }
        })
        .clone()
}

/// A voice command the person says to the call, by the agent's name ("Claude, se muta").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VoiceCmd {
    /// Stop listening to the conversation: answers wait with a tone until called by name.
    Mute,
    /// Back to listening to everything; what was held is said.
    Unmute,
    /// Say what is held.
    Speak,
    /// Called by name with a request: it goes to the agent even on standby.
    Ask(String),
}

/// What `text` tells the agent named `name` ("claude", "codex"), when it is called by name.
pub(crate) fn voice_command(text: &str, name: &str) -> Option<VoiceCmd> {
    let clean: String = text.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' }).collect();
    let words: Vec<&str> = clean.split_whitespace().collect();
    let aliases: &[&str] = match name {
        "codex" => &["codex", "códex", "kodex", "codecs"],
        _ => &["claude", "cloud", "claud", "claudio", "clode", "cláudio", "claudio"],
    };
    // "ei Claude", "ok Claude", "hey Claude"
    let start = words.iter().position(|w| aliases.contains(w)).filter(|&i| i <= 1)?;
    let rest = words[start + 1..].join(" ");
    let has = |options: &[&str]| options.iter().any(|o| rest.contains(o));
    if has(&["desmuta", "desmute", "unmute", "pode ouvir", "volta a ouvir", "acorda", "liga o microfone", "wake up"]) {
        return Some(VoiceCmd::Unmute);
    }
    if has(&["se muta", "muta", "mudo", "silêncio", "silencio", "fica quieto", "cala", "para de ouvir", "mute", "quiet", "stop listening"]) {
        return Some(VoiceCmd::Mute);
    }
    if rest.is_empty() || has(&["pode falar", "fala", "pode responder", "responde", "manda", "go ahead", "you can talk", "speak", "answer"]) {
        return Some(VoiceCmd::Speak);
    }
    Some(VoiceCmd::Ask(rest))
}

/// The call's tones, all of one family: soft bell notes (C5, G5) rising when it connects, falling
/// when it ends, a single one when an answer waits on standby.
pub(crate) const CALL_START: [f32; 2] = [523.25, 783.99];
pub(crate) const CALL_END: [f32; 2] = [783.99, 523.25];
pub(crate) const CALL_WAITING: [f32; 1] = [783.99];

/// The samples of `notes` (Hz), 140 ms apart, at `rate`: each a sine with its octave, a quick
/// attack and a bell's decay.
pub(crate) fn chime_samples(notes: &[f32], rate: u32) -> Vec<f32> {
    let step = rate as usize * 14 / 100;
    let mut out = vec![0f32; step * notes.len() + rate as usize * 35 / 100];
    for (k, hz) in notes.iter().enumerate() {
        for (i, s) in out[k * step..].iter_mut().enumerate() {
            let t = i as f32 / rate as f32;
            let envelope = (t / 0.008).min(1.0) * (-t * 9.0).exp();
            let wave = (t * hz * std::f32::consts::TAU).sin() + 0.25 * (t * hz * 2.0 * std::f32::consts::TAU).sin();
            *s += wave * envelope * 0.16;
        }
    }
    out
}

/// `notes` through the speaker; the stream plays while kept (about half a second).
pub(crate) fn chime(notes: &[f32]) -> Option<cpal::Stream> {
    const RATE: u32 = 22_050;
    play(Arc::new(Mutex::new(chime_samples(notes, RATE).into())), RATE)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum VoiceState {
    #[default]
    Off,
    /// Downloading or starting what it needs.
    Preparing,
    Listening,
    /// Speech on the microphone right now.
    Hearing,
    Speaking,
    Muted,
}

impl VoiceState {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            VoiceState::Off => "Voz desligada",
            VoiceState::Preparing => "Preparando a voz…",
            VoiceState::Listening => "Ouvindo",
            VoiceState::Hearing => "Ouvindo você…",
            VoiceState::Speaking => "Falando",
            VoiceState::Muted => "Mudo",
        }
    }
}

pub(crate) enum VoiceEvent {
    /// whisper.cpp is up: the microphone is on.
    Ready,
    Status(String),
    Heard(String),
    /// Speech on the microphone started or stopped, for the state shown.
    Hearing(bool),
    Failed(String),
}

/// What the threads share with the app.
#[derive(Clone, Default)]
pub(crate) struct VoiceShared {
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) muted: Arc<AtomicBool>,
    /// The agent is being spoken: the microphone sits out (it would hear the speaker).
    pub(crate) hold: Arc<AtomicBool>,
    /// The microphone's level, 0..1 (f32 bits), for the button to show.
    pub(crate) level: Arc<AtomicU32>,
}

/// The call: what it is doing, the programs it runs, and the sentences left to say.
#[derive(Default)]
pub(crate) struct Voice {
    pub(crate) on: bool,
    pub(crate) state: VoiceState,
    /// What is going on, under the box: a download, an error.
    pub(crate) status: String,
    pub(crate) shared: VoiceShared,
    pub(crate) events: Option<Receiver<VoiceEvent>>,
    pub(crate) server: Option<Child>,
    /// The chat spoken for, and how many of its items were already there (not read out).
    pub(crate) agent: Option<String>,
    pub(crate) spoken: usize,
    pub(crate) queue: VecDeque<String>,
    pub(crate) player: Option<Player>,
    /// "Claude, se muta": the conversation isn't listened to; only its name reaches it, and
    /// answers wait in `held` after a tone until "Claude, pode falar".
    pub(crate) standby: bool,
    pub(crate) held: VecDeque<String>,
    pub(crate) tone: Option<(cpal::Stream, f64)>,
}

impl Drop for Voice {
    fn drop(&mut self) {
        self.shut();
    }
}

impl Voice {
    /// Everything off: the microphone thread, whisper.cpp, the sentence being said.
    pub(crate) fn shut(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.hold.store(false, Ordering::Relaxed);
        if let Some(mut server) = self.server.take() {
            let _ = server.kill();
            let _ = server.wait();
        }
        self.hush();
        self.events = None;
        self.on = false;
        self.state = VoiceState::Off;
    }

    /// Stops what is being said and forgets the rest.
    pub(crate) fn hush(&mut self) {
        self.queue.clear();
        self.held.clear();
        self.player = None;
        self.shared.hold.store(false, Ordering::Relaxed);
    }
}

/// Where the voice's programs and models live.
pub(crate) fn voice_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("voice"))
}

/// Speech found by level, 20 ms blocks at a time: a stretch that rises over the noise floor for
/// 60 ms starts it, the pause under it ends it; 0.3 s before the rise comes along. Gives each
/// stretch of at least half a second, 60 s at most.
pub(crate) struct Segmenter {
    floor: f32,
    speech: Vec<f32>,
    pre: VecDeque<Vec<f32>>,
    loud: usize,
    quiet: usize,
    /// Blocks over the floor in the stretch so far: a click alone isn't speech.
    voiced: usize,
    pub(crate) in_speech: bool,
    /// Quiet blocks that end it: a pause shorter than this is part of what is said.
    end: usize,
}

impl Default for Segmenter {
    fn default() -> Self {
        Segmenter::with_pause(2.0)
    }
}

impl Segmenter {
    /// Ends speech after `seconds` of quiet (0.6 s at least).
    pub(crate) fn with_pause(seconds: f32) -> Self {
        let end = (seconds.max(0.6) * SAMPLE_RATE as f32 / BLOCK as f32) as usize;
        Segmenter { floor: 0.01, speech: Vec::new(), pre: VecDeque::new(), loud: 0, quiet: 0, voiced: 0, in_speech: false, end }
    }

    pub(crate) fn feed(&mut self, block: &[f32]) -> Option<Vec<f32>> {
        let rms = (block.iter().map(|s| s * s).sum::<f32>() / block.len().max(1) as f32).sqrt();
        // the floor follows quiet quickly and noise slowly, so a voice doesn't become the floor
        self.floor = if rms < self.floor { self.floor * 0.9 + rms * 0.1 } else { self.floor * 0.998 + rms * 0.002 };
        let loud = rms > (self.floor * 3.5).max(0.012);
        if !self.in_speech {
            self.pre.push_back(block.to_vec());
            if self.pre.len() > 15 {
                self.pre.pop_front();
            }
            self.loud = if loud { self.loud + 1 } else { 0 };
            if self.loud >= 3 {
                self.in_speech = true;
                (self.quiet, self.voiced) = (0, self.loud);
                self.speech = self.pre.drain(..).flatten().collect();
            }
            return None;
        }
        self.speech.extend_from_slice(block);
        self.voiced += loud as usize;
        self.quiet = if loud { 0 } else { self.quiet + 1 };
        if self.quiet < self.end && self.speech.len() < 60 * SAMPLE_RATE as usize {
            return None;
        }
        self.in_speech = false;
        self.loud = 0;
        let speech = std::mem::take(&mut self.speech);
        (self.voiced >= 10 && speech.len() >= SAMPLE_RATE as usize / 2).then_some(speech)
    }
}

/// Mono at 16 kHz from `channels` interleaved at `rate`, by linear interpolation.
pub(crate) fn to_16k(samples: &[f32], channels: usize, rate: u32) -> Vec<f32> {
    let channels = channels.max(1);
    let mono: Vec<f32> = samples.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect();
    if rate == SAMPLE_RATE || mono.is_empty() {
        return mono;
    }
    let step = rate as f32 / SAMPLE_RATE as f32;
    let n = ((mono.len() as f32 - 1.0) / step).floor() as usize + 1;
    (0..n)
        .map(|i| {
            let x = i as f32 * step;
            let (k, t) = (x as usize, x.fract());
            let next = mono.get(k + 1).copied().unwrap_or(mono[k]);
            mono[k] * (1.0 - t) + next * t
        })
        .collect()
}

/// A WAV file (16-bit mono) of `samples` at `rate`.
pub(crate) fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

/// The sentences of an answer, to say one at a time; Markdown marks and code blocks left out.
pub(crate) fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (k, part) in text.split("```").enumerate() {
        if k % 2 == 1 {
            continue;
        }
        let plain: String = part
            .lines()
            .map(|l| l.trim_start_matches(['#', '>', ' ', '-', '*']).trim())
            .filter(|l| !l.is_empty() && !l.starts_with('|'))
            .collect::<Vec<_>>()
            .join("\n")
            .replace("**", "")
            .replace('`', "");
        let mut current = String::new();
        let mut chars = plain.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\n' {
                if !current.trim().is_empty() {
                    out.push(current.trim().to_string());
                }
                current.clear();
                continue;
            }
            current.push(ch);
            // a mark ends it only before a space: "chat.rs", "3.5" go on
            if matches!(ch, '.' | '!' | '?' | '…') && chars.peek().is_none_or(|c| c.is_whitespace()) && current.trim().chars().count() > 2 {
                out.push(current.trim().to_string());
                current.clear();
            }
        }
        if !current.trim().is_empty() {
            out.push(current.trim().to_string());
        }
    }
    out
}

/// What is said of an answer: its first three sentences, but none with a path, a file name or a
/// link; when the answer has more, "Os detalhes estão na tela." closes it.
pub(crate) fn spoken(text: &str) -> Vec<String> {
    let all = sentences(text);
    let mut out: Vec<String> = all.iter().take(3).filter(|s| !has_path(s)).cloned().collect();
    if out.len() < all.len() || text.contains("```") || text.lines().any(|l| l.trim_start().starts_with('|')) {
        out.push("Os detalhes estão na tela.".into());
    }
    out
}

/// Whether a sentence has what can't be said: a link, a path, a file name ("chat.rs:42").
pub(crate) fn has_path(sentence: &str) -> bool {
    sentence.split_whitespace().any(|w| {
        if w.contains("://") || w.contains('/') || w.contains('\\') || w.contains("::") {
            return true;
        }
        let w = w.trim_matches(|c: char| !c.is_alphanumeric()).split(':').next().unwrap_or_default();
        w.rsplit_once('.').is_some_and(|(name, ext)| {
            name.chars().any(char::is_alphabetic)
                && (1..=5).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && ext.chars().any(|c| c.is_ascii_alphabetic())
        })
    })
}

/// Where a program's file came out of its archive, by name.
pub(crate) fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|f| f == name) {
                return Some(path);
            }
        }
    }
    None
}

pub(crate) fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

/// whisper.cpp's server program, as the installer left it (the CUDA build when `cuda`); on the
/// Mac, Homebrew's (with Metal).
pub(crate) fn whisper_server(dir: &Path, cuda: bool) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return crate::ide::lsp::which("whisper-server");
    }
    find_file(&dir.join(if cuda { "whisper-cuda" } else { "whisper" }), &exe("whisper-server"))
}

/// Piper and the voice `name`, as the installer left them; None speaks with the system's voice.
pub(crate) fn piper(dir: &Path, name: &str) -> Option<(PathBuf, PathBuf)> {
    let voice = dir.join("voices").join(format!("{name}.onnx"));
    let program = find_file(&dir.join("piper"), &exe("piper"))?;
    (voice.is_file() && voice.with_extension("onnx.json").is_file()).then_some((program, voice))
}

/// A free local port for whisper.cpp's server.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map_or(8178, |a| a.port())
}

/// Gets everything ready and starts whisper.cpp's server on `port`; the server process comes back
/// so the app can end it.
pub(crate) fn prepare(tx: &Sender<VoiceEvent>, port: u16, setup: &VoiceSetup) -> Result<(Child, Option<(PathBuf, PathBuf)>), String> {
    const MISSING: &str = "A voz não está instalada: rode o instalador do MagoAgent (ele baixa os modelos uma vez só).";
    let dir = voice_dir().ok_or("Sem pasta de dados.")?;
    let cuda = setup.gpu && cfg!(windows) && gpu().1 && whisper_server(&dir, true).is_some();
    let server = whisper_server(&dir, cuda).ok_or(MISSING)?;
    // the turbo model wants a GPU; the small one is the CPU's
    let turbo = dir.join(WHISPER_TURBO);
    let gpu_on = setup.gpu && gpu().1 && (cuda || cfg!(target_os = "macos"));
    let model = if gpu_on && turbo.is_file() { turbo } else { dir.join(WHISPER_MODEL) };
    if !model.is_file() {
        return Err(MISSING.into());
    }
    let piper = piper(&dir, &setup.voice);
    let _ = tx.send(VoiceEvent::Status("Iniciando o reconhecimento de voz…".into()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let language = match setup.language.as_str() {
        "pt" | "en" => setup.language.as_str(),
        _ => "auto",
    };
    let mut child = crate::ide::command(&server)
        .arg("-m")
        .arg(&model)
        .args(["--host", "127.0.0.1", "--port", &port.to_string(), "-l", language, "-t", &threads.to_string(), "-nt"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("whisper-server: {e}"))?;
    // up to 60 s for the model to load
    for _ in 0..600 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok((child, piper));
        }
        if let Ok(Some(status)) = child.try_wait() {
            // the CUDA build may not run here (driver, DLLs): the CPU one then
            if cuda {
                let _ = tx.send(VoiceEvent::Status("O whisper.cpp com CUDA não abriu; usando a CPU.".into()));
                return prepare(tx, port, &VoiceSetup { gpu: false, ..setup.clone() });
            }
            return Err(format!("O whisper-server fechou ({status})."));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = child.kill();
    Err("O reconhecimento de voz não respondeu.".into())
}

/// The words in `samples` (16 kHz mono), from whisper.cpp's server.
pub(crate) fn transcribe(port: u16, samples: &[f32]) -> Result<String, String> {
    let path = std::env::temp_dir().join(format!("magoagent-voz-{}.wav", uuid::Uuid::new_v4().simple()));
    fs::write(&path, wav(samples, SAMPLE_RATE)).map_err(|e| e.to_string())?;
    let output = crate::ide::command("curl")
        .args(["-sS", "--fail", "-F", "response_format=json", "-F", "temperature=0", "-F"])
        .arg(format!("file=@{}", path.display()))
        .arg(format!("http://127.0.0.1:{port}/inference"))
        .output();
    let _ = fs::remove_file(&path);
    let output = output.map_err(|e| format!("curl: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let json: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    Ok(json["text"].as_str().unwrap_or_default().trim().to_string())
}

/// The microphone, until `stop`: what is said goes as `Heard`. Runs on its own thread.
pub(crate) fn listen(shared: VoiceShared, tx: Sender<VoiceEvent>, port: u16, pause: f32) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        let _ = tx.send(VoiceEvent::Failed("Nenhum microfone encontrado.".into()));
        return;
    };
    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(VoiceEvent::Failed(format!("Microfone: {e}")));
            return;
        }
    };
    let (rate, channels) = (config.sample_rate().0, config.channels() as usize);
    let raw: Arc<Mutex<Vec<f32>>> = Arc::default();
    let sink = raw.clone();
    let err = |e| eprintln!("microfone: {e}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(&config.into(), move |data: &[f32], _| sink.lock().unwrap().extend_from_slice(data), err, None),
        cpal::SampleFormat::I16 => {
            device.build_input_stream(&config.into(), move |data: &[i16], _| sink.lock().unwrap().extend(data.iter().map(|s| *s as f32 / 32768.0)), err, None)
        }
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _| sink.lock().unwrap().extend(data.iter().map(|s| (*s as f32 - 32768.0) / 32768.0)),
            err,
            None,
        ),
        other => {
            let _ = tx.send(VoiceEvent::Failed(format!("Microfone em formato {other} não é lido.")));
            return;
        }
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(VoiceEvent::Failed(format!("Microfone: {e}")));
            return;
        }
    };
    if let Err(e) = stream.play() {
        let _ = tx.send(VoiceEvent::Failed(format!("Microfone: {e}")));
        return;
    }
    let _ = tx.send(VoiceEvent::Ready);
    let mut segmenter = Segmenter::with_pause(pause);
    let mut pending: Vec<f32> = Vec::new();
    let mut was_hearing = false;
    while !shared.stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let taken = std::mem::take(&mut *raw.lock().unwrap());
        pending.extend(to_16k(&taken, channels, rate));
        let quiet = shared.muted.load(Ordering::Relaxed) || shared.hold.load(Ordering::Relaxed);
        while pending.len() >= BLOCK {
            let block: Vec<f32> = pending.drain(..BLOCK).collect();
            let peak = block.iter().fold(0f32, |m, s| m.max(s.abs()));
            shared.level.store(peak.to_bits(), Ordering::Relaxed);
            if quiet {
                segmenter = Segmenter::with_pause(pause);
                continue;
            }
            let heard = segmenter.feed(&block);
            if segmenter.in_speech != was_hearing {
                was_hearing = segmenter.in_speech;
                let _ = tx.send(VoiceEvent::Hearing(was_hearing));
            }
            if let Some(samples) = heard {
                let _ = tx.send(VoiceEvent::Status("Entendendo…".into()));
                match transcribe(port, &samples) {
                    Ok(text) if !text.is_empty() && !text.starts_with('[') && !text.starts_with('(') => {
                        let _ = tx.send(VoiceEvent::Heard(text));
                    }
                    Ok(_) => {
                        let _ = tx.send(VoiceEvent::Status(String::new()));
                    }
                    Err(e) => {
                        let _ = tx.send(VoiceEvent::Status(format!("Não entendi: {e}")));
                    }
                }
            }
        }
    }
    drop(stream);
}

/// A sentence being said: Piper's samples (22.05 kHz mono) played on the speaker as they come, or
/// the system's voice program while Piper isn't there.
pub(crate) struct Player {
    child: Child,
    _stream: Option<cpal::Stream>,
    samples: Arc<Mutex<VecDeque<f32>>>,
    ended: Arc<AtomicBool>,
}

impl Player {
    /// Piper, when `piper` is there; else the system's voice.
    pub(crate) fn say(text: &str, piper: Option<&(PathBuf, PathBuf)>, lang: &str) -> Option<Player> {
        let samples: Arc<Mutex<VecDeque<f32>>> = Arc::default();
        let ended = Arc::new(AtomicBool::new(false));
        let Some((program, voice)) = piper else {
            let child = system_voice(text, lang)?;
            ended.store(true, Ordering::Relaxed);
            return Some(Player { child, _stream: None, samples, ended });
        };
        let mut child = crate::ide::command(program)
            .arg("--model")
            .arg(voice)
            .args(["--output-raw", "--sentence-silence", "0.1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        if let Some(mut stdin) = child.stdin.take() {
            let text = format!("{text}\n");
            std::thread::spawn(move || {
                let _ = stdin.write_all(text.as_bytes());
            });
        }
        let rate: u32 = fs::read_to_string(voice.with_extension("onnx.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v["audio"]["sample_rate"].as_u64())
            .unwrap_or(22_050) as u32;
        let (sink, done) = (samples.clone(), ended.clone());
        if let Some(mut stdout) = child.stdout.take() {
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = stdout.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let mut sink = sink.lock().unwrap();
                    for pair in buf[..n - n % 2].chunks(2) {
                        sink.push_back(i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0);
                    }
                }
                done.store(true, Ordering::Relaxed);
            });
        }
        let stream = play(samples.clone(), rate);
        Some(Player { child, _stream: stream, samples, ended })
    }

    /// Said through: the program ended and nothing is left to play.
    pub(crate) fn done(&mut self) -> bool {
        if self._stream.is_none() {
            return matches!(self.child.try_wait(), Ok(Some(_)));
        }
        self.ended.load(Ordering::Relaxed) && self.samples.lock().unwrap().is_empty()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The speaker fed from `samples` (mono at `rate`), resampled to the device's rate on the fly.
fn play(samples: Arc<Mutex<VecDeque<f32>>>, rate: u32) -> Option<cpal::Stream> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_output_device()?;
    let config = device.default_output_config().ok()?;
    let (out_rate, channels) = (config.sample_rate().0 as f32, config.channels() as usize);
    let step = rate as f32 / out_rate;
    let (mut last, mut next, mut t) = (0f32, 0f32, 1f32);
    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| {
                let mut source = samples.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    t += step;
                    while t >= 1.0 {
                        t -= 1.0;
                        last = next;
                        next = source.pop_front().unwrap_or(0.0);
                    }
                    let v = last * (1.0 - t) + next * t;
                    frame.fill(v);
                }
            },
            |e| eprintln!("alto-falante: {e}"),
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some(stream)
}

/// The system's own voice, as a program that ends when it finishes: PowerShell's (System.Speech)
/// on Windows, `say` on the Mac, speech-dispatcher's `spd-say` on Linux.
fn system_voice(text: &str, lang: &str) -> Option<Child> {
    let mut command = if cfg!(windows) {
        let mut c = crate::ide::command("powershell");
        c.args([
            "-NoProfile",
            "-Command",
            &format!("Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $v = $s.GetInstalledVoices() | Where-Object {{ $_.VoiceInfo.Culture.Name -like '{lang}*' }} | Select-Object -First 1; if ($v) {{ $s.SelectVoice($v.VoiceInfo.Name) }}; $s.Speak([Console]::In.ReadToEnd())"),
        ]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = crate::ide::command("say");
        c.arg("-f").arg("-");
        c
    } else {
        let mut c = crate::ide::command("spd-say");
        c.args(["-w", "-l", lang, "-e"]);
        c
    };
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let text = text.to_string();
        std::thread::spawn(move || {
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    Some(child)
}

/// Starts the call: the downloads and whisper.cpp on a thread, then the microphone on another.
/// Gives the events, the port, and the sender for the app's own notes.
pub(crate) fn start(shared: VoiceShared, setup: VoiceSetup) -> (Receiver<VoiceEvent>, u16, Arc<Mutex<Option<Child>>>, Arc<Mutex<Option<(PathBuf, PathBuf)>>>) {
    let (tx, rx) = channel();
    let port = free_port();
    let server: Arc<Mutex<Option<Child>>> = Arc::default();
    let piper: Arc<Mutex<Option<(PathBuf, PathBuf)>>> = Arc::default();
    let (server_slot, piper_slot) = (server.clone(), piper.clone());
    std::thread::spawn(move || match prepare(&tx, port, &setup) {
        Ok((child, voice)) => {
            *server_slot.lock().unwrap() = Some(child);
            *piper_slot.lock().unwrap() = voice;
            if !shared.stop.load(Ordering::Relaxed) {
                listen(shared, tx, port, setup.pause);
            }
        }
        Err(e) => {
            let _ = tx.send(VoiceEvent::Failed(e));
        }
    });
    (rx, port, server, piper)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_is_cut_by_its_level_with_a_lead_in() {
        let mut s = Segmenter::with_pause(0.8);
        let quiet = vec![0.001f32; BLOCK];
        let loud: Vec<f32> = (0..BLOCK).map(|i| if i % 2 == 0 { 0.3 } else { -0.3 }).collect();
        for _ in 0..50 {
            assert_eq!(s.feed(&quiet), None);
        }
        // 1 s of speech, then quiet: the stretch comes after 0.8 s of it, with 0.3 s before
        let mut got = None;
        for _ in 0..50 {
            assert_eq!(s.feed(&loud), None);
        }
        for _ in 0..45 {
            if let Some(samples) = s.feed(&quiet) {
                got = Some(samples);
                break;
            }
        }
        let samples = got.expect("a stretch of speech");
        assert!(samples.len() >= 50 * BLOCK + 15 * BLOCK && samples.len() <= 50 * BLOCK + 15 * BLOCK + 40 * BLOCK, "{}", samples.len());
        // a short click isn't speech
        let mut s = Segmenter::default();
        for _ in 0..50 {
            s.feed(&quiet);
        }
        for _ in 0..5 {
            s.feed(&loud);
        }
        assert!((0..45).all(|_| s.feed(&quiet).is_none()));
        // with the default pause (2 s) a 1 s pause to think is part of what is said
        let mut s = Segmenter::default();
        for _ in 0..50 {
            s.feed(&quiet);
        }
        let mut cut = 0;
        for block in
            std::iter::repeat_n(&loud, 50).chain(std::iter::repeat_n(&quiet, 50)).chain(std::iter::repeat_n(&loud, 50)).chain(std::iter::repeat_n(&quiet, 110))
        {
            if let Some(samples) = s.feed(block) {
                cut += 1;
                assert!(samples.len() >= 150 * BLOCK, "both halves in one: {}", samples.len());
            }
        }
        assert_eq!(cut, 1);
    }

    #[test]
    fn audio_goes_to_mono_16k_and_wav() {
        let stereo: Vec<f32> = (0..96).flat_map(|i| [i as f32 / 96.0, i as f32 / 96.0]).collect();
        let mono = to_16k(&stereo, 2, 48_000);
        assert_eq!(mono.len(), 32);
        assert!((mono[1] - 3.0 / 96.0).abs() < 1e-5, "{}", mono[1]);
        let file = wav(&[0.0, 0.5, -1.0], 16_000);
        assert_eq!(file.len(), 44 + 6);
        assert_eq!(&file[..4], b"RIFF");
        assert_eq!(i16::from_le_bytes([file[46], file[47]]), 16383);
    }

    #[test]
    fn the_person_commands_the_call_by_the_agents_name() {
        assert_eq!(voice_command("Claude, se muta.", "claude"), Some(VoiceCmd::Mute));
        assert_eq!(voice_command("claude mudo", "claude"), Some(VoiceCmd::Mute));
        assert_eq!(voice_command("Claude, desmuta", "claude"), Some(VoiceCmd::Unmute));
        assert_eq!(voice_command("ei Cloud, pode falar", "claude"), Some(VoiceCmd::Speak));
        assert_eq!(voice_command("Claude?", "claude"), Some(VoiceCmd::Speak));
        assert_eq!(voice_command("Claude, roda os testes de novo", "claude"), Some(VoiceCmd::Ask("roda os testes de novo".into())));
        assert_eq!(voice_command("roda os testes, Claude", "claude"), None, "only when called first");
        assert_eq!(voice_command("Codex, se muta", "codex"), Some(VoiceCmd::Mute));
        assert_eq!(voice_command("Claude, se muta", "codex"), None);
        assert_eq!(piper_voice_url("pt_BR-faber-medium").as_deref(), Some("https://huggingface.co/rhasspy/piper-voices/resolve/main/pt/pt_BR/faber/medium"));
        assert_eq!(
            piper_voice_url("en_US-hfc_female-medium").as_deref(),
            Some("https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/hfc_female/medium")
        );
        assert_eq!(piper_voice_url("errado"), None);
        assert!(VOICES.iter().all(|v| piper_voice_url(v).is_some()));
    }

    #[test]
    fn answers_are_said_a_sentence_at_a_time_without_the_markdown() {
        let text = "## Pronto\n\nFiz **duas** coisas. A primeira foi `cargo test`! Veja:\n```sh\ncargo test\n```\n- um item\n- outro item?\nFim";
        assert_eq!(sentences(text), ["Pronto", "Fiz duas coisas.", "A primeira foi cargo test!", "Veja:", "um item", "outro item?", "Fim"]);
    }

    #[test]
    fn the_call_says_the_summary_without_paths_and_points_to_the_screen() {
        let text = "Corrigi o bug do chat. A mudança está em src/app/chat.rs e no voice.rs:42. Os testes passaram.\n\nDetalhes: veja https://x.dev/a.";
        assert_eq!(spoken(text), ["Corrigi o bug do chat.", "Os testes passaram.", "Os detalhes estão na tela."]);
        assert_eq!(spoken("Pronto, tudo certo."), ["Pronto, tudo certo."]);
        assert_eq!(spoken("Feito.\n```rs\nfn a() {}\n```"), ["Feito.", "Os detalhes estão na tela."]);
        assert!(!has_path("Versão 3.5 do Opus, etc.") && has_path("Abra C:\\x") && has_path("veja chat.rs:42."));
    }

    #[test]
    fn the_call_tones_rise_to_start_and_fall_to_end() {
        let start = chime_samples(&CALL_START, 22_050);
        assert!(start.len() > 22_050 / 2 && start.iter().all(|s| s.abs() < 0.5));
        assert_eq!(CALL_END, [CALL_START[1], CALL_START[0]]);
    }
}
