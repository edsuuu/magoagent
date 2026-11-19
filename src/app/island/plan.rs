//! The plan's use in the island's header: the 5-hour session and the week of the agent read first (Claude Code, else
//! Codex), right of the tabs and left of the buttons, as the chat list's foot says it.

use super::paint::{AMBER, DIM, DIM_3, INK, font};
use crate::app::usage::foot::HOT;
use crate::prelude::*;

/// A window's bar, and the room between two windows.
const BAR: f32 = 26.0;
const GAP: f32 = 12.0;

/// The windows of `plan` that fit between `left` and `right` (the session first), as wide as they take: name, bar and
/// how much is used.
pub(crate) fn fitting(plan: &[(&str, f64)], left: f32, right: f32, width: impl Fn(&str, f64) -> f32) -> usize {
    let mut taken = 0.0;

    plan.iter()
        .take_while(|(name, used)| {
            let piece = width(name, *used) + if taken > 0.0 { GAP } else { 0.0 };

            taken += piece;
            taken <= right - left
        })
        .count()
}

/// Draws the windows of `plan` that fit, ending at `right` and from `left` at most, on the row at `y`.
pub(super) fn paint_plan(painter: &Painter, plan: &[(&str, f64)], left: f32, right: f32, y: f32) {
    let percent = |used: f64| format!("{:.0}%", used * 100.0);
    let words = |text: String, color: Color32| painter.layout_no_wrap(text, font(11.0), color);
    let width = |name: &str, used: f64| words(name.to_string(), DIM_3).size().x + 5.0 + BAR + 5.0 + words(percent(used), DIM).size().x;
    let count = fitting(plan, left, right, width);
    let mut x = right;

    for (name, used) in plan[..count].iter().rev() {
        let number = words(percent(*used), DIM);

        x -= number.size().x;
        painter.galley(Pos2::new(x, y - number.size().y / 2.0), number, DIM);
        x -= 5.0 + BAR;
        let bar = Rect::from_min_size(Pos2::new(x, y - 2.0), Vec2::new(BAR, 4.0));
        let fill = Rect::from_min_size(bar.min, Vec2::new(BAR * used.clamp(0.0, 1.0) as f32, 4.0));

        painter.rect_filled(bar, 2, Color32::from_white_alpha(28));
        painter.rect_filled(fill, 2, if *used >= HOT { AMBER } else { INK.gamma_multiply(0.75) });
        let label = words(name.to_string(), DIM_3);

        x -= 5.0 + label.size().x;
        painter.galley(Pos2::new(x, y - label.size().y / 2.0), label, DIM_3);
        x -= GAP;
    }
}
