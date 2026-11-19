//! Ajustes › Navegação e editor: what the wheel does, the zoom's step, and the File Tree's ligatures and hidden files.

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_navigation(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "NAVEGAÇÃO E EDITOR");
        let now = if self.prefs.wheel_auto { 0 } else { 1 + usize::from(self.prefs.mouse_wheel_zooms) };
        let hint = "Automático: a roda de um mouse dá zoom e o touchpad move o canvas.";

        if let Some(i) = row(ui, k, "Dispositivo", hint, |ui| choice(ui, k, "Dispositivo", &["Automático", "Trackpad", "Mouse"], now)) {
            self.prefs.wheel_auto = i == 0;
            self.prefs.mouse_wheel_zooms = i == 2;
        }

        let shown = format!("{}%", self.prefs.zoom_step);

        row(ui, k, "Passo do zoom", "O quanto cada passo da roda muda o zoom.", |ui| {
            stepper(ui, k, "Passo do zoom", &mut self.prefs.zoom_step, 2..=50, &shown)
        });

        toggle(ui, k, "Ligaduras no código", "=>, != e :: como um só sinal na Árvore de Arquivos.", &mut self.prefs.ligatures);
        let hint = "Com os ocultos escondidos, a Árvore de Arquivos ainda mostra estes (separados por vírgula).";

        row(ui, k, "Ocultos que aparecem", hint, |ui| {
            input(ui, k, egui::Id::unique("hidden_shown"), "Ocultos que aparecem", &mut self.prefs.hidden_shown, ".claude, .idea", 220.0, false)
        });
    }
}
