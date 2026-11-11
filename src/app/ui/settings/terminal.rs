//! Ajustes › Terminal: Início Rápido (the presets of the new-terminal dialog) and Avançado.

use super::kit::*;
use crate::prelude::*;

/// What a row of the presets list asked for, applied once the list is drawn.
enum Move {
    Up(usize),
    Down(usize),
    Remove(usize),
}

/// The name (0) or command (1) box of preset `i` while it is edited.
fn preset_field(i: usize, k: usize) -> egui::Id {
    egui::Id::unique(("preset_field", i, k))
}

impl DrawingApp {
    pub(super) fn settings_terminal(&mut self, ui: &mut egui::Ui, k: &K) {
        match super::subtabs(ui, k, "terminal", &["Início Rápido", "Avançado"]) {
            0 => self.settings_presets(ui, k),
            _ => self.settings_terminal_advanced(ui, k),
        }
    }

    fn settings_terminal_advanced(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "DESEMPENHO");
        let history = match self.prefs.terminal_history {
            1_000 => 0,
            5_000 => 1,
            _ => 2,
        };
        let hint = "Vale para os terminais abertos depois da mudança.";
        if let Some(i) = row(ui, k, "Linhas de histórico", hint, |ui| select(ui, k, "Linhas de histórico", &["1.000", "5.000", "10.000"], history)) {
            self.prefs.terminal_history = [1_000, 5_000, 10_000][i];
        }
    }

    fn settings_presets(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "PRESETS DE INÍCIO RÁPIDO");
        note(ui, k, "Até 10 presets ativos aparecem ao criar um terminal.");
        ui.add_space(9.75 - 3.25);
        let mut moved = None;
        let list = ui.available_width();
        let count = self.prefs.presets.len();
        // the list: a rounded box with a line between its rows
        let rows_top = ui.cursor().top();
        ui.spacing_mut().item_spacing.y = 0.0;
        for i in 0..count {
            let editing = self.settings_edit == Some(i);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(list, 48.25), Sense::hover());
            if i > 0 {
                ui.painter().hline(rect.x_range(), rect.top(), Stroke::new(1.0, k.line(0.5)));
            }
            let mut child =
                ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(9.75, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
            self.preset_row(&mut child, k, i, editing, &mut moved);
        }
        ui.spacing_mut().item_spacing.y = 3.25;
        let frame = Rect::from_min_max(Pos2::new(ui.min_rect().left(), rows_top), Pos2::new(ui.min_rect().left() + list, ui.cursor().top()));
        ui.painter().rect_stroke(frame, 8.125, Stroke::new(1.0, k.line(0.6)), StrokeKind::Inside);
        ui.add_space(9.75 - 3.25);
        let (mut add, mut reset) = (false, false);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.5;
            add = button(ui, k, "+  Adicionar Preset").clicked();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                reset = button(ui, k, "Redefinir para Padrão").clicked();
            });
        });
        match moved {
            Some(Move::Up(i)) if i > 0 => self.prefs.presets.swap(i, i - 1),
            Some(Move::Down(i)) if i + 1 < count => self.prefs.presets.swap(i, i + 1),
            Some(Move::Remove(i)) => {
                self.prefs.presets.remove(i);
                self.settings_edit = None;
            }
            _ => {}
        }
        if add {
            self.prefs.presets.push(Preset { name: String::new(), command: String::new(), on: true });
            self.settings_edit = Some(self.prefs.presets.len() - 1);
            ui.memory_mut(|m| m.request_focus(preset_field(self.prefs.presets.len() - 1, 0)));
        }
        if reset {
            self.prefs.presets = default_presets();
            self.settings_edit = None;
        }
    }

    /// One preset: the check, the mark, the name over the command, and ▲ ▼ edit ⊖ at the right.
    fn preset_row(&mut self, ui: &mut egui::Ui, k: &K, i: usize, editing: bool, moved: &mut Option<Move>) {
        let count = self.prefs.presets.len();
        ui.spacing_mut().item_spacing.x = 9.75;
        let preset = &mut self.prefs.presets[i];
        let name = if preset.name.trim().is_empty() { "sem nome".to_string() } else { preset.name.clone() };
        // the check: a filled circle when it shows in the dialog
        let (dot, check) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
        if preset.on {
            ui.painter().circle_filled(dot.center(), 9.0, k.primary);
            paint_icon(ui.painter(), Icon::Check, dot.center(), Color32::WHITE);
        } else {
            ui.painter().circle_stroke(dot.center(), 8.5, Stroke::new(1.2, k.muted_fg));
        }
        let on = preset.on;
        check.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, format!("Mostrar {name} no diálogo")));
        if check.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            preset.on = !preset.on;
        }
        let (mark, _) = ui.allocate_exact_size(Vec2::splat(16.25), Sense::hover());
        terminal_icon(ui.painter(), &TerminalSpec { command: preset.command.clone(), ..Default::default() }, mark.center(), k.fg);
        let buttons = 4.0 * 24.0 + 2.0 * 6.5;
        if editing {
            for (n, (text, hint, width)) in [(&mut preset.name, "Nome", 150.0), (&mut preset.command, "comando (vazio: shell)", 210.0)].into_iter().enumerate()
            {
                input(ui, k, preset_field(i, n), if n == 0 { "Nome do preset" } else { "Comando do preset" }, text, hint, width, false);
            }
        } else {
            let color = if preset.name.trim().is_empty() { k.muted_fg } else { k.fg };
            let width = ui.available_width() - buttons - 9.75;
            let head = galley(ui, &name, 13.0, 18.2, color, width);
            let command = if preset.command.is_empty() { "shell" } else { &preset.command };
            let command = galley(ui, command, 11.0, 15.1, k.muted_fg, width);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, head.size().y + command.size().y), Sense::hover());
            let below = rect.top() + head.size().y;
            ui.painter().galley(rect.min, head, color);
            ui.painter().galley(Pos2::new(rect.left(), below), command, k.muted_fg);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.5;
            if ghost_button(ui, k, Icon::Trash, &format!("Remover {name}"), true, true).clicked() {
                *moved = Some(Move::Remove(i));
            }
            let (icon, tip) = if editing { (Icon::Check, "Pronto") } else { (Icon::Edit, "Editar") };
            if ghost_button(ui, k, icon, &format!("{tip} {name}"), true, false).clicked() {
                self.settings_edit = if editing { None } else { Some(i) };
                // straight into the name, ready to type
                ui.memory_mut(|m| m.request_focus(preset_field(i, 0)));
            }
            if ghost_button(ui, k, Glyph::Chevron(false), &format!("Mover {name} para baixo"), i + 1 < count, false).clicked() {
                *moved = Some(Move::Down(i));
            }
            // the two chevrons side by side, as one control
            ui.add_space(-6.5);
            if ghost_button(ui, k, Glyph::Chevron(true), &format!("Mover {name} para cima"), i > 0, false).clicked() {
                *moved = Some(Move::Up(i));
            }
        });
    }
}
