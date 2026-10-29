use super::*;

mod chat;
mod gestures;
mod layout;
mod links;
mod locks;
mod menu;
mod notes;
mod persist;
mod spaces;

/// One frame of the canvas; gives the cursor it asked for.
fn frame(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) -> egui::CursorIcon {
    frame_at(app, ctx, None, events)
}

/// A frame at a given time, for gestures that depend on timing (double-click).
fn frame_at(app: &mut DrawingApp, ctx: &Context, time: Option<f64>, events: Vec<egui::Event>) -> egui::CursorIcon {
    let mut output =
        ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), time, events, ..Default::default() }, |ui| {
            app.screen_keys(ctx);
            app.handle_shortcuts(ctx);
            app.sync_ides(ctx);
            egui::CentralPanel::no_frame().show(ui, |ui| {
                let (response, _) = ui.allocate_painter(ui.available_size(), Sense::CLICK | Sense::DRAG);
                app.handle_canvas(&response, response.rect);
                app.update_cursor(ctx, &response);
            });
            app.text_editor(ctx);
            app.floating_controls(ctx);
            app.ide_overlay(ctx);
        });
    output.textures_delta.clear();
    output.platform_output.cursor_icon
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

fn drag(app: &mut DrawingApp, ctx: &Context, start: Pos2, end: Pos2) {
    frame(app, ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
    frame(app, ctx, vec![egui::Event::PointerMoved(start + Vec2::new(12.0, 12.0))]);
    frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
    frame(app, ctx, vec![button(end, false)]);
}

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

fn click(app: &mut DrawingApp, ctx: &Context, pos: Pos2) {
    frame(app, ctx, vec![egui::Event::PointerMoved(pos), button(pos, true)]);
    frame(app, ctx, vec![button(pos, false)]);
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

#[test]
fn the_wheel_over_an_ide_card_scrolls_the_panel_only_as_far_as_its_text() {
    let ctx = Context::default();
    let dir = std::env::temp_dir().join(format!("magoagent-ide-wheel-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("long.txt"), (0..80).map(|n| format!("linha {n}\n")).collect::<String>()).unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut e = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: IDE_SIZE.x, height: IDE_SIZE.y });
    e.ide = Some(IdeSpec { name: "proj".into(), root: dir.to_string_lossy().into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    frame(&mut app, &ctx, vec![]);
    let row = card_row_at(&app, 0, 0);
    double_click(&mut app, &ctx, row, 10.0);
    let local = local_bounds(&app.scene.elements[0]);
    // over the panel's text the wheel scrolls it, down to the last line and no further
    let in_panel = Pos2::new(local.left() + 400.0, local.top() + 150.0);
    assert!(app.ide_wheel(in_panel, -40.0));
    assert_eq!((app.ides[&id].panel_scroll, app.ides[&id].card_scroll), (40.0, 0.0));
    assert!(app.ide_wheel(in_panel, -5000.0));
    assert_eq!(app.ides[&id].panel_scroll, 80.0 * crate::ide::card::LINE - (local.height() - 26.0 - 30.0 - 22.0));
    // over the tree, one file has nothing to scroll; outside any card the wheel is the canvas's
    assert!(app.ide_wheel(Pos2::new(local.left() + 100.0, local.top() + 150.0), -40.0));
    assert_eq!(app.ides[&id].card_scroll, 0.0);
    assert!(!app.ide_wheel(Pos2::new(-500.0, -500.0), -40.0));
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(not(target_os = "linux"))]
#[test]
fn a_lifted_portal_hides_the_pages_of_the_other_portals() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut a = app.new_element(ElementKind::Rectangle { x: 60.0, y: 60.0, width: 320.0, height: 240.0 });
    a.portal = Some(PortalSpec { name: "A".into(), url: "about:blank".into(), ..Default::default() });
    let mut b = app.new_element(ElementKind::Rectangle { x: 420.0, y: 60.0, width: 320.0, height: 240.0 });
    b.portal = Some(PortalSpec { name: "B".into(), url: "about:blank".into(), ..Default::default() });
    app.scene.elements = vec![a, b];
    frame(&mut app, &ctx, vec![]);
    assert!(app.portal_page_rect(&app.scene.elements[1]).is_some());
    let (a_id, b_el, a_el) = (app.scene.elements[0].id.clone(), app.scene.elements[1].clone(), app.scene.elements[0].clone());
    app.set_elevated(Some(a_id));
    assert!(app.portal_page_rect(&b_el).is_none(), "B's page would float over the enlarged A");
    assert!(app.portal_page_rect(&a_el).is_some(), "A shows its own page");
}

/// Two presses inside 0.2 s on `at`, from time `t`.
fn double_click(app: &mut DrawingApp, ctx: &Context, at: Pos2, t: f64) {
    for (dt, pressed) in [(0.0, true), (0.05, false), (0.15, true), (0.2, false)] {
        frame_at(app, ctx, Some(t + dt), vec![egui::Event::PointerMoved(at), button(at, pressed)]);
    }
}

#[test]
fn a_double_click_on_a_title_lifts_a_window_and_another_puts_it_back() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let dir = std::env::temp_dir().join(format!("magoagent-lift-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut portal = app.new_element(ElementKind::Rectangle { x: 60.0, y: 60.0, width: 320.0, height: 240.0 });
    portal.portal = Some(PortalSpec { name: "Docs".into(), url: "https://example.com".into(), ..Default::default() });
    let mut ide = app.new_element(ElementKind::Rectangle { x: 60.0, y: 340.0, width: 320.0, height: 200.0 });
    ide.ide = Some(IdeSpec { name: "proj".into(), root: dir.to_string_lossy().into(), ..Default::default() });
    // clear of the context bar, which hangs over the top middle of the canvas
    let [note, text] = app.new_note(Rect::from_min_size(Pos2::new(620.0, 250.0), Vec2::splat(160.0)), "Oi".into());
    app.scene.elements = vec![portal, ide, note, text];
    frame(&mut app, &ctx, vec![]);
    let (portal_id, ide_id, note_id) = (app.scene.elements[0].id.clone(), app.scene.elements[1].id.clone(), app.scene.elements[2].id.clone());
    // the portal: its title strip lifts it, a double-click on its lifted title puts it back
    let at = app.to_screen(Pos2::new(120.0, 70.0));
    double_click(&mut app, &ctx, at, 1.0);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&portal_id));
    frame_at(&mut app, &ctx, Some(2.0), vec![]);
    let lifted = app.terminal_frame(&app.scene.elements[0]).0;
    double_click(&mut app, &ctx, lifted.min + Vec2::new(40.0, 8.0), 3.0);
    assert_eq!(app.terminal_elevated, None);
    // the IDE: its title strip enlarges it and it takes the keyboard; Esc puts it back
    let at = app.to_screen(Pos2::new(120.0, 350.0));
    double_click(&mut app, &ctx, at, 4.0);
    assert_eq!((app.terminal_elevated.as_ref(), app.ide_focus.as_ref()), (Some(&ide_id), Some(&ide_id)));
    let esc = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame_at(&mut app, &ctx, Some(5.0), vec![]);
    frame_at(&mut app, &ctx, Some(6.0), vec![esc]);
    assert_eq!(app.terminal_elevated, None);
    // the note: its header lifts it with its text ready to type; a double-click on the header puts it back
    let at = app.to_screen(Pos2::new(700.0, 260.0));
    double_click(&mut app, &ctx, at, 7.0);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&note_id));
    assert!(app.editing.is_some(), "the lifted note types at once");
    frame_at(&mut app, &ctx, Some(8.0), vec![]);
    let lifted = app.terminal_frame(&app.scene.elements[2]).0;
    double_click(&mut app, &ctx, lifted.min + Vec2::new(20.0, 4.0), 9.0);
    assert_eq!((app.terminal_elevated.as_ref(), app.editing), (None, None));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn click_click_creates_multi_point_arrow() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Arrow;
    frame(&mut app, &ctx, vec![]);
    // below the toolbar's drawing row, which an arrow tool opens
    click(&mut app, &ctx, Pos2::new(400.0, 200.0));
    assert!(app.multi.is_some());
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(500.0, 200.0))]);
    click(&mut app, &ctx, Pos2::new(500.0, 200.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(500.0, 300.0))]);
    click(&mut app, &ctx, Pos2::new(500.0, 300.0));
    app.finish_multi();
    assert_eq!(app.line_points(0), &[[400.0, 200.0], [500.0, 200.0], [500.0, 300.0]]);
    assert!(app.scene.elements[0].style.round, "arrows are curved by default");
    assert_eq!(app.tool, Tool::Select);
}

#[test]
fn line_points_drag_insert_and_delete() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.scene.elements = vec![app.new_element(ElementKind::Line { points: vec![[400.0, 300.0], [600.0, 300.0]], arrow: false })];
    app.selected = vec![0];
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, Pos2::new(600.0, 300.0), Pos2::new(600.0, 400.0));
    assert_eq!(app.line_points(0), &[[400.0, 300.0], [600.0, 400.0]]);
    app.line_editor = Some(0);
    drag(&mut app, &ctx, Pos2::new(500.0, 350.0), Pos2::new(500.0, 450.0));
    assert_eq!(app.line_points(0), &[[400.0, 300.0], [500.0, 450.0], [600.0, 400.0]]);
    assert_eq!(app.selected_point, Some(1));
    assert!(app.delete_selected_point());
    assert_eq!(app.line_points(0).len(), 2);
}

fn two_boxes(app: &mut DrawingApp) {
    app.scene.elements = vec![
        app.new_element(ElementKind::Rectangle { x: 300.0, y: 200.0, width: 100.0, height: 100.0 }),
        app.new_element(ElementKind::Rectangle { x: 600.0, y: 200.0, width: 100.0, height: 100.0 }),
    ];
}

#[test]
fn arrow_binds_to_shapes_and_follows_them() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    app.tool = Tool::Arrow;
    frame(&mut app, &ctx, vec![]);
    // released near the edges -> orbit
    drag(&mut app, &ctx, Pos2::new(395.0, 250.0), Pos2::new(605.0, 250.0));
    let arrow = &app.scene.elements[2];
    assert!(!arrow.ends.start.as_ref().unwrap().inside && !arrow.ends.end.as_ref().unwrap().inside);
    assert_eq!(arrow.ends.start.as_ref().map(|b| &b.element_id), Some(&app.scene.elements[0].id));
    assert_eq!(arrow.ends.end.as_ref().map(|b| &b.element_id), Some(&app.scene.elements[1].id));
    // orbit: endpoints stop on the outline + gap (5 + stroke 2 / 2)
    let pts = app.line_points(2).to_vec();
    assert!((pts[0][0] - 406.0).abs() < 0.5 && (pts[1][0] - 594.0).abs() < 0.5, "{pts:?}");
    // move the right box down: the arrow end follows it
    app.selected = vec![1];
    drag(&mut app, &ctx, Pos2::new(650.0, 250.0), Pos2::new(650.0, 350.0));
    let end = app.line_points(2)[1];
    assert!(end[1] > 280.0 && end[0] < 600.0, "{end:?}");
    assert!(app.scene.elements[2].ends.end.is_some(), "moving the shape keeps the binding");
    // moving the arrow alone detaches it
    app.selected = vec![2];
    app.nudge(Vec2::new(0.0, 5.0));
    assert!(app.scene.elements[2].ends.start.is_none() && app.scene.elements[2].ends.end.is_none());
}

#[test]
fn deep_inside_binds_to_the_exact_point() {
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    app.tool = Tool::Arrow;
    let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[350.0, 250.0], [650.0, 250.0]], arrow: true }));
    assert!(arrow.ends.start.as_ref().unwrap().inside && arrow.ends.end.as_ref().unwrap().inside);
    assert_eq!(arrow.kind, ElementKind::Line { points: vec![[350.0, 250.0], [650.0, 250.0]], arrow: true });
}

#[test]
fn command_disables_binding_and_deleting_target_unbinds() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    app.tool = Tool::Arrow;
    app.mods = Modifiers::COMMAND;
    let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[350.0, 250.0], [650.0, 250.0]], arrow: true }));
    assert!(arrow.ends.start.is_none() && arrow.ends.end.is_none());
    app.mods = Modifiers::NONE;
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, Pos2::new(350.0, 250.0), Pos2::new(650.0, 250.0));
    app.selected = vec![1];
    app.delete_selected();
    app.update_bindings();
    assert!(app.scene.elements[1].ends.end.is_none());
}

#[test]
fn copies_keep_bindings_only_with_their_targets() {
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    app.tool = Tool::Arrow;
    let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[350.0, 250.0], [650.0, 250.0]], arrow: true }));
    app.scene.elements.push(arrow);
    let both = DrawingApp::fresh_copies(&app.scene.elements, Vec2::ZERO);
    assert_eq!(both[2].ends.start.as_ref().unwrap().element_id, both[0].id);
    let alone = DrawingApp::fresh_copies(&app.scene.elements[2..], Vec2::ZERO);
    assert!(alone[0].ends.start.is_none());
}

#[test]
fn box_selection_contain_vs_overlap() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    two_boxes(&mut app); // 300..400 and 600..700, y 200..300
    frame(&mut app, &ctx, vec![]);
    // marquee over the whole first box and half of the second
    drag(&mut app, &ctx, Pos2::new(280.0, 180.0), Pos2::new(650.0, 320.0));
    assert_eq!(app.selected, vec![0], "Contido: only fully enclosed shapes");
    app.selected.clear();
    app.prefs.box_overlap = true;
    drag(&mut app, &ctx, Pos2::new(280.0, 180.0), Pos2::new(650.0, 320.0));
    assert_eq!(app.selected, vec![0, 1], "Sobreposto: touching is enough");
}

#[test]
fn snapping_to_objects_and_grid() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    frame(&mut app, &ctx, vec![]);
    app.prefs.snap_objects = true;
    app.selected = vec![1];
    // move the right box so its top lands 5px below the other's top: it snaps to y = 200
    drag(&mut app, &ctx, Pos2::new(650.0, 250.0), Pos2::new(650.0, 305.0));
    let moved = element_bounds(&app.scene.elements[1]);
    assert_eq!(moved.min.y, 250.0, "snaps 5px to the other box's center line");
    app.prefs.snap_objects = false;
    app.prefs.grid = true;
    app.tool = Tool::Rectangle;
    drag(&mut app, &ctx, Pos2::new(452.0, 407.0), Pos2::new(538.0, 489.0));
    let drawn = element_bounds(app.scene.elements.last().unwrap());
    assert_eq!((drawn.min, drawn.max), (Pos2::new(460.0, 400.0), Pos2::new(540.0, 480.0)), "grid snaps to 20px");
}

#[test]
fn binding_and_midpoint_preferences() {
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    app.tool = Tool::Arrow;
    // released near the right side's middle: snaps the anchor to [1, 0.5]
    let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[395.0, 262.0], [605.0, 250.0]], arrow: true }));
    assert_eq!(arrow.ends.start.as_ref().unwrap().fixed, [1.0, 0.5]);
    app.prefs.arrow_binding = false;
    let free = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[395.0, 262.0], [605.0, 250.0]], arrow: true }));
    assert!(free.ends.start.is_none() && free.ends.end.is_none());
}

#[test]
fn frame_carries_its_contents() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Frame;
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, Pos2::new(250.0, 150.0), Pos2::new(750.0, 400.0));
    assert_eq!(app.scene.elements[0].frame.as_deref(), Some("Frame"));
    app.scene.elements.push(app.new_element(ElementKind::Rectangle { x: 300.0, y: 200.0, width: 100.0, height: 100.0 }));
    app.scene.elements.push(app.new_element(ElementKind::Rectangle { x: 900.0, y: 200.0, width: 50.0, height: 50.0 }));
    // a click inside the frame doesn't grab it
    assert_eq!(app.hit(Pos2::new(600.0, 350.0)), None);
    // dragging its border moves the frame and what's inside, not what's outside
    app.tool = Tool::Select;
    app.selected = vec![0];
    drag(&mut app, &ctx, Pos2::new(250.0, 300.0), Pos2::new(250.0, 350.0));
    assert_eq!(element_bounds(&app.scene.elements[1]).min, Pos2::new(300.0, 250.0));
    assert_eq!(element_bounds(&app.scene.elements[2]).min, Pos2::new(900.0, 200.0));
    let back = crate::tests::round_trip(&app.scene.elements);
    assert_eq!(back[0].frame.as_deref(), Some("Frame"));
}

#[test]
fn extra_tools() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // nota adesiva: a click drops a 250px note, already editing its label; a drag is square
    app.tool = Tool::Sticky;
    drag(&mut app, &ctx, Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    assert_eq!(element_bounds(&app.scene.elements[0]).size(), Vec2::splat(250.0));
    assert!(app.scene.elements[0].created.is_some());
    assert_eq!(app.editing, Some(1));
    app.finish_text();
    app.tool = Tool::Sticky;
    drag(&mut app, &ctx, Pos2::new(700.0, 300.0), Pos2::new(800.0, 340.0));
    // a small drag stops at the note's smallest
    assert_eq!(element_bounds(app.scene.elements.iter().rev().find(|e| e.created.is_some()).unwrap()).size(), Vec2::splat(NOTE_MIN.x));
    app.finish_text();
    let back = crate::tests::round_trip(&app.scene.elements);
    assert_eq!(back[0].created, app.scene.elements[0].created);
    assert_eq!(sticky_date(1_792_497_600_000, true).as_deref(), Some("20 Oct"));
    // dark mode uses the web's invert(93%) hue-rotate(180deg): the note turns brown
    assert_eq!(themed(Color32::from_rgb(0xff, 0xdf, 0x6b), true), Color32::from_rgb(75, 48, 0));
    assert_eq!(sticky_date(1_700_000_000_000, false).as_deref(), Some("14 Nov 2023"));
    // preenchimento paints the shape under the pointer
    app.scene.elements = vec![app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 100.0, height: 100.0 })];
    app.scene.elements[0].style.fill = None;
    app.fill_at(Pos2::new(150.0, 150.0));
    assert!(app.scene.elements[0].style.fill.is_some());
    // laço selects only what it surrounds
    app.scene.elements.push(app.new_element(ElementKind::Rectangle { x: 500.0, y: 100.0, width: 50.0, height: 50.0 }));
    app.select_in_lasso(&[Pos2::new(50.0, 50.0), Pos2::new(250.0, 50.0), Pos2::new(250.0, 250.0), Pos2::new(50.0, 250.0)]);
    assert_eq!(app.selected, vec![0]);
    // desenhar para criar forma: a round scribble becomes an ellipse, a straight one a line
    let circle: Vec<[f32; 2]> = (0..=40)
        .map(|i| {
            let a = i as f32 / 40.0 * std::f32::consts::TAU;
            [100.0 + 50.0 * a.cos(), 100.0 + 50.0 * a.sin()]
        })
        .collect();
    assert!(matches!(recognize_shape(&app, &circle).unwrap().kind, ElementKind::Ellipse { .. }));
    let square = [[0.0, 0.0], [50.0, 0.0], [100.0, 0.0], [100.0, 50.0], [100.0, 100.0], [50.0, 100.0], [0.0, 100.0], [0.0, 50.0], [0.0, 2.0]];
    assert!(matches!(recognize_shape(&app, &square).unwrap().kind, ElementKind::Rectangle { .. }));
    let diamond = [[50.0, 0.0], [100.0, 50.0], [50.0, 100.0], [0.0, 50.0], [49.0, 1.0]];
    assert!(matches!(recognize_shape(&app, &diamond).unwrap().kind, ElementKind::Diamond { .. }));
    app.scene.elements.clear();
    let straight: Vec<[f32; 2]> = (0..10).map(|i| [i as f32 * 10.0, i as f32 * 5.0]).collect();
    assert!(matches!(recognize_shape(&app, &straight).unwrap().kind, ElementKind::Line { arrow: false, .. }));
    // a shaft with a head scribbled at its tip is an arrow
    let mut arrow: Vec<[f32; 2]> = (0..=20).map(|i| [i as f32 * 10.0, 0.0]).collect();
    arrow.extend([[185.0, -15.0], [200.0, 0.0], [185.0, 15.0], [200.0, 0.0], [185.0, -15.0], [200.0, 0.0], [185.0, 15.0]]);
    assert!(matches!(recognize_shape(&app, &arrow).unwrap().kind, ElementKind::Line { arrow: true, .. }));
    // a zigzag is no shape at all
    let zigzag: Vec<[f32; 2]> = (0..12).map(|i| [i as f32 * 20.0, if i % 2 == 0 { 0.0 } else { 80.0 }]).collect();
    assert!(recognize_shape(&app, &zigzag).is_none());
}

#[test]
fn ai_drawings_replay_with_the_agent_name() {
    let path = std::env::temp_dir().join(format!("magoagent-replay-{}.mago", std::process::id()));
    let mut app = DrawingApp::default();
    app.autosave = Some(path.clone());
    let mut external = app.scene.clone();
    external.agent = Some("Claude Code".into());
    external.elements.push(app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 100.0, height: 50.0 }));
    external.elements.push(app.new_element(ElementKind::Text { x: 10.0, y: 10.0, value: "oi".into() }));
    write_scene_file(&path, &external).unwrap();
    app.sync_from_file();
    assert_eq!(app.reveal.len(), 2);
    assert_eq!(app.agent.as_ref().map(|a| a.0.as_str()), Some("Claude Code"));
    // the front element is drawn over time, then the next one starts
    let first = app.scene.elements[0].clone();
    assert_eq!(app.reveal_progress(&first), 0.0);
    app.now += 10.0;
    assert_eq!(app.reveal_progress(&first), 1.0);
    // the file the app writes back never claims to be the AI's
    assert_eq!(app.scene.agent, None);
    let _ = fs::remove_file(path);
}

#[test]
fn image_round_trip() {
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 255])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    assert_eq!(base64_decode(&base64(&png)).unwrap(), png);
    let url = format!("data:image/png;base64,{}", base64(&png));
    let app = DrawingApp::default();
    let mut element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 40.0, height: 20.0 });
    element.image = Some(url.clone());
    let scene = Scene { elements: vec![element], ..Scene::default() };
    let interop = json!(interop_elements(&scene.elements));
    assert_eq!(interop[0]["type"], "image");
    let back = from_interop_elements(&interop);
    assert_eq!(back[0].image.as_deref(), Some(url.as_str()));
    // images are grabbed anywhere inside, and come out in the PNG export
    assert!(back[0].contains(Pos2::new(20.0, 10.0), 1.0));
    let out = image::load_from_memory(&render_svg_png(&scene_svg(&scene, None), 1.0).unwrap()).unwrap().to_rgba8();
    assert_eq!(out.get_pixel(30, 20).0, [255, 0, 0, 255]);
}

#[test]
fn view_mode_only_pans() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.prefs.view_mode = true;
    app.tool = Tool::Rectangle;
    frame(&mut app, &ctx, vec![]);
    drag(&mut app, &ctx, Pos2::new(300.0, 300.0), Pos2::new(400.0, 350.0));
    assert!(app.scene.elements.is_empty());
    assert_eq!(app.pan, Vec2::new(100.0, 50.0));
}

#[test]
fn text_png_contains_visible_pixels() {
    let mut app = DrawingApp::default();
    app.scene.elements.push(app.new_element(ElementKind::Text { x: 20.0, y: 40.0, value: "Planejamento".into() }));
    let png = render_svg_png(&scene_svg(&app.scene, Some(WHITE)), 1.0).unwrap();
    let img = image::load_from_memory(&png).unwrap().into_rgba8();
    assert!(img.pixels().filter(|p| p[0] < 100 && p[1] < 100 && p[2] < 100).count() > 30);
}

#[test]
fn terminal_tool_places_a_focused_terminal() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.select_tool(Tool::Terminal);
    drag(&mut app, &ctx, Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    // the dialog asks for the shell first, with an agent's name already in; a quick start fills the command
    assert!(app.scene.elements.is_empty());
    let named = app.terminal_dialog.as_ref().unwrap().spec.name.clone();
    assert!(AGENT_NAMES.contains(&named.as_str()), "{named}");
    app.terminal_dialog.as_mut().unwrap().spec.command = "claude".into();
    app.confirm_terminal_dialog();
    let e = app.scene.elements[0].clone();
    assert_eq!(e.terminal.as_ref().unwrap().name, named);
    assert_eq!(grid_size(local_bounds(&e).size()), (80, 24));
    assert_eq!(app.terminal_focus.as_ref(), Some(&e.id));
    assert_eq!(app.tool, Tool::Select);
    assert!(e.contains(local_bounds(&e).center(), 1.0), "grabbed anywhere inside");
    assert_eq!(crate::tests::round_trip(&app.scene.elements)[0].terminal, e.terminal);
    // picking a tool stops typing into it; a double-click on it types into it again
    app.select_tool(Tool::Rectangle);
    assert_eq!(app.terminal_focus, None);
    app.select_tool(Tool::Select);
    let at = app.to_screen(local_bounds(&e).center());
    for (t, pressed) in [(1.0, true), (1.05, false), (1.15, true), (1.2, false)] {
        frame_at(&mut app, &ctx, Some(t), vec![egui::Event::PointerMoved(at), button(at, pressed)]);
    }
    assert_eq!(app.terminal_focus.as_ref(), Some(&e.id));
    assert_eq!(app.selected, vec![0], "stays selected while typing, so its bar stays up");
    // a double-click on its title strip enlarges it, another one puts it back (to the right of
    // the top-left buttons and the toolbar, which reach the strip off the Mac)
    let title = app.to_screen(local_bounds(&e).right_top() + Vec2::new(-70.0, 10.0));
    for (t, pressed) in [(3.0, true), (3.05, false), (3.15, true), (3.2, false)] {
        frame_at(&mut app, &ctx, Some(t), vec![egui::Event::PointerMoved(title), button(title, pressed)]);
    }
    assert_eq!(app.terminal_elevated.as_ref(), Some(&e.id));
    assert_eq!(app.unique_terminal_name(&named, None), format!("{named} 2"));
}

#[test]
fn one_click_focuses_a_terminal_and_its_title_still_drags_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut e = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 400.0, height: 300.0 });
    e.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    // the body: focused at once, selected, not moved
    let body = app.to_screen(Pos2::new(300.0, 250.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(body), button(body, true)]);
    frame(&mut app, &ctx, vec![button(body, false)]);
    assert_eq!((app.terminal_focus.as_ref(), &app.selected[..], app.moving), (Some(&id), &[0][..], false));
    // the title strip is still the handle that drags it
    let before = element_bounds(&app.scene.elements[0]);
    let title = app.to_screen(Pos2::new(300.0, 110.0));
    drag(&mut app, &ctx, title, title + Vec2::new(60.0, 40.0));
    assert_eq!(element_bounds(&app.scene.elements[0]).min, before.min + Vec2::new(60.0, 40.0));
}

/// The centre of card row `row` of IDE `index`, on screen.
fn card_row_at(app: &DrawingApp, index: usize, row: usize) -> Pos2 {
    use crate::ide::card::{ROW, TREE_TOP};
    let local = local_bounds(&app.scene.elements[index]);
    app.to_screen(Pos2::new(local.left() + 30.0, local.top() + TREE_TOP + ROW * (row as f32 + 0.5)))
}

#[test]
fn a_click_on_an_ide_card_opens_folders_and_files_without_enlarging_it() {
    use crate::ide::card::{PANEL_W, TREE_MIN, parts, tab_close};
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let dir = std::env::temp_dir().join(format!("magoagent-ide-card-{}", std::process::id()));
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.join("README.md"), "# proj\n").unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut e = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: IDE_SIZE.x, height: IDE_SIZE.y });
    e.ide = Some(IdeSpec { name: "proj".into(), root: dir.to_string_lossy().into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    frame(&mut app, &ctx, vec![]);
    let names = |app: &DrawingApp| app.ides[&id].card_rows().into_iter().map(|r| r.1).collect::<Vec<_>>();
    assert_eq!(names(&app), ["src", "README.md"]);
    // a folder opens in place; the canvas stays as it is, and the card's tree takes the keys
    let row = card_row_at(&app, 0, 0);
    click(&mut app, &ctx, row);
    assert_eq!(names(&app), ["src", "main.rs", "README.md"]);
    assert_eq!((app.terminal_elevated.as_ref(), local_bounds(&app.scene.elements[0]).size()), (None, IDE_SIZE));
    assert_eq!(app.card_focus.as_ref(), Some(&id));
    // one click on a file only selects it; the double-click opens it in the panel, which grows the node
    let row = card_row_at(&app, 0, 1);
    click(&mut app, &ctx, row);
    assert_eq!((app.ides[&id].card_sel, app.ides[&id].active_path()), (Some(1), None));
    double_click(&mut app, &ctx, row, 10.0);
    assert_eq!(app.ides[&id].active_path(), Some(dir.join("src/main.rs")));
    // the tree narrows to its longest name (short ones: its least) and the code takes the rest
    let tree = |app: &DrawingApp| app.scene.elements[0].ide.as_ref().unwrap().tree_width;
    assert_eq!((tree(&app), local_bounds(&app.scene.elements[0]).width()), (Some(TREE_MIN), TREE_MIN + PANEL_W));
    // the bar between them drags the split, keeping room for the code
    let bar = app.to_screen(Pos2::new(100.0 + TREE_MIN, 200.0));
    drag(&mut app, &ctx, bar, bar + Vec2::new(100.0, 0.0));
    assert_eq!(tree(&app), Some(TREE_MIN + 100.0));
    drag(&mut app, &ctx, bar + Vec2::new(100.0, 0.0), bar + Vec2::new(900.0, 0.0));
    assert_eq!(tree(&app), Some(TREE_MIN + PANEL_W - 240.0));
    let far = app.to_screen(Pos2::new(100.0 + TREE_MIN + PANEL_W - 240.0, 200.0));
    drag(&mut app, &ctx, far, bar);
    assert_eq!(tree(&app), Some(TREE_MIN));
    // the arrows walk the tree: ↓ and Enter open README.md too; ↑↑ and ← close src, → opens it again
    let key = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame(&mut app, &ctx, vec![key(Key::ArrowDown), key(Key::Enter)]);
    assert_eq!(app.ides[&id].active_path(), Some(dir.join("README.md")));
    frame(&mut app, &ctx, vec![key(Key::ArrowUp), key(Key::ArrowUp), key(Key::ArrowLeft)]);
    assert_eq!((names(&app), app.ides[&id].card_sel), (vec!["src".to_string(), "README.md".into()], Some(0)));
    frame(&mut app, &ctx, vec![key(Key::ArrowRight), key(Key::ArrowRight)]);
    assert_eq!((names(&app).len(), app.ides[&id].card_sel), (3, Some(1)));
    // each tab closes on its ×, and the last one takes the panel away: the node is back to its tree
    // width (the canvas moves left so the tabs are on the 800 pt test screen)
    app.pan = Vec2::new(-200.0, 0.0);
    for _ in 0..2 {
        let element = &app.scene.elements[0];
        let (_, panel) = parts(local_bounds(element), element.ide.as_ref().unwrap().tree_width);
        let (tab, _) = app.ides[&id].panel_tabs(panel.unwrap())[0];
        let at = app.to_screen(tab_close(tab).center());
        click(&mut app, &ctx, at);
    }
    assert_eq!((local_bounds(&app.scene.elements[0]).size(), app.scene.elements[0].ide.as_ref().unwrap().tree_width), (IDE_SIZE, None));
    assert_eq!(app.terminal_elevated, None);
    // Esc lets the tree go
    frame(&mut app, &ctx, vec![key(Key::Escape)]);
    assert_eq!(app.card_focus, None);
    // a card resized below its tree with no file open (a saved scene): no panel, and it keeps its width
    let e = &mut app.scene.elements[0];
    e.ide.as_mut().unwrap().tree_width = Some(IDE_SIZE.x);
    if let ElementKind::Rectangle { width, .. } = &mut e.kind {
        *width = 276.0;
    }
    frame(&mut app, &ctx, vec![]);
    assert_eq!((local_bounds(&app.scene.elements[0]).width(), app.scene.elements[0].ide.as_ref().unwrap().tree_width), (276.0, None));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_agent_searches_opens_and_edits_through_the_ide_card() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let dir = std::env::temp_dir().join(format!("magoagent-ide-agent-{}", std::process::id()));
    fs::create_dir_all(dir.join("src")).unwrap();
    let main = dir.join("src").join("main.rs");
    fs::write(&main, "fn main() {\n    println!(\"oi\");\n}\n").unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut e = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: IDE_SIZE.x, height: IDE_SIZE.y });
    e.ide = Some(IdeSpec { name: "proj".into(), root: dir.to_string_lossy().into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    frame(&mut app, &ctx, vec![]);
    let shared = std::sync::Mutex::new(Shared { nodes: app.agent_nodes(), ..Default::default() });
    let (sender, commands) = std::sync::mpsc::channel();
    // the bridge's thread waits for the app, which answers meanwhile, as its frames do
    let ask = |app: &mut DrawingApp, cmd: Value| {
        let answer = std::thread::scope(|s| {
            let asking = s.spawn(|| handle(&cmd, &shared, &sender));
            while !asking.is_finished() {
                if let Ok(command) = commands.recv_timeout(std::time::Duration::from_millis(20)) {
                    app.apply_agent_command(command);
                }
            }
            asking.join().unwrap()
        });
        while let Ok(command) = commands.try_recv() {
            app.apply_agent_command(command);
        }
        answer
    };
    // from outside the canvas the only IDE needs no name
    assert!(ask(&mut app, json!({ "cmd": "list" })).unwrap().contains("IDEs:\n  - \"proj\""));
    let found = ask(&mut app, json!({ "cmd": "ide_search", "text": "PRINTLN" })).unwrap();
    assert_eq!(found, format!("{}:2: println!(\"oi\");", std::path::Path::new("src").join("main.rs").display()));
    // the folder opens in the tree, the file in the panel at its line, lit
    assert_eq!(
        ask(&mut app, json!({ "cmd": "ide_open", "path": "src/main.rs", "line": 2 })),
        Ok(format!("{}, linha 2 aberto no IDE.", std::path::Path::new("src").join("main.rs").display()))
    );
    let ide = &app.ides[&id];
    assert_eq!((ide.active_path(), ide.card_sel, ide.panel_mark.clone()), (Some(main.clone()), Some(1), Some((main.clone(), 1, 1))));
    assert_eq!(local_bounds(&app.scene.elements[0]).width(), crate::ide::card::TREE_MIN + crate::ide::card::PANEL_W);
    // an edit in view: saved, the changed lines lit
    let edit = ask(&mut app, json!({ "cmd": "ide_edit", "path": "src/main.rs", "old": "\"oi\"", "new": "\"olá\"" })).unwrap();
    assert!(edit.contains("linhas 2 a 2"), "{edit}");
    assert_eq!(fs::read_to_string(&main).unwrap(), "fn main() {\n    println!(\"olá\");\n}\n");
    // whole lines, as agents send them: the closing line break starts no line of its own
    let edit = ask(
        &mut app,
        json!({ "cmd": "ide_edit", "path": "src/main.rs", "old": "    println!(\"olá\");\n", "new": "    println!(\"olá\");\n    println!(\"tchau\");\n" }),
    )
    .unwrap();
    assert!(edit.contains("linhas 2 a 3"), "{edit}");
    // a missing snippet, or a path above the project: refused, nothing written
    assert!(ask(&mut app, json!({ "cmd": "ide_edit", "path": "src/main.rs", "old": "nada", "new": "x" })).unwrap_err().contains("não está"));
    assert!(ask(&mut app, json!({ "cmd": "ide_open", "path": "../fora.txt" })).unwrap_err().contains("sem `..`"));
    assert_eq!(fs::read_to_string(&main).unwrap(), "fn main() {\n    println!(\"olá\");\n    println!(\"tchau\");\n}\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_file_an_agent_opens_brings_its_ide_card_over_the_nodes_on_its_panel() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let dir = std::env::temp_dir().join(format!("magoagent-ide-raise-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let notes = dir.join("notas.txt");
    fs::write(&notes, "um\ndois\n").unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut e = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: IDE_SIZE.x, height: IDE_SIZE.y });
    e.ide = Some(IdeSpec { name: "proj".into(), root: dir.to_string_lossy().into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    // where the panel grows to, drawn after the card
    let over = app.new_element(ElementKind::Rectangle { x: 100.0 + IDE_SIZE.x, y: 100.0, width: 200.0, height: 200.0 });
    let over_id = over.id.clone();
    app.scene.elements.push(over);
    frame(&mut app, &ctx, vec![]);
    let open = |app: &mut DrawingApp| {
        let (reply, answer) = std::sync::mpsc::channel();
        app.ide_agent(&id, crate::bridge::IdeAction::Open { path: notes.clone(), line: Some(2) }, reply);
        answer.recv().unwrap()
    };
    // an editor open holds indices: the card stays where it is
    app.editing = Some(1);
    assert!(open(&mut app).is_ok());
    assert_eq!(app.scene.elements[0].id, id);
    // between gestures it comes to the front, and the selection follows its element
    app.editing = None;
    app.selected = vec![1];
    assert!(open(&mut app).is_ok());
    assert_eq!((app.scene.elements[0].id.as_str(), app.scene.elements[1].id.as_str()), (over_id.as_str(), id.as_str()));
    assert_eq!(app.selected, vec![0]);
    // a node drawn later but away from it covers nothing: the order stays the person's
    let away = app.new_element(ElementKind::Rectangle { x: 5000.0, y: 5000.0, width: 50.0, height: 50.0 });
    app.scene.elements.push(away);
    assert!(open(&mut app).is_ok());
    assert_eq!(app.scene.elements[1].id, id);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_open_chat_lets_no_press_reach_the_hidden_canvas() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Rectangle;
    frame(&mut app, &ctx, vec![]);
    app.chat_open = true;
    drag(&mut app, &ctx, Pos2::new(300.0, 20.0), Pos2::new(450.0, 60.0));
    assert!(app.scene.elements.is_empty(), "nothing drawn unseen in the strip above the chat");
}

#[test]
fn a_file_dropped_on_the_open_chat_goes_into_its_box_not_on_the_hidden_canvas() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.chat_open = true;
    app.chat_draft = "olha".into();
    #[derive(Debug)]
    struct Dropped(std::path::PathBuf);
    impl egui::DroppedFile for Dropped {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
        fn bytes(&self) -> Result<Vec<u8>, String> {
            Err("not read".into())
        }
    }
    let file = |p: &str| -> egui::DroppedFileHandle { std::sync::Arc::new(Dropped(p.into())) };
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
        dropped_files: vec![file("/tmp/print.png"), file("/tmp/minha pasta")],
        ..Default::default()
    };
    ctx.run_ui(raw, |ui| {
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let (response, _) = ui.allocate_painter(ui.available_size(), Sense::CLICK | Sense::DRAG);
            app.handle_canvas(&response, response.rect);
        });
    })
    .textures_delta
    .clear();
    assert!(app.scene.elements.is_empty(), "no image nor folder card on the canvas behind the chat");
    // the picture waits above the words, the folder goes in as its path
    assert_eq!(app.chat_draft, "olha \"/tmp/minha pasta\"");
    assert_eq!(app.chat_pics.borrow().attached.iter().map(|a| a.path.clone()).collect::<Vec<_>>(), [std::path::PathBuf::from("/tmp/print.png")]);
    let id = egui::Id::unique("chat_draft");
    assert!(ctx.memory(|m| m.has_focus(id)), "the box takes the keys");
    let cursor = egui::TextEdit::load_state(&ctx, id).and_then(|s| s.cursor.char_range()).unwrap().primary.index;
    assert_eq!(cursor, egui::text::CharIndex(app.chat_draft.chars().count()), "typing goes on after the paths");
    assert_eq!(super::pointer::dropped_paths("", &["/a.png".into()]), "/a.png", "no gap in an empty box");
    assert_eq!(
        super::pointer::dropped_paths(
            "oi
",
            &["/a.png".into()]
        ),
        "/a.png",
        "nor after a newline"
    );
    assert_eq!(super::pointer::dropped_paths("", &["/tmp/a\"b c.png".into()]), "'/tmp/a\"b c.png'", "a quote in the name");
    assert_eq!(super::pointer::dropped_paths("", &["/tmp/it's \"x\".png".into()]), "'/tmp/it\\'s \"x\".png'", "both quotes");
    assert_eq!(super::pointer::dropped_paths("", &["/tmp/a\\'\".png".into()]), "'/tmp/a\\\\\\'\".png'", "a backslash before the quote");
    // an image pasted on the open chat: saved as a png, waiting in the box
    app.chat_draft.clear();
    app.chat_pics.borrow_mut().attached.clear();
    let shot = egui::ColorImage::new([3, 2], vec![egui::Color32::RED; 6]);
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
        events: vec![egui::Event::PasteImage(std::sync::Arc::new(shot))],
        ..Default::default()
    };
    ctx.run_ui(raw, |ui| {
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let (response, _) = ui.allocate_painter(ui.available_size(), Sense::CLICK | Sense::DRAG);
            app.handle_canvas(&response, response.rect);
        });
    })
    .textures_delta
    .clear();
    assert!(app.chat_draft.is_empty());
    let saved = app.chat_pics.borrow().attached[0].path.clone();
    let back = image::open(&saved).unwrap().to_rgba8();
    assert_eq!((back.width(), back.height(), back.get_pixel(0, 0).0), (3, 2, [255, 0, 0, 255]), "the pasted pixels, in {saved:?}");
    let _ = std::fs::remove_file(saved);
}

#[test]
fn the_export_dialog_holds_the_keys_and_esc_closes_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.tool = Tool::Rectangle;
    drag(&mut app, &ctx, Pos2::new(300.0, 300.0), Pos2::new(400.0, 380.0));
    app.selected = vec![0];
    app.export_open = true;
    frame(&mut app, &ctx, vec![key_press(Key::Delete)]);
    assert_eq!(app.scene.elements.len(), 1, "a Delete in the dialog leaves the drawing alone");
    frame(&mut app, &ctx, vec![key_press(Key::Escape)]);
    assert!(!app.export_open);
}

#[test]
fn ide_node_opens_enlarged_with_its_claude_terminal_roped() {
    let ctx = Context::default();
    install_fonts(&ctx, false); // the IDE draws with its own font families
    let dir = std::env::temp_dir().join(format!("magoagent-ide-ui-{}", std::process::id()));
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.select_tool(Tool::Ide);
    drag(&mut app, &ctx, Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    app.ide_dialog.as_mut().unwrap().spec.root = dir.to_string_lossy().into();
    app.confirm_ide_dialog();
    let id = app.scene.elements[0].id.clone();
    assert_eq!(app.scene.elements[0].ide.as_ref().unwrap().name, dir.file_name().unwrap().to_string_lossy());
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.ides[&id].card_rows(), [(0, "src".to_string(), true, false)]);
    // its Claude Code button: a `claude` terminal beside it, roped before it starts, so it gets the port
    app.claude_for_ide(&id);
    app.claude_for_ide(&id);
    let terminals: Vec<&Element> = app.scene.elements.iter().filter(|e| e.terminal.is_some()).collect();
    assert_eq!(terminals.len(), 1, "the second press reuses the roped terminal");
    let env = app.ide_env(&terminals[0].id);
    assert!(env.contains(&("ENABLE_IDE_INTEGRATION".into(), "true".into())));
    assert!(env.iter().any(|(k, v)| k == "CLAUDE_CODE_SSE_PORT" && v.parse::<u16>().is_ok()));
    // enlarged it has the keyboard: canvas letters don't pick tools, Esc goes back
    app.set_elevated(Some(id.clone()));
    let r = egui::Event::Key { key: Key::R, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    for t in [10.0, 10.5, 11.0] {
        frame_at(&mut app, &ctx, Some(t), vec![r.clone()]);
    }
    assert_eq!((app.tool, app.ide_focus.as_ref()), (Tool::Select, Some(&id)));
    // its bar goes up in the toolbar's place (the title bar leaves its clicks to it), and the editor right under it
    assert_eq!(app.toolbar_span.top(), TOP_ROW);
    assert!(app.elevated_rect().top() <= app.toolbar_span.bottom() + 12.0, "{:?}", app.elevated_rect());
    let esc = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame_at(&mut app, &ctx, Some(11.5), vec![esc]);
    assert_eq!((app.terminal_elevated.as_ref(), app.ide_focus.as_ref()), (None, None));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn window_tools_show_no_style_panel() {
    let mut app = DrawingApp::default();
    for tool in [Tool::Terminal, Tool::Portal, Tool::Ide, Tool::Sticky, Tool::Text] {
        app.tool = tool;
        assert!(!app.shows_properties(), "{tool:?}");
    }
    app.tool = Tool::Rectangle;
    assert!(app.shows_properties());
    // an image draws with none of the style; a text takes it once typed, not while it is
    app.tool = Tool::Select;
    let mut image = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 10.0, height: 10.0 });
    image.image = Some("data:image/png;base64,".into());
    let text = app.new_element(ElementKind::Text { x: 0.0, y: 40.0, value: "oi".into() });
    app.scene.elements.extend([image, text]);
    app.selected = vec![0];
    assert!(!app.shows_properties(), "image");
    (app.selected, app.editing) = (vec![1], Some(1));
    assert!(!app.shows_properties(), "typing");
    app.editing = None;
    assert!(app.shows_properties(), "a typed text keeps its color and size");
}

#[test]
fn ide_screen_keys_open_the_enlarged_ide_or_the_first() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let dir = std::env::temp_dir().join(format!("magoagent-ide-keys-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut app = DrawingApp::default();
    let cmd = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    frame(&mut app, &ctx, vec![cmd(Key::Num3)]);
    assert!(app.status.starts_with("Nenhum IDE"));
    // two IDEs off screen and a rectangle selected: ⌘3 opens the first IDE, selected like a double-click
    for x in [0.0, 700.0] {
        let spec = IdeSpec { name: String::new(), root: dir.to_string_lossy().into(), ..Default::default() };
        app.ide_dialog = Some(IdeDialog { target: None, at: Rect::from_min_size(Pos2::new(x, 1000.0), IDE_SIZE), spec });
        app.confirm_ide_dialog();
    }
    app.tool = Tool::Rectangle;
    drag(&mut app, &ctx, Pos2::new(300.0, 200.0), Pos2::new(500.0, 350.0));
    assert!(app.shows_properties());
    let (a, b) = (app.scene.elements[0].id.clone(), app.scene.elements[1].id.clone());
    frame(&mut app, &ctx, vec![cmd(Key::Num3)]);
    assert_eq!((app.terminal_elevated.as_ref(), app.ide_focus.as_ref(), &app.selected[..]), (Some(&a), Some(&a), &[0][..]));
    assert!(!app.shows_properties(), "the rectangle's panel would float over the IDE");
    // the second one open but without the keyboard (a dialog took it): ⌘3 gives it back, not the first
    app.set_elevated(Some(b.clone()));
    app.ide_focus = None;
    frame(&mut app, &ctx, vec![cmd(Key::Num3)]);
    assert_eq!((app.terminal_elevated.as_ref(), app.ide_focus.as_ref()), (Some(&b), Some(&b)));
    // ⌘2: the canvas, with the IDE back as a card
    frame(&mut app, &ctx, vec![cmd(Key::Num2)]);
    assert_eq!((app.terminal_elevated.as_ref(), app.ide_focus.as_ref()), (None, None));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn maestro_recruits_opens_portals_and_connects() {
    let mut app = DrawingApp::default();
    let mut boss = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 600.0, height: 400.0 });
    boss.terminal = Some(TerminalSpec { name: "Maestro".into(), cwd: "/tmp/projeto".into(), command: "claude".into(), ..Default::default() });
    let (boss_id, b) = (boss.id.clone(), local_bounds(&boss));
    app.scene.elements.push(boss);
    let (tx, rx) = std::sync::mpsc::channel();
    let newest = |app: &DrawingApp| app.scene.elements.iter().rev().find(|e| e.terminal.is_some()).cloned().unwrap();
    // a recruit: below the caller, its size and folder, roped to it, with an agent's name
    app.recruit(&boss_id, "codex", tx.clone());
    let first = newest(&app);
    let name = first.terminal.as_ref().unwrap().name.clone();
    assert!(AGENT_NAMES.contains(&name.as_str()) && rx.recv().unwrap().unwrap().contains(&format!("\"{name}\"")));
    let r = local_bounds(&first);
    assert!(r.min.y > b.max.y && r.size() == b.size(), "below, the same size: {r:?}");
    assert_eq!((first.terminal.as_ref().unwrap().cwd.as_str(), first.terminal.as_ref().unwrap().command.as_str()), ("/tmp/projeto", "codex"));
    assert!(app.links()[&boss_id].contains(&first.id));
    // the next one gets a name of its own and a free spot further down
    app.recruit(&boss_id, "claude", tx.clone());
    assert!(rx.recv().unwrap().is_ok());
    let second = newest(&app);
    assert_ne!(second.terminal.as_ref().unwrap().name, name);
    assert!(!local_bounds(&second).intersects(r));
    // a portal to the caller's right, named by its site
    app.create_portal(&boss_id, "exemplo.com/docs", "", tx.clone());
    assert!(rx.recv().unwrap().unwrap().contains("\"exemplo.com\""));
    let portal = app.scene.elements.iter().find(|e| e.portal.is_some()).cloned().unwrap();
    assert!(local_bounds(&portal).min.x > b.max.x && app.links()[&boss_id].contains(&portal.id));
    assert_eq!(portal.portal.as_ref().unwrap().url, "https://exemplo.com/docs");
    // two recruits roped together, once
    app.connect_for_agent(&first.id, &second.id, tx.clone());
    assert!(rx.recv().unwrap().unwrap().ends_with("conectados."));
    app.connect_for_agent(&first.id, &second.id, tx.clone());
    assert!(rx.recv().unwrap().unwrap().contains("já estavam"));
    // an end gone since the bridge resolved it: an error, not a hollow "conectados"
    app.connect_for_agent(&first.id, "sumiu", tx.clone());
    assert!(rx.recv().unwrap().is_err());
    // from outside a terminal there is no caller to place things next to
    app.recruit("", "claude", tx);
    assert!(rx.recv().unwrap().is_err());
}

#[test]
fn connect_mode_joins_the_node_clicked_and_cancels_elsewhere() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // off the Mac the canvas starts under the title bar, which these frames leave out
    app.pan = Vec2::new(0.0, TITLE_BAR);
    let mut a = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 200.0, height: 120.0 });
    let mut b = app.new_element(ElementKind::Rectangle { x: 400.0, y: 0.0, width: 200.0, height: 120.0 });
    a.terminal = Some(TerminalSpec { name: "A".into(), ..Default::default() });
    b.terminal = Some(TerminalSpec { name: "B".into(), ..Default::default() });
    let (a_id, b_id) = (a.id.clone(), b.id.clone());
    app.scene.elements.extend([a, b]);
    // "Conectar" on a: a rope hangs from it to the pointer, which moves over the canvas
    app.connecting = Some(a_id.clone());
    let mid = app.to_screen(Pos2::new(300.0, 300.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(mid)]);
    app.swing_ropes(1.0 / 60.0);
    assert!(app.ropes.contains_key(CONNECT_DRAFT), "the draft rope follows the pointer");
    // a click on b joins them, and the draft goes once the mode is off (low on b: off macOS the
    // toolbar row sits lower and covers the card's top)
    let on_b = app.to_screen(Pos2::new(500.0, 100.0));
    click(&mut app, &ctx, on_b);
    assert!(app.connecting.is_none());
    assert!(app.links()[&a_id].contains(&b_id));
    app.swing_ropes(1.0 / 60.0);
    assert!(!app.ropes.contains_key(CONNECT_DRAFT));
    // from here on, nothing new gets made: Esc, a click on the empty canvas and one on a itself cancel
    let count = app.scene.elements.len();
    app.connecting = Some(a_id.clone());
    let esc = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame(&mut app, &ctx, vec![esc]);
    assert!(app.connecting.is_none());
    app.connecting = Some(a_id.clone());
    let empty = app.to_screen(Pos2::new(300.0, 500.0));
    click(&mut app, &ctx, empty);
    assert!(app.connecting.is_none());
    app.connecting = Some(a_id.clone());
    let own = app.to_screen(Pos2::new(100.0, 60.0));
    click(&mut app, &ctx, own);
    assert!(app.connecting.is_none());
    assert_eq!(app.scene.elements.len(), count, "cancelling adds nothing");
    assert_eq!(app.links()[&a_id], vec![b_id]);
}

#[test]
fn a_window_an_agent_makes_is_roped_to_its_maker() {
    let mut app = DrawingApp::default();
    let mut boss = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 600.0, height: 400.0 });
    boss.terminal = Some(TerminalSpec { name: "Maestro".into(), ..Default::default() });
    let mut child = app.new_element(ElementKind::Rectangle { x: 700.0, y: 0.0, width: 600.0, height: 400.0 });
    child.terminal = Some(TerminalSpec { name: "Revisor".into(), maestro: false, ..Default::default() });
    let (boss_id, child_id) = (boss.id.clone(), child.id.clone());
    app.scene.elements.extend([boss, child]);
    // what the app does with a drawing the MCP server wrote from inside the boss terminal
    app.tie_to_terminal(&boss_id, std::slice::from_ref(&child_id));
    assert!(app.links()[&boss_id].contains(&child_id));
    assert!(!app.agent_nodes().iter().find(|n| n.id == child_id).unwrap().maestro);
}

#[test]
fn a_note_at_rest_is_easier_to_pick_up_than_the_one_being_worked_on() {
    let mut app = DrawingApp::default();
    let [note, text] = app.new_note(Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(200.0, 100.0)), "oi".into());
    app.scene.elements.extend([note, text]);
    let at = |y: f32| Pos2::new(100.0, y);
    // at rest: 44 screen px, at least the header, at most 40% of the note (40 here)
    assert!(app.on_title(0, at(30.0)) && !app.on_title(0, at(41.0)));
    app.zoom = 2.0;
    assert!(app.on_title(0, at(21.0)) && !app.on_title(0, at(23.0)));
    app.zoom = 4.0;
    assert!(app.on_title(0, at(15.0)) && !app.on_title(0, at(17.0)), "never under the header");
    // the only one selected, or typed in: just the header
    app.zoom = 1.0;
    app.selected = vec![0];
    assert!(app.on_title(0, at(15.0)) && !app.on_title(0, at(30.0)));
    app.selected = vec![0, 1];
    app.editing = Some(1);
    assert!(!app.on_title(0, at(30.0)));
}

#[test]
fn a_double_click_on_a_note_types_into_that_note() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [a, a_text] = app.new_note(Rect::from_min_size(Pos2::new(640.0, 60.0), Vec2::splat(260.0)), "# Plano".into());
    // long markdown lines, many of them: the label, unwrapped, is wider and taller than the card
    // and centered on it, so it reaches over A
    let long = (0..25).map(|_| "palavra ".repeat(14)).collect::<Vec<_>>().join("\n");
    let [b, b_text] = app.new_note(Rect::from_min_size(Pos2::new(940.0, 60.0), Vec2::new(300.0, 380.0)), long);
    app.scene.elements.extend([a, a_text.clone(), b, b_text]);
    let at = app.to_screen(Pos2::new(770.0, 200.0));
    for (t, pressed) in [(1.0, true), (1.05, false), (1.15, true), (1.2, false)] {
        frame_at(&mut app, &ctx, Some(t), vec![egui::Event::PointerMoved(at), button(at, pressed)]);
    }
    let editing = app.editing.map(|i| app.scene.elements[i].id.clone());
    assert_eq!(editing, Some(a_text.id), "the note under the pointer, not the other one");
    assert_eq!(app.selected, vec![0], "the card stays the selection, its text is typed in place");
}

#[test]
fn clicks_on_a_note_select_its_card_never_its_text() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // inside the 800 pt test window: a short note A and a long, tall note B beside it
    let long = (0..25).map(|_| "palavra ".repeat(14)).collect::<Vec<_>>().join("\n");
    let [a, a_text] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "# Plano".into());
    let [b, b_text] = app.new_note(Rect::from_min_size(Pos2::new(380.0, 60.0), Vec2::new(300.0, 380.0)), long);
    app.scene.elements.extend([a, a_text, b, b_text]);
    // the text of A, where its label sits: the card is what is picked
    let at = app.to_screen(Pos2::new(150.0, 150.0));
    click(&mut app, &ctx, at);
    assert_eq!(app.selected, vec![0], "the short note's card, not its label");
    // over B's label box (which reaches over A) but on A's card: A is picked
    let at = app.to_screen(Pos2::new(250.0, 200.0));
    click(&mut app, &ctx, at);
    assert_eq!(app.selected, vec![0], "a long neighbour's text does not reach over A");
    // a click on B's card picks B; no label is ever part of a selection
    let at = app.to_screen(Pos2::new(600.0, 300.0));
    click(&mut app, &ctx, at);
    assert_eq!(app.selected, vec![2]);
    assert!(app.selected.iter().all(|i| !app.is_card_text(*i)));
}

#[test]
#[cfg(not(target_os = "linux"))] // portals have no native page on Linux yet
fn portal_page_fills_the_card_under_the_bar() {
    // the native page is the card's inside, under the header and the bar, at any zoom
    for zoom in [1.0, 2.0] {
        let app = DrawingApp { zoom, canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::splat(4000.0)), ..Default::default() };
        let mut e = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 960.0, height: 680.0 });
        e.portal = Some(PortalSpec { name: "Docs".into(), url: "https://a.dev".into(), ..Default::default() });
        let b = local_bounds(&e);
        let card = Rect::from_min_max(app.to_screen(b.min), app.to_screen(b.max));
        let chrome = PORTAL_CHROME * zoom;
        assert_eq!(app.portal_page_rect(&e), Some(Rect::from_min_max(card.min + Vec2::new(1.0, chrome + 1.0), card.max - Vec2::splat(1.0))));
        // with the bar hidden the page takes the whole card
        e.portal.as_mut().unwrap().chrome_hidden = true;
        assert_eq!(app.portal_page_rect(&e), Some(card.shrink(1.0)));
    }
}

#[test]
fn an_enlarged_portal_keeps_its_bar_at_any_canvas_zoom() {
    let mut app = DrawingApp { zoom: 0.3, now: 10.0, canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0)), ..Default::default() };
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 960.0, height: 680.0 });
    e.portal = Some(PortalSpec { name: "Docs".into(), url: "https://a.dev".into(), ..Default::default() });
    assert!(!app.chrome_live(&e), "this small on the canvas, its bar is only drawn");
    // enlarged it is shown at its own scale, whatever the canvas zoom
    app.terminal_elevated = Some(e.id.clone());
    assert!(app.chrome_live(&e));
}

#[cfg(not(target_os = "linux"))]
#[test]
fn the_apps_floating_ui_cuts_holes_in_a_portal_page() {
    let ctx = Context::default();
    let page = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(600.0, 400.0));
    let area = |id: &str, at: Pos2, order: egui::Order| {
        egui::Area::new(egui::Id::unique(id)).fixed_pos(at).order(order).show(&ctx, |ui| ui.set_min_size(Vec2::new(200.0, 80.0)));
    };
    for _ in 0..2 {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), ..Default::default() };
        ctx.run_ui(raw, |_| {
            // a menu over the page's left edge, one clear of it, and the canvas layer under it
            area("menu", Pos2::new(50.0, 150.0), egui::Order::Foreground);
            area("clear", Pos2::new(720.0, 520.0), egui::Order::Foreground);
            area("canvas", Pos2::new(300.0, 300.0), egui::Order::Background);
        })
        .textures_delta
        .clear();
    }
    assert_eq!(DrawingApp::page_holes(&ctx, page, &[]), [Rect::from_min_max(Pos2::new(100.0, 150.0), Pos2::new(250.0, 230.0))]);
}

#[test]
fn typing_after_the_click_that_creates_a_text_lands_in_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // the first editor shows on a sizing pass, the second one after it: both must keep focus
    for (word, at) in [("abc", Pos2::new(320.0, 300.0)), ("def", Pos2::new(340.0, 420.0))] {
        app.tool = Tool::Text;
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
        frame(&mut app, &ctx, vec![button(at, false)]);
        frame(&mut app, &ctx, vec![egui::Event::Text(word.into())]);
        frame(&mut app, &ctx, vec![]);
        app.finish_text();
    }
    // finish_text drops empty texts, so both words surviving means the typing landed
    let values: Vec<&str> = app
        .scene
        .elements
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Text { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(values, ["abc", "def"]);
}

#[test]
fn double_click_labels_a_shape_and_an_arrow() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let rect = app.new_element(ElementKind::Rectangle { x: 300.0, y: 200.0, width: 200.0, height: 100.0 });
    let arrow = app.new_element(ElementKind::Line { points: vec![[300.0, 500.0], [500.0, 500.0]], arrow: true });
    let (rect_id, arrow_id) = (rect.id.clone(), arrow.id.clone());
    app.scene.elements = vec![rect, arrow];
    // a double-click inside the transparent box, then on the middle of the selected arrow
    app.selected = vec![1];
    for (t, at) in [(1.0, Pos2::new(400.0, 250.0)), (2.0, Pos2::new(400.0, 500.0))] {
        for (dt, pressed) in [(0.0, true), (0.05, false), (0.15, true), (0.2, false)] {
            frame_at(&mut app, &ctx, Some(t + dt), vec![egui::Event::PointerMoved(at), button(at, pressed)]);
        }
        frame_at(&mut app, &ctx, Some(t + 0.3), vec![]);
        frame_at(&mut app, &ctx, Some(t + 0.31), vec![egui::Event::Text("sim".into())]);
        frame_at(&mut app, &ctx, Some(t + 0.32), vec![]);
        app.finish_text();
    }
    let label = |owner: &str| {
        app.scene.elements.iter().find(|e| e.id == format!("{owner}-label")).map(|e| {
            let ElementKind::Text { value, .. } = &e.kind else { panic!("label is text") };
            value.clone()
        })
    };
    assert_eq!(label(&rect_id).as_deref(), Some("sim"), "labels the shape");
    assert_eq!(label(&arrow_id).as_deref(), Some("sim"), "labels the arrow");
    let ElementKind::Line { points, .. } = &app.scene.elements[1].kind else { panic!("still an arrow") };
    assert_eq!(points.len(), 2, "its middle is not a new point");
}

/// A terminal made the way the dialog makes one, for the keyboard tests below.
fn terminal_at(app: &mut DrawingApp, ctx: &Context, at: Pos2) -> String {
    app.select_tool(Tool::Terminal);
    drag(app, ctx, at, at + Vec2::new(1.0, 0.0));
    app.terminal_dialog.as_mut().unwrap().spec.command = "claude".into();
    app.confirm_terminal_dialog();
    app.select_tool(Tool::Select);
    app.scene.elements.last().unwrap().id.clone()
}

fn key_press(key: Key) -> egui::Event {
    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

#[test]
fn typing_into_a_window_is_not_a_tool_shortcut() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = terminal_at(&mut app, &ctx, Pos2::new(400.0, 300.0));
    app.terminal_focus = Some(id.clone());
    frame(&mut app, &ctx, vec![key_press(Key::T)]);
    assert_eq!(app.tool, Tool::Select, "the focused terminal gets the t");
    app.terminal_focus = None;
    app.chat_open = true;
    frame(&mut app, &ctx, vec![key_press(Key::T)]);
    assert_eq!(app.tool, Tool::Select, "the chat gets the t");
}

#[test]
fn a_terminal_taken_by_undo_keeps_its_shell_while_redo_can_bring_it_back() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    e.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    let id = e.id.clone();
    app.snapshot();
    app.scene.elements.push(e);
    let program = if cfg!(windows) { ("powershell", &["-NoProfile", "-Command", "Start-Sleep 30"][..]) } else { ("sh", &["-c", "sleep 30"][..]) };
    let mut session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program)).unwrap();
    session.started_ms = 1; // marks this very shell
    app.terminals.insert(id.clone(), Ok(session));
    app.terminal_focus = Some(id.clone());
    let same = |app: &DrawingApp| app.terminals.get(&id).is_some_and(|s| s.as_ref().is_ok_and(|s| s.started_ms == 1));
    app.undo();
    app.sync_terminals(&ctx);
    assert!(same(&app), "the undo keeps its shell");
    assert_eq!(app.terminal_focus, None, "and takes the keyboard from it");
    app.redo();
    app.sync_terminals(&ctx);
    assert!(same(&app), "the redo brings the same shell back");
    // a change after the undo clears the redo: the shell ends
    app.undo();
    app.snapshot();
    app.sync_terminals(&ctx);
    assert!(!app.terminals.contains_key(&id));
}

#[test]
fn unload_ends_a_terminals_shell_and_reload_brings_it_back_as_undo_steps() {
    use crate::app::ui::{agents::AgentState, context::Target};
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    e.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    let mut page = app.new_element(ElementKind::Rectangle { x: 500.0, y: 0.0, width: 400.0, height: 300.0 });
    page.portal = Some(PortalSpec { name: "Docs".into(), url: "about:blank".into(), ..Default::default() });
    let (id, page_id) = (e.id.clone(), page.id.clone());
    app.scene.elements.extend([e, page]);
    // a shell that couldn't start stands in for one: no process in this test
    app.terminals.insert(id.clone(), Err("x".into()));
    for target in [Target::Element(id.clone()), Target::Element(page_id.clone())] {
        app.context_action(&ctx, &target, "unload");
    }
    assert!(app.scene.elements.iter().all(|e| e.unloaded()));
    assert_eq!(app.undo.len(), 2);
    app.sync_terminals(&ctx);
    assert!(!app.terminals.contains_key(&id), "its shell ends");
    assert_eq!(app.agent_rows()[0].state, AgentState::Idle(None), "and its agent is idle, not ended");
    app.context_action(&ctx, &Target::Element(id.clone()), "unload");
    assert_eq!(app.undo.len(), 2, "unloading it again is no step");
    app.context_action(&ctx, &Target::Element(id), "reload");
    assert!(!app.scene.elements[0].unloaded());
    app.undo();
    assert!(app.scene.elements[0].unloaded(), "Ctrl+Z puts it back unloaded");
    // the page's own ⟳ (the address bar's) loads an unloaded portal back
    app.portal_reload(&page_id);
    assert!(!app.scene.elements[1].unloaded());
}

/// One pass that only paints the canvas; what it painted, in shapes.
fn painted(app: &DrawingApp, ctx: &Context) -> usize {
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| app.paint_scene(ui.painter(), ui.max_rect()));
    out.textures_delta.clear();
    out.shapes.len()
}

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
    // Instantâneo: there at once
    app.prefs.instant_camera = true;
    app.glide_to(target);
    assert!(app.glide.is_none() && (app.to_screen(target) - app.canvas_rect.center()).length() < 0.01);
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

#[test]
fn ctrl_j_off_the_mac_stays_the_focused_terminals_newline() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = terminal_at(&mut app, &ctx, Pos2::new(400.0, 300.0));
    let ctrl_j = egui::Event::Key { key: Key::J, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    app.terminal_focus = Some(id);
    frame(&mut app, &ctx, vec![ctrl_j.clone()]);
    assert_eq!(app.agents_open, cfg!(target_os = "macos"), "off the Mac the shell gets its ^J");
    app.agents_open = false;
    app.terminal_focus = None;
    frame(&mut app, &ctx, vec![ctrl_j]);
    assert!(app.agents_open, "on the canvas it opens the agents table");
}

#[cfg(windows)]
#[test]
fn ctrl_insert_and_shift_delete_send_nothing_to_the_shell() {
    // egui-winit makes Ctrl+Insert / Shift+Delete the same copy / cut as Ctrl+C / Ctrl+X: only the held
    // key (this thread's keyboard state) tells them apart, and those two must not reach the shell
    unsafe extern "system" {
        fn GetKeyboardState(keys: *mut u8) -> i32;
        fn SetKeyboardState(keys: *const u8) -> i32;
        fn GetKeyState(key: i32) -> i16;
    }
    const VK_INSERT: usize = 0x2D;
    const VK_DELETE: usize = 0x2E;
    let mut saved = [0u8; 256];
    // SAFETY: a 256-byte buffer, as both calls expect
    unsafe { GetKeyboardState(saved.as_mut_ptr()) };
    let mut probe = saved;
    probe[VK_INSERT] = 0x80;
    unsafe { SetKeyboardState(probe.as_ptr()) };
    let held = unsafe { GetKeyState(VK_INSERT as i32) } < 0;
    unsafe { SetKeyboardState(saved.as_ptr()) };
    if !held {
        // an anti-cheat (Arena Breakout's) makes SetKeyboardState a no-op that still says ok
        eprintln!("SetKeyboardState has no effect here: skipped");
        return;
    }
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = terminal_at(&mut app, &ctx, Pos2::new(400.0, 300.0));
    let script = "[Console]::TreatControlCAsInput = $true; Write-Host pronto; Write-Host (1..3 | % { [int][Console]::ReadKey($true).KeyChar })";
    let session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(("powershell", &["-NoProfile", "-Command", script][..]))).unwrap();
    // a ^C before PowerShell takes it as input would end it
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !session.handle.screen_text().contains("pronto") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // ConPTY takes the new input mode a moment after the text shows: a ^C right away still ended it
    std::thread::sleep(std::time::Duration::from_millis(200));
    app.terminals.insert(id.clone(), Ok(session));
    app.terminal_focus = Some(id.clone());
    for (held, event) in [
        (Some(VK_INSERT), egui::Event::Copy),
        (Some(VK_DELETE), egui::Event::Cut),
        (None, egui::Event::Copy),
        (None, egui::Event::Cut),
        (None, egui::Event::Text("a".into())),
    ] {
        let mut keys = saved;
        if let Some(key) = held {
            keys[key] = 0x80;
        }
        unsafe { SetKeyboardState(keys.as_ptr()) };
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), events: vec![event], ..Default::default() };
        ctx.run_ui(raw, |_| app.terminal_overlay(&ctx)).textures_delta.clear();
    }
    unsafe { SetKeyboardState(saved.as_ptr()) };
    let Some(Ok(session)) = app.terminals.get(&id) else { unreachable!() };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    // ConPTY can hand over the last output after the exit: wait for that line too
    let answered = || session.exited() && session.handle.screen_text().lines().last() != Some("pronto");
    while !answered() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(session.exited());
    // ^C and ^X without the keys held, then the a: nothing from the two with Insert / Delete
    let screen = session.handle.screen_text();
    assert_eq!(screen.lines().last(), Some("3 24 97"), "{screen}");
}

#[test]
fn text_tool_on_a_window_neither_writes_over_it_nor_leaves_the_focus() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = terminal_at(&mut app, &ctx, Pos2::new(400.0, 300.0));
    let at = app.to_screen(local_bounds(app.scene.elements.last().unwrap()).center());
    app.tool = Tool::Text;
    click(&mut app, &ctx, at);
    assert!(!app.scene.elements.iter().any(|e| matches!(e.kind, ElementKind::Text { .. })), "no text on the terminal");
    assert_eq!(app.editing, None);
    assert_eq!(app.terminal_focus.as_ref(), Some(&id), "a click on it works like a normal click");
}

#[test]
fn text_tool_click_on_a_shape_labels_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let rect = app.new_element(ElementKind::Rectangle { x: 300.0, y: 200.0, width: 200.0, height: 100.0 });
    let id = rect.id.clone();
    app.scene.elements = vec![rect];
    app.tool = Tool::Text;
    let at = app.to_screen(Pos2::new(400.0, 250.0));
    click(&mut app, &ctx, at);
    let label = app.editing.expect("typing starts");
    assert_eq!(app.scene.elements[label].id, format!("{id}-label"));
}

#[test]
fn escape_leaves_no_empty_text_and_returns_to_selection() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.tool = Tool::Text;
    click(&mut app, &ctx, Pos2::new(300.0, 300.0));
    frame(&mut app, &ctx, vec![key_press(Key::Escape)]);
    assert!(app.scene.elements.is_empty(), "the empty text is dropped");
    assert_eq!(app.tool, Tool::Select);
}

#[test]
fn command_comma_toggles_settings_and_the_keys_come_back() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let cmd_comma = egui::Event::Key { key: Key::Comma, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    frame(&mut app, &ctx, vec![cmd_comma.clone()]);
    assert!(app.settings_open);
    frame(&mut app, &ctx, vec![cmd_comma]);
    assert!(!app.settings_open, "⌘, closes Ajustes too");
    frame(&mut app, &ctx, vec![key_press(Key::T)]);
    assert_eq!(app.tool, Tool::Text, "closed, the canvas has the keys again");
}

#[cfg(windows)]
#[test]
fn a_prompt_is_not_typed_into_a_menu_and_the_session_file_says_the_state() {
    use crate::app::ui::agents::AgentState;
    // a permission menu on screen: the words would pick one of its options
    let menu = "Write-Host 'Do you want to proceed?'; Write-Host ' 1. Yes'; Write-Host ' 2. No'; Start-Sleep 30";
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(("powershell", &["-NoProfile", "-Command", menu][..]))).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !session.handle.screen_text().contains("2. No") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(session.handle.waiting());
    assert!(session.handle.type_prompt("oi").unwrap_err().contains("menu"));
    // a gate says why first, and lifts
    session.handle.set_gate(Some("não está com o claude aberto".into()), false);
    assert_eq!(session.handle.type_prompt("oi"), Err("não está com o claude aberto".into()));
    session.handle.set_gate(None, false);
    assert!(session.handle.type_prompt("oi").unwrap_err().contains("menu"));
    // a session that says it rests is believed over numbered lines under a question (an answer
    // can end that way); only a menu's own footer would still count
    session.handle.set_gate(None, true);
    assert_eq!(session.handle.type_prompt("oi"), Ok(()));
    // the Claude Code session's word over the screen's
    let mut app = DrawingApp::default();
    app.terminals.insert("t".into(), Ok(session));
    let says =
        |status: &str, ago_ms: i64| crate::transcript::Session { status: status.into(), status_at: crate::model::now_ms() - ago_ms, ..Default::default() };
    app.agent_sessions.insert("t".into(), says("busy", 5000));
    assert!(matches!(app.agent_state("t"), AgentState::Working(t) if (app.now - t - 5.0).abs() < 1.0), "{:?}", app.agent_state("t"));
    app.agent_sessions.insert("t".into(), says("idle", 0));
    assert!(matches!(app.agent_state("t"), AgentState::Idle(Some(_))));
    app.terminal_waiting.insert("t".into());
    assert_eq!(app.agent_state("t"), AgentState::Waiting);
    assert_eq!(app.agent_state("outro"), AgentState::Ended);
}

#[cfg(windows)]
#[test]
fn an_ask_answers_with_what_the_agent_wrote_to_its_transcript() {
    // a stand-in agent: reads the prompt, adds its answer to the transcript, shows something else
    let dir = std::env::temp_dir().join(format!("mago-pergunta-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let (transcript, answer) = (dir.join("s.jsonl"), dir.join("resposta.json"));
    let line = |text: &str| format!("{}\n", serde_json::json!({ "type": "assistant", "message": { "content": text } }));
    fs::write(&transcript, line("antiga")).unwrap();
    fs::write(&answer, line("resposta completa")).unwrap();
    let script = format!(
        "$null = [Console]::ReadLine(); Get-Content -LiteralPath '{}' | Add-Content -LiteralPath '{}'; Write-Host 'so o fim'; Start-Sleep 30",
        answer.display(),
        transcript.display()
    );
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(("powershell", &["-NoProfile", "-Command", &script][..]))).unwrap();
    // through the bridge, as `magoagent ask B pergunta` from outside the canvas
    let node = crate::bridge::Node {
        id: "b".into(),
        kind: crate::bridge::NodeKind::Terminal,
        name: "B".into(),
        text: String::new(),
        links: vec![],
        maestro: false,
        hidden: false,
    };
    let shared = std::sync::Mutex::new(crate::bridge::Shared {
        nodes: vec![node],
        terminals: [("b".to_string(), session.handle.clone())].into(),
        transcripts: [("b".to_string(), transcript.clone())].into(),
        ..Default::default()
    });
    let (sender, _commands) = std::sync::mpsc::channel();
    let reply = crate::bridge::handle(&serde_json::json!({ "cmd": "ask", "name": "B", "text": "pergunta", "timeout": 30 }), &shared, &sender);
    assert_eq!(reply, Ok("resposta completa".to_string()));
    // typed, then answered: the chat's question and the events
    let shared = shared.into_inner().unwrap();
    let outside = "um agente fora do canvas".to_string();
    assert_eq!(shared.asked, [("b".to_string(), "pergunta".to_string(), outside.clone())]);
    assert_eq!(shared.events, [("b".to_string(), format!("recebeu uma pergunta de {outside}")), ("b".to_string(), format!("respondeu a {outside}"))]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_chat_box_grows_with_its_text_up_to_its_cap() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut agent = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    agent.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    app.scene.elements.push(agent);
    app.chat_open = true;
    let size = Vec2::new(1000.0, 800.0);
    let height = |app: &mut DrawingApp, draft: &str| {
        app.chat_draft = draft.into();
        for _ in 0..3 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
            ctx.run_ui(raw, |_| app.chat_screen(&ctx)).textures_delta.clear();
        }
        app.chat_composer
    };
    let one = height(&mut app, "oi");
    assert!(one > 20.0 && one < 80.0, "{one}");
    let cap = 220f32.min((size.y - TOP_ROW) * 0.4);
    let long = height(&mut app, &"linha\n".repeat(40));
    assert!(long > one * 3.0 && long <= cap + 1.0, "{long} up to {cap}");
}

#[cfg(windows)]
#[test]
fn the_events_are_what_the_states_prove() {
    let mut app = DrawingApp::default();
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    e.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    let session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(("powershell", &["-NoProfile", "-Command", "Start-Sleep 30"][..]))).unwrap();
    app.terminals.insert(id.clone(), Ok(session));
    let step = |app: &mut DrawingApp, status: &str| {
        let waiting_for = (status == "waiting").then(|| "input needed".to_string());
        app.agent_sessions
            .insert(id.clone(), crate::transcript::Session { status: status.into(), status_at: crate::model::now_ms(), waiting_for, ..Default::default() });
        app.terminal_waiting = if status == "waiting" { [id.clone()].into() } else { Default::default() };
        app.log_agent_events();
    };
    for status in ["idle", "busy", "busy", "waiting", "busy", "idle", "idle"] {
        step(&mut app, status);
    }
    // a shell that went: ended; restarted: started again
    app.terminals.remove(&id);
    app.log_agent_events();
    let again = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(("powershell", &["-NoProfile", "-Command", "Start-Sleep 30"][..]))).unwrap();
    app.terminals.insert(id.clone(), Ok(again));
    step(&mut app, "idle");
    let said: Vec<String> = app.agent_events.iter().map(|e| format!("{} {}", e.name, e.what)).collect();
    let expected = ["iniciado", "começou a trabalhar", "precisa de você (input needed)", "começou a trabalhar", "terminou", "encerrado", "iniciado"]
        .map(|what| format!("Revisor {what}"));
    assert_eq!(said, expected);
}

#[test]
fn undoing_a_terminals_rope_to_its_ide_says_to_restart_its_claude() {
    let mut app = DrawingApp::default();
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    terminal.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    let mut ide = app.new_element(ElementKind::Rectangle { x: 600.0, y: 0.0, width: 400.0, height: 300.0 });
    ide.ide = Some(IdeSpec { name: "proj".into(), ..Default::default() });
    let (t, i) = (terminal.id.clone(), ide.id.clone());
    app.scene.elements.extend([terminal, ide]);
    app.connect(&t, &i);
    // its claude got that IDE's port when it started
    app.terminal_ides.insert(t.clone(), (i.clone(), false));
    app.now = 1.0;
    app.check_agents();
    assert!(app.status.is_empty(), "still roped: nothing to say");
    let ropes: Vec<Element> = app.scene.elements.iter().filter(|e| e.id != t && e.id != i).cloned().collect();
    app.scene.elements.retain(|e| e.id == t || e.id == i);
    app.now = 2.0;
    app.check_agents();
    assert_eq!(app.status, "Reinicie o terminal \"Revisor\" para o claude dele largar o IDE.");
    app.status.clear();
    app.now = 3.0;
    app.check_agents();
    assert!(app.status.is_empty(), "said once");
    // roped again and undone again: said again (the shell still has that port)
    app.scene.elements.extend(ropes);
    app.now = 4.0;
    app.check_agents();
    app.scene.elements.retain(|e| e.id == t || e.id == i);
    app.now = 5.0;
    app.check_agents();
    assert!(app.status.starts_with("Reinicie o terminal"));
}
