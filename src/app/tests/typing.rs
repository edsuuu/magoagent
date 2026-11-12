use super::*;

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
    frame(&mut app, &ctx, vec![key_press(Key::H)]);
    assert_eq!(app.tool, Tool::Hand, "closed, the canvas has the keys again");
}

#[test]
fn a_stray_letter_on_the_bare_canvas_arms_no_drawing_tool() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // the keys of a window that lost them: with the drawing row closed they make nothing
    for key in [Key::R, Key::T, Key::N, Key::E, Key::Num2] {
        frame(&mut app, &ctx, vec![key_press(key)]);
        assert_eq!(app.tool, Tool::Select, "{key:?}");
    }
    // Mão and Seleção make nothing either way: theirs always answer
    frame(&mut app, &ctx, vec![key_press(Key::H)]);
    assert_eq!(app.tool, Tool::Hand);
    frame(&mut app, &ctx, vec![key_press(Key::V)]);
    assert_eq!(app.tool, Tool::Select);
    // the row open, the letters pick their tools; a drawing tool in hand keeps them on
    app.draw_open = true;
    frame(&mut app, &ctx, vec![key_press(Key::R)]);
    assert_eq!(app.tool, Tool::Rectangle);
    app.draw_open = false;
    frame(&mut app, &ctx, vec![key_press(Key::O)]);
    assert_eq!(app.tool, Tool::Ellipse);
}
