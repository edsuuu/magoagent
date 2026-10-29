//! The agents panel: every terminal at a glance (working, idle, waiting for you) like a team
//! board, kept open beside the canvas while you work; and the keys to move between chat,
//! canvas and whoever needs you.

use crate::prelude::*;

pub(crate) const AMBER: Color32 = Color32::from_rgb(0xf0, 0xa0, 0x20);
const GREEN: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x50);
pub(crate) const RED: Color32 = Color32::from_rgb(0xf8, 0x51, 0x49);

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

    /// The status dot: its color and whether it's filled.
    pub(crate) fn dot(&self, c: &UiColors) -> (Color32, bool) {
        match self {
            AgentState::Working(_) => (GREEN, true),
            AgentState::Active(_) => (GREEN, false),
            AgentState::Waiting => (AMBER, true),
            AgentState::Idle(_) => (c.muted, false),
            AgentState::Ended => (RED, true),
        }
    }

    pub(crate) fn paint_dot(&self, painter: &Painter, at: Pos2, c: &UiColors) {
        match self.dot(c) {
            (color, true) => painter.circle_filled(at, 4.0, color),
            (color, false) => painter.circle_stroke(at, 3.5, Stroke::new(1.2, color)),
        };
    }
}

pub(crate) struct AgentRow {
    pub(crate) id: String,
    pub(crate) spec: TerminalSpec,
    pub(crate) state: AgentState,
    /// What the agent says it's doing: Claude Code and others title their terminal.
    pub(crate) mission: Option<String>,
    /// The workspace it's in, when that isn't the one on screen.
    pub(crate) space: Option<String>,
}

/// Something an agent's state or the bridge proves happened (when, app seconds).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AgentEvent {
    pub(crate) at: f64,
    pub(crate) id: String,
    /// Its terminal's name then: the event outlives the terminal.
    pub(crate) name: String,
    pub(crate) what: String,
}

impl DrawingApp {
    /// Every terminal: the workspace on screen's in scene order, then the hidden workspaces' (their
    /// shells go on meanwhile).
    pub(crate) fn agent_rows(&self) -> Vec<AgentRow> {
        let hidden = self
            .spaces
            .spaces
            .iter()
            .filter_map(|s| Some((s, self.parked.get(&s.id)?)))
            .flat_map(|(s, p)| p.scene.elements.iter().map(move |e| (e, Some(s.name.clone()))));
        self.scene
            .elements
            .iter()
            .map(|e| (e, None))
            .chain(hidden)
            .filter_map(|(e, space)| {
                let spec = e.terminal.clone()?;
                let session = self.terminals.get(&e.id).and_then(|s| s.as_ref().ok()).filter(|s| !s.exited());
                // no process, as the reference app shows it: idle, not ended
                let state = if spec.unloaded { AgentState::Idle(None) } else { self.agent_state(&e.id) };
                let mission = session.and_then(|s| s.title()).filter(|t| !shell_title(t));
                Some(AgentRow { id: e.id.clone(), spec, state, mission, space })
            })
            .collect()
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

    /// The terminals running Claude Code, in every workspace open: (id, folder, started ms), by
    /// their command or the title Claude gives them.
    pub(crate) fn claude_terminals(&self) -> Vec<(String, String, i64)> {
        self.all_elements()
            .filter_map(|e| {
                let session = self.terminals.get(&e.id)?.as_ref().ok().filter(|s| !s.exited())?;
                let runs = e.terminal.as_ref()?.command.contains("claude") || claude_title(&session.title().unwrap_or_default());
                Some((e.id.clone(), session.cwd.as_ref()?.to_str()?.to_string(), session.started_ms)).filter(|_| runs)
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
        self.agent_sessions = match &dir {
            Some(dir) if !claude.is_empty() => crate::transcript::assign(&claude, &crate::transcript::sessions(&dir.join("sessions"))).into_iter().collect(),
            _ => Default::default(),
        };
        if let Some(bridge) = &self.bridge {
            bridge.shared.lock().unwrap().transcripts =
                self.agent_sessions.iter().filter_map(|(id, s)| Some((id.clone(), s.transcript(dir.as_ref()?)))).collect();
        }
        let terminals = &self.terminals;
        self.terminal_busy.retain(|id, _| terminals.contains_key(id));
        self.terminal_idle.retain(|id, _| terminals.contains_key(id));
        self.terminal_unread.retain(|id| terminals.contains_key(id));
        self.transcripts.retain(|id, _| terminals.contains_key(id));
        let made_for_claude: std::collections::HashSet<String> =
            self.all_elements().filter(|e| e.terminal.as_ref().is_some_and(|t| t.command.contains("claude"))).map(|e| e.id.clone()).collect();
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
            let states = self.terminals.keys().map(|id| (id.clone(), self.agent_state(id))).collect();
            bridge.shared.lock().unwrap().states = states;
        }
        self.log_agent_events();
        // what the chat queued for an agent that waited goes once it doesn't
        self.flush_chat_queue();
        // a rope to its IDE undone: the port stays in its shell's environment, for its claude and
        // any started there later, until the terminal restarts; said once, again after a re-rope.
        // Only drawings on screen, whose ropes `links` sees
        let links = self.links();
        let on_screen = |id: &str| self.scene.elements.iter().any(|e| e.id == id);
        let mut unroped = Vec::new();
        for (terminal, (ide, warned)) in self.terminal_ides.iter_mut().filter(|(t, (ide, _))| on_screen(t) && on_screen(ide)) {
            let roped = links.get(terminal).is_some_and(|l| l.contains(ide));
            if !roped && !*warned {
                unroped.push(terminal.clone());
            }
            *warned = !roped;
        }
        for terminal in unroped {
            let label =
                self.all_elements().find(|e| e.id == terminal).and_then(|e| e.terminal.as_ref()).map_or("Terminal".to_string(), |s| s.label().to_string());
            self.toast(format!("Reinicie o terminal \"{label}\" para o claude dele largar o IDE."));
        }
        let kept: Vec<String> = self.all_elements().filter(|e| e.terminal.is_some()).map(|e| e.id.clone()).collect();
        self.terminal_ides.retain(|t, _| kept.contains(t));
    }

    /// What changed since the last check, as events: a Claude Code session's word, a menu on
    /// screen, the bridge's questions. A program printing proves nothing, so Active is none.
    // ponytail: in memory only (200); keeping them on disk means deciding privacy and which
    // workspace sees which
    pub(crate) fn log_agent_events(&mut self) {
        let asked = self.bridge.as_ref().map(|b| std::mem::take(&mut b.shared.lock().unwrap().events)).unwrap_or_default();
        for (id, what) in asked {
            self.agent_event(&id, what);
        }
        // an unloaded one leaves the log quietly and comes back as iniciado
        let ids: Vec<String> = self.all_elements().filter(|e| e.terminal.as_ref().is_some_and(|t| !t.unloaded)).map(|e| e.id.clone()).collect();
        for id in &ids {
            let state = self.agent_state(id);
            let what = match (self.agent_seen.insert(id.clone(), state), state) {
                (None, AgentState::Ended) | (Some(AgentState::Working(_)), AgentState::Working(_)) | (Some(AgentState::Waiting), AgentState::Waiting) => None,
                (None, _) => Some("iniciado".to_string()),
                (Some(AgentState::Ended), AgentState::Ended) => None,
                // restarted
                (Some(AgentState::Ended), _) => Some("iniciado".to_string()),
                (_, AgentState::Working(_)) => Some("começou a trabalhar".into()),
                (_, AgentState::Waiting) => Some(match self.agent_sessions.get(id).and_then(|s| s.waiting_for.clone()) {
                    Some(why) => format!("precisa de você ({why})"),
                    None => "precisa de você".into(),
                }),
                (Some(AgentState::Working(_)), AgentState::Idle(_)) => Some("terminou".into()),
                (_, AgentState::Ended) => Some("encerrado".into()),
                _ => None,
            };
            if let Some(what) = what {
                self.agent_event(id, what);
            }
        }
        self.agent_seen.retain(|id, _| ids.contains(id));
    }

    pub(crate) fn agent_event(&mut self, id: &str, what: impl Into<String>) {
        let name = self.all_elements().find(|e| e.id == id).and_then(|e| e.terminal.as_ref()).map_or("Terminal".to_string(), |s| s.label().to_string());
        self.agent_events.push_back(AgentEvent { at: self.now, id: id.into(), name, what: what.into() });
        if self.agent_events.len() > 200 {
            self.agent_events.pop_front();
        }
    }

    /// ⌘1 chat, ⌘2 canvas, ⌘3 IDE, ⌘J the agents table, ⌘⇧A the next agent that needs you; they work
    /// even while a terminal has the keyboard. The enlarged IDE keeps ⌘J and ⌘⇧A for itself while
    /// it shows (the chat covers it without taking its keyboard). Off the Mac ⌘J is Ctrl+J, the
    /// shell's newline, so a focused terminal keeps it.
    pub(crate) fn screen_keys(&mut self, ctx: &Context) {
        let key = |modifiers, key| ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(modifiers, key)));
        let ide = self.ide_focus.is_some() && !self.chat_open;
        if key(Modifiers::COMMAND, Key::Num1) {
            self.show_chat();
        }
        if key(Modifiers::COMMAND, Key::Num2) {
            self.show_canvas();
        }
        if key(Modifiers::COMMAND, Key::Num3) {
            self.open_ide();
        }
        if !ide && (cfg!(target_os = "macos") || self.terminal_focus.is_none()) && key(Modifiers::COMMAND, Key::J) {
            self.agents_open = !self.agents_open;
        }
        if !ide && key(Modifiers::COMMAND | Modifiers::SHIFT, Key::A) {
            self.next_needing_attention();
        }
    }

    /// ⌘1: the chat screen.
    pub(crate) fn show_chat(&mut self) {
        self.chat_open = true;
        self.terminal_focus = None;
    }

    /// ⌘2: the canvas itself; an enlarged IDE goes back to its card.
    pub(crate) fn show_canvas(&mut self) {
        self.chat_open = false;
        if self.terminal_elevated.as_ref().is_some_and(|id| self.scene.elements.iter().any(|e| &e.id == id && e.ide.is_some())) {
            self.set_elevated(None);
        }
    }

    /// Waiting ones first, then the ones that finished unseen; after the current one, round.
    fn next_needing_attention(&mut self) {
        let rows = self.agent_rows();
        let waiting = rows.iter().filter(|r| matches!(r.state, AgentState::Waiting));
        let unread = rows.iter().filter(|r| self.terminal_unread.contains(&r.id));
        let queue: Vec<String> = waiting.chain(unread).map(|r| r.id.clone()).collect();
        let current = if self.chat_open { self.chat_agent.clone() } else { self.terminal_focus.clone() };
        let after = current.and_then(|id| queue.iter().position(|q| *q == id)).map_or(0, |p| p + 1);
        let Some(next) = queue.get(after % queue.len().max(1)).cloned() else {
            self.toast("Nenhum agente precisa de você");
            return;
        };
        self.open_agent(&next);
    }

    /// The chat's button beside the menu (its list holds the agents) and the agents table (⌘J).
    /// Off the Mac ☰ is in the title bar, and the button with it.
    pub(crate) fn agents_panel(&mut self, ctx: &Context, c: &UiColors) {
        // off the Mac it sits in the title bar; the chat keeps the top to itself (⌘J still opens the table over it)
        let tip = "Agentes e chat — ⌘1 (canvas: ⌘2, IDE: ⌘3)";
        if TITLE_BAR == 0.0
            && !self.chat_open
            && egui::Area::new(egui::Id::unique("chat_button"))
                .fixed_pos((MENU_LEFT + 44.0, TOP_ROW + 4.0))
                .show(ctx, |ui| icon_button(ui, 36.0, false, c.surface_low, Icon::Composer, c.text, c).on_hover_text(keys_label(tip)))
                .inner
                .clicked()
        {
            self.chat_open = true;
        }
        if !self.agents_open {
            return;
        }
        let rows = self.agent_rows();
        let count = |f: fn(&AgentState) -> bool| rows.iter().filter(|r| f(&r.state)).count();
        let unread = rows.iter().filter(|r| self.terminal_unread.contains(&r.id)).count();
        let mut summary = vec![format!("{} trabalhando", count(|s| matches!(s, AgentState::Working(_))))];
        match count(|s| matches!(s, AgentState::Active(_))) {
            0 => {}
            1 => summary.push("1 ativo".into()),
            n => summary.push(format!("{n} ativos")),
        }
        summary.push(format!("{} ociosos", count(|s| matches!(s, AgentState::Idle(_)))));
        if unread > 0 {
            summary.push(format!("{unread} sem leitura"));
        }
        match count(|s| matches!(s, AgentState::Waiting)) {
            0 => {}
            1 => summary.push("1 precisa de você".into()),
            n => summary.push(format!("{n} precisam de você")),
        }
        let mut open = None;
        egui::Area::new(egui::Id::unique("agents_panel")).order(egui::Order::Foreground).anchor(egui::Align2::RIGHT_TOP, (-16.0, TOP_ROW + 52.0)).show(
            ctx,
            |ui| {
                self.island().corner_radius(12).inner_margin(14).show(ui, |ui| {
                    ui.set_width(640.0);
                    ui.label(egui::RichText::new(summary.join(" · ")).size(14.0).strong().color(c.text));
                    ui.add_space(8.0);
                    if rows.is_empty() {
                        ui.label(egui::RichText::new("Nenhum agente no canvas: crie um terminal e rode claude, codex…").size(12.0).color(c.muted));
                        return;
                    }
                    egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.65).show(ui, |ui| {
                        egui::Grid::new("agents_table").num_columns(4).spacing([16.0, 12.0]).min_col_width(110.0).show(ui, |ui| {
                            for title in ["Agente", "Estado", "Missão", "Último evento"] {
                                ui.label(egui::RichText::new(title).size(11.0).color(c.muted));
                            }
                            ui.end_row();
                            for row in &rows {
                                let fresh = self.terminal_unread.contains(&row.id);
                                let who = ui
                                    .vertical(|ui| {
                                        ui.set_max_width(170.0);
                                        ui.label(egui::RichText::new(row.spec.label()).size(13.5).strong().color(if fresh { PRIMARY } else { c.text }));
                                        let what = if row.spec.command.trim().is_empty() { "Shell" } else { row.spec.command.trim() };
                                        let space = row.space.as_ref().map(|s| format!("{s} · ")).unwrap_or_default();
                                        let under = egui::RichText::new(format!("{space}{what} · {}", display_folder(&row.spec.cwd))).size(11.0).color(c.muted);
                                        ui.add(egui::Label::new(under).truncate());
                                    })
                                    .response;
                                let when = self.state_note(row);
                                let (color, _) = row.state.dot(c);
                                let status = ui
                                    .vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                                            row.state.paint_dot(ui.painter(), dot.center(), c);
                                            let label = ui.label(egui::RichText::new(row.state.label()).size(12.5).color(color));
                                            if matches!(row.state, AgentState::Active(_)) {
                                                label.on_hover_text("Pela saída do terminal: este programa não diz se está trabalhando");
                                            }
                                        });
                                        if let Some(when) = when {
                                            ui.label(egui::RichText::new(when).size(11.0).color(c.muted));
                                        }
                                    })
                                    .response;
                                let text = row.mission.clone().unwrap_or_else(|| "—".into());
                                let task = ui.add_sized([170.0, 18.0], egui::Label::new(egui::RichText::new(text).size(12.0).color(c.muted)).truncate());
                                let last = self
                                    .agent_events
                                    .iter()
                                    .rev()
                                    .find(|e| e.id == row.id)
                                    .map_or("—".into(), |e| format!("{} · {}", e.what, ago(self.now - e.at)));
                                let event = ui.add_sized([140.0, 18.0], egui::Label::new(egui::RichText::new(last).size(12.0).color(c.muted)).truncate());
                                ui.end_row();
                                let whole = who.rect.union(status.rect).union(task.rect).union(event.rect);
                                if ui.interact(whole, ui.scope_id().with(&row.id), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    open = Some(row.id.clone());
                                }
                            }
                        });
                    });
                    if !self.agent_events.is_empty() {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("Atividade").size(11.0).color(c.muted));
                        for e in self.agent_events.iter().rev().take(8) {
                            let line = egui::RichText::new(format!("{} · {} {}", ago(self.now - e.at), e.name, e.what)).size(12.0).color(c.text);
                            ui.add(egui::Label::new(line).truncate());
                        }
                    }
                });
            },
        );
        if let Some(id) = open {
            self.open_agent(&id);
        }
    }

    /// In the chat, shows the agent's conversation; on the canvas, brings its terminal to the
    /// middle of the screen, selected and taking keys.
    pub(crate) fn open_agent(&mut self, id: &str) {
        if self.chat_open {
            self.terminal_unread.remove(id);
            self.chat_agent = Some(id.to_string());
            return;
        }
        // in a workspace not on screen: that workspace comes up first
        if let Some(space) = self.space_holding(id) {
            self.switch_space(&space);
        }
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return };
        self.glide_to(element_bounds(&self.scene.elements[i]).center());
        self.selected = vec![i];
        self.terminal_focus = Some(id.to_string());
        self.terminal_unread.remove(id);
    }
}
