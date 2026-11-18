//! The island while a song plays: its cover and its bars beside Luna, whatever she shows (her dance, the balloon she
//! thinks in while an agent works) and wherever the island is (on the top edge, standing up on a side).

use super::*;
use crate::app::integrations::spotify::groove::{BANDS, Dance, Groove};
use crate::app::integrations::spotify::{GREEN, Now, Track};
use crate::app::island::{CLOSED_H, CLOSED_W, COMPACT_H, COMPACT_TALL_SONG, EXPANDED_W, Tab};

/// A shape painted: its bounds, its fill, and whether it is a mesh (the cover's placeholder is one).
#[derive(Debug, Clone, Copy)]
pub(super) struct Painted {
    pub(super) rect: Rect,
    pub(super) fill: Color32,
    pub(super) mesh: bool,
}

fn collect(shape: &egui::Shape, into: &mut Vec<Painted>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, into)),
        egui::Shape::Rect(rect) => into.push(Painted { rect: rect.rect, fill: rect.fill, mesh: false }),
        egui::Shape::Circle(circle) => {
            into.push(Painted { rect: Rect::from_center_size(circle.center, Vec2::splat(circle.radius * 2.0)), fill: circle.fill, mesh: false })
        }
        egui::Shape::Mesh(mesh) => {
            let fill = mesh.vertices.first().map_or(Color32::TRANSPARENT, |vertex| vertex.color);
            into.push(Painted { rect: mesh.calc_bounds(), fill, mesh: true });
        }
        _ => {}
    }
}

/// One frame of the island.
pub(super) fn frame(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) -> egui::FullOutput {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, ISLAND_SCREEN)), time: Some(time), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.island_ui(ctx));
    output.textures_delta.clear();
    output
}

/// Every rectangle, circle and mesh a frame painted.
pub(super) fn shapes(output: &egui::FullOutput) -> Vec<Painted> {
    let mut all = Vec::new();
    for clipped in &output.shapes {
        collect(&clipped.shape, &mut all);
    }
    all
}

/// What the song puts on the island: the cover (`side` across with its soft edge, the placeholder's mesh) and the bars,
/// in Spotify's green until a cover gives its colours.
struct Song {
    cover: Rect,
    bars: Vec<Rect>,
}

fn song(all: &[Painted], side: f32) -> Song {
    let near = |length: f32| (length - side).abs() <= 1.5;
    let covers: Vec<Rect> = all.iter().filter(|shape| shape.mesh && near(shape.rect.width()) && near(shape.rect.height())).map(|shape| shape.rect).collect();
    assert_eq!(covers.len(), 1, "one cover of {side}: {covers:?}");
    let bars = all.iter().filter(|shape| !shape.mesh && shape.fill == GREEN).map(|shape| shape.rect).collect();
    Song { cover: covers[0], bars }
}

/// The balloon Luna thinks in while an agent works (`MagoState::Working`'s `…` badge): its light grey disc.
fn balloon(all: &[Painted]) -> Option<Rect> {
    all.iter().find(|shape| !shape.mesh && shape.fill == Color32::from_gray(235)).map(|shape| shape.rect)
}

/// The song heard now: every band up.
pub(super) fn loud() -> Groove {
    Groove { bpm: 120.0, phase: 0.25, energy: 0.9, bass: 0.7, bands: Some([0.7; BANDS]), dance: Dance::Sway, beat: 0.5, since: 0.0 }
}

/// The island on with a song playing and heard here, hung from `anchor` (None: the top centre); `dances`: "A Luna dança
/// com a música".
pub(super) fn playing(anchor: Option<[f32; 2]>, dances: bool) -> (DrawingApp, Context) {
    let mut app = DrawingApp::default();
    app.prefs.island = true;
    app.prefs.integrations.spotify = true;
    app.prefs.integrations.luna_dances = dances;
    app.chats = vec![chat("a", "claude", "Claude Code · loja")];
    app.integrations.spotify.now = Some(Now { track: Track::new("505", "Arctic Monkeys", "", "").unwrap(), playing: true, ..Default::default() });
    if let Some(anchor) = anchor {
        app.island.hang_from(anchor);
    }
    (app, Context::default())
}

/// Frames 50 ms apart from `from` for `seconds`, the song heard all along; the last one.
pub(super) fn settle(app: &mut DrawingApp, ctx: &Context, from: f64, seconds: f64, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut time = from;
    loop {
        app.integrations.spotify.dancer.hear(loud());
        let output = frame(app, ctx, time, events.clone());
        time += 0.05;
        if time >= from + seconds {
            return output;
        }
    }
}

/// The island as the tests draw it: at the top centre of the screen, whatever edge it stands on.
pub(super) fn drawn(app: &DrawingApp) -> Rect {
    let size = app.island.size();
    Rect::from_min_size(Pos2::new((ISLAND_SCREEN.x / 2.0 - size.x / 2.0).round(), 0.0), size)
}

const LEFT: [f32; 2] = [0.0, 400.0];
const RIGHT: [f32; 2] = [ISLAND_SCREEN.x - 1.0, 400.0];

/// Asked: "na ilha, se ela está com o balão de pensamento, a frequência de música não aparece". An agent at work puts
/// the island compact with Luna thinking in her balloon (she does not dance when told not to): the cover and the bars
/// show beside her all the same, the bars up with the song heard, and the balloon on none of them; on the top edge and
/// standing up on either side.
#[test]
fn with_lunas_balloon_the_cover_and_the_bars_still_show() {
    for anchor in [None, Some(LEFT), Some(RIGHT)] {
        let (mut app, ctx) = playing(anchor, false);
        app.chat_runs.insert("a".into(), crate::app::chat::direct::ChatRun::held(app.now));
        let output = settle(&mut app, &ctx, 1.0, 1.0, vec![]);
        let island = drawn(&app);
        let upright = anchor.is_some();
        let compact = if upright { Vec2::new(COMPACT_H, COMPACT_TALL_SONG) } else { Vec2::new(188.0, COMPACT_H) };
        assert_eq!(app.island.size(), compact, "{anchor:?}: compact");
        let all = shapes(&output);
        let balloon = balloon(&all).unwrap_or_else(|| panic!("{anchor:?}: Luna thinks in her balloon"));
        let Song { cover, bars } = song(&all, 23.0);
        assert_eq!(bars.len(), 10, "{anchor:?}: every bar");
        for bar in &bars {
            let (long, thin) = if upright { (bar.width(), bar.height()) } else { (bar.height(), bar.width()) };
            assert!(long > 5.0 && thin == 1.5, "{anchor:?}: up with the song heard: {bar:?}");
            assert!(island.contains_rect(*bar) && !bar.intersects(balloon), "{anchor:?}: inside the island, off the balloon: {bar:?} {balloon:?}");
        }
        assert!(island.contains_rect(cover) && !cover.intersects(balloon), "{anchor:?}: the cover off the balloon: {cover:?} {balloon:?}");
    }
}

/// Asked: "quando está na lateral, não funciona o card e a frequência". Standing up on a side, the closed notch had
/// neither the cover nor the bars: it has them as on the top edge, turned, the cover at the top, Luna in the middle and
/// the bars at the bottom, each lying down, inside the notch.
#[test]
fn on_a_side_the_closed_notch_shows_the_cover_and_the_bars_standing_up() {
    for anchor in [LEFT, RIGHT] {
        let (mut app, ctx) = playing(Some(anchor), true);
        let output = settle(&mut app, &ctx, 1.0, 0.6, vec![]);
        assert_eq!(app.island.size(), Vec2::new(CLOSED_H, CLOSED_W), "{anchor:?}: the notch stands up");
        let island = drawn(&app);
        let Song { cover, bars } = song(&shapes(&output), 17.0);
        assert_eq!(bars.len(), 5, "{anchor:?}: the notch's five bars");
        assert!(island.contains_rect(cover) && cover.center().y < island.center().y - 15.0, "{anchor:?}: the cover at the top: {cover:?} {island:?}");
        for bar in &bars {
            assert!(island.contains_rect(*bar) && bar.center().y > island.center().y + 15.0, "{anchor:?}: the bars at the bottom: {bar:?}");
            assert!(bar.height() == 1.5 && bar.width() > 3.0, "{anchor:?}: lying down, up with the song: {bar:?}");
        }
    }
}

/// Asked ("se eu clicar na música ele me leva para a tab da música"), on a side too: a click on the cover of the closed
/// notch standing up opens the island on Música.
#[test]
fn on_a_side_a_click_on_the_songs_cover_opens_the_music() {
    let (mut app, ctx) = playing(Some(LEFT), true);
    let output = settle(&mut app, &ctx, 1.0, 0.6, vec![]);
    let cover = song(&shapes(&output), 17.0).cover.center();
    let press = |pressed| egui::Event::PointerButton { pos: cover, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(&mut app, &ctx, 1.7, vec![egui::Event::PointerMoved(cover), press(true)]);
    frame(&mut app, &ctx, 1.75, vec![press(false)]);
    settle(&mut app, &ctx, 1.8, 2.0, vec![egui::Event::PointerMoved(cover)]);
    assert_eq!((app.island.size().x, app.island.tab()), (EXPANDED_W, Tab::Music), "open on Música");
}

/// What a frame painted inside `area`, four pixels a point, on a grey desktop, as a PNG at `path`: the island seen
/// without its window, which the scripted `shot` does not catch. Text is left out (the glyphs' atlas is not read).
fn picture(ctx: &Context, output: egui::FullOutput, area: Rect, path: &std::path::Path) {
    use resvg::tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Transform};
    const SCALE: f32 = 4.0;
    let mut pixmap = Pixmap::new((area.width() * SCALE) as u32, (area.height() * SCALE) as u32).unwrap();
    pixmap.fill(resvg::tiny_skia::Color::from_rgba8(110, 112, 118, 255));
    let at = |pos: Pos2| ((pos.x - area.left()) * SCALE, (pos.y - area.top()) * SCALE);
    for primitive in ctx.tessellate(output.shapes, 1.0) {
        let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else { continue };
        let clip = primitive.clip_rect.intersect(area);
        let (left, top) = at(clip.min);
        let Some(clip) = resvg::tiny_skia::Rect::from_xywh(left, top, clip.width() * SCALE, clip.height() * SCALE) else { continue };
        let mut mask = Mask::new(pixmap.width(), pixmap.height()).unwrap();
        mask.fill_path(&PathBuilder::from_rect(clip), FillRule::Winding, false, Transform::identity());
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let corners: Vec<egui::epaint::Vertex> = triangle.iter().map(|index| mesh.vertices[*index as usize]).collect();
            let mut path = PathBuilder::new();
            for (index, corner) in corners.iter().enumerate() {
                let (x, y) = at(corner.pos);
                if index == 0 { path.move_to(x, y) } else { path.line_to(x, y) }
            }
            path.close();
            let Some(path) = path.finish() else { continue };
            // each triangle in the mean of its corners' colours
            let mut sum = [0u32; 4];
            for corner in &corners {
                for (total, channel) in sum.iter_mut().zip(corner.color.to_srgba_unmultiplied()) {
                    *total += u32::from(channel);
                }
            }
            let mut paint = Paint::default();
            paint.set_color_rgba8((sum[0] / 3) as u8, (sum[1] / 3) as u8, (sum[2] / 3) as u8, (sum[3] / 3) as u8);
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), Some(&mask));
        }
    }
    pixmap.save_png(path).unwrap();
}

/// Not a check, a look: `MAGOAGENT_ISLAND_PICTURES=<folder> cargo test -- --ignored island_music::pictures` saves the
/// closed and the compact island with a song, on the top edge and on a side, Luna dancing and in her balloon.
#[test]
#[ignore]
fn pictures() {
    let Some(folder) = std::env::var_os("MAGOAGENT_ISLAND_PICTURES") else { return };
    for (place, anchor) in [("topo", None), ("esquerda", Some(LEFT)), ("direita", Some(RIGHT))] {
        for (shape, working, dances) in [("fechada", false, true), ("compacta", true, true), ("compacta-balao", true, false)] {
            let (mut app, ctx) = playing(anchor, dances);
            if working {
                app.chat_runs.insert("a".into(), crate::app::chat::direct::ChatRun::held(app.now));
            }
            let output = settle(&mut app, &ctx, 1.0, 1.0, vec![]);
            let island = drawn(&app);
            let area = Rect::from_center_size(island.center(), island.size() + Vec2::splat(24.0));
            picture(&ctx, output, area, &std::path::Path::new(&folder).join(format!("{place}-{shape}.png")));
        }
    }
}
