//! Ajustes › Integrações: one group per integration (`app/integrations/`): its dot (green connected, red not), its name
//! and what it does, its switch, then its own fields and buttons. Off, only the head shows.

use super::kit::*;
use crate::app::chat::list::clipped;
use crate::app::integrations::{github, open_page};
use crate::prelude::*;

const GREEN: Color32 = Color32::from_rgb(0x34, 0xc7, 0x59);
const AMBER: Color32 = Color32::from_rgb(0xf5, 0xa5, 0x24);

/// An integration's head: the dot, the name, what it does under it, and its switch at the right. True when flipped.
fn head(ui: &mut egui::Ui, k: &K, name: &str, hint: &str, connected: bool, value: &mut bool) -> bool {
    let width = ui.available_width();
    let text = galley(ui, name, BODY, BODY_LINE, k.fg, width - 67.0);
    let hint_galley = galley(ui, hint, SMALL, SMALL_LINE, k.muted_fg, width - 67.0);
    let (pad, name_height) = (4.875, text.size().y);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 2.0 * pad + name_height + 1.625 + hint_galley.size().y), Sense::hover());
    ui.painter().circle_filled(Pos2::new(rect.left() + 5.0, rect.top() + pad + name_height / 2.0), 4.0, if connected { GREEN } else { k.destructive });
    ui.painter().galley(Pos2::new(rect.left() + 16.0, rect.top() + pad), text, k.fg);
    ui.painter().galley(Pos2::new(rect.left() + 16.0, rect.top() + pad + name_height + 1.625), hint_galley, k.muted_fg);
    switch(ui, k, Pos2::new(rect.right() - 38.0, rect.top() + pad), name, value)
}

/// A line that opens a page: a mark in its color, the words cut to fit, and what is beside them at the right.
fn link(ui: &mut egui::Ui, k: &K, mark: (&str, Color32), text: &str, beside: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, format!("{text} ({beside})")));
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.5, k.muted.gamma_multiply(if k.dark { 0.5 } else { 1.0 }));
    }
    ui.painter().text(Pos2::new(rect.left() + 9.0, rect.center().y), egui::Align2::CENTER_CENTER, mark.0, FontId::proportional(SMALL), mark.1);
    let beside = ui.painter().layout_no_wrap(beside.to_string(), FontId::proportional(SMALL), k.muted_fg);
    let words = clipped(ui.painter(), text.to_string(), FontId::proportional(SMALL), k.fg, rect.width() - 22.0 - beside.size().x - 16.0);
    ui.painter().galley(Pos2::new(rect.left() + 22.0, rect.center().y - words.size().y / 2.0), words, k.fg);
    ui.painter().galley(Pos2::new(rect.right() - 6.5 - beside.size().x, rect.center().y - beside.size().y / 2.0), beside, k.muted_fg);
    response.clicked()
}

fn check_color(k: &K, state: github::CiState) -> Color32 {
    match state {
        github::CiState::Success => GREEN,
        github::CiState::Failure => k.destructive,
        github::CiState::Pending => AMBER,
        github::CiState::Unknown => k.muted_fg,
    }
}

/// The contribution calendar: a square a day, a column a week, the greener the more was done.
fn calendar(ui: &mut egui::Ui, k: &K, activity: &github::Activity) {
    const CELL: f32 = 8.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 7.0 * (CELL + 2.0)), Sense::hover());
    for (column, week) in activity.weeks.iter().enumerate() {
        for (line, level) in week.iter().enumerate() {
            let at = Pos2::new(rect.left() + column as f32 * (CELL + 2.0), rect.top() + line as f32 * (CELL + 2.0));
            let color = if *level == 0 { k.muted } else { k.muted.lerp_to_gamma(GREEN, f32::from(*level) / 4.0) };
            ui.painter().rect_filled(Rect::from_min_size(at, Vec2::splat(CELL)), 2.0, color);
        }
    }
}

impl DrawingApp {
    pub(super) fn settings_integrations(&mut self, ui: &mut egui::Ui, k: &K) {
        note(
            ui,
            k,
            "Serviços de fora, mostrados na ilha. Cada um fica desligado até você ligar; o que é segredo (um token) vai para o cofre do sistema, nunca para um arquivo.",
        );
        ui.add_space(9.75);
        self.settings_github(ui, k);
    }

    fn settings_github(&mut self, ui: &mut egui::Ui, k: &K) {
        let hint = "Os seus pull requests com os checks, as revisões que pedem a você e a sua atividade.";
        head(ui, k, "GitHub", hint, self.integrations.github.connected(), &mut self.prefs.integrations.github);
        if !self.prefs.integrations.github {
            return;
        }
        let (mut save, mut test, mut remove) = (false, false, false);
        let hint = "Clássico com o escopo repo, ou fine-grained só de leitura em Pull requests, Commit statuses e Actions.";
        row(ui, k, "Token pessoal", hint, |ui| {
            save = button(ui, k, "Salvar").clicked();
            if button(ui, k, "Criar token…").clicked() {
                open_page(github::TOKENS_PAGE);
            }
            let field = input(
                ui,
                k,
                egui::Id::unique("settings_github_token"),
                "Token pessoal do GitHub",
                &mut self.integrations.token_field,
                "ghp_… ou github_pat_…",
                250.0,
                true,
            );
            save |= field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        });
        let github = &self.integrations.github;
        if github.has_token() {
            let (status, detail) = match (&github.pulse, &github.error) {
                (_, Some(error)) => ("Não conectado".to_string(), error.clone()),
                (Some(pulse), None) => (format!("Conectado como @{}", pulse.login), pulse.summary()),
                (None, None) => ("Falando com o GitHub…".to_string(), "O token está no cofre do sistema.".to_string()),
            };
            row(ui, k, &status, &detail, |ui| {
                remove = button(ui, k, "Remover").clicked();
                test = button(ui, k, if github.testing { "Testando…" } else { "Testar" }).clicked();
            });
        } else {
            note(ui, k, "Sem token: crie um, cole acima e salve. Ele vai para o cofre do sistema, nunca para um arquivo.");
        }
        if let Some(pulse) = &github.pulse {
            for request in &pulse.mine {
                if link(ui, k, (request.ci.label().0, check_color(k, request.ci)), &format!("{}  {}", request.id, request.title), &request.state_words()) {
                    open_page(&request.url);
                }
            }
            for request in &pulse.to_review {
                if link(ui, k, ("•", AMBER), &format!("{}  {}", request.id, request.title), "pede a sua revisão") {
                    open_page(&request.url);
                }
            }
        }
        if let Some(activity) = &github.activity {
            ui.add_space(6.5);
            note(ui, k, &format!("{} contribuições no último ano · {} hoje", activity.total, activity.today));
            ui.add_space(3.25);
            calendar(ui, k, activity);
        }
        if save {
            let token = std::mem::take(&mut self.integrations.token_field);
            let said = self.github_save_token(&token).unwrap_or_else(|why| why);
            self.toast(said);
        } else if test {
            self.github_test();
        } else if remove {
            let said = self.github_remove_token().unwrap_or_else(|why| format!("O token não foi removido: {why}"));
            self.toast(said);
        }
    }
}
