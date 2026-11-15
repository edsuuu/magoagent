//! The integrations (Ajustes › Integrações, `app/integrations/`) on the island: each one that is on is a pill of
//! Home's grid and, in focus, a card with its lines and its buttons. A press is only written down here
//! (`Integrations::pressed`): the card is drawn where the app is only read.

use super::paint::{DIM, INK};
use super::{Line, forms};
use crate::app::chat::list::clipped;
use crate::app::integrations::ITEM_PREFIX;
use crate::app::mascot::mago::MagoState;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;

/// How far apart the card's lines are.
const ROW: f32 = 15.0;

impl DrawingApp {
    /// The integrations' pills, after the agents'.
    pub(super) fn integration_lines(&self) -> Vec<Line> {
        let line = |item: crate::app::integrations::IslandItem| Line {
            detail: item.rows.first().map(|row| row.text.clone()).unwrap_or_default(),
            id: item.id,
            name: item.name.to_string(),
            state: AgentState::Idle(None),
            mago: if item.failed { MagoState::Error } else { MagoState::Idle },
            color: item.color,
            chat: false,
            badge: item.badge,
            kind: item.kind,
        };
        self.integration_items().into_iter().map(line).collect()
    }

    /// The card of the integration in focus: its name, its lines (one with an action answers the press) and its
    /// buttons. False when `line` is not an integration's.
    pub(super) fn integration_card(&self, ui: &mut egui::Ui, card: Rect, line: &Line) -> bool {
        let Some(name) = line.id.strip_prefix(ITEM_PREFIX) else { return false };
        let Some(item) = self.integration_items().into_iter().find(|item| item.id == line.id) else { return true };
        let mut pressed = vec![format!("seen {name}")];
        let text_x = card.left() + 106.0;
        let width = card.right() - text_x - 14.0;
        let top = card.top() + 19.0;
        ui.painter().circle_filled(Pos2::new(text_x + 4.0, top), 4.0, item.color);
        let title = clipped(ui.painter(), item.name.to_string(), FontId::proportional(13.0), INK, width * 0.5);
        let title_width = title.size().x;
        for shift in [0.0, 0.5] {
            ui.painter().galley(Pos2::new(text_x + 14.0 + shift, top - title.size().y / 2.0), title.clone(), INK);
        }
        let kind = clipped(ui.painter(), item.kind.clone(), FontId::proportional(11.5), DIM, (width - 20.0 - title_width).max(0.0));
        ui.painter().galley(Pos2::new(text_x + 20.0 + title_width, top - kind.size().y / 2.0 + 0.5), kind, DIM);
        let bottom = card.bottom() - if item.chips.is_empty() { 8.0 } else { 42.0 };
        let capacity = ((bottom - (card.top() + 31.0)) / ROW).floor().max(1.0) as usize;
        for (index, row) in item.rows.iter().take(capacity).enumerate() {
            let area = Rect::from_min_size(Pos2::new(text_x, card.top() + 31.0 + index as f32 * ROW), Vec2::new(width, ROW));
            let hovered = row.action.as_ref().is_some_and(|action| {
                let response =
                    ui.interact(area, ui.scope_id().with(("island_integration_row", index)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                if response.clicked() {
                    pressed.push(action.clone());
                }
                response.hovered()
            });
            let color = if hovered { INK } else { row.color };
            let words = clipped(ui.painter(), row.text.clone(), FontId::proportional(11.5), color, width);
            ui.painter().galley(Pos2::new(text_x, area.center().y - words.size().y / 2.0), words, color);
        }
        let mut chip_x = text_x;
        for (index, (label, action)) in item.chips.iter().enumerate() {
            let response = forms::chip(ui, Pos2::new(chip_x, card.bottom() - 36.0), label, index == 0);
            chip_x = response.rect.right() + 6.0;
            if response.clicked() {
                pressed.push(action.clone());
            }
        }
        self.integrations.pressed.borrow_mut().extend(pressed);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::integrations::github::{CiState, GitHub, PullRequest, Pulse};
    use crate::app::maestro::outside::Badge;

    /// A check that fails between two polls: GitHub's pill shows the alert, with the wizard and the badge of an error.
    #[test]
    fn a_failed_check_is_an_error_on_githubs_pill() {
        let request = |ci| PullRequest {
            id: "a/b#1".into(),
            title: "T".into(),
            url: "https://github.com/a/b/pull/1".into(),
            draft: false,
            ci,
            review: "",
            head: Some("s1".into()),
        };
        let pulse = |ci| Pulse { login: "ana".into(), mine: vec![request(ci)], to_review: vec![], remaining: None };
        let mut app = DrawingApp::default();
        app.prefs.integrations.github = true;
        app.integrations.github = GitHub::with_token("ghp_0123456789abcdefghijABCDEFGHIJ012345");
        app.integrations.github.take_pulse(pulse(CiState::Pending), 1_000);
        let calm = app.integration_lines();
        assert_eq!(calm.len(), 1);
        assert!(calm[0].mago == MagoState::Idle && calm[0].badge.is_none() && calm[0].detail == "… a/b#1 T");
        app.integrations.github.take_pulse(pulse(CiState::Failure), 2_000);
        let line = app.integration_lines().remove(0);
        assert_eq!((line.id.as_str(), line.name.as_str(), line.kind.as_str()), ("integration:github", "GitHub", "@ana"));
        assert!(line.mago == MagoState::Error && line.badge == Some(Badge::Error), "the island sounds its error for it");
        assert_eq!(line.detail, "Os checks de a/b#1 falharam");
    }
}
