//! A Claude Code conversation read from Claude Code's own files, for the chat view: which session
//! a terminal runs (~/.claude/sessions/<pid>.json says the folder and when it started) and what
//! was said (~/.claude/projects/<folder>/<session>.jsonl), read from where the last read stopped.
//! The files are Claude Code's, undocumented: anything unexpected is skipped, never fatal.

pub(crate) mod process;
pub(crate) mod watch;

use process::alive;
pub(crate) use process::parents;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// One thing to show in the chat.
// a step is ~300 bytes inline: 400 kept items are ~120 KB, not worth a box at every match
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ChatItem {
    User(String),
    Agent(String),
    /// A tool the agent ran, with what its answer said once it came.
    Tool(Step),
    /// An image as the file has it: base64 and its media type; `user` when the person sent it;
    /// `at`, when (ms since 1970, 0 unknown).
    Image {
        data: String,
        media: String,
        user: bool,
        at: i64,
    },
    /// Claude Code's own note: a slash command typed, the conversation summarized.
    System(String),
    /// What failed: the API (the last of its retries) or a tool.
    Error(String),
}

/// A tool call: what it was given, when, and what its answer says (Claude Code's `toolUseResult`).
/// Saved with the Codex chats (`chats/<id>.json`): a field missing there is its default.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Step {
    pub(crate) id: String,
    pub(crate) name: String,
    /// Its input in one line: the command, file, pattern or address it was given.
    pub(crate) what: String,
    /// When it was called and answered, seconds since 1970 (the lines' timestamps).
    pub(crate) started: Option<f64>,
    pub(crate) ended: Option<f64>,
    pub(crate) failed: bool,
    /// What the answer says, short: "212 linhas" (Read), "novo" (Write).
    pub(crate) note: Option<String>,
    /// Lines added and removed (Edit, Write over a file).
    pub(crate) diff: Option<(usize, usize)>,
    /// The last line its command printed so far (Bash, from Claude Code's progress lines).
    pub(crate) output: Option<String>,
    /// Its whole input, every line (the card under its row shows it).
    pub(crate) input: String,
    /// The folder it ran in, when the line says (Claude Code's `cwd`).
    pub(crate) cwd: Option<String>,
    /// What it printed, stdout then stderr: the end only (`LOG_MAX`).
    pub(crate) log: String,
    /// Why it failed: the tool's error, or that it was interrupted.
    pub(crate) error: Option<String>,
    /// The files it names (Read, Edit, Write; a Codex change).
    pub(crate) files: Vec<String>,
}

/// What a step keeps of what it printed or its error: their end.
pub(crate) const LOG_MAX: usize = 16 << 10;

/// The last `max` bytes of `text` (on a character's edge), saying it was cut.
pub(crate) fn log_tail(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut cut = text.len() - max;
    while !text.is_char_boundary(cut) {
        cut += 1;
    }
    format!("… (cortado: só os últimos {} KB)\n{}", max >> 10, &text[cut..])
}

impl Step {
    /// Its answer: `result` is the line's `toolUseResult`, whose shape each tool decides; `said`,
    /// the answer's text (a failure's reason).
    fn finish(&mut self, result: &Value, failed: bool, said: &str, at: Option<f64>) {
        (self.ended, self.failed) = (at, failed);
        // Bash: what it printed, and whether it was stopped
        let printed: Vec<&str> =
            [&result["stdout"], &result["stderr"]].iter().filter_map(|v| v.as_str()).map(str::trim_end).filter(|s| !s.is_empty()).collect();
        if !printed.is_empty() {
            self.log = log_tail(&printed.join("\n"), LOG_MAX);
        }
        if failed {
            self.error = Some(log_tail(untag(said.trim()), LOG_MAX)).filter(|e| !e.is_empty());
        } else if result["interrupted"] == true {
            self.error = Some("Interrompido".into());
        }
        if let Some(n) = result["file"]["numLines"].as_u64() {
            self.note = Some(if n == 1 { "1 linha".into() } else { format!("{n} linhas") });
        }
        if result["type"] == "create" {
            self.note = Some("novo".into());
        } else if let Some(hunks) = result["structuredPatch"].as_array() {
            let lines = hunks.iter().flat_map(|h| h["lines"].as_array().map(Vec::as_slice).unwrap_or_default()).filter_map(Value::as_str);
            let count = |mark: char| lines.clone().filter(|l| l.starts_with(mark)).count();
            self.diff = Some((count('+'), count('-')));
        }
    }
}

/// The step of a tool call still in the chat, by its id.
pub(crate) fn step_mut<'a>(items: &'a mut [ChatItem], id: &str) -> Option<&'a mut Step> {
    items.iter_mut().rev().find_map(|item| match item {
        ChatItem::Tool(step) if step.id == id && !id.is_empty() => Some(step),
        _ => None,
    })
}

/// "2026-10-28T20:05:00.123Z" in seconds since 1970 (UTC, as Claude Code writes them).
pub(crate) fn unix_seconds(iso: &str) -> Option<f64> {
    let num = |range: std::ops::Range<usize>| iso.get(range)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let seconds: f64 = iso.get(17..)?.trim_end_matches('Z').parse().ok()?;
    // days since 1970-01-01 (Howard Hinnant's days_from_civil)
    let y = if m <= 2 { y - 1 } else { y };
    let (era, yoe) = (y.div_euclid(400), y.rem_euclid(400));
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some((days * 86_400 + num(11..13)? * 3600 + num(14..16)? * 60) as f64 + seconds)
}

/// How an API error starts: a retry's replaces the one before it.
const API_ERROR: &str = "Erro da API";

/// Newest messages kept in memory per conversation.
const KEEP: usize = 400;
/// Of those, the images that keep their pixels.
const KEEP_IMAGES: usize = 20;
/// On first open only the end of the file is read: they reach 100 MB.
const FIRST_READ: u64 = 2 << 20;
/// Prints make lines of megabytes: an end that holds fewer messages than this is read again,
/// four times longer, up to the most below.
const FIRST_ITEMS: usize = 50;
const FIRST_READ_MAX: u64 = 32 << 20;
/// What MagoAgent's view says under its print of the canvas.
pub(crate) const VIEW_MARK: &str = "imagem com 10px de margem";

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

/// Where Claude Code writes what is said in conversation `id`: <claude dir>/projects/<folder>/<id>.jsonl.
pub(crate) fn transcript_path(dir: &Path, folder: &str, id: &str) -> PathBuf {
    dir.join("projects").join(folder).join(format!("{id}.jsonl"))
}

/// Only the ones a terminal runs: the desktop app ("claude-desktop") and headless runs ("sdk-cli")
/// file sessions here too, and would take a terminal's place in its folder.
impl Session {
    pub(crate) fn transcript(&self, dir: &Path) -> PathBuf {
        transcript_path(dir, &project_folder(&self.cwd), &self.id)
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

/// Which session each terminal runs: `terminals` are (id, folder, started ms, shell pid) of the
/// ones running Claude Code. Newest terminal first, each takes the newest session started in its
/// folder since it opened that no newer terminal took. Folders are compared resolved: Claude Code
/// records the physical path (/private/tmp on macOS), the terminal what was typed (/tmp). A
/// terminal left without one takes the session whose process descends from its shell: resuming a
/// conversation moves Claude Code to that conversation's folder. `parents` is only asked then.
// ponytail: by folder first; two Claude terminals in one folder restarted out of order can swap,
// and a Claude started outside the app in the same folder can take a terminal's place. The process
// tree alone would settle both, at the cost of reading every process twice a second.
pub(crate) fn assign(terminals: &[(String, String, i64, u32)], sessions: &[Session], parents: impl FnOnce() -> HashMap<u32, u32>) -> Vec<(String, Session)> {
    let real = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| p.into());
    let mut order: Vec<&(String, String, i64, u32)> = terminals.iter().collect();
    order.sort_by_key(|t| std::cmp::Reverse(t.2));
    let mut taken: Vec<(String, Session)> = Vec::new();
    for (id, cwd, since, _) in &order {
        let cwd = real(cwd);
        let free = sessions.iter().filter(|s| real(&s.cwd) == cwd && s.started >= since - 2000 && !taken.iter().any(|(_, t)| t == *s));
        if let Some(pick) = free.max_by_key(|s| s.started) {
            taken.push((id.clone(), pick.clone()));
        }
    }
    let left: Vec<_> = order.into_iter().filter(|t| t.3 != 0 && !taken.iter().any(|(id, _)| *id == t.0)).collect();
    if left.is_empty() {
        return taken;
    }
    let parents = parents();
    let descends = |mut pid: u32, shell: u32| {
        for _ in 0..32 {
            match parents.get(&pid) {
                Some(&up) if up == shell => return true,
                Some(&up) if up != 0 && up != pid => pid = up,
                _ => return false,
            }
        }
        false
    };
    for (id, _, _, shell) in left {
        let free = sessions.iter().filter(|s| s.pid != 0 && descends(s.pid, *shell) && !taken.iter().any(|(_, t)| t == *s));
        if let Some(pick) = free.max_by_key(|s| s.started) {
            taken.push((id.clone(), pick.clone()));
        }
    }
    taken
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
    /// How much of the end the first read takes.
    first_read: u64,
}

impl Transcript {
    pub(crate) fn new(path: PathBuf) -> Transcript {
        Transcript { path, offset: u64::MAX, rest: Vec::new(), items: Vec::new(), title: None, named: false, running: Vec::new(), first_read: FIRST_READ }
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
            self.offset = len.saturating_sub(self.first_read);
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
                let at = value["timestamp"].as_str().and_then(unix_seconds);
                // a running command's output so far
                if let (Some(id), Some(out), false) = (value["parentToolUseID"].as_str(), value["data"]["output"].as_str(), side)
                    && let Some(step) = step_mut(&mut self.items, id)
                {
                    step.output = out.lines().rev().map(str::trim_end).find(|l| !l.trim().is_empty()).map(Into::into);
                    step.log = log_tail(out.trim_end(), LOG_MAX);
                }
                let blocks = value["message"]["content"].as_array().filter(|_| !side).map(Vec::as_slice).unwrap_or_default();
                // the line's toolUseResult is its one answer's
                let answers = blocks.iter().filter(|b| b["type"] == "tool_result").count();
                for block in blocks {
                    match (block["type"].as_str(), block["id"].as_str()) {
                        (Some("tool_use"), Some(id)) => {
                            let what = format!("{} {}", block["name"].as_str().unwrap_or_default(), summary(&block["input"]));
                            self.running.push((id.into(), what.trim_end().into()));
                        }
                        (Some("tool_result"), _) => {
                            self.running.retain(|(id, _)| block["tool_use_id"] != id.as_str());
                            let result = if answers == 1 { &value["toolUseResult"] } else { &Value::Null };
                            if let Some(step) = step_mut(&mut self.items, block["tool_use_id"].as_str().unwrap_or_default()) {
                                step.finish(result, block["is_error"] == true, block_text(&block["content"]), at);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        self.rest.drain(..end);
        let extra = self.items.len().saturating_sub(KEEP);
        self.items.drain(..extra);
        // the newest images keep their pixels; older ones a note: base64 is megabytes each
        let mut images = 0;
        for item in self.items.iter_mut().rev() {
            if matches!(item, ChatItem::Image { .. }) {
                images += 1;
                if images > KEEP_IMAGES {
                    *item = ChatItem::System("Imagem antiga, fora da memória.".into());
                }
            }
        }
        // skip_partial: the read started past the file's beginning
        if skip_partial && self.items.len() < FIRST_ITEMS && self.first_read < FIRST_READ_MAX {
            *self = Transcript { first_read: self.first_read * 4, ..Transcript::new(std::mem::take(&mut self.path)) };
            self.poll();
        }
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
    let at = line["timestamp"].as_str().and_then(iso_ms).unwrap_or(0);
    let content = &line["message"]["content"];
    if let Some(text) = content.as_str() {
        return words(text).into_iter().collect();
    }
    let blocks = content.as_array().map(Vec::as_slice).unwrap_or_default();
    let one = |block: &Value| match block["type"].as_str()? {
        "text" => words(block["text"].as_str()?),
        "tool_use" => Some(ChatItem::Tool(Step {
            id: block["id"].as_str().unwrap_or_default().into(),
            name: block["name"].as_str()?.into(),
            what: summary(&block["input"]),
            input: input_text(&block["input"]).into(),
            cwd: line["cwd"].as_str().map(Into::into),
            files: ["file_path", "notebook_path"].iter().find_map(|k| block["input"][*k].as_str()).map(String::from).into_iter().collect(),
            started: line["timestamp"].as_str().and_then(unix_seconds),
            ..Default::default()
        })),
        "tool_result" if block["is_error"] == true => {
            let text = block_text(&block["content"]);
            if text.starts_with("The user doesn't want to proceed with this tool use") {
                return Some(ChatItem::System("Você recusou o uso da ferramenta.".into()));
            }
            Some(ChatItem::Error(format!("Ferramenta falhou: {}", one_line(untag(text.trim()), 160))))
        }
        "image" => image(block, user, at),
        _ => None,
    };
    blocks
        .iter()
        .flat_map(|block| {
            if block["type"] != "tool_result" || block["is_error"] == true {
                return one(block).into_iter().collect();
            }
            // the images a tool gave back (a PNG it read, a page's print), in their place: the agent
            // showing them; not the canvas MagoAgent's view prints, it's right behind the chat
            let parts = block["content"].as_array().map(Vec::as_slice).unwrap_or_default();
            if parts.iter().any(|b| b["text"].as_str().is_some_and(|t| t.contains(VIEW_MARK))) {
                return Vec::new();
            }
            parts.iter().filter_map(|b| image(b, false, at)).collect()
        })
        .collect()
}

fn image(block: &Value, user: bool, at: i64) -> Option<ChatItem> {
    (block["type"] == "image").then_some(())?;
    Some(ChatItem::Image {
        data: block["source"]["data"].as_str()?.into(),
        media: block["source"]["media_type"].as_str().unwrap_or("image/png").into(),
        user,
        at,
    })
}

/// "2026-10-28T19:52:03.123Z" (the lines' timestamps, always UTC) as ms since 1970.
pub(crate) fn iso_ms(text: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| text.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    // days since 1970 of a civil date (H. Hinnant's days_from_civil)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(((days * 24 + n(11..13)?) * 60 + n(14..16)?) * 60_000 + n(17..19)? * 1000)
}

/// A tool's input: the command, file, pattern or address it was given.
fn input_text(input: &Value) -> &str {
    let field = ["command", "file_path", "pattern", "url", "description", "prompt"].iter().find_map(|k| input[*k].as_str());
    let first = || input.as_object().and_then(|o| o.values().find_map(Value::as_str));
    field.or_else(first).unwrap_or_default()
}

/// A tool's input in one line.
fn summary(input: &Value) -> String {
    one_line(input_text(input), 90)
}

/// A tool answer's text: a string, or its first text block.
fn block_text(content: &Value) -> &str {
    content.as_str().or_else(|| content.as_array()?.iter().find_map(|b| b["text"].as_str())).unwrap_or_default()
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
