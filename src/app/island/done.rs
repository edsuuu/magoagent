//! An agent's turn ending, on the island (asked: "Sempre quando termina, eles acionam a ilha e ela abre sem necessidade
//! ou faz barulho; poderia só mudar a Luna com um check, aí eu clico e vejo"): one that finished neither opens it nor
//! sounds; Luna wears a check until the person opens the island and sees what finished. One that stopped on a question
//! or a permission still opens it, as a question does.

use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;

/// The green of a done card and of a done mini's badge.
const GREEN: Color32 = Color32::from_rgb(0x34, 0xd3, 0x99);

/// Whether an agent at work last frame (`working`) finished its turn now. One stopped (Esc: idle since no answer) or
/// closed did not.
pub(crate) fn turn_done(working: &std::collections::HashSet<String>, rows: &[AgentRow]) -> bool {
    rows.iter().any(|row| working.contains(&row.id) && matches!(row.state, AgentState::Idle(Some(_))))
}

/// Whether one at work last frame stopped on the person now: a question or a permission only they answer.
pub(crate) fn turn_waits(working: &std::collections::HashSet<String>, rows: &[AgentRow]) -> bool {
    rows.iter().any(|row| working.contains(&row.id) && row.state == AgentState::Waiting)
}

/// The check Luna wears, at her upper right: a green seal, ringed in the island's black, with the check in it. Drawn
/// small as on the closed notch (Luna 18 tall) and as big as the compact island's.
pub(super) fn paint_check(painter: &Painter, luna: Pos2, size: f32) {
    let radius = (size * 0.2).max(4.0);
    let at = luna + Vec2::new(size * 0.34, -size * 0.3);
    painter.circle_filled(at, radius + 1.5, super::paint::BLACK);
    painter.circle_filled(at, radius, GREEN);
    // the check of the chat's lists (24 across), as wide as the seal
    let scale = radius * 1.5 / 24.0;
    let points: Vec<Pos2> = crate::app::chat::list::CHECK[0].iter().map(|&(x, y)| at + Vec2::new(x - 12.0, y - 12.0) * scale).collect();
    painter.add(egui::Shape::line(points, Stroke::new((radius * 0.3).max(1.2), super::paint::BLACK)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_turn_is_told_apart_from_one_that_waits_on_the_person() {
        let working: std::collections::HashSet<String> = ["a".to_string()].into();
        let row = |state| AgentRow { id: "a".into(), spec: Default::default(), state, mission: None, space: String::new() };
        assert!(turn_done(&working, &[row(AgentState::Idle(Some(1.0)))]), "done");
        assert!(!turn_waits(&working, &[row(AgentState::Idle(Some(1.0)))]));
        assert!(turn_waits(&working, &[row(AgentState::Waiting)]), "waiting on the person");
        assert!(!turn_done(&working, &[row(AgentState::Waiting)]));
        assert!(!turn_done(&working, &[row(AgentState::Working(1.0))]) && !turn_waits(&working, &[row(AgentState::Working(1.0))]), "still at work");
        assert!(!turn_done(&working, &[row(AgentState::Idle(None))]), "stopped: no answer ended");
        assert!(!turn_done(&Default::default(), &[row(AgentState::Idle(Some(1.0)))]), "it was not at work");
    }
}
