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
}

/// An element agents can reach.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node {
    pub(crate) id: String,
    pub(crate) kind: NodeKind,
    pub(crate) name: String,
    /// A note's text, a portal's address.
    pub(crate) text: String,
    /// What lines bound at both ends connect it to.
    pub(crate) links: Vec<String>,
}

/// What the UI thread publishes every frame for the bridge threads.
#[derive(Default)]
pub(crate) struct Shared {
    pub(crate) nodes: Vec<Node>,
    pub(crate) terminals: HashMap<String, TermHandle>,
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
        let token = uuid::Uuid::new_v4().to_string();
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
    let first = text.lines().map(|l| l.trim().trim_start_matches(['#', '-', '*', '>', ' '])).find(|l| !l.is_empty());
    first.map_or("Nota".into(), |l| l.chars().take(40).collect::<String>().trim().to_string())
}

fn endpoint_path() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("bridge.json"))
}

fn serve(stream: TcpStream, shared: &Mutex<Shared>, token: &str, sender: &mpsc::Sender<UiCommand>, ctx: Option<&Context>) {
    let mut line = String::new();
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
/// connected to it.
fn reachable(nodes: &[Node], from: &str) -> Vec<Node> {
    if from.is_empty() {
        return nodes.to_vec();
    }
    let links = nodes.iter().find(|n| n.id == from).map(|n| n.links.clone()).unwrap_or_default();
    nodes.iter().filter(|n| links.contains(&n.id)).cloned().collect()
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
        };
        let names: Vec<String> = candidates.iter().map(|n| format!("\"{}\"", n.name)).collect();
        let options = if names.is_empty() { "nenhum".to_string() } else { names.join(", ") };
        format!("Nenhum {what} \"{name}\" conectado. Disponíveis: {options}.")
    })
}

pub(crate) fn handle(request: &Value, shared: &Mutex<Shared>, sender: &mpsc::Sender<UiCommand>) -> Result<String, String> {
    let from = request["from"].as_str().unwrap_or_default();
    let name = request["name"].as_str().unwrap_or_default();
    let text = request["text"].as_str().unwrap_or_default();
    let nodes = shared.lock().unwrap().nodes.clone();
    let handle_of = |node: &Node| shared.lock().unwrap().terminals.get(&node.id).cloned().ok_or("esse terminal não está rodando");
    match request["cmd"].as_str().unwrap_or_default() {
        "list" => Ok(describe(&nodes, from, shared)),
        "check" => Ok(handle_of(&find(&nodes, from, name, NodeKind::Terminal)?)?.screen_text()),
        "ask" => {
            let target = find(&nodes, from, name, NodeKind::Terminal)?;
            if target.id == from {
                return Err("um agente não pode perguntar a si mesmo".into());
            }
            let timeout = Duration::from_secs_f64(request["timeout"].as_f64().unwrap_or(600.0).clamp(1.0, 3600.0));
            Ok(ask(&handle_of(&target)?, text, request["raw"] == true, timeout))
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
    ] {
        let items: Vec<String> = reachable
            .iter()
            .filter(|n| n.kind == kind)
            .map(|n| match kind {
                NodeKind::Terminal => format!("  - \"{}\"{}", n.name, busy(&n.id)),
                NodeKind::Portal => format!("  - \"{}\" ({})", n.name, n.text),
                NodeKind::Note => format!("  - \"{}\"", n.name),
            })
            .collect();
        if !items.is_empty() {
            out += &format!("{title}{}:\n{}\n", if from.is_empty() { "" } else { connected }, items.join("\n"));
        }
    }
    if reachable.is_empty() {
        out += "Nada conectado. Ligue este terminal a outro (ou a uma nota) com uma seta, ou use o botão Conectar.";
    }
    out.trim_end().to_string()
}

/// Types `prompt` into the agent and waits for its screen to settle: pasted (so a
/// multi-line prompt arrives whole), then Enter on its own, as a person would.
pub(crate) fn ask(target: &TermHandle, prompt: &str, raw: bool, timeout: Duration) -> String {
    let before = target.screen_text();
    if raw {
        target.write(unescape(prompt));
    } else {
        target.write(paste_bytes(prompt, target.mode().contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE)));
        std::thread::sleep(Duration::from_millis(150));
        target.write(b"\r".to_vec());
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
    c.innerHTML = '<svg width="18" height="22" viewBox="0 0 18 22" style="filter:drop-shadow(0 1px 2px rgba(0,0,0,.35))"><path d="M1 1L1 17L5.5 13L8.5 20L11.5 18.6L8.6 12L15 12Z" fill="#e8590c" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/></svg><span style="position:absolute;left:15px;top:19px;padding:3px 8px;border-radius:999px;background:#e8590c;color:#fff;white-space:nowrap;box-shadow:0 1px 3px rgba(0,0,0,.3)"></span>';
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
  portal fill <nome> <css> <texto>  seu cursor vai até o campo e digita";

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
