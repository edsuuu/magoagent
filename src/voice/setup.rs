use super::*;

pub(crate) const SAMPLE_RATE: u32 = 16_000;
/// 20 ms blocks at 16 kHz.
pub(super) const BLOCK: usize = 320;
/// whisper.cpp's models: the small one on the CPU, large-v3-turbo (far better Portuguese) on a GPU.
pub(crate) const WHISPER_MODEL: &str = "ggml-small-q5_1.bin";
pub(crate) const WHISPER_TURBO: &str = "ggml-large-v3-turbo-q5_0.bin";
pub(crate) const WHISPER_MODELS_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
pub(crate) const WHISPER_RELEASE: &str = "https://github.com/ggml-org/whisper.cpp/releases/download/b5454";
pub(crate) const PIPER_RELEASE: &str = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2";
pub(super) const PIPER_VOICES_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main";
/// The voices offered in Ajustes › Voz, Brazilian Portuguese only for now (locale-speaker-quality,
/// as Piper names them; "ovos" are OpenVoiceOS's, which have the women's voice Piper's own set
/// lacks).
// ponytail: pt-BR only; English voices (en_US-amy-medium, …) go back here when the app speaks it
pub(crate) const VOICES: [&str; 5] = ["pt_BR-dii-ovos", "pt_BR-faber-medium", "pt_BR-cadu-medium", "pt_BR-jeff-medium", "pt_BR-miro-ovos"];
pub(super) const FEMALE: [&str; 1] = ["dii"];
pub(super) const OVOS_URL: &str = "https://huggingface.co/OpenVoiceOS";

/// "pt_BR-dii-ovos" → "Dii (feminina)".
pub(crate) fn voice_name(voice: &str) -> String {
    let speaker = voice.split('-').nth(1).unwrap_or(voice);
    let name: String = speaker.chars().take(1).flat_map(char::to_uppercase).chain(speaker.chars().skip(1)).collect();
    format!("{name} ({})", if FEMALE.contains(&speaker) { "feminina" } else { "masculina" })
}

/// How the call is set up, from Ajustes: the Piper voice, the language whisper hears ("auto",
/// "pt", "en") and whether to try the GPU build of whisper.cpp.
#[derive(Clone, Debug)]
pub(crate) struct VoiceSetup {
    pub(crate) voice: String,
    pub(crate) language: String,
    pub(crate) gpu: bool,
    /// Seconds of quiet that end what the person says (Ajustes › Voz).
    pub(crate) pause: f32,
    /// OpenAI's key when the call hears on its API (voice/openai.rs) instead of whisper.cpp.
    pub(crate) openai: Option<String>,
}

impl Default for VoiceSetup {
    fn default() -> Self {
        VoiceSetup { voice: VOICES[0].into(), language: "pt".into(), gpu: true, pause: 2.0, openai: None }
    }
}

/// A voice's .onnx (its .json is the same plus ".json"), from its name: Piper's own
/// ("pt_BR-faber-medium" → pt/pt_BR/faber/medium/…) or OpenVoiceOS's (CC BY-NC-ND: personal use).
pub(crate) fn voice_url(voice: &str) -> Option<String> {
    let (locale, rest) = voice.split_once('-')?;
    let (speaker, quality) = rest.rsplit_once('-')?;
    if quality == "ovos" {
        let tag = locale.replace('_', "-");
        return Some(format!("{OVOS_URL}/pipertts_{tag}_{speaker}/resolve/main/{speaker}_{tag}.onnx"));
    }
    let lang = locale.split('_').next()?;
    Some(format!("{PIPER_VOICES_URL}/{lang}/{locale}/{speaker}/{quality}/{voice}.onnx"))
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
