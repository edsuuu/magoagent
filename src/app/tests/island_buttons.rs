//! The island's buttons one by one, by the pointer as the person uses them: the tabs, the sound, the close, the
//! conversation's size, its ✕ and full screen, Esc, and the pointer resting on it (it stays open, steady, the hover of a
//! pill silent).

use super::*;
use crate::app::island::{CLOSED_W, COMPACT_W, EXPANDED_W, Tab};

/// The app, its island opened by clicking on the compact pill, and the time it is at.
fn opened() -> (DrawingApp, Context, f64) {
    island_opened(vec![chat("a", "claude", "Claude Code · loja"), chat("b", "claude", "Codex · site")])
}

fn step(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) {
    island_frame(app, ctx, time, events);
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
    assert_eq!(app.island.tab(), Tab::Claude, "Claude Code's spark: the sessions outside the app");
    let at = tab_button(&app, 4);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.island.tab(), Tab::New, "the plus: the new chat (GitHub and Spotify off: no tab of theirs)");
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
    // with the settings page open too: its own switch must not take the click away
    let gear = right_button(&app, 2);
    click(&mut app, &ctx, &mut time, gear);
    assert!(app.island.settings_open());
    let at = right_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.prefs.island_sound, !before);
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
    let at = right_button(&app, BIG_BUTTON);
    click(&mut app, &ctx, &mut time, at);
    rest(&mut app, &ctx, &mut time, at, 1.0);
    assert!(app.island.chat_big(), "big");
    let big = app.island.size();
    assert!(big.x > small.x && big.y > small.y, "{small:?} → {big:?}");
    // the same button, where it is now on the big island
    let at = right_button(&app, BIG_BUTTON);
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
    let at = right_button(&app, BIG_BUTTON);
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
    assert!(app.island.chat_big() && (app.island.size() - ISLAND_SCREEN * 0.7).length() < 1.0, "{:?}", app.island.size());
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
    // the pointer only lights the pill: the card at the left stays, so nothing slides from under the pointer
    rest(&mut app, &ctx, &mut time, second, 2.0);
    assert_eq!(app.island.focus(), None, "the pointer over a pill changes no focus");
    assert_eq!(app.island.chimes(), asked, "and makes no sound");
    // its click focuses it: the agent in the card at the left, a blip, still on Chats
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

/// The integrations and the sessions outside each on a tab of their own (Claude Code, GitHub, Spotify).
#[test]
fn spotify_has_its_tab_and_chats_only_the_agents() {
    let (mut app, ctx, mut time) = opened();
    app.prefs.integrations.spotify = true;
    let track = crate::app::integrations::spotify::Track::new("Música", "Banda", "Disco", "").unwrap();
    app.integrations.spotify.now = Some(crate::app::integrations::spotify::Now { track, playing: true, ..Default::default() });
    assert_eq!(app.island.tab(), Tab::Home);
    // Home, Chat, Chats, Claude Code, Spotify (GitHub is off), +
    let at = tab_button(&app, 4);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.island.tab(), Tab::Music, "Spotify's tab: the player, not the chats");
    let at = tab_button(&app, 2);
    click(&mut app, &ctx, &mut time, at);
    // the first pill of the grid: the right card, 8 in from its sides, its one row in the middle
    let rect = island_rect(&app);
    let card = Rect::from_min_max(Pos2::new(rect.left() + 10.0 + 332.0, rect.top() + 42.0), rect.max - Vec2::new(10.0, 10.0));
    let inner = card.shrink2(Vec2::new(8.0, 0.0));
    let first = Pos2::new(inner.left() + (inner.width() - 4.0) / 4.0, inner.center().y);
    rest(&mut app, &ctx, &mut time, first, 0.3);
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

/// The GitHub tab: taller, it reads the first pull request when it opens and the one a click picks.
#[test]
fn the_github_tab_reads_the_pull_request_picked() {
    use crate::app::integrations::github::{GitHub, Pulse};
    let (mut app, ctx, mut time) = opened();
    app.prefs.integrations.github = true;
    app.integrations.github = GitHub::with_token("ghp_0123456789abcdefghijABCDEFGHIJ012345");
    let recorded: Value = serde_json::from_str(include_str!("../integrations/fixtures/github_pulse.json")).unwrap();
    app.integrations.github.pulse = Pulse::parse(&recorded);
    app.island_show_page(Tab::GitHub);
    let inside = island_rect(&app).center();
    rest(&mut app, &ctx, &mut time, inside, 1.0);
    assert_eq!(app.island.tab(), Tab::GitHub);
    assert!((app.island.size().y - 404.0).abs() < 2.0, "room for the list and the description: {}", app.island.size().y);
    assert!(app.github_detail_loading("ana-dev/loja#42"), "the last changed shows first, read once");
    assert!(!app.github_detail_loading("time/api#310"));
    // the second row: under the filters, a row and its gap down
    let rect = island_rect(&app);
    click(&mut app, &ctx, &mut time, Pos2::new(rect.left() + 80.0, rect.top() + 82.0 + 40.0 + 19.0));
    assert!(app.github_detail_loading("time/api#310"), "the one clicked is read");
    assert_eq!(app.island.tab(), Tab::GitHub, "a row picks, the tab stays");
}

/// The GitHub tab: the app opened on it with the recorded pulse, and the texts of a frame with where they are.
fn github_tab() -> (DrawingApp, Context, f64) {
    use crate::app::integrations::github::{BranchCi, CiState, GitHub, Pulse};
    let (mut app, ctx, mut time) = opened();
    app.prefs.integrations.github = true;
    app.integrations.github = GitHub::with_token("ghp_0123456789abcdefghijABCDEFGHIJ012345");
    let recorded: Value = serde_json::from_str(include_str!("../integrations/fixtures/github_pulse.json")).unwrap();
    let mut pulse = Pulse::parse(&recorded).unwrap();
    pulse.main =
        vec![BranchCi { repo: "ana-dev/loja".into(), url: "https://github.com/ana-dev/loja".into(), branch: "main".into(), ci: CiState::Failure, head: None }];
    app.integrations.github.pulse = Some(pulse);
    app.island_show_page(Tab::GitHub);
    let inside = island_rect(&app).center();
    rest(&mut app, &ctx, &mut time, inside, 1.0);
    (app, ctx, time)
}

/// Where `text` was painted in the last frame.
fn painted(app: &mut DrawingApp, ctx: &Context, time: f64, text: &str) -> Rect {
    let texts = island_frame(app, ctx, time, vec![]);
    texts.iter().find(|(painted, _)| painted == text).map(|(_, rect)| *rect).unwrap_or_else(|| panic!("{text:?} not painted: {texts:?}"))
}

/// A default branch under the list opens its repository's Actions, where the failed run is.
#[test]
fn the_github_tab_opens_a_main_branchs_actions() {
    let (mut app, ctx, mut time) = github_tab();
    let at = painted(&mut app, &ctx, time, "loja falhou").center();
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.integrations.pressed.borrow().as_slice(), ["open https://github.com/ana-dev/loja/actions"]);
}

/// The last week's squares under the list open Atividade: the calendar, the day under the pointer in words, the profile.
#[test]
fn the_github_tab_shows_the_contribution_calendar() {
    use crate::app::integrations::github::Activity;
    let (mut app, ctx, mut time) = github_tab();
    let recorded: Value = serde_json::from_str(include_str!("../integrations/fixtures/github_activity.json")).unwrap();
    app.integrations.github.activity = Activity::parse(&recorded);
    // the seven squares end the footer's line, at the left card's right
    let footer = painted(&mut app, &ctx, time, "loja falhou");
    let left_right = island_rect(&app).left() + 10.0 + ((island_rect(&app).width() - 20.0 - 10.0) * 1.22 / 2.22).round();
    let spark = Pos2::new(left_right - 12.0 - 8.0 - 30.0, footer.center().y);
    click(&mut app, &ctx, &mut time, spark);
    let summary = painted(&mut app, &ctx, time, "1.412 contribuições no último ano · 12 repositórios · ★ 1,3 mil");
    assert!(summary.left() > left_right, "at the right, instead of the pull request");
    painted(&mut app, &ctx, time, "Menos");
    // 16 de nov.: the second line of the last week, the calendar's last column
    let grid_left = summary.left();
    let grid_top = summary.top() + 42.0 - 2.0;
    let at = Pos2::new(grid_left + 13.0 + 5.0, grid_top + 13.0 + 5.0);
    rest(&mut app, &ctx, &mut time, at, 0.2);
    painted(&mut app, &ctx, time, "16 de nov. · 22 contribuições");
    let profile = painted(&mut app, &ctx, time, "Abrir o perfil").center();
    click(&mut app, &ctx, &mut time, profile);
    assert_eq!(app.integrations.pressed.borrow().last().map(String::as_str), Some("open https://github.com/ana-dev"));
    let back = painted(&mut app, &ctx, time, "Voltar").center();
    click(&mut app, &ctx, &mut time, back);
    painted(&mut app, &ctx, time, "Abrir no navegador");
}

/// Ctrl+↓ and Ctrl+↑ walk the GitHub tab's list, and Ctrl+O opens the one shown.
#[test]
fn the_github_tab_walks_its_list_by_the_keyboard() {
    let (mut app, ctx, mut time) = github_tab();
    let key = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
    step(&mut app, &ctx, time, vec![key(egui::Key::ArrowDown, Modifiers::CTRL)]);
    time += 0.05;
    step(&mut app, &ctx, time, vec![]);
    assert!(app.github_detail_loading("time/api#310"), "the second row is picked and read");
    step(&mut app, &ctx, time + 0.05, vec![key(egui::Key::ArrowUp, Modifiers::CTRL), key(egui::Key::ArrowUp, Modifiers::CTRL)]);
    step(&mut app, &ctx, time + 0.1, vec![key(egui::Key::O, Modifiers::CTRL)]);
    assert_eq!(app.integrations.pressed.borrow().as_slice(), ["open https://github.com/ana-dev/loja/pull/42"], "back at the first, never before it");
}

/// Mesclar and Aprovar ask before they go: Cancelar sends nothing, Confirmar sends it.
#[test]
fn the_github_tab_asks_before_it_merges_or_approves() {
    use crate::app::integrations::github::PrDetail;
    let (mut app, ctx, mut time) = github_tab();
    // the person's own, ready: Mesclar, and no Aprovar
    let request = app.integrations.github.pulse.as_ref().unwrap().mine[0].clone();
    app.integrations.github.answered(PrDetail { request, merge_method: "SQUASH", ..Default::default() });
    let texts: Vec<String> = island_frame(&mut app, &ctx, time, vec![]).into_iter().map(|(text, _)| text).collect();
    assert!(!texts.iter().any(|text| text == "Aprovar"), "never one's own: {texts:?}");
    let merge = painted(&mut app, &ctx, time, "Mesclar").center();
    click(&mut app, &ctx, &mut time, merge);
    painted(&mut app, &ctx, time, "Mesclar ana-dev/loja#42 na main em um commit só (squash)?");
    assert!(!app.github_writing(), "asked, not sent");
    let cancel = painted(&mut app, &ctx, time, "Cancelar").center();
    click(&mut app, &ctx, &mut time, cancel);
    painted(&mut app, &ctx, time, "Abrir no navegador");
    assert!(!app.github_writing() && app.github_last_write().is_none());
    click(&mut app, &ctx, &mut time, merge);
    let confirm = painted(&mut app, &ctx, time, "Confirmar").center();
    click(&mut app, &ctx, &mut time, confirm);
    assert!(app.github_writing(), "confirmed: on its way");
    // someone else's, asking for the person's review: Aprovar
    let key = egui::Event::Key { key: egui::Key::ArrowDown, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::CTRL };
    step(&mut app, &ctx, time, vec![key]);
    time += 0.05;
    let approve = painted(&mut app, &ctx, time, "Aprovar").center();
    click(&mut app, &ctx, &mut time, approve);
    painted(&mut app, &ctx, time, "Aprovar time/api#310?");
}

/// The Spotify tab: its height, aleatório pressed, and a song of the queue played at once, not skipped to.
#[test]
fn the_spotify_tab_plays_what_is_pressed() {
    use crate::app::integrations::spotify::{Command, Now, Playback, Player, QueueItem, Track};
    let (mut app, ctx, mut time) = opened();
    // Soloist only counts on Linux: the Spotify switch keeps the integration on elsewhere
    (app.prefs.integrations.soloist, app.prefs.integrations.spotify) = (true, true);
    let playback = Playback { shuffle: Some(false), position_ms: Some(10_000), duration_ms: Some(200_000), ..Default::default() };
    app.integrations.spotify.now =
        Some(Now { track: Track::new("Canção", "Banda", "Disco", "").unwrap(), playing: true, player: Player::Soloist, playback, ..Default::default() });
    let song = |index: u32| QueueItem::new(&format!("spotify:track:{index}"), &format!("Música {index}"), "Banda", "", Some(180_000)).unwrap();
    app.integrations.spotify.library.queue = vec![song(1), song(2), song(3)];
    app.island_show_page(Tab::Music);
    let inside = island_rect(&app).center();
    rest(&mut app, &ctx, &mut time, inside, 1.0);
    assert_eq!(app.island.tab(), Tab::Music);
    assert!((app.island.size().y - 318.0).abs() < 2.0, "the player's room: {}", app.island.size().y);
    let page = Rect::from_min_max(island_rect(&app).min + Vec2::new(10.0, 42.0), island_rect(&app).max - Vec2::new(10.0, 10.0));
    // the buttons' row: five in the player's card, aleatório the first and tocar in the middle
    let (left, bottom) = (page.left() + 16.0, page.bottom() - 12.0);
    click(&mut app, &ctx, &mut time, Pos2::new(left + 286.0 / 10.0, bottom - 48.0));
    assert_eq!(app.integrations.spotify.asked, [Command::Shuffle(true)]);
    // the second song of A seguir: played at once (without the account, by the player)
    let list_x = page.left() + 318.0 + 10.0 + 120.0;
    click(&mut app, &ctx, &mut time, Pos2::new(list_x, page.top() + 80.0 + 36.0 + 1.0 + 18.0));
    assert_eq!(app.integrations.spotify.asked, [Command::Shuffle(true), Command::PlayUri("spotify:track:2".into())]);
}

/// Spotify turned off in Ajustes: its tab leaves the header and the open page goes back to Início.
#[test]
fn turning_spotify_off_takes_its_tab_away() {
    let (mut app, ctx, mut time) = opened();
    app.prefs.integrations.spotify = true;
    app.island_show_page(Tab::Music);
    let inside = island_rect(&app).center();
    rest(&mut app, &ctx, &mut time, inside, 0.5);
    assert_eq!(app.island.tab(), Tab::Music);
    app.prefs.integrations.spotify = false;
    rest(&mut app, &ctx, &mut time, inside, 0.5);
    assert_eq!(app.island.tab(), Tab::Home, "the page of a music that is off");
    let lines: Vec<_> = app.integration_items().into_iter().map(|item| item.id).collect();
    assert!(!lines.iter().any(|id| id.ends_with("spotify")), "no pill, so no tab: {lines:?}");
}

/// In the island's window (here the screen is as big as it), the model chip's menu opens over the chip inside the
/// island; Mais modelos opens its list in it (egui's menu closed at that click), the menu still in the island, and a
/// model picked is the chat's.
#[test]
fn the_model_menu_stays_in_the_island_and_opens_more_models() {
    let (mut app, ctx, mut time) = opened();
    app.island.chat_full = true;
    app.chat_agent = Some("a".into());
    // the chip's model and effort the chat's own: with none, they come from this computer's ~/.claude/settings.json
    // (`footer::agent_default`), which the CI's machines do not have
    if let Some(chat) = app.chats.iter_mut().find(|chat| chat.id == "a") {
        (chat.model, chat.effort) = ("claude-opus-5-5".into(), "xhigh".into());
    }
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    rest(&mut app, &ctx, &mut time, at, 1.5);
    let screen = app.island.window();
    let at = at - Vec2::new(ISLAND_SCREEN.x / 2.0 - screen.x / 2.0, 0.0);
    let mut frame = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)), time: Some(time), events, ..Default::default() };
        time += 0.05;
        let mut output = ctx.run_ui(raw, |_| app.island_ui(&ctx));
        output.textures_delta.clear();
        let texts: Vec<(String, Rect)> = output
            .shapes
            .iter()
            .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.visual_bounding_rect())) } else { None })
            .collect();
        texts
    };
    let press = |at: Pos2, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let mut tap = |app: &mut DrawingApp, at: Pos2| {
        for events in [vec![egui::Event::PointerMoved(at)], vec![egui::Event::PointerMoved(at)], vec![press(at, true)], vec![press(at, false)], vec![], vec![]]
        {
            frame(app, events);
        }
        frame(app, vec![egui::Event::PointerMoved(at)])
    };
    let texts = tap(&mut app, at);
    let chip = texts.iter().find(|(text, _)| text.contains("Opus 5.5 ·")).map(|(_, rect)| rect.center()).expect("the model chip");
    let texts = tap(&mut app, chip);
    let more = texts.iter().find(|(text, _)| text == "Mais modelos").map(|(_, rect)| rect.center()).expect("the model menu, open");
    let island = Rect::from_min_size(Pos2::new((screen.x - app.island.size().x) / 2.0, 0.0), app.island.size());
    let menu = || {
        ctx.memory(|memory| {
            memory
                .areas()
                .visible_layer_ids()
                .into_iter()
                .filter_map(|layer| memory.area_rect(layer.id))
                .filter(|rect| rect.width() > 100.0 && rect.width() < 300.0)
                .collect::<Vec<_>>()
        })
    };
    let texts = tap(&mut app, more);
    assert!(texts.iter().any(|(text, _)| text == "Opus 4.8"), "Mais modelos opened its list: {texts:?}");
    let shown = menu();
    assert!(
        !shown.is_empty() && shown.iter().all(|rect| island.contains_rect(*rect) && rect.bottom() < chip.y),
        "over the chip, in the island: {shown:?} {island:?}"
    );
    let sonnet = texts.iter().find(|(text, _)| text == "Sonnet 5.5").map(|(_, rect)| rect.center()).unwrap();
    tap(&mut app, sonnet);
    assert_eq!(app.chats[0].model, "claude-sonnet-5-5");
    assert!(menu().is_empty() && app.island.chat_full, "the pick closed the menu, the island stays");
}

/// With the call and Ditar on hold (`voice::CALL_ON`, `voice::DICTATION_ON`) the island's header has no Chat por
/// voz, and neither the island nor the MCP starts them.
#[test]
fn the_island_and_the_chat_have_no_voice_nor_dictation_while_they_are_on_hold() {
    if crate::app::voice::CALL_ON || crate::app::voice::DICTATION_ON {
        return;
    }
    let (mut app, ctx, mut time) = opened();
    // where Chat por voz was, left of Ajustes
    let voice = right_button(&app, 3);
    click(&mut app, &ctx, &mut time, voice);
    assert!(!app.voice.on, "no call from where its button was");
    app.dictate_toggle();
    assert!(app.dictation.is_none(), "Ditar starts nothing");
    assert_eq!(app.act("voice_call", &json!({ "state": "dictate_on" })), Err("O ditado está desligado por enquanto.".into()));
    assert_eq!(app.act("voice_call", &json!({ "state": "on" })), Err("O chat por voz está desligado por enquanto.".into()));
}
