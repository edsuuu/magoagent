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
    /// A file tree (app/filetree.rs): roped to an agent, the one its `file_tree` means (mcp/bridge/tree.rs).
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
    /// What the person wrote to sessions outside the app (maestro/message.rs), by session id, in order: the session's
    /// next `Stop` hook takes it all and Claude Code goes on with it (`relay_hook`).
    pub(crate) outbox: HashMap<String, Vec<String>>,
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
    /// The app opened again while its window was hidden (Fechar mantém em segundo plano): the window comes back.
    Raise,
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
    /// A decision a chat's agent waits for (its `--permission-prompt-tool`, the MCP's `ask_person`): the
    /// tool and its input; the answer for Claude Code goes back on `reply` (app/chat/decide.rs).
    AskPerson {
        from: String,
        request: Value,
        reply: mpsc::Sender<Result<String, String>>,
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

/// The shared state, even after a thread panicked holding it: one panic must not cascade into every request.
pub(crate) fn locked(shared: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
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

    /// For MCP clients outside the canvas, and (in the user's own data folder, a fixed place) for the person's hooks;
    /// readable by this user only. An instance of a scene of its own (MAGOAGENT_SCENE, a test) leaves the fixed one alone,
    /// unless MAGOAGENT_HOOK_ENDPOINT gives it one of its own.
    pub(crate) fn write_endpoint(&self) {
        let json = json!({ "address": self.address, "token": self.token, "pid": std::process::id() }).to_string();
        let fixed = hook_endpoint_path().filter(|_| takes_hooks());

        for path in endpoint_home().into_iter().chain(fixed) {
            write_private(&path, &json);
        }

        // the install folder's `data` may be readable by every account: a token left there by an older build goes
        if cfg!(windows)
            && std::env::var_os("MAGOAGENT_SCENE").is_none()
            && let Some(old) = autosave_path().map(|p| p.with_file_name("bridge.json"))
        {
            let _ = fs::remove_file(old);
        }
    }
}

/// Writes `json` to `path` for this user alone.
fn write_private(path: &Path, json: &str) {
    if let Some(folder) = path.parent() {
        let _ = fs::create_dir_all(folder);
    }

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

/// Where the app tells the person's own hooks (`magoagent hook external`) how to reach it: the same place for every
/// run, in the user's own data folder (the install folder may be everyone's).
/// A test instance (MAGOAGENT_SCENE) takes the hooks only at the place MAGOAGENT_HOOK_ENDPOINT names, the hooks it
/// runs pointed there too: the person's app keeps its own.
pub(crate) fn hook_endpoint_path() -> Option<PathBuf> {
    match std::env::var_os("MAGOAGENT_HOOK_ENDPOINT").filter(|path| !path.is_empty()) {
        Some(path) => Some(PathBuf::from(path)),
        None => crate::file::legacy_data_dir().map(|dir| dir.join("bridge.json")),
    }
}

/// This run answers the hooks: the person's app, or a test pointed at a place of its own.
fn takes_hooks() -> bool {
    std::env::var_os("MAGOAGENT_SCENE").is_none() || std::env::var_os("MAGOAGENT_HOOK_ENDPOINT").is_some_and(|path| !path.is_empty())
}

/// Removes the fixed endpoint on orderly shutdown, but only if it still belongs to this process.
pub(crate) fn remove_hook_endpoint() {
    if !takes_hooks() {
        return;
    }

    let Some(path) = hook_endpoint_path() else { return };

    let ours = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|endpoint| endpoint["pid"].as_u64() == Some(u64::from(std::process::id())));

    if ours {
        let _ = fs::remove_file(path);
    }
}

/// A note is called by its first line, without markdown marks.
pub(crate) fn note_name(text: &str) -> String {
    // emphasis wraps both ends ("**negrito**"), the block marks only start the line
    let first = text.lines().map(|l| l.trim().trim_start_matches(['#', '-', '>', ' ']).trim_matches(['*', '_', '~', '`', ' '])).find(|l| !l.is_empty());

    first.map_or("Nota".into(), |l| l.chars().take(40).collect::<String>().trim().to_string())
}

/// Where this run writes its endpoint: beside the scene, except on Windows for the person's own app, whose data folder
/// (in the install folder) may be readable by other accounts: there it is the user's own folder, like the hooks'.
fn endpoint_home() -> Option<PathBuf> {
    if cfg!(windows) && std::env::var_os("MAGOAGENT_SCENE").is_none() {
        return crate::file::legacy_data_dir().map(|dir| dir.join("bridge.json"));
    }

    autosave_path().map(|p| p.with_file_name("bridge.json"))
}

/// Where to read the endpoint of the app of this scene. On Windows a terminal's scene is a workspace's, in the data
/// folder, whose endpoint is the user's own folder one.
pub(crate) fn endpoint_path() -> Option<PathBuf> {
    let home = endpoint_home();
    let in_data_folder = autosave_path().and_then(|scene| scene.parent().map(Path::to_path_buf)) == crate::file::data_dir();

    if cfg!(windows) && in_data_folder && !home.as_ref().is_some_and(|path| path.exists()) {
        return crate::file::legacy_data_dir().map(|dir| dir.join("bridge.json")).or(home);
    }

    home
}

/// The app of this scene answers at the address it left in `bridge.json` (MAGOAGENT_BRIDGE is not
/// asked: a test instance started from a canvas terminal has a scene of its own).
// ponytail: a port another program took after a crash reads as open; upgrade: send `list` with the token
pub(crate) fn app_open(endpoint: &Path) -> bool {
    let saved = read_json::<Value>(endpoint);
    let address = saved.and_then(|s| s["address"].as_str()?.parse::<std::net::SocketAddr>().ok());

    address.is_some_and(|a| TcpStream::connect_timeout(&a, Duration::from_millis(500)).is_ok())
}
