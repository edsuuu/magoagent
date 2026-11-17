//! Tarefas: the tasks of tarefas.json (app/tasks/), a field to make one for an agent, Abertas and
//! Feitas, and a box that concludes a task (or reopens it).

use super::{card, chip, live};
use crate::app::tasks::TaskState;
use crate::prelude::*;

/// The agent's list and the Criar button, beside the field.
const OWNER_WIDTH: f32 = 100.0;
const BUTTON_WIDTH: f32 = 52.0;

/// The task's box: 16 round-cornered, the accent with a check when concluded (egui's own is too
/// faint on the panel's ground).
fn check_box(ui: &mut egui::Ui, dark: bool, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, ""));
    let square = Rect::from_center_size(rect.center(), Vec2::splat(16.0));
    let p = ui.painter();
    if on {
        p.rect_filled(square, 4, glass_accent(dark));
        paint_icon_scaled(p, Icon::Check, square.center(), Color32::WHITE, 0.8, 0.0);
    } else {
        let ink = glass_ink(dark).gamma_multiply(if response.hovered() { 0.85 } else { 0.55 });
        p.rect_stroke(square, 4, Stroke::new(1.25, ink), StrokeKind::Inside);
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

impl DrawingApp {
    pub(crate) fn tasks_pane(&mut self, ui: &mut egui::Ui) {
        let (colors, dark) = (self.colors(), self.dark());
        let rows = self.agent_rows();
        let mut create = false;
        // right to left: the button, the agent's list, and the field in what is left
        let row = Vec2::new(ui.available_width(), 28.0);
        ui.allocate_ui_with_layout(row, egui::Layout::right_to_left(egui::Align::Center), |ui| {
            create |= ui.add(egui::Button::new("Criar").min_size(Vec2::new(BUTTON_WIDTH, 28.0))).on_hover_text("Criar a tarefa").clicked();
            let owner = rows.iter().find(|r| r.id == self.panes.task_owner).map_or("Sem agente".to_string(), |r| r.spec.label().to_string());
            egui::ComboBox::from_id_salt("pane_task_owner").width(OWNER_WIDTH).selected_text(owner).truncate().show_ui(ui, |ui| {
                ui.selectable_value(&mut self.panes.task_owner, String::new(), "Sem agente");
                for row in &rows {
                    ui.selectable_value(&mut self.panes.task_owner, row.id.clone(), row.spec.label());
                }
            });
            let field = egui::TextEdit::singleline(&mut self.panes.task_draft).hint_text("Nova tarefa").background_color(colors.surface_low);
            let field = ui.add_sized(Vec2::new(ui.available_width(), 28.0), field);
            create |= field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        });
        if create && live(ui) {
            self.pane_new_task();
        }
        let open = self.open_task_count();
        let done = self.tasks.list.len() - open;
        ui.horizontal(|ui| {
            if chip(ui, !self.panes.tasks_done, format!("Abertas {open}")) {
                self.panes.tasks_done = false;
            }
            if chip(ui, self.panes.tasks_done, format!("Feitas {done}")) {
                self.panes.tasks_done = true;
            }
        });
        let mut shown: Vec<_> = self.tasks.list.iter().filter(|t| t.state.is_final() == self.panes.tasks_done).collect();
        shown.sort_by_key(|t| (t.state.rank(), -t.updated));
        if shown.is_empty() {
            let empty = if self.panes.tasks_done { "Nenhuma tarefa feita." } else { "Nenhuma tarefa aberta." };
            ui.label(egui::RichText::new(empty).color(colors.muted));
        }
        let mut checked = Vec::new();
        for task in shown {
            let owner = self.task_owner(task);
            card(ui, dark, |ui| {
                ui.horizontal_top(|ui| {
                    let concluded = task.state == TaskState::Done;
                    if check_box(ui, dark, concluded).on_hover_text(if concluded { "Reabrir" } else { "Concluir" }).clicked() {
                        checked.push((task.id.clone(), !concluded));
                    }
                    ui.vertical(|ui| {
                        let title = egui::RichText::new(&task.title);
                        ui.add(egui::Label::new(if task.state.is_final() { title.strikethrough().color(colors.muted) } else { title }).wrap());
                        ui.label(egui::RichText::new(format!("{owner} · {} · {}", task.state.label(), task.id)).size(12.0).color(colors.muted));
                    });
                });
            });
        }
        for (id, concluded) in checked.into_iter().filter(|_| live(ui)) {
            if let Err(why) = self.task_update(&json!({ "id": id, "state": if concluded { "done" } else { "pending" } })) {
                self.toast(why);
            }
        }
    }

    /// The field's task, asked by the person, for the agent picked (none: nobody yet).
    pub(crate) fn pane_new_task(&mut self) {
        let title = self.panes.task_draft.trim().to_string();
        if title.is_empty() {
            return;
        }
        match self.task_create(&json!({ "title": title, "owner": self.panes.task_owner }), "você") {
            Ok(_) => {
                self.panes.task_draft.clear();
                self.panes.tasks_done = false;
            }
            Err(why) => self.toast(why),
        }
    }
}
