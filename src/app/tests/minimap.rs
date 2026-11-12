use super::*;
use crate::app::canvas::minimap::{MapDot, map_scale, to_world};

/// A terminal at (1000, 0) and a note at the origin, 800x600 on screen at 100%.
fn scene() -> (Context, DrawingApp) {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut term = app.new_element(ElementKind::Rectangle { x: 1000.0, y: 0.0, width: 400.0, height: 300.0 });
    term.terminal = Some(TerminalSpec::default());
    let mut note = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 200.0, height: 200.0 });
    note.created = Some(0);
    app.scene.elements = vec![term, note];
    app.prefs.minimap = true;
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    (ctx, app)
}

fn surface(ctx: &Context) -> Rect {
    ctx.memory(|m| m.area_rect(egui::Id::unique("minimap"))).unwrap().shrink(4.875)
}

fn center(app: &DrawingApp) -> Pos2 {
    app.visible_world().center()
}

#[test]
fn the_map_covers_the_nodes_and_the_view() {
    let (_, app) = scene();
    let world = app.minimap_world();
    // the nodes' box (0..1400 x 0..300) with 200 around it, and the view (0..800 x 0..600)
    assert_eq!((world.min, world.max), (Pos2::new(-200.0, -200.0), Pos2::new(1600.0, 600.0)));
    assert!((map_scale(world) - (168.0_f32 / 1800.0).min(118.0 / 800.0)).abs() < 1e-6);
    // a notes' paper at 0.8 and a window in the accent
    let nodes = app.minimap_nodes();
    assert_eq!((nodes[0].1, nodes[1].1), (MapDot::Window, MapDot::Paper(STICKY_PAPER)));
    // a drawing shows too, as an outline, and widens what the map covers
    let mut app = app;
    let shape = app.new_element(ElementKind::Ellipse { x: -900.0, y: 0.0, width: 100.0, height: 100.0 });
    app.scene.elements.push(shape);
    assert_eq!(app.minimap_nodes()[2].1, MapDot::Shape);
    assert_eq!(app.minimap_world().min.x, -1100.0);
}

#[test]
fn a_press_on_the_map_takes_the_view_there_and_a_drag_follows_with_the_area_held() {
    let (ctx, mut app) = scene();
    let (area, s) = (app.minimap_world(), surface(&ctx));
    let a = s.min + Vec2::new(60.0, 50.0);
    let b = a + Vec2::new(30.0, 10.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a), button(a, true)]);
    assert!((center(&app) - to_world(area, s.min, a)).length() < 1.0, "{:?}", center(&app));
    // the view moved, but the map's area stays as it was under the pointer
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(b)]);
    assert!((center(&app) - to_world(area, s.min, b)).length() < 1.0);
    assert!(!app.minimap_area.settling());
    frame(&mut app, &ctx, vec![button(b, false)]);
    assert!(app.minimap_area.settling(), "it settles after the release");
    frame_at(&mut app, &ctx, Some(5.0), vec![]);
    assert!(!app.minimap_area.settling());
}

#[test]
fn the_arrows_move_a_fifth_of_the_view() {
    let (ctx, mut app) = scene();
    let a = surface(&ctx).center();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a), button(a, true)]);
    frame(&mut app, &ctx, vec![button(a, false)]);
    frame_at(&mut app, &ctx, Some(5.0), vec![]);
    let before = center(&app);
    let key = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame_at(&mut app, &ctx, Some(5.1), vec![key(Key::ArrowRight)]);
    assert!((center(&app) - before - Vec2::new(160.0, 0.0)).length() < 1.0, "{:?} {before:?}", center(&app));
    frame_at(&mut app, &ctx, Some(5.2), vec![key(Key::ArrowUp)]);
    assert!((center(&app).y - before.y + 120.0).abs() < 1.0);
}

#[test]
fn the_x_closes_it() {
    let (ctx, mut app) = scene();
    let at = ctx.memory(|m| m.area_rect(egui::Id::unique("minimap"))).unwrap().right_top() + Vec2::new(-16.0, 16.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    frame(&mut app, &ctx, vec![button(at, false)]);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.prefs.minimap);
}

#[test]
fn dragging_the_percent_scrubs_the_zoom_and_a_click_resets_it() {
    let (ctx, mut app) = scene();
    let at = ctx.memory(|m| m.area_rect(egui::Id::unique("footer"))).unwrap().center();
    app.prefs.minimap = false;
    drag(&mut app, &ctx, at, at + Vec2::new(60.0, 0.0));
    assert!(app.zoom > 1.2 && ((app.zoom / 0.05).round() * 0.05 - app.zoom).abs() < 1e-3, "{}", app.zoom);
    drag(&mut app, &ctx, at, at + Vec2::new(-400.0, 0.0));
    assert!((app.zoom - 0.1).abs() < 1e-3, "{}", app.zoom);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    frame(&mut app, &ctx, vec![button(at, false)]);
    assert_eq!(app.zoom, 1.0);
}
