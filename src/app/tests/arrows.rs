use super::*;

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

#[test]
fn a_prompt_that_never_reached_its_terminal_comes_back_to_its_box() {
    use crate::app::ui::nodes::Returned;
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    let id = app.scene.elements[0].id.clone();
    let back = |id: &str, text: &str| Returned { id: id.into(), text: text.into(), why: "O terminal não respondeu.".into() };
    app.returned_prompts.0.send(back(&id, "revise o login")).unwrap();
    app.returned_prompts(&Context::default());
    assert_eq!(app.composer, Some((id.clone(), "revise o login".into())));
    // its box already open: added below what is there
    app.returned_prompts.0.send(back(&id, "e o cadastro")).unwrap();
    app.returned_prompts(&Context::default());
    assert_eq!(app.composer, Some((id, "revise o login\n\ne o cadastro".into())));
    // a terminal no longer drawn: the clipboard, the box stays as it was
    app.returned_prompts.0.send(back("sumiu", "x")).unwrap();
    app.returned_prompts(&Context::default());
    assert!(app.composer.as_ref().is_some_and(|(_, text)| !text.contains('x')));
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

/// A drawing made after a window still stays under it: the click over both finds the window.
#[test]
fn windows_stay_over_the_drawings() {
    let app = &mut DrawingApp::default();
    let mut portal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 300.0, height: 200.0 });
    portal.portal = Some(PortalSpec { name: "Docs".into(), url: "about:blank".into(), ..Default::default() });
    let mut shape = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 300.0, height: 200.0 });
    shape.style.fill = Some([128, 128, 128, 255]);
    app.scene.elements = vec![portal, shape];
    assert_eq!(app.hit(Pos2::new(150.0, 150.0)), Some(0));
    assert_eq!(app.hit(Pos2::new(350.0, 250.0)), Some(1));
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
fn snapping_to_the_grid() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    frame(&mut app, &ctx, vec![]);
    app.prefs.grid = true;
    app.tool = Tool::Rectangle;
    drag(&mut app, &ctx, Pos2::new(452.0, 407.0), Pos2::new(538.0, 489.0));
    let drawn = element_bounds(app.scene.elements.last().unwrap());
    assert_eq!((drawn.min, drawn.max), (Pos2::new(460.0, 400.0), Pos2::new(540.0, 480.0)), "grid snaps to 20px");
}

#[test]
fn an_arrow_always_binds_and_snaps_to_a_sides_middle_unless_cmd_is_held() {
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    app.tool = Tool::Arrow;
    // released near the right side's middle: snaps the anchor to [1, 0.5]
    let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[395.0, 262.0], [605.0, 250.0]], arrow: true }));
    assert_eq!(arrow.ends.start.as_ref().unwrap().fixed, [1.0, 0.5]);
    assert!(arrow.ends.end.is_some());
    app.mods.command = true;
    let free = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[395.0, 262.0], [605.0, 250.0]], arrow: true }));
    assert!(free.ends.start.is_none() && free.ends.end.is_none());
}

#[test]
fn an_arrow_tip_reaching_a_shape_pulses_it_once() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    app.tool = Tool::Arrow;
    frame(&mut app, &ctx, vec![]);
    let start = Pos2::new(500.0, 100.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
    let pulse = |app: &DrawingApp| {
        let p = app.bind_pulse.take();
        app.bind_pulse.set(p.clone());
        p
    };
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(520.0, 120.0))]);
    painted(&app, &ctx);
    assert_eq!(pulse(&app), None, "far from both boxes");
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(605.0, 250.0))]);
    app.now = 1.0;
    painted(&app, &ctx);
    let target = app.scene.elements[1].id.clone();
    assert_eq!(pulse(&app), Some((target.clone(), 1.0)), "the tip reached the right box: it pulses from now");
    app.now = 1.2;
    painted(&app, &ctx);
    assert_eq!(pulse(&app), Some((target, 1.0)), "still on it: the same pulse, not a new one");
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
