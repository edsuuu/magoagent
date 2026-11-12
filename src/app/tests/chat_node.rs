//! A chat's card on the canvas (chat_node.rs): born with its chat, gone with it, roped like a window.

use super::*;
use crate::app::chat::direct::NewChat;
use crate::app::ui::context::{Entry, Target};
use crate::bridge::NodeKind;

/// A chat as the form makes it, of an agent called `name` run by `command`: its id.
fn new_chat(app: &mut DrawingApp, name: &str, command: &str) -> String {
    let installed = [Preset { name: name.into(), command: command.into(), on: true }];
    app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &installed);
    app.chat_agent.clone().unwrap()
}

/// The ids of a menu's rows, its submenus' within brackets.
fn ids(rows: &[Entry]) -> Vec<String> {
    rows.iter()
        .map(|row| match row {
            Entry::Item { id, .. } => id.clone(),
            Entry::Sub { label, items } => format!("{label}[{}]", ids(items).join(" ")),
            Entry::Sep => "-".into(),
        })
        .collect()
}

#[test]
fn a_new_chat_has_its_card_on_the_canvas_and_each_goes_as_asked() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = new_chat(&mut app, "Claude Code", "claude");
    // the card: a window with the chat's own id, selected, in the middle of the view
    let card = &app.scene.elements[0];
    assert_eq!((app.scene.elements.len(), card.id.as_str(), card.is_node(), app.selected.as_slice()), (1, id.as_str(), true, &[0][..]));
    assert_eq!(card.chat, Some(ChatSpec { name: "Claude Code · projeto".into(), agent: "claude".into(), cwd: "~/projeto".into() }));
    assert_eq!(element_bounds(card).center(), app.to_world(app.canvas_rect.center()));
    // the next chat's lands beside it, not over it
    let other = new_chat(&mut app, "Codex", "codex");
    let (first, second) = (element_bounds(&app.scene.elements[0]), element_bounds(&app.scene.elements[1]));
    assert!(!first.intersects(second) && (second.center() - first.center()).length() < 400.0, "{first:?} {second:?}");
    // and a window under the middle of the view does not push a card out of sight: it goes beside it
    let mut window = app.new_element(ElementKind::Rectangle { x: -1000.0, y: -3000.0, width: 2000.0, height: 3200.0 });
    window.terminal = Some(TerminalSpec::default());
    let spot = {
        app.scene.elements.push(window);
        let spot = app.chat_spot(Pos2::new(0.0, -400.0));
        app.scene.elements.pop();
        spot
    };
    assert!(spot.min.y > 200.0 && spot.center().x == 0.0 && spot.max.y < 600.0, "just under the window: {spot:?}");
    // the card deleted: the conversation stays in the list, and nothing brings the card back
    app.selected = vec![0];
    app.delete_selected();
    app.chat_nodes_tick(&ctx);
    assert!(app.chat_node(&id).is_none() && app.agent_rows().iter().any(|r| r.id == id), "the chat outlives its card");
    // Mostrar no canvas: its card again, one however many times, selected, on the canvas
    app.chat_open = true;
    assert!(app.show_chat_on_canvas(&id).unwrap().contains("agora tem um cartão"));
    assert!(app.show_chat_on_canvas(&id).unwrap().starts_with("Canvas no cartão"));
    assert_eq!(app.scene.elements.iter().filter(|e| e.id == id).count(), 1);
    assert_eq!((app.selected.clone(), app.chat_open), (vec![app.chat_node(&id).unwrap()], false));
    // a copy would be a second card of one conversation: there is none
    app.duplicate();
    assert_eq!(app.scene.elements.len(), 2);
    // the chat removed: its card goes with it, the other chat's stays
    app.remove_chat(&id);
    assert!(app.chat_node(&id).is_none() && app.chat_node(&other).is_some());
    assert!(app.show_chat_on_canvas(&id).is_err());
    // an undo that brings the card back finds no chat for it: gone on the next frame
    app.undo();
    app.undo();
    assert!(app.chat_node(&id).is_some(), "the undo restored the drawing as it was");
    app.chat_nodes_tick(&ctx);
    assert!(app.chat_node(&id).is_none() && app.chat_node(&other).is_some());
}

#[test]
fn a_chat_from_before_gets_its_card_from_the_canvas_menu() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // as chats.json had it before there were cards
    app.chats.push(serde_json::from_value(json!({ "id": "velho", "agent": "codex", "name": "Velho", "cwd": "~/x" })).unwrap());
    app.chat_nodes_tick(&ctx);
    assert!(app.scene.elements.is_empty(), "no card on its own");
    // Adicionar › Chat: a new one, or one of the list with no card here
    assert_eq!(ids(&[app.chat_add_entry()]), ["Chat[add_chat - chat_node:velho]"]);
    let at = Pos2::new(120.0, 80.0);
    app.context_action(&ctx, &Target::Canvas(at), "chat_node:velho");
    let card = app.chat_node("velho").unwrap();
    assert_eq!((app.scene.elements[card].center(), app.selected.clone()), (at, vec![card]), "where the menu opened");
    assert_eq!(ids(&[app.chat_add_entry()]), ["add_chat"], "every chat is on the canvas: a plain row");
    // the card says what chats.json says, renamed there
    app.chats[0].name = "Novo nome".into();
    app.chat_nodes_tick(&ctx);
    assert_eq!(app.scene.elements[card].chat.as_ref().unwrap().name, "Novo nome");
    // the card's last message: whose it is, on one line; an agent's question is that agent's
    use crate::transcript::ChatItem;
    assert_eq!(app.chat_preview("velho"), None);
    app.chat_transcript("velho").items.extend([ChatItem::User("oi,\n tudo bem?".into()), ChatItem::Agent("tudo  certo".into())]);
    assert_eq!(app.chat_preview("velho").as_deref(), Some("tudo certo"));
    app.chat_transcript("velho").items.push(ChatItem::User("Pergunta de \"Dev\" (a sua resposta volta para quem perguntou):\n\nquanto é 2+2?".into()));
    assert_eq!(app.chat_preview("velho").as_deref(), Some("Dev: quanto é 2+2?"));
    app.chat_transcript("velho").items.push(ChatItem::User("oi,\n tudo bem?".into()));
    assert_eq!(app.chat_preview("velho").as_deref(), Some("Você: oi, tudo bem?"));
    // its own menu: it opens the chat, has no copy, and deleting says what it takes
    let rows = app.context_entries(&Target::Element("velho".into()));
    assert_eq!(ids(&rows), ["open_chat", "open_mini_chat", "-", "lock", "front", "back", "-", "delete"]);
    assert!(rows.iter().any(|r| matches!(r, Entry::Item { id, label, .. } if id == "delete" && label == "Tirar do canvas")));
    app.context_action(&ctx, &Target::Element("velho".into()), "open_chat");
    assert_eq!((app.chat_open, app.chat_agent.as_deref()), (true, Some("velho")));
    // the drawing's own format carries the card (the clipboard, the MCP's view of the canvas)
    let json = Value::Array(interop_elements(&app.scene.elements));
    assert_eq!(json[0]["type"], "chat");
    let back = from_interop_elements(&json);
    assert_eq!((back[0].id.as_str(), &back[0].chat), ("velho", &app.scene.elements[card].chat));
}

#[test]
fn a_double_click_on_the_card_opens_its_chat_and_a_drag_anywhere_on_it_moves_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = new_chat(&mut app, "Claude Code", "claude");
    app.selected.clear();
    // its body is no terminal's screen: a drag there carries the card
    let before = element_bounds(&app.scene.elements[0]);
    let on = app.to_screen(before.center());
    drag(&mut app, &ctx, on, on + Vec2::new(80.0, 40.0));
    let after = element_bounds(&app.scene.elements[0]);
    assert_eq!((after.min - before.min, after.size()), (Vec2::new(80.0, 40.0), before.size()));
    assert!(!app.chat_open);
    app.chat_agent = None;
    let on = app.to_screen(after.center());
    double_click(&mut app, &ctx, on, 10.0);
    assert_eq!((app.chat_open, app.chat_agent), (true, Some(id)));
}

#[test]
fn a_rope_between_a_chat_and_a_terminal_is_what_the_bridge_lists_and_the_apps_tools_cut_it() {
    let mut app = DrawingApp { bridge: Bridge::start(None).ok(), ..Default::default() };
    let chat = new_chat(&mut app, "Claude Code", "claude");
    let mut dev = app.new_element(ElementKind::Rectangle { x: 600.0, y: 0.0, width: 200.0, height: 120.0 });
    dev.terminal = Some(TerminalSpec { name: "Dev".into(), ..Default::default() });
    let dev = {
        app.scene.elements.push(dev);
        app.scene.elements[1].id.clone()
    };
    app.connect(&chat, &dev);
    let shared = app.bridge.as_ref().unwrap().shared.clone();
    let (sender, _commands) = std::sync::mpsc::channel();
    let list = |from: &str| crate::bridge::handle(&json!({ "cmd": "list", "from": from }), &shared, &sender).unwrap();
    // on the canvas the chat is the Maestro: an agent like a terminal, called so, that commands what it is roped to
    let nodes = app.agent_nodes();
    let node = nodes.iter().find(|n| n.id == chat).unwrap();
    assert_eq!((node.kind, node.name.as_str(), node.maestro, &node.links), (NodeKind::Chat, "Maestro", true, &vec![dev.clone()]));
    app.publish_nodes();
    assert_eq!(list(&dev), "Você: \"Dev\"\nAgentes conectados:\n  - \"Maestro\" (chat, parado)");
    assert_eq!(list(&chat), "Você: \"Maestro\"\nAgentes conectados:\n  - \"Dev\"");
    // a second chat on the canvas: each Maestro says which chat it is
    let other = new_chat(&mut app, "Codex", "codex");
    app.connect(&other, &chat);
    app.publish_nodes();
    assert_eq!(list(&dev), "Você: \"Dev\"\nAgentes conectados:\n  - \"Maestro (Claude Code · projeto)\" (chat, parado)");
    let seen = list(&chat);
    assert!(seen.contains("\"Dev\"") && seen.contains("\"Maestro (Codex · projeto)\" (chat, parado)"), "{seen}");
    assert!(!list(&other).contains("\"Dev\""), "a chat roped reaches only what its ropes reach");
    // the MCP's disconnect cuts one rope, by the names list shows
    let pair = json!({ "a": "Dev", "b": "Maestro (Claude Code · projeto)" });
    assert!(app.act("disconnect", &pair).unwrap().contains("desconectados"));
    assert_eq!(app.links()[&chat], vec![other.clone()]);
    assert!(app.act("disconnect", &pair).unwrap_err().contains("não estão conectados"));
    assert!(app.act("disconnect", &json!({ "a": "Dev", "b": "ninguém" })).is_err());
    // chat_node: the card off the canvas (the chat stays), and back
    assert!(app.act("chat_node", &json!({ "chat": "Claude Code", "action": "remove" })).unwrap().contains("a conversa continua"));
    assert!(app.chat_node(&chat).is_none() && app.is_chat(&chat));
    assert!(app.act("chat_node", &json!({ "chat": "Claude Code", "action": "remove" })).unwrap_err().contains("não tem cartão"));
    assert!(app.act("chat_node", &json!({ "chat": "Claude Code" })).unwrap().contains("agora tem um cartão"));
    assert!(app.chat_node(&chat).is_some());
    assert!(app.act("chat_node", &json!({ "chat": "Claude Code", "action": "voar" })).is_err());
    assert!(app.act("chat_node", &json!({ "chat": "não existe" })).is_err());
}

#[test]
fn the_maestro_comes_onto_the_canvas_when_it_builds_its_team() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // a chat from before there were cards: none on the canvas
    app.chats.push(serde_json::from_value(json!({ "id": "velho", "agent": "claude", "name": "Velho", "cwd": "~/x" })).unwrap());
    let (reply, said) = std::sync::mpsc::channel();
    // it recruits (the bridge says which chat): its card appears, and the recruit hangs from it, in the chat's folder
    app.apply_agent_command(UiCommand::Recruit { from: "velho".into(), command: "codex".into(), worktree: false, reply: reply.clone() });
    assert!(said.recv().unwrap().unwrap().contains("criado abaixo de você"));
    let card = app.chat_node("velho").expect("the Maestro's card");
    let recruit = app.scene.elements.iter().find(|e| e.terminal.is_some()).cloned().unwrap();
    let spec = recruit.terminal.as_ref().unwrap();
    assert_eq!((spec.cwd.as_str(), spec.command.as_str(), spec.maestro), ("~/x", "codex", false));
    assert_eq!(app.links()["velho"], vec![recruit.id.clone()]);
    assert!(local_bounds(&recruit).min.y > local_bounds(&app.scene.elements[card]).max.y, "below the card");
    // its team is what `list` gives it now: the Maestro, and who it commands
    let nodes = app.agent_nodes();
    let maestro = nodes.iter().find(|n| n.id == "velho").unwrap();
    assert_eq!((maestro.name.as_str(), maestro.maestro, &maestro.links), ("Maestro", true, &vec![recruit.id.clone()]));
    // a portal it opens hangs from the card too; what is no chat still needs a place on the canvas
    app.apply_agent_command(UiCommand::CreatePortal { from: "velho".into(), url: "exemplo.com".into(), name: String::new(), reply: reply.clone() });
    assert!(said.recv().unwrap().is_ok() && app.links()["velho"].len() == 2);
    app.apply_agent_command(UiCommand::Recruit { from: "ninguém".into(), command: "codex".into(), worktree: false, reply });
    assert!(said.recv().unwrap().is_err());
    assert_eq!(app.scene.elements.iter().filter(|e| e.chat.is_some()).count(), 1);
}

#[test]
fn a_chat_asks_the_chat_it_is_roped_to_over_tcp_by_a_token_of_its_own() {
    let ctx = Context::default();
    let mut app = DrawingApp { bridge: Bridge::start(None).ok(), ..Default::default() };
    // agents this machine does not have: the question gets to the chat, its agent never runs
    let asker = new_chat(&mut app, "Sábia", "mago-agente-que-nao-existe");
    let asked = new_chat(&mut app, "Fantasma", "mago-agente-que-nao-existe");
    app.connect(&asker, &asked);
    app.publish_nodes();
    // what the asker's agent runs with: the bridge, and who it is by its own token
    let env: std::collections::HashMap<String, String> = app.chat_env(&asker).into_iter().collect();
    let (address, token) = (env["MAGOAGENT_BRIDGE"].clone(), env["MAGOAGENT_TOKEN"].clone());
    assert_eq!(env["MAGOAGENT_CHAT_ID"], asker);
    assert_eq!((env["MAGOAGENT_TERMINAL_ID"].as_str(), env["MAGOAGENT_TERMINAL_NAME"].as_str()), ("", "Maestro (Sábia · projeto)"));
    assert!(token != app.bridge.as_ref().unwrap().token && env.contains_key("PATH"));
    // the token says which chat speaks: it cannot speak for another, and need not say who it is
    let request = json!({ "cmd": "ask", "name": "Maestro (Fantasma · projeto)", "text": "tudo certo?", "timeout": 20 });
    let wait = std::time::Duration::from_secs(20);
    assert!(crate::bridge::request_to(&address, &token, &asked, request.clone(), wait).unwrap_err().contains("não pode falar por outro"));
    let asking = std::thread::spawn(move || crate::bridge::request_to(&address, &token, "", request, wait));
    let mut prompt = String::new();
    while !asking.is_finished() {
        let commands: Vec<UiCommand> = app.bridge.as_ref().unwrap().commands.try_iter().collect();
        for command in commands {
            app.apply_agent_command(command);
        }
        // in the asked chat's line, as the asker's
        if let Some(ask) = app.chat_asks.first() {
            assert_eq!(ask.chat, asked);
            prompt = ask.prompt();
        }
        app.chat_nodes_tick(&ctx);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let why = asking.join().unwrap().unwrap_err();
    assert!(prompt.starts_with("Pergunta de \"Maestro (Sábia · projeto)\"") && prompt.ends_with("tudo certo?"), "{prompt} / {why}");
    assert!(why.contains("\"Maestro (Fantasma · projeto)\" não pôde receber a pergunta") && why.contains("não foi encontrado"), "{why}");
    assert!(app.chat_asks.is_empty());
}

#[test]
fn the_cards_menu_and_its_button_open_its_chat_in_the_mini_chat() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = new_chat(&mut app, "Claude Code", "claude");
    app.chat_open = false;
    app.chat_agent = None;
    assert!(ids(&app.chat_node_entries(&id)).contains(&"open_mini_chat".to_string()));
    assert!(app.chat_menu_action(&Target::Element(id.clone()), "open_mini_chat"));
    assert!(app.mini.open && app.mini.on && !app.chat_open);
    assert_eq!(app.chat_agent.as_deref(), Some(id.as_str()));
    // the button sits in the header, inside the card, left of its dot
    let (frame_rect, z) = app.terminal_frame(&app.scene.elements[0]);
    let button = crate::app::chat::node_ui::mini_button(frame_rect, z);
    assert!(frame_rect.contains_rect(button) && button.bottom() <= frame_rect.top() + TERMINAL_HEADER * z);
}
