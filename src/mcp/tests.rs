use super::*;
use crate::app::filetree::SortBy;

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
fn an_ide_from_mcp_is_a_file_tree_on_its_folder() {
    let mut scene = Scene::default();
    add_elements(&mut scene, &[json!({ "type": "ide", "id": "i", "x": 0, "y": 0, "root": "/tmp/loja", "name": "loja" })]).unwrap();
    let ide = by_id(&scene, "i");
    assert_eq!((ide.legacy_ide.is_none(), ide.filetree.as_ref().map(|t| (t.root.as_str(), t.editor))), (true, Some(("/tmp/loja", true))));
    assert_eq!(local_bounds(ide).size(), FILETREE_SIZE);
}

#[test]
fn file_tree_from_mcp_keeps_its_folder() {
    let mut scene = Scene::default();
    add_elements(&mut scene, &[json!({ "type": "filetree", "id": "f", "x": 0, "y": 0, "root": "/tmp/loja", "sort": "size", "hideHidden": true })]).unwrap();
    let tree = by_id(&scene, "f");
    assert_eq!(tree.filetree, Some(FileTreeSpec { root: "/tmp/loja".into(), sort: SortBy::Size, hide_hidden: true, ..Default::default() }));
    assert_eq!(local_bounds(tree).size(), FILETREE_SIZE);
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
        assert_eq!((&after.terminal, &after.portal, &after.filetree), (&e.terminal, &e.portal, &e.filetree));
    }
}

#[test]
fn update_elements_turns_resuming_off_and_keeps_the_remembered_conversation() {
    let mut scene = Scene::default();
    add_elements(&mut scene, &[json!({ "type": "terminal", "id": "t", "x": 0, "y": 0, "name": "Revisor", "command": "claude" })]).unwrap();
    assert!(by_id(&scene, "t").terminal.as_ref().unwrap().resume, "on by default");
    let t = scene.elements[0].terminal.as_mut().unwrap();
    (t.claude_session, t.claude_folder) = ("0b6c-41".into(), "-p".into());
    update_elements(&mut scene, &[json!({ "id": "t", "resume": false })]).unwrap();
    let t = by_id(&scene, "t").terminal.clone().unwrap();
    assert_eq!((t.resume, t.claude_session.as_str(), t.claude_folder.as_str()), (false, "0b6c-41", "-p"));
    update_elements(&mut scene, &[json!({ "id": "t", "resume": true })]).unwrap();
    assert!(by_id(&scene, "t").terminal.as_ref().unwrap().resume);
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
fn every_app_action_is_a_tool_once() {
    let list = handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).unwrap();
    let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    for action in [
        "ide_folder",
        "open_path",
        "open_default",
        "canvas_drop",
        "chat_send",
        "mini_chat",
        "chats_view",
        "chat_command",
        "voice_call",
        "portal_tab",
        "canvas_group",
        "history",
        "demands",
        "outside_message",
        "connection_history",
        "tasks",
        "flow",
        "binder",
        "minimap",
        "sidebar",
        "panel",
        "terminal_menu",
        "terminal_find",
        "terminal_look",
        "terminal_processes",
        "file_tree",
    ] {
        assert_eq!(names.iter().filter(|n| **n == action).count(), 1, "{action}");
        assert!(super::app_tools::is_action(action));
    }
    assert!(!super::app_tools::is_action("view"));
    let unique: std::collections::HashSet<&&str> = names.iter().collect();
    assert_eq!(unique.len(), names.len(), "a tool listed twice");
}

#[test]
fn vault_tools_never_tell_the_secret() {
    mock_keyring();
    let list = handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).unwrap();
    let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(["vault_list", "vault_accounts", "vault_fill"].iter().all(|n| names.contains(n)));
    let senha = VaultEntry { site: "github.com".into(), user: "ana@exemplo.com".into(), note: "pessoal".into(), ..VaultEntry::new() };
    let google = VaultEntry { site: "figma.com".into(), user: "ana@exemplo.com".into(), kind: VaultKind::Google, ..VaultEntry::new() };
    set_secret(&senha.id, "s3gr3do").unwrap();
    // what the agents list: no id, no secret
    let listed = describe_accounts(&[senha.clone(), google.clone()]);
    assert!(listed.contains("\"kind\":\"senha\"") && listed.contains("\"note\":\"pessoal\"") && listed.contains("\"kind\":\"google\""));
    assert!(!listed.contains("s3gr3do") && !listed.contains(&senha.id));
    assert!(describe_accounts(&[]).starts_with("O cofre está vazio"));
    // what vault_fill types goes to the app; what comes back to the model names no secret
    let (typed, reply) = vault_login(&senha).unwrap();
    assert_eq!((typed.as_str(), reply.as_str()), ("s3gr3do", "Preenchido em github.com para ana@exemplo.com."));
    let (typed, reply) = vault_login(&google).unwrap();
    assert_eq!(typed, "ana@exemplo.com");
    assert!(reply.starts_with("Login via Google: e-mail preenchido em figma.com"));
    remove_secret(&senha.id).unwrap();
    assert!(vault_login(&senha).unwrap_err().contains("não está no chaveiro"));
    // a site the vault has nothing for is an error, before any bridge call
    let missing = call("vault_fill", &json!({ "site": "nada.com" }));
    assert_eq!(missing["isError"], true);
    assert!(missing["content"][0]["text"].as_str().unwrap().starts_with("Nenhuma conta de nada.com"));
}

#[test]
fn only_a_maestro_builds_terminals_and_links() {
    let mut scene = Scene {
        elements: from_interop_elements(&json!([
            { "type": "terminal", "id": "pai", "x": 0, "y": 0, "name": "Luna", "maestro": true },
            { "type": "terminal", "id": "filho", "x": 400, "y": 0, "name": "Revisor", "maestro": false },
            { "type": "rectangle", "id": "r", "x": 0, "y": 300 },
        ])),
        ..Scene::default()
    };
    let terminal = json!([{ "type": "terminal", "id": "neto", "x": 800, "y": 0, "name": "Neto", "maestro": true }]);
    let shape = json!([{ "type": "rectangle", "id": "s", "x": 0, "y": 500 }]);
    let link = json!([{ "type": "arrow", "start": { "id": "filho" }, "end": { "id": "r" } }]);
    // a child may not add a terminal, nor link one (an arrow bound to a terminal is a connection)
    assert_eq!(may_build(&scene, Some("filho"), terminal.as_array().unwrap()).unwrap_err(), "Só um terminal da Luna pode criar terminais");
    assert_eq!(may_build(&scene, Some("filho"), link.as_array().unwrap()).unwrap_err(), "Só um terminal da Luna pode conectar");
    // nor through the file format's bindings, in a new arrow or a patch
    let bound = json!([{ "type": "arrow", "x": 0, "y": 300, "endBinding": { "elementId": "pai", "fixedPoint": [0.5, 0.5], "mode": "orbit" } }]);
    assert_eq!(may_build(&scene, Some("filho"), bound.as_array().unwrap()).unwrap_err(), "Só um terminal da Luna pode conectar");
    let patch = json!([{ "id": "r", "startBinding": { "elementId": "filho" } }]);
    assert_eq!(may_build(&scene, Some("filho"), patch.as_array().unwrap()).unwrap_err(), "Só um terminal da Luna pode conectar");
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

#[test]
fn the_note_tool_creates_and_changes_a_note_and_sets_its_buttons() {
    let mut scene = Scene::default();
    let made = note_call(
        &mut scene,
        &json!({ "text": "- um", "title": "Plano", "color": "Azul", "buttons": [{ "label": "Abrir", "action": "open_path", "args": { "path": "/tmp" } }] }),
    )
    .unwrap();
    let id = made["id"].as_str().unwrap().to_string();
    let index = scene.elements.iter().position(|e| e.id == id).unwrap();
    assert_eq!(local_bounds(&scene.elements[index]).size(), NOTE_SIZE);
    assert_eq!(scene.elements[index].style.fill, Some(NOTE_PAPERS[2].0));
    assert_eq!(scene.elements[index].note.buttons.len(), 1);
    let text_of = |scene: &Scene| {
        scene
            .elements
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Text { value, .. } if e.id == format!("{id}-label") => Some(value.clone()),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(text_of(&scene), "# Plano\n- um");
    // the text, the title and the color change; a hex turns into the nearest paper; the size keeps its floor
    note_call(&mut scene, &json!({ "id": id, "title": "Novo", "color": "#b0f0b0", "width": 10 })).unwrap();
    assert_eq!(
        text_of(&scene),
        "# Novo
- um"
    );
    assert_eq!(scene.elements[index].style.fill, Some(NOTE_PAPERS[3].0));
    assert_eq!(local_bounds(&scene.elements[index]).width(), NOTE_MIN.x);
    assert_eq!(scene.elements[index].note.buttons.len(), 1, "buttons stay when not mentioned");
    // an action the app doesn't have is refused at once, and nothing is changed
    let refused = note_call(&mut scene, &json!({ "id": id, "buttons": [{ "label": "X", "action": "apagar_tudo" }] })).unwrap_err();
    assert!(refused.contains("apagar_tudo"), "{refused}");
    assert_eq!(scene.elements[index].note.buttons.len(), 1);
    note_call(&mut scene, &json!({ "id": id, "buttons": [] })).unwrap();
    assert!(scene.elements[index].note.buttons.is_empty());
    assert!(note_call(&mut scene, &json!({ "id": "nada", "text": "x" })).is_err());
    // the file format's own sticky note still comes in, with the default size
    add_elements(&mut scene, &[json!({ "type": "stickynote", "id": "velha", "x": 0, "y": 0, "backgroundColor": "#ffdf6b" })]).unwrap();
    assert_eq!(local_bounds(by_id(&scene, "velha")).size(), NOTE_SIZE);
}

#[test]
fn every_tool_is_in_the_mcp_docs() {
    let docs = include_str!("../../docs/mcp.md");
    let missing: Vec<String> = tools()
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .filter(|name| !docs.contains(&format!("### `{name}`")))
        .map(String::from)
        .collect();
    assert!(missing.is_empty(), "docs/mcp.md não tem {missing:?}: rode `python docs/mcp.py` depois de um build release");
}

#[test]
fn the_panel_tool_opens_and_closes_the_right_panels() {
    use crate::app::panes::Pane;
    let mut app = DrawingApp { chat_open: true, ..Default::default() };
    assert_eq!(app.act("panel", &json!({ "name": "changes" })).unwrap(), "Painel Alterações aberto.");
    assert_eq!((app.panes.open, app.chat_open), (Some(Pane::Changes), false), "it shows beside the canvas");
    assert_eq!(app.act("panel", &json!({ "name": "Alterações" })).unwrap(), "Painel Alterações fechado.", "no `open` toggles");
    assert_eq!(app.panes.open, None);
    app.act("panel", &json!({ "name": "changes", "open": true })).unwrap();
    assert_eq!(app.act("panel", &json!({ "open": false })).unwrap(), "Painel fechado.", "no name closes it");
    assert_eq!(app.panes.open, None);
    // the Tarefas panel went ("a aba de tarefas, ali no canvas remove por favor"); the tasks stay in `tasks`
    assert!(app.act("panel", &json!({ "name": "tasks" })).unwrap_err().contains("Use changes"));
    assert!(app.act("panel", &json!({ "name": "nada" })).unwrap_err().contains("Use changes"));
    assert!(app.act("panel", &json!({ "open": true })).is_err());
}

#[test]
fn a_terminal_an_agent_adds_comes_roped_to_it() {
    let mut scene = Scene {
        elements: from_interop_elements(&json!([
            { "type": "terminal", "id": "luna", "x": 0, "y": 0, "name": "Luna", "maestro": true },
            { "type": "chat", "id": "chat", "x": 0, "y": 400, "name": "Luna", "agent": "claude" },
        ])),
        ..Scene::default()
    };
    let roped = |scene: &Scene, a: &str, b: &str| {
        let ends = |e: &Element| [&e.ends.start, &e.ends.end].map(|end| end.as_ref().map(|b| b.element_id.clone()));
        scene.elements.iter().filter(|e| e.rope && matches!(ends(e), [Some(x), Some(y)] if (x == a && y == b) || (x == b && y == a))).count()
    };
    // the Luna's terminal opens one to run the tests and draws a box: the terminal hangs from it, the box does not
    let items = json!([
        { "type": "terminal", "id": "testes", "x": 600, "y": 0, "name": "Testes", "command": "cargo test" },
        { "type": "rectangle", "id": "caixa", "x": 0, "y": 900 },
    ]);
    let added = add_elements(&mut scene, items.as_array().unwrap()).unwrap();
    assert_eq!(tie_to_caller(&mut scene, Some("luna"), &added).unwrap(), ["testes"]);
    assert_eq!(roped(&scene, "luna", "testes"), 1);
    assert_eq!(roped(&scene, "luna", "caixa"), 0);
    let rope = scene.elements.iter().find(|e| e.rope).unwrap();
    assert!(matches!(rope.kind, ElementKind::Line { .. }) && rope.ends.start_head.is_none() && rope.ends.end_head.is_none());
    // asked twice: still one rope
    tie_to_caller(&mut scene, Some("luna"), &added).unwrap();
    assert_eq!(roped(&scene, "luna", "testes"), 1);
    // a chat's agent (its card on this canvas) opens the API's terminal: roped to its card
    let api = add_elements(&mut scene, &[json!({ "type": "terminal", "id": "api", "x": 600, "y": 400, "name": "API", "command": "npm run dev" })]).unwrap();
    assert_eq!(tie_to_caller(&mut scene, Some("chat"), &api).unwrap(), ["api"]);
    assert_eq!(roped(&scene, "chat", "api"), 1);
    // no caller (an agent outside the app), or one with no card here (the app ties it when the drawing comes in): no rope
    let loose = add_elements(&mut scene, &[json!({ "type": "terminal", "id": "solto", "x": 600, "y": 800, "name": "Solto" })]).unwrap();
    assert!(tie_to_caller(&mut scene, None, &loose).unwrap().is_empty());
    assert!(tie_to_caller(&mut scene, Some("outro-chat"), &loose).unwrap().is_empty());
    assert_eq!(roped(&scene, "luna", "solto") + roped(&scene, "chat", "solto"), 0);
}

#[test]
fn the_caller_is_the_terminal_else_the_chat() {
    let env = |pairs: &'static [(&'static str, &'static str)]| move |key: &str| pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| v.to_string());
    assert_eq!(caller_from(env(&[("MAGOAGENT_TERMINAL_ID", "t1"), ("MAGOAGENT_CHAT_ID", "c1")])).as_deref(), Some("t1"));
    // a chat's agent has an empty terminal id, and its chat's
    assert_eq!(caller_from(env(&[("MAGOAGENT_TERMINAL_ID", ""), ("MAGOAGENT_CHAT_ID", "c1")])).as_deref(), Some("c1"));
    assert_eq!(caller_from(env(&[("MAGOAGENT_CHAT_ID", " ")])), None);
    assert_eq!(caller_from(env(&[])), None);
}

/// Asked: "a posição dos terminais, às vezes ele junta tudo na direita". Windows an agent stacks on the same spot (it
/// reads the canvas from a list) go to the free places around it, each with its label; a shape stays where it was put.
#[test]
fn windows_an_agent_stacks_go_around_it() {
    let mut scene = Scene::default();
    let luna = json!({ "type": "terminal", "id": "luna", "x": 0, "y": 0, "width": 600, "height": 400, "name": "Luna" });
    add_elements(&mut scene, &[luna]).unwrap();
    let items = json!([
        { "type": "terminal", "id": "um", "x": 2000, "y": 0, "width": 600, "height": 400, "name": "Um" },
        { "type": "terminal", "id": "dois", "x": 2000, "y": 0, "width": 600, "height": 400, "name": "Dois" },
        { "type": "terminal", "id": "tres", "x": 100, "y": 100, "width": 600, "height": 400, "name": "Três", "label": { "text": "API" } },
        { "type": "rectangle", "id": "caixa", "x": 10, "y": 10 },
    ]);
    let added = add_elements(&mut scene, items.as_array().unwrap()).unwrap();
    make_room(&mut scene, Some("luna"), &added);
    let at = |id: &str| element_bounds(scene.elements.iter().find(|e| e.id == id).unwrap()).min;
    // the first is free where it was put; the second, over it, goes around the Luna (below it first); the third, over
    // the Luna, the next place around it, its label along
    assert_eq!((at("um"), at("dois"), at("tres")), (Pos2::new(2000.0, 0.0), Pos2::new(0.0, 460.0), Pos2::new(660.0, 0.0)));
    let label = element_bounds(scene.elements.iter().find(|e| e.id == "tres-label").unwrap()).center();
    assert!(element_bounds(scene.elements.iter().find(|e| e.id == "tres").unwrap()).contains(label), "the label went along");
    assert_eq!(at("caixa"), Pos2::new(10.0, 10.0), "a shape is no window");
}
