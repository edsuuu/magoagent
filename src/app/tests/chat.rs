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
fn narrow_the_list_is_a_column_of_initials_and_ctrl_b_opens_it_over_the_conversation() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let revisor = agent(&mut app, "Revisor");
    let migrador = agent(&mut app, "Migrador");
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
    assert!(painted(&texts, "Agentes").is_none(), "no list, just initials: {texts:?}");
    assert!(painted(&texts, "Canvas").is_some() && painted(&texts, "Voltar ao canvas").is_none());
    let m = painted(&texts, "M").expect("its initial");
    assert!(m.right() < 70.0, "in the 52 pt column: {m:?}");
    click(&mut app, &ctx, size, m.center());
    assert_eq!(app.chat_agent.as_ref(), Some(&migrador));
    let ctrl_b = egui::Event::Key { key: Key::B, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    chat_frame(&mut app, &ctx, size, vec![ctrl_b]);
    let texts = chat_frame(&mut app, &ctx, size, vec![]);
    let pick = painted(&texts, "Revisor").expect("the whole list, over the conversation");
    assert!(painted(&texts, "Agentes").is_some());
    let texts = click(&mut app, &ctx, size, pick.center());
    assert_eq!(app.chat_agent.as_ref(), Some(&revisor));
    assert!(painted(&texts, "Agentes").is_none(), "picking one closes it");
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
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(program)).unwrap();
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
