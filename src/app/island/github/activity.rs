//! The GitHub tab's Atividade: under the list, the last week's seven squares; pressed, the card at the right shows the
//! contribution calendar of the last months (the day under the pointer in words), the year's total, the repositories and
//! their stars, and the person's profile a press away.

use super::super::forms;
use super::super::paint::{DIM, DIM_3, INK, font, semi};
use super::Page;
use crate::app::integrations::github::day_words;
use crate::prelude::*;

const CELL: f32 = 11.0;
const CELL_GAP: f32 = 2.0;
const SPARK_CELL: f32 = 8.0;
const SPARK_GAP: f32 = 2.0;
/// The seven squares' width, at the end of the list's footer.
pub(super) const SPARK_W: f32 = 7.0 * SPARK_CELL + 6.0 * SPARK_GAP;
const MONTHS: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];

/// GitHub's greens of the dark theme, an empty day barely there.
pub(super) fn level_color(level: u8) -> Color32 {
    match level {
        1 => Color32::from_rgb(0x0e, 0x44, 0x29),
        2 => Color32::from_rgb(0x00, 0x6d, 0x32),
        3 => Color32::from_rgb(0x26, 0xa6, 0x41),
        4 => Color32::from_rgb(0x39, 0xd3, 0x53),
        _ => Color32::from_white_alpha(16),
    }
}

/// The month a week begins in ("2026-11-01" is 11), when GitHub said its date.
fn month_of(week: &[crate::app::integrations::github::Day]) -> Option<usize> {
    week.first()?.date.get(5..7)?.parse().ok().filter(|month| (1..=12).contains(month))
}

impl DrawingApp {
    /// The seven squares ending at `right` on the footer's line: a press opens Atividade, another goes back.
    pub(super) fn github_spark(&self, ui: &mut egui::Ui, right: f32, center_y: f32, state: &mut Page) {
        let Some(activity) = &self.integrations.github.activity else { return };
        let rect = Rect::from_min_size(Pos2::new(right - SPARK_W, center_y - SPARK_CELL / 2.0), Vec2::new(SPARK_W, SPARK_CELL));

        let response = ui
            .interact(rect.expand(4.0), ui.scope_id().with("github_spark"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Atividade no GitHub");

        if response.hovered() || state.activity {
            ui.painter().rect_filled(rect.expand(4.0), 5, Color32::from_white_alpha(14));
        }

        for (index, day) in activity.last_days(7).iter().enumerate() {
            let at = Pos2::new(rect.left() + index as f32 * (SPARK_CELL + SPARK_GAP), rect.top());

            ui.painter().rect_filled(Rect::from_min_size(at, Vec2::splat(SPARK_CELL)), 2, level_color(day.level));
        }

        if response.clicked() {
            state.activity = !state.activity;
        }
    }

    /// Atividade in the card at the right.
    pub(super) fn github_activity(&self, ui: &mut egui::Ui, card: Rect, login: &str, state: &mut Page) {
        let inner = card.shrink2(Vec2::new(14.0, 12.0));

        ui.painter().text(inner.left_top(), egui::Align2::LEFT_TOP, "Atividade", semi(13.0), INK);

        let Some(activity) = &self.integrations.github.activity else {
            ui.painter().text(card.center(), egui::Align2::CENTER_CENTER, "Lendo a atividade no GitHub…", font(12.0), DIM_3);

            return;
        };

        let columns = (((inner.width() + CELL_GAP) / (CELL + CELL_GAP)).floor() as usize).max(1);
        let weeks = &activity.weeks[activity.weeks.len().saturating_sub(columns)..];

        let grid = Rect::from_min_size(
            Pos2::new(inner.left(), inner.top() + 62.0),
            Vec2::new(weeks.len() as f32 * (CELL + CELL_GAP) - CELL_GAP, 7.0 * (CELL + CELL_GAP) - CELL_GAP),
        );

        // the day under the pointer
        let pointer = ui.input(|input| input.pointer.hover_pos()).filter(|at| grid.expand(1.0).contains(*at));

        let hovered = pointer.and_then(|at| {
            let column = ((at.x - grid.left()) / (CELL + CELL_GAP)) as usize;
            let line = ((at.y - grid.top()) / (CELL + CELL_GAP)) as u8;

            weeks.get(column)?.iter().find(|day| day.weekday == line)
        });

        let words = hovered.map_or_else(|| activity.summary(), day_words);
        let galley = crate::app::chat::list::clipped(ui.painter(), words, font(11.0), DIM, inner.width());

        ui.painter().galley(Pos2::new(inner.left(), inner.top() + 20.0), galley, DIM);
        // the months over the weeks they begin
        let mut last_month = None;

        for (column, week) in weeks.iter().enumerate() {
            let Some(month) = month_of(week) else { continue };

            // a month's name needs two weeks of room
            if last_month != Some(month) && column + 2 < weeks.len() {
                let at = Pos2::new(grid.left() + column as f32 * (CELL + CELL_GAP), grid.top() - 6.0);

                ui.painter().text(at, egui::Align2::LEFT_BOTTOM, MONTHS[month - 1], font(10.0), DIM_3);
            }

            last_month = Some(month);
        }

        for (column, week) in weeks.iter().enumerate() {
            for day in week {
                let at = Pos2::new(grid.left() + column as f32 * (CELL + CELL_GAP), grid.top() + f32::from(day.weekday) * (CELL + CELL_GAP));
                let cell = Rect::from_min_size(at, Vec2::splat(CELL));

                ui.painter().rect_filled(cell, 2, level_color(day.level));

                if hovered == Some(day) {
                    ui.painter().rect_stroke(cell.expand(1.0), 3, Stroke::new(1.0, INK), StrokeKind::Outside);
                }
            }
        }

        // Menos ▪▪▪▪▪ Mais, as GitHub's own
        let legend_y = grid.bottom() + 16.0;
        let more = ui.painter().text(Pos2::new(grid.right(), legend_y), egui::Align2::RIGHT_CENTER, "Mais", font(10.0), DIM_3);
        let mut x = more.left() - 6.0 - 5.0 * (CELL + CELL_GAP) + CELL_GAP;

        ui.painter().text(Pos2::new(x - 6.0, legend_y), egui::Align2::RIGHT_CENTER, "Menos", font(10.0), DIM_3);

        for level in 0..=4 {
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x, legend_y - CELL / 2.0), Vec2::splat(CELL)), 2, level_color(level));
            x += CELL + CELL_GAP;
        }

        let today = match activity.today {
            0 => "Nenhuma contribuição hoje".to_string(),
            1 => "1 contribuição hoje".to_string(),
            count => format!("{count} contribuições hoje"),
        };

        ui.painter().text(Pos2::new(inner.left(), legend_y), egui::Align2::LEFT_CENTER, today, font(11.0), DIM);
        let buttons_y = inner.bottom() - 26.0;
        let profile = forms::chip(ui, Pos2::new(inner.left(), buttons_y), "Abrir o perfil", true);

        if profile.clicked() && !login.is_empty() {
            self.integrations.pressed.borrow_mut().push(format!("open https://github.com/{login}"));
        }

        if forms::chip(ui, Pos2::new(profile.rect.right() + 6.0, buttons_y), "Voltar", false).clicked() {
            state.activity = false;
        }
    }
}
