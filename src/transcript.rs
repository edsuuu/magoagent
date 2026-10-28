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
    /// Claude Code's own note: a slash command typed, the conversation summarized.
    System(String),
    /// What failed: the API (the last of its retries) or a tool.
    Error(String),
}

/// How an API error starts: a retry's replaces the one before it.
const API_ERROR: &str = "Erro da API";

/// Newest messages kept in memory per conversation.
const KEEP: usize = 400;
/// On first open only the end of the file is read: they reach 100 MB.
const FIRST_READ: u64 = 2 << 20;

pub(crate) fn claude_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).or_else(|| std::env::home_dir().map(|h| h.join(".claude")))
}

/// The folder name Claude Code files a working directory's sessions under.
pub(crate) fn project_folder(cwd: &str) -> String {
    cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// A Claude Code process as ~/.claude/sessions/<pid>.json describes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Session {
    pub(crate) id: String,
    pub(crate) cwd: String,
    /// ms since 1970
    pub(crate) started: i64,
    pub(crate) pid: u32,
    /// What Claude Code says it is doing: "busy", "idle" or "waiting" (empty in older versions).
    pub(crate) status: String,
    /// While "waiting": for what ("input needed", a permission…).
    pub(crate) waiting_for: Option<String>,
    /// When the status last changed, ms since 1970.
    pub(crate) status_at: i64,
}

/// Only the ones a terminal runs: the desktop app ("claude-desktop") and headless runs ("sdk-cli")
/// file sessions here too, and would take a terminal's place in its folder.
impl Session {
    /// Where it writes what is said: <claude dir>/projects/<folder>/<id>.jsonl.
    pub(crate) fn transcript(&self, dir: &Path) -> PathBuf {
        dir.join("projects").join(project_folder(&self.cwd)).join(format!("{}.jsonl", self.id))
    }
}

pub(crate) fn sessions(dir: &Path) -> Vec<Session> {
    let read = |path: PathBuf| -> Option<Session> {
        let s: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        if !s["entrypoint"].as_str().is_none_or(|e| e == "cli") {
            return None;
        }
        Some(Session {
            id: s["sessionId"].as_str()?.into(),
            cwd: s["cwd"].as_str()?.into(),
            started: s["startedAt"].as_i64().unwrap_or(0),
            pid: s["pid"].as_u64().unwrap_or(0) as u32,
            status: s["status"].as_str().unwrap_or_default().into(),
            waiting_for: s["waitingFor"].as_str().map(Into::into),
            status_at: s["statusUpdatedAt"].as_i64().unwrap_or(0),
        })
        .filter(|s| alive(s.pid))
    };
    std::fs::read_dir(dir).map(|d| d.flatten().filter_map(|e| read(e.path())).collect()).unwrap_or_default()
}

/// Whether process `pid` still runs: a claude that crashed leaves its file behind, saying busy.
/// 0 (a file without one) counts as running, and so does a process that isn't ours to look at.
// ponytail: a dead claude's pid reused by another program reads as running; the process's start
// time against `startedAt` would tell them apart
fn alive(pid: u32) -> bool {
    if pid == 0 {
        return true;
    }
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
            fn GetExitCodeProcess(process: isize, code: *mut u32) -> i32;
            fn CloseHandle(handle: isize) -> i32;
        }
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;
        const ERROR_ACCESS_DENIED: i32 = 5;
        // SAFETY: the handle is checked before use and closed once
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return std::io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED);
            }
            let mut code = 0;
            let read = GetExitCodeProcess(process, &mut code) != 0;
            CloseHandle(process);
            !read || code == STILL_ACTIVE
        }
    }
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        const EPERM: i32 = 1;
        // SAFETY: signal 0 only asks whether the process exists
        let found = unsafe { kill(pid as i32, 0) } == 0;
        found || std::io::Error::last_os_error().raw_os_error() == Some(EPERM)
    }
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
    /// The title is the person's (/rename): the summaries' titles don't replace it.
    named: bool,
    /// The tools called and not answered yet, oldest first: (call id, "name input").
    pub(crate) running: Vec<(String, String)>,
}

impl Transcript {
    pub(crate) fn new(path: PathBuf) -> Transcript {
        Transcript { path, offset: u64::MAX, rest: Vec::new(), items: Vec::new(), title: None, named: false, running: Vec::new() }
    }

    /// Reads only what is appended from now on (an ask over the bridge waits for its answer).
    pub(crate) fn from_end(path: PathBuf) -> Transcript {
        let offset = std::fs::metadata(&path).map_or(0, |m| m.len());
        Transcript { offset, ..Transcript::new(path) }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// The agent's words read so far, paragraphs apart, with the API's errors (its only answer
    /// then); None before it said any.
    pub(crate) fn agent_words(&self) -> Option<String> {
        let words: Vec<&str> = self
            .items
            .iter()
            .filter_map(|item| match item {
                ChatItem::Agent(text) => Some(text.as_str()),
                ChatItem::Error(text) if text.starts_with(API_ERROR) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        (!words.is_empty()).then(|| words.join("\n\n"))
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
                let title = match value["type"].as_str() {
                    Some("custom-title") => value["customTitle"].as_str(),
                    Some("ai-title") if !self.named => value["aiTitle"].as_str(),
                    _ => None,
                };
                if let Some(title) = title {
                    self.named |= value["type"] == "custom-title";
                    self.title = Some(title.to_string());
                }
                let api_error = value["subtype"] == "api_error" || value["isApiErrorMessage"] == true;
                if api_error && matches!(self.items.last(), Some(ChatItem::Error(e)) if e.starts_with(API_ERROR)) {
                    self.items.pop();
                }
                let said = chat_items(&value);
                // the person's new message starts a turn: what the last one left unanswered (claude
                // killed mid-call) is over
                if said.iter().any(|item| matches!(item, ChatItem::User(_))) {
                    self.running.clear();
                }
                self.items.extend(said);
                let side = value["isSidechain"] == true || value["isMeta"] == true;
                for block in value["message"]["content"].as_array().filter(|_| !side).map(Vec::as_slice).unwrap_or_default() {
                    match (block["type"].as_str(), block["id"].as_str()) {
                        (Some("tool_use"), Some(id)) => {
                            let what = format!("{} {}", block["name"].as_str().unwrap_or_default(), summary(&block["input"]));
                            self.running.push((id.into(), what.trim_end().into()));
                        }
                        (Some("tool_result"), _) => self.running.retain(|(id, _)| block["tool_use_id"] != id.as_str()),
                        _ => {}
                    }
                }
            }
        }
        self.rest.drain(..end);
        let extra = self.items.len().saturating_sub(KEEP);
        self.items.drain(..extra);
    }
}

/// What one line of the file adds to the chat: the person's and the agent's words (typed while it
/// worked too), the tools it ran, images, slash commands, summaries, refusals, interruptions and
/// what failed. Tool results that worked, thinking, side conversations and bookkeeping lines are
/// skipped.
pub(crate) fn chat_items(line: &Value) -> Vec<ChatItem> {
    let queued = &line["attachment"];
    if line["type"] == "attachment" && queued["type"] == "queued_command" && queued["commandMode"] == "prompt" {
        let text = unpasted(queued["prompt"].as_str().unwrap_or_default());
        return (!text.trim().is_empty()).then(|| ChatItem::User(text.trim().into())).into_iter().collect();
    }
    if line["type"] == "system" && line["subtype"] == "api_error" {
        let what = line["error"]["formatted"].as_str().or(line["error"]["message"].as_str()).unwrap_or("sem detalhes");
        let attempt = match (line["retryAttempt"].as_u64(), line["maxRetries"].as_u64()) {
            (Some(n), Some(max)) => format!(" (tentativa {n} de {max})"),
            _ => String::new(),
        };
        return vec![ChatItem::Error(format!("{API_ERROR}{attempt}: {}", one_line(what, 160)))];
    }
    let user = match line["type"].as_str() {
        Some("user") => true,
        Some("assistant") => false,
        _ => return Vec::new(),
    };
    if line["isSidechain"] == true || line["isMeta"] == true {
        return Vec::new();
    }
    if line["isCompactSummary"] == true {
        return vec![ChatItem::System("A conversa foi resumida para caber no contexto.".into())];
    }
    // the API's last word after its retries: written as the agent's ("API Error: 529 …")
    if line["isApiErrorMessage"] == true {
        let content = &line["message"]["content"];
        let text = content.as_str().or_else(|| content.as_array()?.iter().find_map(|b| b["text"].as_str())).unwrap_or("sem detalhes");
        return vec![ChatItem::Error(format!("{API_ERROR}: {}", one_line(text.trim().trim_start_matches("API Error:").trim(), 160)))];
    }
    let words = |text: &str| {
        let text = if user { unpasted(text) } else { text.into() };
        let text = text.trim();
        // slash commands and Claude Code's notes arrive as tags: the command shows, the notes don't
        if whole_tag(text) {
            return slash_command(text).filter(|_| user).map(ChatItem::System);
        }
        if user && text.starts_with("[Request interrupted by user") {
            return Some(ChatItem::System("Interrompido pela pessoa.".into()));
        }
        (!text.is_empty()).then(|| if user { ChatItem::User(text.into()) } else { ChatItem::Agent(text.into()) })
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
            "tool_result" if block["is_error"] == true => {
                let content = &block["content"];
                let text = content.as_str().or_else(|| content.as_array()?.iter().find_map(|b| b["text"].as_str())).unwrap_or_default();
                if text.starts_with("The user doesn't want to proceed with this tool use") {
                    return Some(ChatItem::System("Você recusou o uso da ferramenta.".into()));
                }
                Some(ChatItem::Error(format!("Ferramenta falhou: {}", one_line(untag(text.trim()), 160))))
            }
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
    one_line(field.or_else(first).unwrap_or_default(), 90)
}

/// The first line, cut at `max` characters.
fn one_line(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or_default();
    match line.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_string(),
    }
}

/// Text that is only tags (`<name>…</name>`, one after another): Claude Code's notes, not
/// anyone's words.
fn whole_tag(text: &str) -> bool {
    let mut rest = text.trim();
    while !rest.is_empty() {
        let name = rest.strip_prefix('<').and_then(|t| t.split_once('>')).map_or("", |(name, _)| name);
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c == '_') {
            return false;
        }
        let close = format!("</{name}>");
        let Some(end) = rest.find(&close) else { return false };
        rest = rest[end + close.len()..].trim_start();
    }
    !text.trim().is_empty()
}

/// The person's text without the marks around what they pasted (`<pasted_content id="1">`).
fn unpasted(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = [rest.find("<pasted_content"), rest.find("</pasted_content")].into_iter().flatten().min() {
        let Some(end) = rest[at..].find('>') else { break };
        out.push_str(&rest[..at]);
        rest = &rest[at + end + 1..];
    }
    out + rest
}

/// "<x>words</x>" → "words".
fn untag(text: &str) -> &str {
    if !whole_tag(text) {
        return text;
    }
    let start = text.find('>').map_or(0, |i| i + 1);
    let end = text.rfind("</").filter(|&end| end >= start).unwrap_or(text.len());
    text[start..end].trim()
}

/// A slash command typed in the terminal ("/model opus"), from the tags it comes in.
fn slash_command(text: &str) -> Option<String> {
    let inner = |tag: &str| Some(text.split_once(&format!("<{tag}>"))?.1.split_once(&format!("</{tag}>"))?.0.trim());
    let command = format!("{} {}", inner("command-name")?, inner("command-args").unwrap_or_default());
    Some(command.trim_end().to_string())
}
