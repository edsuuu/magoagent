//! A chat's card laid out at any size (chat/node_ui.rs, chat/recruit_card.rs): what is pinned to its bottom edge (the
//! band, the box) stays whole inside it, the conversation scrolls in what is left above, and a recruit's task and
//! answers are whole, in the chat's Markdown.

use super::*;
use crate::app::chat::direct::{ChatRun, NewChat};
use crate::app::chat::node_ui::{CHAT_LIVE_MIN, RECRUIT_LIVE_MIN, chat_card_body, pinned_key};
use crate::transcript::{ChatItem, Step, Transcript};

/// A few lines of conversation: what the card keeps above its pinned part, at the least size it shows its chat at.
const A_FEW_LINES: f32 = 40.0;
const TASK: &str = "Teste INV-168: perguntas via WhatsApp.\n\nLeia o fluxo de perguntas, rode os testes de integração e diga à Luna o que \
                    falta. Já estão prontos o webhook e a fila; o que falta é o retorno das respostas para quem perguntou.";
const ANSWER: &str = "Passo 0 concluído, Luna.\n\n- **Ferramentas:** tenho o MCP do app e o `magoagent`.\n- **Testes:** 14 passaram, nenhum falhou.";

/// A chat as the form makes it, run by `command`: its id.
fn new_chat(app: &mut DrawingApp, command: &str) -> String {
    app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &[Preset::new("Chefe", command)]);
    app.chat_agent.clone().unwrap()
}

/// The Maestro's recruit in chat mode, with a task, the steps it took and its answer: its id.
fn recruit(app: &mut DrawingApp) -> String {
    let boss = new_chat(app, "mago-agente-que-nao-existe");
    let (reply, said) = std::sync::mpsc::channel();
    let command = "claude \"faça o passo 0\"".into();
    app.apply_agent_command(UiCommand::Recruit { from: boss, command, worktree: false, mode: Some(AgentMode::Chat), reply });
    assert!(said.recv().unwrap().is_ok());
    let id = app.chats.iter().find(|c| c.recruit).unwrap().id.clone();
    let mut transcript = Transcript::new(PathBuf::new());
    transcript.items.push(ChatItem::User(TASK.into()));
    for (n, (name, what)) in [("ToolSearch", "select:Read"), ("Read", "src/fila.rs"), ("Bash", "magoagent list")].into_iter().enumerate() {
        transcript.items.push(ChatItem::Tool(Step { id: format!("s{n}"), name: name.into(), what: what.into(), ended: Some(1.0), ..Default::default() }));
    }
    transcript.items.push(ChatItem::Agent(ANSWER.into()));
    app.transcripts.insert(id.clone(), transcript);
    id
}

/// The card `id` made `size` (in the drawing's points).
fn resize(app: &mut DrawingApp, id: &str, size: Vec2) {
    let card = app.chat_node(id).unwrap();
    if let ElementKind::Rectangle { width, height, .. } = &mut app.scene.elements[card].kind {
        (*width, *height) = (size.x, size.y);
    }
    // in the middle of the view, so all of it is on screen
    let middle = app.to_world(app.canvas_rect.center());
    let from = app.scene.elements[card].center();
    app.scene.elements[card].move_by(middle - from);
}

/// A few frames of the cards (a card lays itself out by what the frame before measured); every word painted in the last.
fn cards(app: &mut DrawingApp, ctx: &Context) -> Vec<String> {
    let mut words = Vec::new();
    for _ in 0..4 {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), ..Default::default() };
        let output = ctx.run_ui(raw, |_| app.chat_node_overlay(ctx));
        words.clear();
        let mut shapes: Vec<egui::Shape> = output.shapes.into_iter().map(|clipped| clipped.shape).collect();
        while let Some(shape) = shapes.pop() {
            match shape {
                egui::Shape::Vec(inner) => shapes.extend(inner),
                egui::Shape::Text(text) => words.push(text.galley.text().to_string()),
                _ => {}
            }
        }
    }
    words
}

/// Where the card's pinned part was laid out, and the card's inside, in its own points.
fn pinned_in(app: &DrawingApp, ctx: &Context, id: &str, padding: Vec2) -> (Rect, Rect) {
    let element = &app.scene.elements[app.chat_node(id).unwrap()];
    let (frame_rect, zoom) = app.terminal_frame(element);
    let body = chat_card_body(frame_rect, zoom);
    let body = if app.is_recruit_chat(id) { body.with_max_y(frame_rect.bottom() - 1.0) } else { body };
    let inner = Rect::from_min_size(Pos2::ZERO, body.size() / zoom).shrink2(padding);
    (ctx.data(|d| d.get_temp::<Rect>(pinned_key(id))).expect("the card laid out its pinned part"), inner)
}

#[test]
fn a_recruits_card_keeps_its_band_inside_at_any_size_and_shows_its_task_and_answer_whole() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = recruit(&mut app);
    // born big enough to show its work
    let born = local_bounds(&app.scene.elements[app.chat_node(&id).unwrap()]).size();
    assert!(born.x >= 340.0 && born.y >= 380.0, "{born:?}");
    // working, a command running: the band (Claude's, its mascot in it) pinned to the bottom edge
    app.chat_runs.insert(id.clone(), ChatRun::held(app.now));
    let running = Step { id: "s9".into(), name: "Edit".into(), what: "src/fila.rs".into(), ..Default::default() };
    let transcript = app.transcripts.get_mut(&id).unwrap();
    transcript.items.push(ChatItem::Tool(running));
    transcript.running.push(("s9".into(), "Edit src/fila.rs".into()));
    // the smallest it shows its work at, the size recruits had (whose band went past the edge), the default, bigger;
    // at two zooms: laid out in the card's own points, the same
    for (size, zoom) in [(RECRUIT_LIVE_MIN, 1.0), (Vec2::new(340.0, 220.0), 1.0), (Vec2::new(340.0, 220.0), 0.6), (born, 1.0), (Vec2::new(430.0, 580.0), 1.0)] {
        app.zoom = zoom;
        resize(&mut app, &id, size);
        cards(&mut app, &ctx);
        let (pinned, inner) = pinned_in(&app, &ctx, &id, Vec2::new(10.0, 8.0));
        assert!(pinned.height() >= 40.0, "the band: {pinned:?}");
        assert!(
            (pinned.bottom() - inner.bottom()).abs() < 0.5 && pinned.left() >= inner.left() && pinned.right() <= inner.right() + 0.5,
            "{size:?} {pinned:?} {inner:?}"
        );
        assert!(pinned.top() - inner.top() >= A_FEW_LINES, "{size:?}: room for its work above the band ({pinned:?} in {inner:?})");
    }
    app.zoom = 1.0;
    // done: "Pronto" pinned instead, and with room, the task and the answer whole, their Markdown drawn (no ** left)
    app.chat_runs.clear();
    app.transcripts.get_mut(&id).unwrap().running.clear();
    resize(&mut app, &id, Vec2::new(430.0, 580.0));
    let words = cards(&mut app, &ctx);
    let (pinned, inner) = pinned_in(&app, &ctx, &id, Vec2::new(10.0, 8.0));
    assert!((pinned.bottom() - inner.bottom()).abs() < 0.5 && pinned.height() < 30.0, "{pinned:?}");
    let all = words.join(" ");
    assert!(words.iter().any(|w| w == "Pronto"), "{words:?}");
    assert!(all.contains("o retorno das respostas para quem perguntou.") && all.contains("Teste INV-168"), "the task whole: {words:?}");
    assert!(all.contains("Ferramentas:") && all.contains("14 passaram, nenhum falhou."), "the answer whole: {words:?}");
    assert!(!all.contains("**") && !all.contains('…'), "nothing cut, the Markdown drawn: {words:?}");
    // too small for it all: it scrolls, the band still inside
    resize(&mut app, &id, RECRUIT_LIVE_MIN);
    cards(&mut app, &ctx);
    let (pinned, inner) = pinned_in(&app, &ctx, &id, Vec2::new(10.0, 8.0));
    assert!((pinned.bottom() - inner.bottom()).abs() < 0.5 && pinned.top() > inner.top(), "{pinned:?} {inner:?}");
}

#[test]
fn a_chat_card_keeps_its_box_inside_and_the_conversation_scrolls_above_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = new_chat(&mut app, "claude");
    let mut transcript = Transcript::new(PathBuf::new());
    for n in 0..30 {
        transcript.items.extend([ChatItem::User(format!("pergunta {n}")), ChatItem::Agent(format!("resposta {n}: {ANSWER}"))]);
    }
    transcript.items.push(ChatItem::Agent("A última resposta.".into()));
    app.transcripts.insert(id.clone(), transcript);
    // the person's own card at its usual size is the picture; from the least size it shows the chat, idle and working
    assert!(!crate::app::chat::node_ui::chat_live(Rect::from_min_size(Pos2::ZERO, crate::app::chat::node::CHAT_NODE_SIZE), 1.0, false));
    for working in [false, true] {
        if working {
            app.chat_runs.insert(id.clone(), ChatRun::held(app.now));
        }
        for size in [CHAT_LIVE_MIN, Vec2::new(300.0, 300.0), Vec2::new(420.0, 520.0)] {
            resize(&mut app, &id, size);
            let words = cards(&mut app, &ctx);
            let (pinned, inner) = pinned_in(&app, &ctx, &id, Vec2::new(10.0, 6.0));
            assert!((pinned.bottom() - inner.bottom()).abs() < 0.5 && pinned.right() <= inner.right() + 0.5, "{working} {size:?} {pinned:?} {inner:?}");
            assert!(pinned.top() - inner.top() >= A_FEW_LINES, "{working} {size:?}: room for the conversation ({pinned:?} in {inner:?})");
            // the newest message in view over the box: the conversation scrolled to its end
            assert!(words.iter().any(|w| w == "A última resposta."), "{working} {size:?} {words:?}");
        }
    }
}
