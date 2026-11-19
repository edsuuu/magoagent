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

/// A press at `from` held while the pointer goes 2 and then 12 to the right, and let go: what each frame sent, the
/// frames given by `frame` (events, seconds after the first).
#[cfg(not(target_os = "macos"))]
fn drag_bar(frame: &mut dyn FnMut(Vec<egui::Event>, f64) -> Vec<egui::ViewportCommand>, from: Pos2) -> Vec<egui::ViewportCommand> {
    let mut sent = frame(vec![egui::Event::PointerMoved(from), button(from, true)], 0.0);

    sent.extend(frame(vec![egui::Event::PointerMoved(from + Vec2::new(2.0, 0.0))], 0.02));
    assert!(sent.is_empty(), "2 is not a move yet: {sent:?}");
    sent.extend(frame(vec![egui::Event::PointerMoved(from + Vec2::new(12.0, 0.0))], 0.04));
    sent.extend(frame(vec![button(from + Vec2::new(12.0, 0.0), false)], 0.06));
    sent
}

/// The press is read before the main window's frame, when egui's areas are those of the window it drew last: with
/// the island's drawn in between (its own areas, none on the bar), the bar still moves the window and maximizes.
#[cfg(not(target_os = "macos"))]
#[test]
fn the_island_drawn_in_between_leaves_the_bar_working() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let island = egui::ViewportId::from_hash_of("island");

    app.prefs.sidebar_hidden = true;

    let frame = |app: &mut DrawingApp, time: f64, events: Vec<egui::Event>| {
        let sent = menu_frame(app, &ctx, time, events, true);
        // the island's window, a frame of its own right after
        let viewports = [(island, Default::default())].into_iter().collect();
        let raw = egui::RawInput { viewport_id: island, viewports, time: Some(time + 0.01), ..Default::default() };

        let mut output = ctx.run_ui(raw, |ui| {
            egui::Area::new(egui::Id::unique("island_pill")).fixed_pos(Pos2::ZERO).show(ui.ctx(), |ui| ui.allocate_space(Vec2::new(40.0, 20.0)));
        });

        output.textures_delta.clear();
        sent
    };

    for step in 0..3 {
        frame(&mut app, step as f64 * 0.1, vec![]);
    }

    let empty = Pos2::new(600.0, TITLE_BAR / 2.0);

    assert_eq!(drag_bar(&mut |events, at| frame(&mut app, 1.0 + at, events), empty), vec![egui::ViewportCommand::StartDrag]);
    let mut sent = frame(&mut app, 3.0, vec![egui::Event::PointerMoved(empty), button(empty, true)]);

    sent.extend(frame(&mut app, 3.05, vec![button(empty, false)]));
    sent.extend(frame(&mut app, 3.2, vec![button(empty, true)]));
    sent.extend(frame(&mut app, 3.25, vec![button(empty, false)]));
    assert_eq!(sent, vec![egui::ViewportCommand::Maximized(true)]);
}

/// As in the reference app: ☰ beside the window buttons opens the folded sidebar, where Ajustes is.
#[cfg(target_os = "macos")]
#[test]
fn the_button_beside_the_window_buttons_opens_the_sidebar() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.prefs.sidebar_hidden = true;
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let at = app.top_button(0).center();

    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    frame(&mut app, &ctx, vec![button(at, false)]);
    assert!(!app.prefs.sidebar_hidden, "the sidebar opens");
}

#[test]
fn the_open_sidebar_holds_ajustes_at_its_foot() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let size = Vec2::new(800.0, 600.0);

    let sidebar = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| app.sidebar(&ctx));

        output.textures_delta.clear();
    };

    sidebar(&mut app, vec![]);
    sidebar(&mut app, vec![]);
    // the gear, in the foot's right end
    let at = DrawingApp::sidebar_gear(size.y).center();

    assert!(at.y > size.y - 30.0 && at.x > crate::app::ui::sidebar::SIDEBAR - 40.0 && at.x < crate::app::ui::sidebar::SIDEBAR, "{at:?}");

    let press = |app: &mut DrawingApp| {
        sidebar(app, vec![egui::Event::PointerMoved(at), button(at, true)]);
        sidebar(app, vec![button(at, false)]);
    };

    press(&mut app);
    assert!(app.settings_open, "it opens Ajustes");
    // folded, the sidebar slides away with its gear: nothing answers down there
    (app.settings_open, app.prefs.sidebar_hidden) = (false, true);
    sidebar(&mut app, vec![]);
    assert!(app.sidebar_width() > 0.0, "on its way out, a frame after the fold");
    app.now += 1.0;
    assert_eq!(app.sidebar_width(), 0.0);
    press(&mut app);
    assert!(!app.settings_open, "no gear while folded");
}

/// The sidebar opens and folds along the reference app's spring: fast at first, no bounce.
#[test]
fn the_sidebar_slides_open_and_folded() {
    use crate::app::ui::sidebar::SIDEBAR;
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let draw = |app: &mut DrawingApp| ctx.run_ui(egui::RawInput::default(), |_| app.sidebar(&ctx)).textures_delta.clear();

    draw(&mut app);
    assert_eq!(app.sidebar_width(), SIDEBAR, "open from the first frame, no slide");
    app.prefs.sidebar_hidden = true;
    assert_eq!(app.sidebar_width(), 0.0, "a change no frame drew is already there");
    draw(&mut app);
    assert_eq!(app.sidebar_width(), SIDEBAR, "the slide starts where it stood");
    app.now += 0.1;
    let mid = app.sidebar_width();

    assert!(mid > 0.0 && mid < SIDEBAR / 2.0, "most of the way in a tenth of a second: {mid}");
    // opened again half way: it turns back from where it is
    app.prefs.sidebar_hidden = false;
    draw(&mut app);
    assert_eq!(app.sidebar_width(), mid);
    app.now += 0.5;
    assert_eq!(app.sidebar_width(), SIDEBAR);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn the_title_bar_holds_the_top_buttons_and_moves_the_window() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.prefs.sidebar_hidden = true;
    let mut time = 0.0;

    menu_frame(&mut app, &ctx, time, vec![], true);
    menu_frame(&mut app, &ctx, time, vec![], true);
    // ☰ opens the sidebar and moves past it; there it folds it again
    let menu = app.top_button(0).center();

    press(&mut app, &ctx, &mut time, menu, true);
    assert!(!app.prefs.sidebar_hidden, "☰ opens the sidebar");
    assert_eq!(app.top_button(0).center().x, menu.x + crate::app::ui::sidebar::SIDEBAR);
    menu_frame(&mut app, &ctx, time, vec![], true);
    menu_frame(&mut app, &ctx, time, vec![], true);
    let moved = app.top_button(0).center();

    press(&mut app, &ctx, &mut time, moved, true);
    assert!(app.prefs.sidebar_hidden, "and folds it");
    menu_frame(&mut app, &ctx, time, vec![], true);
    menu_frame(&mut app, &ctx, time, vec![], true);
    // the chat's button beside it; over the chat, the first one is Ajustes' gear
    let chat = app.top_button(1).center();

    press(&mut app, &ctx, &mut time, chat, true);
    assert!(app.chat_open, "the second button opens the chat");
    // the gear took ☰'s place: a new widget answers the pointer a frame later
    menu_frame(&mut app, &ctx, time, vec![], true);
    menu_frame(&mut app, &ctx, time, vec![], true);
    press(&mut app, &ctx, &mut time, menu, true);
    assert!(app.settings_open && app.prefs.sidebar_hidden, "over the chat the first button opens Ajustes");
    press(&mut app, &ctx, &mut time, menu, true);
    press(&mut app, &ctx, &mut time, chat, true);
    assert!(!app.settings_open && !app.chat_open, "and both close");
    menu_frame(&mut app, &ctx, time, vec![], true);
    menu_frame(&mut app, &ctx, time, vec![], true);
    // a press on the empty bar and the pointer going on with the button down move the window, a second press maximizes
    let empty = Pos2::new(300.0, TITLE_BAR / 2.0);

    time += 1.0;
    assert!(press(&mut app, &ctx, &mut time, empty, true).is_empty(), "a click alone is half of a double-click: no move");
    time += 1.0;
    assert_eq!(drag_bar(&mut |events, at| menu_frame(&mut app, &ctx, time + at, events, true), empty), vec![egui::ViewportCommand::StartDrag]);
    // the button already up when the frame gets to the move: none (the window would go on following the pointer)
    time += 1.0;
    let past = empty + Vec2::new(12.0, 0.0);
    let flick = vec![egui::Event::PointerMoved(empty), button(empty, true), egui::Event::PointerMoved(past), button(past, false)];

    assert!(menu_frame(&mut app, &ctx, time, flick, true).is_empty());
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
    let mut sent = press(&mut app, &ctx, &mut time, Pos2::new(840.0, 28.0), true);

    time -= 0.05;
    sent.extend(press(&mut app, &ctx, &mut time, Pos2::new(840.0, 28.0), true));
    assert!(sent.contains(&egui::ViewportCommand::Maximized(true)), "{sent:?}");
    // the window's buttons: close at the right, minimize two to its left (Windows' 46 wide; GNOME's round, on the top row)
    let caption = |k: usize| crate::app::ui::titlebar::caption_button(1200.0, k);

    assert!(caption(0).right() <= 1200.0 && caption(2).right() <= caption(1).left() + 0.5, "{:?}", [caption(0), caption(1), caption(2)]);
    let sent = press(&mut app, &ctx, &mut time, Pos2::new(caption(0).center().x, 10.0), true);

    assert!(sent.contains(&egui::ViewportCommand::Close), "{sent:?}");
    let sent = press(&mut app, &ctx, &mut time, Pos2::new(caption(2).center().x, 10.0), true);

    assert!(sent.contains(&egui::ViewportCommand::Minimized(true)), "{sent:?}");
}

/// Ajustes' veil covers the whole window, the drawn bar too, and took the press: the window did not move while Ajustes
/// was open. The bar's empty stretch under the veil moves it and a second press maximizes, as without Ajustes; Ajustes
/// stays open, and a press on the veil off the bar still closes it.
#[cfg(not(target_os = "macos"))]
#[test]
fn the_title_bar_moves_the_window_with_ajustes_open() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.prefs.sidebar_hidden = true;

    // as in the app: the bar is drawn after Ajustes, which opens over a bar already there
    let frame = |app: &mut DrawingApp, time: f64, events: Vec<egui::Event>| {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let mut raw = egui::RawInput { screen_rect: Some(screen), time: Some(time), events, ..Default::default() };

        app.caption_drag(&ctx, &mut raw);

        let mut output = ctx.run_ui(raw, |_| {
            if app.settings_open {
                app.settings_ui(&ctx);
            }

            app.title_bar(&ctx);
        });

        output.textures_delta.clear();
        output.viewport_output.remove(&egui::ViewportId::ROOT).map(|v| v.commands).unwrap_or_default()
    };

    let mut time = 0.0;

    for _ in 0..2 {
        frame(&mut app, time, vec![]);
    }

    app.settings_open = true;

    for _ in 0..4 {
        time += 0.1;
        frame(&mut app, time, vec![]);
    }

    let empty = Pos2::new(300.0, TITLE_BAR / 2.0);

    assert_eq!(ctx.layer_id_at(empty).map(|layer| layer.id), Some(crate::app::ui::settings::settings_veil()), "the veil is over the bar");

    let press = |app: &mut DrawingApp, time: &mut f64, at: Pos2| {
        let mut sent = frame(app, *time, vec![egui::Event::PointerMoved(at), button(at, true)]);

        sent.extend(frame(app, *time + 0.05, vec![button(at, false)]));
        *time += 0.1;
        sent
    };

    time += 1.0;
    let at = time;

    assert_eq!(drag_bar(&mut |events, step| frame(&mut app, at + step, events), empty), vec![egui::ViewportCommand::StartDrag]);
    assert!(app.settings_open, "the press moved the window, it did not close Ajustes");
    time += 1.0;
    let mut sent = press(&mut app, &mut time, empty);

    time -= 0.05;
    sent.extend(press(&mut app, &mut time, empty));
    assert!(sent.contains(&egui::ViewportCommand::Maximized(true)) && app.settings_open, "{sent:?}");
    // off the bar, the veil is Ajustes' as before: a press there closes it and moves nothing
    time += 1.0;
    let sent = press(&mut app, &mut time, Pos2::new(80.0, 400.0));

    assert!(sent.is_empty() && !app.settings_open, "{sent:?}");
}

/// The welcome covers the whole window, the drawn bar too: the window's buttons stay over it and answer, and the
/// bar's empty stretch under it still moves the window.
#[cfg(not(target_os = "macos"))]
#[test]
fn the_window_buttons_stay_over_the_welcome() {
    let ctx = Context::default();

    crate::app::ui::install_fonts(&ctx, false);
    let mut app = DrawingApp::default();

    app.prefs.sidebar_hidden = true;
    app.welcome = Some(Default::default());

    let frame = |app: &mut DrawingApp, time: f64, events: Vec<egui::Event>| {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let mut raw = egui::RawInput { screen_rect: Some(screen), time: Some(time), events, ..Default::default() };

        app.caption_drag(&ctx, &mut raw);

        let mut output = ctx.run_ui(raw, |_| {
            app.title_bar(&ctx);
            app.welcome_ui(&ctx);
        });

        output.textures_delta.clear();
        output.viewport_output.remove(&egui::ViewportId::ROOT).map(|v| v.commands).unwrap_or_default()
    };

    let mut time = 0.0;

    for _ in 0..3 {
        time += 0.1;
        frame(&mut app, time, vec![]);
    }

    let caption = |k: usize| crate::app::ui::titlebar::caption_button(1200.0, k).center();
    let close = caption(0);

    assert_eq!(ctx.layer_id_at(close).map(|layer| layer.id), Some(egui::Id::unique("title_bar_right")), "Fechar over the welcome");
    assert_eq!(ctx.layer_id_at(Pos2::new(600.0, 400.0)).map(|layer| layer.id), Some(egui::Id::unique("welcome")));

    let press = |app: &mut DrawingApp, time: &mut f64, at: Pos2| {
        let mut sent = frame(app, *time, vec![egui::Event::PointerMoved(at), button(at, true)]);

        sent.extend(frame(app, *time + 0.05, vec![button(at, false)]));
        *time += 1.0;
        sent
    };

    let minimize = caption(2);

    assert_eq!(press(&mut app, &mut time, minimize), vec![egui::ViewportCommand::Minimized(true)]);
    let at = time;

    assert_eq!(drag_bar(&mut |events, step| frame(&mut app, at + step, events), Pos2::new(300.0, TITLE_BAR / 2.0)), vec![egui::ViewportCommand::StartDrag]);
    time += 1.0;
    assert!(press(&mut app, &mut time, close).contains(&egui::ViewportCommand::Close));
    assert!(app.welcome.is_some(), "the welcome stays");
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

/// A panel under the window's edge (the enlarged window's grip asks for Grab, the four-way arrow on
/// Windows) set its cursor after the bar's: the edge's resize arrow has the last word.
#[test]
fn the_window_edge_keeps_its_resize_cursor_over_a_panel() {
    let ctx = Context::default();
    let at = Pos2::new(1198.0, 400.0);

    let raw = || egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
        events: vec![egui::Event::PointerMoved(at)],
        ..Default::default()
    };

    let mut output = ctx.run_ui(raw(), |_| {
        ctx.set_cursor_icon(CursorIcon::Grab);
        crate::app::ui::titlebar::edge_cursor(&ctx);
    });

    output.textures_delta.clear();
    assert_eq!(output.platform_output.cursor_icon, CursorIcon::ResizeEast);
}

/// On Linux the window's buttons are GNOME's, as Chrome draws them: no block of their own, round and gray under the
/// pointer (close too, never red), on the top row with the canvas's buttons.
#[cfg(target_os = "linux")]
#[test]
fn the_linux_window_buttons_are_gnomes_round_ones() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let close = crate::app::ui::titlebar::caption_button(1200.0, 0);

    let shapes = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), events, ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| app.title_bar(&ctx));

        output.textures_delta.clear();
        output.shapes.into_iter().map(|clipped| clipped.shape).collect::<Vec<_>>()
    };

    let red = |shapes: &[Shape]| shapes.iter().any(|shape| matches!(shape, Shape::Rect(rect) if rect.fill == Color32::from_rgb(0xe8, 0x11, 0x23)));

    shapes(&mut app, vec![]);
    let at = Pos2::new(close.center().x, crate::app::ui::TOP_ROW + 16.0);
    let drawn = shapes(&mut app, vec![egui::Event::PointerMoved(at)]);
    let drawn = [drawn, shapes(&mut app, vec![])].concat();

    assert!(!red(&drawn), "close is not Windows' red");
    let round = drawn.iter().any(|shape| matches!(shape, Shape::Circle(circle) if circle.center.distance(at) < 1.0 && (circle.radius - 12.0).abs() < 0.5));

    assert!(round, "a round button under the pointer, on the top row");
    // the three in 8 + 3 × 32 from the window's edge: no 3 × 46 block of Windows
    assert!(1200.0 - crate::app::ui::titlebar::caption_button(1200.0, 2).left() <= 104.5);

    let blocks = drawn
        .iter()
        .filter(|shape| matches!(shape, Shape::Rect(rect) if rect.rect.width() >= 3.0 * 46.0 - 1.0 && rect.rect.top() <= 0.0 && rect.rect.right() >= 1199.0))
        .count();

    assert_eq!(blocks, 0, "no ground of their own");
}
