use super::*;

/// One frame of the drawn title bar alone (off the Mac), at `time`; gives the window commands it sent.
#[cfg(not(target_os = "macos"))]
fn menu_frame(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>, bar: bool) -> Vec<egui::ViewportCommand> {
    let mut raw =
        egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), time: Some(time), events, ..Default::default() };
    if bar {
        app.caption_drag(ctx, &mut raw);
    }
    let mut output = ctx.run_ui(raw, |_| app.title_bar(ctx));
    output.textures_delta.clear();
    output.viewport_output.remove(&egui::ViewportId::ROOT).map(|v| v.commands).unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
fn press(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2, bar: bool) -> Vec<egui::ViewportCommand> {
    let mut sent = menu_frame(app, ctx, *time, vec![egui::Event::PointerMoved(at), button(at, true)], bar);
    sent.extend(menu_frame(app, ctx, *time + 0.05, vec![button(at, false)], bar));
    *time += 0.1;
    sent
}

/// No ☰ menu, as in the reference app: the button beside the window buttons opens Ajustes.
#[cfg(target_os = "macos")]
#[test]
fn the_button_beside_the_window_buttons_opens_ajustes() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let at = Pos2::new(MENU_LEFT + 18.0, TOP_ROW + 22.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    frame(&mut app, &ctx, vec![button(at, false)]);
    assert!(app.settings_open, "Ajustes opens");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn the_title_bar_holds_ajustes_and_moves_the_window() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut time = 0.0;
    menu_frame(&mut app, &ctx, time, vec![], true);
    menu_frame(&mut app, &ctx, time, vec![], true);
    press(&mut app, &ctx, &mut time, DrawingApp::title_settings_button().center(), true);
    assert!(app.settings_open, "the button in the bar opens Ajustes");
    press(&mut app, &ctx, &mut time, DrawingApp::title_settings_button().center(), true);
    assert!(!app.settings_open, "and closes it");
    // a press on the empty bar moves the window at once (no waiting for a drag), a second one maximizes
    let empty = Pos2::new(300.0, TITLE_BAR / 2.0);
    time += 1.0;
    let sent = press(&mut app, &ctx, &mut time, empty, true);
    assert_eq!(sent, vec![egui::ViewportCommand::StartDrag]);
    time += 1.0;
    let mut sent = press(&mut app, &ctx, &mut time, empty, true);
    time -= 0.05;
    sent.extend(press(&mut app, &ctx, &mut time, empty, true));
    assert!(sent.contains(&egui::ViewportCommand::Maximized(true)), "{sent:?}");
    // the toolbar sits in the bar: its stretch is the toolbar's, the bar goes on past it
    app.toolbar_span = Rect::from_min_max(Pos2::new(500.0, 4.0), Pos2::new(700.0, 52.0));
    menu_frame(&mut app, &ctx, time, vec![], true);
    // a moved area answers the pointer at its new place a frame later
    menu_frame(&mut app, &ctx, time, vec![], true);
    time += 1.0;
    let mut sent = press(&mut app, &ctx, &mut time, Pos2::new(600.0, 28.0), true);
    time -= 0.05;
    sent.extend(press(&mut app, &ctx, &mut time, Pos2::new(600.0, 28.0), true));
    assert!(sent.is_empty(), "{sent:?}");
    time += 1.0;
    let mut sent = press(&mut app, &ctx, &mut time, Pos2::new(900.0, 28.0), true);
    time -= 0.05;
    sent.extend(press(&mut app, &ctx, &mut time, Pos2::new(900.0, 28.0), true));
    assert!(sent.contains(&egui::ViewportCommand::Maximized(true)), "{sent:?}");
    // the caption buttons: close is the last 46 of the bar, minimize two to its left
    let sent = press(&mut app, &ctx, &mut time, Pos2::new(1200.0 - 23.0, 10.0), true);
    assert!(sent.contains(&egui::ViewportCommand::Close), "{sent:?}");
    let sent = press(&mut app, &ctx, &mut time, Pos2::new(1200.0 - 46.0 * 2.5, 10.0), true);
    assert!(sent.contains(&egui::ViewportCommand::Minimized(true)), "{sent:?}");
}

#[test]
fn the_window_edges_resize_it() {
    use egui::ResizeDirection as R;
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
    let at = |x, y| resize_edge(screen, Pos2::new(x, y)).map(|(d, _)| d);
    assert_eq!(at(500.0, 300.0), None);
    assert_eq!(at(500.0, 2.0), Some(R::North));
    assert_eq!(at(998.0, 300.0), Some(R::East));
    assert_eq!(at(500.0, 699.0), Some(R::South));
    assert_eq!(at(0.0, 300.0), Some(R::West));
    assert_eq!(at(2.0, 10.0), Some(R::NorthWest), "a corner reaches along the side");
    assert_eq!(at(990.0, 698.0), Some(R::SouthEast));
    assert_eq!(at(-1.0, 300.0), None, "outside the window");
}
