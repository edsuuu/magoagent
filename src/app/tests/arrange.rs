//! Organizar o canvas (canvas/editing): every window, note and drawing in rows with room between them, what a rope
//! ties side by side, a drawing kept whole, the locked left alone; the same by the MCP's `organize_canvas`.

use super::*;

fn window(app: &DrawingApp, name: &str, x: f32, y: f32) -> Element {
    let mut element = app.new_element(ElementKind::Rectangle { x, y, width: 400.0, height: 300.0 });
    element.terminal = Some(TerminalSpec { name: name.into(), cwd: "/p".into(), ..Default::default() });
    element
}

/// Two terminals piled on each other, one far away, a note roped to the first, a drawing of two boxes and an arrow
/// between them, and a locked box.
fn messy_canvas() -> DrawingApp {
    let mut app = DrawingApp::default();
    app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
    let (first, second, far) = (window(&app, "Um", 0.0, 0.0), window(&app, "Dois", 30.0, 20.0), window(&app, "Três", 2000.0, 0.0));
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(4000.0, 3000.0), Vec2::splat(200.0)), "nota".into());
    let left = app.new_element(ElementKind::Rectangle { x: 0.0, y: 5000.0, width: 100.0, height: 80.0 });
    let right = app.new_element(ElementKind::Rectangle { x: 300.0, y: 5000.0, width: 100.0, height: 80.0 });
    let mut arrow = app.new_element(ElementKind::Line { points: vec![[100.0, 5040.0], [300.0, 5040.0]], arrow: true });
    let bind = |id: &str| Some(Binding { element_id: id.into(), fixed: [0.5, 0.5], inside: false });
    arrow.ends = ArrowEnds { start_head: None, end_head: None, start: bind(&left.id), end: bind(&right.id) };
    let mut locked = app.new_element(ElementKind::Rectangle { x: -900.0, y: -900.0, width: 50.0, height: 50.0 });
    locked.locked = true;
    let (first_id, note_id) = (first.id.clone(), note.id.clone());
    app.scene.elements = vec![first, second, far, note, label, left, right, arrow, locked];
    app.rope_between(&first_id, &note_id, false);
    app
}

fn bounds_of(app: &DrawingApp, index: usize) -> Rect {
    local_bounds(&app.scene.elements[index])
}

#[test]
fn organizar_o_canvas_puts_everything_in_rows_with_room_between() {
    let mut app = messy_canvas();
    let drawing_before = bounds_of(&app, 6).min - bounds_of(&app, 5).min;
    let locked_before = bounds_of(&app, 8);
    assert_eq!(app.arrange_canvas(None), 5, "three windows, the note and the drawing; the locked none");
    // no two closer than 60
    let boxes: Vec<Rect> = [0, 1, 2, 3].map(|index| bounds_of(&app, index)).into_iter().chain([bounds_of(&app, 5).union(bounds_of(&app, 6))]).collect();
    for (index, one) in boxes.iter().enumerate() {
        for other in &boxes[index + 1..] {
            assert!(!one.expand(59.0).intersects(*other), "{one:?} {other:?}");
        }
    }
    // the note roped to Um comes right after it, before Dois that was next to it
    let (first, note) = (bounds_of(&app, 0), bounds_of(&app, 3));
    assert_eq!(note.min, Pos2::new(first.max.x + 60.0, first.min.y));
    // the drawing keeps its shape and its arrow follows; the locked box stays
    assert_eq!(bounds_of(&app, 6).min - bounds_of(&app, 5).min, drawing_before);
    let (start, end) = (app.line_points(7)[0], *app.line_points(7).last().unwrap());
    assert!(bounds_of(&app, 5).expand(20.0).contains(Pos2::new(start[0], start[1])), "{start:?}");
    assert!(bounds_of(&app, 6).expand(20.0).contains(Pos2::new(end[0], end[1])), "{end:?}");
    assert_eq!(bounds_of(&app, 8), locked_before);
    // one undo puts it all back
    app.undo();
    assert_eq!(bounds_of(&app, 1).min, Pos2::new(30.0, 20.0));
    assert_eq!(bounds_of(&app, 3).min, Pos2::new(4000.0, 3000.0));
}

#[test]
fn the_agent_organizes_the_canvas_by_the_mcp() {
    let mut app = messy_canvas();
    assert_eq!(app.act("organize_canvas", &json!({})).unwrap(), "5 itens organizados em fileiras, com 60 px entre eles.");
    // only some: by name or id
    let mut app = messy_canvas();
    let note_id = app.scene.elements[3].id.clone();
    assert!(app.act("organize_canvas", &json!({ "elements": ["Um", "Dois", note_id] })).unwrap().starts_with("3 itens"));
    assert_eq!(bounds_of(&app, 2).min, Pos2::new(2000.0, 0.0), "Três was not asked");
    assert!(app.act("organize_canvas", &json!({ "elements": ["Um"] })).unwrap_err().contains("2 ou mais"));
    assert!(app.act("organize_canvas", &json!({ "elements": ["ninguém"] })).is_err());
    // the canvas menu's row does the same
    let mut app = messy_canvas();
    app.context_action(&Context::default(), &crate::app::ui::context::Target::Canvas(Pos2::ZERO), "arrange");
    assert_eq!(bounds_of(&app, 3).min, Pos2::new(bounds_of(&app, 0).max.x + 60.0, bounds_of(&app, 0).min.y));
}

/// What the MCP draws: a shape and its label share a `group_id`; an arrow between two such shapes. It stays one
/// drawing (it was split, a shape apart from the other). A canvas group (Grupo) moves whole.
#[test]
fn a_labelled_drawing_and_a_canvas_group_move_whole() {
    let mut app = DrawingApp::default();
    app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
    let labelled = |app: &DrawingApp, name: &str, x: f32| {
        let mut shape = app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 140.0, height: 80.0 });
        let mut label = app.new_element(ElementKind::Text { x: x + 40.0, y: 30.0, value: name.into() });
        (shape.group_id, label.group_id) = (Some(format!("g-{name}")), Some(format!("g-{name}")));
        label.id = format!("{}-label", shape.id);
        [shape, label]
    };
    let ([api, api_label], [bank, bank_label]) = (labelled(&app, "API", 0.0), labelled(&app, "Banco", 240.0));
    let mut arrow = app.new_element(ElementKind::Line { points: vec![[140.0, 40.0], [240.0, 40.0]], arrow: true });
    let bind = |id: &str| Some(Binding { element_id: id.into(), fixed: [0.5, 0.5], inside: false });
    arrow.ends = ArrowEnds { start_head: None, end_head: None, start: bind(&api.id), end: bind(&bank.id) };
    let [first, first_label] = app.new_note(Rect::from_min_size(Pos2::new(0.0, 600.0), Vec2::splat(200.0)), "um".into());
    let [second, second_label] = app.new_note(Rect::from_min_size(Pos2::new(220.0, 600.0), Vec2::splat(200.0)), "dois".into());
    let [far, far_label] = app.new_note(Rect::from_min_size(Pos2::new(3000.0, 3000.0), Vec2::splat(200.0)), "longe".into());
    app.scene.elements = vec![api, api_label, bank, bank_label, arrow, first, first_label, second, second_label, far, far_label];
    app.selected = vec![5, 7];
    app.group_selection();
    app.rename = None;
    let (drawing, group) = (bounds_of(&app, 2).min - bounds_of(&app, 0).min, bounds_of(&app, 7).min - bounds_of(&app, 5).min);
    assert_eq!(app.arrange_canvas(None), 3, "the drawing, the group and the far note");
    assert_eq!(bounds_of(&app, 2).min - bounds_of(&app, 0).min, drawing);
    assert_eq!(bounds_of(&app, 7).min - bounds_of(&app, 5).min, group);
}
