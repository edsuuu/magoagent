//! What the agents did, as structured events: what the app reads of their states (agents.rs) and
//! what Claude Code's hooks tell through `magoagent hook`. Kept in eventos.db beside the drawing,
//! written off the interface's thread; the demands (app/demands.rs) are made of them.

use crate::prelude::*;
use rusqlite::Connection;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// How long the database keeps an event.
const KEEP_DAYS: i64 = 30;
/// How long `magoagent hook` waits for the app's answer to a permission request: under the hook's
/// own timeout in the settings (`PERMISSION_TIMEOUT`), so the CLI never kills it first.
pub(crate) const PERMISSION_WAIT: u64 = 50;
const PERMISSION_TIMEOUT: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventKind {
    ToolStarted,
    ToolEnded,
    Permission,
    Question,
    Working,
    Finished,
    Error,
    Started,
    Ended,
    Message,
}

pub(crate) const KINDS: [(EventKind, &str); 10] = [
    (EventKind::ToolStarted, "tool_started"),
    (EventKind::ToolEnded, "tool_ended"),
    (EventKind::Permission, "permission"),
    (EventKind::Question, "question"),
    (EventKind::Working, "working"),
    (EventKind::Finished, "finished"),
    (EventKind::Error, "error"),
    (EventKind::Started, "started"),
    (EventKind::Ended, "ended"),
    (EventKind::Message, "message"),
];

impl EventKind {
    fn code(self) -> &'static str {
        KINDS.iter().find(|(k, _)| *k == self).map_or("message", |(_, c)| c)
    }

    fn from_code(code: &str) -> EventKind {
        KINDS.iter().find(|(_, c)| *c == code).map_or(EventKind::Message, |(k, _)| *k)
    }
}

/// Something an agent's state, the bridge or a hook proves happened.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AgentEvent {
    /// When, ms since 1970.
    pub(crate) at: i64,
    /// The terminal's or the chat's id.
    pub(crate) id: String,
    /// Its name then: the event outlives the terminal.
    pub(crate) name: String,
    pub(crate) kind: EventKind,
    pub(crate) tool: String,
    /// The short summary the panels show.
    pub(crate) what: String,
    pub(crate) cwd: String,
    pub(crate) session: String,
    /// What the attention points are read from: the tool's input, a notification's message.
    pub(crate) data: Value,
    /// A message between agents (bridge.rs): who got it; `id` sent it. Empty for the rest.
    pub(crate) to: String,
}

impl AgentEvent {
    pub(crate) fn new(id: &str, name: &str, kind: EventKind, what: impl Into<String>) -> AgentEvent {
        AgentEvent {
            at: now_ms(),
            id: id.into(),
            name: name.into(),
            kind,
            tool: String::new(),
            what: what.into(),
            cwd: String::new(),
            session: String::new(),
            data: Value::Null,
            to: String::new(),
        }
    }

    /// Seconds since it happened.
    pub(crate) fn age(&self) -> f64 {
        (now_ms() - self.at).max(0) as f64 / 1000.0
    }
}

/// The database's writer: events go to its thread, which writes them one by one; dropped, it
/// writes what is left and ends.
pub(crate) struct EventStore {
    tx: Option<mpsc::Sender<AgentEvent>>,
    writer: Option<std::thread::JoinHandle<()>>,
}

impl EventStore {
    pub(crate) fn open(path: PathBuf) -> EventStore {
        let (tx, rx) = mpsc::channel::<AgentEvent>();
        let writer = std::thread::spawn(move || {
            let Ok(db) = connect(&path) else { return };
            for event in rx {
                let _ = insert(&db, &event);
            }
        });
        EventStore { tx: Some(tx), writer: Some(writer) }
    }

    pub(crate) fn save(&self, event: &AgentEvent) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(event.clone());
        }
    }
}

impl Drop for EventStore {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

/// Opens (or makes) the database and drops what is older than `KEEP_DAYS`.
fn connect(path: &Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(2))?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS events (at INTEGER NOT NULL, agent TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, \
         tool TEXT NOT NULL, what TEXT NOT NULL, cwd TEXT NOT NULL, session TEXT NOT NULL, data TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS events_by_agent ON events (agent, at);",
    )?;
    // a database from before messages had a receiver: the column comes in empty (fails once it's there)
    let _ = db.execute("ALTER TABLE events ADD COLUMN target TEXT NOT NULL DEFAULT ''", []);
    db.execute("CREATE INDEX IF NOT EXISTS events_by_target ON events (target, at)", [])?;
    db.execute("DELETE FROM events WHERE at < ?1", [now_ms() - KEEP_DAYS * 86_400_000])?;
    Ok(db)
}

fn insert(db: &Connection, e: &AgentEvent) -> rusqlite::Result<usize> {
    db.execute(
        "INSERT INTO events (at, agent, name, kind, tool, what, cwd, session, data, target) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![e.at, e.id, e.name, e.kind.code(), e.tool, e.what, e.cwd, e.session, e.data.to_string(), e.to],
    )
}

/// The last `n` events `filter` (an SQL condition on ?1 and ?2) picks, oldest first.
fn read_events(path: &Path, filter: &str, a: Option<&str>, b: &str, n: usize, since: Option<i64>) -> Vec<AgentEvent> {
    let read = || -> rusqlite::Result<Vec<AgentEvent>> {
        let db = connect(path)?;
        let mut query = db.prepare(&format!(
            "SELECT at, agent, name, kind, tool, what, cwd, session, data, target FROM events WHERE {filter} ORDER BY at DESC, rowid DESC LIMIT ?4"
        ))?;
        let rows = query.query_map(rusqlite::params![a, b, since, n as i64], |r| {
            Ok(AgentEvent {
                at: r.get(0)?,
                id: r.get(1)?,
                name: r.get(2)?,
                kind: EventKind::from_code(&r.get::<_, String>(3)?),
                tool: r.get(4)?,
                what: r.get(5)?,
                cwd: r.get(6)?,
                session: r.get(7)?,
                data: serde_json::from_str(&r.get::<_, String>(8)?).unwrap_or(Value::Null),
                to: r.get(9)?,
            })
        })?;
        rows.collect()
    };
    let mut events = read().unwrap_or_default();
    events.reverse();
    events
}

/// The last `n` events, of one agent (what it did, sent or got) or of all, oldest first.
pub(crate) fn recent_events(path: &Path, n: usize, agent: Option<&str>) -> Vec<AgentEvent> {
    recent_events_since(path, n, agent, None)
}

/// The last `n` events since `since` (Unix ms), optionally involving one agent.
pub(crate) fn recent_events_since(path: &Path, n: usize, agent: Option<&str>, since: Option<i64>) -> Vec<AgentEvent> {
    read_events(path, "(?1 IS NULL OR agent = ?1 OR target = ?1) AND (?3 IS NULL OR at >= ?3)", agent, "", n, since)
}

/// The last `n` messages between agents `a` and `b`, either way, oldest first.
pub(crate) fn conversation(path: &Path, a: &str, b: &str, n: usize) -> Vec<AgentEvent> {
    read_events(path, "(agent = ?1 AND target = ?2) OR (agent = ?2 AND target = ?1)", Some(a), b, n, None)
}

/// The hook's event name, under the names the CLIs may give it.
fn hook_name(payload: &Value) -> &str {
    ["hook_event_name", "hookEventName", "event"].iter().find_map(|k| payload[*k].as_str()).unwrap_or_default()
}

/// "Bash: cargo test", "Edit: src/main.rs": the tool and the first line of what it was given.
pub(crate) fn tool_summary(tool: &str, input: &Value) -> String {
    let detail = ["command", "file_path", "path", "notebook_path", "url", "pattern", "query", "description", "prompt"]
        .iter()
        .find_map(|k| input[*k].as_str().map(str::trim).filter(|s| !s.is_empty()))
        .and_then(|s| s.lines().next())
        .unwrap_or_default();
    let line = if detail.is_empty() { tool.to_string() } else { format!("{tool}: {detail}") };
    match line.char_indices().nth(90) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line,
    }
}

/// One hook call as an event: its kind, tool and summary; None for what isn't followed. Missing
/// fields read as empty.
// ponytail: the payload's names (hook_event_name, tool_name, tool_input, tool_response, message,
// notification_type, session_id, cwd) are Claude Code's as documented when this was written, not
// checked against the CLI installed here; `claude --debug` prints each hook's input to compare.
// Codex's hooks, once wired, may need their own names here
pub(crate) fn hook_event(payload: &Value) -> Option<(EventKind, String, String)> {
    let tool = payload["tool_name"].as_str().unwrap_or_default().to_string();
    let summary = tool_summary(&tool, &payload["tool_input"]);
    let message = payload["message"].as_str().unwrap_or_default().trim().to_string();
    let response = &payload["tool_response"];
    let failed = response["is_error"] == true
        || response["error"].as_str().is_some_and(|e| !e.trim().is_empty())
        || ["exit_code", "exitCode"].iter().any(|k| response[*k].as_i64().is_some_and(|c| c != 0));
    Some(match hook_name(payload) {
        "PreToolUse" => (EventKind::ToolStarted, tool, summary),
        "PostToolUse" if failed => (EventKind::Error, tool, format!("falhou: {summary}")),
        "PostToolUse" => (EventKind::ToolEnded, tool, format!("terminou {summary}")),
        "PostToolUseFailure" => (EventKind::Error, tool, format!("falhou: {summary}")),
        "PermissionRequest" => (EventKind::Permission, tool, format!("pede permissão: {summary}")),
        "Notification" => {
            let permission = payload["notification_type"].as_str().is_some_and(|t| t.contains("permission")) || message.to_lowercase().contains("permission");
            let what = if message.is_empty() { "precisa de você".to_string() } else { format!("precisa de você: {message}") };
            (if permission { EventKind::Permission } else { EventKind::Question }, tool, what)
        }
        "Stop" => (EventKind::Finished, tool, "terminou a vez".into()),
        "SessionStart" => (EventKind::Started, tool, "sessão começou".into()),
        "SessionEnd" => (EventKind::Ended, tool, "sessão acabou".into()),
        _ => return None,
    })
}

/// What `magoagent hook` prints to answer a permission request (allow or deny).
pub(crate) fn permission_answer(allow: bool, reason: &str) -> String {
    let decision = if allow { json!({ "behavior": "allow" }) } else { json!({ "behavior": "deny", "message": reason }) };
    json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision } }).to_string()
}

/// The settings the app hands the Claude Code it starts (`--settings`): `magoagent hook` on each
/// event. Never the person's own settings file.
// ponytail: the event names, `matcher` and `timeout` follow Claude Code's documented format and the
// quoted path suits bash and cmd, not PowerShell; check both with `claude --settings <file> --debug`
pub(crate) fn hook_settings_json(exe: &str) -> Value {
    let hook = |timeout: u64, matcher: bool| {
        let run = json!({ "type": "command", "command": format!("\"{exe}\" hook"), "timeout": timeout });
        if matcher { json!([{ "matcher": "*", "hooks": [run] }]) } else { json!([{ "hooks": [run] }]) }
    };
    // the status line tells the app the plan's limits and what the session spends (app/usage.rs)
    json!({ "statusLine": { "type": "command", "command": format!("\"{exe}\" statusline") }, "hooks": {
        "PreToolUse": hook(10, true),
        "PostToolUse": hook(10, true),
        "PermissionRequest": hook(PERMISSION_TIMEOUT, true),
        "Notification": hook(10, false),
        "Stop": hook(10, false),
        "SessionStart": hook(10, false),
        "SessionEnd": hook(10, false),
    } })
}

/// A terminal's command with the app's hooks: `claude …` gets `--settings <file>`, anything else
/// stays as it is.
pub(crate) fn with_hooks(command: String, settings: Option<&Path>) -> String {
    let Some(settings) = settings else { return command };
    match command.trim_start().strip_prefix("claude") {
        Some(rest) if rest.is_empty() || rest.starts_with(' ') => format!("claude --settings \"{}\"{rest}", settings.display()),
        _ => command,
    }
}

/// `magoagent hook`: the CLI's event (JSON on stdin) goes to the app by the bridge, as the
/// terminal's (or chat's) agent. A permission request waits up to `PERMISSION_WAIT` for the app's
/// answer and prints it. It never stands in the CLI's way: no app, no answer or an input it can't
/// read, it ends at once, saying nothing, and the CLI goes on as always.
pub(crate) fn hook_cli() -> i32 {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let Ok(payload) = serde_json::from_str::<Value>(&input) else { return 0 };
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    let (Some(address), Some(token)) = (env("MAGOAGENT_BRIDGE"), env("MAGOAGENT_TOKEN")) else { return 0 };
    let Some(from) = env("MAGOAGENT_TERMINAL_ID").or_else(|| env("MAGOAGENT_CHAT_ID")) else { return 0 };
    let wait = if hook_name(&payload) == "PermissionRequest" { PERMISSION_WAIT } else { 0 };
    let request = json!({ "cmd": "hook", "payload": payload, "wait": wait });
    if let Ok(answer) = request_to(&address, &token, &from, request, Duration::from_secs(wait))
        && !answer.is_empty()
    {
        println!("{answer}");
    }
    0
}

/// The app's side: the hook database and the permission requests whose `magoagent hook` still
/// waits for an answer (the event, and where the answer goes).
#[derive(Default)]
pub(crate) struct Hooks {
    pub(crate) store: Option<EventStore>,
    pub(crate) pending: Vec<(AgentEvent, mpsc::Sender<Result<String, String>>)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_kept_in_sqlite_and_old_ones_go() {
        let dir = std::env::temp_dir().join(format!("mago-events-{}", new_id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("eventos.db");
        let mut old = AgentEvent::new("t1", "Revisor", EventKind::Finished, "terminou");
        old.at -= 31 * 86_400_000;
        let mut asked = AgentEvent::new("t1", "Revisor", EventKind::Permission, "pede permissão: Bash: ls");
        asked.data = json!({ "command": "ls" });
        let other = AgentEvent::new("t2", "Front", EventKind::Started, "iniciado");
        let store = EventStore::open(path.clone());
        for e in [&old, &asked, &other] {
            store.save(e);
        }
        drop(store);
        // the next opening drops the event older than 30 days
        assert_eq!(recent_events(&path, 10, None), [asked.clone(), other.clone()]);
        assert_eq!(recent_events(&path, 10, Some("t1")), [asked]);
        assert_eq!(recent_events(&path, 1, None), [other]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn messages_keep_who_sent_and_who_got_them() {
        let dir = std::env::temp_dir().join(format!("mago-conversa-{}", new_id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("eventos.db");
        // a database from before the receiver's column
        let old = Connection::open(&path).unwrap();
        old.execute_batch(
            "CREATE TABLE events (at INTEGER NOT NULL, agent TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, tool TEXT NOT NULL, \
             what TEXT NOT NULL, cwd TEXT NOT NULL, session TEXT NOT NULL, data TEXT NOT NULL);",
        )
        .unwrap();
        old.execute("INSERT INTO events VALUES (?1, 'dev', 'Dev', 'started', '', 'iniciado', '', '', 'null')", [now_ms() - 5000]).unwrap();
        drop(old);
        let said = |from: &str, name: &str, to: &str, what: &str, at: i64| {
            let mut e = AgentEvent::new(from, name, EventKind::Message, what);
            (e.to, e.at) = (to.into(), now_ms() - at);
            e
        };
        let asked = said("rev", "Revisor", "dev", "pergunta: e os testes?", 3000);
        let answered = said("dev", "Dev", "rev", "respondeu", 2000);
        let elsewhere = said("rev", "Revisor", "front", "pergunta: e a tela?", 1000);
        let store = EventStore::open(path.clone());
        for e in [&asked, &answered, &elsewhere] {
            store.save(e);
        }
        drop(store);
        assert_eq!(conversation(&path, "dev", "rev", 10), [asked, answered.clone()], "either way, oldest first, only that pair");
        assert_eq!(conversation(&path, "rev", "dev", 1), [answered], "the last n");
        assert!(conversation(&path, "dev", "front", 10).is_empty());
        // the old row reads with no receiver; an agent's own history has what it got too
        let dev = recent_events(&path, 10, Some("dev"));
        assert_eq!(dev.len(), 3);
        assert_eq!((dev[0].what.as_str(), dev[0].to.as_str()), ("iniciado", ""));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hooks_read_tolerantly_and_answer_permissions() {
        let pre = json!({ "hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": { "command": "cargo test\nmais" } });
        assert_eq!(hook_event(&pre), Some((EventKind::ToolStarted, "Bash".into(), "Bash: cargo test".into())));
        let failed = json!({ "hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {}, "tool_response": { "exit_code": 2 } });
        assert_eq!(hook_event(&failed).map(|e| e.0), Some(EventKind::Error));
        let asks = json!({ "hook_event_name": "Notification", "message": "Claude needs your permission to use Bash" });
        assert_eq!(hook_event(&asks).map(|e| e.0), Some(EventKind::Permission));
        assert_eq!(hook_event(&json!({ "hook_event_name": "Stop" })).map(|e| e.0), Some(EventKind::Finished));
        // fields missing or unknown: no panic, nothing followed
        assert_eq!(hook_event(&json!({})), None);
        assert_eq!(hook_event(&json!({ "hook_event_name": "PermissionRequest" })).map(|e| e.2), Some("pede permissão: ".into()));
        let allow: Value = serde_json::from_str(&permission_answer(true, "")).unwrap();
        assert_eq!(allow["hookSpecificOutput"]["decision"]["behavior"], "allow");
        let deny: Value = serde_json::from_str(&permission_answer(false, "não")).unwrap();
        assert_eq!(deny["hookSpecificOutput"]["decision"], json!({ "behavior": "deny", "message": "não" }));
        let settings = hook_settings_json("C:/MagoAgent/magoagent.exe");
        assert_eq!(settings["hooks"]["PermissionRequest"][0]["hooks"][0]["command"], "\"C:/MagoAgent/magoagent.exe\" hook");
        assert!(settings["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"].as_u64() > Some(PERMISSION_WAIT));
        let file = Path::new("/dados/hooks-claude.json");
        assert_eq!(with_hooks("claude --resume 1".into(), Some(file)), format!("claude --settings \"{}\" --resume 1", file.display()));
        assert_eq!(with_hooks("claude".into(), Some(file)), format!("claude --settings \"{}\"", file.display()));
        assert_eq!(with_hooks("claudette".into(), Some(file)), "claudette");
        assert_eq!(with_hooks("codex".into(), Some(file)), "codex");
        assert_eq!(with_hooks("claude".into(), None), "claude");
    }
}
