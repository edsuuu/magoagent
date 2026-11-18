//! The messages queued for an agent (chat/queue.rs): their bubbles inside the chat with the lines at the left, Enviar
//! agora by its button, its keys and the MCP, and Editar.

use super::*;
use crate::app::chat::direct::{ChatRun, NewChat};
use crate::app::chat::list::Look;
use crate::app::chat::mini::ConvOut;
use crate::app::hotkeys::local::AppAction;
use crate::transcript::ChatItem;

/// A chat as the form makes it: its id.
fn new_chat(app: &mut DrawingApp) -> String {
    app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &[Preset::new("Claude Code", "claude")]);
    app.chat_agent.clone().unwrap()
}

/// A process that runs until it is stopped, standing in for the chat's agent answering.
fn answering(app: &mut DrawingApp, id: &str) {
    let child = if cfg!(windows) {
        crate::editor::command("powershell").args(["-NoProfile", "-Command", "Start-Sleep 60"]).spawn()
    } else {
        std::process::Command::new("sleep").arg("60").spawn()
    };
    app.chat_runs.insert(id.to_string(), ChatRun::stand_in(child.unwrap(), 0.0));
}

/// What a frame painted: each text with where it went, and the boxes (a bubble's is one).
struct Painted {
    texts: Vec<(std::sync::Arc<egui::Galley>, Rect)>,
    boxes: Vec<Rect>,
}

impl Painted {
    fn place_of(&self, text: &str) -> Option<Rect> {
        self.texts.iter().find(|(galley, _)| galley.text() == text).map(|(_, at)| *at)
    }
}

/// One frame of the conversation of the chat open, scrolled in a column `width` wide from x = 100 as the chat's views
/// show it, and what its buttons asked applied after it.
fn conversation_frame(app: &mut DrawingApp, ctx: &Context, width: f32, events: Vec<egui::Event>) -> Painted {
    let id = app.chat_agent.clone().unwrap();
    let row = app.agent_rows().into_iter().find(|row| row.id == id).unwrap();
    let mut out = ConvOut::default();
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 2000.0))), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |ui| {
        let c = app.colors();
        let look = Look::new(true, &c);
        let column = Rect::from_min_size(Pos2::new(100.0, 0.0), Vec2::new(width, 2000.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(column), |ui| {
            ui.set_max_width(width);
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                let transcript = app.transcripts.get(&id);
                app.conversation(ui, &row, transcript, true, true, false, &[], None, width, &look, &c, width < 500.0, 0.0, &mut out);
            });
        });
    });
    output.textures_delta.clear();
    app.conv_apply(ctx, out, egui::Id::unique("chat_draft"));
    let mut painted = Painted { texts: vec![], boxes: vec![] };
    for shape in output.shapes {
        match shape.shape {
            Shape::Text(text) => painted.texts.push((text.galley.clone(), text.visual_bounding_rect())),
            Shape::Rect(rect) => painted.boxes.push(rect.rect),
            _ => {}
        }
    }
    painted
}

#[test]
fn a_queued_and_a_sent_message_stay_inside_the_chat_with_their_lines_at_the_left() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    let id = new_chat(&mut app);
    // a long message with a line of its own, as in the owner's print (the mini chat cut its second line off at the left)
    let long = "Pode criar o webhook do WhatsApp para mandar as mensagens e seguir no meu número\nE no número da IA também, por favor, com o mesmo texto e os mesmos anexos de antes";
    app.chat_transcript(&id).items = vec![ChatItem::User(long.into()), ChatItem::Agent("ok".into())];
    app.queue_message(&id, long.into());
    // the chat screen, the mini chat's window (about 510 wide), the mini chat narrowed and the island
    for width in [900.0, 486.0, 316.0, 280.0] {
        let painted = conversation_frame(&mut app, &ctx, width, vec![]);
        let column = 100.0 - 0.5..=100.0 + width + 0.5;
        let bubbles: Vec<_> = painted.texts.iter().filter(|(galley, _)| galley.text() == long).collect();
        assert_eq!(bubbles.len(), 2, "the sent message and the queued one, at {width}");
        for (galley, at) in bubbles {
            assert!(galley.rows.len() >= if width < 500.0 { 3 } else { 2 }, "it wraps at {width}: {} rows", galley.rows.len());
            assert!(galley.rows.iter().all(|row| row.pos.x.abs() < 0.5), "every line starts at the bubble's left at {width}");
            assert!(column.contains(&at.left()) && column.contains(&at.right()), "the words inside the column at {width}: {at:?}");
            // its bubble, around them, too; at the right
            let bubble = painted.boxes.iter().find(|bubble| bubble.contains_rect(*at)).expect("its bubble");
            assert!(column.contains(&bubble.left()) && column.contains(&bubble.right()), "the bubble inside the column at {width}: {bubble:?}");
            assert!(bubble.right() > 100.0 + width - 40.0, "at the right at {width}: {bubble:?}");
        }
        // the row over the queued one: its buttons, and when it goes when that fits
        let head = ["Editar", "Enviar agora"].map(|text| painted.place_of(text).expect(text));
        assert!(head.iter().all(|at| column.contains(&at.left()) && column.contains(&at.right())), "{head:?}");
        let when = painted.place_of("Na fila · vai quando ele terminar ·").or(painted.place_of("Na fila ·")).expect("when it goes");
        assert!(column.contains(&when.left()) && when.bottom() < head[0].bottom() + 4.0, "{when:?} on the buttons' row");
    }
}

#[test]
fn enviar_agora_stops_the_chats_answer_and_the_queued_message_goes_next() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    let id = new_chat(&mut app);
    answering(&mut app, &id);
    app.queue_message(&id, "e os testes?".into());
    let painted = conversation_frame(&mut app, &ctx, 700.0, vec![]);
    let at = painted.place_of("Enviar agora").expect("the queued bubble has Enviar agora").center();
    assert!(painted.place_of("Na fila · vai quando ele terminar ·").is_some());
    conversation_frame(&mut app, &ctx, 700.0, vec![egui::Event::PointerMoved(at), button_event(at, true)]);
    conversation_frame(&mut app, &ctx, 700.0, vec![button_event(at, false)]);
    // the answer stopped; the queue goes on the next frame, the chat being free (`chat_nodes_tick`)
    assert!(!app.chat_runs.contains_key(&id), "the answer was stopped");
    assert_eq!(app.chat_queue, [(id.clone(), "e os testes?".to_string())]);
    // nothing queued: said so
    app.chat_queue.clear();
    assert!(app.send_queued_now(&id).unwrap_err().contains("Nada na fila"));
}

fn button_event(at: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

#[test]
fn ctrl_shift_enter_in_the_box_sends_now_what_enter_would_queue() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    let id = new_chat(&mut app);
    answering(&mut app, &id);
    app.chat_open = true;
    let size = Vec2::new(1200.0, 800.0);
    let run = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| {
            app.screen_keys(&ctx);
            app.chat_screen(&ctx);
        });
        output.textures_delta.clear();
    };
    run(&mut app, vec![]);
    app.chat_draft = "faz agora".into();
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("chat_draft")));
    let keys = egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND | Modifiers::SHIFT };
    run(&mut app, vec![keys.clone()]);
    // queued as Enter would, then the answer stopped for it: it goes next
    assert_eq!((app.chat_draft.as_str(), app.chat_queue.clone()), ("", vec![(id.clone(), "faz agora".to_string())]));
    assert!(!app.chat_runs.contains_key(&id));
    // the keys are Ajustes'; Ctrl+S stays Salvar and Ctrl+X recortar
    assert_eq!(app.app_keys(&[keys]), [AppAction::SendNow]);
    let def = crate::app::hotkeys::local::def_of(AppAction::SendNow);
    let stored = crate::app::hotkeys::Bindings::new();
    let check = |keys: &str| crate::app::hotkeys::editor::check(def, &crate::app::hotkeys::parse_keys(keys).unwrap(), &stored);
    assert_eq!(check("Ctrl+S"), Err(crate::app::hotkeys::editor::Problem::Taken("save")));
    assert!(matches!(check("Ctrl+X"), Err(crate::app::hotkeys::editor::Problem::Reserved(_))));
    assert_eq!(check("Ctrl+Shift+Enter"), Ok(()));
}

#[test]
fn enviar_agora_answers_a_waiting_terminal_with_esc_and_the_words_follow() {
    let dir = std::env::temp_dir().join(format!("mago-agora-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let saved = dir.join("linha.txt");
    // a stand-in agent on a menu: saves the line typed into it
    let script = if cfg!(windows) {
        format!("$l = [Console]::ReadLine(); Set-Content -LiteralPath '{}' $l; Start-Sleep 30", saved.display())
    } else {
        format!("read l; printf '%s' \"$l\" > '{}'; sleep 30", saved.display())
    };
    let program = if cfg!(windows) { ("powershell", &["-NoProfile", "-Command", script.as_str()][..]) } else { ("sh", &["-c", script.as_str()][..]) };
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(program), 10_000).unwrap();
    let mut app = DrawingApp::default();
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    terminal.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    let id = terminal.id.clone();
    app.scene.elements.push(terminal);
    app.terminals.insert(id.clone(), Ok(session));
    app.terminal_waiting.insert(id.clone());
    app.queue_message(&id, "e os testes?".into());
    // by the MCP, as the button does
    assert!(app.act("chat_queue", &json!({ "agent": "Revisor" })).unwrap().contains("e os testes?"));
    assert!(app.act("chat_queue", &json!({ "agent": "Revisor", "action": "send_now" })).unwrap().starts_with("Esc no terminal"));
    // its menu gone (the screen no longer shows it), the words go
    app.terminal_waiting.clear();
    app.flush_chat_queue();
    assert!(app.chat_queue.is_empty());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while fs::read_to_string(&saved).map_or(true, |s| s.trim().is_empty()) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let line = fs::read_to_string(&saved).unwrap();
    assert!(line.trim_end().ends_with("e os testes?"), "{line:?}");
    if !cfg!(windows) {
        assert!(line.starts_with('\u{1b}'), "Esc reached the terminal first: {line:?}");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_mcp_lists_and_edits_the_queue() {
    let mut app = DrawingApp::default();
    let id = new_chat(&mut app);
    assert_eq!(app.act("chat_queue", &json!({})).unwrap(), "Nada na fila.");
    app.queue_message(&id, "depois".into());
    assert!(app.act("chat_queue", &json!({ "action": "list" })).unwrap().ends_with(": depois"));
    assert!(app.act("chat_queue", &json!({ "action": "voar" })).is_err());
    app.chat_draft = "já escrito".into();
    app.act("chat_queue", &json!({ "action": "edit" })).unwrap();
    assert!(app.chat_queue.is_empty());
    assert_eq!(app.chat_draft, "depois\njá escrito", "back in the box, ahead of what was there");
    assert!(app.act("chat_queue", &json!({ "action": "send_now" })).unwrap_err().contains("Nada na fila"));
}
