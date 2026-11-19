//! The island's conversation with Luna and her agents (chat/mini_agents.rs, island/prompt.rs): who it is with, named in
//! a row of its own; the Agentes button in its own room, showing an agent in the island and never in a window over the
//! screen; the hand over every button, whose press is never the window's drag; what an agent says, signed.

use super::*;
use crate::app::chat::direct::NewChat;
use crate::app::island::Tab;
use crate::transcript::{ChatItem, Transcript};

/// Luna with two recruits in chat mode and a conversation said; Luna's id and her agents'.
fn luna_with_two(app: &mut DrawingApp) -> (String, Vec<String>) {
    app.prefs.island = true;
    app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &[Preset::new("Claude Code", "claude")]);
    let luna = app.chat_agent.clone().unwrap();

    for task in ["claude \"revise o login\"", "claude \"rode os testes\""] {
        let (reply, said) = std::sync::mpsc::channel();

        app.apply_agent_command(UiCommand::Recruit { from: luna.clone(), command: task.into(), worktree: false, mode: Some(AgentMode::Chat), reply });
        assert!(said.recv().unwrap().is_ok());
    }

    let mut transcript = Transcript::new(PathBuf::new());

    transcript.items.push(ChatItem::User("recruta dois agentes para revisar o login e rodar os testes, e passa a lista por favor".into()));
    transcript.items.push(ChatItem::Agent("Pronto: recrutei dois agentes.\n\n- **Revisor**: lendo o login.\n- **Testes**: rodando a suíte.".into()));
    app.transcripts.insert(luna.clone(), transcript);
    app.chat_agent = Some(luna.clone());
    let agents = app.chats.iter().filter(|chat| chat.recruit).map(|chat| chat.id.clone()).collect();

    (luna, agents)
}

/// What a frame of the island painted (each text and where) and the cursor it asked for.
struct Frame {
    texts: Vec<(String, Rect)>,
    cursor: CursorIcon,
}

impl Frame {
    fn place_of(&self, text: &str) -> Option<Rect> {
        self.texts.iter().find(|(painted, _)| painted == text).map(|(_, at)| *at)
    }

    fn says(&self, text: &str) -> bool {
        self.texts.iter().any(|(painted, _)| painted.contains(text))
    }
}

/// The app and its island open on the conversation, the pointer resting on it.
struct Island {
    app: DrawingApp,
    ctx: Context,
    time: f64,
    luna: String,
    agents: Vec<String>,
}

impl Island {
    fn open(full: bool) -> Island {
        let ctx = Context::default();

        install_fonts(&ctx, false);
        let mut app = DrawingApp::default();
        let (luna, agents) = luna_with_two(&mut app);

        app.island_show_page(Tab::Chat);
        app.island.chat_full = full;
        let mut island = Island { app, ctx, time: 1.0, luna, agents };

        island.rest(Pos2::new(ISLAND_SCREEN.x / 2.0, 100.0), 2.5);
        island
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> Frame {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, ISLAND_SCREEN)), time: Some(self.time), events, ..Default::default() };
        let mut output = self.ctx.run_ui(raw, |_| self.app.island_ui(&self.ctx));

        output.textures_delta.clear();
        self.time += 0.05;

        let texts = output
            .shapes
            .iter()
            .filter_map(
                |shape| if let egui::Shape::Text(text) = &shape.shape { Some((text.galley.text().to_string(), text.visual_bounding_rect())) } else { None },
            )
            .collect();

        Frame { texts, cursor: output.platform_output.cursor_icon }
    }

    /// The pointer resting at `at` for `seconds`; the last frame.
    fn rest(&mut self, at: Pos2, seconds: f64) -> Frame {
        let end = self.time + seconds;
        let mut last = self.frame(vec![egui::Event::PointerMoved(at)]);

        while self.time < end {
            last = self.frame(vec![egui::Event::PointerMoved(at)]);
        }

        last
    }

    /// A click as the hand makes it; then the island settles.
    fn click(&mut self, at: Pos2) -> Frame {
        self.rest(at, 0.1);
        self.frame(vec![button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.rest(at, 0.6)
    }

    /// The name of chat `id`.
    fn name(&self, id: &str) -> String {
        self.app.chats.iter().find(|chat| chat.id == id).unwrap().name.clone()
    }
}

/// In the island's conversation, compact or whole, the Agentes button sits in a room of its own, over no word of the
/// conversation; an agent picked in it shows in the island itself, and no mini chat opens over the screen; the way back to
/// Luna does the same.
#[test]
fn the_agents_button_has_its_own_room_and_shows_an_agent_in_the_island_not_in_a_window() {
    for full in [false, true] {
        let mut island = Island::open(full);
        let frame = island.rest(Pos2::new(ISLAND_SCREEN.x / 2.0, 100.0), 0.2);
        let button = frame.place_of("Agentes").unwrap_or_else(|| panic!("the button says what it is ({full}): {:?}", frame.texts));

        assert!(frame.place_of("2").is_some_and(|count| (count.center().y - button.center().y).abs() < 2.0), "and how many");
        // the conversation's words are under the button's row, never under the button
        let said = frame.place_of("Pronto: recrutei dois agentes.").expect("Luna's answer");

        assert!(said.top() > button.bottom(), "{full}: the log starts under the button's row ({said:?} under {button:?})");

        for (text, at) in &frame.texts {
            assert!(text == "Agentes" || text == "2" || !at.intersects(button.expand(4.0)), "{full}: {text:?} at {at:?} under the button {button:?}");
        }

        // its menu names each agent and what it does; one picked shows here
        let menu = island.click(button.center());
        let first = island.name(&island.agents[0]);
        let row = menu.texts.iter().find(|(text, _)| text.starts_with(&format!("{first} · "))).map(|(_, at)| *at);
        let row = row.unwrap_or_else(|| panic!("{full}: the menu names {first}: {:?}", menu.texts));
        let shown = island.click(row.center());

        assert_eq!(island.app.chat_agent.as_deref(), Some(island.agents[0].as_str()), "{full}: the agent in the conversation");
        assert_eq!(island.app.island.tab(), Tab::Chat, "{full}: still the island's conversation");
        assert!(!island.app.mini.open, "{full}: no mini chat opened over the screen for it");
        assert!(shown.says(&first), "{full}: its name shows: {:?}", shown.texts);
        // the way back, by Luna's name
        let back = shown.place_of("Luna").unwrap_or_else(|| panic!("{full}: the way back says Luna: {:?}", shown.texts));

        island.click(back.center());
        assert_eq!(island.app.chat_agent.as_deref(), Some(island.luna.as_str()), "{full}: back to Luna");
        assert!(!island.app.mini.open, "{full}: still no mini chat over the screen");
    }
}

/// The island's conversation says over its log who it is with: Luna (her crown, her chat's title) or the agent by
/// its own name.
#[test]
fn the_islands_conversation_names_who_it_is_with() {
    let mut island = Island::open(false);
    let frame = island.rest(Pos2::new(ISLAND_SCREEN.x / 2.0, 100.0), 0.2);
    let luna = frame.place_of("Luna").expect("Luna named");

    assert!(frame.says("projeto · Pronto"), "her chat and how she is: {:?}", frame.texts);
    let said = frame.place_of("Pronto: recrutei dois agentes.").unwrap();

    assert!(luna.bottom() < said.top(), "the name over the log");
    let agent = island.agents[1].clone();

    island.app.chat_agent = Some(agent.clone());
    let frame = island.rest(Pos2::new(ISLAND_SCREEN.x / 2.0, 100.0), 0.3);
    let name = island.name(&agent);
    let named = frame.place_of(&name).unwrap_or_else(|| panic!("the agent by its name {name}: {:?}", frame.texts));

    assert!((named.center().y - luna.center().y).abs() < 2.0, "in the same row");
    // another agent's question in Luna's log says whose it is, not the bridge's wording
    let question = crate::app::chat::ask::asked_by("Pergunta de \"Ana\" (a sua resposta volta para quem perguntou):\n\nposso subir?");

    assert_eq!(question, Some(("Ana", "posso subir?")));
    island.app.chat_agent = Some(island.luna.clone());
    let luna_id = island.luna.clone();

    island
        .app
        .transcripts
        .get_mut(&luna_id)
        .unwrap()
        .items
        .push(ChatItem::User("Pergunta de \"Ana\" (a sua resposta volta para quem perguntou):\n\nposso subir?".into()));

    let frame = island.rest(Pos2::new(ISLAND_SCREEN.x / 2.0, 100.0), 0.3);

    assert!(frame.place_of("Pergunta de Ana").is_some() && frame.place_of("posso subir?").is_some(), "{:?}", frame.texts);
}

/// Over every button of the island's header and of its conversation the hand shows, and a press on one that moves a
/// little stays the button's: never the drag that takes the island's window (or the mini chat's header) away. No tip
/// covers the conversation.
#[test]
fn the_islands_buttons_show_the_hand_and_their_press_never_drags_the_window() {
    for full in [false, true] {
        let mut island = Island::open(full);
        let frame = island.rest(Pos2::new(ISLAND_SCREEN.x / 2.0, 100.0), 0.2);
        let rect = island_rect(&island.app);
        let agents = frame.place_of("Agentes").expect("the agents' button").center();
        let mut buttons = vec![("Início", Pos2::new(rect.left() + 29.0, rect.top() + 25.0)), ("Agentes", agents)];

        buttons.extend((0..RIGHT_BUTTONS).map(|index| ("à direita", right_button(&island.app, index))));

        if !full {
            buttons.push(("⤢", Pos2::new(rect.right() - 10.0 - 18.0, rect.top() + 42.0 + 18.0)));
        }

        for (name, at) in buttons {
            let rested = island.rest(at, 1.0);

            assert_eq!(rested.cursor, CursorIcon::PointingHand, "{full}: the hand over {name} at {at:?}");

            for tip in ["Chat atual", "Início: o resumo", "Ajustes da ilha", "Recolher", "Abrir o chat completo", "Ver os agentes da Luna"] {
                assert!(rested.place_of(tip).is_none(), "{full}: no tip ({tip}) over the page");
            }

            // pressed, then moved past a click: the drag is the button's, not the window's
            island.frame(vec![button(at, true)]);
            island.frame(vec![egui::Event::PointerMoved(at + Vec2::new(14.0, 3.0))]);
            island.frame(vec![egui::Event::PointerMoved(at + Vec2::new(16.0, 4.0))]);
            let dragged = island.ctx.dragged_id().and_then(|id| island.ctx.read_response(id)).map(|response| response.rect);

            assert!(dragged.is_none_or(|rect| rect.contains(at) && rect.width() < 200.0), "{full}: {name}'s press dragged {dragged:?}");
            island.frame(vec![button(at + Vec2::new(16.0, 4.0), false)]);
            island.rest(at, 0.2);
            // the menu a press opened goes, for the next button
            island.frame(vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);

            if island.app.island.tab() != Tab::Chat || island.app.island.size().x < crate::app::island::EXPANDED_W - 1.0 {
                // a tab or the close took the press as a click after all: open again on the conversation
                island.app.island_show_page(Tab::Chat);
                island.app.island.chat_full = full;
                island.rest(Pos2::new(ISLAND_SCREEN.x / 2.0, 100.0), 2.0);
            }
        }
    }
}

/// What an agent sends to another chat by the MCP's chat_send goes signed with its name, once; the person's own (no asker)
/// and a chat's to itself as they are. `chat_open` with place island shows the chat in the island's conversation, as the
/// Agentes button does.
#[test]
fn an_agents_message_to_a_chat_goes_signed_and_chat_open_shows_one_in_the_island() {
    let mut app = DrawingApp::default();
    let (luna, agents) = luna_with_two(&mut app);
    let name = app.chats.iter().find(|chat| chat.id == agents[0]).unwrap().name.clone();

    app.act("chat_send", &json!({ "chat": "projeto", "text": "estou parado esperando a chave", "asker": agents[0] })).unwrap();
    app.act("chat_send", &json!({ "chat": "projeto", "text": format!("{name} aqui: terminei"), "asker": agents[0] })).unwrap();
    app.act("chat_send", &json!({ "chat": "projeto", "text": "roda os testes" })).unwrap();
    app.act("chat_send", &json!({ "chat": name, "text": "pode subir", "asker": luna })).unwrap();
    let said: Vec<String> = app.chat_outbox.iter().map(|(_, text)| text.clone()).collect();

    assert_eq!(said, [format!("{name}: estou parado esperando a chave"), format!("{name} aqui: terminei"), "roda os testes".into()]);
    // Luna's to the agent she recruited goes in its line as her question, whose answer comes back to her (chat/ask.rs)
    let asked: Vec<String> = app.chat_asks.iter().map(|ask| ask.prompt()).collect();

    assert_eq!(asked, ["Pergunta de \"Luna\" (a sua resposta volta para quem perguntou):\n\npode subir".to_string()]);
    // a notice by the bridge's notify says who too, once
    app.notify(&agents[0], "preciso de você");
    assert_eq!(app.status, format!("{name}: preciso de você"));
    app.notify(&agents[0], &format!("{name} aqui: terminei"));
    assert_eq!(app.status, format!("{name} aqui: terminei"));
    // shown in the island, not in the app's chat screen
    app.chat_open = false;
    app.act("chat_open", &json!({ "chat": name, "place": "island" })).unwrap();
    assert_eq!((app.chat_agent.as_deref(), app.island.tab(), app.chat_open), (Some(agents[0].as_str()), Tab::Chat, false));
    app.prefs.island = false;
    assert!(app.act("chat_open", &json!({ "chat": "projeto", "place": "island" })).is_err(), "no island on: said so");
}

/// The island's own box opens the / menu as it is typed, filtered; Esc closes the menu, not the island, and Enter
/// completes the picked command instead of sending it.
#[test]
fn the_islands_box_opens_the_slash_menu_and_esc_closes_only_the_menu() {
    let mut island = Island::open(false);
    let at = Pos2::new(ISLAND_SCREEN.x / 2.0, 100.0);

    island.ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("chat_draft")));
    island.frame(vec![egui::Event::Text("/".into())]);
    island.frame(vec![egui::Event::Text("c".into())]);
    let frame = island.rest(at, 0.1);

    assert!(frame.place_of("/clear").is_some() && frame.place_of("/compact [instruções]").is_some(), "the menu: {:?}", frame.texts);
    assert!(frame.place_of("/login").is_none(), "only what holds a c");
    let key = |key: Key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };

    island.frame(vec![key(Key::Escape)]);
    let frame = island.rest(at, 0.3);

    assert!(frame.place_of("/clear").is_none(), "Esc closed the menu");
    let still = island.app.island.size().x >= crate::app::island::EXPANDED_W && island.app.island.tab() == Tab::Chat;

    assert!(still && island.app.chat_draft == "/c", "and only the menu: {:?}", island.app.island.size());
    // typed again (egui lets go of the box at an Esc), Enter takes the first command into the box, nothing is sent
    island.ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("chat_draft")));
    island.frame(vec![egui::Event::Text("l".into())]);
    island.rest(at, 0.1);
    let before = island.app.transcripts[&island.luna].items.len();

    island.frame(vec![key(Key::Enter)]);
    island.rest(at, 0.1);
    assert_eq!(island.app.chat_draft, "/clear ");
    assert_eq!(island.app.transcripts[&island.luna].items.len(), before, "not sent");
}
