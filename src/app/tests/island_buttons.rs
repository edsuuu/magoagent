//! The island's buttons one by one, by the pointer as the person uses them: the tabs, the sound, the close, the
//! conversation's size, its ✕ and full screen, Esc, and the pointer resting on it (it stays open, steady, the hover of a
//! pill silent).

use super::*;
use crate::app::island::{CLOSED_W, COMPACT_W, EXPANDED_W, Tab};

const SIZE: Vec2 = Vec2::new(1200.0, 800.0);

fn chat(id: &str, name: &str) -> crate::app::chat::direct::Chat {
    crate::app::chat::direct::Chat {
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
    }
}

/// The app, its island opened by clicking on the compact pill, and the time it is at.
fn opened() -> (DrawingApp, Context, f64) {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.prefs.island = true;
    app.chats = vec![chat("a", "Claude Code · loja"), chat("b", "Codex · site")];
    let mut time = 1.0;
    let notch = Pos2::new(SIZE.x / 2.0, 10.0);
    while time < 2.0 {
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(notch)]);
        time += 0.05;
    }
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "compact on hover: {}", app.island.size().x);
    let press = |pressed| egui::Event::PointerButton { pos: notch, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    step(&mut app, &ctx, time, vec![press(true)]);
    time += 0.05;
    step(&mut app, &ctx, time, vec![press(false)]);
    time += 0.05;
    while time < 3.0 {
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(notch)]);
        time += 0.05;
    }
    assert!((app.island.size().x - EXPANDED_W).abs() < 1.0, "clicked to open: {}", app.island.size().x);
    (app, ctx, time)
}

fn step(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), time: Some(time), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.island_ui(ctx));
    output.textures_delta.clear();
}

/// The island as drawn: hung from the top centre.
fn island_rect(app: &DrawingApp) -> Rect {
    let size = app.island.size();
    Rect::from_min_size(Pos2::new((SIZE.x / 2.0 - size.x / 2.0).round(), 0.0), size)
}

/// A header button, counted from the right: 0 the close, 1 the sound, 2 the settings, 3 the voice, 4 the size.
fn right_button(app: &DrawingApp, index: usize) -> Pos2 {
    let rect = island_rect(app);
    Pos2::new(rect.right() - 25.0 - index as f32 * 32.0, rect.top() + 25.0)
}

/// A tab: 0 Home, 1 the conversation, 2 the new chat.
fn tab_button(app: &DrawingApp, index: usize) -> Pos2 {
    let rect = island_rect(app);
    Pos2::new(rect.left() + 29.0 + index as f32 * 35.0, rect.top() + 25.0)
}

/// A click as the hand makes it: the pointer comes, rests, presses, lets go; then a few frames for it to settle.
fn click(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2) {
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    for events in [vec![egui::Event::PointerMoved(at)], vec![egui::Event::PointerMoved(at)], vec![press(true)], vec![press(false)]] {
        step(app, ctx, *time, events);
        *time += 0.05;
    }
    rest(app, ctx, time, at, 0.5);
}

/// The pointer still at `at` for `seconds`.
fn rest(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2, seconds: f64) {
    let end = *time + seconds;
    while *time < end {
        step(app, ctx, *time, vec![egui::Event::PointerMoved(at)]);
        *time += 0.05;
    }
}

#[test]
fn the_tabs_change_the_page() {
    let (mut app, ctx, mut time) = opened();
    assert_eq!(app.island.tab(), Tab::Home);
    let at = tab_button(&app, 2);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.island.tab(), Tab::Chats, "the list: the conversations");
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.island.tab(), Tab::Chat, "the bubble: the current chat");
    let at = tab_button(&app, 3);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.island.tab(), Tab::New, "the plus: the new chat");
    let at = tab_button(&app, 0);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.island.tab(), Tab::Home, "the house: back home");
}

#[test]
fn the_sound_button_turns_the_sounds_off_and_on() {
    let (mut app, ctx, mut time) = opened();
    let before = app.prefs.island_sound;
    let at = right_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.prefs.island_sound, !before);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.prefs.island_sound, before);
}

#[test]
fn the_gear_opens_island_settings_and_escape_or_the_gear_returns_home() {
    let (mut app, ctx, mut time) = opened();
    let gear = right_button(&app, 2);
    click(&mut app, &ctx, &mut time, gear);
    assert!(app.island.settings_open(), "the gear opens the island's settings page");

    step(&mut app, &ctx, time, vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    time += 0.05;
    assert!(!app.island.settings_open(), "Escape returns to Home without folding the island");
    assert!((app.island.size().x - EXPANDED_W).abs() < 1.0);

    let gear = right_button(&app, 2);
    click(&mut app, &ctx, &mut time, gear);
    assert!(app.island.settings_open());
    let gear = right_button(&app, 2);
    click(&mut app, &ctx, &mut time, gear);
    assert!(!app.island.settings_open(), "the gear toggles back to Home");
}

#[test]
fn the_close_folds_it_even_with_the_pointer_still_on_it() {
    let (mut app, ctx, mut time) = opened();
    let at = right_button(&app, 0);
    click(&mut app, &ctx, &mut time, at);
    rest(&mut app, &ctx, &mut time, at, 1.5);
    assert!(app.island.size().x <= COMPACT_W, "folded: {:?}", app.island.size());
}

#[test]
fn the_conversation_grows_and_comes_back_by_its_size_button() {
    let (mut app, ctx, mut time) = opened();
    // the whole mini chat (the conversation's ⤢)
    app.island.chat_full = true;
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    // the spring that opens the conversation takes a little over a second to settle
    rest(&mut app, &ctx, &mut time, at, 1.0);
    let small = app.island.size();
    let at = right_button(&app, 4);
    click(&mut app, &ctx, &mut time, at);
    rest(&mut app, &ctx, &mut time, at, 1.0);
    assert!(app.island.chat_big(), "big");
    let big = app.island.size();
    assert!(big.x > small.x && big.y > small.y, "{small:?} → {big:?}");
    // the same button, where it is now on the big island
    let at = right_button(&app, 4);
    click(&mut app, &ctx, &mut time, at);
    rest(&mut app, &ctx, &mut time, at, 1.0);
    assert!(!app.island.chat_big(), "back");
    assert!((app.island.size() - small).length() < 1.0, "the size it had: {small:?}, {:?}", app.island.size());
}

#[test]
fn the_chat_tab_is_the_compact_conversation_and_its_expand_opens_the_whole_chat() {
    let (mut app, ctx, mut time) = opened();
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    rest(&mut app, &ctx, &mut time, at, 1.5);
    assert!(!app.island.chat_full && app.mini.rect.is_none(), "the compact conversation, not the mini chat");
    assert_eq!(app.island.size(), Vec2::new(EXPANDED_W, 240.0), "240 tall with nothing said yet");
    // its ⤢, at the card's top right (the page is 10 in from the sides and 42 down)
    let rect = island_rect(&app);
    let expand = Pos2::new(rect.right() - 10.0 - 18.0, rect.top() + 42.0 + 18.0);
    click(&mut app, &ctx, &mut time, expand);
    rest(&mut app, &ctx, &mut time, expand, 1.5);
    assert!(app.island.chat_full && app.mini.rect.is_some(), "the whole mini chat");
    assert!(app.island.size().y > 300.0, "at its own size: {:?}", app.island.size());
}

/// The window has room for the island as it is drawn: nothing of it is cut.
fn window_holds(app: &DrawingApp) -> bool {
    let (window, island) = (app.island.window(), app.island.size());
    window.x >= island.x && window.y >= island.y
}

#[test]
fn pulled_by_its_bottom_edge_the_conversation_grows_and_its_window_holds_it_all_the_way() {
    let (mut app, ctx, mut time) = opened();
    // the whole mini chat (the conversation's ⤢)
    app.island.chat_full = true;
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    let before = app.island.size();
    let grip = Pos2::new(island_rect(&app).center().x, island_rect(&app).bottom() - 4.0);
    let press = |at, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    for events in [vec![egui::Event::PointerMoved(grip)], vec![egui::Event::PointerMoved(grip)], vec![press(grip, true)]] {
        step(&mut app, &ctx, time, events);
        time += 0.05;
    }
    // down, well past the 420 points the window had: at every frame of the pull the window has room for the island
    let mut at = grip;
    for _ in 0..40 {
        at.y += 9.0;
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(at)]);
        time += 0.05;
        assert!(window_holds(&app), "cut while pulled: window {:?}, island {:?}", app.island.window(), app.island.size());
    }
    // the size asked for in one frame is the island's in the next
    step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(at)]);
    time += 0.05;
    let pulled = app.island.size();
    assert!(pulled.y > before.y + 300.0 && pulled.y > 500.0, "it followed the pointer: {before:?} → {pulled:?}");
    assert_eq!(app.prefs.island_chat, None, "the prefs (and their file) are not written at every frame of the pull");
    step(&mut app, &ctx, time, vec![press(at, false)]);
    let end = time + 1.0;
    while time < end {
        time += 0.05;
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(at)]);
        assert!(window_holds(&app), "cut after the pull: window {:?}, island {:?}", app.island.window(), app.island.size());
    }
    assert!((app.island.size() - pulled).length() < 2.0, "let go, it keeps the size: {pulled:?}, {:?}", app.island.size());
    assert_eq!(app.prefs.island_chat.map(|[_, height]| height), Some(pulled.y.round()), "and the size is kept");
}

#[test]
fn the_short_conversation_is_pulled_wider_by_a_side_and_a_double_click_takes_it_back() {
    let (mut app, ctx, mut time) = opened();
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    assert!(!app.island.chat_full);
    let before = app.island.size();
    let grip = Pos2::new(island_rect(&app).right() - 4.0, island_rect(&app).center().y);
    let press = |at, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    for events in [vec![egui::Event::PointerMoved(grip)], vec![egui::Event::PointerMoved(grip)], vec![press(grip, true)]] {
        step(&mut app, &ctx, time, events);
        time += 0.05;
    }
    let mut at = grip;
    for _ in 0..20 {
        at.x += 5.0;
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(at)]);
        time += 0.05;
    }
    step(&mut app, &ctx, time, vec![press(at, false)]);
    for _ in 0..20 {
        time += 0.05;
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(at)]);
    }
    let pulled = app.island.size();
    assert!(pulled.x > before.x + 150.0, "wider on both sides of its centre: {before:?} → {pulled:?}");
    assert_eq!(app.prefs.island_talk.map(|[width, _]| width), Some(pulled.x.round()), "kept for the short conversation");
    assert_eq!(app.prefs.island_chat, None, "not the whole chat's");
    // a double click on the edge: back to growing with what was said
    let edge = Pos2::new(island_rect(&app).right() - 4.0, island_rect(&app).center().y);
    for pressed in [true, false, true, false] {
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(edge), press(edge, pressed)]);
        time += 0.03;
    }
    for _ in 0..20 {
        time += 0.05;
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(edge)]);
    }
    assert_eq!(app.prefs.island_talk, None);
    assert!((app.island.size().x - before.x).abs() < 4.0, "its size from before: {:?}", app.island.size());
}

#[test]
fn the_window_holds_the_big_conversation_while_it_opens() {
    let (mut app, ctx, mut time) = opened();
    // the whole mini chat (the conversation's ⤢)
    app.island.chat_full = true;
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    let at = right_button(&app, 4);
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    for events in [vec![egui::Event::PointerMoved(at)], vec![press(true)], vec![press(false)]] {
        step(&mut app, &ctx, time, events);
        time += 0.05;
    }
    // 70% of the monitor: every frame of the spring that opens it (it overshoots) fits in the window
    let end = time + 2.0;
    while time < end {
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(at)]);
        time += 0.02;
        assert!(window_holds(&app), "cut: window {:?}, island {:?}", app.island.window(), app.island.size());
    }
    assert!(app.island.chat_big() && (app.island.size() - SIZE * 0.7).length() < 1.0, "{:?}", app.island.size());
}

/// The conversation's own header (chat/mini.rs): ✕ at its left, full screen at its right.
fn chat_header(app: &DrawingApp) -> (Pos2, Pos2) {
    let panel = app.mini.rect.expect("the conversation is drawn");
    let y = panel.top() + 25.0;
    (Pos2::new(panel.left() + 24.0, y), Pos2::new(panel.right() - 24.0, y))
}

#[test]
fn the_conversations_close_folds_the_island() {
    let (mut app, ctx, mut time) = opened();
    // the whole mini chat (the conversation's ⤢)
    app.island.chat_full = true;
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.island.tab(), Tab::Chat);
    let (close, _) = chat_header(&app);
    click(&mut app, &ctx, &mut time, close);
    rest(&mut app, &ctx, &mut time, close, 1.5);
    assert!(app.island.size().x <= COMPACT_W, "the ✕ (Recolher) folded it: {:?}", app.island.size());
    assert_eq!(app.island.tab(), Tab::Home, "and it opens on Home next time");
}

#[test]
fn full_screen_takes_the_chat_to_the_app_and_folds_the_island() {
    let (mut app, ctx, mut time) = opened();
    // the whole mini chat (the conversation's ⤢)
    app.island.chat_full = true;
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    let shown = app.chat_agent.clone().expect("the conversation tab picks a chat");
    let (_, full) = chat_header(&app);
    click(&mut app, &ctx, &mut time, full);
    rest(&mut app, &ctx, &mut time, full, 1.5);
    assert!(app.island.size().x <= COMPACT_W, "folded: {:?}", app.island.size());
    assert_eq!(app.chat_agent.as_deref(), Some(shown.as_str()), "the same chat, now in the app");
    assert!(app.chat_open, "open in the app chat screen");
}

#[test]
fn the_pointer_resting_on_it_keeps_it_open_and_steady() {
    let (mut app, ctx, mut time) = opened();
    let middle = island_rect(&app).center();
    let end = time + 10.0;
    while time < end {
        step(&mut app, &ctx, time, vec![egui::Event::PointerMoved(middle)]);
        time += 0.05;
        assert!((app.island.size().x - EXPANDED_W).abs() < 0.01, "it never folds nor shrinks under the pointer (t = {time:.2}): {}", app.island.size().x);
    }
}

#[test]
fn the_pointer_over_a_pill_only_lights_it_and_its_click_puts_its_agent_in_focus() {
    let (mut app, ctx, mut time) = opened();
    let at = tab_button(&app, 2);
    click(&mut app, &ctx, &mut time, at);
    // the right card of Chats holds the agents' pills (two columns, 8 in from its sides, the row in its middle): "b" is the second
    let rect = island_rect(&app);
    let card = Rect::from_min_max(Pos2::new(rect.left() + 10.0 + 332.0, rect.top() + 42.0), rect.max - Vec2::new(10.0, 10.0));
    let inner = card.shrink2(Vec2::new(8.0, 0.0));
    let width = (inner.width() - 4.0) / 2.0;
    let second = Pos2::new(inner.left() + width + 4.0 + width / 2.0, inner.center().y);
    let asked = app.island.chimes();
    // Coucou's mouseenter only lights the pill: the card at the left stays, so nothing slides from under the pointer
    rest(&mut app, &ctx, &mut time, second, 2.0);
    assert_eq!(app.island.focus(), None, "the pointer over a pill changes no focus");
    assert_eq!(app.island.chimes(), asked, "and makes no sound");
    // its click is Coucou's setFocus: the agent in the card at the left, a blip, still on Chats
    click(&mut app, &ctx, &mut time, second);
    assert_eq!(app.island.focus(), Some("b"));
    assert!(app.island.chimes() > asked, "the click has its blip");
    assert_eq!((app.island.tab(), app.island.size().x), (Tab::Chats, EXPANDED_W));
    // the focus stays where it was put, wherever the pointer goes
    let first = Pos2::new(inner.left() + width / 2.0, inner.center().y);
    rest(&mut app, &ctx, &mut time, first, 1.0);
    assert_eq!(app.island.focus(), Some("b"));
}

#[test]
fn abrir_em_miniatura_opens_the_conversation_in_the_island() {
    let (mut app, ctx, time) = opened();
    app.island.tab = Tab::Home;
    app.open_mini_chat("b");
    step(&mut app, &ctx, time + 0.05, vec![]);
    assert_eq!((app.island.tab(), app.chat_agent.as_deref(), app.mini.open), (Tab::Chat, Some("b"), false));
}

#[test]
fn closed_it_is_the_notch_again_after_the_pointer_leaves() {
    let (mut app, ctx, mut time) = opened();
    step(&mut app, &ctx, time, vec![egui::Event::PointerGone]);
    time += 0.05;
    let end = time + 80.0;
    while time < end {
        step(&mut app, &ctx, time, vec![]);
        time += 0.25;
    }
    assert_eq!(app.island.size().x, CLOSED_W);
}

#[test]
fn inicio_holds_only_the_tools_and_chats_only_the_agents() {
    let (mut app, ctx, mut time) = opened();
    app.prefs.integrations.spotify = true;
    let track = crate::app::integrations::spotify::Track::new("Música", "Banda", "Disco", "").unwrap();
    app.integrations.spotify.now = Some(crate::app::integrations::spotify::Now { track, playing: true, ..Default::default() });
    // the first pill of the grid: the right card, 8 in from its sides, its one row in the middle
    let rect = island_rect(&app);
    let card = Rect::from_min_max(Pos2::new(rect.left() + 10.0 + 332.0, rect.top() + 42.0), rect.max - Vec2::new(10.0, 10.0));
    let inner = card.shrink2(Vec2::new(8.0, 0.0));
    let first = Pos2::new(inner.left() + (inner.width() - 4.0) / 4.0, inner.center().y);
    rest(&mut app, &ctx, &mut time, first, 0.3);
    click(&mut app, &ctx, &mut time, first);
    assert_eq!((app.island.tab(), app.island.focus()), (Tab::Home, Some("integration:spotify")), "Início: the tool, not the chats");
    let at = tab_button(&app, 2);
    click(&mut app, &ctx, &mut time, at);
    click(&mut app, &ctx, &mut time, first);
    assert_eq!((app.island.tab(), app.island.focus()), (Tab::Chats, Some("a")), "Chats: the agents, not the tool");
}

/// GitHub's alerts sound on the island as the reference's: checks that passed, a review asked for (a failure is its red wizard's error).
#[test]
fn checks_that_passed_and_a_review_asked_for_sound_on_the_island() {
    use crate::app::integrations::github::{CiState, GitHub, PullRequest, Pulse};
    let (mut app, ctx, mut time) = opened();
    app.prefs.integrations.github = true;
    app.integrations.github = GitHub::with_token("ghp_0123456789abcdefghijABCDEFGHIJ012345");
    let request = |id: &str, ci| PullRequest {
        id: id.into(),
        title: "T".into(),
        url: "https://github.com/a/b/pull/1".into(),
        ci,
        head: Some("s1".into()),
        ..Default::default()
    };
    let pulse = |ci, to_review: Vec<PullRequest>| Pulse { login: "ana".into(), mine: vec![request("a/b#1", ci)], to_review, ..Default::default() };
    app.integrations.github.take_pulse(pulse(CiState::Pending, vec![]), now_ms());
    for (next, what) in [(pulse(CiState::Success, vec![]), "passed"), (pulse(CiState::Success, vec![request("c/d#2", CiState::Unknown)]), "review")] {
        let before = app.island.chimes();
        app.integrations.github.take_pulse(next, now_ms());
        step(&mut app, &ctx, time, vec![]);
        time += 0.05;
        assert!(app.island.chimes() > before, "{what}: a sound");
        assert_eq!(app.integrations.github.ring, None, "{what}: sounded once");
    }
}
