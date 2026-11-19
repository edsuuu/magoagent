//! A message from the person to a Claude Code session outside the app (outside.rs), written in the island's conversation
//! of it (island/session.rs) or asked of the MCP (`outside_message`). A session running in tmux gets it typed in its
//! pane at once, with its Enter, when nothing on its screen waits for the person: a permission or a question there would
//! take the words as their answer. Otherwise it waits in the bridge's outbox (`Shared::outbox`) for the session's next
//! `Stop` hook, whose answer blocks the stop with it (mcp/bridge/handle.rs) and Claude Code goes on: at the end of this
//! turn, or of the next one for a session already idle.

use super::outside::{PILL_PREFIX, PillView, TURN_OVER};
use crate::prelude::*;

/// Between the text and its Enter: Claude Code takes keys that come together as a paste, and an Enter inside a paste
/// is a new line, not the send.
#[cfg(all(unix, not(test)))]
const ENTER_AFTER: std::time::Duration = std::time::Duration::from_millis(500);
/// What runs in front in a pane when Claude Code is not there: the text would be a command.
const SHELLS: [&str; 12] = ["bash", "zsh", "fish", "sh", "dash", "ksh", "mksh", "tcsh", "csh", "nu", "pwsh", "xonsh"];

/// How a message went.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Delivery {
    /// Typed in the session's tmux pane, with its Enter.
    Typed,
    /// In the outbox, for the end of the session's turn; `idle`: it is not working, so that is the end of its next one.
    Queued { idle: bool },
}

impl Delivery {
    /// What the island's box says after sending.
    pub(crate) fn notice(self) -> &'static str {
        match self {
            Delivery::Typed => "Enviada no terminal (tmux).",
            Delivery::Queued { idle: false } => "Na fila: entra quando o Claude terminar esta vez.",
            Delivery::Queued { idle: true } => "Na fila: a sessão está parada; entra quando ela trabalhar de novo.",
        }
    }
}

/// The socket of the tmux server in `TMUX` ("/tmp/tmux-1000/default,4242,0"): its first field, a path from the root.
pub(crate) fn tmux_socket(value: &str) -> Option<&str> {
    value.split(',').next().filter(|socket| socket.starts_with('/'))
}

/// A tmux pane's id as `TMUX_PANE` has it: "%" and digits.
pub(crate) fn tmux_pane(value: &str) -> bool {
    value.strip_prefix('%').is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
}

/// The tmux pane a session runs in: its server's socket and the pane's id.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pane {
    socket: String,
    id: String,
}

/// What the app knows of a session outside to reach it, from its hooks.
#[derive(Default)]
pub(crate) struct Reach {
    pane: Option<Pane>,
    /// The calls on its terminal's screen waiting for the person (tool, input): a permission, a question.
    held: Vec<(String, Value)>,
    /// Its last hook, ms.
    pub(super) seen: i64,
}

/// The text as one line of keys: a line break would be an Enter, a control character a key of its own.
fn keys(text: &str) -> String {
    let line = text.split(['\r', '\n']).map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(" ");

    line.chars().filter(|character| !character.is_control()).collect()
}

/// The program in front in a pane (`pane_current_command`) is a shell, not Claude Code.
fn shell_in_front(command: &str) -> bool {
    let command = command.trim().trim_start_matches('-');

    command.is_empty() || SHELLS.contains(&command)
}

/// Runs tmux; what it printed, or why it failed.
// ponytail: runs on the UI thread (three short calls on a send); a tmux server that hangs would hold a frame, upgrade: a thread with a timeout
#[cfg(all(unix, not(test)))]
fn run_tmux(args: &[String]) -> Result<String, String> {
    // an app opened from the Finder has no Homebrew in its PATH
    let program = ["/opt/homebrew/bin/tmux", "/usr/local/bin/tmux"]
        .into_iter()
        .find(|path| cfg!(target_os = "macos") && std::path::Path::new(path).is_file())
        .unwrap_or("tmux");

    let output = crate::editor::command(program).args(args).stdin(std::process::Stdio::null()).output().map_err(|error| format!("tmux: {error}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

impl super::outside::Outside {
    /// Runs tmux with `args`: in the tests only their fake, never the person's tmux.
    fn tmux(&self, args: &[String]) -> Result<String, String> {
        #[cfg(test)]
        {
            self.fake_tmux.as_ref().map_or_else(|| Err("tmux de verdade não roda nos testes".to_string()), |fake| fake(args))
        }

        #[cfg(all(unix, not(test)))]
        {
            run_tmux(args)
        }

        #[cfg(all(not(unix), not(test)))]
        {
            let _ = args;

            Err("o tmux é do Linux e do macOS".to_string())
        }
    }

    /// The Enter after the text, a moment later (`ENTER_AFTER`), off the UI thread.
    fn tmux_later(&self, args: Vec<String>) {
        #[cfg(test)]
        {
            let _ = self.tmux(&args);
        }

        #[cfg(all(unix, not(test)))]
        {
            std::thread::spawn(move || {
                std::thread::sleep(ENTER_AFTER);
                let _ = run_tmux(&args);
            });
        }

        #[cfg(all(not(unix), not(test)))]
        {
            let _ = args;
        }
    }

    /// Types `text` in `pane` and then its Enter. Nothing is typed, and the error says why, when Claude Code is not in
    /// front there (a shell would run the text) or its screen asks something (the text would answer it).
    fn type_in(&self, pane: &Pane, text: &str) -> Result<(), String> {
        let call = |words: &[&str]| -> Vec<String> { ["-S", pane.socket.as_str()].iter().chain(words).map(|word| word.to_string()).collect() };

        if shell_in_front(&self.tmux(&call(&["display-message", "-p", "-t", &pane.id, "#{pane_current_command}"]))?) {
            return Err("o painel está no shell".into());
        }

        if crate::terminal::asks_for_you(&self.tmux(&call(&["capture-pane", "-p", "-t", &pane.id]))?) {
            return Err("há uma pergunta na tela".into());
        }

        self.tmux(&call(&["send-keys", "-t", &pane.id, "-l", "--", &keys(text)]))?;
        self.tmux_later(call(&["send-keys", "-t", &pane.id, "Enter"]));
        Ok(())
    }
}

impl DrawingApp {
    /// What a hook of `session` tells of how to reach it: its tmux pane, and the calls that wait on its screen (from the
    /// permission or the question until that call ends or the turn is over). A session that ended takes its messages
    /// along: nobody would read them.
    pub(super) fn outside_reach(&mut self, session: &str, event: &str, payload: &Value) {
        if session.is_empty() {
            return;
        }

        if event == "SessionEnd" {
            self.outside.sessions.remove(session);

            if let Some(bridge) = &self.bridge {
                locked(&bridge.shared).outbox.remove(session);
            }

            return;
        }

        let text = |key: &str| payload[key].as_str().unwrap_or_default();
        let reach = self.outside.sessions.entry(session.to_string()).or_default();

        reach.seen = now_ms();

        if let Some(socket) = tmux_socket(text("tmux_socket"))
            && tmux_pane(text("tmux_pane"))
        {
            reach.pane = Some(Pane { socket: socket.to_string(), id: text("tmux_pane").to_string() });
        }

        let (tool, input) = (text("tool_name"), &payload["tool_input"]);
        // the answers come back in a question's input: for it the tool is enough
        let same = |(held, its_input): &(String, Value)| held == tool && (tool == "AskUserQuestion" || its_input == input);
        let waits = event == "PermissionRequest" || (event == "PreToolUse" && tool == "AskUserQuestion");

        if waits && !reach.held.iter().any(same) {
            reach.held.push((tool.to_string(), input.clone()));
        } else if event.starts_with("PostToolUse") {
            reach.held.retain(|call| !same(call));
        } else if TURN_OVER.contains(&event) {
            reach.held.clear();
        }
    }

    /// Pill `id` takes messages from the app: a Claude Code session the hooks named. Claude Desktop, Cursor and Hermes
    /// are answered in their own windows.
    pub(crate) fn outside_takes_messages(&self, id: &str) -> bool {
        self.outside.pill(id).is_some_and(|pill| pill.key.starts_with("code:") && !pill.session.is_empty())
    }

    /// The messages for the session of pill `id` waiting in the outbox for its turn to end.
    pub(crate) fn outside_pending(&self, id: &str) -> usize {
        let Some(pill) = self.outside.pill(id) else { return 0 };

        self.bridge.as_ref().map_or(0, |bridge| locked(&bridge.shared).outbox.get(&pill.session).map_or(0, Vec::len))
    }

    /// Sends `text` to the session of pill `id`: typed in its tmux pane when it runs in one and nothing waits for the
    /// person there, else into the outbox for its next Stop. With messages already waiting it goes after them, in order.
    pub(crate) fn outside_message(&mut self, id: &str, text: &str) -> Result<Delivery, String> {
        let text = text.trim();

        if text.is_empty() {
            return Err("Escreva a mensagem.".into());
        }

        if !self.outside_takes_messages(id) {
            return Err(match self.outside.pill(id) {
                Some(_) => "Só uma sessão do Claude Code recebe mensagens daqui: responda no terminal dela.",
                None => "Essa sessão não está mais aberta.",
            }
            .into());
        }

        let Some(pill) = self.outside.pill(id) else { return Err("Essa sessão não está mais aberta.".into()) };
        let (session, working) = (pill.session.clone(), pill.working);
        let asked = self.person_asks.borrow().iter().any(|ask| ask.source.as_ref().is_some_and(|source| source.pill == pill.key));
        let reach = self.outside.sessions.get(&session);
        let asking = asked || pill.asking || reach.is_some_and(|reach| !reach.held.is_empty());
        let behind = self.outside_pending(id) > 0;

        if !asking
            && !behind
            && let Some(pane) = reach.and_then(|reach| reach.pane.as_ref())
            && self.outside.type_in(pane, text).is_ok()
        {
            return Ok(Delivery::Typed);
        }

        let bridge = self.bridge.as_ref().ok_or("O MagoAgent não está recebendo os hooks agora: responda no terminal.")?;

        locked(&bridge.shared).outbox.entry(session).or_default().push(text.to_string());
        Ok(Delivery::Queued { idle: !working && !asking })
    }

    /// The MCP's `outside_message`: `text` to the session outside named `name` (its pill's name or folder, or part of
    /// it; empty when there is only one).
    pub(crate) fn act_outside_message(&mut self, name: &str, text: &str) -> Result<String, String> {
        let pills: Vec<PillView> = self.outside_pills().into_iter().filter(|pill| self.outside_takes_messages(&pill.id)).collect();

        if pills.is_empty() {
            return Err("Nenhuma sessão do Claude Code fora do app agora (as que os hooks do Claude Code avisam).".into());
        }

        let wanted = name.trim().to_lowercase();

        // what it is called: "Claude Code · loja", "loja" and its folder's whole name (the pill's is cut, `short_name`)
        let called = |pill: &PillView| {
            let cwd = pill.id.strip_prefix(PILL_PREFIX).and_then(|key| key.split_once(':')).map_or("", |(_, cwd)| cwd);
            let folder = if cwd.is_empty() { pill.name.clone() } else { crate::app::chat::direct::folder_label(cwd) };

            [pill.label(), pill.name.clone(), folder].map(|name| name.to_lowercase())
        };

        let exact: Vec<&PillView> = pills.iter().filter(|pill| called(pill).contains(&wanted)).collect();
        let found = if exact.is_empty() { pills.iter().filter(|pill| called(pill).iter().any(|name| name.contains(&wanted))).collect() } else { exact };
        let names = pills.iter().map(|pill| format!("\"{}\"", pill.label())).collect::<Vec<_>>().join(", ");

        match found.as_slice() {
            [pill] => {
                let (id, label) = (pill.id.clone(), pill.label());

                Ok(format!("{label}: {}", self.outside_message(&id, text)?.notice()))
            }
            [] => Err(format!("Nenhuma sessão de fora chamada \"{name}\". Há: {names}.")),
            _ => Err(format!("Mais de uma sessão serve ({names}): diga qual em `session`.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_typed_is_one_line_and_a_shell_in_front_is_no_claude() {
        assert_eq!(keys("  roda os testes\r\n\n e me diz\t o resultado \u{1b}[A"), "roda os testes e me diz o resultado [A");
        assert!(shell_in_front("zsh\n") && shell_in_front("-bash") && shell_in_front(""));
        assert!(!shell_in_front("claude") && !shell_in_front("node"));
    }
}
