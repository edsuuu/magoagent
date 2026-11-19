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
    // what opened did not bring the app's window to the front: only a question waiting on the person does
    assert!(!std::mem::take(&mut app.viewport_commands).contains(&egui::ViewportCommand::Focus));
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
        draft: false,
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
    assert!(app.prefs.island, "the island is on by default");
    app.act("set_setting", &json!({ "name": "island", "value": "off" })).unwrap();
    assert!(!app.prefs.island);
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
    app.act("set_setting", &json!({ "name": "island_compact_after", "value": "0" })).unwrap();
    assert_eq!(app.prefs.island_compact_after, 0, "the compact island never shrinks");
    assert!(app.act("set_setting", &json!({ "name": "island_compact_after", "value": "45" })).is_err());
    app.act("set_setting", &json!({ "name": "run_at_login", "value": "on" })).unwrap();
    assert!(app.prefs.run_at_login, "the app opens at login");
    app.act("set_setting", &json!({ "name": "run_at_login", "value": "off" })).unwrap();
    assert!(!app.prefs.run_at_login);
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
    // Soloist (Spotify's Connect receiver) only on Linux
    let soloist = if cfg!(any(windows, target_os = "macos")) { "" } else { "Soloist: desligado.\n" };
    let all = format!("GitHub: desligado.\nSpotify: desligado.\n{soloist}Hermes Agent: desligado.\nWhatsApp: desligado.");

    assert_eq!(app.act("integrations", &json!({})).unwrap(), all);
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

    app.integrations.spotify.now = Some(Now { track: Track::new("Canção", "Banda", "", "").unwrap(), playing: true, ..Default::default() });
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

/// "A Luna dança com a música" (Ajustes › Integrações › Spotify) through the MCP: its switch and the dance of now.
#[test]
fn the_agents_turn_lunas_dance_on_and_off() {
    use crate::app::integrations::spotify::{Now, Track};
    let spotify = |app: &mut DrawingApp, action: &str| app.act("integrations", &json!({ "integration": "spotify", "action": action }));
    let mut app = DrawingApp::default();

    assert!(spotify(&mut app, "dance").unwrap().starts_with("A Luna dança com a música: ligado"), "on by default");
    assert!(spotify(&mut app, "dance_off").unwrap().starts_with("A Luna dança com a música: desligado"));
    assert!(!app.prefs.integrations.luna_dances);
    spotify(&mut app, "dance_on").unwrap();
    assert!(app.prefs.integrations.luna_dances);
    // playing, before anything is heard: the clock's sway; paused: asleep
    let now = Now { track: Track::new("Canção", "Banda", "", "").unwrap(), playing: true, ..Default::default() };

    app.prefs.integrations.spotify = true;
    app.integrations.spotify.now = Some(now.clone());
    assert!(spotify(&mut app, "dance").unwrap().ends_with("Agora: Balanço."));
    app.integrations_tick(&Context::default());
    assert_eq!(app.integrations.spotify.dancer.starts, 0, "the agents asking is no dancer on screen: she does not listen");
    app.integrations.spotify.now = Some(Now { playing: false, ..now });
    assert!(spotify(&mut app, "dance").unwrap().ends_with("Agora: Cochila."));
    assert!(crate::mcp::app_tools::app_tools().iter().any(|tool| tool["inputSchema"]["properties"]["action"]["enum"].to_string().contains("dance_on")));
}

#[test]
fn the_agents_reach_soloists_own_switch() {
    use crate::app::integrations::spotify::{Command, Now, Player, Track};
    let soloist = |app: &mut DrawingApp, action: &str| app.act("integrations", &json!({ "integration": "soloist", "action": action }));
    let mut app = DrawingApp::default();

    if cfg!(any(windows, target_os = "macos")) {
        assert!(soloist(&mut app, "on").unwrap_err().contains("só existe no Linux"));

        return;
    }

    assert_eq!(soloist(&mut app, "status").unwrap(), "Soloist: desligado.");
    soloist(&mut app, "on").unwrap();
    assert!(app.prefs.integrations.spotify, "one switch for every player");
    assert!(soloist(&mut app, "").unwrap().contains("-w 127.0.0.1:0"), "not playing: how to open it");

    app.integrations.spotify.now =
        Some(Now { track: Track::new("Canção", "Banda", "", "").unwrap(), playing: true, player: Player::Soloist, ..Default::default() });

    assert_eq!(soloist(&mut app, "status").unwrap(), "Soloist: Tocando: Canção — Banda (Soloist)");
    soloist(&mut app, "next").unwrap();
    assert_eq!(app.integrations.spotify.asked, [Command::Next], "its buttons, with only Soloist on");
    assert!(app.act("integrations", &json!({})).unwrap().contains("Soloist: Tocando"));
    soloist(&mut app, "off").unwrap();
    assert!(!app.prefs.integrations.spotify);
}

/// `integrations soloist`'s Configurar: its steps in words and its buttons, one at a time; the key never comes through
/// the MCP, not even when an agent sends one.
#[test]
fn the_agents_set_soloist_up_but_never_carry_its_key() {
    use crate::app::integrations::spotify::setup::{KeyFile, Login, Program, Status};
    const KEY: &str = "spak_SEGREDO0123456789";
    let soloist = |app: &mut DrawingApp, action: &str| app.act("integrations", &json!({ "integration": "soloist", "action": action, "key": KEY }));
    let mut app = DrawingApp::default();

    if cfg!(any(windows, target_os = "macos")) {
        assert!(soloist(&mut app, "setup").unwrap_err().contains("só existe no Linux"));

        return;
    }

    assert!(soloist(&mut app, "setup").unwrap().contains("Conferindo"), "nothing looked at yet");

    app.integrations.soloist_setup.status = Some(Status {
        program: Program::Ready("soloist 1.3.10".into()),
        key: KeyFile::Private,
        unit: true,
        enabled: true,
        active: "active".into(),
        login: Login::Waiting,
        vpn: vec!["wg0".into()],
        ..Default::default()
    });

    app.integrations.soloist_setup.key_field = KEY.into();
    let report = soloist(&mut app, "setup").unwrap();

    assert!(report.contains("1. Baixar o Soloist — feito. soloist 1.3.10") && report.contains("5. Escolher o Soloist no celular — agora"), "{report}");
    assert!(report.contains("wg0") && !report.contains("SEGREDO"), "{report}");
    let refused = soloist(&mut app, "key").unwrap_err();

    assert!(refused.contains("Ajustes › Integrações › Spotify › Configurar o Soloist") && !refused.contains("SEGREDO"), "{refused}");
    assert!(soloist(&mut app, "update").unwrap().contains("Baixando"));
    assert!(soloist(&mut app, "stop").unwrap_err().contains("ocupado"), "one at a time");

    for action in ["stop", "start", "enable"] {
        app.integrations.soloist_setup.busy = None;
        soloist(&mut app, action).unwrap();
    }

    assert_eq!(app.integrations.soloist_setup.asked, ["download", "stop", "start", "turn_on"], "enable keeps the key saved before");
    assert_eq!(app.integrations.soloist_setup.key_field, KEY, "what is in Ajustes' field is the person's, untouched");
}

/// The island's Spotify page by the MCP: shuffle, repeat, the volume, a seek, play, the queue, the heart, the search and
/// the account (here on Soloist, which takes everything on every system; the Web API never reached).
#[test]
fn the_agents_do_what_the_spotify_page_does() {
    use crate::app::integrations::spotify::account::{Account, Link};
    use crate::app::integrations::spotify::library::LISTS_NEED_ACCOUNT;
    use crate::app::integrations::spotify::{Command, Now, Player, Repeat, Track};

    let spotify = |app: &mut DrawingApp, action: &str, more: Value| {
        let mut args = json!({ "integration": "spotify", "action": action });

        args.as_object_mut().unwrap().extend(more.as_object().cloned().unwrap_or_default());
        app.act("integrations", &args)
    };

    let mut app = DrawingApp::default();

    app.prefs.integrations.spotify = true;

    app.integrations.spotify.now =
        Some(Now { track: Track::new("Canção", "Banda", "", "").unwrap(), playing: true, player: Player::Soloist, ..Default::default() });

    assert_eq!(spotify(&mut app, "shuffle", json!({ "value": "on" })).unwrap(), "Embaralhar ligado.");
    assert_eq!(spotify(&mut app, "repeat", json!({ "value": "track" })).unwrap(), "Agora repetindo a faixa.");
    assert_eq!(spotify(&mut app, "volume", json!({ "value": 40 })).unwrap(), "Volume em 40%.");
    assert_eq!(spotify(&mut app, "seek", json!({ "value": "1:23" })).unwrap(), "Indo para 1:23.");
    spotify(&mut app, "play", json!({ "uri": "spotify:playlist:abc" })).unwrap();
    spotify(&mut app, "enqueue", json!({ "uri": "spotify:track:abc" })).unwrap();

    let asked = [
        Command::Shuffle(true),
        Command::Repeat(Repeat::Track),
        Command::Volume(40),
        Command::Seek(83_000),
        Command::PlayUri("spotify:playlist:abc".into()),
        Command::Enqueue("spotify:track:abc".into()),
    ];

    assert_eq!(app.integrations.spotify.asked, asked);

    for (action, more) in
        [("volume", json!({ "value": "alto" })), ("repeat", json!({})), ("enqueue", json!({ "uri": "spotify:album:abc" })), ("play", json!({}))]
    {
        assert!(spotify(&mut app, action, more).is_err(), "{action}");
    }

    assert!(spotify(&mut app, "status", json!({})).unwrap().contains("repetindo a faixa"), "status says what the page shows");
    // Soloist's queue answers without the account; the lists and the heart need it
    assert_eq!(spotify(&mut app, "queue", json!({})).unwrap(), "teste: a fila do Soloist");
    assert_eq!(spotify(&mut app, "playlists", json!({})).unwrap_err(), LISTS_NEED_ACCOUNT);
    assert!(spotify(&mut app, "like", json!({})).unwrap_err().contains("conta do Spotify"));
    assert!(spotify(&mut app, "account", json!({})).unwrap().contains("http://127.0.0.1:8990/callback"));
    // the Client ID, and Conectar (no browser in a test)
    assert!(spotify(&mut app, "client_id", json!({ "value": "abc" })).is_err());
    spotify(&mut app, "client_id", json!({ "value": "0123456789abcdef0123456789abcdef" })).unwrap();
    assert_eq!(app.prefs.integrations.spotify_client_id, "0123456789abcdef0123456789abcdef");
    spotify(&mut app, "connect", json!({})).unwrap();
    assert_eq!(app.integrations.spotify.account.link, Link::Waiting);
    // with the account: what asks the Web API answers from a thread, the rest at once
    app.integrations.spotify.account = Account::with_session("ana");

    let later = |app: &mut DrawingApp, action: &str, more: Value| {
        let mut args = json!({ "integration": "spotify", "action": action });

        args.as_object_mut().unwrap().extend(more.as_object().cloned().unwrap_or_default());
        app.act_later("integrations", &args).map(|job| job())
    };

    assert_eq!(later(&mut app, "search", json!({ "query": "lista" })), Some(Ok("teste: search lista".into())));
    assert_eq!(later(&mut app, "play", json!({ "query": "lista", "kind": "playlist" })), Some(Ok("teste: play lista".into())));
    assert_eq!(later(&mut app, "search", json!({})), Some(Err("diga o que buscar em `query`".into())));
    assert_eq!(later(&mut app, "shuffle", json!({ "value": "on" })), None, "a control answers at once");
    assert_eq!(later(&mut app, "play", json!({ "uri": "spotify:track:abc" })), None);
    assert!(app.act_later("canvas", &json!({ "integration": "spotify", "action": "search" })).is_none());
    assert_eq!(spotify(&mut app, "devices", json!({})).unwrap(), "teste: devices", "act runs the same work in place");
    spotify(&mut app, "like", json!({ "uri": "spotify:track:abc" })).unwrap();
    spotify(&mut app, "play", json!({ "value": "liked" })).unwrap();
    assert_eq!(app.integrations.spotify.library.web_asked, ["save", "control"], "the heart and Liked Songs by the Web API");
    assert!(spotify(&mut app, "status", json!({})).unwrap().contains("conectada como Ana"));
    assert!(spotify(&mut app, "disconnect", json!({})).is_ok() && !app.integrations.spotify.account.linked());
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

/// Limpar todos os dados is the person's alone: an agent asking is told where, and nothing goes.
#[test]
fn the_agents_never_wipe_the_apps_data() {
    let mut app = DrawingApp::default();
    let said = app.act("set_setting", &json!({ "name": "wipe_data", "value": "sim" })).unwrap_err();

    assert!(said.contains("Ajustes › Dados › Limpar todos os dados"), "{said}");
    assert!(!app.quitting && !app.wipe_confirm);
}

#[test]
fn registering_the_mcp_in_another_agent_runs_on_a_thread_and_never_in_a_test() {
    let mut app = DrawingApp::default();
    let args = json!({ "name": "register_mcp", "value": "claude" });
    let job = app.act_later("set_setting", &args).expect("the CLI takes seconds: off the frame");

    assert_eq!(job().unwrap_err(), "nunca nos testes");
    assert!(app.act_later("set_setting", &json!({ "name": "theme", "value": "dark" })).is_none());
    assert!(app.act("set_setting", &json!({ "name": "register_mcp", "value": "gemini" })).unwrap_err().contains("claude ou codex"));
    let tools = crate::mcp::app_tools::app_tools();
    let tool = tools.iter().find(|tool| tool["name"] == "set_setting").unwrap();

    assert!(tool["inputSchema"]["properties"]["name"]["enum"].as_array().unwrap().contains(&json!("register_mcp")));
}

#[test]
fn a_terminal_an_agent_opens_is_roped_to_it() {
    let mut app = DrawingApp::default();

    app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let named = |app: &DrawingApp, name: &str| app.scene.elements.iter().find(|e| e.terminal.as_ref().is_some_and(|t| t.name == name)).cloned();

    // nobody to hang it from (an agent outside the app): no terminal loose on the canvas
    assert!(app.act("create_terminal", &json!({ "command": "cargo test" })).is_err());
    assert!(app.scene.elements.iter().all(|e| e.terminal.is_none()));
    // from the Luna's terminal: to its right, in its folder, running the command, roped to it
    let mut luna = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });

    luna.terminal = Some(TerminalSpec { name: "Luna".into(), cwd: "/p/loja".into(), maestro: true, ..Default::default() });
    let luna_id = luna.id.clone();

    app.scene.elements.push(luna);
    let said = app.act("create_terminal", &json!({ "command": "cargo test", "asker": luna_id })).unwrap();
    let tests = named(&app, "cargo").unwrap();
    let spec = tests.terminal.as_ref().unwrap();

    assert_eq!((spec.command.as_str(), spec.cwd.as_str(), spec.maestro), ("cargo test", "/p/loja", false));
    assert!(element_bounds(&tests).min.x >= 400.0, "to its right");
    assert!(app.links().get(&luna_id).is_some_and(|links| links.contains(&tests.id)), "roped to who opened it");
    assert!(said.contains("\"cargo\"") && said.contains("read_terminal"), "{said}");
    // a copy it makes of it hangs from it too
    app.act("terminal_menu", &json!({ "name": "cargo", "action": "duplicate", "asker": luna_id })).unwrap();
    let copy = named(&app, "cargo 2").unwrap();

    assert!(app.links().get(&luna_id).is_some_and(|links| links.contains(&copy.id)));
    // one Cmd+Z takes back the copy and its rope; another, the terminal and its rope
    app.undo();
    app.undo();
    assert!(named(&app, "cargo").is_none() && app.scene.elements.iter().all(|e| !e.rope));

    // from a chat with no card on the canvas: its card comes, and the terminal hangs from it, named and where it says
    app.chats = vec![crate::app::chat::direct::Chat {
        id: "c".into(),
        agent: "claude".into(),
        name: "Luna".into(),
        cwd: "/p/loja".into(),
        session: None,
        mode: 0,
        model: String::new(),
        effort: String::new(),
        recruit: false,
        named: false,
        draft: false,
    }];

    app.act("create_terminal", &json!({ "command": "npm run dev", "name": "API", "cwd": "/p/loja/api", "asker": "c" })).unwrap();
    let api = named(&app, "API").unwrap();

    assert_eq!(api.terminal.as_ref().unwrap().cwd, "/p/loja/api");
    assert!(app.chat_node("c").is_some());
    assert!(app.links().get("c").is_some_and(|links| links.contains(&api.id)));
}
