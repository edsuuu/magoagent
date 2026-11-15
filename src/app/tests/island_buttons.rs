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
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    assert_eq!(app.island.tab(), Tab::Chat, "the bubble: the conversation");
    let at = tab_button(&app, 2);
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

/// The window has room for the island as it is drawn: nothing of it is cut.
fn window_holds(app: &DrawingApp) -> bool {
    let (window, island) = (app.island.window(), app.island.size());
    window.x >= island.x && window.y >= island.y
}

#[test]
fn pulled_by_its_bottom_edge_the_conversation_grows_and_its_window_holds_it_all_the_way() {
    let (mut app, ctx, mut time) = opened();
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
fn the_window_holds_the_big_conversation_while_it_opens() {
    let (mut app, ctx, mut time) = opened();
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
fn the_pointer_over_a_pill_makes_no_sound_and_its_click_does() {
    let (mut app, ctx, mut time) = opened();
    // the right card of Home holds the pills: sweep it until one answers the pointer
    let rect = island_rect(&app);
    let card = Rect::from_min_max(Pos2::new(rect.left() + 10.0 + 332.0, rect.top() + 42.0), rect.max - Vec2::new(10.0, 10.0));
    let asked = app.island.chimes();
    let mut found = None;
    let points = (0..12).flat_map(|row| (0..30).map(move |column| (row, column)));
    for (row, column) in points {
        let at = Pos2::new(card.left() + 4.0 + column as f32 * 10.0, card.top() + 4.0 + row as f32 * 8.0);
        rest(&mut app, &ctx, &mut time, at, 0.1);
        if app.island.focus().is_some() {
            found = Some(at);
            break;
        }
    }
    let at = found.expect("a pill under the pointer");
    rest(&mut app, &ctx, &mut time, at, 2.0);
    assert_eq!(app.island.chimes(), asked, "coming to a pill and resting on it: no sound");
    assert_eq!(app.island.size().x, EXPANDED_W);
    click(&mut app, &ctx, &mut time, at);
    assert!(app.island.chimes() > asked, "the click has its blip");
    assert_eq!(app.island.tab(), Tab::Chat, "and opens the chat in the conversation");
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
