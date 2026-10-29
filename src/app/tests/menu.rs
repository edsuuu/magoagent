use super::*;

/// One frame of the ☰ menu alone, at `time`; gives the window commands it sent.
fn menu_frame(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>, bar: bool) -> Vec<egui::ViewportCommand> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), time: Some(time), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| {
        if bar {
            app.title_bar(ctx);
        } else if app.menu_open {
            app.main_menu(ctx, BUTTON);
        }
    });
    output.textures_delta.clear();
    output.viewport_output.remove(&egui::ViewportId::ROOT).map(|v| v.commands).unwrap_or_default()
}

const BUTTON: Rect = Rect { min: Pos2::new(16.0, 20.0), max: Pos2::new(52.0, 56.0) };

/// The middle of row `i` of a menu area: 8 of margin and 1 of border, then 32 high rows 1 apart.
fn row(ctx: &Context, area: &str, i: usize) -> Pos2 {
    let rect = ctx.memory(|m| m.area_rect(egui::Id::unique(area))).unwrap_or_else(|| panic!("{area} is showing"));
    Pos2::new(rect.left() + 60.0, rect.top() + 25.0 + 33.0 * i as f32)
}

fn press(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2, bar: bool) -> Vec<egui::ViewportCommand> {
    let mut sent = menu_frame(app, ctx, *time, vec![egui::Event::PointerMoved(at), button(at, true)], bar);
    sent.extend(menu_frame(app, ctx, *time + 0.05, vec![button(at, false)], bar));
    *time += 0.1;
    sent
}

#[test]
fn the_menu_opens_a_section_on_hover_and_runs_its_rows() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut time = 0.0;
    app.toggle_menu();
    // a new area takes a frame to size itself and one shown before it answers the pointer
    menu_frame(&mut app, &ctx, time, vec![], false);
    menu_frame(&mut app, &ctx, time, vec![], false);
    assert_eq!(MENU_SECTIONS, ["Arquivo", "Editar", "Visualizar", "Ir", "Ajuda"]);
    for i in 0..MENU_SECTIONS.len() {
        let at = row(&ctx, "main_menu_popup", i);
        menu_frame(&mut app, &ctx, time, vec![egui::Event::PointerMoved(at)], false);
        assert_eq!(app.menu_section, Some(i), "hovering {} opens it", MENU_SECTIONS[i]);
    }
    // Visualizar › Preferências: the third level, beside the section
    menu_frame(&mut app, &ctx, time, vec![egui::Event::PointerMoved(row(&ctx, "main_menu_popup", 2))], false);
    let prefs = row(&ctx, "main_menu_section", 4) + Vec2::new(0.0, 10.0);
    menu_frame(&mut app, &ctx, time, vec![egui::Event::PointerMoved(prefs)], false);
    assert!(app.prefs_open, "Preferências opens on hover");
    // Ir › Chat runs and closes the menu
    menu_frame(&mut app, &ctx, time, vec![egui::Event::PointerMoved(row(&ctx, "main_menu_popup", 3))], false);
    assert!(!app.prefs_open, "another section closes Preferências");
    menu_frame(&mut app, &ctx, time, vec![], false);
    press(&mut app, &ctx, &mut time, row(&ctx, "main_menu_section", 2), false);
    assert!(app.chat_open, "Ir › Chat opens the chat");
    assert!(!app.menu_open && app.menu_section.is_none());
    // a press anywhere else closes it, submenus and all
    app.toggle_menu();
    menu_frame(&mut app, &ctx, time, vec![egui::Event::PointerMoved(row(&ctx, "main_menu_popup", 1))], false);
    assert_eq!(app.menu_section, Some(1));
    press(&mut app, &ctx, &mut time, Pos2::new(900.0, 700.0), false);
    assert!(!app.menu_open && app.menu_section.is_none());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn the_title_bar_holds_the_menu_and_moves_the_window() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut time = 0.0;
    menu_frame(&mut app, &ctx, time, vec![], true);
    menu_frame(&mut app, &ctx, time, vec![], true);
    press(&mut app, &ctx, &mut time, DrawingApp::title_menu_button().center(), true);
    assert!(app.menu_open, "☰ in the bar opens the menu");
    press(&mut app, &ctx, &mut time, DrawingApp::title_menu_button().center(), true);
    assert!(!app.menu_open, "and closes it");
    // a double-click on the empty bar maximizes
    let empty = Pos2::new(600.0, TITLE_BAR / 2.0);
    time += 1.0;
    let mut sent = press(&mut app, &ctx, &mut time, empty, true);
    time -= 0.05;
    sent.extend(press(&mut app, &ctx, &mut time, empty, true));
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
