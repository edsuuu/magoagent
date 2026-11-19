//! Parar tudo (chat/stop.rs, system/close_ask.rs): every agent stops and nothing makes one go on by itself; the window
//! closed with agents at work asks before they go on hidden.

use super::team::team;
use super::*;
use crate::app::chat::commands::{Local, parse};
use crate::app::chat::direct::ChatRun;
use crate::app::chat::stop::stop_words;
use crate::transcript::ChatItem;

/// "Para tudo" written alone, or /parar: the chat stops everything itself, even while its agent works.
#[test]
fn para_tudo_written_alone_is_parar_tudo() {
    for text in ["para tudo", "Para tudo!", "PARE TUDO.", "parar tudo", "para todos", "/parar"] {
        assert_eq!(parse(text, "claude"), Some(Ok(Local::StopAll)), "{text}");
    }

    for text in ["para tudo menos o Revisor", "para", "não para tudo", "o teste para tudo?!"] {
        assert!(!stop_words(text), "{text}");
    }

    assert_eq!(parse("/parar", "codex"), Some(Ok(Local::StopAll)), "Codex's chats too");
}

/// The boss answering, its recruit on a task nobody waits for, a message in the queue and one about to go: Parar tudo
/// stops both and drops the rest. The recruit's "parou sem responder" used to reach the boss and start it again.
#[test]
fn parar_tudo_stops_every_agent_and_nothing_wakes_them() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let (boss, recruit, _) = team(&mut app, &ctx);
    let (reply, answer) = std::sync::mpsc::channel();

    app.ask_chat(&recruit, &boss, "Luna", "roda os testes", reply);
    app.chat_runs.insert(recruit.clone(), ChatRun::held(app.now));
    app.ask_sent(0);
    drop(answer);
    app.chat_runs.insert(boss.clone(), ChatRun::held(app.now));
    app.queue_message(&boss, "e o lint?".into());
    app.chat_outbox.push((recruit.clone(), "oi".into()));
    let said = app.act("stop_all", &json!({})).unwrap();

    assert!(said.starts_with("Tudo parado: 2 chat(s)") && said.contains("2 mensagem(ns)") && said.contains("1 pergunta(s)"), "{said}");
    assert!(app.chat_runs.is_empty() && app.chat_queue.is_empty() && app.chat_outbox.is_empty() && app.chat_asks.is_empty());
    // the next frames give nobody anything to do
    app.chat_asks_tick(&ctx);
    app.chat_nodes_tick(&ctx);
    assert!(app.chat_runs.is_empty() && app.chat_queue.is_empty() && app.chat_outbox.is_empty());
    // in the box of a chat at work: it stops at once, it doesn't wait in the queue for the turn to end
    app.chat_runs.insert(boss.clone(), ChatRun::held(app.now));
    assert!(app.chat_submit(&ctx, &boss, "Para tudo!", "Para tudo!"));
    assert!(app.chat_runs.is_empty() && app.chat_queue.is_empty());
    assert!(matches!(app.transcripts[&boss].items.last(), Some(ChatItem::System(note)) if note.starts_with("Tudo parado")));
}

/// The window closed: with nothing at work it goes out of sight as before; with an agent at work (or a message in the
/// queue) it stays and asks.
#[test]
fn closing_with_agents_at_work_asks_first() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    let close = |app: &mut DrawingApp| {
        let mut raw = egui::RawInput::default();

        raw.viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
        let mut output = ctx.run_ui(raw, |_| app.background_window(&ctx));

        output.textures_delta.clear();
        output.viewport_output.remove(&egui::ViewportId::ROOT).map(|viewport| viewport.commands).unwrap_or_default()
    };

    assert!(close(&mut app).contains(&egui::ViewportCommand::Visible(false)));
    assert!(!app.close_ask);
    let id = push_chat(&mut app, "Luna");

    app.chat_runs.insert(id, ChatRun::held(app.now));
    let sent = close(&mut app);

    assert!(app.close_ask && sent.contains(&egui::ViewportCommand::CancelClose), "{sent:?}");
    assert!(!sent.contains(&egui::ViewportCommand::Visible(false)), "it stays in sight, asking");
}
