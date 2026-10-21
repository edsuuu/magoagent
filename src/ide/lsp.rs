//! Built-in inspections, no plugins: an LSP client (the protocol VSCode/Zed speak) that
//! auto-starts the right server per language, plus PHPStan on save for PHP projects
//! (picks up phpstan.neon / Larastan automatically).

use super::highlight::Lang;
use eframe::egui;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Clone)]
pub struct Diag {
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub severity: u8, // 1 error, 2 warning, 3 info, 4 hint (LSP)
    pub message: String,
    pub source: String,
}

/// What a request in flight asked the server.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ask {
    Definition,
    Usages,
    Implementations,
    Hover,
    Signature,
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    ready: bool,
    queue: Vec<Value>,
}

impl Drop for Server {
    /// The IDE node went away (or its folder changed): no shutdown/exit dance, the process just ends.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Inspections {
    root: PathBuf,
    ctx: egui::Context,
    servers: HashMap<&'static str, Server>,
    tx: Sender<(&'static str, Value)>,
    rx: Receiver<(&'static str, Value)>,
    /// (path key, origin) -> (path, diagnostics)
    diags: HashMap<(String, &'static str), (PathBuf, Vec<Diag>)>,
    versions: HashMap<String, i32>,
    /// language name -> install hint, for languages with no server on PATH
    pub missing: HashMap<&'static str, &'static str>,
    pub status: String,
    next_id: i64,
    /// requests in flight: id -> (server, what for)
    pending: HashMap<i64, (&'static str, Ask)>,
    /// answer to the last go-to-definition: (path, line, col)
    pub jump: Option<Option<(PathBuf, usize, usize)>>,
    /// answer to the last usages / implementations request
    pub usages: Option<(Ask, Vec<(PathBuf, usize, usize)>)>,
    /// the documentation for the word asked about: (path, line, col) and the text
    pub hover: Option<((PathBuf, usize, usize), String)>,
    hover_at: Option<(PathBuf, usize, usize)>,
    /// the call being typed: its signature and the active parameter's byte range in it
    pub signature: Option<(String, Option<(usize, usize)>)>,
}

impl Inspections {
    pub fn new(ctx: egui::Context, root: PathBuf) -> Self {
        let (tx, rx) = channel();
        Self {
            root,
            ctx,
            servers: HashMap::new(),
            tx,
            rx,
            diags: HashMap::new(),
            versions: HashMap::new(),
            missing: HashMap::new(),
            status: String::new(),
            next_id: 1,
            pending: HashMap::new(),
            jump: None,
            usages: None,
            hover: None,
            hover_at: None,
            signature: None,
        }
    }

    /// Picks the first installed server for `lang`, starting it if needed.
    fn server(&mut self, lang: &'static Lang) -> Option<&'static str> {
        let (argv, hint) = lang.servers.iter().find(|(argv, _)| self.servers.contains_key(argv[0]) || which(argv[0]).is_some())
            .or_else(|| {
                if let Some((_, hint)) = lang.servers.first() {
                    self.missing.insert(lang.name, hint);
                }
                None
            })?;
        let key = argv[0];
        if self.servers.contains_key(key) {
            return Some(key);
        }
        let mut cmd = super::command(which(key)?);
        cmd.args(&argv[1..]).current_dir(&self.root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                self.status = format!("{key}: {e}");
                self.missing.insert(lang.name, hint);
                return None;
            }
        };
        let stdout = child.stdout.take()?;
        let mut stdin = child.stdin.take()?;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || read_loop(key, stdout, tx, ctx));
        let root = uri(&self.root);
        let name = self.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        write(&mut stdin, &json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
            "processId": std::process::id(),
            "rootUri": root, "rootPath": self.root,
            "workspaceFolders": [{"uri": root, "name": name}],
            "capabilities": {
                "textDocument": {
                    "publishDiagnostics": {}, "synchronization": {"didSave": true},
                    "definition": {"linkSupport": true}, "references": {}, "implementation": {"linkSupport": true},
                    "hover": {"contentFormat": ["plaintext", "markdown"]},
                    "signatureHelp": {"signatureInformation": {"documentationFormat": ["plaintext"], "parameterInformation": {"labelOffsetSupport": true}}},
                },
                "workspace": {"configuration": true, "workspaceFolders": true},
            },
        }}));
        self.status = format!("{key} iniciando…");
        self.servers.insert(key, Server { child, stdin, ready: false, queue: vec![] });
        Some(key)
    }

    fn notify(&mut self, key: &'static str, method: &str, params: Value) {
        let msg = json!({"jsonrpc": "2.0", "method": method, "params": params});
        if let Some(s) = self.servers.get_mut(key) {
            if s.ready { write(&mut s.stdin, &msg) } else { s.queue.push(msg) }
        }
    }

    fn active_server(&self, lang: &'static Lang) -> Option<&'static str> {
        lang.servers.iter().map(|(a, _)| a[0]).find(|k| self.servers.contains_key(k))
    }

    /// Ctrl+click / Ctrl+B. Returns false when no server handles this language.
    pub fn definition(&mut self, path: &Path, lang: &'static Lang, line: usize, col: usize) -> bool {
        self.ask(Ask::Definition, path, lang, line, col)
    }

    /// Asks the language's server about the symbol at (line, col); the answer lands in `jump`,
    /// `usages`, `hover` or `signature`. False when no server handles this language.
    pub fn ask(&mut self, ask: Ask, path: &Path, lang: &'static Lang, line: usize, col: usize) -> bool {
        let Some(key) = self.active_server(lang) else { return false };
        let method = match ask {
            Ask::Definition => "textDocument/definition",
            Ask::Usages => "textDocument/references",
            Ask::Implementations => "textDocument/implementation",
            Ask::Hover => "textDocument/hover",
            Ask::Signature => "textDocument/signatureHelp",
        };
        let mut params = json!({"textDocument": {"uri": uri(path)}, "position": {"line": line, "character": col}});
        if ask == Ask::Usages {
            params["context"] = json!({"includeDeclaration": false});
        }
        if ask == Ask::Hover {
            // asked already (the pointer rests on the same word): wait for that answer
            let at = Some((path.to_path_buf(), line, col));
            if self.hover_at == at {
                return true;
            }
            self.hover_at = at;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, (key, ask));
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if let Some(s) = self.servers.get_mut(key) {
            if s.ready { write(&mut s.stdin, &msg) } else { s.queue.push(msg) }
        }
        true
    }

    pub fn open(&mut self, path: &Path, lang: &'static Lang, text: &str) {
        let Some(key) = self.server(lang) else { return };
        self.versions.insert(path_key(path), 1);
        self.notify(key, "textDocument/didOpen", json!({"textDocument": {"uri": uri(path), "languageId": lang.id, "version": 1, "text": text}}));
    }

    pub fn change(&mut self, path: &Path, lang: &'static Lang, text: &str) {
        let Some(key) = lang.servers.iter().map(|(a, _)| a[0]).find(|k| self.servers.contains_key(k)) else { return };
        let v = self.versions.entry(path_key(path)).or_insert(1);
        *v += 1;
        let v = *v;
        self.notify(key, "textDocument/didChange", json!({"textDocument": {"uri": uri(path), "version": v}, "contentChanges": [{"text": text}]}));
    }

    pub fn save(&mut self, path: &Path, lang: &'static Lang) {
        if let Some(key) = lang.servers.iter().map(|(a, _)| a[0]).find(|k| self.servers.contains_key(k)) {
            self.notify(key, "textDocument/didSave", json!({"textDocument": {"uri": uri(path)}}));
        }
        if lang.id == "php" || lang.id == "blade" {
            self.phpstan(path);
        }
    }

    pub fn close(&mut self, path: &Path, lang: &'static Lang) {
        if let Some(key) = lang.servers.iter().map(|(a, _)| a[0]).find(|k| self.servers.contains_key(k)) {
            self.notify(key, "textDocument/didClose", json!({"textDocument": {"uri": uri(path)}}));
        }
    }

    /// PHPStan (and Larastan via phpstan.neon) for the saved file, off the UI thread.
    fn phpstan(&mut self, path: &Path) {
        let bin = self.root.join("vendor/bin/phpstan");
        if !bin.exists() {
            return;
        }
        let Some(php) = which("php") else {
            self.status = "PHPStan encontrado, mas o php não está no PATH".into();
            return;
        };
        let (root, path, tx, ctx) = (self.root.clone(), path.to_path_buf(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let out = super::command(php)
                .args([bin.as_os_str(), "analyse".as_ref(), "--error-format=json".as_ref(), "--no-progress".as_ref(), "--no-interaction".as_ref(), path.as_os_str()])
                .current_dir(root)
                .stderr(Stdio::null())
                .output();
            let Ok(out) = out else { return };
            let report: Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
            let diagnostics: Vec<Value> = report["files"].as_object().into_iter().flat_map(|f| f.values())
                .flat_map(|f| f["messages"].as_array().cloned().unwrap_or_default())
                .map(|m| {
                    let line = m["line"].as_u64().unwrap_or(1).saturating_sub(1);
                    json!({"range": {"start": {"line": line, "character": 0}, "end": {"line": line, "character": 9999}},
                           "severity": 1, "message": m["message"], "source": "phpstan"})
                })
                .collect();
            let _ = tx.send(("phpstan", json!({"method": "textDocument/publishDiagnostics", "params": {"uri": uri(&path), "diagnostics": diagnostics}})));
            ctx.request_repaint();
        });
    }

    pub fn poll(&mut self) {
        while let Ok((key, msg)) = self.rx.try_recv() {
            let method = msg["method"].as_str().unwrap_or("");
            let id = msg.get("id").cloned();
            match (id, method) {
                (Some(id), "") if id == 0 => {
                    if let Some(s) = self.servers.get_mut(key) {
                        s.ready = true;
                        write(&mut s.stdin, &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
                        for m in std::mem::take(&mut s.queue) {
                            write(&mut s.stdin, &m);
                        }
                        self.status = format!("{key} pronto");
                    }
                }
                (Some(id), "") if id.as_i64().is_some_and(|i| self.pending.contains_key(&i)) => {
                    let Some((_, ask)) = id.as_i64().and_then(|i| self.pending.remove(&i)) else { continue };
                    let r = &msg["result"];
                    match ask {
                        Ask::Definition => self.jump = Some(locations(r).into_iter().next()),
                        Ask::Usages | Ask::Implementations => self.usages = Some((ask, locations(r))),
                        Ask::Hover => self.hover = self.hover_at.clone().zip(hover_text(r)),
                        Ask::Signature => self.signature = signature(r),
                    }
                }
                // Requests from the server: answer so it never blocks waiting on us.
                (Some(id), m) if !m.is_empty() => {
                    let result = if m == "workspace/configuration" {
                        json!(vec![Value::Null; msg["params"]["items"].as_array().map_or(0, |a| a.len())])
                    } else {
                        Value::Null
                    };
                    if let Some(s) = self.servers.get_mut(key) {
                        write(&mut s.stdin, &json!({"jsonrpc": "2.0", "id": id, "result": result}));
                    }
                }
                (None, "textDocument/publishDiagnostics") => self.publish(key, &msg["params"]),
                (None, "$exit") => {
                    self.servers.remove(key);
                    self.status = format!("{key} parou");
                }
                _ => {}
            }
        }
    }

    fn publish(&mut self, origin: &'static str, p: &Value) {
        let path = uri_path(p["uri"].as_str().unwrap_or(""));
        let pos = |v: &Value| (v["line"].as_u64().unwrap_or(0) as usize, v["character"].as_u64().unwrap_or(0) as usize);
        let list: Vec<Diag> = p["diagnostics"].as_array().into_iter().flatten().map(|d| {
            let (line, col) = pos(&d["range"]["start"]);
            let (end_line, end_col) = pos(&d["range"]["end"]);
            Diag {
                line, col, end_line, end_col,
                severity: d["severity"].as_u64().unwrap_or(1) as u8,
                message: d["message"].as_str().unwrap_or("").to_string(),
                source: d["source"].as_str().unwrap_or(origin).to_string(),
            }
        }).collect();
        let k = (path_key(&path), origin);
        if list.is_empty() { self.diags.remove(&k); } else { self.diags.insert(k, (path, list)); }
    }

    pub fn for_file(&self, path: &Path) -> Vec<&Diag> {
        let k = path_key(path);
        self.diags.iter().filter(|((p, _), _)| *p == k).flat_map(|(_, (_, d))| d).collect()
    }

    pub fn all(&self) -> impl Iterator<Item = (&PathBuf, &Diag)> {
        self.diags.values().flat_map(|(p, d)| d.iter().map(move |d| (p, d)))
    }
}

fn read_loop(key: &'static str, stdout: impl Read, tx: Sender<(&'static str, Value)>, ctx: egui::Context) {
    let mut r = BufReader::new(stdout);
    loop {
        let mut len = 0;
        loop {
            let mut line = String::new();
            if r.read_line(&mut line).unwrap_or(0) == 0 {
                let _ = tx.send((key, json!({"method": "$exit"})));
                return;
            }
            let line = line.trim();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length:") {
                len = v.trim().parse().unwrap_or(0);
            }
        }
        let mut buf = vec![0; len];
        if r.read_exact(&mut buf).is_err() {
            let _ = tx.send((key, json!({"method": "$exit"})));
            return;
        }
        if let Ok(v) = serde_json::from_slice(&buf) {
            let _ = tx.send((key, v));
            ctx.request_repaint();
        }
    }
}

/// Location | Location[] | LocationLink[] → (path, line, col) of each.
pub fn locations(r: &Value) -> Vec<(PathBuf, usize, usize)> {
    let one = |l: &Value| -> Option<(PathBuf, usize, usize)> {
        let target = l.get("targetUri").or(l.get("uri"))?.as_str()?;
        let start = &l.get("targetSelectionRange").or(l.get("range"))?["start"];
        Some((uri_path(target), start["line"].as_u64()? as usize, start["character"].as_u64()? as usize))
    };
    match r {
        Value::Array(all) => all.iter().filter_map(one).collect(),
        Value::Null => vec![],
        single => one(single).into_iter().collect(),
    }
}

/// A hover's contents (MarkupContent, MarkedString or a list of them) as plain text, without
/// the code fences.
pub fn hover_text(r: &Value) -> Option<String> {
    let piece = |c: &Value| c.as_str().or(c["value"].as_str()).map(str::to_string);
    let text = match &r["contents"] {
        Value::Array(all) => all.iter().filter_map(piece).collect::<Vec<_>>().join("\n\n"),
        one => piece(one)?,
    };
    let text: Vec<&str> = text.lines().filter(|l| !l.trim_start().starts_with("```")).collect();
    let text = text.join("\n").trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// The active signature's label and the byte range of its active parameter in it.
// ponytail: LSP offsets count UTF-16 units; right for ASCII signatures, off by a little otherwise
pub fn signature(r: &Value) -> Option<(String, Option<(usize, usize)>)> {
    let all = r["signatures"].as_array()?;
    let sig = all.get(r["activeSignature"].as_u64().unwrap_or(0) as usize).or(all.first())?;
    let label = sig["label"].as_str()?.to_string();
    let active = sig["activeParameter"].as_u64().or(r["activeParameter"].as_u64()).unwrap_or(0) as usize;
    let range = match &sig["parameters"][active]["label"] {
        Value::Array(at) => at.first().and_then(Value::as_u64).zip(at.get(1).and_then(Value::as_u64)).map(|(a, b)| (a as usize, b as usize)),
        Value::String(name) => label.find(name.as_str()).map(|a| (a, a + name.len())),
        _ => None,
    }
    .filter(|&(a, b)| a <= b && b <= label.len() && label.is_char_boundary(a) && label.is_char_boundary(b));
    Some((label, range))
}

fn write(w: &mut impl Write, v: &Value) {
    let body = v.to_string();
    let _ = write!(w, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = w.flush();
}

/// Finds an executable on PATH (Windows: also .exe/.cmd/.bat, as npm installs .cmd shims).
pub fn which(name: &str) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}"))))
        .find(|p| p.is_file())
}

pub fn uri(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = if s.starts_with('/') { s } else { format!("/{s}") };
    let mut out = String::from("file://");
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) { out.push(b as char) } else { out += &format!("%{b:02X}") }
    }
    out
}

pub fn uri_path(uri: &str) -> PathBuf {
    let s = uri.strip_prefix("file://").unwrap_or(uri).as_bytes();
    let mut bytes = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'%' && i + 2 < s.len() && let Ok(b) = u8::from_str_radix(std::str::from_utf8(&s[i + 1..i + 3]).unwrap_or(""), 16) {
            bytes.push(b);
            i += 3;
        } else {
            bytes.push(s[i]);
            i += 1;
        }
    }
    let s = String::from_utf8_lossy(&bytes).into_owned();
    // "/C:/x" -> "C:/x" on Windows
    let s = if cfg!(windows) && s.as_bytes().get(2) == Some(&b':') { s[1..].to_string() } else { s };
    PathBuf::from(s)
}

pub fn path_key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if cfg!(windows) { s.to_lowercase() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_become_places_docs_and_signatures() {
        let at = |u: &str, l: u64, c: u64| json!({"uri": u, "range": {"start": {"line": l, "character": c}, "end": {"line": l, "character": c}}});
        // Location, Location[] and LocationLink[]
        assert_eq!(locations(&at("file:///a.rs", 3, 4)), vec![(PathBuf::from("/a.rs"), 3, 4)]);
        assert_eq!(locations(&json!([at("file:///a.rs", 1, 0), at("file:///b.rs", 2, 5)])).len(), 2);
        let link = json!([{"targetUri": "file:///c.rs", "targetSelectionRange": {"start": {"line": 9, "character": 1}}}]);
        assert_eq!(locations(&link), vec![(PathBuf::from("/c.rs"), 9, 1)]);
        assert!(locations(&Value::Null).is_empty());

        let markdown = json!({"contents": {"kind": "markdown", "value": "```rust\nfn soma(a: i32) -> i32\n```\nSoma dois números."}});
        assert_eq!(hover_text(&markdown).as_deref(), Some("fn soma(a: i32) -> i32\nSoma dois números."));
        assert_eq!(hover_text(&json!({"contents": ["um", {"language": "php", "value": "dois"}]})).as_deref(), Some("um\n\ndois"));
        assert_eq!(hover_text(&json!({"contents": ""})), None);

        let help = json!({"signatures": [{"label": "soma(a: i32, b: i32)", "parameters": [{"label": [5, 11]}, {"label": "b: i32"}]}], "activeParameter": 1});
        assert_eq!(signature(&help), Some(("soma(a: i32, b: i32)".to_string(), Some((13, 19)))));
        assert_eq!(signature(&json!({"signatures": []})), None);
    }

    #[test]
    fn uri_roundtrip() {
        let p = if cfg!(windows) { Path::new(r"C:\Users\me\My App\ação.php") } else { Path::new("/home/me/My App/ação.php") };
        let u = uri(p);
        assert!(u.starts_with("file:///") && u.contains("My%20App"), "{u}");
        assert_eq!(path_key(&uri_path(&u)), path_key(p));
        // servers may send lowercase drive + encoded colon
        if cfg!(windows) {
            assert_eq!(path_key(&uri_path("file:///c%3A/Users/me/x.php")), "c:/users/me/x.php");
        }
    }
}
