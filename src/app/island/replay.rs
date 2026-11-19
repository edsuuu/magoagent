//! The island at rest draws itself between the app's frames. Its window is drawn in the app's own pass (an immediate
//! viewport), so each frame of Luna, of the song's bars or of the minis painted the whole app's window too: on a laptop
//! with no graphics card (Mesa's llvmpipe) that window was nine in ten of the work, 118% of a core for a closed island
//! with a song. At rest (`Island::at_rest`), the frame the app draws is taken down as it was (`begin`, `end`) with what
//! moves in it (`live`), and the island's window draws that again by itself (`frame`, a deferred viewport) until the app
//! draws again (its own frames) or the person does anything on it (which wakes the app).

use crate::prelude::*;
use egui::{LayerId, Ui};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};

/// What moves draws itself at a frame's time.
type Paint = Box<dyn Fn(&Painter, f64) + Send + Sync>;

/// A thing that moves, drawn this frame: the shapes it drew (`start..end` of `layer`'s), and how to draw it again.
struct Live {
    layer: LayerId,
    start: usize,
    end: usize,
    clip: Rect,
    opacity: f32,
    paint: Paint,
}

/// A part of the island as drawn: shapes as they were, or a thing drawn again each frame.
enum Piece {
    Still(Vec<egui::epaint::ClippedShape>),
    Moving { clip: Rect, opacity: f32, paint: Paint },
}

/// The island at rest as the app last drew it, in the order it was drawn.
#[derive(Default)]
pub(crate) struct Replay {
    pieces: Vec<Piece>,
}

thread_local! {
    /// While the app draws an island at rest: the things that moved in it, in the order they were drawn.
    static TAKING: RefCell<Option<Vec<Live>>> = const { RefCell::new(None) };
}

/// The frames the island's window drew by itself (`MAGOAGENT_PERF` counts them).
pub(crate) static FRAMES: AtomicU32 = AtomicU32::new(0);

/// Draws what moves by itself (`paint` at `time`) and, while an island at rest is taken down (`begin`), keeps it to be
/// drawn again at each frame's time. `paint` asks for its own next frame, as the island's things do.
pub(crate) fn live(painter: &Painter, time: f64, paint: impl Fn(&Painter, f64) + Send + Sync + 'static) {
    let layer = painter.layer_id();
    let next = || painter.ctx().graphics_mut(|graphics| graphics.entry(layer).next_idx().0);
    let taking = TAKING.with(|taking| taking.borrow().is_some());
    let start = if taking { next() } else { 0 };

    paint(painter, time);

    if taking {
        let live = Live { layer, start, end: next(), clip: painter.clip_rect(), opacity: painter.opacity(), paint: Box::new(paint) };

        TAKING.with(|taking| {
            if let Some(lives) = taking.borrow_mut().as_mut() {
                lives.push(live);
            }
        });
    }
}

/// The app is about to draw the island: at `rest`, it is taken down (`end`).
pub(crate) fn begin(rest: bool) {
    TAKING.with(|taking| *taking.borrow_mut() = rest.then(Vec::new));
}

/// The island just drawn on `layer` (`begin`), as it can be drawn again: None when it was not at rest, or when something
/// else was drawn too (a tip, a menu), which it does not draw by itself.
pub(crate) fn end(ctx: &Context, layer: LayerId) -> Option<Replay> {
    let mut lives = TAKING.with(|taking| taking.borrow_mut().take())?;
    let layers: Vec<LayerId> = ctx.memory(|memory| memory.layer_ids().collect());
    let alone = ctx.graphics(|graphics| layers.iter().all(|other| *other == layer || graphics.get(*other).is_none_or(|list| list.is_empty())));

    if !alone || lives.iter().any(|live| live.layer != layer) {
        return None;
    }

    let shapes: Vec<egui::epaint::ClippedShape> = ctx.graphics(|graphics| graphics.get(layer).map(|list| list.all_entries().cloned().collect()))?;

    // a thing drawn inside another (kept after it, as it ends first) is the outer one's
    lives.sort_by_key(|live| (live.start, std::cmp::Reverse(live.end)));
    let mut pieces = Vec::new();
    let mut at = 0;

    for live in lives {
        if live.start < at || live.end > shapes.len() {
            continue;
        }

        pieces.push(Piece::Still(shapes[at..live.start].to_vec()));
        pieces.push(Piece::Moving { clip: live.clip, opacity: live.opacity, paint: live.paint });
        at = live.end;
    }

    pieces.push(Piece::Still(shapes[at..].to_vec()));
    Some(Replay { pieces })
}

impl Replay {
    /// How many things move in it.
    #[cfg(test)]
    pub(crate) fn moving(&self) -> usize {
        self.pieces.iter().filter(|piece| matches!(piece, Piece::Moving { .. })).count()
    }

    /// Draws it on `ui`'s layer: the shapes as they were, what moves at `time`.
    pub(crate) fn paint(&self, ui: &Ui, time: f64) {
        let layer = ui.layer_id();

        for piece in &self.pieces {
            match piece {
                Piece::Still(shapes) => ui.ctx().graphics_mut(|graphics| {
                    let list = graphics.entry(layer);

                    for shape in shapes {
                        list.add(shape.clip_rect, shape.shape.clone());
                    }
                }),
                Piece::Moving { clip, opacity, paint } => {
                    let mut painter = Painter::new(ui.ctx().clone(), layer, *clip);

                    painter.set_opacity(*opacity);
                    paint(&painter, time);
                }
            }
        }
    }
}

/// A frame the island's window draws by itself (a deferred viewport, `DrawingApp::island_window`): what the app took
/// down, what moves at this frame's time. Anything on the window (the pointer, a key, the focus) wakes the app, which
/// draws the island from then on.
pub(crate) fn frame(ui: &mut Ui, replay: &Mutex<Replay>) {
    let ctx = ui.ctx().clone();

    let touched = ctx.input(|input| {
        !input.events.is_empty() || !input.raw.hovered_files.is_empty() || !input.raw.dropped_files.is_empty() || input.viewport().close_requested()
    });

    if touched {
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    let time = ctx.input(|input| input.time);

    replay.lock().unwrap_or_else(PoisonError::into_inner).paint(ui, time);
    FRAMES.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame on `ctx` at `time`; the shapes it painted.
    fn pass(ctx: &Context, time: f64, draw: impl FnMut(&mut Ui)) -> Vec<egui::epaint::ClippedShape> {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))), time: Some(time), ..Default::default() };

        ctx.run_ui(raw, draw).shapes
    }

    /// A dot that goes right as time goes on.
    fn dot(painter: &Painter, time: f64) {
        painter.circle_filled(Pos2::new(10.0 + time as f32, 50.0), 2.0, Color32::RED);
    }

    fn centres(shapes: &[egui::epaint::ClippedShape]) -> Vec<Pos2> {
        shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Rect(rect) => Some(rect.rect.center()),
                Shape::Circle(circle) => Some(circle.center),
                _ => None,
            })
            .collect()
    }

    /// The island at rest drawn again: what stood still where it was, what moves at the frame's time, in the order they
    /// were drawn (what moves between the things around it).
    #[test]
    fn drawn_again_the_still_shapes_stay_and_what_moves_moves_on_in_its_place() {
        let ctx = Context::default();
        let mut taken = None;

        let first = pass(&ctx, 1.0, |ui| {
            let layer = ui.layer_id();

            begin(true);
            ui.painter().rect_filled(Rect::from_center_size(Pos2::new(5.0, 5.0), Vec2::splat(4.0)), 0, Color32::BLACK);
            live(ui.painter(), 1.0, dot);
            ui.painter().rect_filled(Rect::from_center_size(Pos2::new(90.0, 90.0), Vec2::splat(4.0)), 0, Color32::BLACK);
            taken = end(ui.ctx(), layer);
        });

        let taken = taken.expect("at rest, taken down");

        assert_eq!(taken.moving(), 1);
        assert_eq!(centres(&first), vec![Pos2::new(5.0, 5.0), Pos2::new(11.0, 50.0), Pos2::new(90.0, 90.0)]);
        let later = pass(&ctx, 4.0, |ui| taken.paint(ui, 4.0));

        assert_eq!(centres(&later), vec![Pos2::new(5.0, 5.0), Pos2::new(14.0, 50.0), Pos2::new(90.0, 90.0)]);
    }

    /// Not at rest, or with something else drawn too (a menu, a tip), nothing is taken down: the app draws it.
    #[test]
    fn not_at_rest_or_with_something_else_drawn_nothing_is_taken_down() {
        let ctx = Context::default();
        let mut taken = Vec::new();

        for (rest, menu) in [(false, false), (true, true)] {
            pass(&ctx, 1.0, |ui| {
                let layer = ui.layer_id();

                begin(rest);
                live(ui.painter(), 1.0, dot);

                if menu {
                    egui::Area::new(egui::Id::unique("menu")).show(ui.ctx(), |ui| ui.painter().circle_filled(Pos2::new(30.0, 30.0), 3.0, Color32::WHITE));
                }

                taken.push(end(ui.ctx(), layer).is_some());
            });
        }

        assert_eq!(taken, vec![false, false]);
    }
}
