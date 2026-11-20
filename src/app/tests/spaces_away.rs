//! What an agent does to a workspace that is off view (workspaces/away.rs): its team is born in the space on screen and
//! the answer says so, a terminal off view closes, a chat's card moves to another space, and a recruit off view keeps
//! its boss.

use super::*;

/// An app with the spaces "INV-163" and "INV-184" (each on a folder of its own), the second on screen: (app, the data
/// folder, the first space's folder).
fn two_spaces(name: &str) -> (DrawingApp, PathBuf, String) {
    let (mut app, dir) = app_in_folder(name);
    let folders = [dir.join("hdtb-main"), dir.join("outro")];

    folders.iter().for_each(|folder| fs::create_dir_all(folder).unwrap());
    app.create_space("INV-163", &folders[0].to_string_lossy()).unwrap();
    let shown = app.create_space("INV-184", &folders[1].to_string_lossy()).unwrap();

    app.switch_space(&shown).unwrap();
    (app, dir, folders[0].to_string_lossy().into_owned())
}

/// A terminal named `name` on the canvas shown, with a shell that failed to start (none runs in a test): its id.
fn terminal(app: &mut DrawingApp, name: &str) -> String {
    let mut element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });

    element.terminal = Some(TerminalSpec { name: name.into(), ..Default::default() });
    let id = element.id.clone();

    app.scene.elements.push(element);
    app.dirty = true;
    app.terminals.insert(id.clone(), Err("sem shell no teste".into()));
    id
}

#[test]
fn a_recruits_answer_says_the_space_it_was_born_in_and_warns_when_the_folder_is_anothers() {
    let (mut app, dir, folder) = two_spaces("nasce");
    let boss = push_chat(&mut app, "Chefe");

    app.chats[0].cwd = folder.clone();

    let recruit = |app: &mut DrawingApp| {
        let (reply, said) = std::sync::mpsc::channel();

        app.apply_agent_command(UiCommand::Recruit {
            from: boss.clone(),
            command: "claude \"x\"".into(),
            worktree: false,
            mode: Some(AgentMode::Terminal),
            reply,
        });

        said.recv().unwrap().unwrap()
    };

    let said = recruit(&mut app);

    assert!(said.contains("Nasceu no espaço \"INV-184\""), "{said}");
    assert!(said.contains(&format!("a sua pasta ({folder}) é do espaço \"INV-163\"")), "{said}");
    // in the space of its folder there is nothing to warn about
    let home = app.space_named("INV-163").unwrap();

    app.switch_space(&home).unwrap();
    let said = recruit(&mut app);

    assert!(said.contains("Nasceu no espaço \"INV-163\"") && !said.contains("Atenção"), "{said}");
    app.terminals.clear();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn close_window_ends_a_terminal_running_in_a_workspace_off_view() {
    let (mut app, dir, _) = two_spaces("fechar");
    let id = terminal(&mut app, "Cristal");
    let home = app.space_named("INV-163").unwrap();

    app.switch_space(&home).unwrap();
    assert!(app.away.contains_key(&id));
    let said = app.act("close_window", &json!({ "name": "cristal" })).unwrap();

    assert!(said.contains("Cristal fechado no espaço \"INV-184\"") && said.contains("fora da tela"), "{said}");
    assert!(!app.away.contains_key(&id) && !app.terminals.contains_key(&id));
    // gone from that space's drawing: coming back starts no shell for it
    let shown = app.space_named("INV-184").unwrap();

    app.switch_space(&shown).unwrap();
    assert!(!app.scene.elements.iter().any(|element| element.id == id));
    assert_eq!(app.act("close_window", &json!({ "name": "Cristal" })), Err("Nada no canvas se chama Cristal.".into()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_chats_card_moves_to_another_workspace_and_show_says_which_canvas_it_is_on() {
    let (mut app, dir, _) = two_spaces("mover");
    let chat = push_chat(&mut app, "Chefe");
    let shown = app.act("chat_node", &json!({ "action": "show", "chat": "Chefe" })).unwrap();

    // the card is put on the canvas shown, and the answer names it
    assert!(shown.contains("canvas do espaço \"INV-184\""), "{shown}");
    assert_eq!(app.current_space().map(|space| space.name.as_str()), Some("INV-184"));
    let moved = app.act("chat_node", &json!({ "action": "move", "chat": "Chefe", "space": "inv-163" })).unwrap();

    assert!(moved.contains("agora está no espaço \"INV-163\""), "{moved}");
    assert!(app.chat_node(&chat).is_none(), "off the canvas shown");
    let home = app.space_named("INV-163").unwrap();

    app.switch_space(&home).unwrap();
    assert!(app.chat_node(&chat).is_some(), "and on the other space's");
    assert!(app.act("chat_node", &json!({ "action": "move", "chat": "Chefe", "space": "Nenhum" })).is_err());
    assert!(app.act("chat_node", &json!({ "action": "move", "chat": "Chefe" })).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_task_list_says_when_an_owner_is_gone() {
    let (mut app, dir, _) = two_spaces("dono");
    let id = terminal(&mut app, "Cometa");

    app.act("tasks", &json!({ "action": "create", "title": "rodar os testes", "owner": "Cometa" })).unwrap();
    assert!(!app.act("tasks", &json!({ "action": "list" })).unwrap().contains("não existe mais"));
    app.act("close_window", &json!({ "name": "Cometa" })).unwrap();
    app.terminals.remove(&id);
    let list = app.act("tasks", &json!({ "action": "list" })).unwrap();

    assert!(list.contains("atenção: o responsável Cometa não existe mais"), "{list}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_recruit_off_view_keeps_its_boss() {
    let ctx = Context::default();
    let (mut app, dir, _) = two_spaces("chefe");
    let (boss, recruit, _) = team::team(&mut app, &ctx);

    assert_eq!(app.boss_of(&recruit).as_deref(), Some(boss.as_str()));
    let home = app.space_named("INV-163").unwrap();

    app.switch_space(&home).unwrap();
    assert!(app.chat_node(&recruit).is_none());
    assert_eq!(app.boss_of(&recruit).as_deref(), Some(boss.as_str()), "what it says still goes to the chat that recruited it");
    // the boss's words to it are a question whose answer comes back, as on screen
    let said = app.act("chat_send", &json!({ "chat": recruit, "text": "e aí?", "asker": boss })).unwrap();

    assert!(said.contains("a resposta dele chega neste chat"), "{said}");
    app.chat_runs.clear();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_terminal_recruits_last_words_go_to_the_chat_that_recruited_it_when_its_turn_ends() {
    let (mut app, dir, _) = two_spaces("terminou");
    let boss = push_chat(&mut app, "Chefe");
    let (reply, said) = std::sync::mpsc::channel();

    app.apply_agent_command(UiCommand::Recruit { from: boss.clone(), command: "claude \"x\"".into(), worktree: false, mode: Some(AgentMode::Terminal), reply });
    said.recv().unwrap().unwrap();
    let recruit = app.scene.elements.iter().find(|element| element.terminal.is_some()).unwrap().id.clone();
    let name = app.speaker(&recruit);
    let stop = |words: &str| json!({ "hook_event_name": "Stop", "session_id": "s1", "last_assistant_message": words });

    // a turn that said nothing brings nothing
    app.take_hook(&recruit, &stop(" "), None);
    assert!(app.chat_queue.is_empty());
    app.take_hook(&recruit, &stop("Rodei: 14 passaram."), None);
    let message = crate::app::chat::ask::answer_message(&name, "Rodei: 14 passaram.");

    assert_eq!(app.chat_queue, [(boss.clone(), message)]);
    // off view too: the hook comes over the bridge whatever the canvas shows
    let home = app.space_named("INV-163").unwrap();

    app.terminals.insert(recruit.clone(), Err("sem shell no teste".into()));
    app.switch_space(&home).unwrap();
    app.take_hook(&recruit, &stop("Terminei o resto."), None);
    assert_eq!(app.chat_queue.len(), 2);
    assert!(app.chat_queue[1].1.contains("Terminei o resto."));
    // a terminal nobody recruited reports to nobody
    let loose = terminal(&mut app, "Solto");

    app.take_hook(&loose, &stop("oi"), None);
    assert_eq!(app.chat_queue.len(), 2);
    app.terminals.clear();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_task_a_chat_sends_brings_the_owners_answer_back_to_that_chat() {
    let ctx = Context::default();
    let (mut app, dir, _) = two_spaces("tarefa");
    let (boss, recruit, name) = team::team(&mut app, &ctx);

    app.chat_asks.clear();
    app.chat_runs.clear();
    // to a chat: the task waits in its line as written, and its answer comes back to the chat that sent it
    let said = app.act("tasks", &json!({ "action": "create", "title": "rodar o lint", "owner": name, "send": true, "asker": boss })).unwrap();

    assert!(said.contains("chega neste chat"), "{said}");
    assert!(app.chat_outbox.is_empty() && app.chat_asks.len() == 1 && app.chat_asks[0].chat == recruit);
    app.chat_runs.insert(recruit.clone(), crate::app::chat::direct::ChatRun::held(app.now));
    app.ask_sent(0);
    app.chat_heard(&recruit, &json!({ "type": "result", "is_error": false, "result": "Lint limpo." }), false);
    app.chat_runs.remove(&recruit);
    app.chat_runs.insert(boss.clone(), crate::app::chat::direct::ChatRun::held(app.now));
    app.chat_asks_tick(&ctx);
    assert!(app.chat_queue.contains(&(boss.clone(), crate::app::chat::ask::answer_message(&name, "Lint limpo."))), "{:?}", app.chat_queue);
    // to a terminal nobody recruited: its last words when its turn ends go to that chat
    app.chat_queue.clear();
    let dev = terminal(&mut app, "Dev");
    let program = if cfg!(windows) { ("powershell", &["-NoProfile", "-Command", "Start-Sleep 30"][..]) } else { ("sh", &["-c", "sleep 30"][..]) };

    app.terminals.insert(dev.clone(), Ok(crate::terminal::TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program), 10_000).unwrap()));
    app.act("tasks", &json!({ "action": "create", "title": "revisar", "owner": "Dev" })).unwrap();
    let said = app.act("tasks", &json!({ "action": "update", "id": "t2", "send": true, "asker": boss })).unwrap();

    assert!(said.contains("chega neste chat"), "{said}");
    app.take_hook(&dev, &json!({ "hook_event_name": "Stop", "last_assistant_message": "Revisei: dois avisos." }), None);
    assert!(app.chat_queue.iter().any(|(to, message)| to == &boss && message.contains("Revisei: dois avisos.")), "{:?}", app.chat_queue);
    // sent by no chat (the person, a terminal): as before, a message with no answer brought back
    app.chat_asks.clear();
    app.act("tasks", &json!({ "action": "create", "title": "outra", "owner": name, "send": true })).unwrap();
    assert!(app.chat_asks.is_empty() && app.chat_outbox.iter().any(|(to, _)| to == &recruit));
    app.terminals.clear();
    app.chat_runs.clear();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_chat_recruit_keeps_its_agents_name_after_its_first_message() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let (_, recruit, name) = team::team(&mut app, &ctx);
    let chat = app.chats.iter().find(|chat| chat.id == recruit).unwrap();

    assert!(chat.named && chat.name == name, "{name}");
}
