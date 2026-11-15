//! Claude Code sessions outside the app's terminals (Claude Desktop's Code tab, VS Code, any terminal): the person's
//! own `~/.claude/settings.json` runs `magoagent hook external` (claude_hooks.rs), which finds the app by the fixed
//! `bridge.json` and relays the event. Here is the app's side: a pill per origin (name, colour, the badge), and the
//! permissions and AskUserQuestions as `PersonAsk`s the island shows, in the order they came (at most `MAX_ASKS`).
//! The hook waits for the answer up to `WAIT`; with none, or "Responder no terminal", it prints nothing and the
//! CLI asks in its own terminal as if the app were not there.

use crate::app::chat::decide::PersonAsk;
use crate::app::chat::direct::folder_label;
use crate::app::ui::agents::AgentState;
use crate::mcp::bridge::hook_endpoint_path;
use crate::prelude::*;
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::Sender;
use std::time::Duration;

/// The id of an ask of a session outside, and of its pill on the island.
pub(crate) const ASK_PREFIX: &str = "ext:";
pub(crate) const PILL_PREFIX: &str = "pill:";
/// How long the hook waits for the answer: under the settings' 120 s, over nobody's patience.
pub(crate) const WAIT: u64 = 108;
/// The hook's timeout in the settings for the events that wait.
pub(crate) const WAIT_TIMEOUT: u64 = 120;
/// The asks waiting at once (the one shown and the queue); the next goes back to the terminal.
pub(crate) const MAX_ASKS: usize = 5;
/// How long the badge of a turn that ended stays, and the pill with it.
const BADGE_FOR: i64 = 5200;
/// A pill whose session said nothing for this long is gone (it died without a SessionEnd).
const SILENT_FOR: i64 = 15 * 60 * 1000;
/// A question answered "no, in the terminal" asks again through the other hook of the CLI: ignore it for this long.
const DECLINED_FOR: i64 = 3000;
/// The app must answer the bridge within this: no app, no hook.
const CONNECT: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Badge {
    Approval,
    Finished,
    Error,
}

/// Where an ask came from.
pub(crate) struct Source {
    pub(crate) pill: String,
    /// "Claude Desktop", "Claude Code · folder": what the card says.
    pub(crate) label: String,
    pub(crate) session: String,
    /// The hook it waits in (`PermissionRequest` or `PreToolUse`): the answer's shape.
    pub(crate) event: String,
    pub(crate) at: i64,
}

pub(crate) struct Pill {
    key: String,
    name: String,
    color: Color32,
    working: bool,
    asking: bool,
    /// O limite de uso do plano bateu (uma Notification de "rate limit"): fica até a sessão voltar a trabalhar.
    limited: bool,
    /// A última coisa que a sessão fez, numa linha: a ferramenta, o prompt, "+ subagente".
    activity: String,
    badge: Option<Badge>,
    /// When the badge (and, with `gone`, the pill) end, ms; 0: no.
    until: i64,
    gone: i64,
    seen: i64,
}

#[derive(Default)]
pub(crate) struct Outside {
    pills: Vec<Pill>,
    /// The sessions whose question went back to the terminal: (session, when).
    pub(crate) declined: std::cell::RefCell<Vec<(String, i64)>>,
}

#[cfg(test)]
impl Outside {
    /// Time passed: every badge and every pill of a turn that ended is due.
    pub(crate) fn expire_badges(&mut self) {
        for pill in &mut self.pills {
            (pill.until, pill.gone) = (pill.until.min(1), pill.gone.min(1));
        }
    }
}

/// A pill as the island draws it.
pub(crate) struct PillView {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) color: Color32,
    pub(crate) state: AgentState,
    pub(crate) badge: Option<Badge>,
    /// A última atividade da sessão, numa linha ("Bash: cargo test", "+ subagente"); vazia enquanto não houve nenhuma.
    pub(crate) activity: String,
    /// O limite de uso do plano bateu nessa sessão (`state` fica ocioso).
    pub(crate) limited: bool,
}

/// O texto numa linha só, cortado em 90 caracteres.
fn one_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or_default();
    match line.char_indices().nth(90) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_string(),
    }
}

/// A linha de atividade que um evento deixa na pílula; None mantém a que estava.
fn activity(event: &str, payload: &Value) -> Option<String> {
    let text = |key: &str| payload[key].as_str().unwrap_or_default();
    let summary = || tool_summary(text("tool_name"), &payload["tool_input"]);
    let line = match event {
        // o que o Claude Code injeta sozinho (lembretes, avisos de tarefa) vem em tags: não é o que a pessoa pediu
        "UserPromptSubmit" => Some(one_line(text("prompt"))).filter(|line| !line.starts_with('<')).unwrap_or_default(),
        "PreToolUse" if text("tool_name") == "AskUserQuestion" => one_line(payload["tool_input"]["questions"][0]["question"].as_str().unwrap_or_default()),
        "PreToolUse" | "PermissionRequest" => summary(),
        "PostToolUseFailure" => format!("falhou: {}", summary()),
        "SubagentStart" => "+ subagente".into(),
        // os ajudantes do próprio Claude Code (a sugestão de prompt depois de cada vez) terminam com agent_type vazio sem nunca começar
        "SubagentStop" if payload["agent_type"] != "" => "• subagente terminou".into(),
        "Notification" if text("message").trim_end().ends_with('?') => one_line(text("message")),
        "Stop" => Some(one_line(text("last_assistant_message"))).filter(|line| !line.is_empty()).unwrap_or_else(|| "terminou a vez".into()),
        "StopFailure" => "falhou".into(),
        _ => return None,
    };
    (!line.is_empty()).then_some(line)
}

/// Uma Notification que avisa do limite de uso do plano.
fn rate_limited(message: &str) -> bool {
    let message = message.to_lowercase();
    ["rate limit", "usage limit", "limite de"].iter().any(|mark| message.contains(mark))
}

/// The pill of a session: (key, name, colour). Claude Desktop is told by the hook (`CLAUDE_CODE_ENTRYPOINT`).
fn origin(payload: &Value) -> (String, String, Color32) {
    let hex = |value: u32| Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8);
    match payload["mago_origin"].as_str().unwrap_or_default() {
        "claude-desktop" => ("claude-desktop".into(), "Claude Desktop".into(), hex(0xd97757)),
        "cursor" => ("cursor".into(), "Cursor".into(), hex(0xc0c4cc)),
        _ => {
            let cwd = payload["cwd"].as_str().unwrap_or_default();
            let name = if cwd.is_empty() { "Claude Code".to_string() } else { format!("Claude Code · {}", folder_label(cwd)) };
            (format!("code:{cwd}"), name, hex(0xf5f6f8))
        }
    }
}

/// Cuts every string of `value` to `MAX_FIELD` bytes: the hook sends one line, not a file.
fn shorten(value: &mut Value) {
    const MAX_FIELD: usize = 2000;
    match value {
        Value::String(text) if text.len() > MAX_FIELD => {
            let mut cut = MAX_FIELD;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
        }
        Value::Array(items) => items.iter_mut().for_each(shorten),
        Value::Object(fields) => fields.values_mut().for_each(shorten),
        _ => {}
    }
}

/// What the hook sends of the CLI's event: the origin added, what weighs left out; an AskUserQuestion's input whole,
/// as it is what goes back with the answers.
pub(crate) fn tag(payload: &mut Value, env: &dyn Fn(&str) -> Option<String>) {
    let origin = if env("CLAUDE_CODE_ENTRYPOINT").as_deref() == Some("claude-desktop") {
        "claude-desktop"
    } else if env("CURSOR_TRACE_ID").is_some() {
        "cursor"
    } else {
        ""
    };
    let input = (payload["tool_name"] == "AskUserQuestion").then(|| payload["tool_input"].clone());
    if let Some(fields) = payload.as_object_mut() {
        for weighty in ["tool_response", "tool_output", "transcript_path"] {
            fields.remove(weighty);
        }
        fields.insert("mago_origin".into(), origin.into());
        if fields.get("cwd").and_then(Value::as_str).is_none_or(str::is_empty)
            && let Ok(here) = std::env::current_dir()
        {
            fields.insert("cwd".into(), here.to_string_lossy().into_owned().into());
        }
    }
    shorten(payload);
    if let Some(input) = input {
        payload["tool_input"] = input;
    }
}

/// `magoagent hook external` with the app at `endpoint` (address, token): what to print, empty for nothing. It waits
/// only where the person can answer: a permission request, and the AskUserQuestion the `wait` hook is for.
pub(crate) fn send(endpoint: (String, String), mut payload: Value, may_wait: bool, env: &dyn Fn(&str) -> Option<String>) -> String {
    let (address, token) = endpoint;
    // no app listening: end at once, the CLI goes on as always
    let Ok(socket) = address.parse::<SocketAddr>() else { return String::new() };
    if TcpStream::connect_timeout(&socket, CONNECT).is_err() {
        return String::new();
    }
    tag(&mut payload, env);
    let question = payload["tool_name"] == "AskUserQuestion";
    let wait = match hook_name(&payload) {
        "PermissionRequest" if !question => WAIT,
        "PreToolUse" if may_wait && question => WAIT,
        // a pergunta é do hook que espera (Claude Code 2.1.85+): o PermissionRequest dela e o PreToolUse geral voltam na hora, sem cartão
        "PermissionRequest" | "PreToolUse" if question => return String::new(),
        _ => 0,
    };
    let from = format!("{ASK_PREFIX}{}", payload["session_id"].as_str().unwrap_or_default());
    let request = json!({ "cmd": "hook", "payload": payload, "wait": wait });
    request_to(&address, &token, &from, request, Duration::from_secs(wait)).unwrap_or_default()
}

/// The app's address and token, from the fixed `bridge.json` it keeps for the hooks.
pub(crate) fn endpoint_from(path: &std::path::Path) -> Option<(String, String)> {
    let saved: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    Some((saved["address"].as_str()?.to_string(), saved["token"].as_str()?.to_string()))
}

pub(crate) fn relay(payload: Value, may_wait: bool, env: &dyn Fn(&str) -> Option<String>) -> String {
    match hook_endpoint_path().and_then(|path| endpoint_from(&path)) {
        Some(endpoint) => send(endpoint, payload, may_wait, env),
        None => String::new(),
    }
}

impl DrawingApp {
    /// What `magoagent hook external` relayed (`from` is "ext:<session>"): the pill of its origin, what it settles of
    /// the asks waiting, and, when the hook waits (`reply`), a new ask.
    pub(crate) fn take_outside_hook(&mut self, payload: &Value, reply: Option<Sender<Result<String, String>>>) {
        let now = now_ms();
        let text = |key: &str| payload[key].as_str().unwrap_or_default().to_string();
        let (event, tool, session) = (hook_name(payload).to_string(), text("tool_name"), text("session_id"));
        let (key, name, color) = origin(payload);
        // a pergunta só existe pelo hook que espera: o PermissionRequest dela e o PreToolUse geral não fazem cartão nem mexem na pílula
        if tool == "AskUserQuestion" && (event == "PermissionRequest" || (event == "PreToolUse" && reply.is_none())) {
            return;
        }
        // asked here and answered in the terminal, or the turn went on: the card goes
        self.person_asks.get_mut().retain(|ask| match &ask.source {
            Some(source) if source.session == session => {
                !(matches!(event.as_str(), "Stop" | "StopFailure" | "UserPromptSubmit" | "SessionEnd")
                    || (event.starts_with("PostToolUse") && ask.tool == tool))
            }
            _ => true,
        });
        if event == "SessionEnd" {
            // ponytail: a pill per origin, not per session: one session ending takes the pill of its sibling too until its next event
            self.outside.pills.retain(|pill| pill.key != key);
        } else if !event.is_empty() {
            let index = self.outside.pills.iter().position(|pill| pill.key == key).unwrap_or_else(|| {
                let pill = Pill {
                    key: key.clone(),
                    name: name.clone(),
                    color,
                    working: false,
                    asking: false,
                    limited: false,
                    activity: String::new(),
                    badge: None,
                    until: 0,
                    gone: 0,
                    seen: now,
                };
                self.outside.pills.push(pill);
                self.outside.pills.len() - 1
            });
            let pill = &mut self.outside.pills[index];
            (pill.seen, pill.name) = (now, name.clone());
            if let Some(line) = activity(&event, payload) {
                pill.activity = line;
            }
            match event.as_str() {
                "Stop" | "StopFailure" => {
                    (pill.working, pill.asking, pill.limited) = (false, false, false);
                    pill.badge = Some(if event == "Stop" { Badge::Finished } else { Badge::Error });
                    pill.until = now + BADGE_FOR;
                    pill.gone = pill.until;
                }
                // um subagente só deixa a sua linha de atividade: o do fim da vez chega depois do Stop e não pode reabrir a pílula
                "SessionStart" | "SubagentStart" | "SubagentStop" => {}
                // o limite de uso do plano tem estado próprio; fora ele, só é pergunta quando uma foi feita
                "Notification" if rate_limited(&text("message")) => (pill.limited, pill.working, pill.asking) = (true, false, false),
                "Notification" => pill.asking |= text("message").trim_end().ends_with('?'),
                _ => {
                    (pill.working, pill.asking, pill.limited, pill.gone) = (true, false, false, 0);
                    if matches!(pill.badge, Some(Badge::Finished | Badge::Error)) {
                        (pill.badge, pill.until) = (None, 0);
                    }
                }
            }
        }
        let waits = event == "PermissionRequest" || (event == "PreToolUse" && tool == "AskUserQuestion");
        if let Some(reply) = reply.filter(|_| waits) {
            self.outside_ask(payload, (key, name), session, event, reply);
        }
    }

    /// A permission or an AskUserQuestion of a session outside, as a card for the island; back to the terminal when
    /// the island is off, the queue is full or the app can't answer it right (a question with no options, a plan).
    fn outside_ask(&mut self, payload: &Value, (pill, label): (String, String), session: String, event: String, reply: Sender<Result<String, String>>) {
        let tool = payload["tool_name"].as_str().unwrap_or_default();
        let request = json!({ "tool_name": tool, "input": payload["tool_input"] });
        let mut ask = PersonAsk::new(format!("{ASK_PREFIX}{}", new_id()), &request, reply);
        let unanswerable = (tool == "AskUserQuestion" && ask.questions.is_empty()) || tool == "ExitPlanMode";
        let declined = tool == "AskUserQuestion" && self.outside.declined.borrow().iter().any(|(who, _)| *who == session);
        if !self.prefs.island || self.person_asks.borrow().len() >= MAX_ASKS || unanswerable || declined {
            return ask.send_terminal();
        }
        self.toast(format!("{label} {}", if ask.questions.is_empty() { "precisa de permissão" } else { "está perguntando" }));
        ask.source = Some(Source { pill, label, session, event, at: now_ms() });
        self.person_asks.get_mut().push(ask);
    }

    /// With the agents' check: the asks the hook gave up on go, the badges end and the pills of turns long over leave.
    pub(crate) fn outside_tick(&mut self) {
        let now = now_ms();
        self.person_asks.get_mut().retain(|ask| ask.source.as_ref().is_none_or(|source| now - source.at < WAIT as i64 * 1000));
        self.outside.declined.get_mut().retain(|(_, at)| now - at < DECLINED_FOR);
        let waiting: Vec<String> = self.person_asks.get_mut().iter().filter_map(|ask| ask.source.as_ref().map(|source| source.pill.clone())).collect();
        self.outside.pills.retain_mut(|pill| {
            if pill.until != 0 && now >= pill.until {
                (pill.badge, pill.until) = (None, 0);
            }
            let over = pill.gone != 0 && now >= pill.gone && !waiting.contains(&pill.key);
            !over && now - pill.seen < SILENT_FOR
        });
    }

    /// The pills of the sessions outside, for the island: waiting when an ask of theirs waits.
    pub(crate) fn outside_pills(&self) -> Vec<PillView> {
        let waiting: Vec<String> = self.person_asks.borrow().iter().filter_map(|ask| ask.source.as_ref().map(|source| source.pill.clone())).collect();
        self.outside
            .pills
            .iter()
            .map(|pill| {
                let asked = waiting.contains(&pill.key);
                let state = match (asked || pill.asking, pill.working) {
                    (true, _) => AgentState::Waiting,
                    (_, true) => AgentState::Working(0.0),
                    _ => AgentState::Idle(None),
                };
                PillView {
                    id: format!("{PILL_PREFIX}{}", pill.key),
                    name: pill.name.clone(),
                    color: pill.color,
                    state,
                    badge: if asked { Some(Badge::Approval) } else { pill.badge },
                    activity: pill.activity.clone(),
                    limited: pill.limited,
                }
            })
            .collect()
    }

    /// As sessões de fora do app para o `demands` do MCP (o Maestro lê em voz alta): uma linha por pílula, com o estado
    /// e a última atividade; vazio quando não há nenhuma.
    pub(crate) fn outside_report(&self) -> String {
        let mut report = String::new();
        for pill in self.outside_pills() {
            let state = if pill.limited { "limite de uso do plano" } else { pill.state.label() };
            let activity = if pill.activity.is_empty() { String::new() } else { format!(" — {}", pill.activity) };
            report += &format!("\n- {} — {state}{activity}", pill.name);
        }
        if report.is_empty() { report } else { format!("\n\nSessões fora do app:{report}") }
    }

    /// The pointer is on pill `id` of the island: what ended there is seen, its badge goes. False for any other id.
    pub(crate) fn outside_focus(&mut self, id: &str) -> bool {
        let Some(key) = id.strip_prefix(PILL_PREFIX) else { return false };
        for pill in self.outside.pills.iter_mut().filter(|pill| pill.key == key && matches!(pill.badge, Some(Badge::Finished | Badge::Error))) {
            (pill.badge, pill.until) = (None, 0);
        }
        true
    }
}
