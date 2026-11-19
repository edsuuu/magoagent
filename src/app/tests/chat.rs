use super::*;

/// One frame of the chat screen alone, `size` wide and tall: the texts it painted, where.
fn chat_frame(app: &mut DrawingApp, ctx: &Context, size: Vec2, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
    chat_frame_at(app, ctx, size, None, events)
}

/// `chat_frame` at `time` (`None`: a 60th of a second after the last), for what eases over time.
fn chat_frame_at(app: &mut DrawingApp, ctx: &Context, size: Vec2, time: Option<f64>, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time, events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.chat_screen(ctx));

    output.textures_delta.clear();

    output
        .shapes
        .iter()
        .filter_map(|s| if let Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.visual_bounding_rect())) } else { None })
        .collect()
}

/// Where a text starting with `prefix` was painted.
fn painted(texts: &[(String, Rect)], prefix: &str) -> Option<Rect> {
    texts.iter().find(|(t, _)| t.starts_with(prefix)).map(|(_, r)| *r)
}

fn click(app: &mut DrawingApp, ctx: &Context, size: Vec2, at: Pos2) -> Vec<(String, Rect)> {
    chat_frame(app, ctx, size, vec![egui::Event::PointerMoved(at), button(at, true)]);
    chat_frame(app, ctx, size, vec![button(at, false)]);
    chat_frame(app, ctx, size, vec![])
}

/// `click` on an accordion's row, and a frame a second later: what it holds has eased all the way open or shut.
fn click_eased(app: &mut DrawingApp, ctx: &Context, size: Vec2, at: Pos2) -> Vec<(String, Rect)> {
    click(app, ctx, size, at);
    let later = ctx.input(|input| input.time) + 1.0;

    chat_frame_at(app, ctx, size, Some(later), vec![])
}

#[test]
fn a_run_of_tools_folds_into_one_line_that_opens_and_the_latest_stays_open() {
    use crate::transcript::{ChatItem, Step, Transcript};
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id.clone());

    let step = |id: &str, name: &str, what: &str| {
        ChatItem::Tool(Step { id: id.into(), name: name.into(), what: what.into(), started: Some(10.0), ended: Some(22.0), ..Default::default() })
    };

    let mut t = Transcript::new(PathBuf::new());

    t.items = vec![
        ChatItem::User("revise".into()),
        step("a", "Read", "a.rs"),
        step("b", "Read", "b.rs"),
        ChatItem::Agent("feito".into()),
        step("c", "Edit", "c.rs"),
        step("d", "Bash", "cargo test"),
    ];

    app.transcripts.insert(id, t);
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);
    let line = painted(&texts, "2 ferramentas · Read ×2 · 12 s").unwrap_or_else(|| panic!("the run before the answer folds: {texts:?}"));

    assert!(painted(&texts, "a.rs").is_none(), "its steps hidden");
    assert!(painted(&texts, "c.rs").is_some() && painted(&texts, "cargo test").is_some(), "the latest run stays open");
    let texts = click_eased(&mut app, &ctx, size, line.center());

    assert!(app.chat_unfolded.contains("a"));
    assert!(painted(&texts, "a.rs").is_some() && painted(&texts, "b.rs").is_some(), "opened: its steps show under the line");
    let texts = click_eased(&mut app, &ctx, size, line.center());

    assert!(app.chat_unfolded.is_empty(), "and it folds back");
    assert!(painted(&texts, "a.rs").is_none(), "shut: its steps not laid out");
}

#[test]
fn a_run_of_tools_eases_open_and_shut_under_its_line() {
    use crate::transcript::{ChatItem, Step, Transcript};
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id.clone());

    let step = |id: &str, name: &str, what: &str| {
        ChatItem::Tool(Step { id: id.into(), name: name.into(), what: what.into(), started: Some(10.0), ended: Some(13.0), ..Default::default() })
    };

    // the run the user pointed at: "3 ferramentas · ToolSearch, create_portal, focus_canvas · 3 s"
    let mut t = Transcript::new(PathBuf::new());

    t.items = vec![
        ChatItem::User("abra o google".into()),
        step("a", "ToolSearch", "select:mcp__magoagent__create_portal"),
        step("b", "mcp__magoagent__create_portal", "https://www.google.com"),
        step("c", "mcp__magoagent__focus_canvas", "www.google.com"),
        ChatItem::Agent("Abri o Google no canvas.".into()),
    ];

    app.transcripts.insert(id, t);
    let size = Vec2::new(1200.0, 800.0);
    let at = |app: &mut DrawingApp, time: f64, events: Vec<egui::Event>| chat_frame_at(app, &ctx, size, Some(time), events);
    const LINE: &str = "3 ferramentas · ToolSearch, create_portal, focus_canvas · 3 s";

    // how far under the run's line the answer is (in the conversation, not the list's preview at the left): the line,
    // and the room its steps take, with the gap over them
    let room = |texts: &[(String, Rect)]| {
        let line = painted(texts, LINE).unwrap_or_else(|| panic!("{texts:?}"));
        let answer = texts.iter().find(|(text, rect)| text.starts_with("Abri o Google") && rect.left() > line.left() - 40.0);

        (line.top(), answer.unwrap_or_else(|| panic!("{texts:?}")).1.top() - line.top())
    };

    at(&mut app, 0.0, vec![]);
    let texts = at(&mut app, 0.1, vec![]);
    let (line, shut) = room(&texts);

    assert!(painted(&texts, "https://www.google.com").is_none(), "shut: its steps not laid out");
    let middle = painted(&texts, LINE).unwrap().center();

    // a click: its steps ease open under the line, which stays where it is
    at(&mut app, 0.2, vec![egui::Event::PointerMoved(middle), button(middle, true)]);
    at(&mut app, 0.25, vec![button(middle, false)]);
    assert!(app.chat_unfolded.contains("a"));
    at(&mut app, 0.26, vec![]);
    let (still, opening) = room(&at(&mut app, 0.3, vec![]));
    let texts = at(&mut app, 1.0, vec![]);
    let (_, open) = room(&texts);

    assert!(painted(&texts, "https://www.google.com").is_some(), "open: its steps under the line");
    assert!(shut < opening && opening < open, "halfway, between shut and open: {shut} < {opening} < {open}");
    assert_eq!(still, line, "the line stays under the pointer");
    assert_eq!(room(&at(&mut app, 1.1, vec![])).1, open, "and stays open");
    // another: they ease shut, and shut nothing of them is laid out
    at(&mut app, 1.2, vec![button(middle, true)]);
    at(&mut app, 1.25, vec![button(middle, false)]);
    assert!(app.chat_unfolded.is_empty());
    at(&mut app, 1.26, vec![]);
    let (still, closing) = room(&at(&mut app, 1.3, vec![]));

    assert!(shut < closing && closing < open, "halfway back: {shut} < {closing} < {open}");
    assert_eq!(still, line);
    let texts = at(&mut app, 2.0, vec![]);

    assert!((room(&texts).1 - shut).abs() < 0.5, "back to shut: {} vs {shut}", room(&texts).1);
    assert!(painted(&texts, "https://www.google.com").is_none(), "shut: its steps not laid out");
}

#[test]
fn a_click_on_a_step_opens_its_card_with_the_whole_command() {
    use crate::transcript::{ChatItem, Step, Transcript};
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id.clone());

    let step = Step {
        id: "s".into(),
        name: "Bash".into(),
        what: "cd web".into(),
        input: "cd web\nnpm test --silent".into(),
        cwd: Some("/p".into()),
        log: "3 testes ok".into(),
        started: Some(10.0),
        ended: Some(14.0),
        ..Default::default()
    };

    let mut t = Transcript::new(PathBuf::new());

    t.items = vec![ChatItem::User("teste".into()), ChatItem::Tool(step)];
    app.transcripts.insert(id, t);
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);
    let says = |texts: &[(String, Rect)], what: &str| texts.iter().any(|(t, _)| t.contains(what));

    assert!(!says(&texts, "npm test --silent"), "closed: one line, as before");
    let row = painted(&texts, "cd web").expect("its row");
    let texts = click_eased(&mut app, &ctx, size, row.center());

    assert!(app.chat_unfolded.contains("card:s"));
    assert!(says(&texts, "npm test --silent") && says(&texts, "3 testes ok"), "the whole command and its output: {texts:?}");
    assert!(says(&texts, "Bash · terminou em 4 s") && says(&texts, "Pasta: /p") && painted(&texts, "Copiar").is_some());
    let texts = click_eased(&mut app, &ctx, size, row.center());

    assert!(app.chat_unfolded.is_empty(), "and it closes");
    assert!(!says(&texts, "npm test --silent"), "shut: its card not laid out");
}

#[test]
fn a_terminal_remembers_its_conversation_and_its_chat_shows_it_after_a_restart() {
    use crate::transcript::{ChatItem, Session};
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    // its claude runs: the drawing keeps which conversation, with no undo step
    app.agent_sessions.insert(id.clone(), Session { id: "0b6c-41".into(), cwd: "/p".into(), ..Default::default() });
    app.remember_sessions();
    let spec = app.scene.elements[0].terminal.clone().unwrap();

    assert_eq!((spec.claude_session.as_str(), spec.claude_folder.as_str(), app.dirty, app.undo.len()), ("0b6c-41", "-p", true, 0));
    let scene: Scene = serde_json::from_str(&serde_json::to_string(&app.scene).unwrap()).unwrap();

    assert_eq!(scene.elements[0].terminal, Some(spec), "and it comes back from the file");
    // the app reopened: no claude running in it, its conversation still on disk
    let dir = std::env::temp_dir().join(format!("mago-lembrada-{}", uuid::Uuid::new_v4().simple()));
    let file = crate::transcript::transcript_path(&dir, "-p", "0b6c-41");

    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, format!("{}\n", serde_json::json!({"type": "user", "message": {"content": "revise o login"}}))).unwrap();
    app.agent_sessions.clear();
    app.follow_transcript(&id, &dir);
    let items = &app.transcripts[&id].items;

    assert_eq!(items, &[ChatItem::User("revise o login".into()), ChatItem::System(crate::app::chat::EARLIER.into())]);
    app.chat_open = true;
    app.chat_agent = Some(id.clone());
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);

    assert!(painted(&texts, "revise o login").is_some() && painted(&texts, "Conversa anterior").is_some(), "{texts:?}");
    // its file gone: nothing to show
    fs::remove_file(&file).unwrap();
    app.transcripts.clear();
    app.follow_transcript(&id, &dir);
    assert!(!app.transcripts.contains_key(&id));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn narrow_the_list_is_a_column_of_initials_and_ctrl_b_opens_it_over_the_conversation() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let revisor = push_chat(&mut app, "Revisor");
    let migrador = push_chat(&mut app, "Migrador");

    // an agent in a terminal is not in the chat's list: it has its own panel
    agent(&mut app, "Zelador");
    app.chat_open = true;
    app.chat_agent = Some(revisor.clone());
    let mut t = crate::transcript::Transcript::new(PathBuf::new());

    t.items = vec![crate::transcript::ChatItem::User("Pode implementar. Bloqueia por 15 minutos e roda os testes de novo, depois me diz.".into())];
    app.transcripts.insert(revisor.clone(), t);
    let size = Vec2::new(480.0, 900.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);
    let said = painted(&texts, "Pode implementar").expect("the message");

    assert!(said.left() > 62.0 && said.right() < 480.0, "a long message wraps inside the column: {said:?}");
    assert!(painted(&texts, "Conversas").is_none(), "no list, just initials: {texts:?}");
    assert!(painted(&texts, "Canvas").is_none() && painted(&texts, "Abrir canvas").is_none(), "the canvas button is gone: the buttons go at the left");
    assert!(painted(&texts, "Z").is_none(), "only the chats: {texts:?}");
    let m = painted(&texts, "M").expect("its initial");

    assert!(m.right() < 70.0, "in the 52 pt column: {m:?}");
    click(&mut app, &ctx, size, m.center());
    assert_eq!(app.chat_agent.as_ref(), Some(&migrador));
    let ctrl_b = egui::Event::Key { key: Key::B, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };

    chat_frame(&mut app, &ctx, size, vec![ctrl_b]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);
    let pick = painted(&texts, "Revisor").expect("the whole list, over the conversation");

    assert!(painted(&texts, "Conversas").is_some() && painted(&texts, "Zelador").is_none(), "the chats, not the terminals: {texts:?}");
    let texts = click(&mut app, &ctx, size, pick.center());

    assert_eq!(app.chat_agent.as_ref(), Some(&revisor));
    assert!(painted(&texts, "Conversas").is_none(), "picking one closes it");
}

#[test]
fn the_chat_opens_on_a_chat_and_keeps_an_agents_conversation_opened_from_its_terminal() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let zelador = agent(&mut app, "Zelador");
    let codex = push_chat(&mut app, "Codex");
    let claude = push_chat(&mut app, "Claude");

    app.chats[0].agent = "codex".into();
    app.chat_open = true;
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    assert_eq!(app.chat_agent.as_ref(), Some(&claude), "Claude's chat first, never the terminal");
    app.chats.pop();
    chat_frame(&mut app, &ctx, size, vec![]);
    assert_eq!(app.chat_agent.as_ref(), Some(&codex));
    // its bar's Chat button
    app.chat_on(&zelador);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);

    assert_eq!(app.chat_agent.as_ref(), Some(&zelador));
    // the chats, and the terminal whose conversation is open, under their space (none here: no card on a canvas)
    let group = painted(&texts, "SEM ESPAÇO").unwrap_or_else(|| panic!("{texts:?}"));
    let rows: Vec<Rect> = texts.iter().filter(|(t, r)| t == "Zelador" && r.left() < crate::app::chat::list::LIST).map(|(_, r)| *r).collect();

    assert!(rows.iter().any(|r| r.top() > group.top()) && painted(&texts, "Codex").is_some(), "{texts:?}");
}

#[test]
fn on_the_mac_the_lists_title_sits_below_the_window_buttons() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.chat_open = true;
    app.chat_agent = Some(agent(&mut app, "Revisor"));
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);
    // place_window_buttons: three 14 pt buttons from x = 18, centered LIGHTS_Y below the top
    let buttons = Rect::from_center_size(Pos2::new(45.0, LIGHTS_Y), Vec2::new(54.0, 16.0));
    let at = painted(&texts, "Conversas").unwrap_or_else(|| panic!("{texts:?}"));

    assert!(!cfg!(target_os = "macos") || at.top() > buttons.bottom(), "the title under the window buttons: {at:?}");
    // « folds the list: on the window buttons' row on macOS, on the title's row elsewhere
    let row = if cfg!(target_os = "macos") { LIGHTS_Y } else { painted(&texts, "Conversas").unwrap().center().y };

    click(&mut app, &ctx, size, Pos2::new(16.0 + crate::app::chat::list::LIST - 22.0, row));
    assert!(app.chat_list_toggled, "« folds it");
}

fn agent(app: &mut DrawingApp, name: &str) -> String {
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });

    e.terminal = Some(TerminalSpec { name: name.into(), ..Default::default() });
    let id = e.id.clone();

    app.scene.elements.push(e);
    id
}

#[test]
fn a_message_written_while_the_agent_waits_on_its_terminal_goes_once_it_answers() {
    let dir = std::env::temp_dir().join(format!("mago-fila-{}", std::process::id()));

    fs::create_dir_all(&dir).unwrap();
    let saved = dir.join("linha.txt");

    // a stand-in agent: saves the line typed into it
    let script = if cfg!(windows) {
        format!("$l = [Console]::ReadLine(); Set-Content -LiteralPath '{}' $l; Start-Sleep 30", saved.display())
    } else {
        format!("read l; printf '%s' \"$l\" > '{}'; sleep 30", saved.display())
    };

    let program = if cfg!(windows) { ("powershell", &["-NoProfile", "-Command", script.as_str()][..]) } else { ("sh", &["-c", script.as_str()][..]) };
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(program), 10_000).unwrap();
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.terminals.insert(id.clone(), Ok(session));
    // it stopped on a permission menu only its terminal answers
    app.terminal_waiting.insert(id.clone());
    app.chat_open = true;
    app.chat_agent = Some(id.clone());
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    app.chat_draft = "e os testes?".into();
    ctx.memory_mut(|m| m.request_focus(egui::Id::unique("chat_draft")));
    let enter = egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };

    chat_frame(&mut app, &ctx, size, vec![enter.clone()]);
    assert_eq!((app.chat_draft.as_str(), app.chat_queue.clone()), ("", vec![(id.clone(), "e os testes?".to_string())]), "queued, not typed into its menu");
    // a second one joins it; while it waits, nothing goes
    app.chat_draft = "e o lint".into();
    chat_frame(&mut app, &ctx, size, vec![enter]);
    app.flush_chat_queue();
    assert_eq!(app.chat_queue, [(id.clone(), "e os testes?\n\ne o lint".to_string())]);
    // answered in its terminal: it goes, as one prompt
    app.terminal_waiting.clear();
    app.flush_chat_queue();
    assert!(app.chat_queue.is_empty());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);

    while fs::read_to_string(&saved).map_or(true, |s| s.trim().is_empty()) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // a pasted prompt keeps its blank line; the shell reads up to the first newline
    assert_eq!(fs::read_to_string(&saved).unwrap().trim(), "e os testes?");
    // a terminal that went gives what was queued for it back to the box
    app.queue_message("sumiu", "oi".into());
    app.flush_chat_queue();
    assert!(app.chat_queue.is_empty() && app.chat_draft.ends_with("oi"), "{:?}", app.chat_draft);
    let _ = fs::remove_dir_all(&dir);
}

fn key(key: Key, modifiers: Modifiers) -> egui::Event {
    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

#[test]
fn ctrl_f_shows_only_the_messages_that_say_it_and_esc_closes_the_search_then_the_chat() {
    use crate::transcript::{ChatItem, Transcript};
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id.clone());
    let mut t = Transcript::new(PathBuf::new());

    t.items = vec![ChatItem::User("rode os Testes".into()), ChatItem::Agent("feito, tudo verde".into()), ChatItem::Agent("os testes passaram".into())];
    assert_eq!(crate::app::chat::list::found(&t.items, "testes"), [0, 2], "either side, any case");
    app.transcripts.insert(id, t);
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    chat_frame(&mut app, &ctx, size, vec![key(Key::F, Modifiers::COMMAND)]);
    assert_eq!(app.chat_search.as_deref(), Some(""));
    chat_frame(&mut app, &ctx, size, vec![egui::Event::Text("TESTES".into())]);
    assert_eq!(app.chat_search.as_deref(), Some("TESTES"), "the field took the keys");
    let texts = chat_frame(&mut app, &ctx, size, vec![]);

    assert!(painted(&texts, "rode os Testes").is_some() && painted(&texts, "os testes passaram").is_some());
    assert!(painted(&texts, "feito").is_none(), "the rest hides: {texts:?}");
    chat_frame(&mut app, &ctx, size, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.chat_search.is_none() && app.chat_open, "Esc closes the search first");
    let texts = chat_frame(&mut app, &ctx, size, vec![]);

    assert!(painted(&texts, "feito").is_some());
    // nothing running: the next Esc goes back to the canvas
    chat_frame(&mut app, &ctx, size, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!app.chat_open);
}

#[test]
fn citar_puts_a_message_in_the_box_as_a_quote() {
    use crate::transcript::{ChatItem, Transcript};
    let mut draft = "veja".to_string();

    crate::app::chat::list::quote_into(&mut draft, "linha um\nlinha dois\n");
    assert_eq!(draft, "veja\n> linha um\n> linha dois\n\n");
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id.clone());
    let mut t = Transcript::new(PathBuf::new());

    t.items = vec![ChatItem::Agent("tudo verde".into())];
    app.transcripts.insert(id, t);
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    // the conversation's, not the list's last line
    let texts = chat_frame(&mut app, &ctx, size, vec![]);
    let at = texts.iter().find(|(t, r)| t == "tudo verde" && r.left() > crate::app::chat::list::LIST + 16.0).expect("the message").1.center();
    let right = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };

    chat_frame(&mut app, &ctx, size, vec![egui::Event::PointerMoved(at), right(true)]);
    chat_frame(&mut app, &ctx, size, vec![right(false)]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);

    assert!(painted(&texts, "Copiar").is_some(), "its menu: {texts:?}");
    let citar = painted(&texts, "Citar").expect("Citar");

    click(&mut app, &ctx, size, citar.center());
    assert_eq!(app.chat_draft, "> tudo verde\n\n");
}

#[test]
fn a_chat_mode_terminal_opens_the_chat_when_clicked() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.scene.elements[0].terminal.as_mut().unwrap().chat = true;
    app.terminal_unread.insert(id.clone());
    super::frame(&mut app, &ctx, vec![]);
    let at = Pos2::new(200.0, 200.0);

    super::frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    super::frame(&mut app, &ctx, vec![button(at, false)]);
    assert!(app.chat_open && app.chat_agent.as_ref() == Some(&id), "the chat, on it");
    assert!(app.terminal_focus.is_none() && !app.terminal_unread.contains(&id), "its keys stay off, its news read");
    // off: the click types into it
    app.chat_open = false;
    app.scene.elements[0].terminal.as_mut().unwrap().chat = false;
    super::frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    super::frame(&mut app, &ctx, vec![button(at, false)]);
    assert!(!app.chat_open && app.terminal_focus.as_ref() == Some(&id));
}

#[test]
fn the_call_leaves_the_conversation_in_sight() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id);
    let size = Vec2::new(1200.0, 800.0);

    // the big rects painted over the conversation (not the whole screen's)
    let covers = |app: &mut DrawingApp| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| app.chat_screen(&ctx));

        output.textures_delta.clear();

        output
            .shapes
            .iter()
            .filter(|s| matches!(&s.shape, Shape::Rect(r) if r.rect.width() > 600.0 && r.rect.width() < size.x && r.rect.height() > 400.0))
            .count()
    };

    app.voice.on = true;
    app.voice.state = VoiceState::Preparing;
    assert_eq!(covers(&mut app), 0, "starting, the call is in its popup");
    app.voice.state = VoiceState::Listening;
    app.voice.connected_at = Some(app.now - 60.0);
    assert_eq!(covers(&mut app), 0);
}

#[test]
fn the_box_keeps_its_width_while_the_call_is_on() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id);
    let size = Vec2::new(1200.0, 800.0);

    // the box: the widest short rect under the conversation
    let width = |app: &mut DrawingApp| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| app.chat_screen(&ctx));

        output.textures_delta.clear();

        let short = |s: &egui::epaint::ClippedShape| match &s.shape {
            Shape::Rect(r) if r.rect.height() < 160.0 && r.rect.top() > 600.0 => Some(r.rect.width()),
            _ => None,
        };

        output.shapes.iter().filter_map(short).fold(0.0, f32::max)
    };

    // the first frame only lays the screen out
    width(&mut app);
    let off = width(&mut app);

    app.voice.on = true;
    assert!(off > 600.0 && width(&mut app) == off, "{off}");
}

#[test]
fn the_card_has_the_words_on_top_and_its_buttons_under_them() {
    let ctx = Context::default();

    crate::app::ui::install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    let id = push_chat(&mut app, "Claude");

    app.chat_open = true;
    app.chat_agent = Some(id);
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.chat_screen(&ctx));

    output.textures_delta.clear();
    let (mut card, mut hint, mut keys, mut chip) = (Rect::NOTHING, Rect::NOTHING, Rect::NOTHING, Rect::NOTHING);

    for s in &output.shapes {
        match &s.shape {
            Shape::Rect(r) if r.rect.height() < 160.0 && r.rect.top() > 550.0 && r.rect.width() > card.width() => card = r.rect,
            Shape::Text(t) if t.galley.text().starts_with("Escreva uma mensagem") => hint = t.visual_bounding_rect(),
            Shape::Text(t) if t.galley.text().starts_with("Enter envia") => keys = t.visual_bounding_rect(),
            Shape::Text(t) if t.galley.text() == "Auto" => chip = t.visual_bounding_rect(),
            _ => {}
        }
    }

    assert!(card.contains_rect(hint) && hint.top() - card.top() < 20.0, "the words at its top: {hint:?} in {card:?}");
    // the chat's chips sit in it, under the words; nothing under it
    assert!(card.contains_rect(chip) && chip.top() > hint.bottom() + 10.0, "chip {chip:?} card {card:?}");
    assert_eq!(keys, Rect::NOTHING, "no keys' hint under it");
}

/// The header's last button is no longer Abrir em miniatura: a click at the header's right end opens no panel over the
/// canvas.
#[test]
fn the_header_has_no_abrir_em_miniatura() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = push_chat(&mut app, "Claude");

    app.chat_open = true;
    app.chat_agent = Some(id);
    // the Contexto panel closed: the header runs to the window's edge
    app.prefs.chat_panel = false;
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    chat_frame(&mut app, &ctx, size, vec![]);
    // the header's last button, at its right end
    let at = Pos2::new(size.x - 16.0 - 16.0, TOP_ROW.max(TITLE_BAR) + 27.0);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);

    assert!(painted(&texts, "Ver no canvas").is_some_and(|rect| rect.right() > size.x - 60.0), "Ver no canvas is the last button");
    click(&mut app, &ctx, size, at);
    assert!(!app.mini.open, "no panel over the canvas");
}

#[test]
fn a_long_chat_lays_out_only_what_is_on_screen() {
    use crate::transcript::{ChatItem, Transcript};
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id.clone());
    let mut t = Transcript::new(PathBuf::new());

    t.items = (0..120).map(|i| ChatItem::Agent(format!("mensagem {i}"))).collect();
    app.transcripts.insert(id, t);
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    chat_frame(&mut app, &ctx, size, vec![]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);

    assert!(painted(&texts, "mensagem 119").is_some(), "the end, in sight");
    assert!(painted(&texts, "mensagem 0").is_none(), "the top, far above, keeps its room unpainted");
    assert!(texts.iter().filter(|(t, _)| t.starts_with("mensagem")).count() < 60);
}

/// The chat screen with nothing to talk to opens a draft: no card on the canvas, nothing saved, not among the agents.
/// Its first message makes it a chat; one never sent goes when another chat is picked.
#[test]
fn the_chat_screen_makes_a_chat_only_with_its_first_message() {
    // an agent that starts and ends at once
    if crate::editor::lsp::which("true").is_none() {
        return;
    }

    let dir = std::env::temp_dir().join(format!("mago-rascunho-{}", uuid::Uuid::new_v4().simple()));

    std::fs::create_dir_all(&dir).unwrap();
    let ctx = Context::default();
    let mut app = DrawingApp { autosave: Some(dir.join("cena.mago")), ..Default::default() };

    app.chat_installed = Some((app.now, vec![crate::model::Preset::new("Claude Code", "true")]));
    app.prefs.chat_folder = dir.display().to_string();
    app.chat_open = true;
    chat_frame(&mut app, &ctx, Vec2::new(1200.0, 800.0), vec![]);
    let id = app.chat_agent.clone().expect("a draft to write in");

    assert!(app.chats[0].draft && app.chat_node(&id).is_none(), "no card on the canvas yet");
    assert!(app.chat_rows().is_empty() && app.agent_rows().is_empty(), "not among the agents");
    app.save_chats();
    let saved = || std::fs::read_to_string(dir.join("chats.json")).unwrap_or_default();

    assert!(!saved().contains(&id), "nothing saved: {}", saved());
    assert!(app.chat_send(&ctx, &id, "oi"));
    assert!(!app.chats[0].draft && app.chat_node(&id).is_some() && app.chat_rows().len() == 1, "the first message makes it a chat");
    assert!(saved().contains(&id));
    // Nova conversa, never sent: picking the chat drops it
    app.start_draft_chat(true);
    let draft = app.chat_agent.clone().unwrap();

    app.chat_agent = Some(id.clone());
    chat_frame(&mut app, &ctx, Vec2::new(1200.0, 800.0), vec![]);
    assert!(!app.chats.iter().any(|chat| chat.id == draft) && app.chats.len() == 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_recent_folder_picked_in_the_folder_chips_menu_becomes_the_chats_folder() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = push_chat(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id.clone());
    let recent = std::env::temp_dir().join(format!("mago-recente-{}", std::process::id()));

    fs::create_dir_all(&recent).unwrap();
    let recent = recent.display().to_string();

    app.prefs.folders.remember(&recent);
    let size = Vec2::new(1200.0, 800.0);

    chat_frame(&mut app, &ctx, size, vec![]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);
    // the footer's folder chip (the header says the folder too, higher up)
    let chip = texts.iter().filter(|(text, _)| *text == "~").map(|(_, rect)| *rect).max_by(|a, b| a.top().total_cmp(&b.top()));
    let chip = chip.unwrap_or_else(|| panic!("the folder chip: {texts:?}"));
    let texts = click(&mut app, &ctx, size, chip.center());
    let name = crate::app::chat::direct::folder_label(&recent);
    let line = texts.iter().filter(|(text, _)| *text == name).map(|(_, rect)| *rect).next().unwrap_or_else(|| panic!("listed under Recente: {texts:?}"));

    click(&mut app, &ctx, size, line.center());
    assert_eq!(app.chats.iter().find(|chat| chat.id == id).map(|chat| chat.cwd.as_str()), Some(recent.as_str()), "picked");
    let _ = fs::remove_dir_all(&recent);
}

#[test]
fn a_table_in_the_chat_takes_the_columns_width_with_nothing_cut() {
    use crate::transcript::{ChatItem, Transcript};
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");

    app.chat_open = true;
    app.chat_agent = Some(id.clone());
    // the user's table, its headers as the agent wrote them in full; and a narrow one
    let headers = ["Contrato 5 (A3VTQ8K2-9X, financiamento do apartamento)", "Contrato 76 (7CJ4M1PZ-2B, consórcio do carro)"];

    let wide = format!(
        "Os dois contratos:\n\n|  | {} | {} |\n|---|---|---|\n| Parcelas: total / pagas / a pagar | 48 / 12 / 36 | 60 / 30 / 30 |\n\
         | Próxima | 10/12/2026 — R$ 1.234,56 | 15/12/2026 — R$ 987,65 |\n| Última | 10/11/2029 | 15/11/2028 |\n| Dia do vencimento | 10 | 15 |",
        headers[0], headers[1]
    );

    let narrow = "| a | b |\n|---|---|\n| 1 | 2 |";
    let mut t = Transcript::new(PathBuf::new());

    t.items = vec![ChatItem::User("compare".into()), ChatItem::Agent(wide), ChatItem::Agent(narrow.into())];
    app.transcripts.insert(id, t);
    // what each text shows (its rows' letters, without the spaces it breaks at), in how many rows, cut or not, where;
    // after a few frames of a window `wide` wide
    let packed = |text: &str| text.chars().filter(|letter| !letter.is_whitespace()).collect::<String>();

    let shown = |app: &mut DrawingApp, wide: f32| {
        let mut texts: Vec<(String, usize, bool, Rect)> = Vec::new();

        for _ in 0..4 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(wide, 800.0))), ..Default::default() };
            let output = ctx.run_ui(raw, |_| app.chat_screen(&ctx));

            texts = output
                .shapes
                .into_iter()
                .filter_map(|clipped| match clipped.shape {
                    Shape::Text(text) => Some((
                        packed(&text.galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect::<String>()),
                        text.galley.rows.len(),
                        text.galley.elided,
                        text.visual_bounding_rect(),
                    )),
                    _ => None,
                })
                .collect();
        }

        texts
    };

    let find = |texts: &[(String, usize, bool, Rect)], words: &str| {
        texts.iter().find(|(text, ..)| *text == packed(words)).cloned().unwrap_or_else(|| panic!("{words}: {texts:?}"))
    };

    // a wide window: each header whole and on one line, the table wider than a bubble's 720 (what a squeezed table
    // had) and inside the column (1100 at most)
    let texts = shown(&mut app, 1700.0);

    for header in headers {
        let (_, rows, cut, _) = find(&texts, header);

        assert!(rows == 1 && !cut, "{header}: {rows} rows, cut {cut}");
    }

    // in the conversation (the list at the left cuts its previews)
    let words = find(&texts, "Os dois contratos:").3.left_top();
    let cut: Vec<_> = texts.iter().filter(|(_, _, cut, rect)| *cut && rect.left() >= words.x - 1.0 && rect.top() >= words.y).collect();

    assert!(cut.is_empty(), "nothing cut with …: {cut:?}");
    let (left, right) = (find(&texts, "Parcelas: total / pagas / a pagar").3.left(), find(&texts, headers[1]).3.right());

    assert!(right - left > 720.0 && right - left < 1100.0, "{left} → {right}");
    // the narrow one at its own width: not stretched to the column
    let cell = |words_: &str| texts.iter().find(|(text, .., rect)| text == words_ && rect.left() >= words.x - 1.0 && rect.top() >= words.y).unwrap().3;
    let (one, two) = (cell("1"), cell("2"));

    assert!(two.right() - one.left() < 120.0, "{one:?} {two:?}");
    // a window narrower than the table: its wide cells wrap, whole, inside the conversation's column
    let texts = shown(&mut app, 1000.0);
    // the conversation's right edge: the person's bubble at it, its words 12 in
    let column = find(&texts, "compare").3.right() + 12.0;

    for header in headers {
        let (_, rows, cut, rect) = find(&texts, header);

        assert!(rows > 1 && !cut && rect.right() <= column + 1.0, "{header}: {rows} rows, cut {cut}, {rect:?} past {column}");
    }
}

/// Remover chat asks first, saying the conversation goes for good; Esc keeps the chat, Enter (Remover) removes it.
#[test]
fn remover_chat_asks_first_and_only_remover_deletes_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.chats = vec![chat("a", "claude", "Claude Code · loja")];
    app.chat_open = true;
    let size = Vec2::new(1200.0, 800.0);

    // the row's ⋯ › Remover chat
    app.chat_listing.removing = Some("a".into());
    // a new area takes its size in its first frame
    chat_frame(&mut app, &ctx, size, vec![]);
    let shown = chat_frame(&mut app, &ctx, size, vec![]);

    assert!(painted(&shown, "Remover chat").is_some(), "the dialog asks");
    assert!(shown.iter().any(|(text, _)| text.contains("excluídos permanentemente")), "and says it is for good");
    chat_frame(&mut app, &ctx, size, vec![key_press(Key::Escape)]);
    chat_frame(&mut app, &ctx, size, vec![]);
    assert_eq!((app.chats.len(), app.chat_listing.removing.as_deref()), (1, None), "Esc keeps it");
    // the box keeps what is written: the Enter is the dialog's
    app.chat_listing.removing = Some("a".into());
    app.chat_draft = "não envia".into();
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("chat_draft")));
    chat_frame(&mut app, &ctx, size, vec![]);
    chat_frame(&mut app, &ctx, size, vec![key_press(Key::Enter)]);
    assert!(app.chats.iter().all(|chat| chat.id != "a"), "Remover removes it");
    assert_eq!(app.chat_draft, "não envia");
}

/// The context band's words before its amount shorten in a narrow panel instead of running into the Compactar button.
#[test]
fn the_compact_band_says_less_in_a_narrow_panel() {
    use crate::app::chat::context_view::band_words;

    // seven points a letter
    let width = |words: &str| words.chars().count() as f32 * 7.0;

    assert_eq!(band_words(200.0, width), Some("Compacta sozinho em "));
    assert_eq!(band_words(100.0, width), Some("Sozinho em "));
    assert_eq!(band_words(30.0, width), Some("Em "));
    assert_eq!(band_words(10.0, width), None);
}

/// While the agent works its band says, as Claude Code's status line, how long the turn runs, the tokens written and
/// the effort the chat thinks with.
#[test]
fn the_work_band_says_the_turns_time_tokens_and_effort() {
    use crate::app::ui::agents::{AgentRow, AgentState};

    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = push_chat(&mut app, "Luna");

    app.chats[0].effort = "xhigh".into();
    app.now = 300.0;
    let spec = TerminalSpec { name: "Luna".into(), command: "claude".into(), ..Default::default() };
    let row = AgentRow { id: id.clone(), spec, state: AgentState::Working(300.0 - 262.0), mission: None, space: String::new() };
    let dir = std::env::temp_dir().join(format!("magoagent-band-{}", std::process::id()));

    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("t.jsonl");
    let said = json!({"type": "assistant", "message": {"id": "m", "content": [{"type": "text", "text": "ok"}], "usage": {"output_tokens": 23_040}}});

    fs::write(&file, format!("{}\n{said}\n", json!({"type": "user", "message": {"content": "faz"}}))).unwrap();
    let mut transcript = crate::transcript::Transcript::new(file);

    transcript.poll();
    let look = crate::app::chat::list::Look::new(true, &app.colors());
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 200.0));

    let output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
        app.work_band(ui, &row, Some(&transcript), &look, false, 0.0);
    });

    let texts: Vec<String> =
        output.shapes.iter().filter_map(|shape| if let egui::Shape::Text(text) = &shape.shape { Some(text.galley.text().to_string()) } else { None }).collect();

    assert!(texts.iter().any(|text| text == "4m 22s · 23k tokens · esforço Extra"), "{texts:?}");
    let _ = fs::remove_dir_all(dir);
}
