//! The island's buttons one by one, by the pointer as the person uses them: the tabs, the conversation picker,
//! its composer, the sound, the close, Esc and steady hover states.

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

/// A header button, counted from the right: 0 the close, 1 the sound, 2 the settings, 3 the voice.
fn right_button(app: &DrawingApp, index: usize) -> Pos2 {
    let rect = island_rect(app);
    Pos2::new(rect.right() - 25.0 - index as f32 * 32.0, rect.top() + 25.0)
}

/// A tab: 0 Home, 1 the conversation, 2 the new chat.
fn tab_button(app: &DrawingApp, index: usize) -> Pos2 {
    let rect = island_rect(app);
    Pos2::new(rect.left() + 29.0 + index as f32 * 35.0, rect.top() + 25.0)
}

/// The first row in the conversation picker.
fn first_chat(app: &DrawingApp) -> Pos2 {
    let rect = island_rect(app);
    Pos2::new(rect.center().x, rect.top() + 78.0)
}

fn open_first_conversation(app: &mut DrawingApp, ctx: &Context, time: &mut f64) {
    let at = tab_button(app, 1);
    click(app, ctx, time, at);
    let at = first_chat(app);
    click(app, ctx, time, at);
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
fn the_conversation_tab_opens_a_picker_selects_another_chat_and_the_back_arrow_returns_to_the_list() {
    let (mut app, ctx, mut time) = opened();
    let at = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, at);
    assert!(app.island.chat_list(), "starts at the conversation list");
    assert_eq!(app.chat_agent.as_deref(), Some("a"), "the first chat is selected for initial display");
    let row = first_chat(&app);
    click(&mut app, &ctx, &mut time, row + Vec2::new(0.0, 38.0));
    assert_eq!(app.chat_agent.as_deref(), Some("b"), "the selected row chooses that conversation");
    assert!(!app.island.chat_list(), "selecting a row opens the conversation");
    click(&mut app, &ctx, &mut time, at);
    assert!(app.island.chat_list(), "the back arrow returns to the list");
    let row = first_chat(&app);
    click(&mut app, &ctx, &mut time, row + Vec2::new(0.0, 38.0));
    assert!(!app.island.chat_list(), "selecting again returns to the conversation");
}

#[test]
fn sending_a_chat_command_from_the_island_uses_the_selected_conversation() {
    let (mut app, ctx, _time) = opened();
    app.chat_agent = Some("b".into());
    app.chat_transcript("a");
    app.chat_transcript("b");
    let selected = app.chat_agent.clone().unwrap();
    app.send_from_box(&ctx, &selected, "/help".into());
    let items = &app.chat_transcript("b").items;
    assert!(items.iter().any(|item| matches!(item, crate::transcript::ChatItem::System(text) if text.contains("Comandos do chat"))));
    assert!(!app.chat_transcript("a").items.iter().any(|item| matches!(item, crate::transcript::ChatItem::System(_))));
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
fn the_close_folds_it_even_with_the_pointer_still_on_it() {
    let (mut app, ctx, mut time) = opened();
    let at = right_button(&app, 0);
    click(&mut app, &ctx, &mut time, at);
    rest(&mut app, &ctx, &mut time, at, 1.5);
    assert!(app.island.size().x <= COMPACT_W, "folded: {:?}", app.island.size());
}

#[test]
fn the_conversation_stays_the_same_size_while_the_pointer_hovers_over_its_items() {
    let (mut app, ctx, mut time) = opened();
    open_first_conversation(&mut app, &ctx, &mut time);
    let position = island_rect(&app).center();
    rest(&mut app, &ctx, &mut time, position, 1.0);
    let initial = app.island.size();
    rest(&mut app, &ctx, &mut time, position, 3.0);
    assert!((app.island.size() - initial).length() < 0.01, "hover never changes island size: {initial:?} → {:?}", app.island.size());
    assert_eq!(initial.y, 240.0, "the empty conversation follows the reference height");
}

#[test]
fn the_pointer_resting_on_a_chat_row_keeps_the_island_size_constant_for_three_seconds() {
    let (mut app, ctx, mut time) = opened();
    let tab = tab_button(&app, 1);
    click(&mut app, &ctx, &mut time, tab);
    let row = first_chat(&app);
    rest(&mut app, &ctx, &mut time, row, 1.0);
    let initial = app.island.size();
    rest(&mut app, &ctx, &mut time, row, 3.0);
    assert!((app.island.size() - initial).length() < 0.01, "hover changes only row color, never geometry: {initial:?} → {:?}", app.island.size());
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
