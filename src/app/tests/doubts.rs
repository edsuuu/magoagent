//! Dúvidas (app/doubts/, panes/doubts.rs): an agent leaves its questions, the person answers them in the panel, on the
//! canvas or over the chat, and sends the answered ones or all of them back to it in one message.

use super::panes::{pane_click, pane_frame};
use super::*;
use crate::app::doubts::{Doubt, answers_message, asked_questions};
use crate::app::panes::Pane;
use egui::accesskit::Role;

fn doubt(id: u64, question: &str, answer: &str) -> Doubt {
    Doubt { id, agent: "luna".into(), question: question.into(), options: Vec::new(), answer: answer.into() }
}

/// The message back: the answered ones, or all with the rest as the agent's to decide; nothing answered, nothing sent.
#[test]
fn the_answers_go_back_as_one_message() {
    let (porta, banco) = (doubt(1, "Qual porta?", "8080"), doubt(2, "Qual banco?", ""));

    assert_eq!(answers_message(&[&porta, &banco], false).unwrap(), "Respostas às suas dúvidas (tela Dúvidas):\n1. Qual porta?\n   → 8080");
    assert!(answers_message(&[&porta, &banco], true).unwrap().ends_with("2. Qual banco?\n   → sem resposta: decida você"));
    assert_eq!(answers_message(&[&banco], false), None);
    // the questions as texts, as {question, options}, or one alone
    let asked = asked_questions(&json!({ "questions": ["Qual porta?", { "question": "Qual banco?", "options": ["Postgres", " ", "SQLite"] }, ""] }));

    assert_eq!(asked, [("Qual porta?".into(), vec![]), ("Qual banco?".into(), vec!["Postgres".to_string(), "SQLite".into()])]);
    assert_eq!(asked_questions(&json!({ "question": "Só esta?" })).len(), 1);
}

/// The agent adds its questions (the panel opens), the person answers one and sends it: it goes to the chat that asked
/// and leaves the panel; Enviar todas sends the rest as the agent's to decide.
#[test]
fn an_agent_asks_and_the_answered_ones_go_back_to_it() {
    let mut app = DrawingApp::default();
    let id = push_chat(&mut app, "Luna");
    let add = json!({ "action": "add", "asker": id, "questions": ["Qual porta?", { "question": "Qual banco?", "options": ["Postgres", "SQLite"] }] });

    assert!(app.act("doubts", &json!({ "action": "add", "questions": ["x"] })).is_err(), "no agent behind it: nobody to answer to");
    assert!(app.act("doubts", &add).unwrap().contains("ids 1, 2"));
    assert_eq!((app.panes.open, app.doubts.unanswered()), (Some(Pane::Doubts), 2));
    app.act("doubts", &json!({ "action": "answer", "id": 1, "answer": "8080" })).unwrap();
    assert!(app.act("doubts", &json!({ "action": "list", "asker": id })).unwrap().contains("1. Qual porta? → 8080"));
    app.doubts_send(&id, false).unwrap();
    assert!(matches!(&app.chat_outbox[..], [(to, text)] if *to == id && text.contains("→ 8080") && !text.contains("banco")));
    assert_eq!(app.doubts.list.iter().map(|doubt| doubt.id).collect::<Vec<_>>(), [2], "the sent one leaves the panel");
    assert!(app.doubts_send(&id, false).is_err(), "none answered");
    app.doubts_send(&id, true).unwrap();
    assert!(app.chat_outbox[1].1.contains("Qual banco?\n   → sem resposta: decida você"));
    assert!(app.doubts.list.is_empty());
}

/// Over the chat the title bar keeps only the Dúvidas button, and the panel opens on top of the conversation with the
/// questions and their buttons.
#[test]
fn the_doubts_panel_opens_over_the_chat() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = push_chat(&mut app, "Luna");

    app.act("doubts", &json!({ "action": "add", "asker": id, "questions": ["Qual porta?"] })).unwrap();
    app.chat_open = true;
    app.set_pane(None);

    for _ in 0..4 {
        pane_frame(&mut app, &ctx, vec![]);
    }

    let shown = pane_frame(&mut app, &ctx, vec![]);
    let button = shown.iter().find(|(role, text, _)| *role == Role::Button && text == "Dúvidas").map(|(_, _, rect)| rect.center());

    assert!(!shown.iter().any(|(_, text, _)| text == "Alterações"), "the canvas's panels stay off the chat");
    pane_click(&mut app, &ctx, button.expect("the Dúvidas button over the chat"));
    assert_eq!(app.panes.open, Some(Pane::Doubts));
    // a new area sizes itself on its first frame
    pane_frame(&mut app, &ctx, vec![]);
    let shown = pane_frame(&mut app, &ctx, vec![]);

    assert!(shown.iter().any(|(_, text, _)| text.contains("Qual porta?")), "{shown:?}");
    assert!(shown.iter().any(|(role, text, _)| *role == Role::Button && text == "Enviar todas"));
}
