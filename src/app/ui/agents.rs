//! The agents panel: every terminal at a glance (working, idle, waiting for you) like a team
//! board, kept open beside the canvas while you work; and the keys to move between chat,
//! canvas and whoever needs you.

use crate::prelude::*;

pub(crate) const AMBER: Color32 = Color32::from_rgb(0xf0, 0xa0, 0x20);
const GREEN: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x50);
const RED: Color32 = Color32::from_rgb(0xf8, 0x51, 0x49);

/// The agent stopped on a menu only its terminal answers: Claude Code's permission prompt
/// (numbered options under a question) or any "enter to confirm / esc to cancel" footer. A
/// screen with the free-typing box ("? for shortcuts") is never waiting, whatever it asked.
pub(crate) fn asks_for_you(screen: &str) -> bool {
    let tail: Vec<String> = screen.lines().rev().filter(|l| !l.trim().is_empty()).take(14).map(str::to_lowercase).collect();
    let typing = tail.iter().any(|l| l.contains("? for shortcuts") || l.trim_start().starts_with("│ >"));
    let footer = tail.iter().any(|l| l.contains("esc to cancel") || l.contains("enter to confirm"));
    let options = tail
        .iter()
        .filter(|l| {
            let option = l.trim_start().trim_start_matches(['❯', '›', '>']).trim_start();
            option.chars().next().is_some_and(|c| c.is_ascii_digit()) && option.get(1..3) == Some(". ")
        })
        .count();
    !typing && (footer || (options >= 2 && tail.iter().any(|l| l.contains('?'))))
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

pub(crate) enum AgentState {
    Working(f64),
    Waiting,
    Idle(Option<f64>),
    Ended,
}

impl AgentState {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            AgentState::Working(_) => "Trabalhando",
            AgentState::Waiting => "Precisa de você",
            AgentState::Idle(_) => "Ocioso",
            AgentState::Ended => "Encerrado",
        }
    }

    /// The status dot: its color and whether it's filled.
    pub(crate) fn dot(&self, c: &UiColors) -> (Color32, bool) {
        match self {
            AgentState::Working(_) => (GREEN, true),
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
}

impl DrawingApp {
    /// Every terminal on the canvas, in scene order.
    pub(crate) fn agent_rows(&self) -> Vec<AgentRow> {
        self.scene
            .elements
            .iter()
            .filter_map(|e| {
                let spec = e.terminal.clone()?;
                let session = self.terminals.get(&e.id).and_then(|s| s.as_ref().ok()).filter(|s| !s.exited());
                let state = match session {
                    None => AgentState::Ended,
                    Some(s) if s.handle.busy() => AgentState::Working(self.terminal_busy.get(&e.id).copied().unwrap_or(self.now)),
                    Some(_) if self.terminal_waiting.contains(&e.id) => AgentState::Waiting,
                    Some(_) => AgentState::Idle(self.terminal_idle.get(&e.id).copied()),
                };
                // a shell's own "user@host:~" title says nothing about the task
                let mission = session.and_then(|s| s.title()).filter(|t| !(t.contains('@') && t.contains(':')));
                Some(AgentRow { id: e.id.clone(), spec, state, mission })
            })
            .collect()
    }

    /// Re-reads which terminals stopped on a question, twice a second at most.
    pub(crate) fn check_waiting(&mut self) {
        if self.now - self.waiting_checked < 0.5 {
            return;
        }
        self.waiting_checked = self.now;
        self.terminal_waiting = self
            .terminals
            .iter()
            .filter_map(|(id, s)| Some((id, s.as_ref().ok()?)))
            .filter(|(_, s)| !s.exited() && !s.handle.busy() && asks_for_you(&s.handle.screen_text()))
            .map(|(id, _)| id.clone())
            .collect();
    }

    /// ⌘1 chat, ⌘2 canvas, ⌘J the agents table, ⌘⇧A the next agent that needs you; they work
    /// even while a terminal has the keyboard. The enlarged IDE keeps ⌘J and ⌘⇧A for itself.
    pub(crate) fn screen_keys(&mut self, ctx: &Context) {
        let key = |modifiers, key| ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(modifiers, key)));
        let ide = self.ide_focus.is_some();
        if key(Modifiers::COMMAND, Key::Num1) {
            self.chat_open = true;
            self.terminal_focus = None;
        }
        if key(Modifiers::COMMAND, Key::Num2) {
            self.chat_open = false;
        }
        if !ide && key(Modifiers::COMMAND, Key::J) {
            self.agents_open = !self.agents_open;
        }
        if !ide && key(Modifiers::COMMAND | Modifiers::SHIFT, Key::A) {
            self.next_needing_attention();
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

    /// The buttons beside the menu (agents table, chat) and the table itself.
    pub(crate) fn agents_panel(&mut self, ctx: &Context, c: &UiColors) {
        let button = |x: f32, icon: Icon, on: bool, tip: &str| {
            egui::Area::new(egui::Id::unique(tip))
                .fixed_pos((MENU_LEFT + x, TOP_ROW + 4.0))
                .show(ctx, |ui| icon_button(ui, 36.0, on, c.surface_low, icon, c.text, c).on_hover_text(tip))
                .inner
                .clicked()
        };
        if button(44.0, Icon::Agents, self.agents_open, "Agentes — ⌘J") {
            self.agents_open = !self.agents_open;
        }
        if button(88.0, Icon::Composer, self.chat_open, "Chat — ⌘1 (canvas: ⌘2)") {
            self.chat_open = !self.chat_open;
        }
        self.agents_rect = None;
        if !self.agents_open {
            return;
        }
        let rows = self.agent_rows();
        let count = |f: fn(&AgentState) -> bool| rows.iter().filter(|r| f(&r.state)).count();
        let unread = rows.iter().filter(|r| self.terminal_unread.contains(&r.id)).count();
        let mut summary = vec![
            format!("{} trabalhando", count(|s| matches!(s, AgentState::Working(_)))),
            format!("{} ociosos", count(|s| matches!(s, AgentState::Idle(_)))),
        ];
        if unread > 0 {
            summary.push(format!("{unread} sem leitura"));
        }
        match count(|s| matches!(s, AgentState::Waiting)) {
            0 => {}
            1 => summary.push("1 precisa de você".into()),
            n => summary.push(format!("{n} precisam de você")),
        }
        let mut open = None;
        let area = egui::Area::new(egui::Id::unique("agents_panel")).order(egui::Order::Foreground).anchor(egui::Align2::RIGHT_TOP, (-16.0, TOP_ROW + 52.0)).show(ctx, |ui| {
            self.island().corner_radius(12).inner_margin(14).show(ui, |ui| {
                ui.set_width(480.0);
                ui.label(egui::RichText::new(summary.join(" · ")).size(14.0).strong().color(c.text));
                ui.add_space(8.0);
                if rows.is_empty() {
                    ui.label(egui::RichText::new("Nenhum agente no canvas: crie um terminal e rode claude, codex…").size(12.0).color(c.muted));
                    return;
                }
                egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.65).show(ui, |ui| {
                    egui::Grid::new("agents_table").num_columns(3).spacing([16.0, 12.0]).min_col_width(120.0).show(ui, |ui| {
                        for title in ["Agente", "Estado", "Missão"] {
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
                                    let under = egui::RichText::new(format!("{what} · {}", display_folder(&row.spec.cwd))).size(11.0).color(c.muted);
                                    ui.add(egui::Label::new(under).truncate());
                                })
                                .response;
                            let when = match row.state {
                                AgentState::Working(since) => Some(ago(self.now - since)),
                                AgentState::Idle(since) => since.map(|s| ago(self.now - s)),
                                _ => None,
                            };
                            let (color, _) = row.state.dot(c);
                            let status = ui
                                .vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                                        row.state.paint_dot(ui.painter(), dot.center(), c);
                                        ui.label(egui::RichText::new(row.state.label()).size(12.5).color(color));
                                    });
                                    if let Some(when) = when {
                                        ui.label(egui::RichText::new(when).size(11.0).color(c.muted));
                                    }
                                })
                                .response;
                            let text = row.mission.clone().unwrap_or_else(|| "—".into());
                            let task = ui.add_sized([170.0, 18.0], egui::Label::new(egui::RichText::new(text).size(12.0).color(c.muted)).truncate());
                            ui.end_row();
                            let whole = who.rect.union(status.rect).union(task.rect);
                            if ui.interact(whole, ui.scope_id().with(&row.id), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                open = Some(row.id.clone());
                            }
                        }
                    });
                });
            });
        });
        self.agents_rect = Some(area.response.rect);
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
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return };
        self.pan_to(element_bounds(&self.scene.elements[i]).center());
        self.selected = vec![i];
        self.terminal_focus = Some(id.to_string());
        self.terminal_unread.remove(id);
    }
}
