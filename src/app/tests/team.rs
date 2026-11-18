//! A chat's team in chat mode (chat/ask.rs): what a recruit says at the end of what its boss gave it comes to the boss's
//! chat as it ends, the rope lit while it works and a dot running each way, and a line in the boss's conversation for
//! each recruit at work.

use super::*;
use crate::app::chat::ask::{answer_message, asked_by, is_answer};
use crate::app::chat::direct::{ChatRun, NewChat};
use crate::transcript::{ChatItem, Step, Transcript};

/// A boss chat (its card on the canvas) and a recruit it made in chat mode: (boss, recruit, the recruit's name).
fn team(app: &mut DrawingApp, ctx: &Context) -> (String, String, String) {
    frame(app, ctx, vec![]);
    app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &[Preset::new("Chefe", "mago-agente-que-nao-existe")]);
    let boss = app.chat_agent.clone().unwrap();
    // an agent this machine does not have: a test never runs the real one
    app.prefs.chat_agent = "mago-agente-que-nao-existe".into();
    let (reply, said) = std::sync::mpsc::channel();
    let command = "\"rode os testes\"".into();
    app.apply_agent_command(UiCommand::Recruit { from: boss.clone(), command, worktree: false, mode: Some(AgentMode::Chat), reply });
    assert!(said.recv().unwrap().is_ok());
    let recruit = app.chats.iter().find(|chat| chat.recruit).unwrap().id.clone();
    let name = app.speaker(&recruit);
    (boss, recruit, name)
}

/// The texts `team_rows` paints for `boss`.
fn team_lines(app: &mut DrawingApp, ctx: &Context, boss: &str) -> Vec<String> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0))), ..Default::default() };
    let output = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let look = crate::app::chat::list::Look::new(true, &UiColors::new(true));
            app.team_rows(ui, boss, &look, 0.0);
        });
    });
    output.shapes.into_iter().filter_map(|clipped| if let Shape::Text(text) = clipped.shape { Some(text.galley.text().to_string()) } else { None }).collect()
}

#[test]
fn a_recruits_answer_comes_to_its_bosss_chat_when_it_ends_with_a_dot_on_their_rope() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let (boss, recruit, name) = team(&mut app, &ctx);
    assert_eq!(app.boss_of(&recruit).as_deref(), Some(boss.as_str()), "the chat whose rope holds it");
    assert_eq!(app.boss_of(&boss), None);
    // its task went: a dot runs from the boss to it, their rope stays lit while it works, and the boss's
    // conversation has a line for it, on the step it is on
    app.chat_runs.insert(recruit.clone(), ChatRun::held(app.now));
    app.task_sent(&recruit, "rode os testes");
    assert!(app.rope_pulses.iter().any(|(from, to, _)| from == &boss && to == &recruit));
    assert_eq!(app.asks_running(), [(boss.clone(), recruit.clone())]);
    let mut transcript = Transcript::new(PathBuf::new());
    transcript.items.push(ChatItem::Tool(Step { id: "s".into(), name: "Bash".into(), what: "cargo test".into(), ..Default::default() }));
    app.transcripts.insert(recruit.clone(), transcript);
    let lines = team_lines(&mut app, &ctx, &boss);
    assert!(lines.iter().any(|line| line.starts_with(&format!("{name} trabalhando")) && line.ends_with("Bash cargo test")), "{lines:?}");
    // its last words, then the turn over: in the boss's chat as its answer (in line, the boss answering now), a dot
    // back along the rope, and the line gone
    app.chat_heard(&recruit, &json!({ "type": "result", "is_error": false, "result": "Rodei: 14 passaram." }), false);
    app.chat_runs.remove(&recruit);
    app.chat_runs.insert(boss.clone(), ChatRun::held(app.now));
    app.chat_asks_tick(&ctx);
    let message = answer_message(&name, "Rodei: 14 passaram.");
    assert!(app.chat_queue.contains(&(boss.clone(), message.clone())), "{:?}", app.chat_queue);
    assert_eq!(asked_by(&message), Some((name.as_str(), "Rodei: 14 passaram.")));
    assert!(is_answer(&message) && !is_answer("Pergunta de \"Ana\" (a sua resposta volta para quem perguntou):\n\noi"));
    assert!(app.rope_pulses.iter().any(|(from, to, _)| from == &recruit && to == &boss));
    assert!(app.asks_running().is_empty() && app.chat_asks.is_empty());
    assert!(team_lines(&mut app, &ctx, &boss).is_empty());
}

#[test]
fn what_the_boss_sends_its_recruit_and_a_question_past_its_wait_come_back_to_the_boss() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let (boss, recruit, name) = team(&mut app, &ctx);
    // the boss's chat_send to its recruit: in the recruit's line as the boss's question, its answer coming back
    let said = app.act("chat_send", &json!({ "chat": name, "text": "e o lint?", "asker": boss })).unwrap();
    assert!(said.contains("a resposta dele chega neste chat"), "{said}");
    assert!(app.chat_outbox.is_empty() && app.chat_asks.len() == 1);
    app.chat_asks.clear();
    // anyone else's chat_send is a message as before
    app.act("chat_send", &json!({ "chat": name, "text": "oi" })).unwrap();
    assert_eq!(app.chat_outbox, [(recruit.clone(), "oi".to_string())]);
    // ask_agent past its wait (the bridge let go of the answer): the answer still comes, to the chat that asked
    let (reply, answer) = std::sync::mpsc::channel();
    app.ask_chat(&recruit, &boss, "Luna", "terminou?", reply);
    app.chat_runs.insert(recruit.clone(), ChatRun::held(app.now));
    app.ask_sent(0);
    drop(answer);
    app.chat_heard(&recruit, &json!({ "type": "result", "is_error": true, "result": "sem crédito" }), false);
    app.chat_runs.remove(&recruit);
    app.chat_runs.insert(boss.clone(), ChatRun::held(app.now));
    app.chat_asks_tick(&ctx);
    assert!(app.chat_queue.contains(&(boss.clone(), answer_message(&name, "Não terminei: falhou: sem crédito"))), "{:?}", app.chat_queue);
}
