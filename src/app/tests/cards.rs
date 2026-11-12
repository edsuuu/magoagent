use super::*;

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
    assert_eq!(super::canvas::pointer::dropped_paths("", &["/a.png".into()]), "/a.png", "no gap in an empty box");
    assert_eq!(
        super::canvas::pointer::dropped_paths(
            "oi
",
            &["/a.png".into()]
        ),
        "/a.png",
        "nor after a newline"
    );
    assert_eq!(super::canvas::pointer::dropped_paths("", &["/tmp/a\"b c.png".into()]), "'/tmp/a\"b c.png'", "a quote in the name");
    assert_eq!(super::canvas::pointer::dropped_paths("", &["/tmp/it's \"x\".png".into()]), "'/tmp/it\\'s \"x\".png'", "both quotes");
    assert_eq!(super::canvas::pointer::dropped_paths("", &["/tmp/a\\'\".png".into()]), "'/tmp/a\\\\\\'\".png'", "a backslash before the quote");
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
