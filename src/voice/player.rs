use super::*;

/// A sentence being said: Piper's samples (22.05 kHz mono) played on the speaker as they come, or
/// the system's voice program while Piper isn't there.
pub(crate) struct Player {
    pub(super) child: Child,
    pub(super) _stream: Option<cpal::Stream>,
    pub(super) samples: Arc<Mutex<VecDeque<f32>>>,
    pub(super) ended: Arc<AtomicBool>,
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
        Some(Player::from_pcm(child, rate))
    }

    /// The 16-bit mono samples at `rate` that `child` writes, played as they come.
    pub(crate) fn from_pcm(mut child: Child, rate: u32) -> Player {
        let samples: Arc<Mutex<VecDeque<f32>>> = Arc::default();
        let ended = Arc::new(AtomicBool::new(false));
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
        Player { child, _stream: stream, samples, ended }
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
pub(super) fn play(samples: Arc<Mutex<VecDeque<f32>>>, rate: u32) -> Option<cpal::Stream> {
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
pub(super) fn system_voice(text: &str, lang: &str) -> Option<Child> {
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
    // on OpenAI's API there is nothing to load: the microphone at once
    if let Some(key) = setup.openai.clone() {
        let language = setup.language.clone();
        std::thread::spawn(move || listen(shared, tx, |s| crate::voice::openai::transcribe(&key, &language, s), setup.pause));
        return (rx, port, server, piper);
    }
    std::thread::spawn(move || match prepare(&tx, port, &setup) {
        Ok((child, voice)) => {
            *server_slot.lock().unwrap() = Some(child);
            *piper_slot.lock().unwrap() = voice;
            if !shared.stop.load(Ordering::Relaxed) {
                listen(shared, tx, |s| transcribe(port, s), setup.pause);
            }
        }
        Err(e) => {
            let _ = tx.send(VoiceEvent::Failed(e));
        }
    });
    (rx, port, server, piper)
}
