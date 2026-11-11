use super::*;

/// One frame of the chat screen alone, `size` wide and tall: the texts it painted, where.
fn chat_frame(app: &mut DrawingApp, ctx: &Context, size: Vec2, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default() };
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
    let texts = click(&mut app, &ctx, size, line.center());
    assert!(app.chat_unfolded.contains("a"));
    assert!(painted(&texts, "a.rs").is_some() && painted(&texts, "b.rs").is_some(), "opened: its steps show under the line");
    click(&mut app, &ctx, size, line.center());
    assert!(app.chat_unfolded.is_empty(), "and it folds back");
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
    let texts = click(&mut app, &ctx, size, row.center());
    assert!(app.chat_unfolded.contains("card:s"));
    assert!(says(&texts, "npm test --silent") && says(&texts, "3 testes ok"), "the whole command and its output: {texts:?}");
    assert!(says(&texts, "Bash · terminou em 4 s") && says(&texts, "Pasta: /p") && painted(&texts, "Copiar").is_some());
    click(&mut app, &ctx, size, row.center());
    assert!(app.chat_unfolded.is_empty(), "and it closes");
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
    let revisor = chat(&mut app, "Revisor");
    let migrador = chat(&mut app, "Migrador");
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
    assert!(painted(&texts, "Chats").is_none(), "no list, just initials: {texts:?}");
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
    assert!(painted(&texts, "Chats 2").is_some() && painted(&texts, "Zelador").is_none(), "the chats, not the terminals: {texts:?}");
    let texts = click(&mut app, &ctx, size, pick.center());
    assert_eq!(app.chat_agent.as_ref(), Some(&revisor));
    assert!(painted(&texts, "Chats").is_none(), "picking one closes it");
}

#[test]
fn the_chat_opens_on_a_chat_and_keeps_an_agents_conversation_opened_from_its_terminal() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let zelador = agent(&mut app, "Zelador");
    let codex = chat(&mut app, "Codex");
    let claude = chat(&mut app, "Claude");
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
    assert!(painted(&texts, "Chats 1").is_some() && painted(&texts, "Agentes").is_none(), "{texts:?}");
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
    let at = painted(&texts, "Chats").unwrap_or_else(|| panic!("{texts:?}"));
    assert!(!cfg!(target_os = "macos") || at.top() > buttons.bottom(), "the title under the window buttons: {at:?}");
    // « folds the list: on the window buttons' row on macOS, on the title's row elsewhere
    let row = if cfg!(target_os = "macos") { LIGHTS_Y } else { painted(&texts, "Chats").unwrap().center().y };
    click(&mut app, &ctx, size, Pos2::new(16.0 + crate::app::chat_list::LIST - 22.0, row));
    assert!(app.chat_list_toggled, "« folds it");
}

fn chat(app: &mut DrawingApp, name: &str) -> String {
    let chat = crate::app::chat_direct::Chat {
        id: name.to_lowercase(),
        agent: "claude".into(),
        name: name.into(),
        cwd: String::new(),
        session: None,
        mode: 0,
        model: String::new(),
        effort: String::new(),
    };
    app.chats.push(chat);
    name.to_lowercase()
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
    assert_eq!(crate::app::chat_list::found(&t.items, "testes"), [0, 2], "either side, any case");
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
    crate::app::chat_list::quote_into(&mut draft, "linha um\nlinha dois\n");
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
    let at = painted(&chat_frame(&mut app, &ctx, size, vec![]), "tudo verde").expect("the message").center();
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
            Shape::Rect(r) if r.rect.height() < 80.0 && r.rect.top() > 600.0 => Some(r.rect.width()),
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
fn the_hint_sits_in_the_middle_of_the_box() {
    let ctx = Context::default();
    crate::app::ui::install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    let id = agent(&mut app, "Revisor");
    app.chat_open = true;
    app.chat_agent = Some(id);
    let size = Vec2::new(1200.0, 800.0);
    chat_frame(&mut app, &ctx, size, vec![]);
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.chat_screen(&ctx));
    output.textures_delta.clear();
    let mut boxed = Rect::NOTHING;
    let mut hint = Rect::NOTHING;
    for s in &output.shapes {
        match &s.shape {
            Shape::Rect(r) if r.rect.height() < 80.0 && r.rect.top() > 600.0 && r.rect.width() > boxed.width() => boxed = r.rect,
            Shape::Text(t) if t.galley.text().starts_with("Mensagem para") => hint = t.visual_bounding_rect(),
            _ => {}
        }
    }
    // the tails of g and p hang below the middle: the letters' body sits in it
    let (above, below) = (hint.top() - boxed.top(), boxed.bottom() - hint.bottom());
    assert!(boxed.height() == 40.0 && (2.0..=6.0).contains(&(above - below)), "hint {hint:?} box {boxed:?}");
}
