//! The chat's accordions (a folded run of steps, a step's card): what they hold eases open and shut under the row
//! clicked, its chevron turning with it (› shut, ⌄ open); shut, nothing of it is laid out.

use crate::app::chat::list::RIGHT;
use crate::prelude::*;

/// How long an accordion takes to open or shut.
pub(crate) const UNFOLD_SECONDS: f32 = 0.18;

/// How open accordion `id` is, 0 shut to 1 open, eased toward `open` over `UNFOLD_SECONDS` (repainting while it moves).
/// The first time it is seen it stands where it is: a conversation shown with a step open shows it open.
pub(crate) fn openness(ctx: &Context, id: egui::Id, open: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, open, UNFOLD_SECONDS, egui::emath::easing::cubic_out)
}

/// What accordion `id` holds, under its row, `openness` of it shown: laid out whole and cut at the bottom while it moves
/// (the gap over it too, so open and shut meet without a jump), not at all when shut. What `add` gave, when laid out.
pub(crate) fn unfold<R>(ui: &mut egui::Ui, id: egui::Id, openness: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    if openness <= 0.0 {
        return None;
    }
    let key = id.with("unfold_height");
    let gap = ui.spacing().item_spacing.y;
    // as tall as it was laid out last frame (in its first, nothing of it shows yet)
    let full = ui.data(|data| data.get_temp::<f32>(key)).unwrap_or(0.0);
    let shown = if openness < 1.0 { openness * (full + gap) - gap } else { f32::INFINITY };
    if shown < 0.0 {
        ui.add_space(shown);
    }
    let mut body = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(ui.available_rect_before_wrap()));
    let mut clip = body.clip_rect();
    clip.max.y = clip.max.y.min(body.max_rect().top() + shown.max(0.0));
    body.set_clip_rect(clip);
    let inner = add(&mut body);
    let laid = body.min_rect();
    ui.data_mut(|data| data.insert_temp(key, laid.height()));
    ui.advance_cursor_after_rect(laid.with_max_y(laid.top() + laid.height().min(shown.max(0.0))));
    Some(inner)
}

/// The accordion's chevron, `size` across centered at `at`: › shut, ⌄ open, turning between them with `openness`.
pub(crate) fn chevron(p: &Painter, at: Pos2, size: f32, width: f32, color: Color32, openness: f32) {
    p.add(Shape::line(chevron_points(at, size, openness), Stroke::new(width * size / 24.0, color)));
}

/// `RIGHT` (the design's 24-unit drawing) turned a quarter turn clockwise as it opens: `DOWN` when open.
fn chevron_points(at: Pos2, size: f32, openness: f32) -> Vec<Pos2> {
    let turn = egui::emath::Rot2::from_angle(openness.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2);
    RIGHT[0].iter().map(|&(x, y)| at + turn * (Vec2::new(x - 12.0, y - 12.0) * size / 24.0)).collect()
}

#[cfg(test)]
mod tests {
    use crate::app::chat::list::{DOWN, RIGHT};
    use crate::prelude::*;

    #[test]
    fn the_chevron_turns_from_right_to_down() {
        let points = |openness: f32| super::chevron_points(Pos2::new(12.0, 12.0), 24.0, openness);
        let near = |a: &[Pos2], b: &[(f32, f32)]| a.iter().zip(b).all(|(a, &(x, y))| (*a - Pos2::new(x, y)).length() < 1e-3);
        assert!(near(&points(0.0), RIGHT[0]), "shut: ›");
        let down: Vec<(f32, f32)> = DOWN[0].iter().rev().copied().collect();
        assert!(near(&points(1.0), &down), "open: ⌄ {:?}", points(1.0));
        let half = points(0.5);
        assert!(!near(&half, RIGHT[0]) && !near(&half, &down), "between them halfway");
    }
}
