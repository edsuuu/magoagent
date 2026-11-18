//! A message to a Claude Code session outside the app (maestro/message.rs): typed in its tmux pane when nothing waits on
//! its screen, else queued for its Stop hook, which hands it to Claude Code; the island's box and the MCP's
//! `outside_message`. tmux here is always a fake: nothing reaches the person's own.

use super::*;
use crate::app::maestro::message::Delivery;
use crate::app::maestro::relay;
use crate::mcp::bridge::{Bridge, UiCommand, locked};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const PILL: &str = "pill:code:/w/loja";
const SOCKET: &str = "/tmp/magoagent-teste/tmux.sock";

fn app() -> DrawingApp {
    let mut app = DrawingApp { bridge: Bridge::start(None).ok(), ..Default::default() };
    app.prefs.island = true;
    app
}

/// A hook of session s1 in /w/loja as the relay sends it, from inside tmux pane %3 or not.
fn hook(event: &str, tool: &str, input: Value, tmux: bool) -> Value {
    let mut payload = json!({ "hook_event_name": event, "session_id": "s1", "cwd": "/w/loja", "tool_name": tool, "tool_input": input });
    let env = move |name: &str| match name {
        "TMUX" if tmux => Some(format!("{SOCKET},4242,0")),
        "TMUX_PANE" if tmux => Some("%3".to_string()),
        _ => None,
    };
    relay::tag(&mut payload, &env);
    payload
}

/// The fake tmux: Claude Code in front, `screen` on it; the calls it got.
fn fake(app: &mut DrawingApp, front: &'static str, screen: &'static str, works: bool) -> Rc<RefCell<Vec<Vec<String>>>> {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let seen = calls.clone();
    app.outside.fake_tmux = Some(Box::new(move |args: &[String]| {
        seen.borrow_mut().push(args.to_vec());
        match args.get(2).map(String::as_str) {
            _ if !works => Err("no server running on /tmp/magoagent-teste/tmux.sock".into()),
            Some("display-message") => Ok(format!("{front}\n")),
            Some("capture-pane") => Ok(screen.to_string()),
            _ => Ok(String::new()),
        }
    }));
    calls
}

const TYPING_BOX: &str = "⏺ Pronto.\n\n╭──────────╮\n│ >        │\n╰──────────╯\n  ? for shortcuts\n";
const PERMISSION: &str =
    " Bash command\n Do you want to proceed?\n ❯ 1. Yes\n   2. Yes, and don't ask again\n   3. No, and tell Claude what to do differently (esc)\n";

/// What the bridge sent the app: taken as its loop does.
fn take(app: &mut DrawingApp) -> Value {
    let Ok(UiCommand::Hook { from, payload, reply }) = app.bridge.as_ref().unwrap().commands.recv_timeout(Duration::from_secs(5)) else {
        panic!("o hook não chegou")
    };
    app.take_hook(&from, &payload, reply);
    payload
}

/// The session's Stop through the real relay: what the hook printed.
fn stop(app: &mut DrawingApp) -> String {
    let bridge = app.bridge.as_ref().unwrap();
    let printed =
        relay::send((bridge.address.clone(), bridge.token.clone()), json!({ "hook_event_name": "Stop", "session_id": "s1", "cwd": "/w/loja" }), false, &|_| {
            None
        });
    take(app);
    printed
}

#[test]
fn a_message_is_typed_in_the_tmux_pane_when_nothing_waits_on_its_screen() {
    let mut app = app();
    app.take_hook("ext:s1", &hook("UserPromptSubmit", "", Value::Null, true), None);
    let calls = fake(&mut app, "claude", TYPING_BOX, true);
    assert_eq!(app.outside_message(PILL, "  roda os testes\n"), Ok(Delivery::Typed));
    let tmux = |words: &[&str]| ["-S", SOCKET].iter().chain(words).map(|word| word.to_string()).collect::<Vec<_>>();
    assert_eq!(
        *calls.borrow(),
        [
            tmux(&["display-message", "-p", "-t", "%3", "#{pane_current_command}"]),
            tmux(&["capture-pane", "-p", "-t", "%3"]),
            tmux(&["send-keys", "-t", "%3", "-l", "--", "roda os testes"]),
            tmux(&["send-keys", "-t", "%3", "Enter"]),
        ]
    );
    assert_eq!(app.outside_pending(PILL), 0);
    // a shell in front (Claude Code left), a question on its screen, a tmux that is gone: nothing typed but the checks, and it waits in the queue
    for (front, screen, works) in [("zsh", TYPING_BOX, true), ("claude", PERMISSION, true), ("claude", TYPING_BOX, false)] {
        let calls = fake(&mut app, front, screen, works);
        assert_eq!(app.outside_message(PILL, "oi"), Ok(Delivery::Queued { idle: false }), "{front}");
        assert!(!calls.borrow().iter().any(|call| call.contains(&"-l".to_string())), "{front}: nada digitado");
        locked(&app.bridge.as_ref().unwrap().shared).outbox.clear();
    }
}

#[test]
fn while_a_permission_waits_the_message_is_queued_and_the_stop_hands_it_to_claude_code() {
    let mut app = app();
    let calls = fake(&mut app, "claude", TYPING_BOX, true);
    app.take_hook("ext:s1", &hook("PreToolUse", "Bash", json!({ "command": "cargo test" }), true), None);
    // a permission on the terminal's screen (the island off: it is the terminal's): the words would answer it
    app.prefs.island = false;
    let (reply, _answer) = std::sync::mpsc::channel();
    app.take_hook("ext:s1", &hook("PermissionRequest", "Bash", json!({ "command": "cargo test" }), true), Some(reply));
    assert_eq!(app.outside_message(PILL, "roda os testes"), Ok(Delivery::Queued { idle: false }));
    assert!(calls.borrow().is_empty(), "nem olhou o tmux");
    assert_eq!(app.outside_pending(PILL), 1);
    // another call ending says nothing of it; the same call ending frees the screen, but the new one goes after the first, in order
    app.take_hook("ext:s1", &hook("PostToolUse", "Bash", json!({ "command": "ls" }), true), None);
    assert_eq!(app.outside_message(PILL, "e o lint"), Ok(Delivery::Queued { idle: false }));
    app.take_hook("ext:s1", &hook("PostToolUse", "Bash", json!({ "command": "cargo test" }), true), None);
    assert_eq!(app.outside_message(PILL, "e me diz"), Ok(Delivery::Queued { idle: false }));
    assert!(calls.borrow().is_empty());
    assert_eq!(app.outside_pending(PILL), 3);
    // the turn ends: the hook prints the block with all of them, the pill says so and goes on working
    let printed: Value = serde_json::from_str(&stop(&mut app)).unwrap();
    assert_eq!(printed, json!({ "decision": "block", "reason": "Mensagem da pessoa pelo MagoAgent:\nroda os testes\n\ne o lint\n\ne me diz" }));
    let pill = &app.outside_pills()[0];
    assert_eq!((pill.activity.as_str(), pill.state, pill.badge), ("3 mensagens entregues", crate::app::ui::agents::AgentState::Working(0.0), None));
    assert_eq!(app.outside_pending(PILL), 0);
    // and the next Stop is a stop
    assert_eq!(stop(&mut app), "");
    assert_eq!(app.outside_pills()[0].state, crate::app::ui::agents::AgentState::Idle(None));
    // now nothing waits: typed
    assert_eq!(app.outside_message(PILL, "valeu"), Ok(Delivery::Typed));
}

#[test]
fn an_idle_session_without_tmux_gets_it_when_its_next_turn_ends_and_its_pill_stays_meanwhile() {
    let mut app = app();
    app.take_hook("ext:s1", &hook("UserPromptSubmit", "", Value::Null, false), None);
    assert_eq!(app.outside_message(PILL, "oi"), Ok(Delivery::Queued { idle: false }), "working, no tmux: the end of this turn");
    locked(&app.bridge.as_ref().unwrap().shared).outbox.clear();
    app.take_hook("ext:s1", &hook("Stop", "", Value::Null, false), None);
    assert_eq!(app.outside_message(PILL, "oi"), Ok(Delivery::Queued { idle: true }));
    // 5.2 s after the Stop the pill would go: a message waiting keeps it, and so does the island showing it
    app.outside.expire_badges();
    app.outside_tick();
    assert_eq!(app.outside_pills().len(), 1);
    locked(&app.bridge.as_ref().unwrap().shared).outbox.clear();
    app.outside_shown(PILL);
    app.outside_tick();
    assert_eq!(app.outside_pills().len(), 1);
    // a session that ended takes its queue along
    app.outside_message(PILL, "oi").unwrap();
    app.take_hook("ext:s1", &hook("SessionEnd", "", Value::Null, false), None);
    assert!(locked(&app.bridge.as_ref().unwrap().shared).outbox.is_empty());
    assert_eq!(app.outside_message(PILL, "oi"), Err("Essa sessão não está mais aberta.".into()));
}

#[test]
fn only_claude_code_takes_messages() {
    let mut app = app();
    let mut desktop = hook("UserPromptSubmit", "", Value::Null, true);
    desktop["mago_origin"] = "claude-desktop".into();
    app.take_hook("ext:s1", &desktop, None);
    let id = app.outside_pills()[0].id.clone();
    assert!(!app.outside_takes_messages(&id));
    assert!(app.outside_message(&id, "oi").unwrap_err().contains("responda no terminal"));
    assert!(app.outside_message("pill:code:/nada", "oi").is_err() && app.outside_message("chat-1", "oi").is_err());
    app.take_hook("ext:s1", &hook("UserPromptSubmit", "", Value::Null, false), None);
    assert!(app.outside_takes_messages(PILL));
    assert_eq!(app.outside_message(PILL, "  "), Err("Escreva a mensagem.".into()));
    // no bridge, no queue: the box says so
    app.bridge = None;
    assert!(app.outside_message(PILL, "oi").unwrap_err().contains("responda no terminal"));
}

#[test]
fn the_mcp_sends_by_the_sessions_name() {
    let mut app = app();
    assert!(app.act("outside_message", &json!({ "text": "oi" })).unwrap_err().starts_with("Nenhuma sessão do Claude Code"));
    app.take_hook("ext:s1", &hook("UserPromptSubmit", "", Value::Null, false), None);
    let mut other = hook("UserPromptSubmit", "", Value::Null, false);
    (other["session_id"], other["cwd"]) = ("s2".into(), "/w/loja-api".into());
    app.take_hook("ext:s2", &other, None);
    // "loja" is the folder of one and part of the other's: the folder wins
    assert_eq!(
        app.act("outside_message", &json!({ "session": "loja", "text": "roda os testes" })).as_deref(),
        Ok("Claude Code · loja: Na fila: entra quando o Claude terminar esta vez.")
    );
    assert_eq!(locked(&app.bridge.as_ref().unwrap().shared).outbox["s1"], ["roda os testes"]);
    assert!(app.act("outside_message", &json!({ "session": "API", "text": "oi" })).unwrap().starts_with("Claude Code · loja-api:"));
    assert!(app.act("outside_message", &json!({ "text": "oi" })).unwrap_err().contains("Mais de uma sessão"));
    assert!(app.act("outside_message", &json!({ "session": "blog", "text": "oi" })).unwrap_err().contains("\"Claude Code · loja\""));
    assert!(app.act("outside_message", &json!({ "session": "loja" })).unwrap_err().contains("Escreva"));
}

/// One frame of the island; the texts it painted.
fn step(app: &mut DrawingApp, ctx: &Context, time: &mut f64, events: Vec<egui::Event>) -> Vec<String> {
    painted(app, ctx, time, events).into_iter().map(|(text, _)| text).collect()
}

/// One frame of the island; the texts it painted and where.
fn painted(app: &mut DrawingApp, ctx: &Context, time: &mut f64, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
    *time += 0.05;
    island_frame(app, ctx, *time - 0.05, events)
}

/// The pointer at `at` for `frames` frames; the texts of the last.
fn rest(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2, frames: usize) -> Vec<String> {
    (0..frames).map(|_| step(app, ctx, time, vec![egui::Event::PointerMoved(at)])).last().unwrap()
}

/// Where `text` was painted.
fn at(texts: &[(String, Rect)], text: &str) -> Rect {
    texts.iter().find(|(painted, _)| painted == text).map(|(_, rect)| *rect).unwrap_or_else(|| panic!("{text:?} not in {texts:?}"))
}

#[test]
fn the_islands_box_sends_and_says_how_it_went() {
    let ctx = Context::default();
    let mut app = app();
    app.take_hook("ext:s1", &hook("UserPromptSubmit", "", Value::Null, false), None);
    let mut time = 1.0;
    rest(&mut app, &ctx, &mut time, Pos2::new(ISLAND_SCREEN.x / 2.0, 10.0), 20);
    let middle = island_rect(&app).center();
    let press = |at, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    for events in [vec![press(middle, true)], vec![press(middle, false)]] {
        step(&mut app, &ctx, &mut time, events);
    }
    app.island_show(PILL, time);
    let middle = island_rect(&app).center();
    let texts = rest(&mut app, &ctx, &mut time, middle, 20);
    assert!(texts.iter().any(|text| text == "Mensagem para esta sessão…") && texts.iter().any(|text| text == "Ir ao terminal"), "{texts:?}");
    assert!(!texts.iter().any(|text| text == "Responder no terminal" || text.ends_with("na fila")), "{texts:?}");
    // a click in the box, the words and Enter
    let rect = island_rect(&app);
    let field = Pos2::new(rect.left() + 10.0 + 84.0 + 40.0, rect.bottom() - 10.0 - 14.0 - 20.0);
    for events in [vec![egui::Event::PointerMoved(field)], vec![press(field, true)], vec![press(field, false)]] {
        step(&mut app, &ctx, &mut time, events);
    }
    step(&mut app, &ctx, &mut time, vec![egui::Event::Text("roda os testes".into())]);
    let enter = |pressed| egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
    step(&mut app, &ctx, &mut time, vec![enter(true), enter(false)]);
    rest(&mut app, &ctx, &mut time, field, 3);
    assert_eq!(locked(&app.bridge.as_ref().unwrap().shared).outbox["s1"], ["roda os testes"]);
    // the notice in the box, before the way to the terminal; the count beside the name, on its row, before it
    let texts = painted(&mut app, &ctx, &mut time, vec![egui::Event::PointerMoved(field)]);
    let (notice, go) = (at(&texts, "Na fila: entra quando o Claude terminar esta vez."), at(&texts, "Ir ao terminal"));
    assert!(notice.right() < go.left() && (notice.center().y - go.center().y).abs() < 4.0, "{notice:?} {go:?}");
    let name = texts.iter().find(|(text, _)| text.starts_with("Claude Code · loja")).map(|(_, rect)| *rect).unwrap();
    let queued = at(&texts, "1 na fila");
    assert!(queued.right() < name.left() && (queued.center().y - name.center().y).abs() < 4.0 && queued.bottom() < notice.top(), "{queued:?} {name:?}");
    assert!(island_rect(&app).contains_rect(queued) && island_rect(&app).contains_rect(notice));
    // the longest notice goes whole, in two lines inside the box
    app.island.session.as_mut().unwrap().notice(Delivery::Queued { idle: true }.notice(), time);
    let texts = painted(&mut app, &ctx, &mut time, vec![egui::Event::PointerMoved(field)]);
    let idle = at(&texts, "Na fila: a sessão está parada; entra quando ela trabalhar de novo.");
    assert!(idle.right() < go.left() && idle.height() > notice.height() / 2.0 && idle.height() < 40.0, "{idle:?} {go:?}");
    // the turn ends with it: delivered, and the box's count goes
    stop(&mut app);
    let texts = rest(&mut app, &ctx, &mut time, field, 4);
    assert!(!texts.iter().any(|text| text.ends_with("na fila")), "{texts:?}");
}
