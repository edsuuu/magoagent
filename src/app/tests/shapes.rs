use super::*;

#[test]
fn drawing_uses_press_origin_and_drag_moves_selection() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Rectangle;
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, Pos2::new(300.0, 200.0), Pos2::new(500.0, 350.0));
    assert_eq!(app.scene.elements.len(), 1);
    assert_eq!(app.tool, Tool::Select, "returns to selection after drawing, like the reference app");
    let before = element_bounds(&app.scene.elements[0]);
    assert_eq!(before.size(), Vec2::new(200.0, 150.0));
    drag(&mut app, &ctx, Pos2::new(350.0, 250.0), Pos2::new(420.0, 320.0));
    let after = element_bounds(&app.scene.elements[0]);
    assert!(after.min.x > before.min.x + 40.0);
    app.undo();
    assert_eq!(element_bounds(&app.scene.elements[0]), before);
}

#[test]
fn rapid_gesture_preserves_start_and_end() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Rectangle;
    frame(&mut app, &ctx, vec![]);
    let (start, end) = (Pos2::new(300.0, 200.0), Pos2::new(500.0, 350.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end), button(end, false)]);
    assert_eq!(element_bounds(&app.scene.elements[0]), Rect::from_two_pos(start, end));
}

#[test]
fn corner_handle_resizes_selection() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Rectangle;
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, Pos2::new(300.0, 200.0), Pos2::new(500.0, 350.0));
    // bottom-right handle sits 5px outside the element bounds
    drag(&mut app, &ctx, Pos2::new(505.0, 355.0), Pos2::new(705.0, 505.0));
    assert_eq!(element_bounds(&app.scene.elements[0]), Rect::from_min_max(Pos2::new(300.0, 200.0), Pos2::new(700.0, 500.0)));
    app.undo();
    assert_eq!(element_bounds(&app.scene.elements[0]).size(), Vec2::new(200.0, 150.0));
}

#[test]
fn eraser_shows_a_circle_instead_of_the_cursor() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Eraser;
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(300.0, 300.0))]);
    let icon = frame(&mut app, &ctx, vec![]);
    assert_eq!(icon, egui::CursorIcon::None);
}

#[test]
fn eraser_removes_touched_elements() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.scene.elements = vec![
        app.new_element(ElementKind::Line { points: vec![[300.0, 100.0], [300.0, 500.0]], arrow: false }),
        app.new_element(ElementKind::Line { points: vec![[600.0, 100.0], [600.0, 500.0]], arrow: false }),
    ];
    app.tool = Tool::Eraser;
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, Pos2::new(250.0, 300.0), Pos2::new(350.0, 300.0));
    assert_eq!(app.scene.elements.len(), 1);
    app.undo();
    assert_eq!(app.scene.elements.len(), 2);
}

#[test]
fn hand_tool_pans_without_editing() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Hand;
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, Pos2::new(300.0, 300.0), Pos2::new(400.0, 350.0));
    assert_eq!(app.pan, Vec2::new(100.0, 50.0));
    assert!(app.undo.is_empty());
}

#[test]
fn rough_outline_is_stable_and_irregular() {
    let app = DrawingApp::default();
    let e = app.new_element(ElementKind::Ellipse { x: 0.0, y: 0.0, width: 200.0, height: 100.0 });
    let a = rough_outlines(&e);
    assert_eq!(a, rough_outlines(&e));
    assert_eq!(a.len(), 2);
    let mut smooth = e.clone();
    smooth.style.roughness = 0;
    assert_eq!(rough_outlines(&smooth).len(), 1);
}

#[test]
fn rotation_handle_rotates_and_hit_test_follows() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Rectangle;
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, Pos2::new(300.0, 200.0), Pos2::new(500.0, 350.0));
    // handle sits 5 + 16px above the box, centered
    drag(&mut app, &ctx, Pos2::new(400.0, 179.0), Pos2::new(550.0, 275.0));
    let e = &app.scene.elements[0];
    assert!((e.angle - std::f32::consts::FRAC_PI_2).abs() < 0.01, "angle {}", e.angle);
    let b = element_bounds(e);
    assert!((b.width() - 150.0).abs() < 0.5 && (b.height() - 200.0).abs() < 0.5);
    assert!(e.contains(Pos2::new(325.0, 275.0), 5.0), "rotated left edge");
    assert!(!e.contains(Pos2::new(450.0, 200.0), 5.0), "old top edge is empty now");
    app.undo();
    assert_eq!(app.scene.elements[0].angle, 0.0);
}

#[test]
fn resizing_rotated_element_keeps_opposite_corner() {
    let mut app = DrawingApp::default();
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 100.0, height: 50.0 });
    e.angle = 0.7;
    app.scene.elements = vec![e];
    app.selected = vec![0];
    app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let corners = app.handle_corners().unwrap();
    let ctx = Context::default();
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, corners[2], corners[2] + Vec2::new(40.0, 30.0));
    let after = app.handle_corners().unwrap();
    assert!(after[0].distance(corners[0]) < 0.5, "{:?} vs {:?}", after[0], corners[0]);
    assert!((app.scene.elements[0].angle - 0.7).abs() < 1e-5);
}

#[test]
fn unselected_window_resizes_from_its_corner_or_edge() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 60.0, y: 150.0, width: 300.0, height: 200.0 });
    terminal.terminal = Some(TerminalSpec { name: "t".into(), cwd: "/tmp".into(), command: String::new(), ..Default::default() });
    let mut ide = app.new_element(ElementKind::Rectangle { x: 420.0, y: 150.0, width: 320.0, height: 380.0 });
    ide.ide = Some(IdeSpec { name: "proj".into(), root: "/nonexistent-magoagent".into(), ..Default::default() });
    app.scene.elements = vec![terminal, ide];
    frame(&mut app, &ctx, vec![]);
    // nothing selected: the bottom-right corner of the terminal drags it bigger
    let corner = app.to_screen(Pos2::new(360.0, 350.0));
    drag(&mut app, &ctx, corner, corner + Vec2::new(60.0, 40.0));
    assert_eq!(local_bounds(&app.scene.elements[0]), Rect::from_min_size(Pos2::new(60.0, 150.0), Vec2::new(360.0, 240.0)));
    // the IDE's right edge changes its width, without enlarging it
    let edge = app.to_screen(Pos2::new(740.0, 340.0));
    drag(&mut app, &ctx, edge, edge + Vec2::new(40.0, 0.0));
    assert_eq!(local_bounds(&app.scene.elements[1]), Rect::from_min_size(Pos2::new(420.0, 150.0), Vec2::new(360.0, 380.0)));
    assert_eq!(app.terminal_elevated, None);
    // its top edge changes only the height
    let top = app.to_screen(Pos2::new(600.0, 150.0));
    drag(&mut app, &ctx, top, top + Vec2::new(0.0, -50.0));
    assert_eq!(local_bounds(&app.scene.elements[1]), Rect::from_min_size(Pos2::new(420.0, 100.0), Vec2::new(360.0, 430.0)));
}
