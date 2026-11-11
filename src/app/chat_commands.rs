//! Slash commands of the chat's box: `/` at its start opens a menu (name and what it does, filtered
//! as you type); the local ones run here (login, logout, status, mcp, doctor, update on the agent's
//! own CLI, hidden; clear, model, effort, help on the chat itself) and their output lands in the
//! conversation as a note, never going to the model. Any other `/word` goes to the agent as the
//! message, and the menu also lists the commands and skills found in `.claude/` of the chat's
//! folder and of the home.

use crate::app::chat_direct::{Chat, folder};
use crate::app::chat_footer::{Choice, agent_default, efforts, label, models, more_models};
use crate::app::chat_list::Look;
use crate::prelude::*;
use crate::transcript::{ChatItem, Transcript};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};

/// A command of the menu: its name (no slash), the argument it takes, what it does.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) args: String,
    pub(crate) desc: String,
}

/// (name, argument, what it does, Codex has it too)
const DEFS: [(&str, &str, &str, bool); 10] = [
    ("login", "[console|sso]", "Entrar na sua conta (abre o navegador)", true),
    ("logout", "", "Sair da conta", true),
    ("status", "", "Mostra quem está logado e os ajustes do chat", true),
    ("mcp", "", "Lista os servidores MCP", true),
    ("doctor", "", "Confere a instalação do Claude Code", false),
    ("update", "", "Atualiza o Claude Code", false),
    ("clear", "", "Começa uma conversa nova neste chat", true),
    ("model", "<nome>", "Troca o modelo do agente", true),
    ("effort", "<nível>", "Troca o esforço do agente", true),
    ("help", "", "Lista os comandos", true),
];

/// The commands the chat answers itself, for `agent`; /compact is the agent's own.
pub(crate) fn catalog(agent: &str) -> Vec<Entry> {
    let mut list: Vec<Entry> =
        DEFS.iter().filter(|d| d.3 || agent != "codex").map(|d| Entry { name: d.0.into(), args: d.1.into(), desc: d.2.into() }).collect();
    if agent != "codex" {
        list.push(Entry { name: "compact".into(), args: "[instruções]".into(), desc: "Resume a conversa para liberar contexto (vai ao agente)".into() });
    }
    list
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Local {
    Login(String),
    Logout,
    Status,
    Mcp,
    Doctor,
    Update,
    Clear,
    Model(String),
    Effort(String),
    Help,
}

/// What the box's text means: None, it goes to the agent as a message; Some(Err), a local command
/// this agent doesn't have.
pub(crate) fn parse(text: &str, agent: &str) -> Option<Result<Local, String>> {
    let rest = text.trim().strip_prefix('/')?;
    let (word, arg) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(w, a)| (w, a.trim()));
    let arg = arg.to_string();
    let local = match word.to_lowercase().as_str() {
        "login" => Local::Login(arg),
        "logout" => Local::Logout,
        "status" => Local::Status,
        "mcp" => Local::Mcp,
        "doctor" => Local::Doctor,
        "update" => Local::Update,
        "clear" => Local::Clear,
        "model" => Local::Model(arg),
        "effort" => Local::Effort(arg),
        "help" => Local::Help,
        _ => return None,
    };
    if agent == "codex" && matches!(local, Local::Doctor | Local::Update) {
        return Some(Err(format!("/{word} só existe no Claude Code.")));
    }
    Some(Ok(local))
}

/// The agent CLI's arguments for a command that runs it.
pub(crate) fn cli_args(agent: &str, local: &Local) -> Result<Vec<String>, String> {
    let words: &[&str] = match (agent == "codex", local) {
        (false, Local::Login(arg)) => match arg.to_lowercase().as_str() {
            "" => &["auth", "login"],
            "claudeai" => &["auth", "login", "--claudeai"],
            "console" => &["auth", "login", "--console"],
            "sso" => &["auth", "login", "--sso"],
            _ => return Err("Use /login, /login console ou /login sso.".into()),
        },
        (true, Local::Login(arg)) if arg.is_empty() => &["login"],
        (true, Local::Login(_)) => return Err("No Codex, use só /login.".into()),
        (false, Local::Logout) => &["auth", "logout"],
        (true, Local::Logout) => &["logout"],
        (false, Local::Status) => &["auth", "status", "--json"],
        (true, Local::Status) => &["login", "status"],
        (_, Local::Mcp) => &["mcp", "list"],
        (false, Local::Doctor) => &["doctor"],
        (false, Local::Update) => &["update"],
        _ => return Err("Esse comando não roda o agente.".into()),
    };
    Ok(words.iter().map(|w| w.to_string()).collect())
}

/// `claude auth status --json` in words: whether there is a login, and who.
pub(crate) fn describe_status(json: &str) -> (bool, String) {
    let Ok(v) = serde_json::from_str::<Value>(json.trim()) else {
        return (false, json.trim().to_string());
    };
    if v["loggedIn"] != true {
        return (false, "Você não está logado. Digite /login para entrar.".into());
    }
    let get = |k: &str| v[k].as_str().filter(|s| !s.is_empty() && *s != "none").map(str::to_string);
    let mut text = format!("Logado como {}", get("email").unwrap_or_else(|| "uma conta".into()));
    for (key, prefix) in [("subscriptionType", "plano "), ("orgName", ""), ("authMethod", "acesso ")] {
        if let Some(value) = get(key) {
            text += &format!(" · {prefix}{value}");
        }
    }
    (true, text)
}

/// The text of a command's output with the terminal's colors off.
fn clean(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// What a command found in `.claude/` says it is for: the front matter's description, else its
/// first line.
/// A description cut at 100 characters, with an ellipsis.
fn clip(text: &str) -> String {
    if text.chars().count() > 100 { format!("{}…", text.chars().take(99).collect::<String>().trim_end()) } else { text.to_string() }
}

fn describe(text: &str) -> String {
    let body = match text.strip_prefix("---").and_then(|t| t.split_once("\n---")) {
        Some((front, rest)) => {
            let found = front.lines().find_map(|l| l.trim().strip_prefix("description:")).map(|d| d.trim().trim_matches(['"', '\'']).to_string());
            if let Some(d) = found.filter(|d| !d.is_empty()) {
                return clip(&d);
            }
            rest
        }
        None => text,
    };
    clip(body.lines().map(|l| l.trim().trim_start_matches('#').trim()).find(|l| !l.is_empty()).unwrap_or_default())
}

/// The custom commands and skills of a `.claude` folder: `commands/*.md` (a subfolder is a prefix,
/// `pasta:nome`) and `skills/*/SKILL.md`.
fn scan(claude: &Path, from: &str, out: &mut Vec<Entry>) {
    fn walk(dir: &Path, prefix: &str, from: &str, depth: usize, out: &mut Vec<Entry>) {
        let Ok(read) = fs::read_dir(dir) else { return };
        let mut files: Vec<_> = read.flatten().collect();
        files.sort_by_key(|e| e.file_name());
        for e in files {
            let (path, name) = (e.path(), e.file_name().to_string_lossy().to_string());
            if path.is_dir() && depth < 3 {
                walk(&path, &format!("{prefix}{name}:"), from, depth + 1, out);
            } else if let Some(stem) = name.strip_suffix(".md") {
                let desc = describe(&fs::read_to_string(&path).unwrap_or_default());
                out.push(Entry { name: format!("{prefix}{stem}"), args: String::new(), desc: format!("{desc} ({from})").trim().to_string() });
            }
        }
    }
    walk(&claude.join("commands"), "", from, 0, out);
    let Ok(skills) = fs::read_dir(claude.join("skills")) else { return };
    let mut dirs: Vec<_> = skills.flatten().collect();
    dirs.sort_by_key(|e| e.file_name());
    for e in dirs {
        if let Ok(text) = fs::read_to_string(e.path().join("SKILL.md")) {
            let named = text.strip_prefix("---").and_then(|t| t.split_once("\n---")).and_then(|(f, _)| f.lines().find_map(|l| l.trim().strip_prefix("name:")));
            let name = named.map_or_else(|| e.file_name().to_string_lossy().to_string(), |n| n.trim().trim_matches(['"', '\'']).to_string());
            out.push(Entry { name, args: String::new(), desc: format!("Skill · {} ({from})", describe(&text)) });
        }
    }
}

/// The folder's own commands first, then the person's; a name already taken stays with the first.
pub(crate) fn discover(cwd: &Path, home: Option<&Path>) -> Vec<Entry> {
    let mut all = Vec::new();
    scan(&cwd.join(".claude"), "projeto", &mut all);
    if let Some(home) = home {
        scan(&home.join(".claude"), "pessoal", &mut all);
    }
    let mut seen = std::collections::HashSet::new();
    all.retain(|e| seen.insert(e.name.clone()));
    all
}

/// The entries `query` (the text after the slash) leaves: the names that start with it, then the
/// ones that hold it.
pub(crate) fn filter(all: &[Entry], query: &str) -> Vec<Entry> {
    let q = query.to_lowercase();
    let starts = all.iter().filter(|e| e.name.to_lowercase().starts_with(&q));
    let holds = all.iter().filter(|e| !e.name.to_lowercase().starts_with(&q) && e.name.to_lowercase().contains(&q));
    starts.chain(holds).cloned().collect()
}

/// The menu's state: the picked row, the text it was last for, the text Esc closed it on, and the
/// custom commands found for a folder (looked up again every 5 s).
#[derive(Default)]
pub(crate) struct Slash {
    sel: usize,
    last: String,
    off: Option<String>,
    custom: Option<(String, f64, Vec<Entry>)>,
    /// The last message sent per chat, and the chats whose login came back since it failed.
    pub(crate) sent: std::collections::HashMap<String, String>,
    pub(crate) retry: std::collections::HashSet<String>,
}

enum Kind {
    Login,
    /// After a login: who is in now.
    Check,
    /// `/status`: the chat's own settings, then the login.
    Status(String),
    /// The output as it comes, under the command's name.
    Plain(&'static str),
}

/// A command's process running hidden: what it prints comes when it ends.
pub(crate) struct CmdRun {
    child: std::process::Child,
    rx: Receiver<(Vec<u8>, Vec<u8>)>,
    pub(crate) since: f64,
    kind: Kind,
    ctx: Context,
    /// Everything printed so far, both streams; the login's address is read from it.
    live: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    stdin: Option<std::process::ChildStdin>,
    shown_url: bool,
}

/// Reads `from` to its end, handing each chunk to `live` as it comes.
fn pump(mut from: impl Read, live: &std::sync::Mutex<Vec<u8>>) -> Vec<u8> {
    let (mut all, mut buf) = (Vec::new(), [0u8; 2048]);
    while let Ok(n) = from.read(&mut buf) {
        if n == 0 {
            break;
        }
        all.extend_from_slice(&buf[..n]);
        live.lock().unwrap().extend_from_slice(&buf[..n]);
    }
    all
}

/// The first address in what a login printed.
pub(crate) fn first_url(text: &str) -> Option<String> {
    text.split_whitespace().find(|w| w.starts_with("https://")).map(|w| w.trim_end_matches(['.', ',', ')']).to_string())
}

/// A login error's button asks for the login through the context: the item is drawn by `&self`.
const LOGIN_ASK: &str = "slash_login_ask";

pub(crate) fn take_login_ask(ctx: &Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::unique(LOGIN_ASK))).is_some()
}

impl DrawingApp {
    /// A note in the chat's conversation (a block when it has several lines).
    pub(crate) fn chat_note(&mut self, id: &str, text: impl Into<String>) {
        self.chat_transcript(id).items.push(ChatItem::System(text.into()));
        self.save_chat_items(id);
    }

    fn chat_of(&self, id: &str) -> Option<Chat> {
        self.chats.iter().find(|c| c.id == id).cloned()
    }

    /// The box's text sent: a local command runs here; anything else goes to the agent.
    /// False when it wasn't taken (the words stay in the box).
    pub(crate) fn chat_submit(&mut self, ctx: &Context, id: &str, text: &str, prompt: &str) -> bool {
        // while the login waits, the words in the box are the code the browser showed
        let code = self.chat_cmds.get_mut(id).filter(|r| matches!(r.kind, Kind::Login) && !text.trim_start().starts_with('/')).and_then(|r| {
            let stdin = r.stdin.as_mut()?;
            writeln!(stdin, "{}", text.trim()).and_then(|_| stdin.flush()).ok()
        });
        if code.is_some() {
            self.chat_note(id, "Código enviado ao login.");
            return true;
        }
        let parsed = self.chat_of(id).filter(|_| text == prompt).and_then(|c| parse(text, &c.agent));
        let Some(parsed) = parsed else { return self.chat_send(ctx, id, prompt) };
        match parsed {
            Err(error) => self.chat_note(id, error),
            Ok(local) => {
                if self.chat_cmds.contains_key(id) && local != Local::Clear {
                    self.toast("Um comando ainda está rodando neste chat: espere ou pare com Esc.");
                    return false;
                }
                self.run_local(ctx, id, local);
            }
        }
        true
    }

    fn run_local(&mut self, ctx: &Context, id: &str, local: Local) {
        let Some(chat) = self.chat_of(id) else { return };
        match local {
            Local::Clear => self.chat_clear(id),
            Local::Help => {
                let mut lines = vec!["Comandos do chat".to_string()];
                let custom = self.slash_custom(&chat);
                for e in catalog(&chat.agent).iter().chain(&custom) {
                    lines.push(if e.args.is_empty() { format!("/{} · {}", e.name, e.desc) } else { format!("/{} {} · {}", e.name, e.args, e.desc) });
                }
                lines.push("Qualquer outro /comando vai direto ao agente.".into());
                self.chat_note(id, lines.join("\n"));
            }
            Local::Model(arg) => self.chat_set_model(id, &arg),
            Local::Effort(arg) => self.chat_set_effort(id, &arg),
            other => {
                match cli_args(&chat.agent, &other) {
                    Err(error) => self.chat_note(id, error),
                    Ok(args) => {
                        let kind = match other {
                            Local::Login(_) => {
                                self.chat_note(id, "Abrindo o navegador para entrar… Termine o login lá. Se ele mostrar um código, cole aqui na caixa e tecle Enter. Esc cancela.");
                                Kind::Login
                            }
                            Local::Status => Kind::Status(self.chat_summary(&chat)),
                            Local::Logout => Kind::Plain("/logout"),
                            Local::Mcp => Kind::Plain("/mcp"),
                            Local::Doctor => Kind::Plain("/doctor"),
                            _ => Kind::Plain("/update"),
                        };
                        self.start_cmd(ctx, id, args, kind);
                    }
                }
            }
        }
    }

    fn chat_summary(&self, chat: &Chat) -> String {
        let (model, effort) = (agent_default(&chat.agent), (&chat.model, &chat.effort));
        let or_default = |own: &str, default: &str| {
            if own.is_empty() { format!("{} (padrão)", if default.is_empty() { "do agente" } else { default }) } else { own.into() }
        };
        format!(
            "Pasta: {}\nModelo: {}\nEsforço: {}\nPermissão: {}\n",
            chat.cwd,
            or_default(effort.0, &model.0),
            or_default(effort.1, &model.1),
            crate::app::chat_direct::CHAT_MODES[chat.mode.min(1)]
        )
    }

    /// Runs the agent's CLI hidden in the chat's folder; what it prints comes in `poll_cmds`.
    fn start_cmd(&mut self, ctx: &Context, id: &str, args: Vec<String>, kind: Kind) {
        let Some(chat) = self.chat_of(id) else { return };
        let Some(program) = crate::ide::lsp::which(&chat.agent) else {
            self.chat_note(id, format!("{} não foi encontrado no PATH.", chat.agent));
            return;
        };
        let mut command = crate::ide::command(program);
        // a login may ask for the code the browser shows: its stdin stays open for the chat's box
        let stdin = if matches!(kind, Kind::Login) { std::process::Stdio::piped() } else { std::process::Stdio::null() };
        command.args(args).env("NO_COLOR", "1").stdin(stdin).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        if let Some(dir) = folder(&chat.cwd).filter(|d| d.is_dir()) {
            command.current_dir(dir);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return self.chat_note(id, format!("Não foi possível rodar {}: {error}", chat.agent)),
        };
        let (tx, rx) = channel();
        let (stdout, stderr, stdin) = (child.stdout.take(), child.stderr.take(), child.stdin.take());
        let live = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let (live_out, live_err) = (live.clone(), live.clone());
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let errors = std::thread::spawn(move || stderr.map(|e| pump(e, &live_err)).unwrap_or_default());
            let out = stdout.map(|o| pump(o, &live_out)).unwrap_or_default();
            let _ = tx.send((out, errors.join().unwrap_or_default()));
            repaint.request_repaint();
        });
        self.chat_cmds.insert(id.to_string(), CmdRun { child, rx, since: self.now, kind, ctx: ctx.clone(), live, stdin, shown_url: false });
    }

    /// Esc or Interromper: the command running stops.
    pub(crate) fn cmd_cancel(&mut self, id: &str) {
        if let Some(mut run) = self.chat_cmds.remove(id) {
            let _ = run.child.kill();
            let _ = run.child.wait();
            self.chat_note(id, "Comando cancelado.");
        }
    }

    /// Each frame: the commands that ended put their output in the conversation.
    pub(crate) fn poll_cmds(&mut self) {
        let ids: Vec<String> = self.chat_cmds.keys().cloned().collect();
        for id in ids {
            // the login's address, in case the browser didn't open by itself
            let url = self.chat_cmds.get_mut(&id).filter(|r| matches!(r.kind, Kind::Login) && !r.shown_url).and_then(|r| {
                let url = first_url(&clean(&r.live.lock().unwrap()))?;
                r.shown_url = true;
                Some(url)
            });
            if let Some(url) = url {
                self.chat_note(&id, format!("Se o navegador não abriu, abra este endereço: {url}"));
            }
            let Some((out, err)) = self.chat_cmds.get(&id).and_then(|r| r.rx.try_recv().ok()) else { continue };
            let Some(mut run) = self.chat_cmds.remove(&id) else { continue };
            let ok = run.child.wait().is_ok_and(|s| s.success());
            let (out, err) = (clean(&out), clean(&err));
            let said = if out.is_empty() { err.clone() } else { out.clone() };
            let agent = self.chat_of(&id).map(|c| c.agent).unwrap_or_default();
            let status = |said: &str| if agent == "codex" { (said.contains("Logged in"), said.to_string()) } else { describe_status(said) };
            let note = match run.kind {
                _ if !ok && !matches!(run.kind, Kind::Status(_) | Kind::Check) => {
                    format!("O comando falhou.{}", if said.is_empty() { String::new() } else { format!("\n{said}") })
                }
                Kind::Login => {
                    // the browser is back: who is in now
                    if let Ok(args) = cli_args(&agent, &Local::Status) {
                        self.start_cmd(&run.ctx, &id, args, Kind::Check);
                    }
                    continue;
                }
                Kind::Check => match status(&said) {
                    (true, who) if agent == "codex" => format!("Pronto, login feito. {who}"),
                    (true, who) if self.slash.sent.contains_key(&id) => {
                        self.slash.retry.insert(id.clone());
                        format!(
                            "Pronto. {who}.
Clique em Mandar de novo para reenviar a última mensagem."
                        )
                    }
                    (true, who) => format!("Pronto. {who}."),
                    (false, _) => "O login terminou, mas o agente ainda não mostra uma conta. Tente /login de novo.".into(),
                },
                Kind::Status(summary) => format!("{summary}{}", status(&said).1),
                Kind::Plain(title) => format!("{title}\n{}", if said.is_empty() { "(sem saída)" } else { &said }),
            };
            self.chat_note(&id, note);
            run.ctx.request_repaint();
        }
    }

    /// /clear: a new session in the same chat, the conversation shown emptied.
    pub(crate) fn chat_clear(&mut self, id: &str) {
        self.chat_stop(id);
        if let Some(chat) = self.chats.iter_mut().find(|c| c.id == id) {
            chat.session = None;
        }
        self.transcripts.insert(id.to_string(), Transcript::new(PathBuf::new()));
        self.chat_images.borrow_mut().clear();
        self.save_chats();
        self.chat_note(id, "Conversa nova: a próxima mensagem começa uma sessão do zero.");
    }

    fn chat_set_model(&mut self, id: &str, arg: &str) {
        let Some(chat) = self.chat_of(id) else { return };
        let all: Vec<Choice> = models(&chat.agent).into_iter().chain(more_models(&chat.agent)).collect();
        let default = agent_default(&chat.agent).0;
        let names = models(&chat.agent).iter().map(|m| m.name.clone()).collect::<Vec<_>>().join(", ");
        if arg.is_empty() {
            let now = if chat.model.is_empty() { format!("{} (padrão do agente)", label(&all, &default)) } else { label(&all, &chat.model) };
            return self.chat_note(id, format!("Modelo: {now}\nUso: /model <nome>, como {names}; /model padrão volta ao padrão."));
        }
        let wanted = resolve_model(arg, &all);
        let stored = if wanted == default { String::new() } else { wanted.clone() };
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == id) {
            c.model = stored;
        }
        self.save_chats();
        let shown = if wanted.is_empty() { "padrão do agente".to_string() } else { label(&all, &wanted) };
        self.chat_note(id, format!("Modelo: {shown}"));
    }

    fn chat_set_effort(&mut self, id: &str, arg: &str) {
        let Some(chat) = self.chat_of(id) else { return };
        let model = if chat.model.is_empty() { agent_default(&chat.agent).0 } else { chat.model.clone() };
        let list = efforts(&chat.agent, &model);
        let names = list.iter().map(|e| e.name.to_lowercase()).collect::<Vec<_>>().join(", ");
        if arg.is_empty() {
            let now = if chat.effort.is_empty() { "padrão do agente".to_string() } else { label(&list, &chat.effort) };
            return self.chat_note(id, format!("Esforço: {now}\nUso: /effort <nível>: {names}; /effort padrão volta ao padrão."));
        }
        let Some(wanted) = resolve_effort(arg, &list) else {
            return self.chat_note(id, format!("Esforço “{arg}” não existe para este modelo. Use: {names}."));
        };
        let (default, shown) = (agent_default(&chat.agent).1, if wanted.is_empty() { "padrão do agente".to_string() } else { label(&list, &wanted) });
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == id) {
            c.effort = if wanted == default { String::new() } else { wanted };
        }
        self.save_chats();
        self.chat_note(id, format!("Esforço: {shown}"));
    }

    /// The custom commands of the chat's folder and the home, looked up every 5 s.
    fn slash_custom(&mut self, chat: &Chat) -> Vec<Entry> {
        let stale = self.slash.custom.as_ref().is_none_or(|(cwd, at, _)| *cwd != chat.cwd || self.now - at > 5.0);
        if stale {
            let found = folder(&chat.cwd).map(|d| discover(&d, std::env::home_dir().as_deref())).unwrap_or_default();
            self.slash.custom = Some((chat.cwd.clone(), self.now, found));
        }
        let own = catalog(&chat.agent);
        // a custom name the chat answers itself stays the chat's
        self.slash.custom.iter().flat_map(|(.., list)| list.iter().filter(|e| !own.iter().any(|o| o.name == e.name)).cloned()).collect()
    }

    /// The menu's rows for what the box says now; None when it is closed.
    pub(crate) fn slash_entries(&mut self) -> Option<Vec<Entry>> {
        let draft = self.chat_draft.clone();
        let query = draft.strip_prefix('/')?;
        if query.contains(char::is_whitespace) || self.slash.off.as_deref() == Some(draft.as_str()) {
            return None;
        }
        let chat = self.chat_agent.as_deref().and_then(|id| self.chat_of(id))?;
        if self.slash.last != draft {
            (self.slash.last, self.slash.sel) = (draft.clone(), 0);
        }
        let custom = self.slash_custom(&chat);
        let all: Vec<Entry> = catalog(&chat.agent).into_iter().chain(custom).collect();
        Some(filter(&all, query)).filter(|l| !l.is_empty())
    }

    /// The menu's keys while the box has focus: arrows move, Tab and Enter complete, Esc closes.
    /// True when the Enter was taken (it completed, it doesn't send).
    pub(crate) fn slash_keys(&mut self, ctx: &Context, draft: &mut String, entries: &[Entry], id: egui::Id) -> bool {
        if !ctx.memory(|m| m.has_focus(id) || m.had_focus_last_frame(id)) {
            return false;
        }
        let (down, up, tab, esc, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Tab),
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift)),
            )
        });
        let n = entries.len();
        let mut sel = self.slash.sel.min(n - 1);
        if down {
            sel = (sel + 1) % n;
        }
        if up {
            sel = (sel + n - 1) % n;
        }
        self.slash.sel = sel;
        if esc {
            self.slash.off = Some(draft.clone());
        }
        let complete = tab || (enter && draft.trim_start_matches('/') != entries[sel].name);
        if complete {
            complete_into(ctx, draft, &entries[sel].name, id);
        }
        enter && complete
    }

    /// The menu over the box: a row to click completes it. `anchor` is the box's top left.
    pub(crate) fn slash_menu_ui(&self, ctx: &Context, entries: &[Entry], anchor: Pos2, width: f32, look: &Look) -> Option<String> {
        let mut picked = None;
        let selected = self.slash.sel.min(entries.len() - 1);
        egui::Area::new(egui::Id::unique("slash_menu")).order(egui::Order::Foreground).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(anchor).show(ctx, |ui| {
            egui::Frame::new().fill(look.surface).stroke(Stroke::new(1.0, look.line)).corner_radius(14).inner_margin(6).show(ui, |ui| {
                ui.set_width(width - 12.0);
                egui::ScrollArea::vertical().max_height(264.0).auto_shrink([false, true]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for (i, e) in entries.iter().enumerate() {
                        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
                        if i == selected || response.hovered() {
                            ui.painter().rect_filled(rect, 8, if i == selected { look.selected } else { look.raised });
                        }
                        if i == selected && ctx.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowUp)) {
                            ui.scroll_to_rect(rect, None);
                        }
                        let name = if e.args.is_empty() { format!("/{}", e.name) } else { format!("/{} {}", e.name, e.args) };
                        let p = ui.painter();
                        let font = FontId::proportional(13.5);
                        let galley = p.layout_no_wrap(name, font.clone(), look.text);
                        let desc_x = rect.left() + (galley.size().x + 24.0).max(190.0);
                        p.galley(Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, look.text);
                        let room = (rect.right() - desc_x - 8.0).max(0.0);
                        let desc = p.layout(e.desc.clone(), FontId::proportional(12.0), look.muted, room);
                        if desc.rows.len() <= 1 {
                            p.galley(Pos2::new(desc_x, rect.center().y - desc.size().y / 2.0), desc, look.muted);
                        } else {
                            // too long for the row: cut at its end
                            let cut: String = e.desc.chars().take(((room / 6.5) as usize).saturating_sub(1)).collect();
                            p.text(Pos2::new(desc_x, rect.center().y), egui::Align2::LEFT_CENTER, format!("{cut}…"), FontId::proportional(12.0), look.muted);
                        }
                        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            picked = Some(e.name.clone());
                        }
                    }
                });
            });
        });
        picked
    }
}

/// The box takes "/name " and its cursor goes to the end.
pub(crate) fn complete_into(ctx: &Context, draft: &mut String, name: &str, id: egui::Id) {
    *draft = format!("/{name} ");
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(draft.chars().count()))));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// The model `arg` names: an id or a name from the lists, a word one holds ("opus"), "padrão" for
/// the agent's own; else what was typed, which the agent reads as its own alias.
pub(crate) fn resolve_model(arg: &str, all: &[Choice]) -> String {
    let a = arg.trim().to_lowercase();
    if ["padrão", "padrao", "default"].contains(&a.as_str()) {
        return String::new();
    }
    let exact = all.iter().find(|c| c.id.to_lowercase() == a || c.name.to_lowercase() == a);
    let holds = || all.iter().find(|c| c.id.to_lowercase().contains(&a) || c.name.to_lowercase().contains(&a));
    exact.or_else(holds).map_or_else(|| arg.trim().to_string(), |c| c.id.clone())
}

/// The effort `arg` names, by id or by name; Some("") is the default; None, not one of them.
pub(crate) fn resolve_effort(arg: &str, list: &[Choice]) -> Option<String> {
    let a = arg.trim().to_lowercase();
    if ["padrão", "padrao", "default"].contains(&a.as_str()) {
        return Some(String::new());
    }
    list.iter().find(|e| e.id == a || e.name.to_lowercase() == a || e.name.to_lowercase().replace('á', "a") == a).map(|e| e.id.clone())
}

/// The two buttons under a login error: Entrar, and once the login is done Mandar de novo. They
/// ask through the context (`take_login_ask`, `take_retry_ask`): the conversation is drawn by `&self`.
const RETRY_ASK: &str = "slash_retry_ask";

pub(crate) fn take_retry_ask(ctx: &Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::unique(RETRY_ASK))).is_some()
}

impl DrawingApp {
    /// Under the conversation, when its last word is an expired-login error: the primary button.
    pub(crate) fn login_buttons(&self, ui: &mut egui::Ui, id: &str, items: &[ChatItem], look: &Look) {
        let expired = items
            .iter()
            .rev()
            .find(|i| !matches!(i, ChatItem::System(_)))
            .is_some_and(|i| matches!(i, ChatItem::Error(e) if crate::app::chat::error_hint(e).is_some()));
        if !expired || !self.is_chat(id) || self.chat_cmds.contains_key(id) {
            return;
        }
        let retry = self.slash.retry.contains(id);
        let (fill, ink) = if look.dark { (Color32::WHITE, Color32::BLACK) } else { (Color32::BLACK, Color32::WHITE) };
        ui.add_space(6.0);
        let label = egui::RichText::new(if retry { "Mandar de novo" } else { "Entrar" }).size(14.0).strong().color(ink);
        // no keyboard focus: an Enter meant for the box never presses it
        let sense = Sense::CLICK;
        let button = egui::Button::new(label).sense(sense).fill(fill).corner_radius(18).min_size(Vec2::new(120.0, 34.0));
        if ui.add(button).on_hover_cursor(CursorIcon::PointingHand).clicked() {
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique(if retry { RETRY_ASK } else { LOGIN_ASK }), true));
        }
        ui.add_space(8.0);
    }

    /// Mandar de novo: the message that failed goes again, now that the login is back.
    pub(crate) fn chat_retry(&mut self, ctx: &Context, id: &str) {
        self.slash.retry.remove(id);
        if let Some(prompt) = self.slash.sent.get(id).cloned() {
            self.chat_send(ctx, id, &prompt);
        }
    }
}

#[cfg(test)]
mod tests;
