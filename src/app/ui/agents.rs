//! The agents panel: every terminal at a glance (working, idle, waiting for you) like a team
//! board, kept open beside the canvas while you work; and the keys to move between chat,
//! canvas and whoever needs you.

use crate::prelude::*;

pub(crate) const AMBER: Color32 = Color32::from_rgb(0xf0, 0xa0, 0x20);

/// A terminal title Claude Code set: "✳ topic" at rest, a spinner (braille) while it works, or
/// its name.
pub(crate) fn claude_title(title: &str) -> bool {
    let t = title.trim();
    t.to_lowercase().starts_with("claude") || t.starts_with('✳') || t.starts_with(|c: char| ('\u{2800}'..='\u{28ff}').contains(&c))
}

/// A title a shell gives its terminal ("user@host:~", its program's or folder's path): it says
/// nothing about the task.
pub(crate) fn shell_title(title: &str) -> bool {
    let t = title.trim();
    (t.contains('@') && t.contains(':')) || t.get(1..3) == Some(":\\") || t.starts_with(['/', '~'])
}

/// Why a prompt typed into a terminal now wouldn't reach its agent (`TermHandle::ready`): its
/// Claude Code session waits on a question, or the claude it ran (`had_session`) or was made for
/// left (`/exit`, a crash) and the text would run in the shell. A terminal made for it is given
/// 20 s to start, and the benefit of the doubt with a title of Claude's on it (its sessions may
/// be filed elsewhere, CLAUDE_CONFIG_DIR).
// ponytail: other CLIs (codex…) get no such check; upgrade: tcgetpgrp on Unix, the shell's
// child processes on Windows
pub(crate) fn prompt_gate(claude: Option<&crate::transcript::Session>, made_for_claude: bool, had_session: bool, age_ms: i64, title: &str) -> Option<String> {
    match claude {
        Some(s) if s.status == "waiting" => Some(format!(
            "espera uma resposta no próprio terminal ({}): o texto iria como resposta. Responda lá primeiro.",
            s.waiting_for.as_deref().unwrap_or("permissão")
        )),
        None if had_session || (made_for_claude && age_ms > 20_000 && !claude_title(title)) => {
            Some("não está com o claude aberto: o texto iria para o shell como comando.".into())
        }
        _ => None,
    }
}

/// "agora", "há 30s", "há 12 min", "há 2 h".
// ponytail: relative times; "desde 19h43" would need the local time zone (no date crate here)
pub(crate) fn ago(seconds: f64) -> String {
    match seconds.max(0.0) as u64 {
        0..=4 => "agora".into(),
        s @ 5..=59 => format!("há {s}s"),
        s @ 60..=3599 => format!("há {} min", s / 60),
        s => format!("há {} h", s / 3600),
    }
}

/// What an agent is doing, since when (app seconds). Working, Idle and Waiting come from its
/// Claude Code session's own file when it has one; Active is only what its screen shows (a
/// program printing is not an agent working).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AgentState {
    Working(f64),
    Active(f64),
    Waiting,
    Idle(Option<f64>),
    Ended,
}

impl AgentState {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            AgentState::Working(_) => "Trabalhando",
            AgentState::Active(_) => "Ativo",
            AgentState::Waiting => "Precisa de você",
            AgentState::Idle(_) => "Ocioso",
            AgentState::Ended => "Encerrado",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AgentRow {
    pub(crate) id: String,
    pub(crate) spec: TerminalSpec,
    pub(crate) state: AgentState,
    /// What the agent says it's doing: Claude Code and others title their terminal.
    pub(crate) mission: Option<String>,
    /// The workspace of a terminal's (workspaces.rs); "" for a chat, which belongs to none.
    pub(crate) space: String,
}

pub(crate) use crate::app::maestro::events::AgentEvent;

impl DrawingApp {
    /// Every terminal, in scene order, then the ones running in the workspaces off view (by space,
    /// then name).
    pub(crate) fn agent_rows(&self) -> Vec<AgentRow> {
        let mut rows = self.chat_rows();
        let row = |id: &String, spec: &TerminalSpec, space: &String| {
            let session = self.terminals.get(id).and_then(|s| s.as_ref().ok()).filter(|s| !s.exited());
            // no process, as the reference app shows it: idle, not ended
            let state = if spec.unloaded { AgentState::Idle(None) } else { self.agent_state(id) };
            let mission = session.and_then(|s| s.title()).filter(|t| !shell_title(t));
            AgentRow { id: id.clone(), spec: spec.clone(), state, mission, space: space.clone() }
        };
        rows.extend(self.scene.elements.iter().filter_map(|e| Some(row(&e.id, e.terminal.as_ref()?, &self.scope_key()))));
        let mut away: Vec<AgentRow> = self.away.iter().map(|(id, (space, spec))| row(id, spec, space)).collect();
        let order = |space: &str| self.spaces.list.iter().position(|w| w.id == space.split('#').next().unwrap_or_default());
        away.sort_by(|a, b| (order(&a.space), a.spec.label()).cmp(&(order(&b.space), b.spec.label())));
        rows.extend(away);
        rows
    }

    /// The workspace of a row that is not the one on screen, by name.
    pub(crate) fn elsewhere(&self, row: &AgentRow) -> Option<String> {
        (!row.space.is_empty() && row.space != self.scope_key()).then(|| self.space_name(&row.space))
    }

    /// The terminals drawn on screen and the ones running off view: (id, spec).
    fn terminal_specs(&self) -> Vec<(&String, &TerminalSpec)> {
        let drawn = self.scene.elements.iter().filter_map(|e| Some((&e.id, e.terminal.as_ref()?)));
        drawn.chain(self.away.iter().map(|(id, (_, spec))| (id, spec))).collect()
    }

    /// Since when, or what for: "há 2 min", "input needed" (when its session says).
    pub(crate) fn state_note(&self, row: &AgentRow) -> Option<String> {
        match row.state {
            AgentState::Working(since) | AgentState::Active(since) => Some(ago(self.now - since)),
            AgentState::Idle(since) => since.map(|s| ago(self.now - s)),
            AgentState::Waiting => self.agent_sessions.get(&row.id).and_then(|s| s.waiting_for.clone()),
            AgentState::Ended => None,
        }
    }

    /// What a terminal's agent is doing: its Claude Code session says so when there is one, else
    /// what its screen shows.
    pub(crate) fn agent_state(&self, id: &str) -> AgentState {
        if let Some(state) = self.chat_state(id) {
            return state;
        }
        let Some(session) = self.terminals.get(id).and_then(|s| s.as_ref().ok()).filter(|s| !s.exited()) else { return AgentState::Ended };
        if self.terminal_waiting.contains(id) {
            return AgentState::Waiting;
        }
        let since = |ms: i64| self.now - (now_ms() - ms).max(0) as f64 / 1000.0;
        match self.agent_sessions.get(id) {
            Some(s) if s.status == "busy" => AgentState::Working(since(s.status_at)),
            Some(s) if !s.status.is_empty() => AgentState::Idle(Some(since(s.status_at))),
            // other programs (and a Claude Code too old to say): the screen
            _ if session.handle.busy() => AgentState::Active(self.terminal_busy.get(id).copied().unwrap_or(self.now)),
            _ => AgentState::Idle(self.terminal_idle.get(id).copied()),
        }
    }

    /// The terminals running Claude Code, in every workspace open: (id, folder, started ms, shell pid), by
    /// their command or the title Claude gives them.
    pub(crate) fn claude_terminals(&self) -> Vec<(String, String, i64, u32)> {
        self.terminal_specs()
            .into_iter()
            .filter_map(|(id, spec)| {
                let session = self.terminals.get(id)?.as_ref().ok().filter(|s| !s.exited())?;
                let runs = spec.command.contains("claude") || claude_title(&session.title().unwrap_or_default());
                runs.then_some((id.clone(), session.cwd.as_ref()?.to_str()?.to_string(), session.started_ms, session.shell_pid))
            })
            .collect()
    }

    /// Twice a second at most: which Claude Code session each terminal runs (its file says whether
    /// it works, rests or waits on a question), which terminals wait on one, and what may be typed
    /// into each (`TermHandle::ready`). Maps of terminals that went are pruned.
    pub(crate) fn check_agents(&mut self) {
        if self.now - self.waiting_checked < 0.5 {
            return;
        }
        self.waiting_checked = self.now;
        let claude = self.claude_terminals();
        let dir = crate::transcript::claude_dir();
        if cfg!(test) {
            // a test reads what it just wrote: here, at once
            self.agent_sessions = match &dir {
                Some(dir) if !claude.is_empty() => {
                    crate::transcript::assign(&claude, &crate::transcript::sessions(&dir.join("sessions")), crate::transcript::parents).into_iter().collect()
                }
                _ => Default::default(),
            };
        } else if claude.is_empty() {
            self.agent_sessions.clear();
        } else {
            // the files and the process list are read off this thread (transcript/watch.rs): what
            // it found last stays until it finds again, at most a check behind
            self.session_watch.ask(claude, dir.clone());
            if let Some(found) = self.session_watch.take() {
                self.agent_sessions = found.into_iter().collect();
            }
        }
        self.remember_sessions();
        if let Some(bridge) = &self.bridge {
            locked(&bridge.shared).transcripts = self.agent_sessions.iter().filter_map(|(id, s)| Some((id.clone(), s.transcript(dir.as_ref()?)))).collect();
        }
        let terminals = &self.terminals;
        self.terminal_busy.retain(|id, _| terminals.contains_key(id));
        self.terminal_idle.retain(|id, _| terminals.contains_key(id));
        self.terminal_unread.retain(|id| terminals.contains_key(id));
        // the direct chats keep theirs (chat/direct.rs), and a terminal without a shell the one it remembers
        let (chats, scene) = (&self.chats, &self.scene);
        self.transcripts.retain(|id, _| terminals.contains_key(id) || chats.iter().any(|c| &c.id == id) || scene.elements.iter().any(|e| &e.id == id));
        let made_for_claude: std::collections::HashSet<String> =
            self.terminal_specs().into_iter().filter(|(_, t)| t.command.contains("claude")).map(|(id, _)| id.clone()).collect();
        let mut waiting = std::collections::HashSet::new();
        for (id, session) in self.terminals.iter().filter_map(|(id, s)| Some((id, s.as_ref().ok()?))) {
            let claude = self.agent_sessions.get(id);
            let asks = match claude {
                Some(s) if !s.status.is_empty() => s.status == "waiting",
                _ => !session.exited() && !session.handle.busy() && session.handle.waiting(),
            };
            if asks {
                waiting.insert(id.clone());
            }
            if claude.is_some() {
                self.claude_seen.insert(id.clone(), session.started_ms);
            }
            let had_session = self.claude_seen.get(id) == Some(&session.started_ms);
            let title = session.title().unwrap_or_default();
            let gate = prompt_gate(claude, made_for_claude.contains(id.as_str()), had_session, now_ms() - session.started_ms, &title);
            session.handle.set_gate(gate, claude.is_some_and(|s| !s.status.is_empty()));
        }
        self.claude_seen.retain(|id, _| terminals.contains_key(id));
        self.terminal_waiting = waiting;
        if let Some(bridge) = &self.bridge {
            let states = self.terminals.keys().chain(self.chats.iter().map(|c| &c.id)).map(|id| (id.clone(), self.agent_state(id))).collect();
            locked(&bridge.shared).states = states;
        }
        self.log_agent_events();
        // what the chat queued for an agent that waited goes once it doesn't
        self.flush_chat_queue();
    }

    /// Each terminal keeps the conversation it runs in the drawing, for its chat and `claude --resume`
    /// after a restart. Written only when it changes, and like a portal's address, no undo step: the
    /// merge with the file keeps it unless an agent changed that terminal meanwhile (the next check
    /// writes it again).
    pub(crate) fn remember_sessions(&mut self) {
        for (id, session) in &self.agent_sessions {
            let folder = crate::transcript::project_folder(&session.cwd);
            let Some(t) = self.scene.elements.iter_mut().find(|e| &e.id == id).and_then(|e| e.terminal.as_mut()) else { continue };
            if t.claude_session != session.id || t.claude_folder != folder {
                (t.claude_session, t.claude_folder) = (session.id.clone(), folder);
                self.dirty = true;
            }
        }
    }

    /// What changed since the last check, as events: a Claude Code session's word, a menu on
    /// screen, the bridge's questions. A program printing proves nothing, so Active is none. The
    /// hooks' events come on their own (demands.rs).
    pub(crate) fn log_agent_events(&mut self) {
        self.events_tick();
        let asked = self.bridge.as_ref().map(|b| std::mem::take(&mut locked(&b.shared).events)).unwrap_or_default();
        for (from, to, what) in asked {
            let name = if from.is_empty() { "fora do canvas".to_string() } else { self.agent_label(&from) };
            let mut event = AgentEvent::new(&from, &name, EventKind::Message, what);
            event.to = to;
            self.record(event);
        }
        // an unloaded one leaves the log quietly and comes back as iniciado
        let ids: Vec<String> = self.terminal_specs().into_iter().filter(|(_, t)| !t.unloaded).map(|(id, _)| id.clone()).collect();
        for id in &ids {
            let state = self.agent_state(id);
            let what = match (self.agent_seen.insert(id.clone(), state), state) {
                (None, AgentState::Ended) | (Some(AgentState::Working(_)), AgentState::Working(_)) | (Some(AgentState::Waiting), AgentState::Waiting) => None,
                (None, _) => Some((EventKind::Started, "iniciado".to_string())),
                (Some(AgentState::Ended), AgentState::Ended) => None,
                // restarted
                (Some(AgentState::Ended), _) => Some((EventKind::Started, "iniciado".to_string())),
                (_, AgentState::Working(_)) => Some((EventKind::Working, "começou a trabalhar".into())),
                (_, AgentState::Waiting) => Some(match self.agent_sessions.get(id).and_then(|s| s.waiting_for.clone()) {
                    Some(why) if why.contains("permission") => (EventKind::Permission, format!("precisa de você ({why})")),
                    Some(why) => (EventKind::Question, format!("precisa de você ({why})")),
                    None => (EventKind::Question, "precisa de você".into()),
                }),
                (Some(AgentState::Working(_)), AgentState::Idle(_)) => Some((EventKind::Finished, "terminou".into())),
                (_, AgentState::Ended) => Some((EventKind::Ended, "encerrado".into())),
                _ => None,
            };
            if let Some((kind, what)) = what {
                self.agent_event(id, kind, what);
            }
        }
        self.agent_seen.retain(|id, _| ids.contains(id));
    }

    pub(crate) fn agent_event(&mut self, id: &str, kind: EventKind, what: impl Into<String>) {
        let event = AgentEvent::new(id, &self.agent_label(id), kind, what);
        self.record(event);
    }

    /// The chat (⌘1), the canvas (⌘2), the first file tree (⌘3) and the next agent that needs you (⌘⇧A), by the keys
    /// Ajustes › Atalhos has for them: they work even while a terminal has the keyboard, which never gets them. None
    /// while Ajustes records a shortcut's keys.
    pub(crate) fn screen_keys(&mut self, ctx: &Context) {
        use crate::app::hotkeys::local::AppAction;
        if self.hotkeys.recording.is_some() {
            return;
        }
        if self.take_app_key(ctx, AppAction::ShowChat) {
            self.show_chat();
        }
        if self.take_app_key(ctx, AppAction::ShowCanvas) {
            self.show_canvas();
        }
        if self.take_app_key(ctx, AppAction::OpenTree) {
            self.open_tree();
        }
        if self.take_app_key(ctx, AppAction::NextNeedingYou) {
            self.next_needing_attention();
        }
    }

    /// ⌘1: the chat screen.
    pub(crate) fn show_chat(&mut self) {
        self.chat_open = true;
        self.terminal_focus = None;
    }

    /// The chat, on the agent of terminal `id` (its bar's Chat button, a chat-mode terminal).
    pub(crate) fn chat_on(&mut self, id: &str) {
        self.show_chat();
        self.open_agent(id);
    }

    /// ⌘2: the canvas itself; an enlarged IDE goes back to its card.
    pub(crate) fn show_canvas(&mut self) {
        self.chat_open = false;
        if self.terminal_elevated.as_ref().is_some_and(|id| self.scene.elements.iter().any(|e| &e.id == id && e.filetree.is_some())) {
            self.set_elevated(None);
        }
    }

    /// The agents that need the person, in every workspace: the waiting ones first, then the ones that finished unseen
    /// (each once).
    pub(crate) fn needing_you(&self) -> Vec<String> {
        let rows = self.agent_rows();
        let waiting = rows.iter().filter(|r| matches!(r.state, AgentState::Waiting));
        let unread = rows.iter().filter(|r| self.terminal_unread.contains(&r.id) && !matches!(r.state, AgentState::Waiting));
        waiting.chain(unread).map(|r| r.id.clone()).collect()
    }

    /// ⌘⇧A and the sidebar's "Ir para o próximo": the next one of `needing_you` after the current one, round. Gives
    /// the one it opened.
    pub(crate) fn next_needing_attention(&mut self) -> Option<String> {
        let queue = self.needing_you();
        let current = if self.chat_open { self.chat_agent.clone() } else { self.terminal_focus.clone() };
        let after = current.and_then(|id| queue.iter().position(|q| *q == id)).map_or(0, |p| p + 1);
        let Some(next) = queue.get(after % queue.len().max(1)).cloned() else {
            self.toast("Nenhum agente precisa de você");
            return None;
        };
        self.open_agent(&next);
        Some(next)
    }

    /// In the chat, shows the agent's conversation; on the canvas, brings its terminal to the
    /// middle of the screen, selected and taking keys.
    pub(crate) fn open_agent(&mut self, id: &str) {
        if self.chat_open {
            self.terminal_unread.remove(id);
            self.chat_agent = Some(id.to_string());
            return;
        }
        // an agent of a workspace off view: that workspace comes on screen first
        if let Some(Err(error)) = self.away.get(id).map(|(space, _)| space.clone()).map(|space| self.switch_space(&space)) {
            self.toast(error);
            return;
        }
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return };
        self.glide_to(element_bounds(&self.scene.elements[i]).center());
        self.selected = vec![i];
        self.terminal_focus = Some(id.to_string());
        self.terminal_unread.remove(id);
    }
}
