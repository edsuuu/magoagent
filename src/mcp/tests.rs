use super::*;

fn by_id<'a>(scene: &'a Scene, id: &str) -> &'a Element {
    scene.elements.iter().find(|e| e.id == id).unwrap()
}

#[test]
fn late_frame_goes_behind_its_contents() {
    let mut scene = Scene::default();
    let box_ = json!({ "type": "rectangle", "id": "b", "x": 20, "y": 20, "width": 50, "height": 30 });
    let frame = json!({ "type": "frame", "id": "f", "x": 0, "y": 0, "width": 200, "height": 100, "backgroundColor": "#000000" });
    add_elements(&mut scene, &[box_, frame]).unwrap();
    assert_eq!(scene.elements.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["f", "b"]);
}

#[test]
fn ide_from_mcp_keeps_its_folder() {
    let mut scene = Scene::default();
    add_elements(&mut scene, &[json!({ "type": "ide", "id": "i", "x": 0, "y": 0, "root": "/tmp/loja", "name": "loja" })]).unwrap();
    let ide = by_id(&scene, "i");
    assert_eq!(ide.ide, Some(IdeSpec { name: "loja".into(), root: "/tmp/loja".into(), ..Default::default() }));
    assert_eq!(local_bounds(ide).size(), IDE_SIZE);
}

#[test]
fn agents_resize_windows_with_update_elements() {
    let mut scene = Scene::default();
    let nodes = [
        json!({ "type": "terminal", "id": "t", "x": 0, "y": 0, "name": "Revisor", "command": "claude", "cwd": "/tmp", "icon": "search", "color": [224, 89, 12], "quiet": true }),
        json!({ "type": "portal", "id": "p", "x": 900, "y": 0, "url": "https://exemplo.com", "name": "Docs" }),
        json!({ "type": "ide", "id": "i", "x": 0, "y": 700, "root": "/tmp/loja", "name": "loja" }),
    ];
    add_elements(&mut scene, &nodes).unwrap();
    let before: Vec<Element> = scene.elements.clone();
    let patches = json!([
        { "id": "t", "width": 900, "height": 500 },
        { "id": "p", "x": 1000, "width": 640, "height": 400 },
        { "id": "i", "width": 1200, "height": 800 },
    ]);
    update_elements(&mut scene, patches.as_array().unwrap()).unwrap();
    assert_eq!(local_bounds(by_id(&scene, "t")).size(), Vec2::new(900.0, 500.0));
    assert_eq!(local_bounds(by_id(&scene, "p")), Rect::from_min_size(Pos2::new(1000.0, 0.0), Vec2::new(640.0, 400.0)));
    assert_eq!(local_bounds(by_id(&scene, "i")).size(), Vec2::new(1200.0, 800.0));
    // what the window is stays the same
    for e in &before {
        let after = by_id(&scene, &e.id);
        assert_eq!((&after.terminal, &after.portal, &after.ide), (&e.terminal, &e.portal, &e.ide));
    }
}

#[test]
fn agent_names_are_readable() {
    assert_eq!(agent_label("claude-code"), "Claude Code");
    assert_eq!(agent_label("codex-mcp-client"), "Codex");
    assert_eq!(agent_label("gemini_cli"), "Gemini Cli");
    assert_eq!(agent_label("ágil"), "Ágil");
    assert_eq!(agent_label(""), "IA");
}

#[test]
fn base64_matches_rfc4648() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn diagram_with_labels_and_bound_arrow() {
    let mut scene = Scene::default();
    let created = add_elements(
        &mut scene,
        json!([
            { "id": "a", "type": "rectangle", "x": 0, "y": 0, "width": 200, "height": 100, "label": { "text": "Início" } },
            { "id": "b", "type": "ellipse", "x": 400, "y": 0, "width": 200, "height": 100, "backgroundColor": "#a5d8ff" },
            { "id": "seta", "type": "arrow", "start": { "id": "a" }, "end": { "id": "b" } },
        ])
        .as_array()
        .unwrap(),
    )
    .unwrap();
    assert_eq!(created.len(), 4, "label adds a text element");
    let label = scene.elements.iter().find(|e| matches!(e.kind, ElementKind::Text { .. })).unwrap();
    assert_eq!(label.id, "a-label");
    assert_eq!(label.group_id, by_id(&scene, "a").group_id);
    assert!((element_bounds(label).center() - Pos2::new(100.0, 50.0)).length() < 1.0);
    assert!(by_id(&scene, "a").style.round, "rectangles default to rounded corners");
    let ElementKind::Line { points, .. } = &by_id(&scene, "seta").kind else { panic!() };
    // orbit endpoints: just outside each outline, on the center line
    assert!((points[0][0] - 206.0).abs() < 0.5 && (points[1][0] - 394.0).abs() < 0.5, "{points:?}");
    // moving the ellipse drags the arrow along
    update_elements(&mut scene, json!([{ "id": "b", "y": 300 }]).as_array().unwrap()).unwrap();
    let ElementKind::Line { points, .. } = &by_id(&scene, "seta").kind else { panic!() };
    assert!(points[1][1] > 250.0, "{points:?}");
    // a moved shape takes its label along
    update_elements(&mut scene, json!([{ "id": "a", "x": 50 }]).as_array().unwrap()).unwrap();
    recenter_labels(&mut scene.elements);
    let label = by_id(&scene, "a-label");
    assert!((element_bounds(label).center().x - 150.0).abs() < 1.0);
    assert!(add_elements(&mut scene, json!([{ "id": "a", "type": "rectangle" }]).as_array().unwrap()).is_err());
}

#[test]
fn update_changes_text_and_style_and_rebinds() {
    let mut scene = Scene::default();
    add_elements(
        &mut scene,
        json!([
            { "id": "t", "type": "text", "x": 10, "y": 10, "text": "olá" },
            { "id": "c", "type": "rectangle", "x": 300, "y": 0 },
            { "id": "s", "type": "arrow", "x": 0, "y": 200, "width": 100, "height": 0 },
        ])
        .as_array()
        .unwrap(),
    )
    .unwrap();
    update_elements(
        &mut scene,
        json!([
            { "id": "t", "text": "tchau", "strokeColor": "#e03131" },
            { "id": "s", "end": { "id": "c" } },
        ])
        .as_array()
        .unwrap(),
    )
    .unwrap();
    let t = by_id(&scene, "t");
    assert!(matches!(&t.kind, ElementKind::Text { value, .. } if value == "tchau"));
    assert_eq!(t.style.stroke, [0xe0, 0x31, 0x31, 255]);
    let s = by_id(&scene, "s");
    assert_eq!(s.ends.end.as_ref().map(|b| b.element_id.as_str()), Some("c"));
    let ElementKind::Line { points, .. } = &s.kind else { panic!() };
    assert_eq!(points[0], [0.0, 200.0], "the free end stays put");
}

#[test]
fn view_returns_a_png() {
    let mut scene = Scene::default();
    add_elements(&mut scene, json!([{ "type": "diamond", "x": 0, "y": 0 }]).as_array().unwrap()).unwrap();
    let content = view(&scene, &json!({})).unwrap();
    assert_eq!(content[0]["mimeType"], "image/png");
    assert!(content[0]["data"].as_str().unwrap().starts_with("iVBORw0KGgo"), "PNG signature");
}

#[test]
fn json_rpc_round_trip() {
    let init = handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } })).unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
    assert!(handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
    let list = handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
    assert!(list["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "view"));
    let unknown = handle(&json!({ "jsonrpc": "2.0", "id": 3, "method": "nope" })).unwrap();
    assert_eq!(unknown["error"]["code"], -32601);
}

#[test]
fn only_a_maestro_builds_terminals_and_links() {
    let mut scene = Scene::default();
    scene.elements = from_interop_elements(&json!([
        { "type": "terminal", "id": "pai", "x": 0, "y": 0, "name": "Maestro", "maestro": true },
        { "type": "terminal", "id": "filho", "x": 400, "y": 0, "name": "Revisor", "maestro": false },
        { "type": "rectangle", "id": "r", "x": 0, "y": 300 },
    ]));
    let terminal = json!([{ "type": "terminal", "id": "neto", "x": 800, "y": 0, "name": "Neto", "maestro": true }]);
    let shape = json!([{ "type": "rectangle", "id": "s", "x": 0, "y": 500 }]);
    let link = json!([{ "type": "arrow", "start": { "id": "filho" }, "end": { "id": "r" } }]);
    // a child may not add a terminal, nor link one (an arrow bound to a terminal is a connection)
    assert_eq!(may_build(&scene, Some("filho"), terminal.as_array().unwrap()).unwrap_err(), "Só um terminal Maestro pode criar terminais");
    assert_eq!(may_build(&scene, Some("filho"), link.as_array().unwrap()).unwrap_err(), "Só um terminal Maestro pode conectar");
    // nor through the file format's bindings, in a new arrow or a patch
    let bound = json!([{ "type": "arrow", "x": 0, "y": 300, "endBinding": { "elementId": "pai", "fixedPoint": [0.5, 0.5], "mode": "orbit" } }]);
    assert_eq!(may_build(&scene, Some("filho"), bound.as_array().unwrap()).unwrap_err(), "Só um terminal Maestro pode conectar");
    let patch = json!([{ "id": "r", "startBinding": { "elementId": "filho" } }]);
    assert_eq!(may_build(&scene, Some("filho"), patch.as_array().unwrap()).unwrap_err(), "Só um terminal Maestro pode conectar");
    // it can still draw; the Maestro and anything outside a canvas terminal may build
    assert!(may_build(&scene, Some("filho"), shape.as_array().unwrap()).is_ok());
    assert!(may_build(&scene, Some("pai"), terminal.as_array().unwrap()).is_ok());
    assert!(may_build(&scene, None, terminal.as_array().unwrap()).is_ok());
    // a terminal an agent adds starts with Maestro off, whatever the JSON says
    add_elements(&mut scene, terminal.as_array().unwrap()).unwrap();
    assert!(!by_id(&scene, "neto").terminal.as_ref().unwrap().maestro);
    // and a patch cannot switch a child's Maestro on
    update_elements(&mut scene, &[json!({ "id": "filho", "maestro": true, "x": 420 })]).unwrap();
    assert!(!by_id(&scene, "filho").terminal.as_ref().unwrap().maestro);
}
