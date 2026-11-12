use super::*;

#[test]
fn the_agents_open_a_folder_a_pdf_and_resize_windows() {
    let dir = std::env::temp_dir().join(format!("mago-acts-{}", std::process::id()));
    fs::create_dir_all(dir.join("src/fundo")).unwrap();
    fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.join("manual.pdf"), "%PDF-1.4\n").unwrap();
    fs::write(dir.join(".oculto"), "").unwrap();
    let tree = folder_tree(&dir, 3, 300);
    assert_eq!(tree, "src/\n  fundo/\n  main.rs\nmanual.pdf");
    let mut app = DrawingApp::default();
    app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    app.chat_open = true;
    let path = dir.to_string_lossy().to_string();
    // the folder: an IDE on it, enlarged, and its structure
    assert!(app.act("ide_folder", &json!({ "path": path })).unwrap().contains("main.rs"));
    let ide = app.scene.elements.iter().position(|e| e.ide.is_some()).unwrap();
    assert_eq!(app.terminal_elevated.as_ref(), Some(&app.scene.elements[ide].id));
    assert!(!app.chat_open);
    // a source file goes to that same IDE, once its editor is up
    app.act("open_path", &json!({ "path": dir.join("src/main.rs").to_string_lossy(), "line": 1 })).unwrap();
    assert_eq!(app.scene.elements.iter().filter(|e| e.ide.is_some()).count(), 1);
    assert!(app.open_pending.as_ref().is_some_and(|(id, ..)| *id == app.scene.elements[ide].id));
    // a PDF opens in a portal
    app.act("open_path", &json!({ "path": dir.join("manual.pdf").to_string_lossy() })).unwrap();
    let portal = app.scene.elements.iter().find_map(|e| e.portal.as_ref()).unwrap();
    assert!(portal.url.starts_with("file:///") && portal.url.ends_with("manual.pdf"), "{}", portal.url);
    // resized by name, enlarged, back, and the app's window
    let name = app.scene.elements[ide].ide.as_ref().unwrap().name.clone();
    app.act("resize_window", &json!({ "target": name, "width": 900, "height": 700 })).unwrap();
    assert!(matches!(app.scene.elements[ide].kind, ElementKind::Rectangle { width: 900.0, height: 700.0, .. }));
    app.act("resize_window", &json!({ "target": name, "state": "shrink" })).unwrap();
    assert!(app.terminal_elevated.is_none());
    // what opened brought the app's window to the front
    assert!(std::mem::take(&mut app.viewport_commands).contains(&egui::ViewportCommand::Focus));
    app.act("resize_window", &json!({ "target": "app", "state": "maximize" })).unwrap();
    assert_eq!(app.viewport_commands, vec![egui::ViewportCommand::Maximized(true)]);
    assert!(app.act("resize_window", &json!({ "target": "nada" })).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_ide_commit_names_its_files() {
    let dir = std::env::temp_dir().join(format!("mago-acts-git-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    assert!(crate::ide::command("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    app.act("ide_folder", &json!({ "path": dir.to_string_lossy() })).unwrap();
    app.sync_ides(&Context::default());
    // `files` reaches the IDE: a file with no change is refused, nothing runs
    let err = app.act("ide_git", &json!({ "action": "commit", "message": "m", "files": ["nada.txt"] })).unwrap_err();
    assert!(err.contains("nada.txt"), "{err}");
    assert!(app.act("ide_git", &json!({ "action": "commit" })).is_err(), "no message");
    app.ides.clear();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_ide_excludes_and_includes_a_folder() {
    let dir = std::env::temp_dir().join(format!("mago-acts-exclude-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("docs")).unwrap();
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    app.act("ide_folder", &json!({ "path": dir.to_string_lossy() })).unwrap();
    app.sync_ides(&Context::default());
    assert!(app.act("ide_exclude", &json!({ "action": "list" })).unwrap().contains("node_modules"));
    assert!(app.act("ide_exclude", &json!({ "action": "exclude", "path": "docs" })).unwrap().contains("excluída"));
    assert!(app.act("ide_exclude", &json!({ "action": "list" })).unwrap().contains("/docs"));
    assert!(app.act("ide_exclude", &json!({ "action": "include", "path": "docs" })).unwrap().contains("deixou"));
    assert!(app.act("ide_exclude", &json!({ "action": "exclude", "path": "../fora" })).is_err());
    assert!(app.act("ide_exclude", &json!({ "action": "exclude" })).is_err(), "no folder");
    app.ides.clear();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_ide_diff_takes_a_base_and_a_mode() {
    let dir = std::env::temp_dir().join(format!("mago-acts-diff-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    assert!(crate::ide::command("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    app.act("ide_folder", &json!({ "path": dir.to_string_lossy() })).unwrap();
    app.sync_ides(&Context::default());
    let err = app.act("ide_git", &json!({ "action": "diff", "mode": "lado" })).unwrap_err();
    assert!(err.contains("lado"), "{err}");
    assert!(app.act("ide_git", &json!({ "action": "diff", "base": "nao-existe" })).unwrap_err().contains("nao-existe"));
    // empty base is HEAD; the mode sticks
    assert!(app.act("ide_git", &json!({ "action": "diff", "base": "", "mode": "unified" })).unwrap().contains("HEAD"));
    assert!(app.ides.values().all(|ide| ide.unified_diff));
    app.ides.clear();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_ide_git_reaches_branches_stash_and_history() {
    let dir = std::env::temp_dir().join(format!("mago-acts-branches-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    assert!(crate::ide::command("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    app.act("ide_folder", &json!({ "path": dir.to_string_lossy() })).unwrap();
    app.sync_ides(&Context::default());
    assert!(app.act("ide_git", &json!({ "action": "branches" })).unwrap().contains("Remotos configurados: nenhum"));
    assert_eq!(app.act("ide_git", &json!({ "action": "log" })).unwrap(), "Nenhum commit ainda.");
    assert_eq!(app.act("ide_git", &json!({ "action": "stash_list" })).unwrap(), "Nenhum stash.");
    // what loses work wants the confirmation, and a missing name is said
    assert!(app.act("ide_git", &json!({ "action": "branch_delete", "name": "x" })).unwrap_err().contains("confirm"));
    assert!(app.act("ide_git", &json!({ "action": "stash_drop" })).unwrap_err().contains("confirm"));
    assert!(app.act("ide_git", &json!({ "action": "merge" })).unwrap_err().contains("diga"));
    assert!(app.act("ide_git", &json!({ "action": "fetch" })).is_err(), "no remote");
    assert!(app.act("ide_git", &json!({ "action": "show", "name": "nada" })).is_err());
    app.ides.clear();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_agents_drive_chats_tabs_settings_and_history() {
    let mut app = DrawingApp::default();
    app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let chat = |id: &str, name: &str| crate::app::chat::direct::Chat {
        id: id.into(),
        agent: "claude".into(),
        name: name.into(),
        cwd: "C:/p".into(),
        session: None,
        mode: 0,
        model: String::new(),
        effort: String::new(),
        recruit: false,
    };
    app.chats = vec![chat("a", "Claude Code · loja"), chat("b", "Codex · site")];
    // chats: listed, opened by part of the name, a message queued for the next frame
    app.act("chat_open", &json!({ "chat": "site" })).unwrap();
    assert!(app.chat_open && app.chat_agent.as_deref() == Some("b"));
    assert!(app.act("chat_list", &json!({})).unwrap().contains("Codex · site (aberto)"));
    app.act("chat_send", &json!({ "chat": "loja", "text": "roda os testes" })).unwrap();
    app.transcripts
        .entry("b".into())
        .or_insert_with(|| crate::transcript::Transcript::new(PathBuf::new()))
        .items
        .push(crate::transcript::ChatItem::Agent("A revisão está pronta".into()));
    assert!(app.act("chat_search", &json!({ "query": "revisao" })).unwrap().contains("site"));
    assert_eq!(app.chat_outbox, vec![("a".to_string(), "roda os testes".to_string())]);
    assert!(app.act("chat_open", &json!({ "chat": "nada" })).is_err());
    assert!(app.act("voice_call", &json!({ "state": "mute" })).is_err(), "no call on");
    // a portal's tabs
    let mut portal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 600.0, height: 400.0 });
    portal.portal = Some(PortalSpec { name: "Docs".into(), url: "https://a.dev".into(), ..Default::default() });
    app.scene.elements.push(portal);
    app.act("portal_tab", &json!({ "portal": "docs", "action": "new", "url": "b.dev" })).unwrap();
    let spec = |app: &DrawingApp| app.scene.elements[0].portal.clone().unwrap();
    assert_eq!((spec(&app).tab_urls(), spec(&app).active_tab()), (vec!["https://a.dev".to_string(), "https://b.dev".to_string()], 1));
    app.act("portal_tab", &json!({ "portal": "Docs", "action": "select", "index": 0 })).unwrap();
    app.act("portal_tab", &json!({ "portal": "Docs", "action": "close", "index": 0 })).unwrap();
    assert_eq!((spec(&app).url.as_str(), spec(&app).tab_urls().len()), ("https://b.dev", 1));
    assert!(app.act("portal_tab", &json!({ "portal": "Docs", "action": "close" })).is_err(), "the last tab stays");
    assert!(app.act("terminal_restart", &json!({ "name": "Docs" })).is_err());
    // settings, closing a window and undoing it
    app.act("set_setting", &json!({ "name": "theme", "value": "claro" })).unwrap();
    app.act("set_setting", &json!({ "name": "voice", "value": "local" })).unwrap();
    assert!(app.prefs.theme == Theme::Light && !app.prefs.voice_codex && !app.prefs.voice_openai);
    // the avatar: shown, and left where it is instead of following the window in use
    app.act("set_setting", &json!({ "name": "avatar", "value": "on" })).unwrap();
    app.act("set_setting", &json!({ "name": "avatar_follow", "value": "off" })).unwrap();
    assert!(app.prefs.avatar && app.prefs.avatar_at.is_some());
    app.act("set_setting", &json!({ "name": "avatar_follow", "value": "on" })).unwrap();
    assert_eq!(app.prefs.avatar_at, None);
    app.act("close_window", &json!({ "name": "docs" })).unwrap();
    assert!(app.scene.elements.is_empty());
    app.act("history", &json!({ "action": "undo" })).unwrap();
    assert_eq!(app.scene.elements.len(), 1);
}
