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
    /// An image as the file has it: base64 and its media type.
    Image { data: String, media: String },
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

/// The newest session started in `cwd` since `since_ms` (ms since 1970), and its status
/// ("busy", "idle"…).
// ponytail: two sessions started in the same folder after the terminal opened: the newest wins;
// walking each session pid's parents up to the terminal's shell would settle it (unix only)
pub(crate) fn find_session(sessions: &Path, cwd: &str, since_ms: i64) -> Option<(String, String)> {
    let started = |s: &Value| s["startedAt"].as_i64().unwrap_or(0);
    std::fs::read_dir(sessions)
        .ok()?
        .flatten()
        .filter_map(|e| serde_json::from_str::<Value>(&std::fs::read_to_string(e.path()).ok()?).ok())
        .filter(|s| s["cwd"].as_str() == Some(cwd) && started(s) >= since_ms - 2000)
        .max_by_key(started)
        .and_then(|s| Some((s["sessionId"].as_str()?.to_string(), s["status"].as_str().unwrap_or_default().to_string())))
}

pub(crate) struct Transcript {
    path: PathBuf,
    offset: u64,
    rest: String,
    pub(crate) items: Vec<ChatItem>,
    pub(crate) title: Option<String>,
}

impl Transcript {
    pub(crate) fn new(path: PathBuf) -> Transcript {
        Transcript { path, offset: u64::MAX, rest: String::new(), items: Vec::new(), title: None }
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
        self.rest.push_str(&String::from_utf8_lossy(&bytes));
        let text = std::mem::take(&mut self.rest);
        let mut lines: Vec<&str> = text.split('\n').collect();
        self.rest = lines.pop().unwrap_or_default().to_string(); // an unfinished last line waits
        for line in lines.into_iter().skip(skip_partial as usize) {
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                if let Some(title) = value["aiTitle"].as_str().filter(|_| value["type"] == "ai-title") {
                    self.title = Some(title.to_string());
                }
                self.items.extend(chat_items(&value));
            }
        }
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
