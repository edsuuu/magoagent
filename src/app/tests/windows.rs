use super::*;

#[test]
fn a_double_click_on_a_chat_terminals_title_enlarges_it_and_a_click_on_its_body_opens_its_chat() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 60.0, y: 150.0, width: 300.0, height: 200.0 });

    terminal.terminal = Some(TerminalSpec { name: "t".into(), cwd: "/tmp".into(), chat: true, ..Default::default() });
    let id = terminal.id.clone();

    app.scene.elements = vec![terminal];
    frame(&mut app, &ctx, vec![]);
    let title = app.to_screen(Pos2::new(200.0, 158.0));

    double_click(&mut app, &ctx, title, 1.0);
    assert_eq!((app.terminal_elevated.as_ref(), app.chat_open), (Some(&id), false));
    app.set_elevated(None);
    frame_at(&mut app, &ctx, Some(3.0), vec![]);
    let body = app.to_screen(Pos2::new(200.0, 280.0));

    for (t, pressed) in [(4.0, true), (4.05, false)] {
        frame_at(&mut app, &ctx, Some(t), vec![egui::Event::PointerMoved(body), button(body, pressed)]);
    }

    assert!(app.chat_open);
}

#[test]
fn a_lifted_portal_hides_the_pages_of_the_other_portals() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let mut a = app.new_element(ElementKind::Rectangle { x: 60.0, y: 60.0, width: 320.0, height: 240.0 });

    a.portal = Some(PortalSpec { name: "A".into(), url: "about:blank".into(), ..Default::default() });
    let mut b = app.new_element(ElementKind::Rectangle { x: 420.0, y: 60.0, width: 320.0, height: 240.0 });

    b.portal = Some(PortalSpec { name: "B".into(), url: "about:blank".into(), ..Default::default() });
    app.scene.elements = vec![a, b];
    frame(&mut app, &ctx, vec![]);
    assert!(app.portal_page_rect(&app.scene.elements[1]).is_some());
    let (a_id, b_el, a_el) = (app.scene.elements[0].id.clone(), app.scene.elements[1].clone(), app.scene.elements[0].clone());

    app.set_elevated(Some(a_id));
    assert!(app.portal_page_rect(&b_el).is_none(), "B's page would float over the enlarged A");
    assert!(app.portal_page_rect(&a_el).is_some(), "A shows its own page");
}

/// A portal in full screen has its way out in its bar, the button before the trash, as the other windows do; Esc too.
#[test]
fn a_portal_in_full_screen_leaves_it_by_its_bar_or_by_esc() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut portal = app.new_element(ElementKind::Rectangle { x: 60.0, y: 60.0, width: 320.0, height: 240.0 });

    portal.portal = Some(PortalSpec { name: "A".into(), url: "about:blank".into(), ..Default::default() });
    let id = portal.id.clone();

    app.scene.elements = vec![portal];
    app.selected = vec![0];
    app.set_elevated(Some(id.clone()));

    for time in [1.0, 2.0, 3.0] {
        frame_at(&mut app, &ctx, Some(time), vec![]);
    }

    let bar = ctx.memory(|m| m.area_rect(crate::app::ui::nodes::context_bar_id(&app.scene.elements[0]))).expect("the bar shows");
    // from the right: the margin (6), the trash (30), a gap (5), the divider (1), a gap (5), then the button's middle (15)
    let at = Pos2::new(bar.right() - 62.0, bar.center().y);

    frame_at(&mut app, &ctx, Some(4.0), vec![egui::Event::PointerMoved(at), button(at, true)]);
    frame_at(&mut app, &ctx, Some(4.1), vec![button(at, false)]);
    assert_eq!(app.terminal_elevated, None, "Sair da tela cheia");

    app.set_elevated(Some(id.clone()));
    frame_at(&mut app, &ctx, Some(5.0), vec![key_press(Key::Escape)]);
    assert_eq!(app.terminal_elevated, None, "Esc");
}

#[test]
fn a_double_click_on_a_title_lifts_a_window_and_another_puts_it_back() {
    let ctx = Context::default();

    install_fonts(&ctx, false);
    let dir = std::env::temp_dir().join(format!("magoagent-lift-{}", std::process::id()));

    fs::create_dir_all(&dir).unwrap();
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let mut portal = app.new_element(ElementKind::Rectangle { x: 60.0, y: 60.0, width: 320.0, height: 240.0 });

    portal.portal = Some(PortalSpec { name: "Docs".into(), url: "https://example.com".into(), ..Default::default() });
    let mut ide = app.new_element(ElementKind::Rectangle { x: 60.0, y: 340.0, width: 320.0, height: 300.0 });

    ide.filetree = Some(FileTreeSpec { root: dir.to_string_lossy().into(), ..Default::default() });
    // clear of the context bar, which hangs over the top middle of the canvas
    let [note, text] = app.new_note(Rect::from_min_size(Pos2::new(620.0, 250.0), Vec2::splat(160.0)), "Oi".into());

    app.scene.elements = vec![portal, ide, note, text];
    frame(&mut app, &ctx, vec![]);
    let (portal_id, ide_id, note_id) = (app.scene.elements[0].id.clone(), app.scene.elements[1].id.clone(), app.scene.elements[2].id.clone());
    // the portal: its title strip lifts it, a double-click on its lifted title puts it back
    let at = app.to_screen(Pos2::new(120.0, 70.0));

    double_click(&mut app, &ctx, at, 1.0);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&portal_id));
    frame_at(&mut app, &ctx, Some(2.0), vec![]);
    let lifted = app.terminal_frame(&app.scene.elements[0]).0;

    double_click(&mut app, &ctx, lifted.min + Vec2::new(40.0, 8.0), 3.0);
    assert_eq!(app.terminal_elevated, None);
    // the file tree: its title strip enlarges it, and another double-click on it puts it back
    let at = app.to_screen(Pos2::new(120.0, 350.0));

    double_click(&mut app, &ctx, at, 4.0);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&ide_id));
    frame_at(&mut app, &ctx, Some(5.0), vec![]);
    let lifted = app.terminal_frame(&app.scene.elements[1]).0;

    double_click(&mut app, &ctx, lifted.min + Vec2::new(40.0, 8.0), 6.0);
    assert_eq!(app.terminal_elevated, None);
    // the note: its header lifts it with its text ready to type; a double-click on the header puts it back
    let at = app.to_screen(Pos2::new(700.0, 260.0));

    double_click(&mut app, &ctx, at, 7.0);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&note_id));
    assert!(app.editing.is_some(), "the lifted note types at once");
    frame_at(&mut app, &ctx, Some(8.0), vec![]);
    let lifted = app.terminal_frame(&app.scene.elements[2]).0;

    double_click(&mut app, &ctx, lifted.min + Vec2::new(20.0, 4.0), 9.0);
    assert_eq!((app.terminal_elevated.as_ref(), app.editing), (None, None));
    let _ = fs::remove_dir_all(&dir);
}

/// What the agents open never brings the app's window to the front; a question or a permission an agent waits on does,
/// on the chat that asks: its card selected on the canvas, or the chat itself when it has no card there.
#[test]
fn only_an_agent_waiting_on_the_person_brings_the_window_to_the_front() {
    let mut app = DrawingApp::default();
    let raised = |app: &mut DrawingApp| std::mem::take(&mut app.viewport_commands).contains(&egui::ViewportCommand::Focus);
    let id = push_chat(&mut app, "Luna");

    app.maestro_show("canvas");
    assert!(!raised(&mut app), "showing a screen stays in place");
    let card = app.add_chat_node(&id, None).unwrap();

    app.selected.clear();
    assert!(app.maestro_look(&id, None).is_ok());
    assert!(!raised(&mut app), "focus_canvas moves the view, not the window");
    let (reply, _answer) = std::sync::mpsc::channel();

    app.ask_person(&id, &json!({ "tool_name": "AskUserQuestion", "input": { "questions": [{ "question": "Qual?", "options": [] }] } }), reply);
    assert!(raised(&mut app), "a question brings it");
    assert_eq!((app.selected.clone(), app.chat_open), (vec![card], false), "on the asker's card");
    // with no card on the canvas, the chat itself
    let other = push_chat(&mut app, "Ana");
    let (reply, _answer) = std::sync::mpsc::channel();

    app.ask_person(&other, &json!({ "tool_name": "Bash", "input": { "command": "ls" } }), reply);
    assert!(raised(&mut app) && app.chat_open, "a permission too, on its chat");
}

/// The press that brings the window back from another program lands wherever the pointer was: the note in focus (and
/// its typing) stays as the person left it. It used to end the typing, and getting back to it took a double-click more.
#[test]
fn coming_back_from_another_program_keeps_the_note_in_focus() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let [note, text] = app.new_note(Rect::from_min_size(Pos2::new(300.0, 250.0), Vec2::splat(160.0)), "Oi".into());

    app.scene.elements = vec![note, text];
    frame(&mut app, &ctx, vec![]);
    let id = app.scene.elements[0].id.clone();
    let header = app.to_screen(Pos2::new(380.0, 260.0));

    double_click(&mut app, &ctx, header, 1.0);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&id));
    frame_at(&mut app, &ctx, Some(2.0), vec![]);
    let lifted = app.terminal_frame(&app.scene.elements[0]).0;
    let margin = Pos2::new(lifted.left() / 2.0, lifted.center().y);

    assert!(lifted.left() > 8.0, "{lifted:?}");

    let press = |app: &mut DrawingApp, at: f64| {
        app.now = at;
        frame_at(app, &ctx, Some(at), vec![egui::Event::PointerMoved(margin), button(margin, true)]);
        frame_at(app, &ctx, Some(at + 0.05), vec![button(margin, false)]);
    };

    // back from another program: its press on the margin only brings the window
    app.window_back = 3.0;
    press(&mut app, 3.1);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&id));
    assert!(app.editing.is_some(), "still typing");
    // a press later is the person's: it ends the typing, and the next one puts the note back on the canvas
    press(&mut app, 5.0);
    assert_eq!((app.terminal_elevated.as_ref(), app.editing), (Some(&id), None));
    press(&mut app, 6.0);
    assert_eq!(app.terminal_elevated, None);
}

/// The note in focus stays in the middle, and its sides pull it wider or narrower together; the width stays for the
/// next note lifted.
#[test]
fn the_note_in_focus_widens_by_its_sides() {
    use crate::app::terminal::note_lift_width;

    assert_eq!(note_lift_width(2000.0, 1400.0), 1400.0 - 96.0, "the canvas around it stays in sight");
    assert_eq!(note_lift_width(100.0, 1400.0), 320.0);
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let [note, text] = app.new_note(Rect::from_min_size(Pos2::new(300.0, 250.0), Vec2::splat(160.0)), "Oi".into());

    app.scene.elements = vec![note, text];
    app.enlarge(0);
    app.now = 10.0;
    let lifted = app.terminal_frame(&app.scene.elements[0]).0;
    let clock = std::cell::Cell::new(10.0);

    let grip = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        clock.set(clock.get() + 0.05);

        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            time: Some(clock.get()),
            events,
            ..Default::default()
        };

        let _ = ctx.run_ui(raw, |_| app.dock_grip(&ctx));
    };

    // the right side pulled 100 in: 200 narrower, still in the middle
    let (from, to) = (Pos2::new(lifted.right(), lifted.center().y), Pos2::new(lifted.right() - 100.0, lifted.center().y));

    // a new area sizes itself on its first frame, and takes the pointer from the next
    grip(&mut app, vec![]);
    grip(&mut app, vec![egui::Event::PointerMoved(from)]);
    grip(&mut app, vec![button(from, true)]);

    for step in 1..=5 {
        grip(&mut app, vec![egui::Event::PointerMoved(from.lerp(to, step as f32 / 5.0))]);
    }

    grip(&mut app, vec![button(to, false)]);
    let narrower = app.terminal_frame(&app.scene.elements[0]).0;

    assert!((narrower.width() - (lifted.width() - 200.0)).abs() < 1.0, "{lifted:?} → {narrower:?}");
    assert!((narrower.center().x - lifted.center().x).abs() < 1.0, "still in the middle");
    assert_eq!(app.prefs.note_lift_width, Some(narrower.width()), "kept for the next one");
}
