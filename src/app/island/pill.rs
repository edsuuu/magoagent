//! The pill while Spotify plays (asked: "como a Dynamic Island do iPhone quando toca música"): the song's cover beside
//! the wizard, at the left, and the sound's bars at the right, rising and falling with it. Paused or with nothing
//! loaded, the pill shows what it always did (`paint::island_small`).

use crate::prelude::*;

use super::music::{GREEN, picture};
use crate::app::integrations::spotify::groove::Groove;

/// The side of the cover and its corner on the compact island, and where it starts after the wizard (40 from the left,
/// 30 big).
const COVER: f32 = 26.0;
const COVER_CORNER: u8 = 7;
const COVER_LEFT: f32 = 62.0;
/// The same on the closed notch, which is smaller.
const COVER_SMALL: f32 = 16.0;
const COVER_CORNER_SMALL: u8 = 5;
const COVER_LEFT_SMALL: f32 = 4.0;

/// The bars: how many, the side of each one and the room between them, how far their middle is from the right edge,
/// and the tallest one.
const BARS: usize = 5;
const BAR: f32 = 3.0;
const BAR_GAP: f32 = 2.5;
const BAR_SMALL: f32 = 2.0;
const BAR_GAP_SMALL: f32 = 1.5;
const BARS_RIGHT: f32 = 20.0;
const BARS_RIGHT_SMALL: f32 = 12.0;
const BARS_TALL: f32 = 18.0;
const BARS_TALL_SMALL: f32 = 11.0;
/// The room the bars take at the right of the compact island: the agents' minis leave it free while the music plays.
const BARS_ROOM: f32 = BARS as f32 * BAR + (BARS - 1) as f32 * BAR_GAP;
/// How far left the agents' minis go while the bars are there: their room and a gap.
pub(super) const MINIS_LEFT: f32 = BARS_ROOM + 10.0;

/// The bars' heights at `time`, each 0 to 1: with the `groove` Luna dances to, the song's energy and bass on its beat,
/// each bar a little after the one before it; without it, the clock's slow sway. No two bars rise together.
pub(crate) fn bars(groove: Option<&Groove>, time: f64) -> [f32; BARS] {
    let mut heights = [0.0; BARS];
    for (index, height) in heights.iter_mut().enumerate() {
        let offset = index as f32 * 0.17;
        *height = match groove {
            Some(groove) => {
                let wave = (std::f32::consts::TAU * (groove.phase + offset)).sin() * 0.5 + 0.5;
                let beat = 1.0 - (groove.phase + offset * 0.5).fract();
                (0.15 + 0.45 * groove.energy + 0.3 * wave + 0.3 * groove.bass * beat * beat).clamp(0.0, 1.0)
            }
            None => {
                let index = index as f64;
                let sway = (time * 2.6 + index * 1.7).sin() * 0.5 + 0.5;
                let drift = (time * 0.9 + index * 0.6).sin() * 0.5 + 0.5;
                (0.25 + 0.45 * sway + 0.2 * drift) as f32
            }
        };
    }
    heights
}

/// The song's cover beside the wizard and the sound's bars at the right, in the `rect` of the compact island (`compact`)
/// or of the closed notch. `cover` is the song's picture (its placeholder without one); `groove` is what Luna dances to,
/// None while she doesn't. While they move, the next frame is asked for.
pub(super) fn spotify(ui: &egui::Ui, rect: Rect, cover: Option<egui::TextureId>, title: &str, compact: bool, groove: Option<&Groove>, time: f64) {
    let painter = ui.painter();
    let (side, corner, left, bar, gap, right, tall) = if compact {
        (COVER, COVER_CORNER, COVER_LEFT, BAR, BAR_GAP, BARS_RIGHT, BARS_TALL)
    } else {
        (COVER_SMALL, COVER_CORNER_SMALL, COVER_LEFT_SMALL, BAR_SMALL, BAR_GAP_SMALL, BARS_RIGHT_SMALL, BARS_TALL_SMALL)
    };
    let art = Rect::from_min_size(Pos2::new(rect.left() + left, rect.center().y - side / 2.0), Vec2::splat(side));
    picture(painter, art, corner, cover, title);
    let total = BARS as f32 * bar + (BARS - 1) as f32 * gap;
    for (index, level) in bars(groove, time).into_iter().enumerate() {
        let x = rect.right() - right - total / 2.0 + index as f32 * (bar + gap) + bar / 2.0;
        // a dot at its lowest, the tallest at its highest
        let height = (tall * (0.18 + 0.82 * level)).max(bar);
        let rise = Rect::from_center_size(Pos2::new(x, rect.center().y), Vec2::new(bar, height));
        painter.rect_filled(rise, bar / 2.0, GREEN);
    }
    ui.ctx().request_repaint_after(crate::app::update::frame_spacing());
}
