//! The pill while Spotify plays: the song's cover beside the wizard, at the left, and the sound's bars at the right,
//! rising and falling with it. Paused or with nothing loaded, the pill shows what it always did
//! (`paint::island_small`).

use crate::prelude::*;

use super::music::{GREEN, picture};

use crate::app::integrations::spotify::CoverFade;
use crate::app::integrations::spotify::groove::{Ear, Groove};

/// The side of the cover and its corner on the compact island.
const COVER: f32 = 22.0;
const COVER_CORNER: u8 = 6;
/// The wizard on the compact island: its middle from the left edge and its size (`paint::island_small`); the last item
/// ends as far from the right edge as the wizard starts from the left.
const WIZARD_AT: f32 = super::COMPACT_SIDE;
const WIZARD: f32 = 30.0;
/// The same on the closed notch, which is smaller: the cover this far from its left end (standing up, its top).
const COVER_SMALL: f32 = 16.0;
const COVER_CORNER_SMALL: u8 = 5;
const COVER_LEFT_SMALL: f32 = 4.0;

/// The bars: how many, the side of each one and the room between them, how far their middle is from the right edge
/// (standing up, from the bottom), and the tallest one.
const BARS: usize = crate::app::integrations::spotify::groove::BANDS;
const BAR: f32 = 1.5;
const BAR_GAP: f32 = 3.0;
const BAR_SMALL: f32 = 1.5;
const BAR_GAP_SMALL: f32 = 1.5;
const BARS_RIGHT_SMALL: f32 = 12.0;
const BARS_TALL: f32 = 16.0;
const BARS_TALL_SMALL: f32 = 11.0;
/// The width the bars take on the compact island.
const BARS_ROOM: f32 = BARS as f32 * BAR + (BARS - 1) as f32 * BAR_GAP;

/// The room between the cover and the bars, which go together.
const PAIR_GAP: f32 = 10.0;

/// Where the cover and the bars start on the compact island `rect`, which go side by side: with the agents' minis
/// (`minis`, their grid) in the middle between the wizard and them; with none, at the right, as far from the edge as the
/// wizard is from the left. Around a camera `notch` (its width; 0 without one) the middle is under it: the cover goes in
/// the left wing after the wizard and the bars in the right one before the minis.
pub(crate) fn spread(rect: Rect, minis: Option<Rect>, notch: f32) -> (f32, f32) {
    let pair = COVER + PAIR_GAP + BARS_ROOM;
    let edge = WIZARD_AT - WIZARD / 2.0;
    if notch > 0.0 {
        let bars_end = minis.map_or(rect.right() - edge, |grid| grid.left() - PAIR_GAP);
        return (rect.left() + edge + WIZARD + PAIR_GAP, bars_end - BARS_ROOM);
    }
    let cover = match minis {
        Some(grid) => (rect.left() + edge + WIZARD + grid.left()) / 2.0 - pair / 2.0,
        // no minis: the song by the right side, with the minis' room to the edge
        None => rect.right() - (super::MINIS_SIDE - WIZARD / 2.0) - pair,
    };
    (cover, cover + COVER + PAIR_GAP)
}

/// The same on the compact island standing up on a side (`rect`, `COMPACT_TALL_SONG` tall): where the cover and the
/// bars start from its top, one under the other, in the middle between the wizard and the minis' grid (`minis`), or
/// between the wizard and the bottom without agents.
pub(crate) fn spread_upright(rect: Rect, minis: Option<Rect>) -> (f32, f32) {
    let pair = COVER + PAIR_GAP + BARS_ROOM;
    let wizard_end = rect.top() + super::minis::UPRIGHT_INSET + WIZARD / 2.0;
    let end = minis.map_or(rect.bottom(), |grid| grid.top());
    let cover = (wizard_end + end) / 2.0 - pair / 2.0;
    (cover, cover + COVER + PAIR_GAP)
}

/// The bars' heights, each 0 to 1: the song heard here, each bar its own band, low to high; with nothing heard (the
/// song plays on another device, the listening is off or failed), they rest: nothing made up.
pub(crate) fn bars(groove: Option<&Groove>) -> [f32; BARS] {
    // a curve, so only the loud parts reach the top
    groove.and_then(|groove| groove.bands).map(|bands| bands.map(|level| level.powf(1.6))).unwrap_or([0.0; BARS])
}

/// The song's cover and the sound's bars in the `rect` of the compact island (`minis`: Some, with the agents' grid, or
/// None without agents; spread by `spread`) or of the closed notch (`minis` is not asked: the cover at the left, the bars
/// at the right, the wizard between). Standing up on a side (`upright`) the same turned: the cover at the top, the bars at
/// the bottom, each lying down. `cover` is the song's picture (its placeholder without one); `ear` is the listening the
/// bars follow, None while paused. A click on the cover or the bars is true: the island opens on the music's tab. The bars
/// take the cover's two liveliest colours, from the left one to the right one (`spotify::accents`), and go from the
/// colours before to the new ones and the cover of the song before fades under the new one; `tint` is the first colour,
/// for when the tick has not run (Spotify's green until the cover comes). While they move, the next frame is asked for;
/// they are drawn as the island at rest draws them again by itself (`replay::live`).
pub(super) fn spotify(
    ui: &egui::Ui,
    rect: Rect,
    cover: Option<egui::TextureId>,
    tint: Option<Color32>,
    title: &str,
    playing: bool,
    compact: Option<Option<Rect>>,
    upright: bool,
    notch: f32,
    ear: Option<&Ear>,
    time: f64,
) -> bool {
    let painter = ui.painter();
    let (side, corner, bar, gap, tall) = if compact.is_some() {
        (COVER, COVER_CORNER, BAR, BAR_GAP, BARS_TALL)
    } else {
        (COVER_SMALL, COVER_CORNER_SMALL, BAR_SMALL, BAR_GAP_SMALL, BARS_TALL_SMALL)
    };
    let count = if compact.is_some() { BARS } else { BARS.div_ceil(2) };
    let total = count as f32 * bar + (count - 1) as f32 * gap;
    // standing up (`upright`), the cover under the wizard and the bars under it, each bar lying down
    let (cover_left, bars_left) = match compact {
        Some(minis) if upright => spread_upright(rect, minis),
        Some(minis) => spread(rect, minis, notch),
        None if upright => (rect.top() + COVER_LEFT_SMALL, rect.bottom() - BARS_RIGHT_SMALL - total / 2.0),
        None => (rect.left() + COVER_LEFT_SMALL, rect.right() - BARS_RIGHT_SMALL - total / 2.0),
    };
    let art = if upright {
        Rect::from_min_size(Pos2::new(rect.center().x - side / 2.0, cover_left), Vec2::splat(side))
    } else {
        Rect::from_min_size(Pos2::new(cover_left, rect.center().y - side / 2.0), Vec2::splat(side))
    };
    // the cover of the song before stays while the new one is still to come, and goes under it as it fades in
    let fade = CoverFade::read(ui.ctx());
    let progress = fade.progress(time);
    let before = fade.previous.filter(|_| progress < 1.0);
    if let Some(before) = before {
        picture(painter, art, corner, Some(before), title);
    }
    match cover {
        Some(cover) => picture_over(painter, art, corner, cover, title, if before.is_some() { progress } else { 1.0 }),
        // the cover still to come: the one before is what shows; with none, the placeholder
        None if before.is_none() => picture(painter, art, corner, None, title),
        None => {}
    }
    // paused, the dots rest (as the Dynamic Island's), and nothing moves
    let colours = fade.colours_at(time).unwrap_or_else(|| (tint.unwrap_or(GREEN), tint.unwrap_or(GREEN)));
    let row = Row { center: rect.center(), upright, start: bars_left, bar, gap, tall, halves: compact.is_none(), colours };
    let ear = ear.filter(|_| playing).cloned();
    super::replay::live(painter, time, move |painter, _| {
        row.paint(painter, ear.as_ref().map_or([0.0; BARS], |ear| bars(Some(&ear.groove(true)))));
        // at the ambient rate: the screen's kept the whole app painting all song long
        if ear.is_some() {
            painter.ctx().request_repaint_after(crate::app::update::AMBIENT);
        }
    });
    if fade.fading(time) {
        ui.ctx().request_repaint_after(crate::app::update::AMBIENT);
    }
    let bars_area = if upright {
        Rect::from_min_size(Pos2::new(rect.center().x - tall / 2.0, bars_left), Vec2::new(tall, total))
    } else {
        Rect::from_min_size(Pos2::new(bars_left, rect.center().y - tall / 2.0), Vec2::new(total, tall))
    };
    let music = art.union(bars_area);
    ui.interact(music.expand(3.0), ui.scope_id().with("island_pill_music"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked()
}

/// Where the bars go and how they look: along the middle line of the island (`center`; standing up, `upright`, down
/// it), from `start`, each `bar` thick with `gap` between them and `tall` at their highest, in `colours` from the first
/// to the last. On the closed notch (`halves`), which is small, a bar is two bands, the louder of them.
#[derive(Clone, Copy)]
struct Row {
    center: Pos2,
    upright: bool,
    start: f32,
    bar: f32,
    gap: f32,
    tall: f32,
    halves: bool,
    colours: (Color32, Color32),
}

impl Row {
    /// The bars at `levels` (each 0 to 1, a band's).
    fn paint(&self, painter: &Painter, levels: [f32; BARS]) {
        let levels: Vec<f32> = if self.halves { levels.chunks(2).map(|pair| pair.iter().copied().fold(0.0, f32::max)).collect() } else { levels.to_vec() };
        let count = levels.len();
        for (index, level) in levels.into_iter().enumerate() {
            let along_line = self.start + index as f32 * (self.bar + self.gap) + self.bar / 2.0;
            // a round dot at rest, the tallest at its highest, from the middle line up and down (standing up: to the sides)
            let height = self.bar + (self.tall - self.bar) * level.clamp(0.0, 1.0);
            let rise = if self.upright {
                Rect::from_center_size(Pos2::new(self.center.x, along_line), Vec2::new(height, self.bar))
            } else {
                Rect::from_center_size(Pos2::new(along_line, self.center.y), Vec2::new(self.bar, height))
            };
            // the first bar takes the first colour, the last the second, and the ones between them a bit of each
            let along = if count > 1 { index as f32 / (count - 1) as f32 } else { 0.0 };
            painter.rect_filled(rise, self.bar / 2.0, self.colours.0.lerp_to_gamma(self.colours.1, along));
        }
    }
}

/// A picture in `rect`, `alpha` of the way in over the one before it (0 to 1): the fade from a song's cover to the next.
fn picture_over(painter: &Painter, rect: Rect, corner: u8, texture: egui::TextureId, name: &str, alpha: f32) {
    if alpha >= 1.0 {
        return picture(painter, rect, corner, Some(texture), name);
    }
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    painter.add(egui::epaint::RectShape::filled(rect, corner, Color32::WHITE.gamma_multiply(alpha)).with_texture(texture, uv));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::island::{COMPACT_H, COMPACT_TALL_SONG, minis::upright_cells};

    /// Standing up, the cover and the bars go one under the other between the wizard and the minis, with room on each
    /// side, inside the island.
    #[test]
    fn standing_up_the_cover_and_the_bars_go_between_the_wizard_and_the_minis() {
        let island = Rect::from_min_size(Pos2::new(0.0, 100.0), Vec2::new(COMPACT_H, COMPACT_TALL_SONG));
        let grid = upright_cells(island, 2).into_iter().reduce(Rect::union).unwrap();
        let wizard_end = island.top() + crate::app::island::minis::UPRIGHT_INSET + WIZARD / 2.0;
        for minis in [Some(grid), None] {
            let (cover, bars) = spread_upright(island, minis);
            let end = bars + BARS_ROOM;
            let below = minis.map_or(island.bottom(), |grid| grid.top());
            assert!(cover - wizard_end >= 8.0 && below - end >= 8.0, "room around them: {cover} {end} {minis:?}");
            assert!(((cover - wizard_end) - (below - end)).abs() < 1.0, "in the middle");
            assert_eq!(bars, cover + COVER + PAIR_GAP);
        }
        // the bars lying down fit its width
        const { assert!(BARS_TALL < COMPACT_H - 8.0) };
    }

    /// Around a notch nothing goes under it; the cover in the left wing after the wizard, the bars in the right one
    /// before the minis, with room to the notch.
    #[test]
    fn around_a_notch_the_cover_and_the_bars_go_on_the_wings() {
        use crate::app::island::{Shape, around_notch, minis::mini_cells};
        let notch = Vec2::new(179.0, 32.0);
        let island = Rect::from_min_size(Pos2::ZERO, around_notch(Shape::Compact, Vec2::new(188.0, COMPACT_H), notch, true, false));
        let under = Rect::from_center_size(Pos2::new(island.center().x, 16.0), notch);
        for count in [0, 1, 2, 4] {
            let grid = mini_cells(island, count).into_iter().reduce(Rect::union);
            let (cover, bars) = spread(island, grid, notch.x);
            let (cover, bars) = (cover..cover + COVER, bars..bars + BARS_ROOM);
            assert!(cover.start >= WIZARD_AT + WIZARD / 2.0 + 8.0, "after the wizard: {cover:?}");
            assert!(cover.end <= under.left() - 8.0 && bars.start >= under.right() + 8.0, "off the notch: {cover:?} {bars:?}");
            assert!(bars.end <= grid.map_or(island.right() - 12.0, |grid| grid.left() - 8.0), "before the minis: {bars:?} {grid:?}");
        }
        // with no notch, as always
        assert_eq!(spread(Rect::from_min_size(Pos2::ZERO, Vec2::new(188.0, COMPACT_H)), None, 0.0).1 + BARS_ROOM, 188.0 - 12.0);
    }
}
