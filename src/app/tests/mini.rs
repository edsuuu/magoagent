//! The mini chat: the pill over the canvas after leaving a conversation, the panel, its drag.

use super::*;
use crate::app::chat::mini::{mini_place, mini_size};
use crate::transcript::{ChatItem, Transcript};

const SIZE: Vec2 = Vec2::new(1200.0, 800.0);

fn app_with_chat() -> DrawingApp {
    let mut app = DrawingApp::default();
    app.canvas_rect = Rect::from_min_size(Pos2::ZERO, SIZE);
    app.chats = vec![crate::app::chat::direct::Chat {
        id: "a".into(),
        agent: "claude".into(),
        name: "Claude Code · loja".into(),
        cwd: "C:/p".into(),
        session: None,
        mode: 0,
        model: String::new(),
        effort: String::new(),
        recruit: false,
    }];
    let mut t = Transcript::new(PathBuf::new());
    t.items = vec![ChatItem::User("oi".into()), ChatItem::Agent("olá".into())];
    app.transcripts.insert("a".into(), t);
    app.chat_agent = Some("a".into());
    app
}

fn frame(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.mini_chat_ui(ctx));
    output.textures_delta.clear();
}

fn press(app: &mut DrawingApp, ctx: &Context, at: Pos2) {
    frame(app, ctx, vec![egui::Event::PointerMoved(at)]);
    frame(app, ctx, vec![button(at, true)]);
    frame(app, ctx, vec![button(at, false)]);
    frame(app, ctx, vec![]);
}

/// Leaves the big chat the way a person does: it was open, now it isn't.
fn leave_chat(app: &mut DrawingApp, ctx: &Context) {
    app.chat_open = true;
    frame(app, ctx, vec![]);
    app.chat_open = false;
    frame(app, ctx, vec![]);
}

#[test]
fn leaving_a_conversation_leaves_the_pill_and_a_click_opens_the_panel() {
    let ctx = Context::default();
    let mut app = app_with_chat();
    frame(&mut app, &ctx, vec![]);
    assert!(!app.mini.on, "nothing left yet");
    leave_chat(&mut app, &ctx);
    assert!(app.mini.on && !app.mini.open && app.mini.rect.is_none(), "the pill, no panel");
    // the pill hangs from the top right, 12 in and 56 down
    press(&mut app, &ctx, Pos2::new(SIZE.x - 12.0 - 60.0, 56.0 + 20.0 + TITLE_BAR));
    assert!(app.mini.open && app.mini.rect.is_some());
    // an empty chat leaves no pill
    let mut empty = app_with_chat();
    empty.transcripts.clear();
    leave_chat(&mut empty, &ctx);
    assert!(!empty.mini.on);
}

#[test]
fn the_panel_expands_to_the_big_chat_and_folds_back_to_the_pill() {
    let ctx = Context::default();
    let mut app = app_with_chat();
    leave_chat(&mut app, &ctx);
    app.mini_set(true);
    frame(&mut app, &ctx, vec![]);
    let rect = app.mini.rect.unwrap();
    assert_eq!(rect.size(), Vec2::new(340.0, 600.0));
    // the × at the left folds it
    press(&mut app, &ctx, Pos2::new(rect.left() + 24.0, rect.top() + 25.0));
    assert!(app.mini.on && !app.mini.open && !app.chat_open);
    // Esc in the box folds it too
    app.mini_set(true);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    assert!(!app.mini.open, "Esc folds the panel");
    // the expand button at the right opens the big chat
    app.mini_set(true);
    frame(&mut app, &ctx, vec![]);
    let rect = app.mini.rect.unwrap();
    press(&mut app, &ctx, Pos2::new(rect.right() - 24.0, rect.top() + 25.0));
    assert!(app.chat_open && !app.mini.open && app.chat_agent.as_deref() == Some("a"));
}

#[test]
fn the_header_drags_the_panel_but_never_out_of_the_window() {
    let area = Rect::from_min_size(Pos2::ZERO, SIZE);
    let size = mini_size(area);
    for at in [Vec2::new(-900.0, -900.0), Vec2::new(9000.0, 9000.0), Vec2::new(300.0, 100.0)] {
        let r = mini_place(area, size, Some(at));
        assert!(area.contains_rect(r), "{r:?} out of {area:?}");
    }
    // small windows shrink the panel: 24 off the width, 68 off the height
    assert_eq!(mini_size(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 400.0))), Vec2::new(276.0, 332.0));
    let ctx = Context::default();
    let mut app = app_with_chat();
    leave_chat(&mut app, &ctx);
    app.mini_set(true);
    frame(&mut app, &ctx, vec![]);
    let from = app.mini.rect.unwrap();
    let grab = Pos2::new(from.center().x, from.top() + 25.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(grab)]);
    frame(&mut app, &ctx, vec![button(grab, true)]);
    let to = Pos2::new(-400.0, 5000.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(grab.x - 100.0, grab.y + 100.0))]);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
    frame(&mut app, &ctx, vec![button(to, false)]);
    frame(&mut app, &ctx, vec![]);
    let now = app.mini.rect.unwrap();
    assert!(area.contains_rect(now) && now.min.x < from.min.x - 50.0, "it moved, inside the window: {now:?}");
    assert!(app.prefs.mini_chat_at.is_some(), "and the place is remembered");
}

#[test]
fn a_message_written_in_the_mini_chat_lands_in_the_same_conversation() {
    let ctx = Context::default();
    let mut app = app_with_chat();
    leave_chat(&mut app, &ctx);
    app.mini_set(true);
    // the box takes the focus on opening
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let before = app.transcripts["a"].items.len();
    app.chat_draft = "/help".into();
    let enter = egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame(&mut app, &ctx, vec![enter]);
    assert!(app.transcripts["a"].items.len() > before, "the big chat's own transcript took it");
    assert!(app.chat_draft.is_empty());
}

#[test]
fn the_action_opens_and_folds_the_mini_chat() {
    let mut app = app_with_chat();
    app.chat_open = true;
    app.act("mini_chat", &serde_json::json!({ "chat": "loja" })).unwrap();
    assert!(app.mini.open && app.mini.on && !app.chat_open);
    app.act("mini_chat", &serde_json::json!({ "state": "close" })).unwrap();
    assert!(!app.mini.open && app.mini.on);
}

#[test]
fn the_pill_goes_where_it_is_dragged_and_stays_there() {
    let ctx = Context::default();
    let mut app = app_with_chat();
    leave_chat(&mut app, &ctx);
    let grab = Pos2::new(SIZE.x - 12.0 - 60.0, 56.0 + 22.0 + TITLE_BAR);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(grab)]);
    frame(&mut app, &ctx, vec![button(grab, true)]);
    for k in 1..=10 {
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(grab + Vec2::new(-50.0 * k as f32, 40.0 * k as f32))]);
    }
    let drop = grab + Vec2::new(-500.0, 400.0);
    frame(&mut app, &ctx, vec![button(drop, false)]);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.mini.open, "a drag is not a click");
    let [x, y] = app.prefs.mini_pill_at.expect("kept");
    // kept from the canvas area's corner (right of the sidebar, under the title bar)
    assert!((x + app.sidebar_width() - (SIZE.x - 12.0 - 500.0)).abs() < 2.0 && (y - (56.0 + 400.0)).abs() < 2.0, "{x} {y}");
    // a click where it now is opens the panel
    press(&mut app, &ctx, drop);
    assert!(app.mini.open);
    // a canvas made smaller keeps it in sight
    let small = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
    let at = crate::app::chat::mini::pill_place(small, Pos2::new(900.0, 700.0));
    assert!(small.contains(at));
}
