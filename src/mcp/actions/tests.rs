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
    // the folder: a file tree on it, enlarged, and its structure
    assert!(app.act("ide_folder", &json!({ "path": path })).unwrap().contains("main.rs"));
    let ide = app.scene.elements.iter().position(|e| e.filetree.is_some()).unwrap();
    assert_eq!(app.terminal_elevated.as_ref(), Some(&app.scene.elements[ide].id));
    assert!(!app.chat_open);
    // a source file opens in that same tree's editor pane
    app.act("open_path", &json!({ "path": dir.join("src/main.rs").to_string_lossy(), "line": 1 })).unwrap();
    assert_eq!(app.scene.elements.iter().filter(|e| e.filetree.is_some()).count(), 1);
    let id = app.scene.elements[ide].id.clone();
    assert!(app.filetrees.editors.get(&id).is_some_and(|ed| ed.active.as_ref().is_some_and(|p| p.ends_with("main.rs"))));
    // a PDF opens in a portal, roped to the chat or terminal that asked
    let asker = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 200.0, height: 100.0 });
    let asker_id = asker.id.clone();
    app.scene.elements.push(asker);
    app.act("open_path", &json!({ "path": dir.join("manual.pdf").to_string_lossy(), "asker": asker_id })).unwrap();
    let (portal_id, portal) = app.scene.elements.iter().find_map(|e| e.portal.as_ref().map(|p| (e.id.clone(), p.clone()))).unwrap();
    assert!(portal.url.starts_with("file:///") && portal.url.ends_with("manual.pdf"), "{}", portal.url);
    assert!(app.links().get(&asker_id).is_some_and(|links| links.contains(&portal_id)), "roped to who asked");
    // resized by name, enlarged, back, and the app's window
    let name = app.scene.elements[ide].filetree.as_ref().unwrap().name();
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
        named: false,
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
    // Renomear: the name given wins over the conversation's own title; a blank one is refused
    app.transcripts.get_mut("b").unwrap().title = Some("Revisão do site".into());
    let row = |app: &DrawingApp| app.chat_rows().into_iter().find(|r| r.id == "b").unwrap();
    assert_eq!(app.chat_title(&row(&app)), "Revisão do site");
    app.act("chat_rename", &json!({ "chat": "site", "name": " Loja nova " })).unwrap();
    assert_eq!(app.chat_title(&row(&app)), "Loja nova");
    assert!(app.act("chat_rename", &json!({ "chat": "Loja nova", "name": " " })).is_err());
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
    // the selected window's outline: crawling by default, still, or none; an unknown one is refused
    assert_eq!(app.prefs.window_outline, WindowOutline::Animated);
    assert!(!app.prefs.island, "the island is off until it is asked for");
    app.act("set_setting", &json!({ "name": "island", "value": "on" })).unwrap();
    assert!(app.prefs.island);
    app.act("set_setting", &json!({ "name": "island", "value": "off" })).unwrap();
    assert!(app.prefs.island_sound, "its sounds are on by default");
    app.act("set_setting", &json!({ "name": "island_sound", "value": "off" })).unwrap();
    assert!(!app.prefs.island_sound);
    app.act("set_setting", &json!({ "name": "island_sound", "value": "on" })).unwrap();
    assert!(app.prefs.island_sound);
    app.act("set_setting", &json!({ "name": "island_volume", "value": "0.15" })).unwrap();
    app.act("set_setting", &json!({ "name": "island_close_after", "value": "30" })).unwrap();
    app.act("set_setting", &json!({ "name": "island_color", "value": "#123456" })).unwrap();
    assert_eq!((app.prefs.island_volume, app.prefs.island_close_after, app.prefs.island_color), (0.15, 30, [0x12, 0x34, 0x56]));
    assert!(app.act("set_setting", &json!({ "name": "island_volume", "value": "0.5" })).is_err());
    assert!(app.act("set_setting", &json!({ "name": "island_close_after", "value": "60" })).is_err());
    assert!(app.act("set_setting", &json!({ "name": "island_color", "value": "pink" })).is_err());
    assert!(!app.prefs.island);
    assert!(app.act("set_setting", &json!({ "name": "island", "value": "talvez" })).is_err());
    app.act("set_setting", &json!({ "name": "window_outline", "value": "Still" })).unwrap();
    assert_eq!(app.prefs.window_outline, WindowOutline::Still);
    app.act("set_setting", &json!({ "name": "window_outline", "value": "none" })).unwrap();
    assert_eq!(app.prefs.window_outline, WindowOutline::Hidden);
    assert!(app.act("set_setting", &json!({ "name": "window_outline", "value": "piscando" })).is_err());
    app.act("set_setting", &json!({ "name": "voice", "value": "openai" })).unwrap();
    assert!(app.prefs.theme == Theme::Light && !app.prefs.voice_codex && app.prefs.voice_openai);
    // VoiceStudio: the engine, its server's address and its voice
    app.act("set_setting", &json!({ "name": "voice", "value": "voicestudio" })).unwrap();
    app.act("set_setting", &json!({ "name": "voice_studio_url", "value": "localhost:4000" })).unwrap();
    app.act("set_setting", &json!({ "name": "voice_studio_voice", "value": "p1" })).unwrap();
    assert!(!app.prefs.voice_codex && !app.prefs.voice_openai);
    assert_eq!((app.prefs.voice_studio_url.as_str(), app.prefs.voice_studio_voice.as_str()), ("localhost:4000", "p1"));
    assert!(app.act("set_setting", &json!({ "name": "voice", "value": "local" })).is_err(), "the local voice is gone");
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

/// `integrations`: GitHub's switch, what it shows and its buttons; the token itself never comes through the MCP.
#[test]
fn the_agents_turn_github_on_read_its_pull_requests_and_never_see_the_token() {
    use crate::app::integrations::github::{GitHub, Pulse};
    let token = "ghp_0123456789abcdefghijABCDEFGHIJ012345";
    let mut app = DrawingApp::default();
    app.integrations.github = GitHub::with_token("");
    assert_eq!(app.act("integrations", &json!({})).unwrap(), "GitHub: desligado.\nSpotify: desligado.\nHermes Agent: desligado.\nWhatsApp: desligado.");
    assert!(app.act("integrations", &json!({ "integration": "github", "action": "test" })).is_err(), "off: nothing to test");
    app.act("integrations", &json!({ "integration": "github", "action": "on" })).unwrap();
    assert!(app.prefs.integrations.github);
    assert!(app.act("integrations", &json!({ "integration": "github" })).unwrap().contains("sem token"));
    assert!(app.act("integrations", &json!({ "integration": "github", "action": "test" })).is_err(), "no token: nothing to test");
    app.integrations.github = GitHub::with_token(token);
    app.integrations.github.pulse = Pulse::parse(&serde_json::from_str(include_str!("../../app/integrations/fixtures/github_pulse.json")).unwrap());
    let status = app.act("integrations", &json!({ "integration": "github", "action": "status" })).unwrap();
    assert!(status.contains("@ana-dev") && status.contains("ana-dev/site#7") && status.contains("checks falharam"), "{status}");
    assert!(!status.contains(token), "the token stays out of what the agents read");
    app.act("integrations", &json!({ "integration": "github", "action": "test" })).unwrap();
    app.act("integrations", &json!({ "integration": "github", "action": "off" })).unwrap();
    assert!(!app.prefs.integrations.github);
    assert!(app.act("integrations", &json!({ "integration": "github", "action": "explodir" })).is_err());
    assert!(app.act("integrations", &json!({ "integration": "orkut", "action": "on" })).is_err());
    assert!(crate::mcp::app_tools::is_action("integrations"), "listed among the tools");
}

#[test]
fn the_agents_see_what_spotify_plays_and_press_its_buttons() {
    use crate::app::integrations::spotify::{Command, Now, Track};
    let spotify = |app: &mut DrawingApp, action: &str| app.act("integrations", &json!({ "integration": "spotify", "action": action }));
    let mut app = DrawingApp::default();
    app.integrations.spotify.now = Some(Now { track: Track::new("Canção", "Banda", "", "").unwrap(), playing: true });
    assert_eq!(spotify(&mut app, "status").unwrap(), "Spotify: desligado.");
    assert!(spotify(&mut app, "next").is_err(), "off: nothing is asked of the player");
    spotify(&mut app, "on").unwrap();
    assert!(app.prefs.integrations.spotify);
    assert_eq!(spotify(&mut app, "").unwrap(), "Spotify: Tocando: Canção — Banda");
    for action in ["previous", "playpause", "next"] {
        spotify(&mut app, action).unwrap();
    }
    assert_eq!(app.integrations.spotify.asked, [Command::Previous, Command::PlayPause, Command::Next]);
    assert!(spotify(&mut app, "explodir").is_err());
    app.integrations.spotify.now = None;
    assert!(spotify(&mut app, "status").unwrap().contains("não está aberto"));
    assert!(spotify(&mut app, "playpause").is_err(), "closed: nothing to press");
    spotify(&mut app, "off").unwrap();
    assert!(!app.prefs.integrations.spotify);
}

/// `integrations hermes`: the switch, and the plugin's buttons (here on a Hermes that is not there: the tests' own
/// folder, which is never made; what they write is in integrations/hermes/tests.rs).
#[test]
fn the_agents_reach_the_hermes_switch_and_the_plugins_buttons() {
    use crate::app::integrations::hermes;
    let act = |app: &mut DrawingApp, action: &str| app.act("integrations", &json!({ "integration": "hermes", "action": action }));
    let mut app = DrawingApp::default();
    let home = hermes::home().unwrap();
    assert!(home.starts_with(std::env::temp_dir()), "never the person's own Hermes");
    assert_eq!(act(&mut app, "status").unwrap(), format!("Hermes Agent: desligado. {}", hermes::Status::NoHermes.words()));
    act(&mut app, "on").unwrap();
    assert!(app.prefs.integrations.hermes && act(&mut app, "status").unwrap().contains("ligado."));
    assert_eq!(act(&mut app, "install").unwrap_err(), hermes::Status::NoHermes.words());
    assert!(act(&mut app, "remove").unwrap().contains("não estava instalado"));
    assert!(!home.exists(), "nothing was written");
    assert!(act(&mut app, "explodir").is_err());
    act(&mut app, "off").unwrap();
    assert!(!app.prefs.integrations.hermes);
}
