//! Ajustes › Geral: Aparência and Início. The island, the canvas and the navigation have pages of their own.

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_general(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "APARÊNCIA");

        let now = match self.prefs.theme {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        };

        if let Some(i) = row(ui, k, "Tema", "Sistema segue o do seu sistema operacional.", |ui| choice(ui, k, "Tema", &["Sistema", "Claro", "Escuro"], now)) {
            self.prefs.theme = [Theme::System, Theme::Light, Theme::Dark][i];
        }

        let hint = "A Luna fala no idioma em que você fala com ela; aqui você fixa um.";

        if let Some(i) = row(ui, k, "Idioma", hint, |ui| select(ui, k, "Idioma", &super::LANGUAGES, self.prefs.language)) {
            self.prefs.language = i;
        }

        let now = FRAME_RATES.iter().position(|rate| *rate == self.prefs.frame_rate).unwrap_or(1);
        let hint = "Da ilha e das animações. Mais fica mais suave e gasta mais processador; acima da taxa da sua tela não muda nada.";
        let rates = FRAME_RATES.map(|rate| rate.to_string());
        let rates: Vec<&str> = rates.iter().map(String::as_str).collect();

        if let Some(i) = row(ui, k, "Quadros por segundo", hint, |ui| choice(ui, k, "Quadros por segundo", &rates, now)) {
            self.prefs.frame_rate = FRAME_RATES[i];
        }

        title(ui, k, "INÍCIO");
        let now = usize::from(self.prefs.start_in_chat);

        if let Some(i) = row(ui, k, "Abrir o app em", "", |ui| choice(ui, k, "Abrir o app em", &["Canvas", "Chat"], now)) {
            self.prefs.start_in_chat = i == 1;
        }

        let was_enabled = self.prefs.run_at_login;
        let system = crate::app::system::run_at_login::system_name();
        let (label, hint) = (format!("Iniciar com o {system}"), format!("Abre o MagoAgent ao entrar na sua conta do {system}."));

        toggle(ui, k, &label, &hint, &mut self.prefs.run_at_login);

        if self.prefs.run_at_login != was_enabled
            && let Err(error) = crate::app::system::run_at_login::set_run_at_login(self.prefs.run_at_login)
        {
            self.prefs.run_at_login = was_enabled;
            self.toast(error);
        }

        let hint = "Fechar a janela deixa o MagoAgent rodando, com os terminais e os agentes. Ele volta pelo ícone perto do relógio; Ctrl+Q sai de verdade.";

        toggle(ui, k, "Fechar mantém em segundo plano", hint, &mut self.prefs.close_to_background);
        let hint = "O passo a passo que mostra onde fica cada parte do app.";

        if row(ui, k, "Apresentação", hint, |ui| button(ui, k, "Ver a apresentação de novo")).clicked() {
            self.start_tour();
        }
    }
}
