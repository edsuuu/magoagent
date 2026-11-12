use super::*;

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
    // the corner buttons (zoom, minimap) step aside for it
    let shown = |name: &str| ctx.memory(|m| m.areas().visible_last_frame(&egui::LayerId::new(egui::Order::Middle, egui::Id::unique(name))));
    assert!(!shown("footer") && !shown("minimap_toggle"));
    let esc = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame_at(&mut app, &ctx, Some(11.5), vec![esc]);
    assert_eq!((app.terminal_elevated.as_ref(), app.ide_focus.as_ref()), (None, None));
    // back on the canvas, they come back
    for t in [12.0, 12.5] {
        frame_at(&mut app, &ctx, Some(t), vec![]);
    }
    assert!(shown("footer") && shown("minimap_toggle"));
    // bottom right, in a row: the minimap toggle, the zoom (one line, not a column)
    let rect = |name: &str| ctx.memory(|m| m.area_rect(egui::Id::unique(name))).unwrap();
    let (map, zoom) = (rect("minimap_toggle"), rect("footer"));
    assert!(map.right() < zoom.left() && zoom.left() > app.canvas_rect.center().x, "{map:?} {zoom:?}");
    assert!(zoom.height() <= GLASS_HEIGHT && (zoom.bottom() - map.bottom()).abs() < 1.0, "{zoom:?} {map:?}");
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
