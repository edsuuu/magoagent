use super::*;

fn pointer(app: &mut DrawingApp, phase: &str, x: f32, y: f32) {
    let args = json!({ "phase": phase, "px": x, "py": y, "x": 0.0, "y": 0.0, "zoom": 1.0, "width": 1200.0, "height": 800.0 });

    app.native_canvas_act("pointer", &args).unwrap();
}

/// A note at `rect` with `text`, its card's index.
fn add_note(app: &mut DrawingApp, rect: Rect, text: &str) -> usize {
    let note = app.new_note(rect, text.into());

    app.scene.elements.extend(note);
    app.scene.elements.len() - 2
}

#[test]
fn a_rectangle_drawn_from_swift_lands_in_the_scene_with_its_rough_strokes() {
    let mut app = DrawingApp::default();

    app.native_canvas_act("tool", &json!({ "name": "rectangle" })).unwrap();
    pointer(&mut app, "down", 100.0, 100.0);
    pointer(&mut app, "move", 200.0, 150.0);
    // mid-drag the shape is the overlay's draft, not yet in the scene
    let mut native = Native::default();

    assert!(app.overlay(&mut native).draft.is_some_and(|draft| !draft.ops.is_empty()));
    pointer(&mut app, "move", 300.0, 220.0);
    pointer(&mut app, "up", 300.0, 220.0);
    assert_eq!(app.scene.elements.len(), 1);
    assert_eq!(element_bounds(&app.scene.elements[0]), Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 220.0)));

    let changed = app.sync_scene(&mut native);

    assert_eq!(changed.len(), 1);
    let (id, drawing) = &changed[0];

    assert_eq!(*id, app.scene.elements[0].id);
    // the same strokes egui paints: rough_outlines, two passes of four edges
    let strokes = drawing.ops.iter().filter(|op| matches!(op, Op::Stroke { .. })).count();

    assert_eq!(strokes, rough_outlines(&app.scene.elements[0]).len());
    assert_eq!(native.list.items, vec![Item { id: id.clone(), rev: drawing.rev, card: false }]);
    let version = native.list.version;

    // nothing changed: nothing is sent again
    assert!(app.sync_scene(&mut native).is_empty());
    assert_eq!(native.list.version, version);

    // one element moved: only it comes back, with a new revision
    app.snapshot();
    app.scene.elements.push(app.new_element(ElementKind::Ellipse { x: 0.0, y: 0.0, width: 10.0, height: 10.0 }));
    app.sync_scene(&mut native);
    app.scene.elements[1].move_by(Vec2::new(5.0, 0.0));
    let changed = app.sync_scene(&mut native);

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].0, app.scene.elements[1].id);
    assert!(native.list.version > version);

    // undo through the key takes the ellipse back out
    app.native_canvas_act("key", &json!({ "key": "Z", "cmd": true })).unwrap();
    app.sync_scene(&mut native);
    assert_eq!(native.list.items.len(), 1);
}

#[test]
fn windows_paint_over_drawings_and_carry_their_kind() {
    let mut app = DrawingApp::default();
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 300.0, height: 200.0 });

    terminal.terminal = Some(TerminalSpec { name: "build".into(), ..Default::default() });
    let line = app.new_element(ElementKind::Line { points: vec![[0.0, 0.0], [50.0, 50.0]], arrow: true });

    app.scene.elements = vec![terminal, line];
    let mut native = Native::default();
    let changed: HashMap<String, Drawing> = app.sync_scene(&mut native).into_iter().collect();
    let order: Vec<&str> = native.list.items.iter().map(|item| item.id.as_str()).collect();

    assert_eq!(order, [app.scene.elements[1].id.as_str(), app.scene.elements[0].id.as_str()]);
    assert!(native.list.items[1].card && !native.list.items[0].card);
    assert_eq!(changed[&app.scene.elements[0].id].node, Some(Node { kind: "terminal", name: "build".into(), header: TERMINAL_HEADER }));
    // the arrow's head is a stroke after its line
    assert!(matches!(changed[&app.scene.elements[1].id].ops.last(), Some(Op::Stroke { closed: false, .. })));
}

#[test]
fn notes_and_windows_stack_in_the_drawings_order_over_the_drawings() {
    let mut app = DrawingApp::default();
    let note = add_note(&mut app, Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(200.0, 150.0)), "# Por cima");
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 300.0, height: 200.0 });

    terminal.terminal = Some(TerminalSpec { name: "build".into(), ..Default::default() });
    app.scene.elements.push(terminal);
    let shape = app.new_element(ElementKind::Ellipse { x: 400.0, y: 0.0, width: 50.0, height: 50.0 });

    app.scene.elements.push(shape);
    let mut native = Native::default();

    app.sync_scene(&mut native);
    let ids = |native: &Native| native.list.items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    let (note_id, terminal_id, shape_id) = (app.scene.elements[note].id.clone(), app.scene.elements[2].id.clone(), app.scene.elements[3].id.clone());

    // the note's text paints with its card; the drawing under both cards, the terminal (later) over the note
    assert_eq!(ids(&native), [shape_id.clone(), note_id.clone(), terminal_id.clone()]);
    // typed in, the note comes to the front (raise_card, as egui's text_editor does)
    let note = app.raise_card(note);

    app.sync_scene(&mut native);
    assert_eq!(ids(&native), [shape_id, terminal_id, note_id]);
    assert!(native.list.items[1].card && native.list.items[2].card);
    assert!(app.scene.elements[note].created.is_some());
}

#[test]
fn a_note_carries_its_live_text_its_buttons_and_its_piles_tabs() {
    let mut app = DrawingApp::default();
    let first = add_note(&mut app, Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0)), "# Plano\n- item");

    app.scene.elements[first].note.buttons = vec![NoteButton { label: "Novo chat".into(), action: "chat_new".into(), args: json!({}) }];
    let mut native = Native::default();
    let changed: HashMap<String, Drawing> = app.sync_scene(&mut native).into_iter().collect();
    let note = changed[&app.scene.elements[first].id].note.clone().unwrap();
    // at rest the heading's "# " hides (next to no size) and the rest is at the heading's size; the bullet is a mark
    let ink = |run: &markdown::LiveRun| run.ink;

    assert_eq!((note.runs[0].from, note.runs[0].to, ink(&note.runs[0])), (0, 2, 0.0));
    assert_eq!(note.runs[1].size, markdown::size_of(1, note.size));
    assert_eq!(note.marks, marks(&[markdown::Ornament::Bullet(8)]));
    // the button sits at the foot of the card, inside its padding
    let [x, y, _, height] = note.buttons[0].rect;

    assert_eq!((x, y + height), (NOTE_PAD, 300.0 - NOTE_PAD));
    assert!(note.tabs.is_empty());

    // a pile: its front page has a tab per page, the front's last (over the others)
    let second = add_note(&mut app, Rect::from_min_size(Pos2::new(400.0, 0.0), Vec2::new(300.0, 300.0)), "# Dois");

    app.selected = vec![first, second];
    assert!(app.stack_notes());
    let changed: HashMap<String, Drawing> = app.sync_scene(&mut native).into_iter().collect();
    let front = app.selected[0];
    let note = changed[&app.scene.elements[front].id].note.clone().unwrap();

    assert_eq!(note.tabs.len(), 2);
    assert!(note.tabs[1].front && note.tabs[1].id == app.scene.elements[front].id);
    assert!(note.tabs[0].rect[0] > 300.0 - 3.0, "the rail stands out of the page's right edge");
    // only the front page is listed
    assert_eq!(native.list.items.len(), 1);
}

#[test]
fn the_note_typed_in_shows_the_marks_of_the_line_the_caret_is_on() {
    let mut app = DrawingApp::default();
    let note = add_note(&mut app, Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0)), "# Um\n# Dois");

    app.begin_text_edit(note);
    let mut native = Native::default();

    // the caret on the second line: its "# " shows faint, the first line's hides
    app.native_canvas_act("text", &json!({ "value": "# Um\n# Dois", "head": 7, "anchor": 7 })).unwrap();
    NATIVE.with_borrow(|shared| native.caret = shared.caret);
    let editing = app.overlay(&mut native).editing.unwrap();

    assert!(editing.note.as_ref() == Some(&app.scene.elements[note].id) && !editing.raw);
    let at = |index: usize| editing.runs.iter().find(|run| (run.from..run.to).contains(&index)).unwrap().ink;

    assert_eq!((at(0), at(5)), (0.0, 0.4));
    assert_eq!(app.note_selection, Some((note + 1, 7, 7)));
}

#[test]
fn the_camera_serial_moves_only_when_rust_moves_the_camera() {
    let mut app = DrawingApp::default();
    let mut native = Native::default();
    let first = app.overlay(&mut native).camera;

    // Swift's own pan: nothing goes back (the topic stays as it was)
    app.native_view(&json!({ "x": 40.0, "y": -20.0, "zoom": 2.0, "width": 800.0, "height": 600.0 }));
    NATIVE.with_borrow(|shared| native.camera = shared.camera);
    assert_eq!(app.overlay(&mut native).camera, first);
    app.zoom_at(1.0, Pos2::new(400.0, 300.0));
    let moved = app.overlay(&mut native).camera;

    assert_eq!((moved.serial, moved.x, moved.y, moved.zoom), (first.serial + 1, app.origin().x, app.origin().y, app.zoom));
}

#[test]
fn the_minimap_takes_the_view_where_it_is_pressed() {
    let mut app = DrawingApp::default();

    app.native_view(&json!({ "x": 0.0, "y": 0.0, "zoom": 1.0, "width": 800.0, "height": 600.0 }));
    app.scene.elements.push(app.new_element(ElementKind::Rectangle { x: 2000.0, y: 2000.0, width: 100.0, height: 100.0 }));
    let mut native = Native::default();

    assert!(app.overlay(&mut native).minimap.is_none(), "closed");
    app.prefs.minimap = true;
    let map = app.overlay(&mut native).minimap.unwrap();

    assert_eq!(map.nodes.len(), 1);
    let before = app.visible_world().center();
    // the shape's dot pressed: the view goes to it
    let [x, y, width, height] = map.nodes[0].rect;

    app.native_canvas_act("minimap", &json!({ "phase": "down", "x": x + width / 2.0, "y": y + height / 2.0 })).unwrap();
    app.native_canvas_act("minimap", &json!({ "phase": "up", "x": x + width / 2.0, "y": y + height / 2.0 })).unwrap();
    let after = app.visible_world().center();

    assert!(after.distance(Pos2::new(2050.0, 2050.0)) < before.distance(Pos2::new(2050.0, 2050.0)));
}

#[test]
fn note_buttons_link_marks_and_the_enlarged_windows_grip_answer_swift() {
    let mut app = DrawingApp::default();

    app.native_view(&json!({ "x": 0.0, "y": 0.0, "zoom": 1.0, "width": 1400.0, "height": 900.0 }));
    let note = add_note(&mut app, Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0)), "Nota");

    app.scene.elements[note].note.buttons = vec![NoteButton { label: "Nada".into(), action: "nada".into(), args: Value::Null }];
    let id = app.scene.elements[note].id.clone();

    // Remover botão, from its menu
    app.native_canvas_act("note_button", &json!({ "id": id, "index": 0, "remove": true })).unwrap();
    assert!(app.scene.elements[note].note.buttons.is_empty());

    // a drawing with a link has its mark; without one the verb says so
    let mut shape = app.new_element(ElementKind::Rectangle { x: 500.0, y: 0.0, width: 80.0, height: 80.0 });

    shape.link = Some("https://example.com".into());
    let shape_id = shape.id.clone();

    app.scene.elements.push(shape);
    let mut native = Native::default();

    assert_eq!(app.overlay(&mut native).links.iter().map(|mark| mark.id.clone()).collect::<Vec<_>>(), std::slice::from_ref(&shape_id));
    assert!(app.native_canvas_act("link", &json!({ "id": id })).is_err());
    // "Voltar" while the link's tree is on the canvas; its × goes without going back
    let (pan, zoom) = (app.pan, app.zoom);
    let tree = shape_id;

    app.filetrees.link_back =
        Some(crate::app::filetree::link::LinkReturn { chat_open: false, chat_agent: None, pan, zoom, elevated: None, card_focus: None, tree });

    assert_eq!(app.overlay(&mut native).back, Some("Voltar"));
    app.native_canvas_act("link_dismiss", &json!({})).unwrap();
    assert!(app.overlay(&mut native).back.is_none());

    // enlarged, its title docks it at the side it is dragged to, and a double click puts it back
    app.set_elevated(Some(id.clone()));
    app.now = app.elevate_since + 1.0;
    let lift = app.overlay(&mut native).lift.unwrap();

    assert_eq!(lift.id, id);
    let grip = lift.grip.unwrap();
    let start = Pos2::new(grip[0] + 20.0, grip[1] + 4.0);

    app.native_canvas_act("dock", &json!({ "phase": "start", "x": start.x, "y": start.y })).unwrap();
    app.native_canvas_act("dock", &json!({ "phase": "move", "x": 1390.0, "y": start.y })).unwrap();
    assert!(app.overlay(&mut native).lift.unwrap().zone.is_some(), "where it would dock shows");
    app.native_canvas_act("dock", &json!({ "phase": "end", "x": 1390.0, "y": start.y })).unwrap();
    assert_eq!(app.dock.map(|(right, _)| right), Some(true));
    app.native_canvas_act("dock", &json!({ "phase": "restore" })).unwrap();
    assert!(app.terminal_elevated.is_none());

    // an enlarged tree zooms itself (Swift's pinch over it), and shows at that scale
    let mut tree = app.new_element(ElementKind::Rectangle { x: 0.0, y: 400.0, width: 400.0, height: 300.0 });

    tree.filetree = Some(FileTreeSpec { root: "/".into(), ..Default::default() });
    let tree_id = tree.id.clone();

    app.scene.elements.push(tree);
    app.set_elevated(Some(tree_id));
    app.now = app.elevate_since + 1.0;
    let before = app.overlay(&mut native).lift.unwrap().zoom;

    app.native_canvas_act("lifted_zoom", &json!({ "factor": 0.1_f32.exp() })).unwrap();
    assert!((app.lifted_zoom - 1.1).abs() < 1e-4, "{}", app.lifted_zoom);
    assert!(app.overlay(&mut native).lift.unwrap().zoom > before);
}
