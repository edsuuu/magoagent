use super::*;

/// Screen unchanged this long after an ask: the agent finished its turn.
pub(super) const IDLE: Duration = Duration::from_secs(3);
/// No reaction at all this long after an ask: give back what is there.
pub(super) const NO_REACTION: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum NodeKind {
    Terminal,
    Note,
    Portal,
    /// A chat's card (app/chat/node.rs): the Maestro, an agent too, with no terminal.
    Chat,
    /// A file tree (app/filetree.rs): roped to an agent, the one its `file_tree` means (bridge/tree.rs).
    Tree,
}

/// An element agents can reach.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node {
    pub(crate) id: String,
    pub(crate) kind: NodeKind,
    pub(crate) name: String,
    /// A note's text, a portal's address, an IDE's project folder.
    pub(crate) text: String,
    /// What lines bound at both ends connect it to.
    pub(crate) links: Vec<String>,
    /// A terminal's Maestro flag: only a Maestro recruits, opens portals and connects.
    pub(crate) maestro: bool,
}

/// What the UI thread publishes every frame for the bridge threads.
#[derive(Default)]
pub(crate) struct Shared {
    /// Live terminal credentials, revoked when their shell ends.
    pub(crate) terminal_tokens: HashMap<String, String>,
    pub(crate) nodes: Vec<Node>,
    /// The workspaces off view, by id: their name and their nodes as they were when they left the
    /// screen. Their terminals still reach each other (`list`, `check`, `ask`, `note_read`).
    pub(crate) away: HashMap<String, (String, Vec<Node>)>,
    pub(crate) terminals: HashMap<String, TermHandle>,
    /// `ask`s in flight (asking terminal, asked terminal): their rope lights up meanwhile.
    pub(crate) asking: Vec<(String, String)>,
    /// The Claude Code transcript each terminal writes: an `ask` answers with what was added there.
    pub(crate) transcripts: HashMap<String, PathBuf>,
    /// The last prompts typed by an `ask` (asked terminal, prompt, who asked): the chat shows them
    /// as that agent's, not the person's.
    pub(crate) asked: Vec<(String, String, String)>,
    /// The messages the bridge carried, for the app's events (it takes them): who sent, who got
    /// it ("" is outside the canvas), what.
    pub(crate) events: Vec<(String, String, String)>,
    /// What each terminal's agent is doing, as the app last read it (`check_agents`).
    pub(crate) states: HashMap<String, AgentState>,
    /// The chats, by id: what each is called (the Maestro). One speaks by a token of its own while
    /// its agent answers.
    pub(crate) chats: HashMap<String, String>,
}

/// How an `ask` ended, for the asked agent's events.
#[derive(Debug, PartialEq)]
pub(crate) enum Asked {
    Answered,
    StillWorking,
    /// Stopped on a question in its own terminal (a menu, its session's word).
    Waiting,
    Exited,
    /// Its screen never moved.
    Silent,
}

/// Changes only the UI thread can make (it owns the scene and the pages).
#[derive(Debug)]
pub(crate) enum UiCommand {
    PortalNavigate {
        id: String,
        url: String,
    },
    /// Runs a script in a portal's page and sends back its JSON-encoded result.
    PortalEval {
        id: String,
        script: String,
        reply: mpsc::Sender<Result<String, String>>,
    },
    PortalHistory {
        id: String,
        action: String,
    },
    /// A picture of a portal's page, a PNG in base64 (app/browser/shot.rs).
    PortalShot {
        id: String,
        reply: mpsc::Sender<Result<String, String>>,
    },
    WriteNote {
        id: String,
        text: String,
    },
    /// A new sticky note next to terminal `from`, connected to it.
    CreateNote {
        from: String,
        text: String,
    },
    /// The Maestro's moves: a new agent below `from`, a portal to its right, a rope between two
    /// things, a notification. What they create is roped to `from`.
    Recruit {
        from: String,
        command: String,
        /// In a git worktree of its own (app/environments.rs).
        worktree: bool,
        /// Terminal or chat, over Ajustes' `agent_mode`; None: Ajustes decides.
        mode: Option<AgentMode>,
        reply: mpsc::Sender<Result<String, String>>,
    },
    CreatePortal {
        from: String,
        url: String,
        name: String,
        reply: mpsc::Sender<Result<String, String>>,
    },
    Connect {
        a: String,
        b: String,
        reply: mpsc::Sender<Result<String, String>>,
    },
    Notify {
        from: String,
        text: String,
    },
    /// The Maestro's (the chat's agent) moves on the app: show a screen, move the conversation to a
    /// folder, a screenshot of the app (PNG in base64), the diff of a folder in the IDE.
    Show {
        screen: String,
    },
    SetFolder {
        path: String,
        reply: mpsc::Sender<Result<String, String>>,
    },
    Screenshot {
        reply: mpsc::Sender<Result<String, String>>,
    },
    Review {
        path: String,
        reply: mpsc::Sender<Result<String, String>>,
    },
    /// The canvas shown, centered on `at` (ids or names; empty: everything), at `zoom` or fitting.
    Look {
        at: String,
        zoom: Option<f32>,
        reply: mpsc::Sender<Result<String, String>>,
    },
    /// One of the app's actions by name, with its JSON arguments (app/actions.rs).
    Act {
        action: String,
        args: Value,
        reply: mpsc::Sender<Result<String, String>>,
    },
    /// What `magoagent hook` relayed (events.rs); a permission request waits on `reply` for the
    /// answer to print (empty: none, the CLI asks in its terminal).
    Hook {
        from: String,
        payload: Value,
        reply: Option<mpsc::Sender<Result<String, String>>>,
    },
    /// A question for a chat, from `from` (named `asker`): it waits its turn there and the chat's
    /// answer comes back on `reply` (app/chat/ask.rs).
    AskChat {
        id: String,
        from: String,
        asker: String,
        text: String,
        reply: mpsc::Sender<Result<String, String>>,
    },
    /// `check` on a chat: its state and the end of its conversation.
    CheckChat {
        id: String,
        reply: mpsc::Sender<Result<String, String>>,
    },
}

pub(crate) struct Bridge {
    pub(crate) address: String,
    pub(crate) token: String,
    pub(crate) shared: Arc<Mutex<Shared>>,
    pub(crate) commands: mpsc::Receiver<UiCommand>,
}

impl Bridge {
    pub(crate) fn start(ctx: Option<Context>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?.to_string();
        let token = new_id();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (sender, commands) = mpsc::channel();
        let (thread_shared, thread_token) = (shared.clone(), token.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (shared, token, sender, ctx) = (thread_shared.clone(), thread_token.clone(), sender.clone(), ctx.clone());
                // one thread per request: an ask can wait minutes for an agent
                std::thread::spawn(move || serve(stream, &shared, &token, &sender, ctx.as_ref()));
            }
        });
        Ok(Self { address, token, shared, commands })
    }

    /// For MCP clients outside the canvas; readable by this user only.
    pub(crate) fn write_endpoint(&self) {
        let Some(path) = endpoint_path() else { return };
        let json = json!({ "address": self.address, "token": self.token }).to_string();
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        if let Ok(mut file) = options.open(path) {
            // `mode` only applies to a new file: an older one keeps the permissions it had
            #[cfg(unix)]
            let _ = file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600));
            let _ = file.write_all(json.as_bytes());
        }
    }
}

/// A note is called by its first line, without markdown marks.
pub(crate) fn note_name(text: &str) -> String {
    // emphasis wraps both ends ("**negrito**"), the block marks only start the line
    let first = text.lines().map(|l| l.trim().trim_start_matches(['#', '-', '>', ' ']).trim_matches(['*', '_', '~', '`', ' '])).find(|l| !l.is_empty());
    first.map_or("Nota".into(), |l| l.chars().take(40).collect::<String>().trim().to_string())
}

pub(crate) fn endpoint_path() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("bridge.json"))
}

/// The app of this scene answers at the address it left in `bridge.json` (MAGOAGENT_BRIDGE is not
/// asked: a test instance started from a canvas terminal has a scene of its own).
// ponytail: a port another program took after a crash reads as open; upgrade: send `list` with the token
pub(crate) fn app_open(endpoint: &Path) -> bool {
    let saved = fs::read_to_string(endpoint).ok().and_then(|d| serde_json::from_str::<serde_json::Value>(&d).ok());
    let address = saved.and_then(|s| s["address"].as_str()?.parse::<std::net::SocketAddr>().ok());
    address.is_some_and(|a| TcpStream::connect_timeout(&a, Duration::from_millis(500)).is_ok())
}
