use super::*;

#[test]
fn extra_tools() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // nota adesiva: a click drops a 260×160 note, already editing its label
    app.tool = Tool::Sticky;
    drag(&mut app, &ctx, Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    assert_eq!(element_bounds(&app.scene.elements[0]).size(), NOTE_SIZE);
    assert!(app.scene.elements[0].created.is_some());
    assert_eq!(app.editing, Some(1));
    app.finish_text();
    app.tool = Tool::Sticky;
    drag(&mut app, &ctx, Pos2::new(700.0, 300.0), Pos2::new(800.0, 340.0));
    // a small drag stops at the note's smallest
    assert_eq!(element_bounds(app.scene.elements.iter().rev().find(|e| e.created.is_some()).unwrap()).size(), NOTE_MIN);
    app.finish_text();
    let back = crate::tests::round_trip(&app.scene.elements);
    assert_eq!(back[0].created, app.scene.elements[0].created);
    // dark mode uses the web's invert(93%) hue-rotate(180deg): the note turns brown
    assert_eq!(themed(Color32::from_rgb(0xff, 0xdf, 0x6b), true), Color32::from_rgb(75, 48, 0));
    // preenchimento paints the shape under the pointer
    app.scene.elements = vec![app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 100.0, height: 100.0 })];
    app.scene.elements[0].style.fill = None;
    app.fill_at(Pos2::new(150.0, 150.0));
    assert!(app.scene.elements[0].style.fill.is_some());
    // laço selects only what it surrounds
    app.scene.elements.push(app.new_element(ElementKind::Rectangle { x: 500.0, y: 100.0, width: 50.0, height: 50.0 }));
    app.select_in_lasso(&[Pos2::new(50.0, 50.0), Pos2::new(250.0, 50.0), Pos2::new(250.0, 250.0), Pos2::new(50.0, 250.0)]);
    assert_eq!(app.selected, vec![0]);
    // desenhar para criar forma: a round scribble becomes an ellipse, a straight one a line
    let circle: Vec<[f32; 2]> = (0..=40)
        .map(|i| {
            let a = i as f32 / 40.0 * std::f32::consts::TAU;
            [100.0 + 50.0 * a.cos(), 100.0 + 50.0 * a.sin()]
        })
        .collect();
    assert!(matches!(recognize_shape(&app, &circle).unwrap().kind, ElementKind::Ellipse { .. }));
    let square = [[0.0, 0.0], [50.0, 0.0], [100.0, 0.0], [100.0, 50.0], [100.0, 100.0], [50.0, 100.0], [0.0, 100.0], [0.0, 50.0], [0.0, 2.0]];
    assert!(matches!(recognize_shape(&app, &square).unwrap().kind, ElementKind::Rectangle { .. }));
    let diamond = [[50.0, 0.0], [100.0, 50.0], [50.0, 100.0], [0.0, 50.0], [49.0, 1.0]];
    assert!(matches!(recognize_shape(&app, &diamond).unwrap().kind, ElementKind::Diamond { .. }));
    app.scene.elements.clear();
    let straight: Vec<[f32; 2]> = (0..10).map(|i| [i as f32 * 10.0, i as f32 * 5.0]).collect();
    assert!(matches!(recognize_shape(&app, &straight).unwrap().kind, ElementKind::Line { arrow: false, .. }));
    // a shaft with a head scribbled at its tip is an arrow
    let mut arrow: Vec<[f32; 2]> = (0..=20).map(|i| [i as f32 * 10.0, 0.0]).collect();
    arrow.extend([[185.0, -15.0], [200.0, 0.0], [185.0, 15.0], [200.0, 0.0], [185.0, -15.0], [200.0, 0.0], [185.0, 15.0]]);
    assert!(matches!(recognize_shape(&app, &arrow).unwrap().kind, ElementKind::Line { arrow: true, .. }));
    // a zigzag is no shape at all
    let zigzag: Vec<[f32; 2]> = (0..12).map(|i| [i as f32 * 20.0, if i % 2 == 0 { 0.0 } else { 80.0 }]).collect();
    assert!(recognize_shape(&app, &zigzag).is_none());
}

#[test]
fn ai_drawings_replay_with_the_agent_name() {
    let path = std::env::temp_dir().join(format!("magoagent-replay-{}.mago", std::process::id()));
    let mut app = DrawingApp::default();
    app.autosave = Some(path.clone());
    let mut external = app.scene.clone();
    external.agent = Some("Claude Code".into());
    external.elements.push(app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 100.0, height: 50.0 }));
    external.elements.push(app.new_element(ElementKind::Text { x: 10.0, y: 10.0, value: "oi".into() }));
    write_scene_file(&path, &external).unwrap();
    app.sync_from_file();
    assert_eq!(app.reveal.len(), 2);
    assert_eq!(app.agent.as_ref().map(|a| a.0.as_str()), Some("Claude Code"));
    // the front element is drawn over time, then the next one starts
    let first = app.scene.elements[0].clone();
    assert_eq!(app.reveal_progress(&first), 0.0);
    app.now += 10.0;
    assert_eq!(app.reveal_progress(&first), 1.0);
    // the file the app writes back never claims to be the AI's
    assert_eq!(app.scene.agent, None);
    let _ = fs::remove_file(path);
}

#[test]
fn image_round_trip() {
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 255])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    assert_eq!(base64_decode(&base64(&png)).unwrap(), png);
    let url = format!("data:image/png;base64,{}", base64(&png));
    let app = DrawingApp::default();
    let mut element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 40.0, height: 20.0 });
    element.image = Some(url.clone().into());
    let scene = Scene { elements: vec![element], ..Scene::default() };
    let interop = json!(interop_elements(&scene.elements));
    assert_eq!(interop[0]["type"], "image");
    let back = from_interop_elements(&interop);
    assert_eq!(back[0].image.as_deref(), Some(url.as_str()));
    // images are grabbed anywhere inside, and come out in the PNG export
    assert!(back[0].contains(Pos2::new(20.0, 10.0), 1.0));
    let out = image::load_from_memory(&render_svg_png(&scene_svg(&scene, None), 1.0).unwrap()).unwrap().to_rgba8();
    assert_eq!(out.get_pixel(30, 20).0, [255, 0, 0, 255]);
}

#[test]
fn snapshot_shares_image_data() {
    let mut app = DrawingApp::default();
    let mut element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 40.0, height: 20.0 });
    element.image = Some("data:image/png;base64,abc".into());
    app.scene.elements.push(element);
    app.snapshot();
    assert!(std::sync::Arc::ptr_eq(app.scene.elements[0].image.as_ref().unwrap(), app.undo[0].elements[0].image.as_ref().unwrap()));
}

#[test]
fn text_png_contains_visible_pixels() {
    let mut app = DrawingApp::default();
    app.scene.elements.push(app.new_element(ElementKind::Text { x: 20.0, y: 40.0, value: "Planejamento".into() }));
    let png = render_svg_png(&scene_svg(&app.scene, Some(WHITE)), 1.0).unwrap();
    let img = image::load_from_memory(&png).unwrap().into_rgba8();
    assert!(img.pixels().filter(|p| p[0] < 100 && p[1] < 100 && p[2] < 100).count() > 30);
}

#[test]
fn terminal_tool_places_a_focused_terminal() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.select_tool(Tool::Terminal);
    drag(&mut app, &ctx, Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    // the dialog asks for the shell first, named after it; a quick start's command names it after that one
    assert!(app.scene.elements.is_empty());
    assert_eq!(app.terminal_dialog.as_ref().unwrap().spec.name, "Shell");
    let dialog = app.terminal_dialog.as_mut().unwrap();
    (dialog.spec.name, dialog.spec.command) = (String::new(), "claude".into());
    app.confirm_terminal_dialog();
    let e = app.scene.elements[0].clone();
    assert_eq!(e.terminal.as_ref().unwrap().name, "Claude Code");
    assert_eq!(grid_size(local_bounds(&e).size()), (80, 24));
    assert_eq!(app.terminal_focus.as_ref(), Some(&e.id));
    assert_eq!(app.tool, Tool::Select);
    assert!(e.contains(local_bounds(&e).center(), 1.0), "grabbed anywhere inside");
    assert_eq!(crate::tests::round_trip(&app.scene.elements)[0].terminal, e.terminal);
    // picking a tool stops typing into it; a double-click on it types into it again
    app.select_tool(Tool::Rectangle);
    assert_eq!(app.terminal_focus, None);
    app.select_tool(Tool::Select);
    let at = app.to_screen(local_bounds(&e).center());
    for (t, pressed) in [(1.0, true), (1.05, false), (1.15, true), (1.2, false)] {
        frame_at(&mut app, &ctx, Some(t), vec![egui::Event::PointerMoved(at), button(at, pressed)]);
    }
    assert_eq!(app.terminal_focus.as_ref(), Some(&e.id));
    assert_eq!(app.selected, vec![0], "stays selected while typing, so its bar stays up");
    // a double-click on its title strip enlarges it, another one puts it back (to the right of
    // the top-left buttons, the toolbar and the terminal's bar under it, which reach the strip off the Mac)
    let title = app.to_screen(local_bounds(&e).left_top() + Vec2::new(120.0, 10.0));
    for (t, pressed) in [(3.0, true), (3.05, false), (3.15, true), (3.2, false)] {
        frame_at(&mut app, &ctx, Some(t), vec![egui::Event::PointerMoved(title), button(title, pressed)]);
    }
    assert_eq!(app.terminal_elevated.as_ref(), Some(&e.id));
    assert_eq!(app.unique_terminal_name("Claude Code", None), "Claude Code 2");
}

#[test]
fn one_click_focuses_a_terminal_and_its_title_still_drags_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut e = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 400.0, height: 300.0 });
    e.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    // the body: focused at once, selected, not moved
    let body = app.to_screen(Pos2::new(300.0, 250.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(body), button(body, true)]);
    frame(&mut app, &ctx, vec![button(body, false)]);
    assert_eq!((app.terminal_focus.as_ref(), &app.selected[..], app.moving), (Some(&id), &[0][..], false));
    // the title strip is still the handle that drags it
    let before = element_bounds(&app.scene.elements[0]);
    let title = app.to_screen(Pos2::new(300.0, 110.0));
    drag(&mut app, &ctx, title, title + Vec2::new(60.0, 40.0));
    assert_eq!(element_bounds(&app.scene.elements[0]).min, before.min + Vec2::new(60.0, 40.0));
}
