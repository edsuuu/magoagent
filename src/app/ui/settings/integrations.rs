use super::kit::*;
use crate::prelude::*;

const GITHUB_TOKEN_ID: &str = "github-token";

impl DrawingApp {
    pub(super) fn settings_integrations(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "GITHUB");

        let has_token = github_token().is_some();
        let connected = has_token && matches!(self.github.lock().unwrap().state, crate::app::integrations::github::GitHubState::Loaded(_));

        let status_color = if connected { Color32::from_rgb(52, 199, 89) } else { Color32::from_rgb(255, 59, 48) };
        let status_text = if connected { "Conectado" } else { "Não conectado" };

        row(ui, k, "Status", "", |ui| {
            ui.painter().circle_filled(Pos2::new(ui.cursor().left() + 4.0, ui.cursor().center().y), 4.0, status_color);
            ui.add_space(13.0);
            ui.label(egui::RichText::new(status_text).size(BODY).color(k.fg));
        });

        let mut token_field = self.github_token_field.clone();
        row(ui, k, "Token pessoal", "Cole o token do GitHub aqui. Ele fica no keychain do sistema.", |ui| {
            let response = input(ui, k, egui::Id::unique("github_token"), "Token pessoal", &mut token_field, "ghp_...", 300.0, true);
            if response.changed() {
                self.github_token_field = token_field.clone();
            }
        });

        row(ui, k, "", "", |ui| {
            if button(ui, k, "Salvar").clicked() {
                let token = self.github_token_field.trim();
                if !token.is_empty() {
                    match set_github_token(token) {
                        Ok(()) => {
                            self.toast("Token do GitHub salvo.");
                            self.github_token_field.clear();
                        }
                        Err(e) => self.toast(format!("Erro ao salvar token: {e}")),
                    }
                } else {
                    self.toast("Token vazio.");
                }
            }
            ui.add_space(6.5);
            if button(ui, k, "Testar").clicked() {
                if let Some(token) = github_token() {
                    let github = self.github.clone();
                    let ctx = ui.ctx().clone();
                    crate::app::integrations::github::poll_github(github, token, ctx);
                    self.toast("Buscando informações do GitHub...");
                } else {
                    self.toast("Nenhum token salvo.");
                }
            }
            ui.add_space(6.5);
            if button(ui, k, "Remover").clicked() {
                match remove_github_token() {
                    Ok(()) => {
                        self.toast("Token do GitHub removido.");
                        *self.github.lock().unwrap() = Default::default();
                    }
                    Err(e) => self.toast(format!("Erro ao remover token: {e}")),
                }
            }
        });

        divider(ui, k);
        title(ui, k, "PULL REQUESTS");

        let state = self.github.lock().unwrap().state.clone();
        match state {
            crate::app::integrations::github::GitHubState::Idle => {
                note(ui, k, "Salve um token do GitHub e clique em Testar para ver seus PRs.");
            }
            crate::app::integrations::github::GitHubState::Loading => {
                note(ui, k, "Carregando...");
            }
            crate::app::integrations::github::GitHubState::Loaded(prs) => {
                if prs.is_empty() {
                    note(ui, k, "Nenhum PR aberto.");
                } else {
                    for pr in &prs {
                        let ci_icon = match pr.ci {
                            crate::app::integrations::github::CiState::Success => "✓",
                            crate::app::integrations::github::CiState::Failure => "✗",
                            crate::app::integrations::github::CiState::Pending => "⋯",
                            crate::app::integrations::github::CiState::Unknown => "•",
                        };
                        let ci_color = match pr.ci {
                            crate::app::integrations::github::CiState::Success => Color32::from_rgb(52, 199, 89),
                            crate::app::integrations::github::CiState::Failure => Color32::from_rgb(255, 59, 48),
                            crate::app::integrations::github::CiState::Pending => Color32::from_rgb(255, 204, 0),
                            crate::app::integrations::github::CiState::Unknown => k.muted_fg,
                        };

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.5;
                            ui.label(egui::RichText::new(ci_icon).size(BODY).color(ci_color));
                            let response = ui.label(egui::RichText::new(format!("{} #{}", pr.repo, pr.number)).size(BODY).color(k.primary));
                            if response.clicked() {
                                crate::app::browser::open_external(&pr.url);
                            }
                            if response.hovered() {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                            ui.label(egui::RichText::new(&pr.title).size(SMALL).color(k.muted_fg));
                        });
                        ui.add_space(3.25);
                    }
                }
            }
            crate::app::integrations::github::GitHubState::Failed(why) => {
                note(ui, k, &format!("Erro: {why}"));
            }
        }
    }
}

pub(crate) fn github_token() -> Option<String> {
    take_secret(GITHUB_TOKEN_ID)
}

pub(crate) fn set_github_token(token: &str) -> Result<(), String> {
    set_secret(GITHUB_TOKEN_ID, token)
}

pub(crate) fn remove_github_token() -> Result<(), String> {
    remove_secret(GITHUB_TOKEN_ID)
}
