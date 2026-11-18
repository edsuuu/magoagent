//! Claude Code sessions outside the app's terminals (Claude Desktop's Code tab, VS Code, any terminal): the person's
//! own `~/.claude/settings.json` runs `magoagent hook external` (claude_hooks.rs), which finds the app by the fixed
//! `bridge.json` and relays the event (relay.rs). Here is the app's side: a pill per origin and folder (name, colour, the badge), and the
//! permissions and AskUserQuestions as `PersonAsk`s the island shows, in the order they came (at most `MAX_ASKS`).
//! The hook waits for the answer up to `WAIT`; with none, or "Responder no terminal", it prints nothing and the
//! CLI asks in its own terminal as if the app were not there.

use super::message::Reach;
use crate::app::chat::decide::PersonAsk;
use crate::app::chat::direct::folder_label;
use crate::app::mascot::identity::{make_distinct, short_name};
use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use std::sync::mpsc::Sender;

/// The id of an ask of a session outside, and of its pill on the island.
pub(crate) const ASK_PREFIX: &str = "ext:";
pub(crate) const PILL_PREFIX: &str = "pill:";
/// How long the hook waits for the answer: under the settings' 120 s, over nobody's patience.
pub(crate) const WAIT: u64 = 108;
/// The hook's timeout in the settings for the events that wait.
pub(crate) const WAIT_TIMEOUT: u64 = 120;
/// The asks waiting at once (the one shown and the queue); the next goes back to the terminal.
pub(crate) const MAX_ASKS: usize = 5;
/// How long the badge of a turn that ended (its ✓, or the error) stays when nobody looks at it: then the pill sleeps.
const BADGE_FOR: i64 = 20 * 60 * 1000;
/// A pill whose session said nothing for this long is gone (it died without a SessionEnd). Until then a session whose
/// turn ended stays, asleep (asked: "se eles nao está trabalhando ou se ja terminou ele fica com a expressao zzz ou com o
/// check ali"), instead of leaving with its badge.
const SILENT_FOR: i64 = 8 * 60 * 60 * 1000;
/// A question answered "no, in the terminal" asks again through the other hook of the CLI: ignore it for this long.
const DECLINED_FOR: i64 = 3000;
/// A pill whose conversation the island shows stays this long after the island last said so (`outside_shown`).
const SHOWN_FOR: i64 = 2000;
/// Os eventos que encerram a vez: um pedido que ainda esperava a pessoa nessa sessão perdeu o sentido.
pub(super) const TURN_OVER: [&str; 5] = ["Stop", "StopFailure", "UserPromptSubmit", "SessionEnd", "Interrupt"];

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
    /// O número que a ponte deu ao pedido (`mago_request`): é por ele que ela avisa que o relay desligou.
    pub(crate) request: String,
}

pub(crate) struct Pill {
    pub(super) key: String,
    /// Its folder's name ("loja"; the program's with no folder) and the program ("Claude Code", "Claude Desktop").
    name: String,
    kind: String,
    /// A program's own colour (Claude Desktop's, Cursor's, Hermes'); none: Claude Code's, an agent's of the palette
    /// (`agent_color`).
    brand: Option<Color32>,
    pub(super) working: bool,
    pub(super) asking: bool,
    /// O limite de uso do plano bateu (uma Notification de "rate limit"): fica até a sessão voltar a trabalhar.
    limited: bool,
    /// A última coisa que a sessão fez, numa linha: a ferramenta, o prompt, "+ subagente".
    activity: String,
    badge: Option<Badge>,
    /// When the badge (and, with `gone`, the pill) end, ms; 0: no.
    until: i64,
    gone: i64,
    seen: i64,
    /// When the island last showed its conversation (`outside_shown`): meanwhile the pill stays, to be written to.
    shown: i64,
    pub(super) session: String,
    /// The file Claude Code writes the session's conversation in (`outside_transcript`).
    transcript: String,
}

#[derive(Default)]
pub(crate) struct Outside {
    pills: Vec<Pill>,
    /// The sessions whose question went back to the terminal: (session, when).
    pub(crate) declined: std::cell::RefCell<Vec<(String, i64)>>,
    /// How to reach each session, by its id (message.rs): its tmux pane, what waits on its screen.
    pub(super) sessions: std::collections::HashMap<String, Reach>,
    /// The tmux the tests give in place of the person's (message.rs): what it printed, or why it failed.
    #[cfg(test)]
    pub(crate) fake_tmux: Option<Box<dyn Fn(&[String]) -> Result<String, String>>>,
}

impl Outside {
    /// Pill `id` of the island ("pill:<key>").
    pub(super) fn pill(&self, id: &str) -> Option<&Pill> {
        let key = id.strip_prefix(PILL_PREFIX)?;
        self.pills.iter().find(|pill| pill.key == key)
    }
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
    /// Its folder's name, no other pill's ("loja", "loja 2"): the lists show it whole, its program goes apart (`kind`).
    pub(crate) name: String,
    /// The program: "Claude Code", "Claude Desktop", "Cursor", "Hermes".
    pub(crate) kind: String,
    pub(crate) color: Color32,
    pub(crate) state: AgentState,
    pub(crate) badge: Option<Badge>,
    /// A última atividade da sessão, numa linha ("Bash: cargo test", "+ subagente"); vazia enquanto não houve nenhuma.
    pub(crate) activity: String,
    /// O limite de uso do plano bateu nessa sessão (`state` fica ocioso).
    pub(crate) limited: bool,
    #[allow(dead_code)]
    pub(crate) session: String,
    /// Parada: a vez acabou e o selo dela já foi visto (ou passou): a mini dorme.
    pub(crate) asleep: bool,
    /// O último evento da sessão (ms).
    pub(crate) at: i64,
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

/// Where a session comes from (`origin`).
struct Origin {
    key: String,
    name: String,
    kind: String,
    brand: Option<Color32>,
}

/// A session's pill, one per program and folder: its key, its name (the folder's, short: `short_name`), its program and
/// that program's own colour. Claude Desktop is told by the hook (relay.rs `origin`); only Claude Code's pills ("code:")
/// take messages from the app (message.rs).
fn origin(payload: &Value) -> Origin {
    let cwd = payload["cwd"].as_str().unwrap_or_default();
    let (prefix, program, brand) = match payload["mago_origin"].as_str().unwrap_or_default() {
        "hermes" => return Origin { key: "hermes".into(), name: "Hermes".into(), kind: "Hermes".into(), brand: Some(rgb_hex(0xc084fc)) },
        "claude-desktop" => ("desktop", "Claude Desktop", Some(rgb_hex(0xd97757))),
        "cursor" => ("cursor", "Cursor", Some(rgb_hex(0xc0c4cc))),
        _ => ("code", "Claude Code", None),
    };
    // asked ("fica sempre com o prefixo claude code"): the program no longer eats the room of the folder's name
    let name = if cwd.is_empty() { program.to_string() } else { short_name(&folder_label(cwd)) };
    Origin { key: format!("{prefix}:{cwd}"), name, kind: program.into(), brand }
}

/// "Claude Code · loja": a pill's program and name, as a card, a toast and the Maestro say it; the name alone when it is
/// the program's (Hermes).
pub(crate) fn pill_label(kind: &str, name: &str) -> String {
    if name.starts_with(kind) { name.to_string() } else { format!("{kind} · {name}") }
}

impl PillView {
    pub(crate) fn label(&self) -> String {
        pill_label(&self.kind, &self.name)
    }
}

/// The file of a session's conversation as its hook says (`transcript_path`), or where Claude Code keeps it by the
/// session's folder and id when the hook leaves it out.
fn transcript_of(payload: &Value, session: &str) -> Option<String> {
    if let Some(file) = payload["transcript_path"].as_str().filter(|file| !file.is_empty()) {
        // a session in WSL says where its file is in the distro (`/home/…`): from Windows, by `\\wsl.localhost\<distro>`
        return Some(match payload["wsl_distro"].as_str().filter(|distro| cfg!(windows) && !distro.is_empty() && file.starts_with('/')) {
            Some(distro) => crate::app::system::wsl::windows_path(distro, file),
            None => file.to_string(),
        });
    }
    let cwd = payload["cwd"].as_str().filter(|cwd| !cwd.is_empty() && !session.is_empty())?;
    let dir = crate::transcript::claude_dir()?;
    Some(crate::transcript::transcript_path(&dir, &crate::transcript::project_folder(cwd), session).display().to_string())
}

impl DrawingApp {
    /// What `magoagent hook external` relayed (`from` is "ext:<session>"): the pill of its origin, what it settles of
    /// the asks waiting, and, when the hook waits (`reply`), a new ask.
    pub(crate) fn take_outside_hook(&mut self, payload: &Value, reply: Option<Sender<Result<String, String>>>) {
        crate::app::maestro::session_window::note(payload);
        if payload["mago_origin"] == "hermes" && !(crate::app::integrations::hermes::available() && self.prefs.integrations.hermes) {
            return;
        }
        let now = now_ms();
        let text = |key: &str| payload[key].as_str().unwrap_or_default().to_string();
        let (event, tool, session) = (hook_name(payload).to_string(), text("tool_name"), text("session_id"));
        let Origin { key, name, kind, brand } = origin(payload);
        // o relay que esperava desligou (a ponte avisa): responderam em outro lugar, e o cartão sai na hora
        if event == HOOK_GONE {
            let request = text("mago_request");
            let settled = self.settle_elsewhere(|_, source| !request.is_empty() && source.request == request);
            for pill in self.outside.pills.iter_mut().filter(|pill| settled.contains(&pill.key)) {
                pill.activity = "respondido fora da ilha".into();
            }
            return;
        }
        self.outside_reach(&session, &event, payload);
        // a pergunta só existe pelo hook que espera: o PermissionRequest dela e o PreToolUse geral não fazem cartão nem mexem na pílula
        if tool == "AskUserQuestion" && (event == "PermissionRequest" || (event == "PreToolUse" && reply.is_none())) {
            return;
        }
        // asked here and answered in the terminal, or the turn went on: the card goes. A chamada que acabou tem de ser a
        // do pedido (a mesma ferramenta com a mesma entrada): a de um subagente trabalhando ao lado não conta; a resposta
        // de uma pergunta volta com a entrada mudada, então para ela basta a ferramenta
        let same_call = |ask: &PersonAsk| ask.tool == tool && (tool == "AskUserQuestion" || ask.input == payload["tool_input"]);
        self.settle_elsewhere(|ask, source| {
            source.session == session && (TURN_OVER.contains(&event.as_str()) || (event.starts_with("PostToolUse") && same_call(ask)))
        });
        if event == "SessionEnd" {
            // ponytail: a pill per program and folder, not per session: one session ending takes the pill of a sibling in its folder too until its next event
            self.outside.pills.retain(|pill| pill.key != key);
        } else if !event.is_empty() {
            let index = self.outside.pills.iter().position(|pill| pill.key == key).unwrap_or_else(|| {
                let pill = Pill {
                    key: key.clone(),
                    name: name.clone(),
                    kind: kind.clone(),
                    brand,
                    working: false,
                    asking: false,
                    limited: false,
                    activity: String::new(),
                    badge: None,
                    until: 0,
                    gone: 0,
                    seen: now,
                    shown: 0,
                    session: session.clone(),
                    transcript: String::new(),
                };
                self.outside.pills.push(pill);
                self.outside.pills.len() - 1
            });
            let pill = &mut self.outside.pills[index];
            (pill.seen, pill.name, pill.session) = (now, name.clone(), session.clone());
            if let Some(file) = transcript_of(payload, &session) {
                pill.transcript = file;
            }
            if let Some(line) = activity(&event, payload) {
                pill.activity = line;
            }
            // the Stop hook handed the session what the person wrote (message.rs): its turn goes on
            let delivered = payload["mago_delivered"].as_u64().unwrap_or(0);
            if delivered > 0 {
                pill.activity = if delivered == 1 { "mensagem entregue".into() } else { format!("{delivered} mensagens entregues") };
            }
            match event.as_str() {
                "Stop" | "StopFailure" if delivered == 0 => {
                    (pill.working, pill.asking, pill.limited) = (false, false, false);
                    pill.badge = Some(if event == "Stop" { Badge::Finished } else { Badge::Error });
                    (pill.until, pill.gone) = (now + BADGE_FOR, 0);
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
            self.outside_ask(payload, (key, pill_label(&kind, &name)), session, event, reply);
        }
    }

    /// Tira os cartões das sessões de fora que `settled` aponta: foram resolvidos em outro lugar (no terminal, ou a vez
    /// seguiu) e o hook de cada um é solto sem resposta. Devolve as pílulas deles.
    fn settle_elsewhere(&mut self, settled: impl Fn(&PersonAsk, &Source) -> bool) -> Vec<String> {
        let mut pills = Vec::new();
        self.person_asks.get_mut().retain(|ask| {
            let elsewhere = ask.source.as_ref().filter(|source| settled(ask, source));
            pills.extend(elsewhere.map(|source| source.pill.clone()));
            elsewhere.is_none()
        });
        pills
    }

    /// A permission or an AskUserQuestion of a session outside, as a card for the island; back to the terminal when
    /// the island is off, the queue is full or the app can't answer it right (a question with no options, a plan).
    fn outside_ask(&mut self, payload: &Value, (pill, label): (String, String), session: String, event: String, reply: Sender<Result<String, String>>) {
        let tool = payload["tool_name"].as_str().unwrap_or_default();
        let request = json!({ "tool_name": tool, "input": payload["tool_input"] });
        let mut ask = PersonAsk::new(format!("{ASK_PREFIX}{}", new_id()), &request, reply);
        let unanswerable = (tool == "AskUserQuestion" && ask.questions.is_empty()) || tool == "ExitPlanMode";
        let declined = tool == "AskUserQuestion" && self.outside.declined.borrow().iter().any(|(who, _)| *who == session);
        let alerts_paused = crate::app::system::tray::alerts_paused(self.prefs.pause_alerts_until, crate::app::system::tray::unix_seconds());
        // a question while the island can't show (off, paused, the welcome still on) is the terminal's
        let hidden = !self.prefs.island || alerts_paused || self.welcome.is_some();
        if hidden || self.person_asks.borrow().len() >= MAX_ASKS || unanswerable || declined {
            return ask.send_terminal();
        }
        self.toast(format!("{label} {}", if ask.questions.is_empty() { "precisa de permissão" } else { "está perguntando" }));
        let request = payload["mago_request"].as_str().unwrap_or_default().to_string();
        ask.source = Some(Source { pill, label, session, event, at: now_ms(), request });
        self.person_asks.get_mut().push(ask);
    }

    /// With the agents' check: the asks the hook gave up on go, the badges end and the pills of turns long over leave.
    pub(crate) fn outside_tick(&mut self) {
        let now = now_ms();
        self.person_asks.get_mut().retain(|ask| ask.source.as_ref().is_none_or(|source| now - source.at < WAIT as i64 * 1000));
        self.outside.declined.get_mut().retain(|(_, at)| now - at < DECLINED_FOR);
        let waiting: Vec<String> = self.person_asks.get_mut().iter().filter_map(|ask| ask.source.as_ref().map(|source| source.pill.clone())).collect();
        // a session with messages waiting for its turn to end keeps its pill: the island says how many
        let queued: Vec<String> = self.bridge.as_ref().map(|bridge| locked(&bridge.shared).outbox.keys().cloned().collect()).unwrap_or_default();
        self.outside.pills.retain_mut(|pill| {
            if pill.until != 0 && now >= pill.until {
                (pill.badge, pill.until) = (None, 0);
            }
            let kept = waiting.contains(&pill.key) || queued.contains(&pill.session) || now - pill.shown < SHOWN_FOR;
            let over = pill.gone != 0 && now >= pill.gone && !kept;
            !over && now - pill.seen < SILENT_FOR
        });
        self.outside.sessions.retain(|_, reach| now - reach.seen < SILENT_FOR);
    }

    /// The island shows the conversation of pill `id` (island/session.rs, while it is open): the pill stays though its
    /// turn ended, so the person can still write to it.
    pub(crate) fn outside_shown(&mut self, id: &str) {
        let Some(key) = id.strip_prefix(PILL_PREFIX) else { return };
        for pill in self.outside.pills.iter_mut().filter(|pill| pill.key == key) {
            pill.shown = now_ms();
        }
    }

    /// The pills of the sessions outside, for the island: waiting when an ask of theirs waits; two of one folder (Claude
    /// Code's and Claude Desktop's) told apart by a number after the second's name.
    pub(crate) fn outside_pills(&self) -> Vec<PillView> {
        let waiting: Vec<String> = self.person_asks.borrow().iter().filter_map(|ask| ask.source.as_ref().map(|source| source.pill.clone())).collect();
        let mut pills: Vec<PillView> = self
            .outside
            .pills
            .iter()
            .map(|pill| {
                let asked = waiting.contains(&pill.key);
                let state = match (asked || pill.asking, pill.working) {
                    (true, _) => AgentState::Waiting,
                    (_, true) => AgentState::Working(0.0),
                    _ => AgentState::Idle(None),
                };
                let badge = if asked { Some(Badge::Approval) } else { pill.badge };
                let id = format!("{PILL_PREFIX}{}", pill.key);
                PillView {
                    color: pill.brand.unwrap_or_else(|| self.agent_color(&id)),
                    id,
                    name: pill.name.clone(),
                    kind: pill.kind.clone(),
                    state,
                    badge,
                    activity: pill.activity.clone(),
                    limited: pill.limited,
                    session: pill.session.clone(),
                    asleep: state == AgentState::Idle(None) && badge.is_none() && !pill.limited,
                    at: pill.seen,
                }
            })
            .collect();
        make_distinct(&mut pills, |pill| &mut pill.name);
        pills
    }

    /// The pills that take an agent's colour (`agent_color`): Claude Code's, by their ids.
    pub(crate) fn outside_agent_ids(&self) -> Vec<String> {
        self.outside.pills.iter().filter(|pill| pill.brand.is_none()).map(|pill| format!("{PILL_PREFIX}{}", pill.key)).collect()
    }

    /// The colours the other pills have of their program, which no agent takes.
    pub(crate) fn outside_brands(&self) -> Vec<Color32> {
        self.outside.pills.iter().filter_map(|pill| pill.brand).collect()
    }

    /// As sessões de fora do app para o `demands` do MCP (o Maestro lê em voz alta): uma linha por pílula, com o estado
    /// e a última atividade; vazio quando não há nenhuma.
    pub(crate) fn outside_report(&self) -> String {
        let mut report = String::new();
        for pill in self.outside_pills() {
            let state = if pill.limited { "limite de uso do plano" } else { pill.state.label() };
            let activity = if pill.activity.is_empty() { String::new() } else { format!(" — {}", pill.activity) };
            report += &format!("\n- {} — {state}{activity}", pill.label());
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

    /// The file where Claude Code writes the conversation of pill `id`'s session, when it is there (Hermes keeps none).
    pub(crate) fn outside_transcript(&self, id: &str) -> Option<PathBuf> {
        let key = id.strip_prefix(PILL_PREFIX)?;
        let pill = self.outside.pills.iter().find(|pill| pill.key == key && !pill.transcript.is_empty())?;
        Some(PathBuf::from(&pill.transcript)).filter(|path| path.is_file())
    }

    /// What the conversation of pill `id` says while `outside_transcript` has no file: Claude Code writes it with the
    /// first message; Hermes and Cursor keep none the app reads.
    pub(crate) fn outside_unread(&self, id: &str) -> &'static str {
        let key = id.strip_prefix(PILL_PREFIX).unwrap_or_default();
        match self.outside.pills.iter().any(|pill| pill.key == key && !pill.transcript.is_empty()) {
            true => "Ainda não há mensagens nesta sessão.",
            false => "A conversa desta sessão não pode ser lida daqui.",
        }
    }

    /// Pill `id` as a row of the chat screen, whose conversation it shows (chat/mod.rs): a Claude Code of its folder.
    pub(crate) fn outside_row(&self, id: &str) -> Option<crate::app::ui::agents::AgentRow> {
        let view = self.outside_pills().into_iter().find(|pill| pill.id == id)?;
        let cwd = view.id.strip_prefix(PILL_PREFIX).and_then(|key| key.split_once(':')).map(|(_, cwd)| cwd.to_string()).unwrap_or_default();
        // chat: a conversation only, the session's terminal is not the app's
        let spec = TerminalSpec { name: view.name, cwd, command: "claude".into(), chat: true, ..Default::default() };
        Some(crate::app::ui::agents::AgentRow { id: view.id, spec, state: view.state, mission: None, space: crate::app::chat::list::running::OUTSIDE.into() })
    }

    /// "Ir ao terminal" of pill `id`: its session's window to the front. False when no window of it is known, the system
    /// did not let it come, or `id` is no pill.
    pub(crate) fn outside_terminal(&self, id: &str) -> bool {
        let Some(key) = id.strip_prefix(PILL_PREFIX) else { return false };
        let Some(pill) = self.outside.pills.iter().find(|pill| pill.key == key) else { return false };
        crate::app::maestro::session_window::focus_session_window(&pill.session)
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    #[test]
    fn a_session_is_named_by_its_folder_and_its_program_goes_apart() {
        let code = origin(&json!({ "cwd": "/var/www/projects/magoagent/.claude/worktrees/agent-a47cc47699b17a639" }));
        assert_eq!((code.name.as_str(), code.kind.as_str(), code.brand), ("agent-a47cc47", "Claude Code", None));
        assert!(!code.name.starts_with("Claude Code"), "no program before the name");
        let desktop = origin(&json!({ "cwd": "C:/Users/eu/loja", "mago_origin": "claude-desktop" }));
        assert_eq!((desktop.name.as_str(), pill_label(&desktop.kind, &desktop.name).as_str()), ("loja", "Claude Desktop · loja"));
        let hermes = origin(&json!({ "mago_origin": "hermes" }));
        assert_eq!(pill_label(&hermes.kind, &hermes.name), "Hermes");
        assert_eq!(origin(&json!({})).name, "Claude Code", "no folder: the program's");
    }

    #[test]
    fn a_session_in_wsl_is_read_from_windows_by_its_distro() {
        let payload = json!({ "transcript_path": "/home/eu/.claude/projects/-home-eu-loja/s1.jsonl", "wsl_distro": "Ubuntu" });
        let expected = if cfg!(windows) {
            r"\\wsl.localhost\Ubuntu\home\eu\.claude\projects\-home-eu-loja\s1.jsonl"
        } else {
            "/home/eu/.claude/projects/-home-eu-loja/s1.jsonl"
        };
        assert_eq!(transcript_of(&payload, "s1").as_deref(), Some(expected));
        let here = json!({ "transcript_path": r"C:\Users\eu\.claude\projects\C--loja\s1.jsonl", "wsl_distro": "" });
        assert_eq!(transcript_of(&here, "s1").as_deref(), Some(r"C:\Users\eu\.claude\projects\C--loja\s1.jsonl"));
    }
}
