use super::*;

#[test]
fn save_format_round_trips_scene() {
    let scene = Scene { elements: vec![rect_element(5.0, 8.0, 40.0, 30.0, None)], ..Default::default() };
    let decoded: Scene = serde_json::from_str(&serde_json::to_string(&scene).unwrap()).unwrap();
    assert_eq!(decoded, scene);
}

#[test]
fn image_scene_round_trips_and_history_preference_defaults() {
    let mut element = rect_element(0.0, 0.0, 3.0, 4.0, None);
    element.image = Some("data:image/png;base64,AA==".into());
    let scene = Scene { elements: vec![element], ..Default::default() };
    let saved = serde_json::to_string(&scene).unwrap();
    assert_eq!(parse_scene(&saved).unwrap(), scene);
    assert_eq!(serde_json::from_str::<Prefs>("{}").unwrap().terminal_history, 10_000);
    let prefs = Prefs { terminal_history: 5_000, ..Default::default() };
    assert_eq!(serde_json::from_str::<Prefs>(&serde_json::to_string(&prefs).unwrap()).unwrap().terminal_history, 5_000);
}

#[test]
fn terminal_config_uses_requested_history() {
    assert_eq!(crate::terminal::terminal_config(1_000).scrolling_history, 1_000);
    assert_eq!(crate::terminal::terminal_config(5_000).scrolling_history, 5_000);
}

#[test]
fn legacy_rea_files_still_load() {
    let old = r#"{"version":1,"elements":[{"id":"a","kind":{"type":"rectangle","x":1,"y":2,"width":3,"height":4},"stroke":[1,2,3,255],"fill":null,"stroke_width":4}]}"#;
    let scene = parse_scene(old).unwrap();
    assert_eq!(scene.elements[0].style.stroke_width, 4.0);
    assert_eq!(scene.elements[0].style.roughness, 1);
}

#[test]
fn interop_format_round_trips() {
    let mut app = DrawingApp::default();
    app.scene.elements = vec![
        rect_element(10.0, 20.0, 50.0, 40.0, Some([255, 201, 201, 255])),
        app.new_element(ElementKind::Line { points: vec![[0.0, 0.0], [30.0, 40.0]], arrow: true }),
        app.new_element(ElementKind::Text { x: 5.0, y: 6.0, value: "Olá\nmundo".into() }),
    ];
    app.scene.elements[0].style.fill_style = FillStyle::Hachure;
    app.scene.elements[0].angle = 0.5;
    app.scene.elements[1].style.stroke_style = StrokeStyle::Dashed;
    let back = round_trip(&app.scene.elements);
    assert_eq!(back.len(), 3);
    assert_eq!(back[0].kind, app.scene.elements[0].kind);
    assert_eq!(back[0].style.fill_style, FillStyle::Hachure);
    assert!((back[0].angle - 0.5).abs() < 1e-6);
    assert_eq!(back[1].kind, app.scene.elements[1].kind);
    assert_eq!(back[1].style.stroke_style, StrokeStyle::Dashed);
    assert_eq!(back[2].kind, app.scene.elements[2].kind);
}

#[test]
fn terminal_look_round_trips_and_old_files_default_it() {
    let mut plain = rect_element(0.0, 0.0, 300.0, 200.0, None);
    plain.terminal = Some(TerminalSpec { name: "a".into(), command: "claude".into(), ..Default::default() });
    let mut tinted = plain.clone();
    tinted.terminal = Some(TerminalSpec {
        icon: "eye".into(),
        color: Some([1, 2, 3]),
        quiet: true,
        maestro: false,
        chat: true,
        unloaded: true,
        claude_session: "0b6c-41".into(),
        claude_folder: "-p".into(),
        resume: false,
        ..plain.terminal.clone().unwrap()
    });
    // no tint is null on the wire and None back
    assert_eq!(interop_elements(&[plain.clone()])[0]["color"], Value::Null);
    let back = round_trip(&[plain.clone(), tinted.clone()]);
    assert_eq!((&back[0].terminal, &back[1].terminal), (&plain.terminal, &tinted.terminal));
    // scenes and prefs saved before the fields existed: monitoring on, the default quick starts
    let old: TerminalSpec = serde_json::from_str(r#"{"name":"a","cwd":"","command":"claude"}"#).unwrap();
    assert_eq!((old.icon.as_str(), old.color, old.quiet), ("", None, false));
    assert!(old.maestro, "scenes saved before Maestro are Maestros");
    assert!(!old.unloaded && !serde_json::to_string(&TerminalSpec::default()).unwrap().contains("unloaded"), "written only when set");
    assert!(!old.chat && !serde_json::to_string(&TerminalSpec::default()).unwrap().contains("chat"), "chat mode off by default, written only when set");
    // the conversation it remembers: in the .mago too, written only when there is one; resuming on by default
    let saved: TerminalSpec = serde_json::from_str(&serde_json::to_string(&tinted.terminal).unwrap()).unwrap();
    assert_eq!(Some(saved), tinted.terminal);
    assert!(old.resume && old.claude_session.is_empty() && !serde_json::to_string(&TerminalSpec::default()).unwrap().contains("claude_session"));
    assert!(interop_elements(&[plain.clone()])[0].get("claudeSession").is_none());
    assert_eq!(serde_json::from_str::<Prefs>("{}").unwrap().presets, default_presets());
}

#[test]
fn ide_element_round_trips_through_both_formats() {
    let mut e = rect_element(10.0, 20.0, 720.0, 460.0, None);
    // its file panel open: the tree's width comes back too, or the panel would be lost
    e.ide = Some(IdeSpec { name: "loja".into(), root: "/tmp/loja".into(), tree_width: Some(300.0) });
    assert!(e.is_node());
    let scene = Scene { elements: vec![e.clone()], ..Default::default() };
    let saved = serde_json::to_string(&scene).unwrap();
    assert_eq!(parse_scene(&saved).unwrap(), scene);
    let back = round_trip(&[e.clone()]);
    assert_eq!((back[0].ide.as_ref(), back[0].kind.clone()), (e.ide.as_ref(), e.kind.clone()));
    // plain shapes don't grow the field in the saved file
    assert!(!serde_json::to_string(&rect_element(0.0, 0.0, 1.0, 1.0, None)).unwrap().contains("\"ide\""));
}

#[test]
fn svg_export_contains_drawn_shape() {
    let scene = Scene { elements: vec![rect_element(10.0, 20.0, 50.0, 40.0, Some([240, 240, 250, 255]))], ..Default::default() };
    let svg = scene_svg(&scene, Some(WHITE));
    assert!(svg.contains("<polygon") && svg.contains("<polyline"));
    assert!(svg.contains("width=\"70\""));
}

#[test]
fn png_export_is_a_valid_image() {
    let png = render_svg_png(&scene_svg(&Scene::default(), Some(WHITE)), 2.0).unwrap();
    let image = image::load_from_memory(&png).unwrap();
    assert_eq!(image.width(), 360);
}
