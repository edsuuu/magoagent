//! Reading a long note: a text file of a chat opened in a note sized to read it (notes/file.rs), the note resized by
//! its corner and edges, and its text scrolled by the wheel (notes/scroll.rs).

use super::*;
use crate::app::chat::direct::NewChat;

fn long_text(lines: usize) -> String {
    let mut text = vec!["# Gabarito".to_string()];
    text.extend((1..=lines).map(|line| format!("Linha {line}: a resposta certa e o porquê dela, para conferir.")));
    text.join("\n")
}

#[test]
fn a_text_file_opens_in_a_note_sized_to_read_beside_its_chats_card() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &[Preset::new("Claude Code", "claude")]);
    let chat = app.chat_agent.clone().unwrap();
    let card = local_bounds(&app.scene.elements[app.chat_node(&chat).unwrap()]);
    let dir = std::env::temp_dir().join(format!("magoagent-note-file-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let (long, short, plain) = (dir.join("gabarito.md"), dir.join("curto.md"), dir.join("lista.txt"));
    fs::write(&long, long_text(200)).unwrap();
    fs::write(&short, "# Curto\nsó isto").unwrap();
    fs::write(&plain, "um *dois*").unwrap();
    app.chat_open = true;
    assert_eq!(app.open_file_as_note(&long, Some(&chat)).unwrap(), "gabarito.md aberto numa nota no canvas.");
    let notes = |app: &DrawingApp| (0..app.scene.elements.len()).filter(|index| app.scene.elements[*index].created.is_some()).collect::<Vec<_>>();
    let note = notes(&app)[0];
    let bounds = local_bounds(&app.scene.elements[note]);
    // beside the chat's card, wide enough to read and as tall as most of the view (600 here)
    assert_eq!(bounds.min, Pos2::new(card.right() + 60.0, card.top()));
    assert_eq!(bounds.size(), Vec2::new(560.0, 480.0));
    assert_eq!(app.note_text(note), long_text(200));
    assert!(!app.scene.elements[note].note.raw, "markdown is formatted");
    assert_eq!(app.selected, [note]);
    assert!(!app.chat_open, "the chat screen gives way to the canvas");
    // the same file again: the note it is in
    assert!(app.open_file_as_note(&long, Some(&chat)).unwrap().contains("já estava"));
    assert_eq!(notes(&app).len(), 1);
    // a short one as tall as its text; plain text as typed
    app.open_file_as_note(&short, None).unwrap();
    let height = local_bounds(&app.scene.elements[*notes(&app).last().unwrap()]).height();
    assert!((NOTE_MIN.y..150.0).contains(&height), "{height}");
    app.open_file_as_note(&plain, None).unwrap();
    assert!(app.scene.elements[*notes(&app).last().unwrap()].note.raw);
    // what is not a text file, or not there, is said
    let binary = dir.join("dados.bin");
    fs::write(&binary, [0u8, 159, 146]).unwrap();
    assert!(app.open_file_as_note(&binary, None).is_err());
    assert!(app.open_file_as_note(&dir.join("nada.md"), None).is_err());
    // the MCP opens it the same way
    let before = notes(&app).len();
    fs::write(dir.join("outro.md"), "outro").unwrap();
    assert!(app.act("open_as_note", &json!({ "path": dir.join("outro.md").to_string_lossy() })).unwrap().contains("outro.md aberto"));
    assert_eq!(notes(&app).len(), before + 1);
    assert!(app.act("open_as_note", &json!({})).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_click_on_a_text_files_card_in_the_chat_opens_its_note() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let dir = std::env::temp_dir().join(format!("magoagent-note-card-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("gabarito.md");
    fs::write(&path, "# Gabarito\n1. b").unwrap();
    let words = format!("Pronto:\n\n[gabarito.md]({})", path.to_string_lossy());
    let run = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), events, ..Default::default() };
        let mut output = ctx.run_ui(raw, |ui| {
            let c = app.colors();
            app.agent_words(ui, &words, 600.0, &c);
        });
        output.textures_delta.clear();
        output.shapes.iter().find_map(|shape| match &shape.shape {
            Shape::Text(text) if text.galley.text() == "gabarito.md" => Some(text.visual_bounding_rect()),
            _ => None,
        })
    };
    let at = run(&mut app, vec![]).expect("the file's card").center();
    run(&mut app, vec![egui::Event::PointerMoved(at), button(at, true)]);
    run(&mut app, vec![button(at, false)]);
    // the next frame's chat tick opens it
    app.chat_nodes_tick(&ctx);
    let note = app.scene.elements.iter().position(|element| element.created.is_some()).expect("its note on the canvas");
    assert_eq!(app.note_text(note), "# Gabarito\n1. b");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_note_resizes_by_its_corner_and_its_edges() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(100.0, 150.0), Vec2::new(300.0, 200.0)), long_text(40));
    app.scene.elements = vec![note, label];
    frame(&mut app, &ctx, vec![]);
    // unselected: the bottom-right corner makes it wider and taller
    let corner = app.to_screen(Pos2::new(400.0, 350.0));
    drag(&mut app, &ctx, corner, corner + Vec2::new(60.0, 40.0));
    assert_eq!(local_bounds(&app.scene.elements[0]), Rect::from_min_size(Pos2::new(100.0, 150.0), Vec2::new(360.0, 240.0)));
    // the right edge only its width, the bottom edge only its height
    let right = app.to_screen(Pos2::new(460.0, 300.0));
    drag(&mut app, &ctx, right, right + Vec2::new(100.0, 0.0));
    assert_eq!(local_bounds(&app.scene.elements[0]).size(), Vec2::new(460.0, 240.0));
    let bottom = app.to_screen(Pos2::new(300.0, 390.0));
    drag(&mut app, &ctx, bottom, bottom + Vec2::new(0.0, 150.0));
    assert_eq!(local_bounds(&app.scene.elements[0]), Rect::from_min_size(Pos2::new(100.0, 150.0), Vec2::new(460.0, 390.0)));
}

#[test]
fn the_wheel_scrolls_a_long_notes_text_and_leaves_its_header_and_ctrl_to_the_canvas() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(300.0, 200.0)), long_text(60));
    let [short, short_label] = app.new_note(Rect::from_min_size(Pos2::new(500.0, 100.0), Vec2::new(200.0, 200.0)), "pouco".into());
    let (id, short_id) = (note.id.clone(), short.id.clone());
    app.scene.elements = vec![note, label, short, short_label];
    // a frame of the canvas painted (what each note hides is learned there) with the wheel at `at`, and the wheel's
    // frames after it: whether a note took it, and where the first line of the long one was painted
    let wheel = |app: &mut DrawingApp, at: Pos2, modifiers: Modifiers| {
        let (mut took, mut first) = (false, None);
        for step in 0..12 {
            let mut events = vec![egui::Event::ModifiersChanged(modifiers), egui::Event::PointerMoved(at)];
            if step == 0 {
                let delta = Vec2::new(0.0, -120.0);
                let (unit, phase, source) = (egui::MouseWheelUnit::Point, egui::TouchPhase::Move, egui::MouseWheelSource::Wheel);
                events.push(egui::Event::MouseWheel { unit, delta, phase, source, modifiers });
            }
            let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
            let raw = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
            let mut output = ctx.run_ui(raw, |ui| {
                app.paint_scene(ui.painter(), screen);
                took |= app.note_wheel(&ctx);
            });
            output.textures_delta.clear();
            first = output.shapes.iter().find_map(|shape| match &shape.shape {
                Shape::Text(text) if text.galley.text().starts_with("Linha 1:") => Some(text.pos.y),
                _ => None,
            });
        }
        (took, first)
    };
    let body = app.to_screen(Pos2::new(250.0, 220.0));
    // the wheel away from the notes: the canvas's; the notes painted learned what their cards hide
    let (away, beside) = (app.to_screen(Pos2::new(700.0, 500.0)), app.to_screen(Pos2::new(600.0, 200.0)));
    let (took, top) = wheel(&mut app, away, Modifiers::NONE);
    assert!(!took && top.is_some());
    let offset = |app: &DrawingApp| app.note_scroll.borrow().get(&id).map_or(0.0, |scroll| scroll.offset);
    assert_eq!(offset(&app), 0.0);
    let (took, scrolled) = wheel(&mut app, body, Modifiers::NONE);
    assert!(took && offset(&app) > 0.0, "the wheel scrolled the text: {}", offset(&app));
    assert!(scrolled.unwrap() < top.unwrap() - 1.0, "its lines went up: {top:?} → {scrolled:?}");
    // never past its end
    for _ in 0..40 {
        wheel(&mut app, body, Modifiers::NONE);
    }
    let overflow = app.note_scroll.borrow()[&id].overflow;
    assert!(overflow > 0.0 && (offset(&app) - overflow).abs() < 0.01, "{} of {overflow}", offset(&app));
    // the header picks the note up, Ctrl + wheel zooms and a note whose text fits has nothing to scroll: the canvas's
    let header = app.to_screen(Pos2::new(250.0, 105.0));
    assert!(!wheel(&mut app, header, Modifiers::NONE).0);
    assert!(!wheel(&mut app, body, Modifiers::COMMAND).0);
    assert!(!wheel(&mut app, beside, Modifiers::NONE).0);
    assert!(!app.note_scroll.borrow().contains_key(&short_id));
}
