use super::*;

#[test]
fn notify_asks_the_taskbar_for_a_look_off_the_mac() {
    let mut app = DrawingApp::default();
    app.notify("ninguem", "pronto");
    assert_eq!(app.attention, !cfg!(target_os = "macos"));
}

#[test]
fn zoom_goes_by_steps_and_the_wheel_by_at_most_0_4() {
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    let mut step = |from: f32, delta: f32| {
        app.zoom = from;
        app.zoom_step(delta);
        app.zoom
    };
    assert_eq!([step(1.0, 0.1), step(1.1, 0.1), step(1.05, -0.1), step(0.1, -0.1), step(3.0, 0.1), step(30.0, 0.1)], [1.1, 1.25, 1.0, 0.1, 4.0, 30.0]);
    let mut wheel = |from: f32, factor: f32| {
        app.zoom = from;
        app.wheel_zoom(factor, Pos2::new(400.0, 300.0));
        (app.zoom * 100.0).round() / 100.0
    };
    assert_eq!([wheel(1.0, 10.0), wheel(1.0, 0.01), wheel(6.0, 10.0), wheel(1.0, 1.1)], [1.4, 0.6, 6.8, 1.1]);
    // Ctrl + wheel: a notch is 10 points of zoom, whatever the zoom (the same share of it past 300%)
    let mut notch = |from: f32, notches: f32| {
        app.zoom = from;
        app.wheel_step(notches, Pos2::new(400.0, 300.0));
        (app.zoom * 100.0).round() / 100.0
    };
    assert_eq!([notch(1.0, 1.0), notch(0.5, 1.0), notch(2.0, -1.0), notch(1.0, 9.0), notch(6.0, 1.0)], [1.1, 0.6, 1.9, 1.4, 6.2]);
}

#[test]
fn a_jump_to_a_node_glides_there_unless_the_camera_is_instant() {
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    let target = Pos2::new(2000.0, 1000.0);
    app.glide_to(target);
    assert_eq!(app.pan, Vec2::ZERO, "it sets off from where it is");
    app.now += 0.1;
    app.step_glide();
    let middle = app.to_screen(target);
    assert!(middle.x > 400.0 && middle.x < 2000.0, "on its way: {middle:?}");
    app.now += 1.0;
    app.step_glide();
    assert!((app.to_screen(target) - app.canvas_rect.center()).length() < 0.01);
    assert!(app.glide.is_none());
    // a zoom on the way stops it
    app.glide_to(Pos2::ZERO);
    app.zoom_at(2.0, Pos2::new(400.0, 300.0));
    assert!(app.glide.is_none());
}

#[test]
fn a_selected_card_crawls_at_sixteen_frames_not_every_vsync() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "Plano".into());
    app.scene.elements.extend([note, label]);
    app.selected = vec![0];
    for _ in 0..3 {
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    }
    painted(&app, &ctx);
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    assert!(!ctx.requested_repaint_last_pass(), "no frame right away");
    assert!(ctx.repaint_causes().iter().any(|c| c.file.ends_with("paint.rs")), "but one soon, for the dashes");
}

#[test]
fn a_selected_card_off_the_screen_asks_for_no_frames() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(5000.0, 5000.0), Vec2::splat(260.0)), "Longe".into());
    app.scene.elements.extend([note, label]);
    app.selected = vec![0];
    for _ in 0..3 {
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    }
    painted(&app, &ctx);
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    assert!(!ctx.repaint_causes().iter().any(|c| c.file.ends_with("paint.rs")), "nobody sees its dashes");
}

#[test]
fn a_card_off_the_screen_or_under_the_chat_is_not_painted() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let empty = painted(&app, &ctx);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(5000.0, 5000.0), Vec2::splat(260.0)), "Longe".into());
    app.scene.elements.extend([note, label]);
    assert_eq!(painted(&app, &ctx), empty, "off the screen");
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 300.0), Vec2::splat(200.0)), "Perto".into());
    app.scene.elements.extend([note, label]);
    assert!(painted(&app, &ctx) > empty, "on it");
    app.chat_open = true;
    let ground = painted(&app, &ctx);
    assert!(ground <= empty, "under the open chat");
    assert_eq!(ground, 1, "its ground alone");
    // a screen of its own: not even the grid or a shape in the strip above it
    app.prefs.grid = true;
    let shape = app.new_element(ElementKind::Rectangle { x: 10.0, y: 10.0, width: 100.0, height: 30.0 });
    app.scene.elements.push(shape);
    assert_eq!(painted(&app, &ctx), ground, "only its ground");
}

#[test]
fn minimized_the_app_keeps_painting_unseen() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut asks_to_paint = |minimized| {
        let mut raw = egui::RawInput::default();
        let info = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
        (info.minimized, info.occluded) = (Some(minimized), Some(false));
        let out = ctx.run_logic(&raw, |c| eframe::App::logic(&mut app, c, &mut eframe::Frame::_new_kittest()));
        out.viewport_commands.into_values().flatten().any(|c| matches!(c, egui::ViewportCommand::RequestPaintWhileHidden))
    };
    assert!(asks_to_paint(true));
    assert!(!asks_to_paint(true), "at most every 400 ms");
    assert!(!asks_to_paint(false));
}
