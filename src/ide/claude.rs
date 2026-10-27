//! Native Claude Code integration: the IDE side of Claude Code's IDE protocol (what `/ide` connects
//! to), recovered from the JetBrains plugin (Ilha's docs/claude-ide-protocol.md).
//! Lock file ~/.claude/ide/<port>.lock -> WebSocket (subprotocol "mcp", auth header) -> MCP JSON-RPC.
//! The app side answers tool calls, shows proposed edits as diffs and streams the selection.

use super::*;
use serde_json::{Value, json};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tungstenite::Message;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};

/// How Claude Code lists this IDE (`/ide`).
const APP_NAME: &str = "MagoAgent";

/// A tool call the UI thread must answer (text items of the MCP result).
pub struct Call {
    pub name: String,
    pub args: Value,
    pub reply: Sender<Vec<String>>,
}

pub struct Claude {
    pub port: u16,
    lock: PathBuf,
    pub calls: Receiver<Call>,
    /// One sender per connected `claude` (the Arc is its identity, removed when it leaves).
    clients: Arc<Mutex<Vec<Arc<Sender<String>>>>>,
    /// Set on drop: the accept loop and every connection end, freeing the port.
    stop: Arc<AtomicBool>,
}

fn token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// `~/.claude/ide` (`CLAUDE_CONFIG_DIR` moves it), where Claude Code finds IDEs. Tests use a temp
/// folder, so they never announce an IDE to the Claude sessions running on this machine.
fn lock_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("magoagent-claude-{}", std::process::id())).join("ide");
    }
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from)
        .or_else(|| std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(|h| PathBuf::from(h).join(".claude")))
        .unwrap_or_default()
        .join("ide")
}

/// `C:\a\b` -> `/mnt/c/a/b`, the form the JetBrains plugin advertises by default.
pub fn to_wsl(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    match s.as_bytes() {
        [d, b':', b'/', ..] => format!("/mnt/{}/{}", (*d as char).to_ascii_lowercase(), &s[3..]),
        _ => s,
    }
}

/// Paths from the CLI may be native, forward-slashed, `/mnt/c/...` or `file://` URIs.
pub fn to_os(p: &str) -> PathBuf {
    let p = p.trim_start_matches("file://").trim_start_matches("_claude_fs_right:");
    let b = p.as_bytes();
    if cfg!(windows) && b.len() > 7 && p.starts_with("/mnt/") && b[6] == b'/' {
        return PathBuf::from(format!("{}:/{}", (b[5] as char).to_ascii_uppercase(), &p[7..]));
    }
    if cfg!(windows) && b.len() > 2 && b[0] == b'/' && b[2] == b':' {
        return PathBuf::from(&p[1..]);
    }
    PathBuf::from(p)
}

impl Claude {
    pub fn start(ctx: eframe::egui::Context, workspace: &Path) -> std::io::Result<Claude> {
        let listener = TcpListener::bind("127.0.0.1:0")?; // loopback only (the plugin binds 0.0.0.0 on Windows)
        let port = listener.local_addr()?.port();
        let token = token();
        let dir = lock_dir();
        std::fs::create_dir_all(&dir)?;
        let lock = dir.join(format!("{port}.lock"));
        // both forms, so native Windows and WSL `claude` both match their cwd
        let native = if cfg!(windows) { workspace.to_string_lossy().replace('/', "\\") } else { workspace.to_string_lossy().into_owned() };
        let mut folders = vec![native, to_wsl(workspace)];
        folders.dedup();
        // the token is in it: owner only
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        std::io::Write::write_all(&mut opts.open(&lock)?, json!({
            "workspaceFolders": folders, "pid": std::process::id(), "ideName": APP_NAME,
            "transport": "ws", "runningInWindows": cfg!(windows), "authToken": token,
        }).to_string().as_bytes())?;
        let (tx, calls) = channel();
        let clients: Arc<Mutex<Vec<Arc<Sender<String>>>>> = Arc::default();
        let stop: Arc<AtomicBool> = Arc::default();
        let (cl, stopped) = (clients.clone(), stop.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
                let (tx, cl, token, ctx, stopped) = (tx.clone(), cl.clone(), token.clone(), ctx.clone(), stopped.clone());
                std::thread::spawn(move || serve(stream, &token, tx, cl, ctx, &stopped));
            }
        });
        Ok(Claude { port, lock, calls, clients, stop })
    }

    /// Env for terminals so `claude` started there connects to us automatically.
    pub fn env(&self) -> Vec<(String, String)> {
        vec![("ENABLE_IDE_INTEGRATION".into(), "true".into()), ("CLAUDE_CODE_SSE_PORT".into(), self.port.to_string())]
    }

    pub fn connected(&self) -> bool {
        !self.clients.lock().unwrap().is_empty()
    }

    /// IDE -> CLI notification (`selection_changed`, `at_mentioned`), to every connected client.
    pub fn notify(&self, method: &str, params: Value) {
        let msg = json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string();
        self.clients.lock().unwrap().retain(|c| c.send(msg.clone()).is_ok());
    }
}

impl Drop for Claude {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port)); // wakes `accept`, which then sees the flag
        let _ = std::fs::remove_file(&self.lock);
    }
}

fn serve(stream: TcpStream, token: &str, calls: Sender<Call>, clients: Arc<Mutex<Vec<Arc<Sender<String>>>>>, ctx: eframe::egui::Context, stop: &AtomicBool) {
    let check = |req: &Request, mut resp: Response| -> Result<Response, ErrorResponse> {
        let auth = req.headers().get("x-claude-code-ide-authorization").and_then(|v| v.to_str().ok());
        if auth != Some(token) {
            let mut e = ErrorResponse::new(Some("Invalid or missing authentication token".into()));
            *e.status_mut() = tungstenite::http::StatusCode::UNAUTHORIZED;
            return Err(e);
        }
        let mcp = req.headers().get("sec-websocket-protocol").and_then(|v| v.to_str().ok()).is_some_and(|v| v.split(',').any(|p| p.trim() == "mcp"));
        if mcp {
            resp.headers_mut().insert("sec-websocket-protocol", "mcp".parse().unwrap());
        }
        Ok(resp)
    };
    let Ok(mut ws) = tungstenite::accept_hdr(stream, check) else { return };
    let _ = ws.get_ref().set_read_timeout(Some(Duration::from_millis(100)));
    let (out_tx, out_rx) = channel::<String>();
    let out_tx = Arc::new(out_tx);
    clients.lock().unwrap().push(out_tx.clone());
    ctx.request_repaint();
    'talk: while !stop.load(Ordering::Relaxed) {
        match ws.read() {
            Ok(Message::Text(t)) => handle(&t, &out_tx, &calls, &ctx),
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
        while let Ok(m) = out_rx.try_recv() {
            if ws.send(Message::text(m)).is_err() {
                break 'talk;
            }
        }
        let _ = ws.flush();
    }
    let _ = ws.close(None); // the node went away: tell the CLI instead of vanishing
    let _ = ws.flush();
    clients.lock().unwrap().retain(|c| !Arc::ptr_eq(c, &out_tx));
    ctx.request_repaint();
}

fn handle(text: &str, out: &Sender<String>, calls: &Sender<Call>, ctx: &eframe::egui::Context) {
    let Ok(msg) = serde_json::from_str::<Value>(text) else { return };
    let Some(id) = msg.get("id").cloned() else { return }; // notifications (initialized, ide_connected…)
    let Some(method) = msg["method"].as_str() else { return }; // responses to our requests
    let reply = |result: Value| {
        let _ = out.send(json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string());
    };
    match method {
        "initialize" => {
            let asked = msg["params"]["protocolVersion"].as_str().unwrap_or("");
            let pv = if ["2024-11-05", "2024-10-07", "2025-03-26", "2025-06-18"].contains(&asked) { asked } else { "2024-11-05" };
            reply(json!({"protocolVersion": pv, "capabilities": {"tools": {"listChanged": true}},
                "serverInfo": {"name": APP_NAME, "version": env!("CARGO_PKG_VERSION")}}));
        }
        "ping" => reply(json!({})),
        "tools/list" => reply(json!({"tools": tools()})),
        "tools/call" => {
            let (tx, rx) = channel();
            let name = msg["params"]["name"].as_str().unwrap_or("").to_string();
            let args = msg["params"]["arguments"].clone();
            if calls.send(Call { name, args, reply: tx }).is_err() {
                return;
            }
            ctx.request_repaint();
            // openDiff blocks until the user decides, so wait off the socket loop
            let (out, id) = (out.clone(), id.clone());
            std::thread::spawn(move || {
                let texts = rx.recv().unwrap_or_else(|_| vec!["DIFF_REJECTED".into()]);
                let content: Vec<Value> = texts.into_iter().map(|t| json!({"type": "text", "text": t})).collect();
                let _ = out.send(json!({"jsonrpc": "2.0", "id": id, "result": {"content": content, "isError": false}}).to_string());
            });
        }
        _ => {
            let _ = out.send(json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("Server does not support {method}")}}).to_string());
        }
    }
}

fn tools() -> Value {
    let s = |props: Value, req: &[&str]| json!({"type": "object", "properties": props, "required": req});
    json!([
        {"name": "openDiff", "description": "Opens a diff in the IDE",
         "inputSchema": s(json!({"old_file_path": {"type": "string"}, "new_file_path": {"type": "string"}, "new_file_contents": {"type": "string"}, "tab_name": {"type": "string"}}), &["old_file_path", "new_file_path", "new_file_contents", "tab_name"])},
        {"name": "close_tab", "description": "Closes a tab in the IDE, given either the tab_name used in openDiff or the\nabsolute file path.",
         "inputSchema": s(json!({"tab_name": {"type": "string"}}), &["tab_name"])},
        {"name": "openFile", "description": "Opens the specified file in the IDE",
         "inputSchema": s(json!({"filePath": {"type": "string"}, "preview": {"type": "boolean"}, "startText": {"type": "string"}, "endText": {"type": "string"}, "selectToEndOfLine": {"type": "boolean"}, "makeFrontmost": {"type": "boolean"}}), &["filePath"])},
        {"name": "open_files", "description": "Opens the specified files in the IDE using their absolute file paths.",
         "inputSchema": s(json!({"file_paths": {"type": "array"}}), &["file_paths"])},
        {"name": "get_all_opened_file_paths", "description": "Gets a list of the absolute file paths that are currently open (usually as tabs) in the IDE. Returns \na list of absolute file paths separated by newline symbol.",
         "inputSchema": s(json!({}), &[])},
        {"name": "reformat_file", "description": "Reformats the specified file (entire file) in the IDE given an absolute file path.",
         "inputSchema": s(json!({"file_path": {"type": "string"}}), &["file_path"])},
        {"name": "getDiagnostics", "description": "Gets diagnostic info.",
         "inputSchema": s(json!({"uri": {"type": "string"}}), &[])},
    ])
}

// ---------------------------------------------------------------- app side

impl Ide {
    /// Answer pending tool calls from connected `claude` sessions.
    pub(crate) fn claude_calls(&mut self) {
        let Some(c) = &self.claude else { return };
        let calls: Vec<Call> = c.calls.try_iter().collect();
        for call in calls {
            let a = &call.args;
            let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
            match call.name.as_str() {
                "openFile" => {
                    let p = to_os(&s("filePath"));
                    self.open_file(p, None);
                    let (start, end) = (s("startText"), s("endText"));
                    if let Some(tab) = self.tabs.get_mut(self.active)
                        && !start.is_empty()
                        && let Some(b) = tab.text.find(&start)
                    {
                        let e = if end.is_empty() { b + start.len() } else { tab.text[b..].find(&end).map_or(b + start.len(), |x| b + x + end.len()) };
                        tab.select = Some((tab.text[..b].chars().count(), tab.text[..e].chars().count()));
                    }
                    let _ = call.reply.send(vec!["OK".into()]);
                }
                "open_files" => {
                    for p in a["file_paths"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
                        self.open_file(to_os(p), None);
                    }
                    let _ = call.reply.send(vec!["OK".into()]);
                }
                "get_all_opened_file_paths" => {
                    let paths: Vec<String> = self.tabs.iter().filter(|t| t.is_text()).map(|t| t.path.display().to_string()).collect();
                    let _ = call.reply.send(vec![paths.join("\n")]);
                }
                "close_tab" => {
                    let name = s("tab_name");
                    let key = path_key(&to_os(&name));
                    let i = self.tabs.iter().position(|t| t.diff.as_ref().is_some_and(|d| d.title.as_deref() == Some(name.as_str())))
                        .or_else(|| self.tabs.iter().position(|t| t.is_text() && path_key(&t.path) == key));
                    let ok = i.is_some();
                    if let Some(i) = i {
                        self.close_tab(i);
                    }
                    let _ = call.reply.send(vec![if ok { "OK" } else { "Tab not found" }.into()]);
                }
                "getDiagnostics" => {
                    let path = if s("uri").is_empty() { self.tabs.get(self.active).map(|t| t.path.clone()) } else { Some(to_os(&s("uri"))) };
                    let out = match (path, &self.insp) {
                        (Some(p), Some(insp)) => {
                            let diags: Vec<Value> = insp.for_file(&p).into_iter().map(|d| {
                                let severity = ["Error", "Error", "Warning", "Info", "Hint"].get(d.severity as usize).copied().unwrap_or("Info");
                                json!({
                                "message": d.message,
                                "severity": severity,
                                "range": {"start": {"line": d.line, "character": d.col}, "end": {"line": d.end_line, "character": d.end_col}},
                            })}).collect();
                            json!([{"uri": lsp::uri(&p), "diagnostics": diags}]).to_string()
                        }
                        _ => "File not found".into(),
                    };
                    let _ = call.reply.send(vec![out]);
                }
                "openDiff" => {
                    let path = to_os(&s("old_file_path"));
                    let new_text = s("new_file_contents").replace("\r\n", "\n");
                    let old = self.tabs.iter().find(|t| t.is_text() && t.path == path).map(|t| t.text.clone())
                        .or_else(|| std::fs::read_to_string(&path).ok().map(|x| x.replace("\r\n", "\n")))
                        .unwrap_or_default();
                    let mut tab = Tab::new(path, String::new(), false);
                    let mut d = git::Diff::new(&old, &new_text);
                    d.title = Some(s("tab_name"));
                    d.claude = Some((call.reply, new_text));
                    tab.diff = Some(Box::new(d));
                    self.tabs.push(tab);
                    self.active = self.tabs.len() - 1;
                    self.notify("O Claude Code propõe uma mudança: revise e aceite ou rejeite".into());
                    // reply happens on Accept / Reject / tab close
                }
                _ => {
                    // reformat_file and anything newer: acknowledge like the plugin does
                    let _ = call.reply.send(vec!["OK".into()]);
                }
            }
        }
    }

    /// `selection_changed` whenever the caret/selection moves (what Claude sees as "current selection").
    pub(crate) fn claude_selection(&mut self) {
        let (Some(c), Some(tab)) = (&self.claude, self.tabs.get(self.active)) else { return };
        if !tab.is_text() || !c.connected() {
            return;
        }
        let key = (tab.path.clone(), tab.sel);
        if self.claude_sel.as_ref() == Some(&key) {
            return;
        }
        let (a, b) = tab.sel;
        let pos = |ch: usize| {
            let l = line_of(&tab.text, ch);
            json!({"line": l, "character": ch - char_at(&tab.text, l, 0)})
        };
        let text: String = tab.text.chars().skip(a).take(b - a).collect();
        c.notify("selection_changed", json!({
            "text": text, "filePath": tab.path.display().to_string(), "fileUrl": lsp::uri(&tab.path),
            "selection": {"start": pos(a), "end": pos(b), "isEmpty": a == b},
        }));
        self.claude_sel = Some(key);
    }

    /// Ctrl+Alt+K: insert an @-mention of the current file/selection into the Claude prompt.
    pub(crate) fn claude_mention(&mut self) {
        let (Some(c), Some(tab)) = (&self.claude, self.tabs.get(self.active)) else { return };
        let (a, b) = tab.sel;
        let mut params = json!({"filePath": tab.path.display().to_string()});
        if a != b {
            params["lineStart"] = json!(line_of(&tab.text, a));
            params["lineEnd"] = json!(line_of(&tab.text, b));
        }
        c.notify("at_mentioned", params);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn paths() {
        assert_eq!(to_wsl(Path::new(r"C:\Users\me\app")), "/mnt/c/Users/me/app");
        if cfg!(windows) {
            assert_eq!(to_os("/mnt/c/Users/me/app/a.php"), PathBuf::from("C:/Users/me/app/a.php"));
            assert_eq!(to_os("file:///C:/x/y.rs"), PathBuf::from("C:/x/y.rs"));
        }
    }

    /// End-to-end over a real socket: auth, MCP initialize, tools/list, tools/call answered by "the UI".
    #[test]
    fn websocket_mcp_roundtrip() {
        let ws_dir = std::env::temp_dir();
        let c = Claude::start(eframe::egui::Context::default(), &ws_dir).unwrap();
        let lock_path = lock_dir().join(format!("{}.lock", c.port));
        let lock: Value = serde_json::from_str(&std::fs::read_to_string(&lock_path).unwrap()).unwrap();
        assert_eq!(lock["transport"], "ws");
        assert_eq!(lock["workspaceFolders"].as_array().unwrap().len(), if cfg!(windows) { 2 } else { 1 });
        #[cfg(unix)]
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&lock_path).unwrap().permissions()) & 0o777, 0o600);
        let token = lock["authToken"].as_str().unwrap().to_string();

        // wrong token is refused at the handshake
        let mut bad = TcpStream::connect(("127.0.0.1", c.port)).unwrap();
        write!(bad, "GET / HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: mcp\r\nX-Claude-Code-Ide-Authorization: nope\r\n\r\n").unwrap();
        let mut head = [0u8; 12];
        bad.read_exact(&mut head).unwrap();
        assert!(String::from_utf8_lossy(&head).contains("401"));

        let mut req = tungstenite::client::IntoClientRequest::into_client_request(format!("ws://127.0.0.1:{}/", c.port)).unwrap();
        req.headers_mut().insert("Sec-WebSocket-Protocol", "mcp".parse().unwrap());
        req.headers_mut().insert("X-Claude-Code-Ide-Authorization", token.parse().unwrap());
        let (mut ws, resp) = tungstenite::client(req, TcpStream::connect(("127.0.0.1", c.port)).unwrap()).unwrap();
        assert_eq!(resp.headers()["sec-websocket-protocol"], "mcp");
        let mut call = |m: Value| -> Value {
            ws.send(Message::text(m.to_string())).unwrap();
            loop {
                if let Message::Text(t) = ws.read().unwrap() {
                    return serde_json::from_str(&t).unwrap();
                }
            }
        };
        let init = call(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}));
        assert_eq!(init["result"]["serverInfo"]["name"], APP_NAME);
        let list = call(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 7);
        let ui = std::thread::spawn(move || {
            let call = c.calls.recv().unwrap();
            assert_eq!(call.name, "get_all_opened_file_paths");
            call.reply.send(vec!["/a.php".into()]).unwrap();
            c
        });
        let r = call(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "get_all_opened_file_paths", "arguments": {}}}));
        assert_eq!(r["result"]["content"][0]["text"], "/a.php");
        let c = ui.join().unwrap();
        assert!(c.connected());
        let port = c.port;
        drop(c);
        assert!(!lock_path.exists(), "lock file removed on shutdown");
        // the node is gone: the CLI is told, the threads end and the port is free again
        assert!(matches!(ws.read(), Ok(Message::Close(_)) | Err(_)));
        assert!((0..100).any(|_| {
            std::thread::sleep(Duration::from_millis(20));
            TcpListener::bind(("127.0.0.1", port)).is_ok()
        }));
    }
}
