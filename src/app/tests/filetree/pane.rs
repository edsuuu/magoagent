use super::*;

#[test]
fn a_file_opens_in_the_trees_own_editor_pane() {
    use crate::app::filetree::editor::Body;
    let ctx = Context::default();
    let dir = temp("open");
    fs::write(dir.join("main.rs"), "fn main() {}\r\n").unwrap();
    fs::write(dir.join("notes.txt"), "one\n").unwrap();
    fs::write(dir.join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
    fs::write(dir.join("big.txt"), vec![b'x'; (2 << 20) + 1]).unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let tree = tree_on(&mut app, &dir);
    frame(&mut app, &ctx, vec![]);
    let width = |app: &DrawingApp| match app.scene.elements.iter().find(|e| e.id == tree).unwrap().kind {
        ElementKind::Rectangle { width, .. } => width,
        _ => 0.0,
    };
    let before = width(&app);
    app.ft_activate(&tree, dir.join("main.rs"));
    let spec = app.ft_spec(&tree).unwrap().clone();
    assert!(spec.editor && spec.tree_width == Some(before), "the pane is shown and the tree keeps its width");
    assert_eq!(width(&app), before + 601.0, "the node grows to the right by the pane and its seam");
    let ed = &app.filetrees.editors[&tree];
    assert_eq!((ed.tabs.len(), ed.active.as_deref()), (1, Some(dir.join("main.rs").as_path())));
    assert!(matches!(&ed.tabs[0].body, Body::Text(t) if t.text == "fn main() {}\n"), "read with LF ends");
    // typed in, saved with the file's own line ends
    if let Body::Text(t) = &mut app.filetrees.editors.get_mut(&tree).unwrap().tabs[0].body {
        t.text.push_str("// x\n");
        t.dirty = true;
    }
    assert!(app.filetrees.editors[&tree].dirty());
    app.filetrees.editors.get_mut(&tree).unwrap().save().unwrap();
    assert_eq!(fs::read_to_string(dir.join("main.rs")).unwrap(), "fn main() {}\r\n// x\r\n");
    // a click only previews, the next click takes its place; a double-click keeps the tab
    app.ft_apply(&ctx, &tree, crate::app::filetree::ui::Act::Select(dir.join("notes.txt")));
    app.ft_apply(&ctx, &tree, crate::app::filetree::ui::Act::Select(dir.join("blob.bin")));
    let ed = &app.filetrees.editors[&tree];
    let names: Vec<_> = ed.tabs.iter().map(|t| t.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["main.rs", "blob.bin"], "the notes preview gave way");
    assert!(matches!(&ed.tabs[1].body, Body::Refused(why) if why.starts_with("Este não é um arquivo de texto")));
    app.ft_activate(&tree, dir.join("big.txt"));
    let ed = &app.filetrees.editors[&tree];
    assert!(matches!(&ed.tabs[2].body, Body::Refused(why) if why.contains("além do que o editor embutido comporta")), "over 2 MiB");
    // what the agents see, and close
    assert!(app.describe_tree(&tree).contains("Editor: aberto, abas main.rs, ~blob.bin, >big.txt"), "{}", app.describe_tree(&tree));
    app.act_file_tree(&json!({ "action": "close_file", "path": "big.txt" })).unwrap();
    app.act_file_tree(&json!({ "action": "editor", "show": false })).unwrap();
    assert!(!app.ft_spec(&tree).unwrap().editor);
    assert_eq!(width(&app), before, "the node shrinks back to the tree");
    app.act_file_tree(&json!({ "action": "open", "path": "notes.txt" })).unwrap();
    assert!(app.ft_spec(&tree).unwrap().editor);
    // a folder opens or closes instead
    fs::create_dir_all(dir.join("sub")).unwrap();
    app.ft_activate(&tree, dir.join("sub"));
    assert!(app.filetrees.views[&tree].expanded.contains(&dir.join("sub")));
    app.ft_activate(&tree, dir.join("sub"));
    assert!(app.filetrees.views[&tree].expanded.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_picture_and_a_pdf_open_in_the_pane_with_their_size() {
    use crate::app::filetree::editor::Body;
    let ctx = Context::default();
    let dir = temp("media");
    let png = dir.join("p.png");
    image::RgbaImage::from_pixel(7, 5, image::Rgba([1, 2, 3, 255])).save(&png).unwrap();
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    frame(&mut app, &ctx, vec![]);
    app.ft_activate(&tree, png);
    let ed = &app.filetrees.editors[&tree];
    assert!(matches!(ed.tabs[0].body, Body::Media(_)));
    assert_eq!(crate::app::filetree::editor::bytes(500), "500 B");
    assert_eq!(crate::app::filetree::editor::bytes(2048), "2,0 KB");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_click_on_a_row_selects_it_and_the_arrows_walk_the_tree() {
    let ctx = Context::default();
    let dir = temp("keys");
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::write(dir.join("a/inside.txt"), "x").unwrap();
    fs::write(dir.join("b.txt"), "x").unwrap();
    let mut app = DrawingApp::default();
    tree_frame(&mut app, &ctx, 0.0, vec![]);
    let id = tree_on(&mut app, &dir);
    // the node sits where the view's middle is; its frame on screen sets where the rows are
    for t in [1.0, 1.5, 2.0] {
        tree_frame(&mut app, &ctx, t, vec![]);
    }
    let frame = app.terminal_frame(&app.scene.elements[0]).0;
    let shown = |name: &str| ctx.memory(|m| m.area_rect(egui::Id::unique((name, id.as_str())))).is_some();
    assert!(shown("ft_nav") && shown("ft_tools") && shown("ft_body"), "the buttons and the rows are over the card");
    // the title strip is the canvas's: no area over it between the buttons
    let strip = Pos2::new(frame.center().x, frame.top() + 10.0);
    let body = ctx.memory(|m| m.area_rect(egui::Id::unique(("ft_body", id.as_str())))).unwrap();
    assert!(!body.contains(strip), "{body:?} {strip:?}");
    let row0 = Pos2::new(frame.left() + 80.0, frame.top() + FT_BAR + 1.0 + FT_ROW * 0.5);
    tree_frame(&mut app, &ctx, 3.0, vec![egui::Event::PointerMoved(row0)]);
    tree_frame(&mut app, &ctx, 3.1, vec![button(row0, true)]);
    tree_frame(&mut app, &ctx, 3.2, vec![button(row0, false)]);
    assert_eq!(app.filetrees.views[&id].selected, Some(dir.join("a")));
    assert_eq!(app.card_focus.as_ref(), Some(&id), "the tree has the keys");
    assert_eq!(app.selected, vec![0]);
    // down to the next row; right opens the folder; Esc lets go
    let key = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    tree_frame(&mut app, &ctx, 4.0, vec![key(Key::ArrowDown)]);
    assert_eq!(app.filetrees.views[&id].selected, Some(dir.join("b.txt")));
    tree_frame(&mut app, &ctx, 4.1, vec![key(Key::ArrowUp), key(Key::ArrowRight)]);
    assert!(app.filetrees.views[&id].expanded.contains(&dir.join("a")));
    tree_frame(&mut app, &ctx, 4.2, vec![key(Key::ArrowDown)]);
    assert_eq!(app.filetrees.views[&id].selected, Some(dir.join("a/inside.txt")));
    tree_frame(&mut app, &ctx, 4.3, vec![key(Key::ArrowLeft)]);
    assert_eq!(app.filetrees.views[&id].selected, Some(dir.join("a")), "left goes up to the folder");
    tree_frame(&mut app, &ctx, 4.4, vec![key(Key::Escape)]);
    assert_eq!((app.card_focus.as_ref(), app.selected.len()), (None, 0));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_filename_search_finds_files_below_the_folder() {
    let ctx = Context::default();
    let dir = temp("search");
    fs::create_dir_all(dir.join("deep/er")).unwrap();
    fs::write(dir.join("deep/er/target.rs"), "x").unwrap();
    fs::write(dir.join("other.txt"), "x").unwrap();
    let spec = FileTreeSpec { root: dir.to_string_lossy().into(), ..Default::default() };
    let mut view = TreeView::new();
    view.filter = "TARG".into();
    for _ in 0..200 {
        view.poll(&spec, &ctx, 0.0);
        if view.dirty {
            view.rebuild(&spec);
        }
        if !view.rows.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert_eq!(view.rows.iter().map(|r| (r.name.as_str(), r.hint.replace('\\', "/"))).collect::<Vec<_>>(), [("target.rs", "deep/er".to_string())]);
    view.filter = "zzz".into();
    view.rebuild(&spec);
    assert_eq!(view.empty_note(&spec).as_deref(), Some("Nenhuma correspondência"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn agents_make_and_close_trees_by_name() {
    let dir = temp("agent");
    let mut app = DrawingApp::default();
    assert!(app.act_file_tree(&json!({ "action": "list" })).unwrap_err().contains("Não há árvore"));
    assert!(app.act_file_tree(&json!({ "action": "create", "path": dir.join("nada").to_string_lossy() })).is_err());
    let made = app.act_file_tree(&json!({ "action": "create", "path": dir.to_string_lossy() })).unwrap();
    assert!(made.starts_with("Árvore de arquivos") && made.contains("Esta pasta está vazia."), "{made}");
    // by the folder's name or by id
    let name = dir.file_name().unwrap().to_string_lossy().to_string();
    assert!(app.act_file_tree(&json!({ "action": "list", "tree": name.to_uppercase() })).is_ok());
    assert!(app.act_file_tree(&json!({ "action": "list", "tree": "nenhuma" })).is_err());
    app.act_file_tree(&json!({ "action": "close" })).unwrap();
    assert!(app.scene.elements.is_empty());
    app.undo();
    assert_eq!(app.scene.elements.len(), 1, "closing can be undone");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_search_counts_hits_and_the_tree_rows_apart() {
    let dir = temp("hits");
    fs::create_dir_all(dir.join("deep/er")).unwrap();
    fs::write(dir.join("deep/er/target.rs"), "x").unwrap();
    fs::write(dir.join("target.txt"), "x").unwrap();
    fs::write(dir.join("other.txt"), "x").unwrap();
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    // the agents' search: the hits with their folders (slashes as the reference writes them) and the tree's own count
    let found = app.act_file_tree(&json!({ "action": "search", "query": "TARGET" })).unwrap();
    assert!(found.contains("Busca \"TARGET\": 2 ocorrências; a árvore tem 3 linhas"), "{found}");
    assert!(found.contains("target.rs (deep/er)") && found.contains("target.txt"), "{found}");
    let view = &app.filetrees.views[&id];
    assert_eq!((view.rows.len(), view.tree_rows, view.hits_cut), (2, 3, false));
    // an empty query is the tree again
    let back = app.act_file_tree(&json!({ "action": "search", "query": "" })).unwrap();
    assert!(!back.contains("Busca") && back.contains("other.txt"), "{back}");
    assert_eq!(crate::app::filetree::ui::hits_note(1), "1 ocorrência");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_name_is_asked_in_a_popover_that_keeps_the_extension() {
    let ctx = Context::default();
    let dir = temp("popover");
    fs::write(dir.join("nota.txt"), "x").unwrap();
    let mut app = DrawingApp::default();
    tree_frame(&mut app, &ctx, 0.0, vec![]);
    let id = tree_on(&mut app, &dir);
    for t in [1.0, 1.5, 2.0] {
        tree_frame(&mut app, &ctx, t, vec![]);
    }
    // F2 asks with the base name; the extension is not the field's
    app.ft_apply(&ctx, &id, crate::app::filetree::ui::Act::Select(dir.join("nota.txt")));
    let row = app.filetrees.views[&id].rows.iter().find(|r| r.name == "nota.txt").cloned();
    app.ft_rename_ask(&ctx, &id, &row);
    match &app.filetrees.ask {
        Some(TreeAsk::Name { text, kind: NameKind::Rename, .. }) => assert_eq!(text, "nota"),
        other => panic!("{other:?}"),
    }
    assert!(app.filetree_dialog_open());
    tree_frame(&mut app, &ctx, 3.0, vec![]);
    assert!(ctx.memory(|m| m.area_rect(egui::Id::unique("ft_name_popover_area"))).is_some(), "the popover is under the row");
    // the field typed, Enter renames to the new base with the old extension
    if let Some(TreeAsk::Name { text, .. }) = app.filetrees.ask.as_mut() {
        *text = "memo".into();
    }
    let enter = egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    tree_frame(&mut app, &ctx, 4.0, vec![enter]);
    assert!(dir.join("memo.txt").is_file() && !dir.join("nota.txt").exists(), "{:?}", app.status);
    assert!(app.filetrees.ask.is_none());
    // Escape leaves it as it was
    app.filetrees.ask = Some(TreeAsk::Name { tree: id, at: dir.join("memo.txt"), kind: NameKind::Rename, text: "x".into() });
    tree_frame(&mut app, &ctx, 5.0, vec![]);
    let esc = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    tree_frame(&mut app, &ctx, 6.0, vec![esc]);
    assert!(app.filetrees.ask.is_none() && dir.join("memo.txt").is_file());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn code_in_the_pane_knows_the_trees_project_with_no_ide_involved() {
    use crate::app::filetree::code::{Found, Project};
    use crate::app::filetree::editor::Body;
    /// Frames until `done`: the project is listed, read and searched on threads.
    fn until(app: &mut DrawingApp, ctx: &Context, what: &str, done: impl Fn(&mut DrawingApp) -> bool) {
        for i in 0..400 {
            tree_frame(app, ctx, i as f64 * 0.02, vec![]);
            if done(app) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("{what}");
    }
    let ctx = Context::default();
    install_fonts(&ctx, false); // the code is drawn with the IDE's font families
    let dir = temp("code");
    let (lib, index) = (dir.join("lib.php"), dir.join("index.php"));
    fs::write(&lib, "<?php\n\nfunction montar(int $largura, string $titulo) {}\n").unwrap();
    fs::write(&index, "<?php\nmontar(10, 'a');\nnada();\n").unwrap();
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    frame(&mut app, &ctx, vec![]);
    app.ft_activate(&tree, index.clone());
    // the parameter names come from a declaration in another file of the tree's root
    let hints = |app: &DrawingApp| match &app.filetrees.editors[&tree].tabs[0].body {
        Body::Text(t) => t.laid.as_ref().map(|l| l.hints.iter().map(|(h, line, _)| (h.label.clone(), *line)).collect::<Vec<_>>()).unwrap_or_default(),
        _ => vec![],
    };
    until(&mut app, &ctx, "no parameter names", |app| hints(app) == [("largura:".to_string(), 1), ("titulo:".to_string(), 1)]);
    // Ctrl+click on the call: searched on a thread, then the declaration's file comes forward with the caret asked there
    let ask = |app: &mut DrawingApp, c: usize| {
        let ed = app.filetrees.editors.get_mut(&tree).unwrap();
        let Body::Text(t) = &ed.tabs[0].body else { panic!("not a text") };
        let (lang, text) = (t.lang, t.text.clone());
        assert_eq!(ed.code.declaration(&index, lang, &text, c, &ctx), None, "a name is searched off the UI thread");
    };
    ask(&mut app, 8);
    until(&mut app, &ctx, "the declaration's file did not open", |app| app.filetrees.editors[&tree].active.as_deref() == Some(lib.as_path()));
    let ed = &app.filetrees.editors[&tree];
    assert!(ed.preview.is_none() && ed.tabs.len() == 2, "opened for good, next to the file it was asked from");
    // a name declared nowhere says so
    ask(&mut app, 23);
    until(&mut app, &ctx, "no word of the missing declaration", |app| app.status == "Declaração de 'nada' não encontrada");
    // where it is, by the project alone
    let mut code = Project::default();
    code.open(dir.clone(), &ctx);
    while code.indexing() {
        code.poll();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let Body::Text(t) = crate::app::filetree::editor::load(&index, &ctx) else { panic!("not a text") };
    assert_eq!(code.declaration(&index, t.lang, &t.text, 8, &ctx), None);
    let found = loop {
        if let Some(found) = code.poll() {
            break found;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(found, Found::At(lib.clone(), 2, 9));
    // a tab nobody typed in never writes its file: closing it leaves what is on disk
    fs::write(&index, "<?php\n// de fora\n").unwrap();
    app.filetrees.editors.get_mut(&tree).unwrap().close(&index).unwrap();
    assert_eq!(fs::read_to_string(&index).unwrap(), "<?php\n// de fora\n");
    let _ = fs::remove_dir_all(&dir);
}
