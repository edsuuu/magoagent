//! The pill while Spotify plays (asked: "como a Dynamic Island do iPhone quando toca música"): the song's cover beside
//! the wizard, at the left, and the sound's bars at the right, rising and falling with it. Paused or with nothing
//! loaded, the pill shows what it always did (`paint::island_small`).

use crate::prelude::*;

use super::music::{GREEN, picture};

use crate::app::integrations::spotify::CoverFade;
use crate::app::integrations::spotify::groove::Groove;

/// The side of the cover and its corner on the compact island.
const COVER: f32 = 22.0;
const COVER_CORNER: u8 = 6;
/// The wizard on the compact island: its middle from the left edge and its size (`paint::island_small`); the last item
/// ends as far from the right edge as the wizard starts from the left.
const WIZARD_AT: f32 = super::COMPACT_SIDE;
const WIZARD: f32 = 30.0;
/// The same on the closed notch, which is smaller.
const COVER_SMALL: f32 = 16.0;
const COVER_CORNER_SMALL: u8 = 5;
const COVER_LEFT_SMALL: f32 = 4.0;

/// The bars: how many, the side of each one and the room between them, how far their middle is from the right edge,
/// and the tallest one.
const BARS: usize = crate::app::integrations::spotify::groove::BANDS;
// asked: "os palitinhos da frequência estão muito grossos, daria para deixar mais finos"
const BAR: f32 = 1.5;
const BAR_GAP: f32 = 3.0;
const BAR_SMALL: f32 = 1.0;
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
/// wizard is from the left (asked: "Luna à esquerda, capa e frequência no meio, os mini agentes à direita").
pub(crate) fn spread(rect: Rect, minis: Option<Rect>) -> (f32, f32) {
    let pair = COVER + PAIR_GAP + BARS_ROOM;
    let edge = WIZARD_AT - WIZARD / 2.0;
    let cover = match minis {
        Some(grid) => (rect.left() + edge + WIZARD + grid.left()) / 2.0 - pair / 2.0,
        None => rect.right() - edge - pair,
    };
    (cover, cover + COVER + PAIR_GAP)
}

/// The bars' heights at `time`, each 0 to 1: with the `groove` Luna dances to, the song's energy and bass on its beat,
/// each bar a little after the one before it; without it, the clock's slow sway. No two bars rise together.
pub(crate) fn bars(groove: Option<&Groove>, time: f64) -> [f32; BARS] {
    // the song heard: each bar is its own band, low to high (asked: "de acordo com a música no Spotify")
    if let Some(bands) = groove.and_then(|groove| groove.bands) {
        // a curve, so only the loud parts reach the top (asked: "a sensibilidade está muito alta")
        return bands.map(|level| level.powf(1.6));
    }
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

/// The song's cover and the sound's bars in the `rect` of the compact island (`minis`: Some, with the agents' grid, or
/// None without agents; spread by `spread`) or of the closed notch (`minis` is not asked: the cover at the left, the
/// bars at the right, the wizard between). `cover` is the song's picture (its placeholder without one); `groove` is what
/// Luna dances to, None while she doesn't. A click on the cover or the bars is true: the island opens on the music's
/// tab (asked: "se eu clicar na música ele me leva para a tab da música"). The bars take the cover's two liveliest
/// colours, from the left one to the right one (`spotify::accents`), and go from the colours before to the new ones and
/// the cover of the song before fades under the new one (`CoverFade`, asked: "a transição entre músicas está ruim na
/// ilha, e as cores podem ter um degradê"); `tint` is the first colour, for when the tick has not run (Spotify's green
/// until the cover comes). While they move, the next frame is asked for.
pub(super) fn spotify(
    ui: &egui::Ui,
    rect: Rect,
    cover: Option<egui::TextureId>,
    tint: Option<Color32>,
    title: &str,
    playing: bool,
    compact: Option<Option<Rect>>,
    groove: Option<&Groove>,
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
    let (cover_left, bars_left) = match compact {
        Some(minis) => spread(rect, minis),
        None => (rect.left() + COVER_LEFT_SMALL, rect.right() - BARS_RIGHT_SMALL - total / 2.0),
    };
    let art = Rect::from_min_size(Pos2::new(cover_left, rect.center().y - side / 2.0), Vec2::splat(side));
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
    // paused, the dots rest (as the Dynamic Island's), and nothing moves; the closed notch is small: its bars are the
    // bands two by two
    let levels = if playing { bars(groove, time) } else { [0.0; BARS] };
    let levels: Vec<f32> = if compact.is_some() { levels.to_vec() } else { levels.chunks(2).map(|pair| pair.iter().copied().fold(0.0, f32::max)).collect() };
    let colours = fade.colours_at(time).unwrap_or_else(|| (tint.unwrap_or(GREEN), tint.unwrap_or(GREEN)));
    for (index, level) in levels.into_iter().enumerate() {
        let x = bars_left + index as f32 * (bar + gap) + bar / 2.0;
        // a round dot at rest, the tallest at its highest, from the middle line up and down
        let height = bar + (tall - bar) * level.clamp(0.0, 1.0);
        let rise = Rect::from_center_size(Pos2::new(x, rect.center().y), Vec2::new(bar, height));
        // the first bar takes the first colour, the last the second, and the ones between them a bit of each
        let along = if count > 1 { index as f32 / (count - 1) as f32 } else { 0.0 };
        painter.rect_filled(rise, bar / 2.0, colours.0.lerp_to_gamma(colours.1, along));
    }
    if playing || fade.fading(time) {
        ui.ctx().request_repaint_after(crate::app::update::frame_spacing());
    }
    let music = art.union(Rect::from_min_size(Pos2::new(bars_left, rect.center().y - tall / 2.0), Vec2::new(total, tall)));
    ui.interact(music.expand(3.0), ui.scope_id().with("island_pill_music"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked()
}

/// A picture in `rect`, `alpha` of the way in over the one before it (0 to 1): the fade from a song's cover to the next.
fn picture_over(painter: &Painter, rect: Rect, corner: u8, texture: egui::TextureId, name: &str, alpha: f32) {
    if alpha >= 1.0 {
        return picture(painter, rect, corner, Some(texture), name);
    }
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    painter.add(egui::epaint::RectShape::filled(rect, corner, Color32::WHITE.gamma_multiply(alpha)).with_texture(texture, uv));
}
