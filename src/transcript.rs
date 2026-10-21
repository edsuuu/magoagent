//! A Claude Code conversation read from Claude Code's own files, for the chat view: which session
//! a terminal runs (~/.claude/sessions/<pid>.json says the folder and when it started) and what
//! was said (~/.claude/projects/<folder>/<session>.jsonl), read from where the last read stopped.
//! The files are Claude Code's, undocumented: anything unexpected is skipped, never fatal.

use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// One thing to show in the chat.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ChatItem {
    User(String),
    Agent(String),
    /// A tool the agent ran: its name and a one-line summary of what it got.
    Tool(String, String),
    /// An image as the file has it: base64 and its media type; `user` when the person sent it.
    Image { data: String, media: String, user: bool },
}

/// Newest messages kept in memory per conversation.
const KEEP: usize = 400;
/// On first open only the end of the file is read: they reach 100 MB.
const FIRST_READ: u64 = 2 << 20;

pub(crate) fn claude_dir() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).or_else(|| home.map(|h| PathBuf::from(h).join(".claude")))
}

/// The folder name Claude Code files a working directory's sessions under.
pub(crate) fn project_folder(cwd: &str) -> String {
    cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// A Claude Code process as ~/.claude/sessions/<pid>.json describes it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Session {
    pub(crate) id: String,
    pub(crate) cwd: String,
    /// ms since 1970
    pub(crate) started: i64,
}

/// Only the ones a terminal runs: the desktop app ("claude-desktop") and headless runs ("sdk-cli")
/// file sessions here too, and would take a terminal's place in its folder.
pub(crate) fn sessions(dir: &Path) -> Vec<Session> {
    let read = |path: PathBuf| -> Option<Session> {
        let s: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        if !s["entrypoint"].as_str().is_none_or(|e| e == "cli") {
            return None;
        }
        Some(Session { id: s["sessionId"].as_str()?.into(), cwd: s["cwd"].as_str()?.into(), started: s["startedAt"].as_i64().unwrap_or(0) })
    };
    std::fs::read_dir(dir).map(|d| d.flatten().filter_map(|e| read(e.path())).collect()).unwrap_or_default()
}

/// Which session each terminal runs: `terminals` are (id, folder, started ms) of the ones running
/// Claude Code. Newest terminal first, each takes the newest session started in its folder since
/// it opened that no newer terminal took. Folders are compared resolved: Claude Code records the
/// physical path (/private/tmp on macOS), the terminal what was typed (/tmp).
// ponytail: guessed by folder and time; two Claude terminals in one folder restarted out of order
// can swap, and a Claude started outside the app (or a crashed one's leftover file) in the same
// folder can take a terminal's place. Following each session pid's parents up to the terminal's
// shell (Pty::child) would settle both.
pub(crate) fn assign(terminals: &[(String, String, i64)], sessions: &[Session]) -> Vec<(String, Session)> {
    let real = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| p.into());
    let mut order: Vec<&(String, String, i64)> = terminals.iter().collect();
    order.sort_by_key(|t| std::cmp::Reverse(t.2));
    let mut taken: Vec<Session> = Vec::new();
    order
        .into_iter()
        .filter_map(|(id, cwd, since)| {
            let cwd = real(cwd);
            let free = sessions.iter().filter(|s| real(&s.cwd) == cwd && s.started >= since - 2000 && !taken.contains(s));
            let pick = free.max_by_key(|s| s.started)?.clone();
            taken.push(pick.clone());
            Some((id.clone(), pick))
        })
        .collect()
}

pub(crate) struct Transcript {
    path: PathBuf,
    offset: u64,
    /// Bytes after the last newline read: a poll can land inside a line, even inside a character.
    rest: Vec<u8>,
    pub(crate) items: Vec<ChatItem>,
    pub(crate) title: Option<String>,
}

impl Transcript {
    pub(crate) fn new(path: PathBuf) -> Transcript {
        Transcript { path, offset: u64::MAX, rest: Vec::new(), items: Vec::new(), title: None }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Reads what was appended since the last call.
    pub(crate) fn poll(&mut self) {
        let Ok(mut file) = std::fs::File::open(&self.path) else { return };
        let len = file.metadata().map_or(0, |m| m.len());
        let first = self.offset == u64::MAX;
        if first || len < self.offset {
            // first read, or the file was rewritten: start near its end
            self.offset = len.saturating_sub(FIRST_READ);
            self.rest.clear();
        }
        if len == self.offset || file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut bytes = Vec::new();
        if file.take(len - self.offset).read_to_end(&mut bytes).is_err() {
            return;
        }
        let skip_partial = first && self.offset > 0;
        self.offset = len;
        self.rest.append(&mut bytes);
        // whole lines only; what follows the last newline waits for the rest
        let end = self.rest.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        for line in self.rest[..end].split(|b| *b == b'\n').skip(skip_partial as usize) {
            if let Ok(value) = serde_json::from_slice::<Value>(line) {
                if let Some(title) = value["aiTitle"].as_str().filter(|_| value["type"] == "ai-title") {
                    self.title = Some(title.to_string());
                }
                self.items.extend(chat_items(&value));
            }
        }
        self.rest.drain(..end);
        let extra = self.items.len().saturating_sub(KEEP);
        self.items.drain(..extra);
    }
}

/// What one line of the file adds to the chat: the person's and the agent's words, the tools it
/// ran and images. Tool results, thinking, side conversations and bookkeeping lines are skipped.
pub(crate) fn chat_items(line: &Value) -> Vec<ChatItem> {
    let user = match line["type"].as_str() {
        Some("user") => true,
        Some("assistant") => false,
        _ => return Vec::new(),
    };
    if line["isSidechain"] == true || line["isMeta"] == true {
        return Vec::new();
    }
    let words = |text: &str| {
        let text = text.trim();
        // slash commands and system notes arrive as tags
        (!text.is_empty() && !text.starts_with('<')).then(|| if user { ChatItem::User(text.into()) } else { ChatItem::Agent(text.into()) })
    };
    let content = &line["message"]["content"];
    if let Some(text) = content.as_str() {
        return words(text).into_iter().collect();
    }
    let blocks = content.as_array().map(Vec::as_slice).unwrap_or_default();
    blocks
        .iter()
        .filter_map(|block| match block["type"].as_str()? {
            "text" => words(block["text"].as_str()?),
            "tool_use" => Some(ChatItem::Tool(block["name"].as_str()?.into(), summary(&block["input"]))),
            "image" => Some(ChatItem::Image {
                data: block["source"]["data"].as_str()?.into(),
                media: block["source"]["media_type"].as_str().unwrap_or("image/png").into(),
                user,
            }),
            _ => None,
        })
        .collect()
}

/// A tool's input in one line: the command, file, pattern or address it was given.
fn summary(input: &Value) -> String {
    let field = ["command", "file_path", "pattern", "url", "description", "prompt"].iter().find_map(|k| input[*k].as_str());
    let first = || input.as_object().and_then(|o| o.values().find_map(Value::as_str));
    let line = field.or_else(first).unwrap_or_default().lines().next().unwrap_or_default();
    match line.char_indices().nth(90) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_string(),
    }
}
