use super::*;

pub(crate) fn default_background() -> [u8; 4] {
    WHITE
}

/// The `.mago` format this build reads and writes. Bump it when an old build would lose data
/// rewriting a new file: old builds then only read such files.
pub(crate) const SCENE_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Scene {
    pub(crate) version: u32,
    pub(crate) elements: Vec<Element>,
    #[serde(default = "default_background")]
    pub(crate) background: [u8; 4],
    /// The AI client that wrote this file through the MCP server, shown while the app
    /// replays what it drew.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent: Option<String>,
    /// The canvas terminal that AI runs in, when it does: what it draws gets tied to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent_terminal: Option<String>,
    /// The canvas groups: a name and a color for the elements whose `frame_group` is the id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) groups: Vec<FrameGroup>,
}

/// A named dashed frame around the elements that point to it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct FrameGroup {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) color: Option<[u8; 4]>,
}

impl Default for Scene {
    fn default() -> Self {
        Self { version: SCENE_VERSION, elements: Vec::new(), background: WHITE, agent: None, agent_terminal: None, groups: Vec::new() }
    }
}

/// A quick start of the new terminal dialog (Ajustes → Terminal → Início rápido).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Preset {
    pub(crate) name: String,
    pub(crate) command: String,
    /// Shown in the dialog (the first 10 that are).
    pub(crate) on: bool,
}

pub(crate) fn default_presets() -> Vec<Preset> {
    [("Claude Code", "claude"), ("Codex", "codex"), ("Shell", "")]
        .map(|(name, command)| Preset { name: name.into(), command: command.into(), on: true })
        .to_vec()
}

/// The folders a folder field offers: the last ones used and the starred ones (`app/ui/folders.rs`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct Folders {
    pub(crate) recent: Vec<String>,
    pub(crate) favorite: Vec<String>,
}

/// Main menu → Preferências (DefaultItems.tsx Preferences), remembered between launches.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct Prefs {
    pub(crate) terminal_history: usize,
    pub(crate) box_overlap: bool,
    pub(crate) mouse_wheel_zooms: bool,
    /// Snap to the 20 px grid while drawing and moving (Cmd+').
    pub(crate) grid: bool,
    /// The canvas as graph paper (Ajustes › Fundo: Grade), the reference app's default.
    pub(crate) paper: bool,
    pub(crate) show_hints: bool,
    pub(crate) theme: Theme,
    pub(crate) presets: Vec<Preset>,
    /// The minimap stays as the person left it; closed at first.
    pub(crate) minimap: bool,
    /// Ligaduras no código (=>, !=, ::) in the IDE's JetBrains Mono; off, as in JetBrains IDEs.
    pub(crate) ligatures: bool,
    /// The app opens on the chat (on by default) or on the canvas.
    pub(crate) start_in_chat: bool,
    /// Fechar mantém em segundo plano: the window's close hides it and the terminals and agents keep running;
    /// the tray (Windows) or opening the app again brings it back, Ctrl+Q quits.
    pub(crate) close_to_background: bool,
    /// A new chat's folder ("" is `projects_dir`, inside the app's data) and leave (`CHAT_MODES`).
    pub(crate) chat_folder: String,
    pub(crate) folders: Folders,
    /// What holds the Maestro back (app/usage.rs): how many agents may run when it recruits (0: no
    /// ceiling), the use of the plan's 5-hour window from which it recruits nobody (0: never stops),
    /// and the model its recruits run ("" is the CLI's own).
    pub(crate) max_agents: usize,
    pub(crate) usage_stop: u8,
    pub(crate) recruit_model: String,
    /// How the agents the Maestro recruits run: in a terminal (Claude Code whole) or as a light chat of the app.
    pub(crate) agent_mode: AgentMode,
    pub(crate) chat_mode: usize,
    /// The day (since 1970) the app last looked for a newer release: once a day when it opens.
    pub(crate) update_checked_day: u64,
    /// O painel Contexto do chat aberto, e o valor de "Compactar automaticamente em" (vazio: o padrão do modelo).
    pub(crate) chat_panel: bool,
    pub(crate) chat_autocompact: String,
    /// The program of a new chat's agent: "claude" or "codex".
    pub(crate) chat_agent: String,
    /// The avatar, the wizard in a window of its own above everything on the screen (avatar.rs).
    pub(crate) avatar: bool,
    /// Where the avatar's bar was left (screen, top-left); none: it follows the window being worked in.
    pub(crate) avatar_at: Option<[f32; 2]>,
    /// Where the call's popup was left (screen, top-left); none: over the app's box.
    pub(crate) call_popup: Option<[f32; 2]>,
    /// Where the mini chat was left over the canvas (its top left, from the canvas's own); none: top right.
    pub(crate) mini_chat_at: Option<[f32; 2]>,
    /// Where its pill was left (its top right, from the canvas's top left); none: top right.
    pub(crate) mini_pill_at: Option<[f32; 2]>,
    /// The mini chat's own window as it was left: x, y (screen) and its width and height.
    pub(crate) mini_chat_window: Option<[f32; 4]>,
    /// The pill's own window as it was left: its top left on the screen (it goes out of the app's).
    pub(crate) mini_pill_window: Option<[f32; 2]>,
    /// API keys (Ajustes › Chaves de API) for what runs on an API instead of the CLIs' own logins.
    // ponytail: plain text in prefs.json; move them to the cofre once it lands
    pub(crate) anthropic_key: String,
    pub(crate) openai_key: String,
    /// The chats run claude/codex with the keys above instead of their own logins (settings.rs).
    pub(crate) use_api_keys: bool,
    /// The Maestro's language, an index in `LANGUAGES`: 0 follows whatever the person speaks.
    pub(crate) language: usize,
    /// The call's voice (a Piper voice name, `voice::VOICES`) and whether whisper.cpp tries the GPU.
    pub(crate) voice: String,
    pub(crate) voice_gpu: bool,
    /// Seconds of quiet that end what the person says on the call: a shorter pause is part of it.
    pub(crate) voice_pause: f32,
    /// The call hears and speaks on OpenAI's API (voice/openai.rs), in `openai_voice`; off, it is
    /// whisper.cpp and Piper on this computer.
    pub(crate) voice_openai: bool,
    pub(crate) openai_voice: String,
    /// Before both: the call on Codex's own voice, on the ChatGPT login (voice/codex.rs), in
    /// `codex_voice`; not on Linux, which has no page for its sound.
    pub(crate) voice_codex: bool,
    pub(crate) codex_voice: String,
    /// The Claude Code the app starts reports each step to it by hooks (events.rs); off by default.
    pub(crate) agent_hooks: bool,
    /// The workspaces' sidebar folded away (ui/sidebar.rs).
    pub(crate) sidebar_hidden: bool,
    /// Mostrar Tamanho: when a portal's header shows its width × height.
    pub(crate) portal_size: crate::app::browser::extras::SizeReadout,
    /// Points of zoom a notch of the wheel adds (Ajustes › Geral › Tela), 2 to 50.
    pub(crate) zoom_step: u8,
    /// Every terminal's defaults: theme, font size, shell, the size of a new one, copy on select.
    pub(crate) terminal: TermPrefs,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            terminal_history: 10_000,
            box_overlap: false,
            mouse_wheel_zooms: false,
            grid: false,
            paper: true,
            show_hints: true,
            theme: Theme::Dark,
            presets: default_presets(),
            minimap: false,
            ligatures: true,
            start_in_chat: true,
            close_to_background: true,
            chat_folder: String::new(),
            folders: Folders::default(),
            max_agents: 6,
            usage_stop: 90,
            recruit_model: String::new(),
            agent_mode: AgentMode::Terminal,
            chat_mode: 0,
            update_checked_day: 0,
            chat_panel: false,
            chat_autocompact: String::new(),
            chat_agent: "claude".into(),
            avatar: false,
            avatar_at: None,
            call_popup: None,
            mini_chat_at: None,
            mini_chat_window: None,
            mini_pill_window: None,
            mini_pill_at: None,
            anthropic_key: String::new(),
            openai_key: String::new(),
            use_api_keys: false,
            language: 0,
            voice: "pt_BR-faber-medium".into(),
            voice_gpu: true,
            voice_pause: 2.0,
            voice_openai: true,
            openai_voice: "marin".into(),
            voice_codex: true,
            codex_voice: "juniper".into(),
            agent_hooks: false,
            sidebar_hidden: false,
            portal_size: Default::default(),
            zoom_step: 10,
            terminal: TermPrefs::default(),
        }
    }
}

pub(crate) const GRID: f32 = 20.0;

/// How a recruited agent is invoked (Ajustes › Agentes, `recruit_agent`'s `mode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentMode {
    #[default]
    Terminal,
    Chat,
}

impl AgentMode {
    /// "terminal" or "chat"; None for anything else.
    pub(crate) fn parse(text: &str) -> Option<AgentMode> {
        match text.trim().to_lowercase().as_str() {
            "terminal" => Some(AgentMode::Terminal),
            "chat" => Some(AgentMode::Chat),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Theme {
    Light,
    #[default]
    Dark,
    System,
}
