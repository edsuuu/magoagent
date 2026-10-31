//! The voice call: the microphone is listened to by its level (speech starts when it rises,
//! ends after 0.8 s of quiet), each stretch of speech goes to whisper.cpp's server (run here as
//! a program, the small model loaded once) and its words come back as a message; what the agent
//! answers is spoken a sentence at a time by Piper (pt-BR voice) through the speaker, or by the
//! system's voice while Piper isn't there. whisper.cpp, Piper and their models are programs and
//! files downloaded into the data folder on the first call, never built into the app.
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
const WHISPER_MODEL: &str = "ggml-small-q5_1.bin";
const WHISPER_MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin";
const WHISPER_RELEASE: &str = "https://github.com/ggml-org/whisper.cpp/releases/download/b5454";
const PIPER_RELEASE: &str = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2";
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
}

impl Default for VoiceSetup {
    fn default() -> Self {
        VoiceSetup { voice: VOICES[0].into(), language: "auto".into(), gpu: true }
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

/// A short tone through the speaker (an answer waits while on standby); the stream plays while kept.
pub(crate) fn tone() -> Option<cpal::Stream> {
    const RATE: u32 = 22_050;
    let samples: VecDeque<f32> = (0..RATE as usize * 3 / 20)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let fade = (1.0 - t / 0.15).max(0.0);
            (t * 880.0 * std::f32::consts::TAU).sin() * 0.25 * fade
        })
        .collect();
    play(Arc::new(Mutex::new(samples)), RATE)
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
/// 60 ms starts it, 0.8 s under it ends it; 0.3 s before the rise comes along. Gives each stretch
/// of at least half a second, 30 s at most.
pub(crate) struct Segmenter {
    floor: f32,
    speech: Vec<f32>,
    pre: VecDeque<Vec<f32>>,
    loud: usize,
    quiet: usize,
    /// Blocks over the floor in the stretch so far: a click alone isn't speech.
    voiced: usize,
    pub(crate) in_speech: bool,
}

impl Default for Segmenter {
    fn default() -> Self {
        Segmenter { floor: 0.01, speech: Vec::new(), pre: VecDeque::new(), loud: 0, quiet: 0, voiced: 0, in_speech: false }
    }
}

impl Segmenter {
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
        if self.quiet < 40 && self.speech.len() < 30 * SAMPLE_RATE as usize {
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
        for ch in plain.chars() {
            if ch == '\n' {
                if !current.trim().is_empty() {
                    out.push(current.trim().to_string());
                }
                current.clear();
                continue;
            }
            current.push(ch);
            if matches!(ch, '.' | '!' | '?' | '…') && current.trim().chars().count() > 2 {
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

/// Where a program's file came out of its archive, by name.
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
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

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

/// Fetches `url` to `to` with curl (there on the three systems), saying so.
fn download(tx: &Sender<VoiceEvent>, what: &str, url: &str, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Ok(());
    }
    let _ = tx.send(VoiceEvent::Status(format!("Baixando {what}…")));
    if let Some(dir) = to.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let part = to.with_extension("part");
    let status = crate::ide::command("curl").args(["-L", "--fail", "-sS", "-o"]).arg(&part).arg(url).status().map_err(|e| format!("curl: {e}"))?;
    if !status.success() {
        let _ = fs::remove_file(&part);
        return Err(format!("Não foi possível baixar {what}."));
    }
    fs::rename(&part, to).map_err(|e| e.to_string())
}

/// Unpacks `archive` (zip or tar.gz, tar reads both) into `dir`.
fn unpack(archive: &Path, dir: &Path) -> Result<(), String> {
    let _ = fs::create_dir_all(dir);
    // Windows' own tar (bsdtar, which opens zips), not one a shell on the PATH may bring
    let tar = match std::env::var_os("SystemRoot").filter(|_| cfg!(windows)) {
        Some(root) => PathBuf::from(root).join("System32").join("tar.exe"),
        None => PathBuf::from("tar"),
    };
    let status = crate::ide::command(tar).arg("-xf").arg(archive).arg("-C").arg(dir).status().map_err(|e| format!("tar: {e}"))?;
    let _ = fs::remove_file(archive);
    if status.success() { Ok(()) } else { Err(format!("Não foi possível abrir {}.", archive.display())) }
}

/// whisper.cpp's server program: downloaded on Windows and Linux (the CUDA build when `gpu` and
/// the computer has NVIDIA), Homebrew's on the Mac (with Metal).
fn whisper_server(tx: &Sender<VoiceEvent>, dir: &Path, gpu: bool) -> Result<PathBuf, String> {
    let cuda = gpu && cfg!(windows) && crate::voice::gpu().1;
    let home = dir.join(if cuda { "whisper-cuda" } else { "whisper" });
    if let Some(found) = find_file(&home, &exe("whisper-server")) {
        return Ok(found);
    }
    if cfg!(target_os = "macos") {
        return crate::ide::lsp::which("whisper-server").ok_or_else(|| "Instale o whisper.cpp: brew install whisper-cpp".to_string());
    }
    let (asset, what) = match (cuda, cfg!(windows)) {
        (true, _) => ("whisper-bin-win-cuda-12.4.0-x64.zip", "o whisper.cpp com CUDA (684 MB, uma vez só)"),
        (false, true) => ("whisper-bin-x64.zip", "o whisper.cpp (8 MB)"),
        (false, false) => ("whisper-bin-ubuntu-x64.tar.gz", "o whisper.cpp (10 MB)"),
    };
    let archive = dir.join(asset);
    download(tx, what, &format!("{WHISPER_RELEASE}/{asset}"), &archive)?;
    unpack(&archive, &home)?;
    find_file(&home, &exe("whisper-server")).ok_or_else(|| "O whisper.cpp baixado não trouxe o whisper-server.".into())
}

/// Piper and the voice chosen; None when the system can't have it (the system's voice speaks).
fn piper(tx: &Sender<VoiceEvent>, dir: &Path, name: &str) -> Option<(PathBuf, PathBuf)> {
    let url = piper_voice_url(name)?;
    let voice = dir.join("voices").join(format!("{name}.onnx"));
    download(tx, &format!("a voz {name} (até 80 MB)"), &format!("{url}/{name}.onnx"), &voice).ok()?;
    download(tx, &format!("a voz {name}"), &format!("{url}/{name}.onnx.json"), &voice.with_extension("onnx.json")).ok()?;
    let program = exe("piper");
    if let Some(found) = find_file(&dir.join("piper"), &program) {
        return Some((found, voice));
    }
    let asset = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => "piper_windows_amd64.zip",
        ("macos", "aarch64") => "piper_macos_aarch64.tar.gz",
        ("macos", _) => "piper_macos_x64.tar.gz",
        (_, "aarch64") => "piper_linux_aarch64.tar.gz",
        _ => "piper_linux_x86_64.tar.gz",
    };
    let archive = dir.join(asset);
    download(tx, "o Piper (22 MB)", &format!("{PIPER_RELEASE}/{asset}"), &archive).ok()?;
    unpack(&archive, dir).ok()?;
    find_file(&dir.join("piper"), &program).map(|p| (p, voice))
}

/// A free local port for whisper.cpp's server.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map_or(8178, |a| a.port())
}

/// Gets everything ready and starts whisper.cpp's server on `port`; the server process comes back
/// so the app can end it.
pub(crate) fn prepare(tx: &Sender<VoiceEvent>, port: u16, setup: &VoiceSetup) -> Result<(Child, Option<(PathBuf, PathBuf)>), String> {
    let dir = voice_dir().ok_or("Sem pasta de dados.")?;
    let server = whisper_server(tx, &dir, setup.gpu)?;
    let model = dir.join(WHISPER_MODEL);
    download(tx, "o modelo de voz (190 MB)", WHISPER_MODEL_URL, &model)?;
    let piper = piper(tx, &dir, &setup.voice);
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
            if setup.gpu && cfg!(windows) && gpu().1 {
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
pub(crate) fn listen(shared: VoiceShared, tx: Sender<VoiceEvent>, port: u16) {
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
    let mut segmenter = Segmenter::default();
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
                segmenter = Segmenter::default();
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
                listen(shared, tx, port);
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
        let mut s = Segmenter::default();
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
}
