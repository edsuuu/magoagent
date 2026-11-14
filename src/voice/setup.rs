pub(crate) const SAMPLE_RATE: u32 = 16_000;
/// 20 ms blocks at 16 kHz.
pub(super) const BLOCK: usize = 320;

/// How the call is set up, from Ajustes: the language heard ("pt", "en"), the pause that ends a
/// sentence, the engine that hears it (VoiceStudio or OpenAI) and the microphone.
#[derive(Clone, Debug)]
pub(crate) struct VoiceSetup {
    pub(crate) language: String,
    /// Seconds of quiet that end what the person says (Ajustes › Voz).
    pub(crate) pause: f32,
    /// OpenAI's key when the call hears on its API (voice/openai.rs); after `studio`.
    pub(crate) openai: Option<String>,
    /// The VoiceStudio server when the call hears on it (voice/studio.rs); first.
    pub(crate) studio: Option<crate::voice::studio::StudioServer>,
    /// The microphone by name (Ajustes › Voz); empty: `pick_input`'s.
    pub(crate) mic: String,
}
