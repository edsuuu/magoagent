//! Ajustes › Sobre: a versão, a verificação de atualizações (updates.rs), o botão Atualizar e as
//! notas da última release (changelog).

use super::kit::*;
use crate::app::updates::{RELEASES_PAGE, UpdateState, plain_notes};
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_about(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "VERSÃO");
        let busy = matches!(self.updates.state, UpdateState::Checking | UpdateState::Downloading(_));
        row(ui, k, &format!("{APP_TITLE} {}", env!("CARGO_PKG_VERSION")), &self.updates.state_line(), |ui| {
            let available = matches!(self.updates.state, UpdateState::Available(_));
            if available && button(ui, k, "Atualizar").clicked() {
                if let Err(why) = self.update_install() {
                    self.toast(why);
                }
            } else if !available && !busy && button(ui, k, "Verificar atualizações").clicked() {
                self.update_check(false);
            }
        });
        let release = match &self.updates.state {
            UpdateState::Available(r) | UpdateState::UpToDate(r) | UpdateState::Downloading(r) | UpdateState::Started(r) => Some(r.clone()),
            _ => None,
        };
        if let Some(release) = release {
            if matches!(self.updates.state, UpdateState::Available(_)) && !cfg!(windows) {
                note(ui, k, "Neste sistema a atualização se baixa pela página da release: o botão a abre.");
            }
            divider(ui, k);
            title(ui, k, &format!("NOVIDADES DA {}", release.version));
            let notes = plain_notes(&release.notes);
            let text = if notes.is_empty() { "Essa release não traz notas." } else { notes.as_str() };
            let g = galley(ui, text, SMALL, SMALL_LINE, k.fg, ui.available_width());
            let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());
            ui.painter().galley(rect.min, g, k.fg);
            ui.add_space(6.5);
            if button(ui, k, "Abrir a página da release").clicked() {
                crate::app::portal::open_external(&release.page);
            }
        } else {
            ui.add_space(6.5);
            if button(ui, k, "Ver as releases no GitHub").clicked() {
                crate::app::portal::open_external(RELEASES_PAGE);
            }
        }
        divider(ui, k);
        note(
            ui,
            k,
            "Tudo roda neste computador: os agentes são o claude e o codex instalados, com o login deles. O app olha por uma versão nova ao abrir, no máximo uma vez por dia.",
        );
    }
}
