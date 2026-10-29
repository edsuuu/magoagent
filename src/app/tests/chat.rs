use super::*;

/// One frame of the chat screen alone, `size` wide and tall.
fn chat_frame(app: &mut DrawingApp, ctx: &Context, size: Vec2, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default() };
    ctx.run_ui(raw, |_| app.chat_screen(ctx)).textures_delta.clear();
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
