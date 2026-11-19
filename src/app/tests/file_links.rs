//! Links to files (filetree/link.rs): a path in a chat's words in the code colour, a click opening it in a tree beside
//! the chat with "Voltar" back to it, and a drawing's `link` mark doing the same from the canvas.

use super::*;
use crate::app::chat::direct::NewChat;
use crate::app::filetree::link::{FileLink, file_link_at, path_like, split_line, trimmed};
use crate::transcript::{ChatItem, Transcript};

/// A project with a git folder and `src/main.rs` (ten lines) in a fresh temp folder.
fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("magoagent-links-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    fs::create_dir_all(dir.join(".git")).unwrap();
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/main.rs"), (1..=10).map(|line| format!("// linha {line}\n")).collect::<String>()).unwrap();
    dir
}

/// One frame of the chat screen: each text painted, with its galley and where it went.
fn chat_texts(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) -> Vec<egui::epaint::TextShape> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))), time: Some(time), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.chat_screen(ctx));

    output.textures_delta.clear();
    let mut shapes: Vec<Shape> = output.shapes.into_iter().map(|clipped| clipped.shape).collect();
    let mut texts = Vec::new();

    while let Some(shape) = shapes.pop() {
        match shape {
            Shape::Vec(inner) => shapes.extend(inner),
            Shape::Text(text) => texts.push(text),
            _ => {}
        }
    }

    texts
}

#[test]
fn a_word_names_a_file_with_its_line_and_a_sentence_keeps_its_marks() {
    assert_eq!(split_line("src/a.rs:120"), ("src/a.rs", Some(120)));
    assert_eq!(split_line("src/a.rs:120:4"), ("src/a.rs", Some(120)));
    assert_eq!(split_line("src/a.rs:12-30"), ("src/a.rs", Some(12)));
    assert_eq!(split_line("src/a.rs#L7"), ("src/a.rs", Some(7)));
    assert_eq!(split_line("C:\\p\\a.rs:9"), ("C:\\p\\a.rs", Some(9)));
    assert_eq!(split_line("src/a.rs"), ("src/a.rs", None));

    for word in ["src/a.rs", "a.rs:3", ".env", "/abs/Cargo.toml"] {
        assert!(path_like(word), "{word}");
    }

    for word in ["https://x.com/a.rs", "fim", "1.5", "e.g.", "a b.rs", "*.rs", "src/"] {
        assert!(!path_like(word), "{word}");
    }

    assert_eq!(trimmed("(src/a.rs:12)."), (1, "src/a.rs:12"));
    assert_eq!(trimmed("\"x.rs\","), (1, "x.rs"));
    // only a file that is there: from the chat's folder when relative
    let dir = project("words");
    let link = file_link_at("src/main.rs:3", Some(&dir), 1.0).unwrap();

    assert_eq!(link, FileLink { path: dir.join("src/main.rs"), line: Some(3) });
    assert!(file_link_at("src/nada.rs:3", Some(&dir), 1.0).is_none());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_path_in_the_chat_is_a_link_in_the_code_colour_to_the_file_in_a_tree_beside_it_and_voltar_comes_back() {
    let ctx = Context::default();
    let dir = project("chat");
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    app.create_chat(&NewChat { agent: 0, cwd: dir.to_string_lossy().into_owned(), mode: 0 }, &[Preset::new("Claude Code", "claude")]);
    let id = app.chat_agent.clone().unwrap();
    let mut transcript = Transcript::new(PathBuf::new());

    transcript.items.push(ChatItem::Agent("Mudei src/main.rs:3 e o `Cargo.toml` não existe.".into()));
    app.transcripts.insert(id.clone(), transcript);
    app.chat_open = true;
    let (pan, zoom) = (app.pan, app.zoom);
    let mut texts = Vec::new();

    for time in [1.0, 1.1, 1.2] {
        texts = chat_texts(&mut app, &ctx, time, vec![]);
    }

    let words = texts.iter().find(|text| text.galley.text().starts_with("Mudei")).unwrap_or_else(|| panic!("no answer painted"));
    // the path and the code in the code colour, on no grey ground; the rest in the words' colour
    let code = UiColors::new(app.dark()).code;

    let colors: Vec<(String, Color32, Color32)> = words
        .galley
        .job
        .sections
        .iter()
        .map(|section| (words.galley.text()[section.byte_range.start.0..section.byte_range.end.0].to_string(), section.format.color, section.format.background))
        .collect();

    assert!(colors.iter().any(|(text, color, _)| text == "src/main.rs:3" && *color == code), "{colors:?}");
    assert!(colors.iter().any(|(text, color, _)| text == "Cargo.toml" && *color == code), "{colors:?}");
    assert!(colors.iter().all(|(.., ground)| *ground == Color32::TRANSPARENT), "no chip behind code: {colors:?}");
    assert!(colors.iter().any(|(text, color, _)| text.starts_with("Mudei") && *color != code), "{colors:?}");
    // a click on the path: the chat closes, the file opens in a new tree on its project beside the chat's card, at its line
    let start = words.galley.text().find("src/main").unwrap();
    let letter = words.galley.text()[..start + 4].chars().count();
    let at = words.pos + words.galley.pos_from_cursor(egui::text::CCursor::new(letter)).center().to_vec2();

    chat_texts(&mut app, &ctx, 2.0, vec![egui::Event::PointerMoved(at)]);
    chat_texts(&mut app, &ctx, 2.1, vec![button(at, true)]);
    chat_texts(&mut app, &ctx, 2.15, vec![button(at, false)]);
    app.chat_nodes_tick(&ctx);
    let tree = app.scene.elements.iter().position(|element| element.filetree.is_some()).expect("a tree for the file");
    let tree_id = app.scene.elements[tree].id.clone();

    assert_eq!(app.scene.elements[tree].filetree.as_ref().unwrap().root_path(), dir);
    let card = element_bounds(&app.scene.elements[app.chat_node(&id).unwrap()]);
    let tree_rect = element_bounds(&app.scene.elements[tree]);

    assert!(tree_rect.left() >= card.right() && !tree_rect.intersects(card), "beside the chat: {tree_rect:?} {card:?}");
    assert!(!app.chat_open && app.card_focus.as_deref() == Some(tree_id.as_str()), "the tree in view, with the keys");
    assert_eq!(app.filetrees.editors.get(&tree_id).and_then(|editor| editor.active.clone()), Some(dir.join("src/main.rs")));
    assert_eq!(app.filetrees.views[&tree_id].selected, Some(dir.join("src/main.rs")), "picked in the tree");
    assert!(app.to_screen(tree_rect.center()).distance(app.canvas_rect.center()) < 2.0, "the camera on the tree");
    // a second link while there opens the same tree, and Voltar still goes back to the chat
    app.open_file_link(&FileLink { path: dir.join("src/main.rs"), line: Some(9) }, Some(&id)).unwrap();
    assert_eq!(app.scene.elements.iter().filter(|element| element.filetree.is_some()).count(), 1);

    // "Voltar ao chat" over the canvas: back to the chat on its conversation, the camera where it was
    let back = |app: &mut DrawingApp, time: f64, events: Vec<egui::Event>| {
        let raw =
            egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), time: Some(time), events, ..Default::default() };

        let output = ctx.run_ui(raw, |_| app.link_back_ui(&ctx));

        output.shapes.into_iter().find_map(|clipped| match clipped.shape {
            Shape::Text(text) if text.galley.text() == "Voltar ao chat" => Some(text.visual_bounding_rect().center()),
            _ => None,
        })
    };

    // (a new area is measured, unseen, in its first frame)
    back(&mut app, 2.9, vec![]);
    let button_at = back(&mut app, 3.0, vec![]).expect("the Voltar button");

    back(&mut app, 3.1, vec![egui::Event::PointerMoved(button_at), button(button_at, true)]);
    back(&mut app, 3.15, vec![button(button_at, false)]);
    assert!(app.chat_open && app.chat_agent.as_deref() == Some(id.as_str()), "back on the chat");
    assert_eq!((app.pan, app.zoom), (pan, zoom));
    assert!(app.filetrees.link_back.is_none() && back(&mut app, 3.3, vec![]).is_none(), "the button went");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_drawings_link_rides_the_file_and_its_mark_opens_the_file_and_voltar_brings_the_camera_back() {
    let ctx = Context::default();
    let dir = project("drawing");
    let mut app = DrawingApp::default();

    frame(&mut app, &ctx, vec![]);
    let mut shape = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 160.0, height: 80.0 });

    shape.link = Some(format!("{}:4", dir.join("src/main.rs").display()));
    let id = shape.id.clone();

    app.scene.elements.push(shape);
    // in the drawing's file, in the format other whiteboards read, and through a merge with an outside change
    let scene: Scene = serde_json::from_str(&serde_json::to_string(&app.scene).unwrap()).unwrap();

    assert_eq!(scene.elements[0].link, app.scene.elements[0].link);
    let exchanged = from_interop_elements(&Value::Array(interop_elements(&app.scene.elements)));

    assert_eq!(exchanged[0].link, app.scene.elements[0].link);
    let mut outside = app.scene.clone();

    outside.elements[0].link = Some("/outro/a.rs".into());
    assert_eq!(merge_scenes(&app.scene, &app.scene, &outside).elements[0].link.as_deref(), Some("/outro/a.rs"));
    let (pan, zoom) = (app.pan, app.zoom);
    // its mark at the top right corner: a click opens the file in a tree, at its line
    let corner = app.to_screen(element_bounds(&app.scene.elements[0]).right_top());
    let mark = corner + Vec2::new(12.0, -12.0);

    let marks = |app: &mut DrawingApp, time: f64, events: Vec<egui::Event>| {
        let raw =
            egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), time: Some(time), events, ..Default::default() };

        let _ = ctx.run_ui(raw, |_| {
            app.element_links_ui(&ctx);
            app.link_back_ui(&ctx);
        });
    };

    marks(&mut app, 0.9, vec![]);
    marks(&mut app, 1.0, vec![egui::Event::PointerMoved(mark)]);
    marks(&mut app, 1.1, vec![button(mark, true)]);
    marks(&mut app, 1.15, vec![button(mark, false)]);
    let tree = app.scene.elements.iter().find(|element| element.filetree.is_some()).map(|element| element.id.clone()).expect("a tree");

    assert_eq!(app.filetrees.editors.get(&tree).and_then(|editor| editor.active.clone()), Some(dir.join("src/main.rs")));
    let back = app.filetrees.link_back.clone().unwrap();

    assert!(!back.chat_open && back.tree == tree, "{back:?}");
    assert!(element_bounds(app.scene.elements.iter().find(|element| element.id == tree).unwrap()).left() > 260.0, "beside the drawing");
    // Voltar, as the MCP's file_link back does it: the camera where it was
    assert_eq!(app.act("file_link", &json!({ "action": "back" })).unwrap(), "De volta ao canvas onde estava.");
    assert_eq!((app.pan, app.zoom), (pan, zoom));
    assert!(app.act("file_link", &json!({ "action": "back" })).is_err(), "nothing more to go back to");

    // the MCP's open, a relative path from the asker's folder, its line after it
    let chat = {
        app.create_chat(&NewChat { agent: 0, cwd: dir.to_string_lossy().into_owned(), mode: 0 }, &[Preset::new("Claude Code", "claude")]);
        app.chat_agent.clone().unwrap()
    };

    let said = app.act("file_link", &json!({ "action": "open", "path": "src/main.rs:7", "asker": chat })).unwrap();

    assert!(said.contains("na linha 7"), "{said}");
    assert!(app.act("file_link", &json!({ "action": "open", "path": "src/nada.rs", "asker": chat })).is_err());
    let _ = id;
    let _ = fs::remove_dir_all(&dir);
}
