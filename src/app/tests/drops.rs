//! Files dropped on the canvas, pictures pasted onto it.

use super::*;

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);

    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn each_dropped_file_becomes_the_right_node() {
    let dir = std::env::temp_dir().join(format!("magoagent-drops-{}", std::process::id()));

    std::fs::create_dir_all(dir.join("projeto")).unwrap();
    let mut png = Vec::new();

    image::DynamicImage::new_rgba8(40, 20).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();

    let paths = [
        write(&dir, "ler.md", b"# Titulo\r\ncorpo"),
        write(&dir, "solto.txt", b"so texto"),
        write(&dir, "pagina.html", b"<p>oi</p>"),
        dir.join("projeto"),
        write(&dir, "foto.png", &png),
        write(&dir, "dados.bin", &[0, 159, 146, 150]),
        write(&dir, "enorme.txt", &vec![b'a'; (1 << 20) + 1]),
    ];

    let mut app = DrawingApp::default();
    let made = app.drop_on_canvas(&paths, Pos2::new(500.0, 300.0)).unwrap();

    assert_eq!(made.lines().count(), 7);
    let els = &app.scene.elements;

    let label = |note: &Element| match &els.iter().find(|e| e.id == format!("{}-label", note.id)).unwrap().kind {
        ElementKind::Text { value, .. } => value.clone(),
        _ => unreachable!(),
    };

    // .md and .txt: a note with the content (markdown formatted, text as typed)
    let notes: Vec<&Element> = els.iter().filter(|e| e.created.is_some()).collect();

    assert_eq!((label(notes[0]), notes[0].note.raw), ("# Titulo\ncorpo".to_string(), false));
    assert_eq!((label(notes[1]), notes[1].note.raw), ("so texto".to_string(), true));
    // .html: a portal on the file; a folder: a file tree
    assert!(els.iter().any(|e| e.portal.as_ref().is_some_and(|p| p.name == "pagina" && p.url.starts_with("file:///") && p.url.ends_with("/pagina.html"))));
    assert!(els.iter().any(|e| e.filetree.as_ref().is_some_and(|t| t.root.ends_with("projeto"))));
    // a picture: a rectangle carrying it
    assert!(els.iter().any(|e| e.image.as_ref().is_some_and(|i| i.starts_with("data:image/png;base64,"))));
    // anything else (and a text file too big for a note): a card with name, size and the buttons that open it
    let cards: Vec<&Element> = notes[2..].to_vec();

    assert_eq!(cards.len(), 2);
    assert_eq!(label(cards[0]), "dados.bin\n1 KB");
    assert_eq!(label(cards[1]), "enorme.txt\n1,0 MB");
    let actions: Vec<&str> = cards[0].note.buttons.iter().map(|b| b.action.as_str()).collect();

    assert_eq!(actions, ["open_path", "open_default"]);
    assert!(app.drop_on_canvas(&[dir.join("nada.md")], Pos2::ZERO).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_svg_goes_in_as_a_picture() {
    let dir = std::env::temp_dir().join(format!("magoagent-drops-svg-{}", std::process::id()));

    std::fs::create_dir_all(&dir).unwrap();
    let svg = write(&dir, "a.svg", br#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="red"/></svg>"#);
    let mut app = DrawingApp::default();

    app.drop_on_canvas(&[svg], Pos2::ZERO).unwrap();
    assert!(app.scene.elements[0].image.as_ref().is_some_and(|i| i.starts_with("data:image/png;base64,")));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_image_pasted_on_the_canvas_becomes_a_picture() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let shot = egui::ColorImage::new([30, 20], vec![egui::Color32::RED; 600]);

    frame(&mut app, &ctx, vec![egui::Event::PasteImage(std::sync::Arc::new(shot))]);
    assert_eq!(app.scene.elements.len(), 1);
    assert!(app.scene.elements[0].image.as_ref().is_some_and(|i| i.starts_with("data:image/png;base64,")));
    // with the chat open the paste belongs to its box, not the canvas behind it
    app.chat_open = true;
    frame(&mut app, &ctx, vec![egui::Event::PasteImage(std::sync::Arc::new(egui::ColorImage::new([3, 2], vec![egui::Color32::RED; 6])))]);
    assert_eq!(app.scene.elements.len(), 1);
}

/// A picture pasted while a chat card's box on the canvas has the keys goes into that box, as on the chat screen. It used
/// to go nowhere: the canvas left it to the box, and only the chat screen and the mini chat took pictures.
#[test]
fn an_image_pasted_in_a_chat_cards_box_goes_into_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = push_chat(&mut app, "Luna");

    // the test's frame and the chat cards drawn over the canvas, as the app draws them
    let with_cards = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))), events, ..Default::default() };

        let mut output = ctx.run_ui(raw, |ui| {
            app.handle_shortcuts(&ctx);

            egui::CentralPanel::no_frame().show(ui, |ui| {
                let (response, _) = ui.allocate_painter(ui.available_size(), Sense::CLICK | Sense::DRAG);

                app.handle_canvas(&response, response.rect);
            });

            app.chat_node_overlay(&ctx);
        });

        output.textures_delta.clear();
    };

    with_cards(&mut app, vec![]);
    let card = app.add_chat_node(&id, None).unwrap();

    // a small card shows only its last words; grown (or enlarged, "Tela cheia no canvas") it has its box
    if let ElementKind::Rectangle { width, height, .. } = &mut app.scene.elements[card].kind {
        (*width, *height) = (500.0, 600.0);
    }

    with_cards(&mut app, vec![]);
    with_cards(&mut app, vec![]);
    ctx.memory_mut(|m| m.request_focus(crate::app::chat::node_ui::card_box_id(&id)));
    with_cards(&mut app, vec![]);
    assert_eq!(app.focused_chat_card(&ctx).as_deref(), Some(id.as_str()));
    let elements = app.scene.elements.len();

    with_cards(&mut app, vec![egui::Event::PasteImage(std::sync::Arc::new(egui::ColorImage::new([3, 2], vec![egui::Color32::RED; 6])))]);
    assert!(app.has_attached(), "the picture waits in the card's box");
    assert_eq!(app.scene.elements.len(), elements, "not on the canvas");
    assert_eq!(app.focused_chat_card(&ctx).as_deref(), Some(id.as_str()), "the box keeps the keys");
}

#[test]
fn the_mcp_drops_files_and_never_runs_programs() {
    let dir = std::env::temp_dir().join(format!("magoagent-drops-act-{}", std::process::id()));

    std::fs::create_dir_all(&dir).unwrap();
    let note = write(&dir, "n.md", b"oi");
    let exe = write(&dir, "faz.bat", b"echo oi");
    let mut app = DrawingApp::default();
    let made = app.act("canvas_drop", &json!({ "paths": [note.to_string_lossy()], "x": 100, "y": 50 })).unwrap();

    assert_eq!(made, "n.md: nota");
    assert!(app.act("canvas_drop", &json!({ "paths": [] })).is_err());
    assert!(app.act("open_default", &json!({ "path": exe.to_string_lossy() })).unwrap_err().contains("é um programa"));
    assert!(app.act("open_default", &json!({ "path": note.to_string_lossy() })).unwrap().contains("n.md aberto"));
    std::fs::remove_dir_all(&dir).ok();
}
