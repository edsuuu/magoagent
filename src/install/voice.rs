use super::*;

/// What the local voice's download in the background is doing, for Ajustes › Voz; empty when done.
pub(crate) static LOCAL_VOICE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// The local voice (whisper.cpp, Piper) downloads the first time something asks for it (dictation
/// without the OpenAI key); nothing of it comes with the installer. What is there already isn't
/// fetched again, and a second ask while it downloads starts nothing.
// ponytail: a curl still running when the app closes leaves its .part, fetched again next time
pub(crate) fn fetch_voice_in_background() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Some(dir) = crate::voice::voice_dir() else { return };
    std::thread::spawn(move || {
        let say = |line: String| *LOCAL_VOICE.lock().unwrap() = line;
        let end = fetch_voice(&dir, &say).err().unwrap_or_default();
        *LOCAL_VOICE.lock().unwrap() = end;
    });
}

/// Everything the call uses, into `dir`: whisper.cpp (and its CUDA build with NVIDIA on Windows),
/// the small model (and large-v3-turbo with a GPU), Piper and every voice of Ajustes › Voz.
pub(super) fn fetch_voice(dir: &Path, say: &dyn Fn(String)) -> Result<(), String> {
    use crate::voice::*;
    let (_, gpu) = gpu();
    if !cfg!(target_os = "macos") {
        let asset = if cfg!(windows) { "whisper-bin-x64.zip" } else { "whisper-bin-ubuntu-x64.tar.gz" };
        if whisper_server(dir, false).is_none() {
            let archive = dir.join(asset);
            download(say, "o whisper.cpp (10 MB)", &format!("{WHISPER_RELEASE}/{asset}"), &archive)?;
            unpack(&archive, &dir.join("whisper"))?;
        }
        if gpu && cfg!(windows) && whisper_server(dir, true).is_none() {
            let asset = "whisper-bin-win-cuda-12.4.0-x64.zip";
            let archive = dir.join(asset);
            download(say, "o whisper.cpp com CUDA (684 MB)", &format!("{WHISPER_RELEASE}/{asset}"), &archive)?;
            unpack(&archive, &dir.join("whisper-cuda"))?;
        }
    } else if whisper_server(dir, false).is_none() {
        say("No Mac o whisper.cpp vem do Homebrew: brew install whisper-cpp".into());
    }
    download(say, "o modelo de voz (190 MB)", &format!("{WHISPER_MODELS_URL}/{WHISPER_MODEL}"), &dir.join(WHISPER_MODEL))?;
    if gpu {
        download(say, "o modelo de voz grande, para a GPU (550 MB)", &format!("{WHISPER_MODELS_URL}/{WHISPER_TURBO}"), &dir.join(WHISPER_TURBO))?;
    }
    if find_file(&dir.join("piper"), &exe("piper")).is_none() {
        let asset = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", _) => "piper_windows_amd64.zip",
            ("macos", "aarch64") => "piper_macos_aarch64.tar.gz",
            ("macos", _) => "piper_macos_x64.tar.gz",
            (_, "aarch64") => "piper_linux_aarch64.tar.gz",
            _ => "piper_linux_x86_64.tar.gz",
        };
        let archive = dir.join(asset);
        download(say, "o Piper (22 MB)", &format!("{PIPER_RELEASE}/{asset}"), &archive)?;
        unpack(&archive, dir)?;
    }
    for name in VOICES {
        let url = voice_url(name).ok_or("voz sem endereço")?;
        let voice = dir.join("voices").join(format!("{name}.onnx"));
        download(say, &format!("a voz {} (até 110 MB)", voice_name(name)), &url, &voice)?;
        download(say, &format!("a voz {}", voice_name(name)), &format!("{url}.json"), &voice.with_extension("onnx.json"))?;
    }
    // the voices no longer offered go, with their space
    for entry in fs::read_dir(dir.join("voices")).into_iter().flatten().flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        if !VOICES.iter().any(|v| file.starts_with(&format!("{v}.onnx"))) {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

/// Fetches `url` to `to` with curl (there on the three systems), unless it is already there.
pub(super) fn download(say: &dyn Fn(String), what: &str, url: &str, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Ok(());
    }
    say(format!("Baixando {what}…"));
    if let Some(dir) = to.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let part = to.with_extension("part");
    let status = crate::ide::command("curl").args(["-L", "--fail", "-sS", "-o"]).arg(&part).arg(url).status().map_err(|e| format!("curl: {e}"))?;
    if !status.success() {
        let _ = fs::remove_file(&part);
        return Err(format!("Não foi possível baixar {what}. Confira a internet e tente de novo."));
    }
    fs::rename(&part, to).map_err(|e| e.to_string())
}

/// Unpacks `archive` (zip or tar.gz, tar reads both) into `dir`.
pub(super) fn unpack(archive: &Path, dir: &Path) -> Result<(), String> {
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
