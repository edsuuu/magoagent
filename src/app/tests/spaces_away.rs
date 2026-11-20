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
fn a_recruit_is_born_in_the_workspace_of_the_chats_folder_even_off_view() {
    let (mut app, dir, folder) = two_spaces("nasce");
    let boss = push_chat(&mut app, "Chefe");

    app.chats[0].cwd = folder.clone();
    // the chat's card is on the canvas shown (INV-184), its folder is INV-163's
    app.add_chat_node(&boss, None);

    let recruit = |app: &mut DrawingApp| {
        let (reply, said) = std::sync::mpsc::channel();

        app.apply_agent_command(UiCommand::Recruit {
            from: boss.clone(),
            name: String::new(),
            command: "claude \"x\"".into(),
            worktree: false,
            mode: Some(AgentMode::Terminal),
            reply,
        });

        said.recv().unwrap().unwrap()
    };

    let said = recruit(&mut app);
    let home = app.space_named("INV-163").unwrap();

    assert!(said.contains("no espaço \"INV-163\", o da sua pasta (fora da tela)"), "{said}");
    // not on the canvas shown: running off view in INV-163, its shell started by the next frame
    assert!(!app.scene.elements.iter().any(|element| element.terminal.is_some()));
    let (id, (key, spec)) = app.away.iter().next().map(|(id, entry)| (id.clone(), entry.clone())).unwrap();

    assert_eq!((key.as_str(), spec.cwd.as_str()), (home.as_str(), folder.as_str()));
    assert_eq!(app.recruit_boss.get(&id), Some(&boss));
    // in that space's drawing, under the chat's card there, roped to it
    app.switch_space(&home).unwrap();
    let card = app.chat_node(&boss).expect("the chat's card in INV-163");
    let agent = app.scene.elements.iter().position(|element| element.id == id).expect("the recruit in INV-163");

    assert!(local_bounds(&app.scene.elements[agent]).min.y > local_bounds(&app.scene.elements[card]).max.y, "below the card");
    assert!(app.links().get(&boss).is_some_and(|links| links.contains(&id)), "roped to the chat's card");
    // there now, the next one is born on screen
    let said = recruit(&mut app);

    assert!(said.contains("(no espaço \"INV-163\", o que está na tela)"), "{said}");
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
fn the_open_tasks_of_an_agent_closed_go_back_to_no_owner() {
    let ctx = Context::default();
    let (mut app, dir, _) = two_spaces("dono");

    terminal(&mut app, "Cometa");
    app.act("tasks", &json!({ "action": "create", "title": "rodar os testes", "owner": "Cometa" })).unwrap();
    app.act("tasks", &json!({ "action": "create", "title": "revisar", "owner": "Cometa" })).unwrap();
    app.act("tasks", &json!({ "action": "update", "id": "t1", "state": "in_progress" })).unwrap();
    app.act("tasks", &json!({ "action": "update", "id": "t2", "state": "done" })).unwrap();
    app.act("close_window", &json!({ "name": "Cometa" })).unwrap();
    // the shell ends with the frame that sees the terminal gone
    app.sync_terminals(&ctx);
    let list = app.act("tasks", &json!({ "action": "list", "state": "pending" })).unwrap();

    assert!(list.contains("t1 · Pendente · sem responsável") && list.contains("Cometa foi fechado"), "{list}");
    // a task already done keeps who did it
    assert!(app.act("tasks", &json!({ "action": "list", "state": "done" })).unwrap().contains("t2 · Concluída · Cometa"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_terminal_ends_with_what_it_left_running_in_the_background() {
    if cfg!(windows) {
        // the same tree on Windows (taskkill), started another way: `start /b` keeps no pid to look at here
        return;
    }

    let dir = std::env::temp_dir().join(format!("mago-arvore-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    fs::create_dir_all(&dir).unwrap();
    let pid_file = dir.join("pid");
    let script = format!("nohup sleep 300 >/dev/null 2>&1 & echo $! > '{}'; sleep 300", pid_file.display());
    let session = crate::terminal::TerminalSession::spawn("", "", (40, 5), None, vec![], Some(("sh", &["-c", script.as_str()][..])), 10_000).unwrap();
    let started = std::time::Instant::now();

    while fs::read_to_string(&pid_file).map_or(true, |pid| pid.trim().is_empty()) && started.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let background: u32 = fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();

    assert!(crate::transcript::process::alive(background));
    drop(session);
    let ended = std::time::Instant::now();

    while crate::transcript::process::alive(background) && ended.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    assert!(!crate::transcript::process::alive(background), "what was left in the background ends with the terminal");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_container_command_is_read_from_its_exec_line() {
    use crate::app::terminal::procs::container_command;

    let read = |line: &str| container_command(line).map(|(program, container, command)| format!("{program}|{container}|{command}"));

    assert_eq!(read("docker exec -it -w /app hdtb-php php artisan test").as_deref(), Some("docker|hdtb-php|php artisan test"));
    assert_eq!(read("/usr/local/bin/docker exec --user=www -e APP_ENV=testing app vendor/bin/pest").as_deref(), Some("docker|app|vendor/bin/pest"));
    assert_eq!(read(r#""C:\Program Files\Docker\docker.exe" exec -i web npm test"#).as_deref(), Some("docker|web|npm test"));
    assert_eq!(read("podman exec db psql").as_deref(), Some("podman|db|psql"));
    // not an exec, or no command: nothing to stop inside
    assert_eq!(read("docker run -it php"), None);
    assert_eq!(read("docker exec app"), None);
    assert_eq!(read("php artisan test"), None);
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

    app.apply_agent_command(UiCommand::Recruit {
        from: boss.clone(),
        name: String::new(),
        command: "claude \"x\"".into(),
        worktree: false,
        mode: Some(AgentMode::Terminal),
        reply,
    });

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

#[test]
fn a_page_stays_loaded_off_view_and_comes_back_where_the_agent_took_it() {
    let (mut app, dir, _) = two_spaces("pagina");
    let mut element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 600.0, height: 400.0 });

    element.portal = Some(PortalSpec { name: "QA INV-163".into(), url: "https://exemplo.com".into(), ..Default::default() });
    let id = element.id.clone();

    app.scene.elements.push(element);
    app.dirty = true;
    let shown = app.scope_key();
    let home = app.space_named("INV-163").unwrap();

    app.switch_space(&home).unwrap();
    // kept, with its space: the page loop keeps its webview, hidden
    assert_eq!(app.away_portals.get(&id).map(|(key, _)| key.as_str()), Some(shown.as_str()));
    assert!(app.portal_spec(&id).is_some(), "the agents' page commands find it");
    // an agent takes it elsewhere meanwhile: no undo step in the drawing on screen
    let undo = app.undo.len();

    app.portal_navigate(&id, "exemplo.com/login");
    assert_eq!(app.undo.len(), undo);
    let back = app.space_named("INV-184").unwrap();

    app.switch_space(&back).unwrap();
    let url = app.scene.elements.iter().find(|element| element.id == id).and_then(|element| element.portal.as_ref()).map(|spec| spec.url.clone());

    assert_eq!(url.as_deref(), Some("https://exemplo.com/login"), "where the agent left it");
    assert!(app.away_portals.is_empty());
    // a space removed takes its pages
    app.switch_space(&home).unwrap();
    assert!(app.away_portals.contains_key(&id));
    app.remove_space(&back).unwrap();
    app.prune_away_portals();
    assert!(app.away_portals.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_question_queued_for_a_busy_terminal_goes_once_it_is_free_and_its_answer_to_the_chat() {
    let (mut app, dir, _) = two_spaces("fila");
    let boss = push_chat(&mut app, "Chefe");
    let dev = terminal(&mut app, "Dev");

    app.apply_agent_command(UiCommand::QueueAsk { from: boss.clone(), to: dev.clone(), asker: "Luna".into(), text: "e o lint?".into() });
    let queued = app.chat_queue.iter().find(|(to, _)| to == &dev).map(|(_, text)| text.clone()).unwrap();

    assert_eq!(crate::app::chat::ask::asked_by(&queued), Some(("Luna", "e o lint?")));
    // the answer comes to the chat when its turn ends
    app.take_hook(&dev, &json!({ "hook_event_name": "Stop", "last_assistant_message": "Lint limpo." }), None);
    assert!(app.chat_queue.iter().any(|(to, text)| to == &boss && text.contains("Lint limpo.")), "{:?}", app.chat_queue);
    app.terminals.clear();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn set_folder_moves_the_conversation_and_its_card_once_the_answer_ends() {
    let (mut app, dir, folder) = two_spaces("pasta");
    let chat = push_chat(&mut app, "Chefe");

    app.chats[0].session = Some("s1".into());
    app.chat_agent = Some(chat.clone());
    app.add_chat_node(&chat, None);
    // the agent asks it mid-turn: the move waits for that answer
    app.chat_runs.insert(chat.clone(), crate::app::chat::direct::ChatRun::held(app.now));
    let said = app.maestro_set_folder(&folder).unwrap();

    assert!(said.contains("assim que esta resposta terminar") && said.contains("todo o contexto"), "{said}");
    assert_eq!(app.chats.len(), 1, "no second chat");
    assert_ne!(app.chats[0].cwd, folder);
    app.apply_chat_moves();
    assert_ne!(app.chats[0].cwd, folder, "still answering");
    app.chat_runs.clear();
    app.apply_chat_moves();
    // the same conversation (its session) in the new folder, its card in that folder's workspace
    assert_eq!((app.chats[0].cwd.as_str(), app.chats[0].session.as_deref()), (folder.as_str(), Some("s1")));
    assert!(app.chat_node(&chat).is_none(), "off the canvas shown (INV-184)");
    let home = app.space_named("INV-163").unwrap();

    app.switch_space(&home).unwrap();
    assert!(app.chat_node(&chat).is_some(), "on INV-163's");
    let _ = fs::remove_dir_all(&dir);
}
