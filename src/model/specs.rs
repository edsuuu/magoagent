use super::*;

/// A terminal's shell: the name agents call it by, its starting folder (empty: home) and the
/// command typed once the shell starts (empty: just the shell).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct TerminalSpec {
    pub(crate) name: String,
    pub(crate) cwd: String,
    pub(crate) command: String,
    /// The mark in its title strip (`TERMINAL_ICONS`; empty: by the command) and its tint.
    pub(crate) icon: String,
    pub(crate) color: Option<[u8; 3]>,
    /// "Monitorar atividade" off: no "terminou" when its work ends.
    pub(crate) quiet: bool,
    /// Maestro: may recruit, open portals and connect. Terminals the person makes are Maestros, and
    /// so are scenes saved before this field; agents' children are not.
    pub(crate) maestro: bool,
    /// "Iniciar no modo de chat": a click on it opens the chat on it instead of its screen.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) chat: bool,
    /// Descarregar: no shell until Recarregar; the drawing keeps the rest.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) unloaded: bool,
    /// The Claude Code conversation it ran last (its session id) and the folder Claude Code files
    /// it under (`project_folder`): its chat shows it after a restart, and `launch_command` resumes it.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(crate) claude_session: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(crate) claude_folder: String,
    /// "Retomar a conversa ao reiniciar".
    pub(crate) resume: bool,
    /// Its own theme, font size and shell, and Desfocar (terminal.rs).
    #[serde(skip_serializing_if = "TermLook::is_default")]
    pub(crate) look: TermLook,
}

impl Default for TerminalSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            cwd: String::new(),
            command: String::new(),
            icon: String::new(),
            color: None,
            quiet: false,
            maestro: true,
            chat: false,
            unloaded: false,
            claude_session: String::new(),
            claude_folder: String::new(),
            resume: true,
            look: TermLook::default(),
        }
    }
}

impl TerminalSpec {
    /// Its name, or "Terminal" while it has none.
    pub(crate) fn label(&self) -> &str {
        if self.name.is_empty() { "Terminal" } else { &self.name }
    }

    /// What it types once its shell starts: plain `claude` resumes the conversation it remembers
    /// while that file is still in `claude` (Claude Code's folder) and the option is on.
    // ponytail: the file is looked up where the conversation was filed; a terminal moved to another
    // folder still resumes it, and Claude Code may not find it from there
    pub(crate) fn launch_command(&self, claude: Option<&std::path::Path>) -> String {
        let id = &self.claude_session;
        // typed into a shell: a UUID's characters only, whatever the file says
        let safe = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        match claude {
            Some(dir)
                if self.resume && safe && self.command.trim() == "claude" && crate::transcript::transcript_path(dir, &self.claude_folder, id).is_file() =>
            {
                format!("claude --resume {id}")
            }
            _ => self.command.clone(),
        }
    }
}

/// A web page on the canvas. Its data keeps every tab's address (the bar shows one page for now).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct PortalSpec {
    pub(crate) name: String,
    /// The open tab's address (what agents navigate and read).
    pub(crate) url: String,
    /// Every tab's address; empty means just `url`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) tabs: Vec<String>,
    pub(crate) active: usize,
    /// Hides the header and the address bar, leaving just the page.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) chrome_hidden: bool,
    /// Descarregar: no page until Recarregar (its addresses stay).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) unloaded: bool,
    /// Agente de Usuário: what the portal tells sites; empty is the webview's own.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(crate) user_agent: String,
    /// Armazenamento: whose cookies and logins the portal uses.
    #[serde(skip_serializing_if = "is_default_scope")]
    pub(crate) storage: crate::app::browser::extras::StorageScope,
    /// Silenciar: the page's audio and video are off.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) muted: bool,
}

pub(super) fn is_default_scope(scope: &crate::app::browser::extras::StorageScope) -> bool {
    *scope == Default::default()
}

impl PortalSpec {
    pub(crate) fn tab_urls(&self) -> Vec<String> {
        if self.tabs.is_empty() { vec![self.url.clone()] } else { self.tabs.clone() }
    }

    pub(crate) fn active_tab(&self) -> usize {
        self.active.min(self.tab_urls().len() - 1)
    }

    pub(crate) fn set_tabs(&mut self, tabs: Vec<String>, active: usize) {
        self.active = active.min(tabs.len().saturating_sub(1));
        self.url = tabs.get(self.active).cloned().unwrap_or_default();
        self.tabs = tabs;
    }

    pub(crate) fn set_tab_url(&mut self, tab: usize, url: String) {
        let mut tabs = self.tab_urls();
        if let Some(slot) = tabs.get_mut(tab) {
            *slot = url;
        }
        self.set_tabs(tabs, self.active_tab());
    }
}

/// A sticky note's settings (the card's own; its text is the label element).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct NoteSpec {
    pub(crate) font_size: f32,
    /// Shows the markdown source instead of the formatted text.
    pub(crate) raw: bool,
    /// The text can't be edited (Mais → Bloquear conteúdo).
    pub(crate) content_locked: bool,
    /// On a pile of notes (the notes of a pile share a group), and the page showing in front.
    pub(crate) stacked: bool,
    pub(crate) front: bool,
    /// Desfocar: the text goes behind a veil.
    pub(crate) blurred: bool,
    /// Mover Arquivo para…: where the note's `.md` lives when it is not in `notas/` beside the scene.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) file: Option<String>,
    /// Buttons at the foot of the card that run an app action (an AI makes them).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) buttons: Vec<NoteButton>,
}

/// A note button: its label and the app action (MCP's `act`) with its arguments.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct NoteButton {
    pub(crate) label: String,
    pub(crate) action: String,
    pub(crate) args: serde_json::Value,
}

/// The note text sizes: smallest, largest and the step between them.
pub(crate) const NOTE_FONT_MIN: f32 = 9.0;
pub(crate) const NOTE_FONT_MAX: f32 = 24.0;
pub(crate) const NOTE_FONT_STEP: f32 = 2.0;

impl Default for NoteSpec {
    fn default() -> Self {
        Self {
            font_size: crate::render::markdown::NOTE_FONT,
            raw: false,
            content_locked: false,
            stacked: false,
            front: false,
            blurred: false,
            file: None,
            buttons: Vec::new(),
        }
    }
}

impl NoteSpec {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// An IDE node's size when a click makes it (it is used enlarged, at 100%).
pub(crate) const IDE_SIZE: Vec2 = Vec2::new(300.0, 300.0);

/// A project folder on the canvas: the IDE node (the editor itself lives only while the app runs).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct IdeSpec {
    pub(crate) name: String,
    pub(crate) root: String,
    /// The tree column's width while the file panel is open: its longest name at first, a third
    /// of the node at most, then where its bar is dragged; the node goes back to it on close.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tree_width: Option<f32>,
}
