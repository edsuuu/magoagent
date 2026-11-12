use super::*;
use std::path::Path;

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
fn window_tools_show_no_style_panel() {
    let mut app = DrawingApp::default();
    for tool in [Tool::Terminal, Tool::Portal, Tool::FileTree, Tool::Sticky, Tool::Text] {
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

/// A file tree on `root`, at `x`, pushed straight into the scene.
fn tree_at(app: &mut DrawingApp, x: f32, root: &Path) -> String {
    let mut tree = app.new_element(ElementKind::Rectangle { x, y: 1000.0, width: FILETREE_SIZE.x, height: FILETREE_SIZE.y });
    tree.filetree = Some(FileTreeSpec { root: root.to_string_lossy().into(), ..Default::default() });
    let id = tree.id.clone();
    app.scene.elements.push(tree);
    id
}

#[test]
fn the_trees_claude_button_ropes_one_claude_terminal_in_its_folder() {
    let ctx = Context::default();
    let dir = std::env::temp_dir().join(format!("magoagent-tree-claude-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = tree_at(&mut app, 100.0, &dir);
    app.claude_for_tree(&id);
    app.claude_for_tree(&id);
    let terminals: Vec<&Element> = app.scene.elements.iter().filter(|e| e.terminal.is_some()).collect();
    assert_eq!(terminals.len(), 1, "the second press reuses the roped terminal");
    let spec = terminals[0].terminal.as_ref().unwrap();
    assert_eq!((spec.command.as_str(), spec.cwd.as_str()), ("claude", dir.to_string_lossy().as_ref()));
    assert!(app.links()[&id].contains(&terminals[0].id));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn screen_keys_open_the_enlarged_file_tree_or_the_first() {
    let ctx = Context::default();
    let dir = std::env::temp_dir().join(format!("magoagent-tree-keys-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut app = DrawingApp::default();
    let cmd = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    frame(&mut app, &ctx, vec![cmd(Key::Num3)]);
    assert!(app.status.starts_with("Nenhuma Árvore"));
    // two trees off screen: ⌘3 opens the first, selected like a double-click
    let a = tree_at(&mut app, 0.0, &dir);
    let b = tree_at(&mut app, 700.0, &dir);
    frame(&mut app, &ctx, vec![cmd(Key::Num3)]);
    assert_eq!((app.terminal_elevated.as_ref(), &app.selected[..]), (Some(&a), &[0][..]));
    // one already open stays: ⌘3 does not jump to the first
    app.set_elevated(Some(b.clone()));
    frame(&mut app, &ctx, vec![cmd(Key::Num3)]);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&b));
    // ⌘2: the canvas, with the tree back as a card
    frame(&mut app, &ctx, vec![cmd(Key::Num2)]);
    assert_eq!(app.terminal_elevated, None);
    let _ = fs::remove_dir_all(&dir);
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
