//! A chat's card laid out at any size (chat/node_ui.rs, chat/recruit_card.rs): what is pinned to its bottom edge (the
//! band, the box) stays whole inside it, the conversation scrolls in what is left above, and a recruit's task and
//! answers are whole, in the chat's Markdown (its picture's too); its steps open on a click, with no tip.

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

    // its words in the chat's Markdown, as the chat screen draws them
    let all = cards(&mut app, &ctx).join(" ");

    assert!(all.contains("Ferramentas:") && !all.contains("**"), "{all}");
}

/// A recruit's report, as the user saw it with its marks painted: a list with bold, code and italics.
const REPORT: &str = "Feito.\n\n- **O que aconteceu:** o clique na caixa *Buscar* não chegou\n- **O que fiz:** rodei `--fechar` e abri de novo";
/// A command longer than its row.
const COMMAND: &str =
    "php ~/.claude/obsidian-memory/ferramentas/hdtb-inv168/whatsapp/rodar.php -- --desde=35728,2 --ate=35800 --fila=perguntas --modo=completo";

/// Each text painted in a frame of the cards at `time`: what shows of it (its rows' letters: a cut one ends in "…"),
/// whether it was cut, and where.
fn shown(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) -> Vec<(String, bool, Rect)> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), time: Some(time), events, ..Default::default() };
    let output = ctx.run_ui(raw, |_| app.chat_node_overlay(ctx));
    let mut texts = Vec::new();
    let mut shapes: Vec<egui::Shape> = output.shapes.into_iter().map(|clipped| clipped.shape).collect();

    while let Some(shape) = shapes.pop() {
        match shape {
            egui::Shape::Vec(inner) => shapes.extend(inner),
            egui::Shape::Text(text) => {
                let letters: String = text.galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect();

                texts.push((letters, text.galley.elided, text.visual_bounding_rect()));
            }
            _ => {}
        }
    }

    texts
}

/// Without its spaces (a wrapped text loses the one it breaks at).
fn packed(text: &str) -> String {
    text.chars().filter(|letter| !letter.is_whitespace()).collect()
}

#[test]
fn a_recruits_card_draws_its_markdown_its_errors_whole_and_opens_a_step_on_a_click_with_no_tip() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let id = recruit(&mut app);
    // after its task: a command longer than its row, its report, what the Maestro asked since and a tool that failed in JSON
    let mut failed = Step { id: "falhou".into(), name: "mcp__computer__type_text".into(), what: "Buscar".into(), ..Default::default() };
    let said = "{\"action\":\"type_text\",\"implemented\":true,\"message\":\"X11 input verification failed after typing; text may have been sent. Inspect the target before submitting or retrying.\"}";

    failed.finish(&Value::Null, true, said, Some(2.0));
    let transcript = app.transcripts.get_mut(&id).unwrap();

    transcript.items.truncate(1);

    let step = Step {
        id: "longo".into(),
        name: "Bash".into(),
        what: COMMAND.chars().take(90).collect(),
        input: COMMAND.into(),
        ended: Some(1.0),
        ..Default::default()
    };

    transcript.items.extend([ChatItem::Tool(step), ChatItem::Agent(REPORT.into()), ChatItem::User("Agora **rode** os testes de `fila`.".into())]);
    transcript.items.push(ChatItem::Tool(failed));
    resize(&mut app, &id, Vec2::new(430.0, 580.0));
    let mut time = 10.0;

    for _ in 0..4 {
        shown(&mut app, &ctx, time, vec![]);
        time += 0.1;
    }

    let texts = shown(&mut app, &ctx, time, vec![]);
    let all: Vec<&str> = texts.iter().map(|(text, ..)| text.as_str()).collect();
    // the report and the Maestro's words in Markdown: no marks painted, nothing cut
    let joined = all.join("\n");

    for words in ["O que aconteceu:", "o clique na caixa", "Buscar", "--fechar", "rode", "fila"] {
        assert!(joined.contains(words), "{words}: {all:?}");
    }

    assert!(!joined.contains("**") && !joined.contains('`') && !joined.contains("- **"), "{all:?}");
    // the tool's failure on its own step, why in red after its action (its card has it whole): no line of its own
    assert!(texts.iter().any(|(text, ..)| text.starts_with("· X11 input")), "{all:?}");
    assert!(!joined.contains("Ferramenta falhou"), "{all:?}");
    assert!(texts.iter().all(|(text, cut, _)| !*cut || text.starts_with("php") || text.starts_with("· X11")), "only the steps' lines are cut: {all:?}");
    // the step: one line cut short, a chevron at its end; resting on it shows no tip with the rest. The card just tall
    // enough for all of it, so its card opened makes it scroll
    resize(&mut app, &id, Vec2::new(430.0, 360.0));

    for _ in 0..3 {
        shown(&mut app, &ctx, time, vec![]);
    }

    let texts = shown(&mut app, &ctx, time, vec![]);
    let (row_text, cut, row) = texts.iter().find(|(text, ..)| text.starts_with("php ~/.claude")).cloned().unwrap_or_else(|| panic!("{all:?}"));

    assert!(cut && row_text.ends_with('…'), "{row_text}");
    let at = row.center();

    shown(&mut app, &ctx, time, vec![egui::Event::PointerMoved(at)]);

    for _ in 0..15 {
        time += 0.1;
        let texts = shown(&mut app, &ctx, time, vec![]);

        assert!(!texts.iter().any(|(text, ..)| packed(text).contains(&packed(COMMAND))), "no tip with the whole command");
    }

    assert!(!ctx.memory(|memory| memory.areas().visible_layer_ids().iter().any(|layer| layer.order == egui::Order::Tooltip)), "no tip at all");

    // a click opens it under its row, easing open: the whole command, wrapped inside the card; the mini chat stays shut.
    // Its row stays under the pointer all along (the card, taller than its room now, doesn't run to its end)
    let row_at = |texts: &[(String, bool, Rect)]| {
        texts.iter().find(|(text, ..)| text.starts_with("php ~/.claude") && text.ends_with('…')).map(|(.., rect)| rect.center()).unwrap()
    };

    shown(&mut app, &ctx, time, vec![button(at, true)]);
    shown(&mut app, &ctx, time + 0.05, vec![button(at, false)]);
    let mut texts = Vec::new();

    for later in [0.06, 0.1, 0.15, 0.2, 0.3, 0.5, 1.0] {
        texts = shown(&mut app, &ctx, time + later, vec![]);
        let still = row_at(&texts);

        assert!((still - at).length() < 1.0, "{later} s after: the row moved from {at:?} to {still:?}");
    }

    assert!(app.chat_unfolded.contains("card:longo") && !app.mini.open && !app.island.chat_asked);
    let whole = texts.iter().find(|(text, ..)| packed(text) == packed(COMMAND)).unwrap_or_else(|| panic!("{texts:?}"));
    let inner = app.terminal_frame(&app.scene.elements[app.chat_node(&id).unwrap()]).0.shrink(10.0);

    assert!(!whole.1 && whole.2.height() > 30.0 && whole.2.right() <= inner.right(), "wrapped inside the card: {whole:?} {inner:?}");
    // a click on it again eases it shut
    time += 2.0;
    shown(&mut app, &ctx, time, vec![button(at, true)]);
    shown(&mut app, &ctx, time + 0.05, vec![button(at, false)]);
    shown(&mut app, &ctx, time + 0.1, vec![]);
    let texts = shown(&mut app, &ctx, time + 1.0, vec![]);

    assert!(!app.chat_unfolded.contains("card:longo") && !texts.iter().any(|(text, ..)| packed(text) == packed(COMMAND)));
    assert!((row_at(&texts) - at).length() < 1.0, "shut, the row still where it was");
    // opened again at the end of the conversation: the view holds still, past the easing too...
    time += 2.0;
    shown(&mut app, &ctx, time, vec![button(at, true)]);
    shown(&mut app, &ctx, time + 0.05, vec![button(at, false)]);

    for later in [0.1, 0.3, 0.6, 1.0, 1.5] {
        let still = row_at(&shown(&mut app, &ctx, time + later, vec![]));

        assert!((still - at).length() < 1.0, "{later} s after: the row moved from {at:?} to {still:?}");
    }

    // ...until something new comes in the conversation: then it follows the newest again, down to its end
    const NEWS: &str = "Rodei de novo: 14 passaram.";

    app.transcripts.get_mut(&id).unwrap().items.push(ChatItem::Agent(NEWS.into()));
    time += 2.0;
    let mut texts = Vec::new();

    for later in [0.0, 0.05, 0.1] {
        texts = shown(&mut app, &ctx, time + later, vec![]);
    }

    let news = texts.iter().find(|(text, ..)| text.starts_with("Rodei de novo")).map(|(.., rect)| *rect).unwrap_or_else(|| panic!("{texts:?}"));
    // the line pinned under the conversation ("Pronto"): the newest just over it, in sight
    let pinned = texts.iter().filter(|(text, ..)| text == "Pronto").map(|(.., rect)| *rect).max_by(|a, b| a.top().total_cmp(&b.top())).unwrap();

    assert!(news.bottom() <= pinned.top() && pinned.top() - news.bottom() < 30.0, "at the end: {news:?} over {pinned:?}");
    assert!(row_at(&texts).y < at.y - 1.0, "the view went down to it");
}

#[test]
fn a_chat_cards_picture_shows_the_last_answer_in_markdown() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let id = new_chat(&mut app, "claude");
    let mut transcript = Transcript::new(PathBuf::new());

    transcript.items.extend([ChatItem::User("e aí?".into()), ChatItem::Agent(REPORT.into())]);
    app.transcripts.insert(id.clone(), transcript);
    // at its usual size the card is its picture: the mascot, its state and the last message
    let element = app.scene.elements[app.chat_node(&id).unwrap()].clone();
    let rect = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(360.0, 180.0));

    assert!(!crate::app::chat::node_ui::chat_live(rect, 1.0, false));
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), ..Default::default() };
    let output = ctx.run_ui(raw, |ui| app.paint_chat_node(ui.painter(), &element, rect, 1.0));

    let texts: Vec<String> = output
        .shapes
        .into_iter()
        .filter_map(|clipped| if let egui::Shape::Text(text) = clipped.shape { Some(text.galley.text().to_string()) } else { None })
        .collect();

    let preview = texts.iter().find(|text| text.contains("O que aconteceu")).unwrap_or_else(|| panic!("{texts:?}"));

    assert!(preview.starts_with("Feito.\n• O que aconteceu: o clique na caixa Buscar"), "its lines, a list's bullets: {preview:?}");
    assert!(!preview.contains("**") && !preview.contains('`') && !preview.contains('*'), "{preview:?}");
}

/// The box of a chat's card on the canvas opens the / menu as the big chat's does, filtered as it is typed, and Tab
/// completes it.
#[test]
fn the_cards_box_opens_the_slash_menu_filtered_as_it_is_typed() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let id = new_chat(&mut app, "claude");

    resize(&mut app, &id, Vec2::new(430.0, 580.0));
    cards(&mut app, &ctx);
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique(("chat_card_box", id.as_str()))));

    let mut typed = |events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), events, ..Default::default() };
        let output = ctx.run_ui(raw, |_| app.chat_node_overlay(&ctx));

        output
            .shapes
            .iter()
            .filter_map(|shape| if let Shape::Text(text) = &shape.shape { Some(text.galley.text().to_string()) } else { None })
            .collect::<Vec<_>>()
    };

    typed(vec![egui::Event::Text("/".into())]);
    typed(vec![egui::Event::Text("c".into())]);
    let words = typed(vec![]);

    assert!(words.iter().any(|w| w == "/clear") && words.iter().any(|w| w == "/compact [instruções]"), "the menu: {words:?}");
    assert!(!words.iter().any(|w| w == "/login"), "only what holds a c: {words:?}");
    typed(vec![egui::Event::Key { key: Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    let draft = ctx.data(|d| d.get_temp::<String>(egui::Id::unique(("chat_card_draft", id.as_str()))));

    assert_eq!(draft.as_deref(), Some("/clear "), "Tab completes the first");
}
