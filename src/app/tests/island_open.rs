//! What a click opens from the island: a mini of the compact island opens it on its agent, the pill already in the
//! card opens it as its Abrir does, and the Abrir of a session outside the app opens its conversation, read from the
//! file Claude Code writes and followed while it works, answered in its terminal.

use super::*;
use crate::app::island::{COMPACT_W, EXPANDED_W, Tab};
use std::io::Write as _;

/// One frame of the island; the texts it painted.
fn step(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) -> Vec<String> {
    island_texts(app, ctx, time, events)
}

/// The pointer at `at` for `seconds`; the texts of the last frame.
fn rest(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2, seconds: f64) -> Vec<String> {
    let end = *time + seconds;
    let mut texts = Vec::new();

    while *time < end {
        texts = step(app, ctx, *time, vec![egui::Event::PointerMoved(at)]);
        *time += 0.05;
    }

    texts
}

/// A click as the hand makes it, and a second for what it opens to settle.
fn click(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2) -> Vec<String> {
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };

    for events in [vec![egui::Event::PointerMoved(at)], vec![egui::Event::PointerMoved(at)], vec![press(true)], vec![press(false)]] {
        step(app, ctx, *time, events);
        *time += 0.05;
    }

    rest(app, ctx, time, at, 1.0)
}

/// The compact island under the pointer, with two chats of the app.
fn compact() -> (DrawingApp, Context, f64) {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.prefs.island = true;
    app.chats = vec![chat("a", "claude", "Claude Code · loja"), chat("b", "claude", "Codex · site")];
    let mut time = 1.0;

    rest(&mut app, &ctx, &mut time, Pos2::new(ISLAND_SCREEN.x / 2.0, 10.0), 1.0);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "compact on hover: {}", app.island.size().x);
    (app, ctx, time)
}

/// The second pill of the grid (two columns in the right card, 8 in from its sides, the row in the middle).
fn second_pill(app: &DrawingApp) -> Pos2 {
    let rect = island_rect(app);
    let card = Rect::from_min_max(Pos2::new(rect.left() + 10.0 + 332.0, rect.top() + 42.0), rect.max - Vec2::new(10.0, 10.0));
    let inner = card.shrink2(Vec2::new(8.0, 0.0));
    let width = (inner.width() - 4.0) / 2.0;

    Pos2::new(inner.left() + width + 4.0 + width / 2.0, inner.center().y)
}

/// The first row of the feed list in Início (the right card, after the summary card).
fn first_row(app: &DrawingApp) -> Pos2 {
    let rect = island_rect(app);
    let page = Rect::from_min_max(Pos2::new(rect.left() + 10.0, rect.top() + 32.0), rect.max - Vec2::new(10.0, 10.0));
    // left card (summary) width + 10 gap
    let left_w = 236.0 + 10.0;
    let card = Rect::from_min_max(Pos2::new(page.left() + left_w, page.top()), page.max);
    let inner = card.shrink2(Vec2::new(8.0, 7.0));

    // first row center
    Pos2::new(inner.center().x, inner.top() + 19.0) // ROW_H / 2 = 38 / 2 = 19
}

#[test]
fn a_click_on_a_mini_of_the_compact_island_opens_it_on_that_agent() {
    let (mut app, ctx, mut time) = compact();
    // the minis where the compact island puts them (two: side by side on the middle line); "b" is the second
    let second = crate::app::island::minis::mini_cells(island_rect(&app), 2)[1].center();

    click(&mut app, &ctx, &mut time, second);
    assert!((app.island.size().x - EXPANDED_W).abs() < 1.0, "open: {}", app.island.size().x);
    assert_eq!((app.island.tab(), app.island.focus()), (Tab::Chats, Some("b")), "on the agents' page, b in the card");
}

#[test]
fn the_click_on_the_pill_already_in_the_card_opens_its_agent() {
    let (mut app, ctx, mut time) = compact();
    let rect = island_rect(&app);

    click(&mut app, &ctx, &mut time, rect.center());
    app.island.tab = Tab::Chats;
    let middle = island_rect(&app).center();

    rest(&mut app, &ctx, &mut time, middle, 0.5);
    let pill = second_pill(&app);

    // the first click puts it in the card, as before
    click(&mut app, &ctx, &mut time, pill);
    assert_eq!((app.island.tab(), app.island.focus(), app.chat_agent.as_deref()), (Tab::Chats, Some("b"), None));
    // the second, on the one in the card, is its Abrir: a chat opens in the island's conversation
    click(&mut app, &ctx, &mut time, pill);
    assert_eq!((app.island.tab(), app.chat_agent.as_deref()), (Tab::Chat, Some("b")));
}

#[test]
fn a_click_on_inicios_line_of_a_session_outside_opens_its_conversation_at_once() {
    let dir = std::env::temp_dir().join(format!("magoagent-island-feed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ilha-inicio.jsonl");

    fs::write(&file, said("user", "arrume o login") + &said("assistant", "Vou olhar o formulário.")).unwrap();
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.prefs.island = true;
    let hook = |event: &str| json!({ "hook_event_name": event, "session_id": "ilha-inicio", "cwd": "/tmp/loja", "transcript_path": file.display().to_string(), "prompt": "arrume o login" });

    app.take_outside_hook(&hook("UserPromptSubmit"), None);
    let id = app.outside_pills()[0].id.clone();
    let mut time = 1.0;

    rest(&mut app, &ctx, &mut time, Pos2::new(ISLAND_SCREEN.x / 2.0, 10.0), 1.0);
    let middle = island_rect(&app).center();

    click(&mut app, &ctx, &mut time, middle);
    assert_eq!(app.island.tab(), Tab::Home);
    // one click, and its conversation is there with what the terminal said
    let row_pos = first_row(&app);
    let texts = click(&mut app, &ctx, &mut time, row_pos);

    assert_eq!((app.island.tab(), app.island.focus()), (Tab::Chat, Some(id.as_str())), "{texts:?}");
    assert!(texts.iter().any(|text| text == "arrume o login"), "{texts:?}");
    let _ = fs::remove_dir_all(dir);
}

/// A line of Claude Code's conversation file.
fn said(kind: &str, text: &str) -> String {
    let content = if kind == "user" { json!(text) } else { json!([{ "type": "text", "text": text }]) };

    format!("{}\n", json!({ "type": kind, "message": { "content": content } }))
}

#[test]
fn abrir_on_a_session_outside_opens_its_conversation_followed_while_it_works_and_answered_in_its_terminal() {
    let dir = std::env::temp_dir().join(format!("magoagent-island-session-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ilha-abrir.jsonl");

    fs::write(&file, said("user", "arrume o login") + &said("assistant", "Vou olhar o formulário.")).unwrap();
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.prefs.island = true;
    // the session's hook brings where its conversation is
    let hook = |event: &str| json!({ "hook_event_name": event, "session_id": "ilha-abrir", "cwd": "/tmp/loja", "transcript_path": file.display().to_string(), "prompt": "arrume o login" });

    app.take_outside_hook(&hook("UserPromptSubmit"), None);
    let id = app.outside_pills()[0].id.clone();

    assert_eq!(app.outside_transcript(&id), Some(file.clone()));
    let mut time = 1.0;

    rest(&mut app, &ctx, &mut time, Pos2::new(ISLAND_SCREEN.x / 2.0, 10.0), 1.0);
    let middle = island_rect(&app).center();

    click(&mut app, &ctx, &mut time, middle);
    // Abrir in the card: the conversation page, with what was said in the terminal, and where to answer
    app.island_show(&id, time);
    let middle = island_rect(&app).center();
    let texts = rest(&mut app, &ctx, &mut time, middle, 1.0);

    assert_eq!((app.island.tab(), app.island.focus()), (Tab::Chat, Some(id.as_str())));

    for words in ["arrume o login", "Vou olhar o formulário.", "Mensagem para esta sessão…", "Ir ao terminal"] {
        assert!(texts.iter().any(|text| text == words), "{words:?} in {texts:?}");
    }

    // it goes on working: the new answer shows in a moment
    fs::OpenOptions::new().append(true).open(&file).unwrap().write_all(said("assistant", "Pronto, corrigido.").as_bytes()).unwrap();
    let middle = island_rect(&app).center();
    let texts = rest(&mut app, &ctx, &mut time, middle, 1.0);

    assert!(texts.iter().any(|text| text == "Pronto, corrigido."), "{texts:?}");
    // its terminal's window is not known here: the box says so, and the answer is still the terminal's
    let rect = island_rect(&app);
    let go = Pos2::new(rect.right() - 10.0 - 16.0 - 7.0 - 40.0, rect.bottom() - 10.0 - 14.0 - 20.0);
    let texts = click(&mut app, &ctx, &mut time, go);

    assert!(texts.iter().any(|text| text.starts_with("Não deu para trazer a janela")), "{texts:?}");
    // another page leaves it: the conversation page is the app's chats' again
    let home = Pos2::new(island_rect(&app).left() + 29.0, 25.0);

    click(&mut app, &ctx, &mut time, home);
    assert_eq!((app.island.tab(), app.island.session.is_none()), (Tab::Home, true));
    let _ = fs::remove_dir_all(dir);
}

/// WhatsApp's line in Início opens Ajustes › Integrações while the gateway is not ready, and folds the island so the
/// app shows.
#[test]
fn a_click_on_whatsapps_line_in_inicio_opens_its_settings() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.prefs.island = true;
    app.prefs.integrations.whatsapp = true;
    let mut time = 1.0;

    rest(&mut app, &ctx, &mut time, Pos2::new(ISLAND_SCREEN.x / 2.0, 10.0), 1.0);
    let middle = island_rect(&app).center();

    click(&mut app, &ctx, &mut time, middle);
    assert_eq!(app.island.tab(), Tab::Home);
    let row = first_row(&app);
    let texts = click(&mut app, &ctx, &mut time, row);

    assert!(texts.iter().any(|text| text == "Gateway ainda não testado"), "{texts:?}");
    app.integrations_tick(&ctx);
    let integrations = crate::app::ui::settings::SETTINGS_TABS.iter().position(|tab| *tab == "Integrações");

    assert!(app.settings_open && app.raise_window && app.integrations.scroll_to_whatsapp);
    assert_eq!(Some(app.settings_tab), integrations);
    // the MCP does the same
    app.settings_open = false;
    assert_eq!(app.act("integrations", &json!({ "integration": "whatsapp", "action": "open" })).unwrap(), "Abri Ajustes › Integrações no WhatsApp.");
    assert!(app.settings_open);
    // ready: its gateway's own page instead (never opened in a test), Ajustes stays as it is
    app.settings_open = false;
    app.prefs.integrations.whatsapp_base = "http://127.0.0.1:3000".into();
    app.integrations.whatsapp.link = crate::app::integrations::whatsapp::Link::Connected;
    assert_eq!(app.whatsapp_open(), "Abri a página do gateway do WhatsApp no navegador.");
    assert!(!app.settings_open);
}
