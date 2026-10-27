//! The agent bridge: terminals on the canvas talk to each other through the app.
//!
//! The app listens on 127.0.0.1 with a random token. Each terminal gets the address, the token
//! and its own id in its environment (also written to `bridge.json` next to the scene, for MCP
//! clients outside the canvas); `magoagent list|ask|check|note` and the MCP agent tools send one
//! JSON line per request and read one back. A terminal only reaches what a line connects it to.

use crate::prelude::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Screen unchanged this long after an ask: the agent finished its turn.
const IDLE: Duration = Duration::from_secs(3);
/// No reaction at all this long after an ask: give back what is there.
const NO_REACTION: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum NodeKind {
    Terminal,
    Note,
    Portal,
    Ide,
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
    pub(crate) nodes: Vec<Node>,
    pub(crate) terminals: HashMap<String, TermHandle>,
    /// `ask`s in flight (asking terminal, asked terminal): their rope lights up meanwhile.
    pub(crate) asking: Vec<(String, String)>,
}

/// Changes only the UI thread can make (it owns the scene and the pages).
#[derive(Debug)]
pub(crate) enum UiCommand {
    PortalNavigate { id: String, url: String },
    /// Runs a script in a portal's page and sends back its JSON-encoded result.
    PortalEval { id: String, script: String, reply: mpsc::Sender<Result<String, String>> },
    WriteNote { id: String, text: String },
    /// A new sticky note next to terminal `from`, connected to it.
    CreateNote { from: String, text: String },
    /// The Maestro's moves: a new agent below `from`, a portal to its right, a rope between two
    /// things, a notification. What they create is roped to `from`.
    Recruit { from: String, name: String, command: String, reply: mpsc::Sender<Result<String, String>> },
    CreatePortal { from: String, url: String, name: String, reply: mpsc::Sender<Result<String, String>> },
    Connect { a: String, b: String, reply: mpsc::Sender<Result<String, String>> },
    Notify { from: String, text: String },
    /// An agent working in an IDE card, where the person sees it (app/ide.rs).
    Ide { id: String, action: IdeAction, reply: mpsc::Sender<Result<String, String>> },
}

impl UiCommand {
    /// The elements it acts on (the asking terminal when it creates something). A notification
    /// has none: it shows wherever the person is.
    pub(crate) fn targets(&self) -> Vec<&str> {
        use UiCommand::*;
        match self {
            PortalNavigate { id, .. } | PortalEval { id, .. } | WriteNote { id, .. } | Ide { id, .. } => vec![id],
            CreateNote { from, .. } | Recruit { from, .. } | CreatePortal { from, .. } => vec![from],
            Connect { a, b, .. } => vec![a, b],
            Notify { .. } => vec![],
        }
    }

    /// Answers the agent that it can't be done, when it waits for an answer.
    pub(crate) fn refuse(self, why: &str) {
        use UiCommand::*;
        if let PortalEval { reply, .. } | Recruit { reply, .. } | CreatePortal { reply, .. } | Connect { reply, .. } | Ide { reply, .. } = self {
            let _ = reply.send(Err(why.into()));
        }
    }
}

/// What an agent does in an IDE: open a folder, or a file at a line (1-based); change a file;
/// or a search it ran, for the card to tell.
#[derive(Debug)]
pub(crate) enum IdeAction {
    Open { path: PathBuf, line: Option<usize> },
    Edit { path: PathBuf, old: String, new: String },
    Searched { query: String, hits: usize },
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

fn endpoint_path() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("bridge.json"))
}

fn serve(stream: TcpStream, shared: &Mutex<Shared>, token: &str, sender: &mpsc::Sender<UiCommand>, ctx: Option<&Context>) {
    let mut line = String::new();
    // a client that connects and never sends its line would hold this thread forever
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(mut writer) = stream.try_clone() else { return };
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return;
    }
    let reply = match serde_json::from_str::<Value>(&line) {
        Ok(request) if request["token"] == token => handle(&request, shared, sender),
        Ok(_) => Err("token inválido".into()),
        Err(e) => Err(format!("pedido inválido: {e}")),
    };
    if let Some(ctx) = ctx {
        ctx.request_repaint();
    }
    let reply = match reply {
        Ok(text) => json!({ "ok": true, "text": text }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    let _ = writeln!(writer, "{reply}");
}

/// What `from` can reach: everything when the caller is outside the canvas, else what is
/// connected to it, plus the notes chained to its notes (note to note, like the reference app).
fn reachable(nodes: &[Node], from: &str) -> Vec<Node> {
    if from.is_empty() {
        return nodes.to_vec();
    }
    let links = nodes.iter().find(|n| n.id == from).map(|n| n.links.clone()).unwrap_or_default();
    let mut out: Vec<Node> = nodes.iter().filter(|n| n.kind != NodeKind::Note && links.contains(&n.id)).cloned().collect();
    let mut notes = note_tree(nodes, from);
    // `find` takes the first match: the nearest note of a name wins
    notes.sort_by_key(|(depth, _)| *depth);
    out.extend(notes.into_iter().map(|(_, n)| n.clone()));
    out
}

/// The notes `from` reaches, each followed by the notes chained to it one level deeper (all the
/// notes, from outside the canvas).
fn note_tree<'a>(nodes: &'a [Node], from: &str) -> Vec<(usize, &'a Node)> {
    let note = |id: &str| nodes.iter().find(|n| n.id == id && n.kind == NodeKind::Note);
    let caller = nodes.iter().find(|n| n.id == from);
    let roots: Vec<&Node> = match caller {
        Some(caller) => caller.links.iter().filter_map(|l| note(l)).collect(),
        None if from.is_empty() => nodes.iter().filter(|n| n.kind == NodeKind::Note).collect(),
        // a caller the canvas no longer has reaches nothing, like `reachable`
        None => Vec::new(),
    };
    // a note roped straight to the caller is a root, never a sibling's child
    let direct = |id: &str| caller.is_some_and(|c| c.links.iter().any(|l| l == id));
    let (mut out, mut seen) = (Vec::new(), std::collections::HashSet::new());
    let mut stack: Vec<(usize, &Node)> = roots.into_iter().rev().map(|n| (0, n)).collect();
    while let Some((depth, n)) = stack.pop() {
        if seen.insert(n.id.as_str()) {
            out.push((depth, n));
            stack.extend(n.links.iter().rev().filter_map(|l| note(l)).filter(|m| !seen.contains(m.id.as_str()) && !direct(&m.id)).map(|m| (depth + 1, m)));
        }
    }
    out
}

fn find(nodes: &[Node], from: &str, name: &str, kind: NodeKind) -> Result<Node, String> {
    let candidates: Vec<Node> = reachable(nodes, from).into_iter().filter(|n| n.kind == kind).collect();
    let exact = candidates.iter().find(|n| n.name == name);
    let loose = || candidates.iter().find(|n| n.name.to_lowercase() == name.trim().to_lowercase());
    exact.or_else(loose).cloned().ok_or_else(|| {
        let what = match kind {
            NodeKind::Terminal => "agente",
            NodeKind::Note => "nota",
            NodeKind::Portal => "portal",
            NodeKind::Ide => "IDE",
        };
        let names: Vec<String> = candidates.iter().map(|n| format!("\"{}\"", n.name)).collect();
        let options = if names.is_empty() { "nenhum".to_string() } else { names.join(", ") };
        format!("Nenhum {what} \"{name}\" conectado. Disponíveis: {options}.")
    })
}

/// The IDE an agent means: by name, or the only one it reaches when it names none.
fn find_ide(nodes: &[Node], from: &str, name: &str) -> Result<Node, String> {
    if name.trim().is_empty()
        && let [ide] = &reachable(nodes, from).into_iter().filter(|n| n.kind == NodeKind::Ide).collect::<Vec<_>>()[..]
    {
        return Ok(ide.clone());
    }
    find(nodes, from, name, NodeKind::Ide)
}

/// An agent's path in an IDE: relative to its project folder, or absolute inside it; never above it.
fn ide_path(root: &Path, path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path.trim());
    if path.as_os_str().is_empty() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("diga um caminho dentro do projeto, sem `..`".into());
    }
    let full: PathBuf = root.join(path).components().collect();
    // Windows paths ignore case (c:\proj\a.rs is C:\Proj\a.rs); whole components, so C:\Proj-x stays out
    let key = |c: std::path::Component| if cfg!(windows) { c.as_os_str().to_string_lossy().to_lowercase() } else { c.as_os_str().to_string_lossy().into_owned() };
    let n = root.components().count();
    let inside = full.components().count() >= n && full.components().zip(root.components()).all(|(a, b)| key(a) == key(b));
    // the project's own spelling, so an open tab of the same file is found
    inside.then(|| root.components().chain(full.components().skip(n)).collect()).ok_or_else(|| format!("{} está fora do projeto ({})", path.display(), root.display()))
}

pub(crate) fn handle(request: &Value, shared: &Mutex<Shared>, sender: &mpsc::Sender<UiCommand>) -> Result<String, String> {
    let from = request["from"].as_str().unwrap_or_default();
    let name = request["name"].as_str().unwrap_or_default();
    let text = request["text"].as_str().unwrap_or_default();
    let nodes = shared.lock().unwrap().nodes.clone();
    let handle_of = |node: &Node| shared.lock().unwrap().terminals.get(&node.id).cloned().ok_or("esse terminal não está rodando");
    let send = |command: UiCommand| sender.send(command).map_err(|_| "o app fechou".to_string());
    let answer = |rx: mpsc::Receiver<Result<String, String>>| rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "o app não respondeu".to_string())?;
    // `connect` reaches the whole canvas, not only what is roped to the caller: building the team is the point
    let anywhere = |n: &str| {
        let exact = nodes.iter().find(|x| x.name == n);
        exact.or_else(|| nodes.iter().find(|x| x.name.to_lowercase() == n.trim().to_lowercase())).map(|x| x.id.clone()).ok_or(format!("Nada chamado \"{n}\" no canvas."))
    };
    // a child agent builds nothing: only a Maestro recruits, opens portals and connects
    if matches!(request["cmd"].as_str(), Some("recruit" | "portal_create" | "connect")) && nodes.iter().any(|n| n.id == from && !n.maestro) {
        return Err("Só um terminal Maestro pode criar terminais, portais ou conexões.".into());
    }
    match request["cmd"].as_str().unwrap_or_default() {
        "list" => Ok(describe(&nodes, from, shared)),
        "check" => Ok(handle_of(&find(&nodes, from, name, NodeKind::Terminal)?)?.screen_text()),
        "ask" => {
            let target = find(&nodes, from, name, NodeKind::Terminal)?;
            if target.id == from {
                return Err("um agente não pode perguntar a si mesmo".into());
            }
            let timeout = Duration::from_secs_f64(request["timeout"].as_f64().unwrap_or(600.0).clamp(1.0, 3600.0));
            let pair = (from.to_string(), target.id.clone());
            shared.lock().unwrap().asking.push(pair.clone());
            let answer = handle_of(&target).map(|h| ask(&h, text, request["raw"] == true, timeout));
            shared.lock().unwrap().asking.retain(|p| *p != pair);
            Ok(answer?)
        }
        "note_read" => Ok(find(&nodes, from, name, NodeKind::Note)?.text),
        "note_write" => {
            let note = find(&nodes, from, name, NodeKind::Note)?;
            sender.send(UiCommand::WriteNote { id: note.id, text: text.into() }).map_err(|_| "o app fechou")?;
            Ok(format!("Nota \"{}\" atualizada.", note.name))
        }
        "note_create" => {
            if from.is_empty() {
                return Err("crie notas de dentro de um terminal do MagoAgent".into());
            }
            sender.send(UiCommand::CreateNote { from: from.into(), text: text.into() }).map_err(|_| "o app fechou")?;
            Ok("Nota criada e conectada a este terminal.".into())
        }
        "portal_navigate" => {
            let portal = find(&nodes, from, name, NodeKind::Portal)?;
            let url = normalize_url(text);
            sender.send(UiCommand::PortalNavigate { id: portal.id, url: url.clone() }).map_err(|_| "o app fechou")?;
            Ok(format!("Portal \"{}\" abrindo {url}.", portal.name))
        }
        cmd @ ("portal_text" | "portal_eval" | "portal_click" | "portal_fill") => {
            let portal = find(&nodes, from, name, NodeKind::Portal)?;
            let script = match cmd {
                "portal_text" => "document.body ? document.body.innerText : ''".into(),
                "portal_eval" => text.to_string(),
                _ => {
                    let who = nodes.iter().find(|n| n.id == from).map_or("IA", |n| n.name.as_str());
                    cursor_script(who, request["selector"].as_str().unwrap_or_default(), (cmd == "portal_fill").then_some(text))
                }
            };
            let (reply, answer) = mpsc::channel();
            sender.send(UiCommand::PortalEval { id: portal.id, script, reply }).map_err(|_| "o app fechou")?;
            let json = answer.recv_timeout(Duration::from_secs(20)).map_err(|_| "a página não respondeu")??;
            // strings come back JSON-quoted; plain text reads better
            Ok(serde_json::from_str::<Value>(&json).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or(json))
        }
        "recruit" | "portal_create" if from.is_empty() => Err("isso só funciona de dentro de um terminal do MagoAgent".into()),
        "recruit" => {
            let (reply, rx) = mpsc::channel();
            let command = if text.trim().is_empty() { "claude" } else { text };
            send(UiCommand::Recruit { from: from.into(), name: name.into(), command: command.into(), reply })?;
            answer(rx)
        }
        "portal_create" => {
            let (reply, rx) = mpsc::channel();
            send(UiCommand::CreatePortal { from: from.into(), url: text.into(), name: name.into(), reply })?;
            answer(rx)
        }
        "connect" => {
            let to = request["to"].as_str().unwrap_or_default();
            let (a, b) = match to {
                "" if from.is_empty() => return Err("diga as duas pontas: connect <a> <b>".into()),
                "" => (from.to_string(), anywhere(name)?),
                to => (anywhere(name)?, anywhere(to)?),
            };
            if a == b {
                return Err("as duas pontas são a mesma coisa".into());
            }
            let (reply, rx) = mpsc::channel();
            send(UiCommand::Connect { a, b, reply })?;
            answer(rx)
        }
        "notify" => {
            send(UiCommand::Notify { from: from.into(), text: text.into() })?;
            Ok("Aviso mostrado.".into())
        }
        "ide_search" => {
            let ide = find_ide(&nodes, from, name)?;
            // on this thread: a big project would hold the app's frame
            let hits = crate::ide::project_search(Path::new(&ide.text), text, 200);
            let (reply, _) = mpsc::channel();
            send(UiCommand::Ide { id: ide.id, action: IdeAction::Searched { query: text.into(), hits: hits.len() }, reply })?;
            Ok(if hits.is_empty() { format!("Nada encontrado para \"{text}\" em \"{}\".", ide.name) } else { hits.join("\n") })
        }
        cmd @ ("ide_open" | "ide_edit") => {
            let ide = find_ide(&nodes, from, name)?;
            let path = ide_path(Path::new(&ide.text), request["path"].as_str().unwrap_or_default())?;
            let action = match cmd {
                "ide_open" => IdeAction::Open { path, line: request["line"].as_u64().map(|l| l as usize) },
                _ => IdeAction::Edit { path, old: request["old"].as_str().unwrap_or_default().into(), new: request["new"].as_str().unwrap_or_default().into() },
            };
            let (reply, rx) = mpsc::channel();
            send(UiCommand::Ide { id: ide.id, action, reply })?;
            answer(rx)
        }
        other => Err(format!("comando desconhecido: {other}")),
    }
}

fn describe(nodes: &[Node], from: &str, shared: &Mutex<Shared>) -> String {
    let busy = |id: &str| match shared.lock().unwrap().terminals.get(id) {
        Some(t) if t.exited() => " (encerrado)",
        Some(t) if t.busy() => " (trabalhando)",
        Some(_) => " (parado)",
        None => "",
    };
    let reachable = reachable(nodes, from);
    let mut out = String::new();
    if let Some(me) = nodes.iter().find(|n| n.id == from) {
        out += &format!("Você: \"{}\"\n", me.name);
    }
    for (kind, title, connected) in [
        (NodeKind::Terminal, "Agentes", " conectados"),
        (NodeKind::Note, "Notas", " conectadas"),
        (NodeKind::Portal, "Portais", " conectados"),
        (NodeKind::Ide, "IDEs", " conectados"),
    ] {
        let items: Vec<String> = match kind {
            // a chained note sits under the note it hangs from
            NodeKind::Note => note_tree(nodes, from).into_iter().map(|(depth, n)| format!("{}  - \"{}\"", "  ".repeat(depth), n.name)).collect(),
            _ => reachable
                .iter()
                .filter(|n| n.kind == kind)
                .map(|n| match kind {
                    NodeKind::Portal | NodeKind::Ide => format!("  - \"{}\" ({})", n.name, n.text),
                    _ => format!("  - \"{}\"{}", n.name, busy(&n.id)),
                })
                .collect(),
        };
        if !items.is_empty() {
            out += &format!("{title}{}:\n{}\n", if from.is_empty() { "" } else { connected }, items.join("\n"));
        }
    }
    if reachable.is_empty() {
        out += "Nada conectado. Ligue este terminal a outro (ou a uma nota) com uma seta, ou use o botão Conectar.";
    }
    out.trim_end().to_string()
}

/// Types `prompt` into the agent (`type_prompt`, or raw keys) and waits for its screen to settle.
pub(crate) fn ask(target: &TermHandle, prompt: &str, raw: bool, timeout: Duration) -> String {
    let before = target.screen_text();
    if raw {
        target.write(unescape(prompt));
    } else {
        target.type_prompt(prompt);
    }
    let start = Instant::now();
    let (mut last, mut changed_at) = (before, None::<Instant>);
    loop {
        std::thread::sleep(Duration::from_millis(250));
        let screen = target.screen_text();
        if screen != last {
            last = screen;
            changed_at = Some(Instant::now());
        }
        let quiet = changed_at.map_or(start.elapsed(), |t| t.elapsed());
        let settled = if changed_at.is_some() { quiet >= IDLE } else { quiet >= NO_REACTION };
        if settled || target.exited() {
            return last;
        }
        if start.elapsed() >= timeout {
            return format!("{last}\n\n(o agente ainda está trabalhando; leia de novo com check/read_terminal)");
        }
    }
}

/// `\n` Enter, `\t` Tab, `\e` Esc, `\xNN` a byte, `\\` a backslash.
pub(crate) fn unescape(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.extend(c.to_string().into_bytes());
            continue;
        }
        match chars.next() {
            Some('n') | Some('r') => out.push(b'\r'),
            Some('t') => out.push(b'\t'),
            Some('e') => out.push(0x1b),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                out.extend(u8::from_str_radix(&hex, 16).ok());
            }
            Some(other) => out.extend(other.to_string().into_bytes()),
            None => out.push(b'\\'),
        }
    }
    out
}

/// The page side of `portal click|fill`: the agent's cursor, labeled with its name, glides to
/// the element the way a person's would, then clicks it or types into it.
pub(crate) fn cursor_script(agent: &str, selector: &str, fill: Option<&str>) -> String {
    let args = json!([agent, selector, fill]);
    format!(
        r##"(function([who, sel, text]) {{
  const el = document.querySelector(sel);
  if (!el) return 'nada encontrado com ' + sel;
  let c = document.getElementById('__mago_cursor');
  if (!c) {{
    c = document.createElement('div');
    c.id = '__mago_cursor';
    c.style.cssText = 'position:fixed;left:0;top:0;z-index:2147483647;pointer-events:none;transition:transform .5s cubic-bezier(.25,.8,.25,1);font:600 12px -apple-system,system-ui,sans-serif';
    // built node by node, styled through the CSSOM: a page with Trusted Types refuses HTML strings,
    // one whose CSP has no 'unsafe-inline' refuses style attributes
    const svg = 'http://www.w3.org/2000/svg';
    const make = (ns, tag, attrs, css) => {{ const n = ns ? document.createElementNS(ns, tag) : document.createElement(tag); for (const k in attrs) n.setAttribute(k, attrs[k]); n.style.cssText = css; return n; }};
    const arrow = make(svg, 'svg', {{ width: '18', height: '22', viewBox: '0 0 18 22' }}, 'filter:drop-shadow(0 1px 2px rgba(0,0,0,.35))');
    arrow.appendChild(make(svg, 'path', {{ d: 'M1 1L1 17L5.5 13L8.5 20L11.5 18.6L8.6 12L15 12Z', fill: '#e8590c', stroke: '#fff', 'stroke-width': '1.5', 'stroke-linejoin': 'round' }}, ''));
    c.append(arrow, make(null, 'span', {{}}, 'position:absolute;left:15px;top:19px;padding:3px 8px;border-radius:999px;background:#e8590c;color:#fff;white-space:nowrap;box-shadow:0 1px 3px rgba(0,0,0,.3)'));
    c.style.transform = 'translate(' + innerWidth / 2 + 'px,' + innerHeight / 2 + 'px)';
    document.documentElement.appendChild(c);
  }}
  c.querySelector('span').textContent = who;
  el.scrollIntoView({{ block: 'center', inline: 'center' }});
  const r = el.getBoundingClientRect();
  requestAnimationFrame(() => {{ c.style.transform = 'translate(' + (r.left + r.width / 2) + 'px,' + (r.top + r.height / 2) + 'px)'; }});
  setTimeout(() => {{
    if (text === null) return el.click();
    el.focus();
    // the native setter, so pages built with React see the change too
    const value = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value');
    value && value.set ? value.set.call(el, text) : (el.value = text);
    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
  }}, 550);
  return (text === null ? 'clicando em ' : 'preenchendo ') + sel;
}})({args})"##
    )
}

/// Sends a request to the running app: from inside a terminal through its environment,
/// otherwise through `bridge.json`.
pub(crate) fn request(request: Value, wait: Duration) -> Result<String, String> {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    let (address, token, from) = match (env("MAGOAGENT_BRIDGE"), env("MAGOAGENT_TOKEN")) {
        (Some(address), Some(token)) => (address, token, env("MAGOAGENT_TERMINAL_ID").unwrap_or_default()),
        _ => {
            let saved = endpoint_path().and_then(|p| fs::read_to_string(p).ok()).and_then(|d| serde_json::from_str::<Value>(&d).ok());
            let saved = saved.ok_or("O MagoAgent não está aberto.")?;
            (saved["address"].as_str().unwrap_or_default().into(), saved["token"].as_str().unwrap_or_default().into(), String::new())
        }
    };
    request_to(&address, &token, &from, request, wait)
}

pub(crate) fn request_to(address: &str, token: &str, from: &str, mut request: Value, wait: Duration) -> Result<String, String> {
    request["token"] = token.into();
    request["from"] = from.into();
    let mut stream = TcpStream::connect(address).map_err(|_| "O MagoAgent não está aberto (a ponte de agentes não respondeu).")?;
    stream.set_read_timeout(Some(wait + Duration::from_secs(30))).map_err(|e| e.to_string())?;
    writeln!(stream, "{request}").map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).map_err(|e| e.to_string())?;
    let reply: Value = serde_json::from_str(&line).map_err(|_| "resposta inválida do MagoAgent")?;
    match reply["ok"].as_bool() {
        Some(true) => Ok(reply["text"].as_str().unwrap_or_default().into()),
        _ => Err(reply["error"].as_str().unwrap_or("erro desconhecido").into()),
    }
}

const HELP: &str = "Uso: magoagent <comando>
  list                         o que está conectado a este terminal
  ask <nome> <prompt>          manda o prompt ao agente e espera ele terminar
      --raw                    envia teclas cruas (\\n Enter, \\t Tab, \\e Esc, \\x03 Ctrl+C)
      --timeout <segundos>     padrão 600
  check <nome>                 lê a tela do agente sem mandar nada
  note read <nome>             lê uma nota conectada
  note write <nome> <texto>    troca o texto da nota
  note create <texto>          cria uma nota ligada a este terminal
  portal navigate <nome> <url> abre um endereço num portal conectado
  portal text <nome>           lê o texto da página
  portal eval <nome> <js>      roda JavaScript na página e mostra o resultado
  portal click <nome> <css>    seu cursor vai até o elemento e clica
  portal fill <nome> <css> <texto>  seu cursor vai até o campo e digita
  recruit <nome> [comando]     cria um agente abaixo de você, ligado a você (padrão: claude)
  portal create <url> [nome]   abre um portal à sua direita, ligado a você
  connect <a> [b]              liga você a <a>, ou <a> a <b> (qualquer coisa do canvas)
  notify <texto>               avisa a pessoa (no app e nas notificações do sistema)";

/// `magoagent list|ask|check|note …`; returns the exit code.
pub(crate) fn cli(args: &[String]) -> i32 {
    let (mut words, mut raw, mut timeout) = (Vec::new(), false, 600.0);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--raw" => raw = true,
            "--timeout" => timeout = rest.next().and_then(|t| t.parse().ok()).unwrap_or(timeout),
            word => words.push(word),
        }
    }
    let request = match words.as_slice() {
        ["list"] => json!({ "cmd": "list" }),
        ["ask", name, prompt @ ..] if !prompt.is_empty() => {
            json!({ "cmd": "ask", "name": name, "text": prompt.join(" "), "raw": raw, "timeout": timeout })
        }
        ["check", name] => json!({ "cmd": "check", "name": name }),
        ["note", "read", name] => json!({ "cmd": "note_read", "name": name }),
        ["note", "write", name, text @ ..] => json!({ "cmd": "note_write", "name": name, "text": text.join(" ") }),
        ["note", "create", text @ ..] => json!({ "cmd": "note_create", "text": text.join(" ") }),
        ["portal", "navigate", name, url] => json!({ "cmd": "portal_navigate", "name": name, "text": url }),
        ["portal", "text", name] => json!({ "cmd": "portal_text", "name": name }),
        ["portal", "eval", name, script @ ..] => json!({ "cmd": "portal_eval", "name": name, "text": script.join(" ") }),
        ["portal", "click", name, selector] => json!({ "cmd": "portal_click", "name": name, "selector": selector }),
        ["portal", "fill", name, selector, text @ ..] => json!({ "cmd": "portal_fill", "name": name, "selector": selector, "text": text.join(" ") }),
        ["recruit", name, command @ ..] => json!({ "cmd": "recruit", "name": name, "text": command.join(" ") }),
        ["portal", "create", url, name @ ..] => json!({ "cmd": "portal_create", "name": name.join(" "), "text": url }),
        ["connect", a] => json!({ "cmd": "connect", "name": a }),
        ["connect", a, b] => json!({ "cmd": "connect", "name": a, "to": b }),
        ["notify", text @ ..] if !text.is_empty() => json!({ "cmd": "notify", "text": text.join(" ") }),
        _ => {
            println!("{HELP}");
            return if matches!(words.as_slice(), ["help"] | []) { 0 } else { 2 };
        }
    };
    match self::request(request, Duration::from_secs_f64(timeout)) {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_follows_the_note_chain() {
        let node = |id: &str, kind, links: &[&str]| Node { id: id.into(), kind, name: id.into(), text: String::new(), links: links.iter().map(|l| l.to_string()).collect(), maestro: false };
        // eu — Plano — Detalhes — Anexo — Plano (cycle) — Plano2 (named "Plano" too)
        // eu — Lado — Plano (roped to me and to a sibling), eu — Comum — Outro (shared note)
        // eu — Outro (terminal) — Dele (its note)
        let nodes = [
            node("eu", NodeKind::Terminal, &["Plano", "Outro", "Lado", "Comum"]),
            node("Plano", NodeKind::Note, &["eu", "Detalhes", "Lado"]),
            node("Detalhes", NodeKind::Note, &["Plano", "Anexo"]),
            node("Anexo", NodeKind::Note, &["Detalhes", "Plano", "Plano2"]),
            Node { name: "Plano".into(), ..node("Plano2", NodeKind::Note, &["Anexo"]) },
            node("Lado", NodeKind::Note, &["eu", "Plano"]),
            node("Outro", NodeKind::Terminal, &["eu", "Dele", "Comum"]),
            node("Dele", NodeKind::Note, &["Outro"]),
            node("Comum", NodeKind::Note, &["eu", "Outro"]),
        ];
        let names = |from: &str| reachable(&nodes, from).into_iter().map(|n| n.name).collect::<Vec<_>>();
        assert_eq!(names("eu"), ["Outro", "Plano", "Lado", "Comum", "Detalhes", "Anexo", "Plano"], "nearest first; a terminal's notes are not mine");
        assert_eq!(names("Outro"), ["eu", "Dele", "Comum"], "the shared note brings none of my notes along");
        assert!(names("sumiu").is_empty(), "a terminal the canvas lost reaches nothing");
        let shared = Mutex::new(Shared { nodes: nodes.to_vec(), ..Default::default() });
        let list = describe(&nodes, "eu", &shared);
        assert!(list.contains("Notas conectadas:\n  - \"Plano\"\n    - \"Detalhes\"\n      - \"Anexo\"\n        - \"Plano\"\n  - \"Lado\"\n  - \"Comum\""), "{list}");
        assert!(describe(&nodes, "sumiu", &shared).contains("Nada conectado"));
        assert!(find(&nodes, "eu", "anexo", NodeKind::Note).is_ok());
        assert_eq!(find(&nodes, "eu", "Plano", NodeKind::Note).unwrap().id, "Plano", "the direct note, not the deep one");
        assert_eq!(note_tree(&nodes, "").len(), 7, "outside the canvas: every note, once");
        assert!(describe(&nodes, "", &shared).contains("  - \"Plano\"\n    - \"Detalhes\""), "still a tree from outside");
    }

    #[test]
    fn an_ide_path_stays_inside_its_project() {
        let root = if cfg!(windows) { Path::new(r"C:\Users\eu\Proj") } else { Path::new("/home/eu/Proj") };
        assert_eq!(ide_path(root, "src/a.rs").unwrap(), root.join("src").join("a.rs"));
        assert!(ide_path(root, "../x").is_err() && ide_path(root, " ").is_err());
        let sibling = root.with_file_name("Proj-segredo").join("a.rs");
        assert!(ide_path(root, &sibling.to_string_lossy()).is_err(), "a folder that only starts with the project's name");
        let other_case = root.join("src").join("a.rs").to_string_lossy().to_lowercase();
        // the same file on Windows, in the project's spelling; another folder elsewhere
        assert_eq!(ide_path(root, &other_case).ok(), cfg!(windows).then(|| root.join("src").join("a.rs")));
    }
}
