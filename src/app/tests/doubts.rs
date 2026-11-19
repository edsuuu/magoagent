//! Dúvidas (app/doubts/, panes/doubts.rs): an agent leaves its questions, the person answers them in the panel, on the
//! canvas or over the chat, and sends the answered ones or all of them back to it in one message.

use super::panes::{pane_click, pane_frame};
use super::*;
use crate::app::doubts::{Doubt, answers_message, asked_questions};
use crate::app::panes::Pane;
use egui::accesskit::Role;

fn doubt(id: u64, question: &str, answer: &str) -> Doubt {
    Doubt { id, agent: "luna".into(), question: question.into(), answer: answer.into(), ..Default::default() }
}

/// The recommendation shows apart from the question: its own field, or what the question says after "Recomendação:"
/// or on a line of its own; a question that only looks like one stays whole.
#[test]
fn the_recommendation_comes_apart_from_the_question() {
    let parts = |question: &str, recommendation: &str| Doubt { question: question.into(), recommendation: recommendation.into(), ..doubt(1, "", "") }.parts();
    let pair = |question: &str, recommendation: &str| (question.to_string(), recommendation.to_string());

    assert_eq!(parts("Qual banco?", " Postgres. "), pair("Qual banco?", "Postgres."));
    assert_eq!(parts("Qual banco?\n**Recomendo** Postgres: já está no projeto.", ""), pair("Qual banco?", "**Recomendo** Postgres: já está no projeto."));
    assert_eq!(parts("Qual banco? Recomendação: Postgres.", ""), pair("Qual banco?", "Recomendação: Postgres."));
    assert_eq!(parts("Recomendas Postgres?", ""), pair("Recomendas Postgres?", ""), "nothing left to ask: the question whole");
    assert_eq!(parts("Qual porta?", ""), pair("Qual porta?", ""));
}

/// The section of an agent says the branch it was on when it last asked.
#[test]
fn an_agents_section_has_its_last_branch() {
    let mut board = crate::app::doubts::DoubtBoard::default();

    board.list = vec![Doubt { branch: "main".into(), ..doubt(1, "a", "") }, Doubt { branch: "mago/luna".into(), ..doubt(2, "b", "") }, doubt(3, "c", "")];
    assert_eq!(board.branch("luna"), "mago/luna", "the last known, not the empty one after it");
    assert_eq!(board.branch("sol"), "");
}

/// The message back: the answered ones, or all with the rest as the agent's to decide; nothing answered, nothing sent.
#[test]
fn the_answers_go_back_as_one_message() {
    let (porta, banco) = (doubt(1, "Qual porta?", "8080"), doubt(2, "Qual banco?", ""));

    assert_eq!(answers_message(&[&porta, &banco], false).unwrap(), "Respostas às suas dúvidas (tela Dúvidas):\n1. Qual porta?\n   → 8080");
    assert!(answers_message(&[&porta, &banco], true).unwrap().ends_with("2. Qual banco?\n   → sem resposta: decida você"));
    assert_eq!(answers_message(&[&banco], false), None);
    // the questions as texts, as {question, options}, or one alone
    let banco = json!({ "question": "Qual banco?", "options": ["Postgres", " ", "SQLite"], "recommendation": "Postgres" });
    let asked = asked_questions(&json!({ "questions": ["Qual porta?", banco, ""] }));

    assert_eq!(
        asked,
        [("Qual porta?".into(), vec![], String::new()), ("Qual banco?".into(), vec!["Postgres".to_string(), "SQLite".into()], "Postgres".into())]
    );

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

    let banco = json!({ "question": "Qual banco?", "options": ["Postgres", "SQLite"], "recommendation": "Postgres, já está no projeto" });

    app.act("doubts", &json!({ "action": "add", "asker": id, "questions": ["Qual porta?", banco] })).unwrap();
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
    // the agent's section, each question folds; the suggested answers are buttons, the recommendation folds apart
    assert!(shown.iter().any(|(role, text, _)| *role == Role::DisclosureTriangle && text.starts_with("Luna")), "{shown:?}");
    assert!(shown.iter().any(|(role, text, _)| *role == Role::Button && text == "Postgres"));

    let recommendation =
        shown.iter().find(|(role, text, _)| *role == Role::DisclosureTriangle && text == "Recomendação do agente").map(|(.., rect)| rect.center());

    assert!(!shown.iter().any(|(_, text, _)| text.contains("já está no projeto")), "folded until asked");
    pane_click(&mut app, &ctx, recommendation.expect("the recommendation's fold"));
    let shown = pane_frame(&mut app, &ctx, vec![]);

    assert!(shown.iter().any(|(role, text, _)| *role == Role::Label && text.contains("já está no projeto")), "{shown:?}");
}
