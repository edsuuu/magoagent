//! Ajustes › Sobre: a versão, a verificação de atualizações (updates.rs), o botão Atualizar, as
//! notas da última release (changelog), o autor e a privacidade, que fica só aqui.

use super::kit::*;
use crate::app::system::updates::{CLOSES_AND_REOPENS, RELEASES_PAGE, UpdateState, installs_in_place, plain_notes};
use crate::prelude::*;

/// Who made the app, as on GitHub, and the page its button opens.
pub(crate) const AUTHOR: &str = "edsuuu";
pub(super) const AUTHOR_PAGE: &str = "https://github.com/edsuuu";

impl DrawingApp {
    pub(super) fn settings_about(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "VERSÃO");
        let busy = matches!(self.updates.state, UpdateState::Checking | UpdateState::Downloading(_));

        row(ui, k, &format!("{APP_TITLE} {}", env!("CARGO_PKG_VERSION")), &self.updates.state_line(), |ui| {
            let available = matches!(self.updates.state, UpdateState::Available(_));

            if available && button(ui, k, "Atualizar").clicked() {
                // an update that closes and reopens the app (Windows, the .deb on Linux): a second click, after the warning
                if matches!(&self.updates.state, UpdateState::Available(release) if installs_in_place(release)) {
                    self.updates.confirming = true;
                } else if let Err(why) = self.update_install() {
                    self.toast(why);
                }
            } else if !available && !busy && button(ui, k, "Verificar atualizações").clicked() {
                self.update_check(false);
            }
        });

        let hint = "Sempre: ao abrir o app e a cada hora. Ao abrir: só quando o app abre. Nunca: só por este botão.";
        let options = ["Sempre", "Ao abrir", "Nunca"];
        let chosen = usize::from(self.prefs.update_check.min(2));

        if let Some(index) = row(ui, k, "Procurar atualizações", hint, |ui| select(ui, k, "Procurar atualizações", &options, chosen)) {
            self.prefs.update_check = index as u8;
        }

        if self.updates.confirming && matches!(self.updates.state, UpdateState::Available(_)) {
            note(ui, k, CLOSES_AND_REOPENS);

            // every button in the card's padding, as the notes' one is: not stuck to its edge
            block(ui, k, |ui| {
                ui.horizontal(|ui| {
                    if button(ui, k, "Atualizar e reiniciar").clicked()
                        && let Err(why) = self.update_install()
                    {
                        self.toast(why);
                    }

                    if button(ui, k, "Cancelar").clicked() {
                        self.updates.confirming = false;
                    }
                });
            });
        }

        let release = match &self.updates.state {
            UpdateState::Available(r) | UpdateState::UpToDate(r) | UpdateState::Downloading(r) | UpdateState::Started(r) => Some(r.clone()),
            _ => None,
        };

        if let Some(release) = release {
            if matches!(&self.updates.state, UpdateState::Available(release) if !installs_in_place(release)) {
                note(ui, k, "Neste sistema a atualização se baixa pela página da release: o botão a abre.");
            }

            divider(ui, k);
            title(ui, k, &format!("NOVIDADES DA {}", release.version));
            let notes = plain_notes(&release.notes);
            let text = if notes.is_empty() { "Essa release não traz notas." } else { notes.as_str() };

            // in the card's padding, as the other blocks are: not stuck to its top edge
            block(ui, k, |ui| {
                let g = galley(ui, text, SMALL, SMALL_LINE, k.fg, ui.available_width());
                let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());

                ui.painter().galley(rect.min, g, k.fg);
                ui.add_space(6.5);

                if button(ui, k, "Abrir a página da release").clicked() {
                    crate::app::browser::open_external(&release.page);
                }
            });
        } else if block(ui, k, |ui| button(ui, k, "Ver as releases no GitHub").clicked()) {
            crate::app::browser::open_external(RELEASES_PAGE);
        }

        divider(ui, k);
        title(ui, k, "AUTOR");

        if row(ui, k, &format!("Criado por {AUTHOR}"), "github.com/edsuuu", |ui| button(ui, k, "Abrir no GitHub").clicked()) {
            crate::app::browser::open_external(AUTHOR_PAGE);
        }

        divider(ui, k);
        title(ui, k, "PRIVACIDADE");

        let rows = [
            ("O app", "Não envia nada por conta própria. Olha por uma versão nova no GitHub como Procurar atualizações manda."),
            ("Agentes", "O Claude Code e o Codex instalados falam com os serviços deles, com o login de cada um."),
            ("Navegadores", "Sites comuns, cada navegador com o seu perfil."),
            ("Microfone", "Só fica aberto durante a chamada; o que você diz vai ao motor de Ajustes › Voz (o VoiceStudio do seu computador ou a OpenAI)."),
        ];

        for (label, hint) in rows {
            row(ui, k, label, hint, |_| ());
        }

        note(ui, k, "Tudo roda neste computador: o desenho, os chats e os dados do app ficam na pasta de Ajustes › Dados.");
    }
}
