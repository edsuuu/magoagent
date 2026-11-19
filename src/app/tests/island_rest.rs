//! The island at rest draws itself between the app's frames (`island/replay.rs`): taken down as the app drew it, it
//! shows again as it was, with what moves in it (Luna, the song's bars, the minis) moving on by itself.

use super::island_music::{Painted, frame, loud, playing, settle, shapes};
use super::*;
use crate::app::integrations::spotify::GREEN;
use crate::app::integrations::spotify::groove::{BANDS, Dance, Groove};
use crate::app::island::{CLOSED_H, EXPANDED_W};

/// The song's bars a frame painted (Spotify's green until a cover gives its colours).
fn bars(all: &[Painted]) -> Vec<Rect> {
    all.iter().filter(|shape| !shape.mesh && shape.fill == GREEN).map(|shape| shape.rect).collect()
}

/// The island as its window draws it by itself at `time`: what the app took down last, on a frame of its own.
fn again(app: &DrawingApp, ctx: &Context, time: f64) -> egui::FullOutput {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, ISLAND_SCREEN)), time: Some(time), ..Default::default() };
    let taken = app.island.taken();
    let mut output = ctx.run_ui(raw, |ui| taken.paint(ui, time));
    output.textures_delta.clear();
    output
}

/// The song gone quiet: every band low.
fn quiet() -> Groove {
    Groove { bands: Some([0.1; BANDS]), dance: Dance::Sway, ..loud() }
}

/// Each frame of the island at rest painted the whole app. At rest it is taken down with what moves in it (Luna and the
/// bars), and drawn again it shows as the app drew it, the bars following what is heard at that frame.
#[test]
fn at_rest_the_island_is_drawn_again_with_its_song_moving() {
    let (mut app, ctx) = playing(None, true);
    let drawn = settle(&mut app, &ctx, 1.0, 1.5, vec![]);
    assert!(app.island.at_rest(), "closed, nobody on it");
    assert!(app.island.taken().moving() >= 2, "Luna and the bars move: {}", app.island.taken().moving());
    let (by_app, by_itself) = (shapes(&drawn), shapes(&again(&app, &ctx, 2.6)));
    let body = |all: &[Painted]| all.iter().find(|shape| !shape.mesh && shape.rect.size() == app.island.size()).map(|shape| shape.rect);
    assert!(body(&by_app).is_some() && body(&by_app) == body(&by_itself), "the island where the app drew it");
    let loud_bars = bars(&by_itself);
    assert_eq!(bars(&by_app), loud_bars, "the bars as the app drew them, the song heard the same");
    app.integrations.spotify.dancer.hear(quiet());
    let quiet_bars = bars(&shapes(&again(&app, &ctx, 2.7)));
    assert_eq!(quiet_bars.len(), loud_bars.len(), "every bar");
    assert!(quiet_bars.iter().zip(&loud_bars).all(|(quiet, loud)| quiet.height() < loud.height()), "they follow what is heard now: {quiet_bars:?}");
}

/// The person on it, the island springing open or open, is the app's to draw: it is not at rest then.
#[test]
fn the_person_on_the_island_or_it_springing_is_not_rest() {
    let (mut app, ctx) = playing(None, true);
    settle(&mut app, &ctx, 1.0, 1.5, vec![]);
    assert!(app.island.at_rest());
    let on_it = Pos2::new(ISLAND_SCREEN.x / 2.0, CLOSED_H / 2.0);
    frame(&mut app, &ctx, 2.5, vec![egui::Event::PointerMoved(on_it)]);
    frame(&mut app, &ctx, 2.55, vec![]);
    assert!(!app.island.at_rest(), "the pointer on it");
    let press = |pressed| egui::Event::PointerButton { pos: on_it, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(&mut app, &ctx, 2.6, vec![press(true)]);
    frame(&mut app, &ctx, 2.65, vec![press(false)]);
    frame(&mut app, &ctx, 2.7, vec![egui::Event::PointerGone]);
    frame(&mut app, &ctx, 2.75, vec![]);
    assert!(!app.island.at_rest(), "springing open");
    settle(&mut app, &ctx, 2.8, 1.0, vec![]);
    assert_eq!(app.island.size().x, EXPANDED_W, "open");
    assert!(!app.island.at_rest(), "open");
}
