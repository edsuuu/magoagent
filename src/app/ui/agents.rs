//! The agents panel: every terminal at a glance (working, idle, waiting for you) like a team
//! board, kept open beside the canvas while you work.

use crate::prelude::*;

/// A confirmation or permission prompt at the bottom of the screen: the agent waits for you.
pub(crate) fn asks_for_you(screen: &str) -> bool {
    const PROMPTS: [&str; 5] = ["Do you want", "Would you like", "Esc to cancel", "Enter to confirm", "Allow command"];
    screen.lines().rev().filter(|l| !l.trim().is_empty()).take(12).any(|l| PROMPTS.iter().any(|p| l.contains(p)))
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

enum State {
    Working(f64),
    Waiting,
    Idle(Option<f64>),
    Ended,
}

impl DrawingApp {
    /// The button beside the menu and ⌘J (even while typing in a terminal) open the table.
    pub(crate) fn agents_panel(&mut self, ctx: &Context, c: &UiColors) {
        let toggle = egui::Area::new(egui::Id::unique("agents_button"))
            .fixed_pos((MENU_LEFT + 44.0, TOP_ROW + 4.0))
            .show(ctx, |ui| {
                let response = square_button(ui, 36.0, self.agents_open, c.surface_low, c);
                paint_icon(ui.painter(), Icon::Agents, response.rect.center(), c.text);
                response.on_hover_text("Agentes — ⌘J")
            })
            .inner;
        if toggle.clicked() || ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::J))) {
            self.agents_open = !self.agents_open;
        }
        self.agents_rect = None;
        if !self.agents_open {
            return;
        }
        // ponytail: reads each screen every frame the panel is open; cache it if there are dozens
        let rows: Vec<(String, TerminalSpec, State, Option<String>)> = self
            .scene
            .elements
            .iter()
            .filter_map(|e| {
                let spec = e.terminal.clone()?;
                let session = self.terminals.get(&e.id).and_then(|s| s.as_ref().ok()).filter(|s| !s.exited());
                let state = match session {
                    None => State::Ended,
                    Some(s) if s.handle.busy() => State::Working(self.terminal_busy.get(&e.id).copied().unwrap_or(self.now)),
                    Some(s) if asks_for_you(&s.handle.screen_text()) => State::Waiting,
                    Some(_) => State::Idle(self.terminal_idle.get(&e.id).copied()),
                };
                // a shell's own "user@host:~" title says nothing about the task
                let mission = session.and_then(|s| s.title()).filter(|t| !(t.contains('@') && t.contains(':')));
                Some((e.id.clone(), spec, state, mission))
            })
            .collect();
        let count = |f: fn(&State) -> bool| rows.iter().filter(|r| f(&r.2)).count();
        let unread = rows.iter().filter(|r| self.terminal_unread.contains(&r.0)).count();
        let mut summary = vec![format!("{} trabalhando", count(|s| matches!(s, State::Working(_)))), format!("{} ociosos", count(|s| matches!(s, State::Idle(_))))];
        if unread > 0 {
            summary.push(format!("{unread} sem leitura"));
        }
        match count(|s| matches!(s, State::Waiting)) {
            0 => {}
            1 => summary.push("1 precisa de você".into()),
            n => summary.push(format!("{n} precisam de você")),
        }
        let (green, amber, red) = (Color32::from_rgb(0x3f, 0xb9, 0x50), Color32::from_rgb(0xf0, 0xa0, 0x20), Color32::from_rgb(0xf8, 0x51, 0x49));
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
                        for (id, spec, state, mission) in &rows {
                            let fresh = self.terminal_unread.contains(id);
                            let who = ui
                                .vertical(|ui| {
                                    let name = if spec.name.is_empty() { "Terminal" } else { &spec.name };
                                    ui.label(egui::RichText::new(name).size(13.5).strong().color(if fresh { PRIMARY } else { c.text }));
                                    let what = if spec.command.trim().is_empty() { "Shell" } else { spec.command.trim() };
                                    ui.label(egui::RichText::new(format!("{what} · {}", display_folder(&spec.cwd))).size(11.0).color(c.muted));
                                })
                                .response;
                            let (label, when, color, filled) = match state {
                                State::Working(since) => ("Trabalhando", Some(ago(self.now - since)), green, true),
                                State::Waiting => ("Precisa de você", None, amber, true),
                                State::Idle(since) => ("Ocioso", since.map(|s| ago(self.now - s)), c.muted, false),
                                State::Ended => ("Encerrado", None, red, true),
                            };
                            let status = ui
                                .vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                                        if filled {
                                            ui.painter().circle_filled(dot.center(), 4.0, color);
                                        } else {
                                            ui.painter().circle_stroke(dot.center(), 3.5, Stroke::new(1.2, color));
                                        }
                                        ui.label(egui::RichText::new(label).size(12.5).color(color));
                                    });
                                    if let Some(when) = when {
                                        ui.label(egui::RichText::new(when).size(11.0).color(c.muted));
                                    }
                                })
                                .response;
                            // what the agent says it's doing: Claude Code and others title their terminal
                            let text = mission.clone().unwrap_or_else(|| "—".into());
                            let task = ui.add_sized([170.0, 18.0], egui::Label::new(egui::RichText::new(text).size(12.0).color(c.muted)).truncate());
                            ui.end_row();
                            let row = who.rect.union(status.rect).union(task.rect);
                            if ui.interact(row, ui.scope_id().with(id), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                open = Some(id.clone());
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

    /// Brings an agent's terminal to the middle of the screen, selected and taking keys.
    pub(crate) fn open_agent(&mut self, id: &str) {
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return };
        self.pan_to(element_bounds(&self.scene.elements[i]).center());
        self.selected = vec![i];
        self.terminal_focus = Some(id.to_string());
        self.terminal_unread.remove(id);
    }
}
