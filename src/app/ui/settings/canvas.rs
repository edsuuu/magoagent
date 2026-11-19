//! Ajustes › Canvas: the background, the box selection, the selected card's outline and the hints.

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_canvas(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "CANVAS");
        let hint = "A grade de 20 px. Para encaixar nela o que você desenha e move, Ctrl+' (Alinhar à grade).";

        if let Some(i) = row(ui, k, "Fundo", hint, |ui| choice(ui, k, "Fundo", &["Liso", "Grade"], usize::from(self.prefs.paper))) {
            self.prefs.paper = i == 1;
        }

        let now = usize::from(self.prefs.box_overlap);
        let hint = "Contido pega só o que cabe inteiro na caixa; Sobreposto, também o que ela toca.";

        if let Some(i) = row(ui, k, "Seleção por caixa", hint, |ui| choice(ui, k, "Seleção por caixa", &["Contido", "Sobreposto"], now)) {
            self.prefs.box_overlap = i == 1;
        }

        let now = WindowOutline::ALL.iter().position(|outline| *outline == self.prefs.window_outline).unwrap_or(0);
        let hint = "A borda azul de um terminal, nota, navegador, chat ou árvore selecionado.";

        if let Some(picked) = row(ui, k, "Contorno da seleção", hint, |ui| select(ui, k, "Contorno da seleção", &WindowOutline::NAMES, now)) {
            self.prefs.window_outline = WindowOutline::ALL[picked];
        }

        toggle(ui, k, "Dicas nos botões", "Dicas sobre os botões e atalhos.", &mut self.prefs.show_hints);
    }
}
