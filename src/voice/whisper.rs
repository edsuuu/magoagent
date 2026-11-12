use super::*;

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
pub(super) fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map_or(8178, |a| a.port())
}

/// Gets everything ready and starts whisper.cpp's server on `port`; the server process comes back
/// so the app can end it.
pub(crate) fn prepare(tx: &Sender<VoiceEvent>, port: u16, setup: &VoiceSetup) -> Result<(Child, Option<(PathBuf, PathBuf)>), String> {
    const MISSING: &str = "A voz local está baixando agora, em segundo plano (só na primeira vez); tente daqui a pouco.";
    let dir = voice_dir().ok_or("Sem pasta de dados.")?;
    // nothing of it comes with the app: the first ask fetches it
    if whisper_server(&dir, false).is_none() || !dir.join(WHISPER_MODEL).is_file() {
        crate::install::fetch_voice_in_background();
    }
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

/// The microphone, until `stop`: what is said is written down by `hear` and goes as `Heard`.
/// Runs on its own thread.
pub(crate) fn listen(shared: VoiceShared, tx: Sender<VoiceEvent>, hear: impl Fn(&[f32]) -> Result<String, String>, pause: f32) {
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
    // while the agent speaks, only someone talking over it gets through (and stops it)
    let (mut barge, mut broke_in) = (BargeIn::default(), false);
    while !shared.stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let taken = std::mem::take(&mut *raw.lock().unwrap());
        pending.extend(to_16k(&taken, channels, rate));
        let muted = shared.muted.load(Ordering::Relaxed);
        let hold = shared.hold.load(Ordering::Relaxed);
        if !hold {
            (barge, broke_in) = (BargeIn::default(), false);
        }
        while pending.len() >= BLOCK {
            let block: Vec<f32> = pending.drain(..BLOCK).collect();
            let peak = block.iter().fold(0f32, |m, s| m.max(s.abs()));
            shared.level.store(peak.to_bits(), Ordering::Relaxed);
            if hold && !broke_in && !muted && barge.feed(&block) {
                broke_in = true;
                let _ = tx.send(VoiceEvent::Interrupt);
                // what was said while it was noticed starts the stretch
                segmenter = Segmenter::with_pause(pause);
                for kept in barge.kept.drain(..) {
                    segmenter.feed(&kept);
                }
                continue;
            }
            if muted || (hold && !broke_in) {
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
                match hear(&samples) {
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
