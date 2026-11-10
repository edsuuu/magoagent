//! The agents panel's Tarefas and Fichário tabs. The tasks grouped by state, what needs you on top:
//! made, moved and followed to their agent from here, each through the MCP's own `tasks` actions.
//! The binder's notes, read-only for now.

use crate::app::tasks::{STATES, Task, TaskState};
use crate::prelude::*;

pub(crate) const PANEL_TABS: [&str; 3] = ["Agentes", "Tarefas", "Fichário"];

/// A state's color on the panel: danger, warning and success as the agents' dots use them.
fn state_color(state: TaskState, c: &UiColors) -> Color32 {
    match state {
        TaskState::Blocked | TaskState::Failed => c.danger,
        TaskState::WaitingApproval => c.warning,
        TaskState::InProgress | TaskState::InReview | TaskState::InTest | TaskState::Done => c.success,
        _ => c.muted,
    }
}

impl DrawingApp {
    /// The panel on the Tarefas or Fichário tab (`tasks.tab`), where the agents table sits.
    pub(crate) fn tasks_panel(&mut self, ctx: &Context, c: &UiColors) {
        self.tasks_tick();
        let rows = self.agent_rows();
        let (mut tab, mut create, mut change, mut go) = (self.tasks.tab, false, None::<Value>, None::<String>);
        egui::Area::new(egui::Id::unique("agents_panel")).order(egui::Order::Foreground).anchor(egui::Align2::RIGHT_TOP, (-16.0, TOP_ROW + 52.0)).show(
            ctx,
            |ui| {
                self.island().corner_radius(12).inner_margin(14).show(ui, |ui| {
                    ui.set_width(640.0);
                    segmented(ui, &mut tab, &PANEL_TABS, c);
                    ui.add_space(10.0);
                    if tab == 2 {
                        self.binder_list(ui, c);
                        return;
                    }
                    // a new task: its title, who does it, whether it goes to them now
                    ui.horizontal(|ui| {
                        let field = egui::TextEdit::singleline(&mut self.tasks.title)
                            .hint_text("Nova tarefa…")
                            .desired_width(300.0)
                            .margin(egui::Margin::symmetric(10, 7))
                            .background_color(c.surface_low)
                            .font(FontId::proportional(13.5));
                        let typed = ui.add(field);
                        let owner = rows.iter().find(|r| r.id == self.tasks.owner).map_or("Sem responsável".to_string(), |r| r.spec.label().to_string());
                        let pick =
                            ui.add(egui::Button::new(egui::RichText::new(owner).size(12.5).color(c.text)).fill(c.surface_low).corner_radius(8).truncate());
                        egui::Popup::menu(&pick).show(|ui| {
                            ui.set_width(220.0);
                            if ui.selectable_label(self.tasks.owner.is_empty(), "Sem responsável").clicked() {
                                self.tasks.owner.clear();
                            }
                            for row in &rows {
                                if ui.selectable_label(self.tasks.owner == row.id, row.spec.label()).clicked() {
                                    self.tasks.owner = row.id.clone();
                                }
                            }
                        });
                        if !self.tasks.owner.is_empty() {
                            dialog_check(ui, &mut self.tasks.send, "Entregar", c)
                                .on_hover_text("Manda o texto da tarefa ao terminal ou ao chat do responsável");
                        }
                        let ready = !self.tasks.title.trim().is_empty();
                        let button = egui::Button::new(egui::RichText::new("Criar").size(12.5).color(Color32::WHITE)).fill(PRIMARY).corner_radius(8);
                        create = ui.add_enabled(ready, button).clicked() || (ready && typed.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                    });
                    ui.add_space(8.0);
                    if self.tasks.list.is_empty() {
                        ui.label(egui::RichText::new("Nenhuma tarefa: crie uma aqui ou peça ao Maestro para delegar algo.").size(12.0).color(c.muted));
                        return;
                    }
                    egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.6).show(ui, |ui| {
                        for (state, _, label) in STATES {
                            let mut group: Vec<&Task> = self.tasks.list.iter().filter(|t| t.state == state).collect();
                            if group.is_empty() {
                                continue;
                            }
                            group.sort_by_key(|t| -t.updated);
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(format!("{label} · {}", group.len())).size(11.5).strong().color(state_color(state, c)));
                            for t in group {
                                if let Some(act) = self.task_row(ui, t, c, &mut go) {
                                    change = Some(act);
                                }
                            }
                        }
                    });
                });
            },
        );
        self.tasks.tab = tab;
        if create {
            let args = json!({ "title": self.tasks.title, "owner": self.tasks.owner, "send": self.tasks.send });
            match self.task_create(&args, "você") {
                Ok(said) => {
                    self.tasks.title.clear();
                    self.toast(said);
                }
                Err(why) => self.toast(why),
            }
        }
        if let Some(args) = change {
            let said = self.task_update(&args).unwrap_or_else(|why| why);
            self.toast(said);
        }
        if let Some(id) = go {
            if self.is_chat(&id) { self.chat_on(&id) } else { self.open_agent(&id) }
        }
    }

    /// One task: its id and title, why it's there and since when, its owner (a click goes to it)
    /// and its state (a click opens the states). The state picked comes back as `tasks update`'s args.
    fn task_row(&self, ui: &mut egui::Ui, t: &Task, c: &UiColors, go: &mut Option<String>) -> Option<Value> {
        let mut change = None;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(360.0);
                let title = egui::RichText::new(format!("{}  {}", t.id, t.title)).size(13.0).color(c.text);
                ui.add(egui::Label::new(title).truncate());
                let since = crate::app::ui::agents::ago((now_ms() - t.updated).max(0) as f64 / 1000.0);
                let under = if t.note.is_empty() { since } else { format!("{} · {since}", t.note) };
                ui.add(egui::Label::new(egui::RichText::new(under).size(11.0).color(c.muted)).truncate());
            });
            let owner = if t.owner.is_empty() { None } else { Some(self.agent_rows().into_iter().find(|r| r.id == t.owner)) };
            match owner {
                None => _ = ui.add_sized([130.0, 20.0], egui::Label::new(egui::RichText::new("Sem responsável").size(12.0).color(c.muted))),
                Some(row) => {
                    let name = row.as_ref().map_or(t.owner_name.clone(), |r| r.spec.label().to_string());
                    let link =
                        egui::Button::new(egui::RichText::new(name).size(12.0).color(if row.is_some() { PRIMARY } else { c.muted })).frame(false).truncate();
                    let link = ui.add_sized([130.0, 20.0], link);
                    if row.is_some() && link.on_hover_text("Ir ao agente").on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        *go = Some(t.owner.clone());
                    }
                }
            }
            let state = egui::Button::new(egui::RichText::new(t.state.label()).size(12.0).color(state_color(t.state, c))).fill(c.surface_low).corner_radius(8);
            let state = ui.add_sized([130.0, 24.0], state).on_hover_text("Mudar o estado");
            egui::Popup::menu(&state).show(|ui| {
                ui.set_width(180.0);
                for (to, code, label) in STATES {
                    // a final state only reopens: the others aren't offered
                    let offered = !t.state.is_final() || to == TaskState::Pending;
                    if to != t.state && ui.add_enabled(offered, egui::Button::selectable(false, label)).clicked() {
                        change = Some(json!({ "id": t.id, "state": code }));
                    }
                }
            });
        });
        change
    }

    /// The binder's notes (name, size, first line) and where they are; read again every 2 s.
    fn binder_list(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        let Some(dir) = self.autosave.as_ref().map(|p| p.with_file_name("fichario")) else { return };
        if self.tasks.notes.as_ref().is_none_or(|(at, _)| self.now - at > 2.0) {
            self.tasks.notes = Some((self.now, crate::app::binder::list(&dir)));
        }
        ui.label(egui::RichText::new(format!("Notas que o Maestro e os agentes guardam entre as sessões, em {}", dir.display())).size(11.5).color(c.muted));
        ui.add_space(8.0);
        let notes = self.tasks.notes.as_ref().map(|(_, n)| n.as_slice()).unwrap_or_default();
        if notes.is_empty() {
            ui.label(egui::RichText::new("O fichário está vazio: peça ao Maestro para anotar uma decisão.").size(12.0).color(c.muted));
            return;
        }
        egui::ScrollArea::vertical().max_height(ui.ctx().content_rect().height() * 0.6).show(ui, |ui| {
            for (name, size, first) in notes {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(name).size(13.0).strong().color(c.text));
                    ui.label(egui::RichText::new(format!("{:.1} KB", *size as f64 / 1024.0).replace('.', ",")).size(11.0).color(c.muted));
                });
                ui.add(egui::Label::new(egui::RichText::new(first).size(12.0).color(c.muted)).truncate());
                ui.add_space(4.0);
            }
        });
    }
}
