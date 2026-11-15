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
    /// The mark and tint the terminals made from it wear (`TERMINAL_ICONS`, `TERMINAL_COLORS`; empty and none: by the command).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) icon: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) color: Option<[u8; 3]>,
}

impl Preset {
    pub(crate) fn new(name: &str, command: &str) -> Self {
        Self { name: name.into(), command: command.into(), on: true, icon: String::new(), color: None }
    }
}

pub(crate) fn default_presets() -> Vec<Preset> {
    [("Claude Code", "claude"), ("Codex", "codex"), ("OpenCode", "opencode"), ("Shell", "")].map(|(name, command)| Preset::new(name, command)).to_vec()
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
    /// Dispositivo de entrada = Automático: each scroll says whether a mouse wheel or a touchpad sent it
    /// (`wheel_from_mouse`), and `mouse_wheel_zooms` is not looked at. Off by default: Trackpad (asked).
    pub(crate) wheel_auto: bool,
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
    /// The hidden names (".x") a file tree still lists with its hidden files off, comma-separated: PhpStorm's
    /// exceptions (".git" stays hidden, ".claude" shows).
    pub(crate) hidden_shown: String,
    /// The outline of a selected window (Ajustes › Geral › Tela).
    pub(crate) window_outline: WindowOutline,
    /// The app opens on the chat (on by default) or on the canvas.
    pub(crate) start_in_chat: bool,
    /// Fechar mantém em segundo plano: the window's close hides it and the terminals and agents keep running;
    /// the tray (Windows) or opening the app again brings it back, Ctrl+Q quits.
    pub(crate) close_to_background: bool,
    pub(crate) pause_alerts_until: Option<u64>,
    pub(crate) run_at_login: bool,
    /// A new chat's folder ("" is `projects_dir`, inside the app's data) and leave (`CHAT_MODES`).
    pub(crate) chat_folder: String,
    pub(crate) folders: Folders,
    /// What holds the Maestro back (app/usage.rs): how many agents may run when it recruits (0: no
    /// ceiling), the use of the plan's 5-hour window from which it recruits nobody (0: never stops),
    /// and the model its recruits run (Sonnet by default; "" is the CLI's own).
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
    /// The person's style skill (a SKILL.md) that the flow's Padronização step reads; `<home>/Skill/SKILL.md` at first.
    pub(crate) style_skill: String,
    /// The avatar, the wizard in a window of its own above everything on the screen (avatar.rs).
    pub(crate) avatar: bool,
    /// Where the avatar's bar was left (screen, top-left); none: it follows the window being worked in.
    pub(crate) avatar_at: Option<[f32; 2]>,
    /// The island: the Maestro in a pill at the top of the screen, over every window (app/island/).
    pub(crate) island: bool,
    /// Where the island was left (screen): its centre x and its top.
    pub(crate) island_at: Option<[f32; 2]>,
    /// The conversation in the island, as big as the person pulled it (width, height).
    pub(crate) island_chat: Option<[f32; 2]>,
    /// The island plays its sounds (its sound button).
    pub(crate) island_sound: bool,
    /// The folders the island's "+" offers, most recent first (app/island/forms.rs).
    pub(crate) island_projects: Vec<String>,
    /// Volume of the island's sounds (0.0 to 0.2; default 0.1).
    pub(crate) island_volume: f32,
    /// Seconds before the open island folds after the pointer leaves.
    pub(crate) island_close_after: u8,
    /// Background colour of the island, RGB.
    pub(crate) island_color: [u8; 3],
    /// The island's global shortcuts the person changed, by id (app/hotkeys/); the rest keep their default keys.
    pub(crate) shortcuts: crate::app::hotkeys::Bindings,
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
    /// The OpenAI key of the call's OpenAI engine (Ajustes › Voz).
    // ponytail: plain text in prefs.json; move it to the cofre once it lands
    pub(crate) openai_key: String,
    /// The Maestro's language, an index in `LANGUAGES`: 0 follows whatever the person speaks.
    pub(crate) language: usize,
    /// Seconds of quiet that end what the person says on the call: a shorter pause is part of it.
    pub(crate) voice_pause: f32,
    /// The microphone the call and Ditar hear, by name; empty: the system's, unless that is a virtual cable.
    pub(crate) voice_mic: String,
    /// Ajustes › Ok Mago: "Ok Mago, …" calls the Maestro in a call, even on standby; and the call says the answers out loud (off:
    /// it starts with the speakers off, as the button does).
    pub(crate) ok_mago: bool,
    pub(crate) speak_answers: bool,
    /// The call hears and speaks on OpenAI's API (voice/openai.rs), in `openai_voice`; off, it is the
    /// VoiceStudio server (also what an old prefs.json that chose the local voice gets).
    pub(crate) voice_openai: bool,
    pub(crate) openai_voice: String,
    /// The VoiceStudio server of the call (voice/studio.rs): its address, API key (empty: none) and the voice
    /// chosen from its list.
    pub(crate) voice_studio_url: String,
    pub(crate) voice_studio_key: String,
    pub(crate) voice_studio_voice: String,
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
    /// Ajustes › Email: the mail MCP server the chats' agents get (app/chat/email.rs).
    pub(crate) email: crate::app::chat::email::EmailPrefs,
    /// Ajustes › Dicionário: the words the ears spell right and what they write for what they hear.
    pub(crate) dictionary: crate::voice::dictionary::Dictionary,
    /// Ajustes › Integrações: the switches and what is not secret (a token lives in the keychain).
    pub(crate) integrations: crate::app::integrations::IntegrationPrefs,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            terminal_history: 10_000,
            box_overlap: false,
            mouse_wheel_zooms: false,
            wheel_auto: false,
            grid: false,
            paper: true,
            show_hints: true,
            theme: Theme::Dark,
            presets: default_presets(),
            minimap: false,
            ligatures: true,
            hidden_shown: ".claude, .idea".into(),
            window_outline: WindowOutline::Animated,
            start_in_chat: true,
            close_to_background: true,
            pause_alerts_until: None,
            run_at_login: false,
            chat_folder: String::new(),
            folders: Folders::default(),
            max_agents: 6,
            usage_stop: 90,
            // the recruits run on Sonnet unless Ajustes says otherwise ("cli" goes back to the CLI's own)
            recruit_model: "sonnet".into(),
            agent_mode: AgentMode::Terminal,
            chat_mode: 0,
            update_checked_day: 0,
            chat_panel: false,
            chat_autocompact: String::new(),
            chat_agent: "claude".into(),
            style_skill: std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|home| PathBuf::from(home).join("Skill").join("SKILL.md").to_string_lossy().into_owned())
                .unwrap_or_default(),
            avatar: false,
            avatar_at: None,
            island: false,
            island_at: None,
            island_chat: None,
            island_sound: true,
            island_projects: Vec::new(),
            island_volume: 0.1,
            island_close_after: 15,
            island_color: [0, 0, 0],
            shortcuts: Default::default(),
            call_popup: None,
            mini_chat_at: None,
            mini_chat_window: None,
            mini_pill_window: None,
            mini_pill_at: None,
            openai_key: String::new(),
            language: 0,
            voice_pause: 2.0,
            voice_mic: String::new(),
            ok_mago: true,
            speak_answers: true,
            voice_openai: true,
            openai_voice: "marin".into(),
            voice_studio_url: crate::voice::studio::STUDIO_URL.into(),
            voice_studio_key: String::new(),
            voice_studio_voice: crate::voice::studio::STUDIO_VOICE.into(),
            voice_codex: true,
            codex_voice: "juniper".into(),
            agent_hooks: false,
            sidebar_hidden: false,
            portal_size: Default::default(),
            zoom_step: 10,
            terminal: TermPrefs::default(),
            integrations: Default::default(),
            dictionary: Default::default(),
            email: Default::default(),
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

/// The blue outline of a selected window (terminal, note, portal, chat, tree): dashes that crawl, still
/// dashes, or none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WindowOutline {
    #[default]
    Animated,
    Still,
    Hidden,
}

impl WindowOutline {
    pub(crate) const ALL: [WindowOutline; 3] = [WindowOutline::Animated, WindowOutline::Still, WindowOutline::Hidden];
    pub(crate) const NAMES: [&str; 3] = ["Traçado animado", "Traçado estático", "Sem traçado"];

    /// Its MCP name: animated, still or none.
    pub(crate) fn parse(name: &str) -> Option<WindowOutline> {
        match name {
            "animated" | "animado" => Some(WindowOutline::Animated),
            "still" | "static" | "estático" | "estatico" => Some(WindowOutline::Still),
            "none" | "hidden" | "sem" => Some(WindowOutline::Hidden),
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
