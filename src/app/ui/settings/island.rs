//! Ajustes › Ilha: where the island is, when it closes and shrinks, and, on GNOME, the extension that lets the Luna's
//! eyes follow the pointer on the whole screen.

use super::kit::*;
use crate::app::island::Place;
use crate::prelude::*;

/// The seconds the island's "Fechar sozinho" offers (the same as the island's own chips and `set_setting`).
pub(super) const CLOSE_AFTER: [u8; 4] = [5, 10, 15, 30];
/// How long the compact island waits before it shrinks to the notch, in seconds; 0 is never.
pub(super) const COMPACT_AFTER: [u16; 5] = [15, 30, 60, 300, 0];

impl DrawingApp {
    pub(super) fn settings_island(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "ILHA DO MAESTRO");
        let hint = "Uma pílula no topo da tela, por cima de tudo: os agentes, o que precisa de você, o chat e a voz.";

        toggle(ui, k, "Mostrar a ilha", hint, &mut self.prefs.island);
        // none chosen when it was let go somewhere else on the screen
        let now = self.island_place(ui.ctx()).and_then(|place| Place::ALL.iter().position(|each| *each == place)).unwrap_or(usize::MAX);
        let hint = "No topo, embaixo (abre para cima) ou numa lateral, onde fechada ela fica em pé. Solta perto de uma borda, a ilha também gruda nela.";

        if let Some(index) = row(ui, k, "Lugar da ilha", hint, |ui| choice(ui, k, "Lugar da ilha", &Place::NAMES, now)) {
            self.put_island(Place::ALL[index]);
        }

        let hint = "Sumiu ou ficou fora da tela? Volta a ilha ao meio do topo do monitor onde está a janela do MagoAgent.";

        if row(ui, k, "Trazer a ilha de volta", hint, |ui| button(ui, k, "Voltar ao lugar padrão")).clicked() {
            self.reset_island();
            self.toast("A ilha voltou ao topo da tela.");
        }

        let now = CLOSE_AFTER.iter().position(|seconds| *seconds == self.prefs.island_close_after).unwrap_or(2);
        let hint = "Quanto a ilha aberta espera depois que o mouse sai.";

        if let Some(i) = row(ui, k, "Fechar sozinho", hint, |ui| choice(ui, k, "Fechar sozinho", &["5 s", "10 s", "15 s", "30 s"], now)) {
            self.prefs.island_close_after = CLOSE_AFTER[i];
        }

        let now = COMPACT_AFTER.iter().position(|seconds| *seconds == self.prefs.island_compact_after).unwrap_or(2);
        let hint = "Quanto a ilha compacta (a pílula com a Luna, a música e os agentes) espera, depois que o mouse sai, até virar o notch pequeno.";
        let labels = ["15 s", "30 s", "1 min", "5 min", "Nunca"];

        if let Some(i) = row(ui, k, "Encolher para o notch", hint, |ui| choice(ui, k, "Encolher para o notch", &labels, now)) {
            self.prefs.island_compact_after = COMPACT_AFTER[i];
        }

        #[cfg(target_os = "linux")]
        if crate::app::island::gnome::wanted() {
            self.settings_gnome_cursor(ui, k);
        }
    }

    /// GNOME on Wayland: the extension that tells the island's eyes where the pointer is off the app's windows. On by
    /// default (put in at the start); the switch takes it out or puts it back.
    #[cfg(target_os = "linux")]
    fn settings_gnome_cursor(&mut self, ui: &mut egui::Ui, k: &K) {
        use crate::app::island::gnome::{Status, install, remove, status};

        let hint = match status() {
            Status::On => "Ativa: uma extensão pequena do GNOME diz onde o mouse está, e os olhos da Luna seguem ele na tela toda.",
            Status::Installed => "Instalada: o GNOME a carrega quando você sair e entrar de novo na sessão.",
            Status::Missing => "No GNOME o app só vê o mouse sobre as janelas dele. Ligado, uma extensão pequena do GNOME diz onde ele está.",
        };

        if toggle(ui, k, "Olhos seguem o mouse na tela toda", hint, &mut self.prefs.gnome_cursor) {
            let done = if self.prefs.gnome_cursor { install() } else { remove() };

            self.toast(done.unwrap_or_else(|error| error));
        }
    }
}
