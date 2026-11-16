//! Ajustes › Geral: one page with Aparência, Início, Ilha do Maestro, Canvas and Navegação e editor.

use super::kit::*;
use crate::app::island::Place;
use crate::prelude::*;

/// The seconds the island's "Fechar sozinho" offers (the same as the island's own chips and `set_setting`).
const CLOSE_AFTER: [u8; 4] = [5, 10, 15, 30];

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
        let hint = "O Maestro fala no idioma em que você fala com ele; aqui você fixa um.";
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
        if cfg!(windows) {
            let was_enabled = self.prefs.run_at_login;
            toggle(ui, k, "Iniciar com o Windows", "Abre o MagoAgent ao entrar na sua conta do Windows.", &mut self.prefs.run_at_login);
            if self.prefs.run_at_login != was_enabled
                && let Err(error) = crate::app::system::run_at_login::set_run_at_login(self.prefs.run_at_login)
            {
                self.prefs.run_at_login = was_enabled;
                self.toast(error);
            }
        }
        let hint = "Fechar a janela deixa o MagoAgent rodando, com os terminais e os agentes. Ele volta pelo ícone perto do relógio; Ctrl+Q sai de verdade.";
        toggle(ui, k, "Fechar mantém em segundo plano", hint, &mut self.prefs.close_to_background);

        title(ui, k, "ILHA DO MAESTRO");
        let hint = "Uma pílula no topo da tela, por cima de tudo: os agentes, o que precisa de você, o chat e a voz.";
        toggle(ui, k, "Mostrar a ilha", hint, &mut self.prefs.island);
        // none chosen when it was let go somewhere else on the screen
        let now = self.island_place(ui.ctx()).and_then(|place| Place::ALL.iter().position(|each| *each == place)).unwrap_or(usize::MAX);
        let hint =
            "No topo ou embaixo (abre para cima). Solta perto do topo ou de baixo da tela, a ilha também gruda ali; em outro lugar, fica onde foi solta.";
        if let Some(index) = row(ui, k, "Lugar da ilha", hint, |ui| choice(ui, k, "Lugar da ilha", &Place::NAMES, now)) {
            self.put_island(Place::ALL[index]);
        }
        #[cfg(target_os = "linux")]
        if crate::app::island::gnome::wanted() {
            self.settings_gnome_cursor(ui, k);
        }
        let now = CLOSE_AFTER.iter().position(|seconds| *seconds == self.prefs.island_close_after).unwrap_or(2);
        let hint = "Quanto a ilha aberta espera depois que o mouse sai.";
        if let Some(i) = row(ui, k, "Fechar sozinho", hint, |ui| choice(ui, k, "Fechar sozinho", &["5 s", "10 s", "15 s", "30 s"], now)) {
            self.prefs.island_close_after = CLOSE_AFTER[i];
        }

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

    /// GNOME on Wayland: the extension that tells the island's eyes where the pointer is off the app's windows.
    #[cfg(target_os = "linux")]
    fn settings_gnome_cursor(&mut self, ui: &mut egui::Ui, k: &K) {
        use crate::app::island::gnome::{Status, install, remove, status};
        let now = status();
        let (hint, action) = match now {
            Status::On => ("Ativa: a extensão do GNOME diz onde o mouse está, e os olhos da ilha seguem ele na tela toda.", "Remover"),
            Status::Installed => ("Instalada: o GNOME a carrega quando você sair e entrar de novo na sessão.", "Remover"),
            Status::Missing => ("No GNOME o app só vê o mouse sobre as janelas dele. Uma extensão pequena do GNOME diz onde ele está.", "Instalar"),
        };
        if row(ui, k, "Olhos seguem o mouse na tela toda", hint, |ui| button(ui, k, action)).clicked() {
            let done = if now == Status::Missing { install() } else { remove() };
            self.toast(done.unwrap_or_else(|error| error));
        }
    }
}
