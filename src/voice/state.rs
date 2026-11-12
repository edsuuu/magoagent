use super::*;

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
            VoiceState::Preparing => "Iniciando chat por voz",
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
    /// The person talked over the agent: what is being said stops.
    Interrupt,
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
    /// On Codex's voice (voice/codex.rs): `codex app-server`, the page's WebRTC offer and the
    /// answer on their way, and whether the voice is being heard on the speaker.
    pub(crate) codex: Option<crate::voice::codex::CodexVoice>,
    pub(crate) offer: Option<String>,
    pub(crate) answer: Option<String>,
    pub(crate) talking: bool,
    pub(crate) working: bool,
    /// "se muta" asked if it should: the next yes puts it on standby.
    pub(crate) asked_mute: bool,
    /// On Codex's voice for a Claude chat or a terminal: what is heard goes to that agent, its
    /// answers are spoken by the voice.
    pub(crate) relay: bool,
    /// The speakers are silenced: answers aren't said (Codex's voice: the page plays muted).
    pub(crate) speakers_off: bool,
    /// When the call connected, for the orb's launch; and why one couldn't start (title, text,
    /// whether the microphone was refused), shown until Tentar novamente or Voltar.
    pub(crate) connected_at: Option<f64>,
    pub(crate) launch_failed: Option<(String, String, bool)>,
    /// Where the call's popup opened (screen, top-left); the system moves it from there.
    pub(crate) popup_at: Option<Pos2>,
    /// The popup's window lost the system's border and rounded corners (Windows 11).
    pub(crate) popup_plain: bool,
}

/// The page's word when the system refuses it the microphone.
pub(crate) const MIC_DENIED: &str = "O acesso ao microfone é necessário para iniciar o chat por voz";

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
        self.codex = None;
        self.offer = None;
        self.answer = None;
        self.talking = false;
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
