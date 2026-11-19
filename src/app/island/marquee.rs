//! A name too long for its room ("Claude Code · mag…"), as the pills of the grid and the card's name row show it: cut
//! with "…" as it is, and with the pointer on it, scrolled to its end and back in a loop, a pause at each end.

use crate::app::chat::list::clipped;
use crate::prelude::*;

/// How long the name rests at each end, in seconds.
const PAUSE: f64 = 1.2;
/// How fast it scrolls, in points a second.
const SPEED: f64 = 32.0;

/// How far a name `overflow` points wider than its room is scrolled `elapsed` seconds after the pointer came on it: at
/// its start for a pause, to its end at `SPEED`, a pause there, back at the same speed, and round again.
pub(crate) fn marquee_offset(overflow: f32, elapsed: f64) -> f32 {
    if overflow <= 0.0 || elapsed <= 0.0 {
        return 0.0;
    }

    let travel = overflow as f64 / SPEED;
    let at = elapsed % (2.0 * (PAUSE + travel));

    let offset = if at < PAUSE {
        0.0
    } else if at < PAUSE + travel {
        (at - PAUSE) * SPEED
    } else if at < 2.0 * PAUSE + travel {
        overflow as f64
    } else {
        overflow as f64 - (at - 2.0 * PAUSE - travel) * SPEED
    };

    offset.clamp(0.0, overflow as f64) as f32
}

/// `text` in `font` and `color` with its left at `left` and centred on `center_y`, in `width` at most: cut with "…" as
/// it is; with `hovered` (the pointer on it) and too long, scrolled (`marquee_offset`) inside `width`, asking for the
/// frames it needs. `id` keeps when the pointer came. Returns the width it takes.
pub(crate) fn marquee(ui: &egui::Ui, id: egui::Id, text: &str, font: FontId, color: Color32, (left, center_y): (f32, f32), width: f32, hovered: bool) -> f32 {
    let whole = ui.painter().layout_no_wrap(text.to_string(), font.clone(), color);
    let overflow = whole.size().x - width;
    let time = ui.input(|input| input.time);

    let since = ui.ctx().data_mut(|data| {
        if hovered && overflow > 0.0 {
            Some(*data.get_temp_mut_or_insert_with(id, || time))
        } else {
            data.remove::<f64>(id);
            None
        }
    });

    let Some(since) = since else {
        let cut = clipped(ui.painter(), text.to_string(), font, color, width);
        let taken = cut.size().x;

        ui.painter().galley(Pos2::new(left, center_y - cut.size().y / 2.0), cut, color);

        return taken;
    };

    let offset = marquee_offset(overflow, time - since);
    let room = Rect::from_min_max(Pos2::new(left, center_y - whole.size().y), Pos2::new(left + width, center_y + whole.size().y));
    let height = whole.size().y;

    ui.painter().with_clip_rect(room.intersect(ui.clip_rect())).galley(Pos2::new(left - offset, center_y - height / 2.0), whole, color);
    ui.ctx().request_repaint();
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_name_rests_scrolls_to_its_end_rests_and_comes_back() {
        let overflow = 64.0;
        let travel = overflow as f64 / SPEED;

        assert_eq!(marquee_offset(overflow, 0.0), 0.0);
        assert_eq!(marquee_offset(overflow, PAUSE * 0.9), 0.0, "a pause at the start");
        assert!((marquee_offset(overflow, PAUSE + travel / 2.0) - overflow / 2.0).abs() < 0.01, "half way there");
        assert_eq!(marquee_offset(overflow, PAUSE + travel + PAUSE * 0.5), overflow, "a pause at the end");
        let back = marquee_offset(overflow, 2.0 * PAUSE + travel + travel / 4.0);

        assert!((back - overflow * 0.75).abs() < 0.01, "on its way back: {back}");
        assert!(marquee_offset(overflow, 2.0 * (PAUSE + travel) + PAUSE * 0.5) < 0.01, "and round again from the start");

        for step in 0..400 {
            let offset = marquee_offset(overflow, step as f64 * 0.025);

            assert!((0.0..=overflow).contains(&offset), "never past either end: {offset}");
        }

        assert_eq!(marquee_offset(0.0, 3.0), 0.0, "a name that fits never moves");
        assert_eq!(marquee_offset(-10.0, 3.0), 0.0);
    }
}
