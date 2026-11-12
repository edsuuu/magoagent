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
fn the_wheel_over_an_ide_card_scrolls_the_panel_only_as_far_as_its_text() {
    let ctx = Context::default();
    let dir = std::env::temp_dir().join(format!("magoagent-ide-wheel-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("long.txt"), (0..80).map(|n| format!("linha {n}\n")).collect::<String>()).unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut e = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: IDE_SIZE.x, height: IDE_SIZE.y });
    e.ide = Some(IdeSpec { name: "proj".into(), root: dir.to_string_lossy().into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    frame(&mut app, &ctx, vec![]);
    let row = card_row_at(&app, 0, 0);
    double_click(&mut app, &ctx, row, 10.0);
    let local = local_bounds(&app.scene.elements[0]);
    // over the panel's text the wheel scrolls it, down to the last line and no further
    let in_panel = Pos2::new(local.left() + 400.0, local.top() + 150.0);
    assert!(app.ide_wheel(in_panel, -40.0));
    assert_eq!((app.ides[&id].panel_scroll, app.ides[&id].card_scroll), (40.0, 0.0));
    assert!(app.ide_wheel(in_panel, -5000.0));
    assert_eq!(app.ides[&id].panel_scroll, 80.0 * crate::ide::card::LINE - (local.height() - 26.0 - 30.0 - 22.0));
    // over the tree, one file has nothing to scroll; outside any card the wheel is the canvas's
    assert!(app.ide_wheel(Pos2::new(local.left() + 100.0, local.top() + 150.0), -40.0));
    assert_eq!(app.ides[&id].card_scroll, 0.0);
    assert!(!app.ide_wheel(Pos2::new(-500.0, -500.0), -40.0));
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(not(target_os = "linux"))]
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
    let mut ide = app.new_element(ElementKind::Rectangle { x: 60.0, y: 340.0, width: 320.0, height: 200.0 });
    ide.ide = Some(IdeSpec { name: "proj".into(), root: dir.to_string_lossy().into(), ..Default::default() });
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
    // the IDE: its title strip enlarges it and it takes the keyboard; Esc puts it back
    let at = app.to_screen(Pos2::new(120.0, 350.0));
    double_click(&mut app, &ctx, at, 4.0);
    assert_eq!((app.terminal_elevated.as_ref(), app.ide_focus.as_ref()), (Some(&ide_id), Some(&ide_id)));
    let esc = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame_at(&mut app, &ctx, Some(5.0), vec![]);
    frame_at(&mut app, &ctx, Some(6.0), vec![esc]);
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
