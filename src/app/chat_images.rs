//! The chat's pictures: thumbnails in the conversation (a click opens `chat_viewer`).

use crate::prelude::*;
use super::chat_viewer::Viewer;
use crate::app::chat::image_key;
use crate::transcript::ChatItem;
use std::sync::{Arc, OnceLock};

/// Thumbnails are made once, 480 px at most: twice the 240 pt they're shown at.
const THUMB: u32 = 480;
/// A thumbnail in the conversation and one in the box, without their borders.
const CARD: Vec2 = Vec2::new(240.0, 150.0);

/// A picture decoded off the UI thread: its texture (shrunk to fit), its real size and bytes.
pub(crate) struct Pic {
    pub(crate) texture: egui::TextureHandle,
    pub(crate) size: [u32; 2],
    pub(crate) bytes: usize,
}

/// Empty while a thread decodes; then the picture, or None when it can't be read.
pub(crate) type Decoding = Arc<OnceLock<Option<Pic>>>;

/// Decodes `bytes()` in a thread into a texture that fits `max` × `max` (and the GPU's limit);
/// repaints when done.
// ponytail: a thread per picture, a burst of them when a conversation with 20 opens; a small
// pool if that ever shows
pub(crate) fn decode(ctx: &Context, max: u32, bytes: impl FnOnce() -> Option<Vec<u8>> + Send + 'static) -> Decoding {
    let slot: Decoding = Default::default();
    let (out, ctx, max) = (slot.clone(), ctx.clone(), max.min(ctx.input(|i| i.max_texture_side) as u32));
    std::thread::spawn(move || {
        let pic = bytes().and_then(|bytes| {
            let image = image::load_from_memory(&bytes).ok()?;
            let size = [image.width(), image.height()];
            let image = if size[0] > max || size[1] > max { image.thumbnail(max, max) } else { image };
            let rgba = image.to_rgba8();
            let pixels = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba);
            Some(Pic { texture: ctx.load_texture("chat-image", pixels, egui::TextureOptions::LINEAR), size, bytes: bytes.len() })
        });
        let _ = out.set(pic);
        ctx.request_repaint();
    });
    slot
}

#[derive(Default)]
pub(crate) struct ChatPics {
    /// The picture open over the chat.
    pub(crate) viewer: Option<Viewer>,
    /// The last thumbnail drawn (its ui, the pass, where it left the cursor, its card, the
    /// person's): an image right after it goes beside it.
    last: Option<(egui::Id, u64, f32, Rect, bool)>,
}

impl DrawingApp {
    /// An image of the conversation: a thumbnail with its name, size and weight, beside the one
    /// right before it when there's room; a click opens it over the chat.
    pub(crate) fn chat_thumb(&self, ui: &mut egui::Ui, item: &ChatItem, c: &UiColors) {
        let ChatItem::Image { data, user, .. } = item else { return };
        let key = image_key(data);
        let pic = self.chat_images.borrow_mut().entry(key.clone()).or_insert_with(|| thumb(ui.ctx(), data)).clone();
        // unreadable: left out, as before
        if matches!(pic.get(), Some(None)) {
            return;
        }
        let pic = pic.get().and_then(Option::as_ref);
        let name = self.chat_image_names().into_iter().find(|(i, _)| std::ptr::eq(*i, item)).map(|(_, n)| n).unwrap_or_default();
        // the box with its border, the gap and two lines of 16
        let card = Vec2::new(CARD.x + 2.0, CARD.y + 2.0 + 6.0 + 32.0);
        let mut pics = self.chat_pics.borrow_mut();
        let (bounds, pass) = (ui.max_rect(), ui.ctx().cumulative_pass_nr());
        // ponytail: "right after" is read off the cursor (nothing drawn since the last card);
        // the conversation's loop could group the runs instead
        let beside = match pics.last {
            Some((id, at, y, prev, side)) if id == ui.scope_id() && at == pass && side == *user && (0.0..12.0).contains(&(ui.cursor().min.y - y)) => {
                let x = if *user { prev.left() - 12.0 - card.x } else { prev.right() + 12.0 };
                (x >= bounds.left() && x + card.x <= bounds.right()).then(|| Pos2::new(x, prev.top()))
            }
            _ => None,
        };
        let at = beside.unwrap_or(Pos2::new(if *user { bounds.right() - card.x } else { bounds.left() }, ui.cursor().min.y));
        let rect = Rect::from_min_size(at, card);
        let response = ui.allocate_rect(rect, Sense::click());
        pics.last = Some((ui.scope_id(), pass, ui.cursor().min.y, rect, *user));
        let dark = self.dark();
        let p = ui.painter();
        let frame = Rect::from_min_size(rect.min, CARD + Vec2::splat(2.0));
        picture(p, pic, frame.shrink(1.0), 9, c);
        p.rect_stroke(frame, 10, Stroke::new(1.0, if dark { Color32::from_rgb(0x4a, 0x4a, 0x50) } else { Color32::from_black_alpha(36) }), StrokeKind::Inside);
        // the expand mark, top right
        let mark = Rect::from_min_size(Pos2::new(frame.right() - 30.0, frame.top() + 6.0), Vec2::splat(24.0));
        p.rect_filled(mark, 6, Color32::from_rgb(0x12, 0x12, 0x12).gamma_multiply(0.85));
        paint_icon(p, Icon::Expand, mark.center(), Color32::from_rgb(0xec, 0xec, 0xf1));
        let under = if dark { Color32::from_rgb(0xb0, 0xb0, 0xb8) } else { c.muted };
        caption(p, name, frame.left_bottom() + Vec2::new(0.0, 6.0), 12.0, c.text, frame.width());
        caption(p, pic.map(details).unwrap_or_default(), frame.left_bottom() + Vec2::new(0.0, 22.0), 12.0, under, frame.width());
        if response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Clique para ampliar").clicked() {
            pics.viewer = Some(Viewer::open(ui.ctx(), key, data));
        }
    }

    /// The images of the open conversation, oldest first, with their names.
    pub(super) fn chat_image_names(&self) -> Vec<(&ChatItem, String)> {
        let items = self.chat_agent.as_ref().and_then(|id| self.transcripts.get(id)).map(|t| t.items.as_slice()).unwrap_or_default();
        image_names(items).into_iter().map(|(k, name)| (&items[k], name)).collect()
    }
}

fn thumb(ctx: &Context, data: &str) -> Decoding {
    let data = data.to_string();
    decode(ctx, THUMB, move || crate::render::base64_decode(&data))
}

/// Each image of a conversation (its index) and a name: the file a tool read it from (the tools
/// right before a run of images, in their order), else "Imagem N".
pub(crate) fn image_names(items: &[ChatItem]) -> Vec<(usize, String)> {
    let (mut out, mut files, mut run) = (Vec::new(), Vec::new(), 0);
    for (k, item) in items.iter().enumerate() {
        match item {
            ChatItem::Image { .. } => {
                out.push((k, files.get(run).cloned().unwrap_or_else(|| format!("Imagem {}", out.len() + 1))));
                run += 1;
            }
            ChatItem::Tool(_, what) => {
                if run > 0 {
                    (files, run) = (Vec::new(), 0);
                }
                let name = what.rsplit(['/', '\\']).next().unwrap_or_default();
                if name.rsplit_once('.').is_some_and(|(_, ext)| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str())) {
                    files.push(name.to_string());
                }
            }
            _ => (files, run) = (Vec::new(), 0),
        }
    }
    out
}

/// "84 KB", "1,5 MB": a file's weight as the design writes it.
pub(crate) fn size_label(bytes: u64) -> String {
    let kb = (bytes as f64 / 1024.0).round().max(1.0);
    if kb < 1024.0 { format!("{} KB", kb as u64) } else { format!("{:.1} MB", kb / 1024.0).replace('.', ",") }
}

/// "1280 × 800 · 84 KB"
pub(super) fn details(pic: &Pic) -> String {
    format!("{} × {} · {}", pic.size[0], pic.size[1], size_label(pic.bytes as u64))
}

/// The part of a picture that covers a box of `size`: centered across, from the top down (a tall
/// print shows its top).
fn cover(image: Vec2, size: Vec2) -> Rect {
    let part = size / (size.x / image.x).max(size.y / image.y) / image;
    Rect::from_min_size(Pos2::new((1.0 - part.x) / 2.0, 0.0), part)
}

/// A picture covering `rect` with rounded corners; a plain box while it's read.
pub(super) fn picture(p: &Painter, pic: Option<&Pic>, rect: Rect, radius: u8, c: &UiColors) {
    match pic {
        Some(pic) => {
            p.add(egui::epaint::RectShape::filled(rect, radius, Color32::WHITE).with_texture(pic.texture.id(), cover(pic.texture.size_vec2(), rect.size())));
        }
        None => {
            p.rect_filled(rect, radius, c.surface_low);
        }
    }
}

/// One line of text at `at`, cut with "…" to fit `width`.
fn caption(p: &Painter, text: String, at: Pos2, size: f32, color: Color32, width: f32) {
    let galley = clipped(p, text, size, color, width);
    p.galley(at, galley, color);
}

pub(super) fn clipped(p: &Painter, text: String, size: f32, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text, FontId::proportional(size), color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(10.0));
    p.layout_job(job)
}

pub(super) fn cross(p: &Painter, at: Pos2, r: f32, stroke: Stroke) {
    p.line_segment([at + Vec2::new(-r, -r), at + Vec2::new(r, r)], stroke);
    p.line_segment([at + Vec2::new(r, -r), at + Vec2::new(-r, r)], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut png = Vec::new();
        image::RgbaImage::new(w, h).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        png
    }

    /// Waits for a thread's picture.
    fn ready(d: &Decoding) -> Option<&Pic> {
        for _ in 0..500 {
            if let Some(pic) = d.get() {
                return pic.as_ref();
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("never decoded")
    }

    fn image_item(bytes: &[u8], user: bool) -> ChatItem {
        ChatItem::Image { data: crate::render::base64(bytes), media: "image/png".into(), user, at: 0 }
    }

    /// One frame of the open chat at 1000 × 800.
    fn chat_frame(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))), events, ..Default::default() };
        ctx.run_ui(raw, |_| app.chat_screen(ctx)).textures_delta.clear();
    }

    fn click(app: &mut DrawingApp, ctx: &Context, at: Pos2) {
        chat_frame(app, ctx, vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        chat_frame(app, ctx, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
    }

    fn key(key: Key) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }
    }

    #[test]
    fn a_thumbnail_is_made_once_at_480_px_with_the_real_size() {
        let ctx = Context::default();
        let bytes = png(2000, 1000);
        let pic = decode(&ctx, THUMB, { let b = bytes.clone(); move || Some(b) });
        let pic = ready(&pic).unwrap();
        assert_eq!((pic.texture.size(), pic.size, pic.bytes), ([480, 240], [2000, 1000], bytes.len()));
        assert!(ready(&decode(&ctx, THUMB, || Some(b"not an image".to_vec()))).is_none());
        assert_eq!((size_label(84 * 1024), size_label(10), size_label(1536 * 1024)), ("84 KB".into(), "1 KB".into(), "1,5 MB".into()));
        // a tall print shows its top, a wide one its middle
        assert_eq!(cover(Vec2::new(100.0, 400.0), Vec2::new(100.0, 100.0)), Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 0.25)));
        assert_eq!(cover(Vec2::new(400.0, 100.0), Vec2::new(100.0, 100.0)), Rect::from_min_size(Pos2::new(0.375, 0.0), Vec2::new(0.25, 1.0)));
    }

    #[test]
    fn images_are_named_by_the_file_a_tool_read() {
        let image = || ChatItem::Image { data: "x".into(), media: "image/png".into(), user: false, at: 0 };
        let items = vec![
            ChatItem::Tool("Read".into(), "C:\\proj\\login-bloqueado.png".into()),
            ChatItem::Tool("Read".into(), "/proj/fluxo.PNG".into()),
            image(),
            image(),
            ChatItem::Agent("e esta".into()),
            image(),
        ];
        assert_eq!(image_names(&items), vec![(2, "login-bloqueado.png".into()), (3, "fluxo.PNG".into()), (5, "Imagem 3".into())]);
    }

    #[test]
    fn a_thumbnail_opens_the_viewer_which_walks_and_closes() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        let mut agent = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
        agent.terminal = Some(TerminalSpec { name: "Revisor".into(), command: "claude".into(), ..Default::default() });
        let id = agent.id.clone();
        app.scene.elements.push(agent);
        app.chat_open = true;
        let mut transcript = crate::transcript::Transcript::new("nada.jsonl".into());
        transcript.items = vec![ChatItem::Agent("Olha:".into()), image_item(&png(1280, 800), false), image_item(&png(960, 600), false)];
        app.transcripts.insert(id, transcript);
        chat_frame(&mut app, &ctx, vec![]);
        for d in app.chat_images.borrow().values() {
            ready(d);
        }
        chat_frame(&mut app, &ctx, vec![]);
        // the second sits beside the first, on its row
        let (_, _, _, second, _) = app.chat_pics.borrow().last.unwrap();
        assert!(second.left() > 240.0, "{second:?}");
        click(&mut app, &ctx, second.center());
        let open = |app: &DrawingApp| app.chat_pics.borrow().viewer.as_ref().map(|v| v.key.0);
        let (first, last) = match &app.transcripts.values().next().unwrap().items[1..] {
            [ChatItem::Image { data: a, .. }, ChatItem::Image { data: b, .. }] => (a.len(), b.len()),
            _ => unreachable!(),
        };
        assert_eq!(open(&app), Some(last), "the clicked one opens");
        chat_frame(&mut app, &ctx, vec![key(Key::ArrowRight)]);
        assert_eq!(open(&app), Some(last), "no next after the last");
        chat_frame(&mut app, &ctx, vec![key(Key::ArrowLeft)]);
        assert_eq!(open(&app), Some(first));
        // the full picture, read in a thread
        ready(&app.chat_pics.borrow().viewer.as_ref().unwrap().full);
        chat_frame(&mut app, &ctx, vec![]);
        assert_eq!(app.chat_pics.borrow().viewer.as_ref().unwrap().zoom, None, "fitted");
        chat_frame(&mut app, &ctx, vec![key(Key::Escape)]);
        assert!(app.chat_pics.borrow().viewer.is_none(), "Esc closes it");
    }

}
