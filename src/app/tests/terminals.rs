use super::*;

#[test]
fn a_terminal_taken_by_undo_keeps_its_shell_while_redo_can_bring_it_back() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    e.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    let id = e.id.clone();
    app.snapshot();
    app.scene.elements.push(e);
    let program = if cfg!(windows) { ("powershell", &["-NoProfile", "-Command", "Start-Sleep 30"][..]) } else { ("sh", &["-c", "sleep 30"][..]) };
    let mut session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program), 10_000).unwrap();
    session.started_ms = 1; // marks this very shell
    app.terminals.insert(id.clone(), Ok(session));
    app.terminal_focus = Some(id.clone());
    let same = |app: &DrawingApp| app.terminals.get(&id).is_some_and(|s| s.as_ref().is_ok_and(|s| s.started_ms == 1));
    app.undo();
    app.sync_terminals(&ctx);
    assert!(same(&app), "the undo keeps its shell");
    assert_eq!(app.terminal_focus, None, "and takes the keyboard from it");
    app.redo();
    app.sync_terminals(&ctx);
    assert!(same(&app), "the redo brings the same shell back");
    // a change after the undo clears the redo: the shell ends
    app.undo();
    app.snapshot();
    app.sync_terminals(&ctx);
    assert!(!app.terminals.contains_key(&id));
}

#[test]
fn unload_ends_a_terminals_shell_and_reload_brings_it_back_as_undo_steps() {
    use crate::app::ui::{agents::AgentState, context::Target};
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    e.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    let mut page = app.new_element(ElementKind::Rectangle { x: 500.0, y: 0.0, width: 400.0, height: 300.0 });
    page.portal = Some(PortalSpec { name: "Docs".into(), url: "about:blank".into(), ..Default::default() });
    let (id, page_id) = (e.id.clone(), page.id.clone());
    app.scene.elements.extend([e, page]);
    // a shell that couldn't start stands in for one: no process in this test
    app.terminals.insert(id.clone(), Err("x".into()));
    for target in [Target::Element(id.clone()), Target::Element(page_id.clone())] {
        app.context_action(&ctx, &target, "unload");
    }
    assert!(app.scene.elements.iter().all(|e| e.unloaded()));
    assert_eq!(app.undo.len(), 2);
    app.sync_terminals(&ctx);
    assert!(!app.terminals.contains_key(&id), "its shell ends");
    assert_eq!(app.agent_rows()[0].state, AgentState::Idle(None), "and its agent is idle, not ended");
    app.context_action(&ctx, &Target::Element(id.clone()), "unload");
    assert_eq!(app.undo.len(), 2, "unloading it again is no step");
    app.context_action(&ctx, &Target::Element(id), "reload");
    assert!(!app.scene.elements[0].unloaded());
    app.undo();
    assert!(app.scene.elements[0].unloaded(), "Ctrl+Z puts it back unloaded");
    // the page's own ⟳ (the address bar's) loads an unloaded portal back
    app.portal_reload(&page_id);
    assert!(!app.scene.elements[1].unloaded());
}

#[test]
fn ctrl_j_off_the_mac_stays_the_focused_terminals_newline() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = terminal_at(&mut app, &ctx, Pos2::new(400.0, 300.0));
    let ctrl_j = egui::Event::Key { key: Key::J, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    app.terminal_focus = Some(id);
    frame(&mut app, &ctx, vec![ctrl_j.clone()]);
    assert_eq!(app.agents_open, cfg!(target_os = "macos"), "off the Mac the shell gets its ^J");
    app.agents_open = false;
    app.terminal_focus = None;
    frame(&mut app, &ctx, vec![ctrl_j]);
    assert!(app.agents_open, "on the canvas it opens the agents table");
}

#[cfg(windows)]
#[test]
fn ctrl_insert_and_shift_delete_send_nothing_to_the_shell() {
    // egui-winit makes Ctrl+Insert / Shift+Delete the same copy / cut as Ctrl+C / Ctrl+X: only the held
    // key (this thread's keyboard state) tells them apart, and those two must not reach the shell
    unsafe extern "system" {
        fn GetKeyboardState(keys: *mut u8) -> i32;
        fn SetKeyboardState(keys: *const u8) -> i32;
        fn GetKeyState(key: i32) -> i16;
    }
    const VK_INSERT: usize = 0x2D;
    const VK_DELETE: usize = 0x2E;
    let mut saved = [0u8; 256];
    // SAFETY: a 256-byte buffer, as both calls expect
    unsafe { GetKeyboardState(saved.as_mut_ptr()) };
    let mut probe = saved;
    probe[VK_INSERT] = 0x80;
    unsafe { SetKeyboardState(probe.as_ptr()) };
    let held = unsafe { GetKeyState(VK_INSERT as i32) } < 0;
    unsafe { SetKeyboardState(saved.as_ptr()) };
    if !held {
        // an anti-cheat (Arena Breakout's) makes SetKeyboardState a no-op that still says ok
        eprintln!("SetKeyboardState has no effect here: skipped");
        return;
    }
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let id = terminal_at(&mut app, &ctx, Pos2::new(400.0, 300.0));
    let script = "[Console]::TreatControlCAsInput = $true; Write-Host pronto; Write-Host (1..3 | % { [int][Console]::ReadKey($true).KeyChar })";
    let session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(("powershell", &["-NoProfile", "-Command", script][..])), 10_000).unwrap();
    // a ^C before PowerShell takes it as input would end it
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !session.handle.screen_text().contains("pronto") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // ConPTY takes the new input mode a moment after the text shows: a ^C right away still ended it
    std::thread::sleep(std::time::Duration::from_millis(200));
    app.terminals.insert(id.clone(), Ok(session));
    app.terminal_focus = Some(id.clone());
    for (held, event) in [
        (Some(VK_INSERT), egui::Event::Copy),
        (Some(VK_DELETE), egui::Event::Cut),
        (None, egui::Event::Copy),
        (None, egui::Event::Cut),
        (None, egui::Event::Text("a".into())),
    ] {
        let mut keys = saved;
        if let Some(key) = held {
            keys[key] = 0x80;
        }
        unsafe { SetKeyboardState(keys.as_ptr()) };
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), events: vec![event], ..Default::default() };
        ctx.run_ui(raw, |_| app.terminal_overlay(&ctx)).textures_delta.clear();
    }
    unsafe { SetKeyboardState(saved.as_ptr()) };
    let Some(Ok(session)) = app.terminals.get(&id) else { unreachable!() };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    // ConPTY can hand over the last output after the exit: wait for that line too
    let answered = || session.exited() && session.handle.screen_text().lines().last() != Some("pronto");
    while !answered() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(session.exited());
    // ^C and ^X without the keys held, then the a: nothing from the two with Insert / Delete
    let screen = session.handle.screen_text();
    assert_eq!(screen.lines().last(), Some("3 24 97"), "{screen}");
}

#[test]
fn undoing_a_terminals_rope_to_its_ide_says_to_restart_its_claude() {
    let mut app = DrawingApp::default();
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    terminal.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    let mut ide = app.new_element(ElementKind::Rectangle { x: 600.0, y: 0.0, width: 400.0, height: 300.0 });
    ide.ide = Some(IdeSpec { name: "proj".into(), ..Default::default() });
    let (t, i) = (terminal.id.clone(), ide.id.clone());
    app.scene.elements.extend([terminal, ide]);
    app.connect(&t, &i);
    // its claude got that IDE's port when it started
    app.terminal_ides.insert(t.clone(), (i.clone(), false));
    app.now = 1.0;
    app.check_agents();
    assert!(app.status.is_empty(), "still roped: nothing to say");
    let ropes: Vec<Element> = app.scene.elements.iter().filter(|e| e.id != t && e.id != i).cloned().collect();
    app.scene.elements.retain(|e| e.id == t || e.id == i);
    app.now = 2.0;
    app.check_agents();
    assert_eq!(app.status, "Reinicie o terminal \"Revisor\" para o claude dele largar o IDE.");
    app.status.clear();
    app.now = 3.0;
    app.check_agents();
    assert!(app.status.is_empty(), "said once");
    // roped again and undone again: said again (the shell still has that port)
    app.scene.elements.extend(ropes);
    app.now = 4.0;
    app.check_agents();
    app.scene.elements.retain(|e| e.id == t || e.id == i);
    app.now = 5.0;
    app.check_agents();
    assert!(app.status.starts_with("Reinicie o terminal"));
}
