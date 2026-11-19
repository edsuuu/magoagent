//! The chat's pictures (chat/images.rs): no caption under a thumbnail, in the box or in the conversation, its name and
//! size in the tip instead; a click on one in the box opens it in the viewer the conversation's use (chat/viewer.rs).

use super::*;
use crate::app::chat::viewer::PictureSource;
use crate::transcript::ChatItem;

const SIZE: Vec2 = Vec2::new(1000.0, 800.0);

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();

    image::RgbaImage::new(width, height).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    png
}

/// One frame of the chat screen: the texts painted and where, and where each texture went.
fn frame(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) -> (Vec<(String, Rect)>, Vec<(egui::TextureId, Rect)>) {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.chat_screen(ctx));

    output.textures_delta.clear();

    let texts = output
        .shapes
        .iter()
        .filter_map(|s| if let Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.visual_bounding_rect())) } else { None })
        .collect();

    let pictures = output.shapes.iter().map(|s| (s.shape.texture_id(), s.shape.visual_bounding_rect())).collect();

    (texts, pictures)
}

fn tap(app: &mut DrawingApp, ctx: &Context, at: Pos2) {
    frame(app, ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    frame(app, ctx, vec![button(at, false)]);
}

/// Frames until every picture of the box and of the conversation is read.
fn read_all(app: &mut DrawingApp, ctx: &Context) {
    for _ in 0..500 {
        frame(app, ctx, vec![]);
        let box_read = app.chat_pics.borrow().attached.iter().all(|picture| picture.pic.get().is_some());
        let said_read = app.chat_images.borrow().values().all(|picture| picture.get().is_some());

        if box_read && said_read && !app.chat_images.borrow().is_empty() {
            return;
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    panic!("never read");
}

/// Revisor's conversation open with a picture it sent, and a picture pasted in the box (its file).
fn picture_in_both(name: &str) -> (DrawingApp, Context, PathBuf) {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut agent = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });

    agent.terminal = Some(TerminalSpec { name: "Revisor".into(), command: "claude".into(), ..Default::default() });
    let id = agent.id.clone();

    app.scene.elements.push(agent);
    app.chat_open = true;
    let mut transcript = crate::transcript::Transcript::new("nada.jsonl".into());

    transcript.items = vec![ChatItem::Image { data: crate::render::base64(&png(64, 32)), media: "image/png".into(), user: false, at: 0 }];
    app.transcripts.insert(id.clone(), transcript);
    app.chat_agent = Some(id);
    let dir = std::env::temp_dir().join(format!("mago-{name}-{}", std::process::id()));

    fs::create_dir_all(&dir).unwrap();
    let pasted = dir.join("colada.png");

    fs::write(&pasted, png(361, 890)).unwrap();
    app.chat_add_files(&ctx, vec![pasted.clone()], true);
    read_all(&mut app, &ctx);
    (app, ctx, pasted)
}

/// Where the box's picture is drawn.
fn box_picture(app: &DrawingApp, pictures: &[(egui::TextureId, Rect)]) -> Rect {
    let texture = app.chat_pics.borrow().attached[0].pic.get().unwrap().as_ref().unwrap().texture.id();

    pictures.iter().find(|(id, _)| *id == texture).map(|(_, rect)| *rect).expect("the box's thumbnail")
}

#[test]
fn no_caption_under_the_pictures_and_none_of_its_room() {
    let (mut app, ctx, pasted) = picture_in_both("legenda");
    let (texts, pictures) = frame(&mut app, &ctx, vec![]);

    for gone in ["Colado · Ctrl+V", "Imagem colada (Ctrl+V)", "Imagem 1", "361 × 890", "64 × 32"] {
        assert!(!texts.iter().any(|(text, _)| text.contains(gone)), "{gone} under a picture: {texts:?}");
    }

    // what goes with the message still says itself at the box's right
    assert!(texts.iter().any(|(text, _)| text.starts_with("1 imagem · ")), "{texts:?}");
    // the box's words right under the thumbnail: no room left for the two lines it had
    let thumbnail = box_picture(&app, &pictures);
    let words = texts.iter().find(|(text, _)| text == "Escreva uma mensagem…").map(|(_, rect)| *rect).expect("the box's hint");

    assert!((0.0..16.0).contains(&(words.top() - thumbnail.bottom())), "{thumbnail:?} {words:?}");
    let _ = fs::remove_dir_all(pasted.parent().unwrap());
}

#[test]
fn a_click_on_a_picture_in_the_box_opens_it_in_the_viewer_and_the_x_only_takes_it_out() {
    let (mut app, ctx, pasted) = picture_in_both("ampliar");
    let (_, pictures) = frame(&mut app, &ctx, vec![]);
    let thumbnail = box_picture(&app, &pictures);

    tap(&mut app, &ctx, thumbnail.center());
    let source = app.chat_pics.borrow().viewer.as_ref().map(|viewer| viewer.source.clone());

    assert_eq!(source, Some(PictureSource::Attached(pasted.clone())), "the conversation's viewer, on the box's picture");

    // the whole picture, read off its file
    for _ in 0..500 {
        if app.chat_pics.borrow().viewer.as_ref().is_some_and(|viewer| viewer.full.get().is_some()) {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // a new area shows from its second frame
    frame(&mut app, &ctx, vec![]);
    let (texts, _) = frame(&mut app, &ctx, vec![]);
    let says = |words: &str| texts.iter().any(|(text, _)| text == words);

    assert!(says("Imagem colada (Ctrl+V)"), "its name in the header: {texts:?}");
    assert!(texts.iter().any(|(text, _)| text.starts_with("361 × 890 · ") && text.ends_with("colada · vai com a próxima mensagem")), "{texts:?}");
    assert!(says("1 de 1") && !says("Copiar"), "the person's own file: no Copiar: {texts:?}");
    frame(&mut app, &ctx, vec![key_press(Key::Escape)]);
    assert!(app.chat_pics.borrow().viewer.is_none(), "Esc closes it");
    assert_eq!(app.chat_pics.borrow().attached.len(), 1, "and the picture still goes with the message");
    // the × takes it out, and opens nothing
    let (_, pictures) = frame(&mut app, &ctx, vec![]);
    let thumbnail = box_picture(&app, &pictures);

    tap(&mut app, &ctx, thumbnail.right_top() + Vec2::new(-3.0, 3.0));
    assert!(app.chat_pics.borrow().attached.is_empty() && app.chat_pics.borrow().viewer.is_none());
    let _ = fs::remove_dir_all(pasted.parent().unwrap());
}
