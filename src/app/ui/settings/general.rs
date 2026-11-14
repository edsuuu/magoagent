//! Ajustes › Geral: Sistema (idioma, aparência, início) and Tela (canvas, navegação, editor).

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_general(&mut self, ui: &mut egui::Ui, k: &K) {
        match super::subtabs(ui, k, "general", &["Sistema", "Tela"]) {
            0 => self.settings_system(ui, k),
            _ => self.settings_screen(ui, k),
        }
    }

    fn settings_system(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "IDIOMA");
        let hint = "O Maestro fala no idioma em que você fala com ele; aqui você fixa um.";
        if let Some(i) = row(ui, k, "Idioma", hint, |ui| select(ui, k, "Idioma", &super::LANGUAGES, self.prefs.language)) {
            self.prefs.language = i;
        }
        divider(ui, k);
        title(ui, k, "APARÊNCIA");
        let hint = "Escolha a aparência do MagoAgent. Sistema segue a configuração do seu sistema operacional.";
        let now = match self.prefs.theme {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        };
        if let Some(i) = row(ui, k, "Aparência", hint, |ui| select(ui, k, "Aparência", &["Sistema", "Claro", "Escuro"], now)) {
            self.prefs.theme = [Theme::System, Theme::Light, Theme::Dark][i];
        }
        divider(ui, k);
        title(ui, k, "INÍCIO");
        let now = usize::from(self.prefs.start_in_chat);
        if let Some(i) = row(ui, k, "Abrir o app em", "", |ui| select(ui, k, "Abrir o app em", &["Canvas", "Chat"], now)) {
            self.prefs.start_in_chat = i == 1;
        }
        let hint = "Fechar a janela deixa o MagoAgent rodando, com os terminais e os agentes. Ele volta pelo ícone perto do relógio ou abrindo o app de novo; Ctrl+Q sai de verdade.";
        toggle(ui, k, "Fechar mantém em segundo plano", hint, &mut self.prefs.close_to_background);
        divider(ui, k);
        title(ui, k, "ILHA");
        let hint = "O Maestro numa pílula no topo da tela, por cima de todas as janelas: o resumo dos agentes, o que precisa de você, o chat novo e a voz. Arraste para outro lugar ou outro monitor.";
        toggle(ui, k, "Mostrar a ilha", hint, &mut self.prefs.island);
    }

    fn settings_screen(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "APARÊNCIA");
        let hint = "A grade quadriculada do canvas, de 20 px. Para encaixar o que você desenha e move nela, Ctrl+' (Alinhar à grade).";
        if let Some(i) = row(ui, k, "Fundo", hint, |ui| select(ui, k, "Fundo", &["Liso", "Grade"], usize::from(self.prefs.paper))) {
            self.prefs.paper = i == 1;
        }
        let now = usize::from(self.prefs.box_overlap);
        let hint = "Contido pega só o que cabe inteiro na caixa; Sobreposto, também o que ela toca.";
        if let Some(i) = row(ui, k, "Modo de seleção por caixa", hint, |ui| select(ui, k, "Modo de seleção por caixa", &["Contido", "Sobreposto"], now)) {
            self.prefs.box_overlap = i == 1;
        }
        let now = WindowOutline::ALL.iter().position(|outline| *outline == self.prefs.window_outline).unwrap_or(0);
        let hint = "A borda azul de um terminal, nota, navegador, chat ou árvore selecionado.";
        if let Some(picked) = row(ui, k, "Contorno da seleção", hint, |ui| select(ui, k, "Contorno da seleção", &WindowOutline::NAMES, now)) {
            self.prefs.window_outline = WindowOutline::ALL[picked];
        }
        toggle(ui, k, "Mostrar dicas", "Dicas sobre os botões e atalhos.", &mut self.prefs.show_hints);
        divider(ui, k);
        title(ui, k, "NAVEGAÇÃO");
        let now = if self.prefs.wheel_auto { 0 } else { 1 + usize::from(self.prefs.mouse_wheel_zooms) };
        let hint = "Automático: a roda de um mouse dá zoom e o touchpad move o canvas, conforme o que você usar.";
        if let Some(i) = row(ui, k, "Dispositivo de entrada", hint, |ui| select(ui, k, "Dispositivo de entrada", &["Automático", "Trackpad", "Mouse"], now)) {
            self.prefs.wheel_auto = i == 0;
            self.prefs.mouse_wheel_zooms = i == 2;
        }
        let hint = "O quanto cada passo da roda do mouse muda o zoom.";
        let shown = format!("{}%", self.prefs.zoom_step);
        row(ui, k, "Passo do zoom", hint, |ui| stepper(ui, k, "Passo do zoom", &mut self.prefs.zoom_step, 2..=50, &shown));
        divider(ui, k);
        title(ui, k, "EDITOR");
        toggle(ui, k, "Ligaduras no código", "=>, != e :: como um só sinal no código da Árvore de Arquivos.", &mut self.prefs.ligatures);
        let hint = "Com os arquivos ocultos escondidos, a Árvore de Arquivos ainda mostra estes (separados por vírgula).";
        row(ui, k, "Ocultos que aparecem", hint, |ui| {
            input(ui, k, egui::Id::unique("hidden_shown"), "Ocultos que aparecem", &mut self.prefs.hidden_shown, ".claude, .idea", 200.0, false)
        });
    }
}
