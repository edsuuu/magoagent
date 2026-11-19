//! What the agent's words carry besides words: pictures (`![alt](path)`, a data address or a file
//! it wrote) drawn in the conversation and files as cards; a click shows a picture big in the app (island/picture.rs),
//! opens a text file (.md, .txt) on the canvas in a note (notes/file.rs) and any other file with the system.

use crate::app::browser::open_external;
use crate::app::chat::direct::folder;
use crate::app::chat::images::{Pic, decode, picture};
use crate::app::chat::list::Look;
use crate::app::chat::markdown::{code_words, prose_with};
use crate::app::chat::segments::{Seg, data_uri, file_size, is_local, is_picture, middle, resolve};
use crate::app::chat::{image_key, tint};
use crate::app::filetree::link::{FileLink, ask_open, opens_in_tree, with_link_base};
use crate::app::island::picture::Shown;
use crate::app::ui::Tip;
use crate::prelude::*;
use crate::transcript::Step;
use std::path::Path;

/// The file card: 360 wide at most, a 32 pt page icon, the name over its kind and weight.
const CARD_MAX: f32 = 360.0;
const ICON: f32 = 32.0;
/// A picture in the conversation: its widest and tallest.
const PICTURE_MAX: Vec2 = Vec2::new(420.0, 320.0);

/// A text file's card clicked asks for its note through the context (the card is drawn by `&self`): the file and the
/// chat it was in.
const NOTE_ASK: &str = "chat_file_note_ask";

/// The text file a card asked to open in a note this frame, with its chat (`chat_nodes_tick` opens it).
pub(crate) fn take_note_ask(ctx: &Context) -> Option<(PathBuf, Option<String>)> {
    ctx.data_mut(|d| d.remove_temp::<(PathBuf, Option<String>)>(egui::Id::unique(NOTE_ASK)))
}

impl DrawingApp {
    /// The agent's words: markdown in bubbles, with its pictures and files drawn in between.
    pub(crate) fn agent_words(&self, ui: &mut egui::Ui, text: &str, width: f32, c: &UiColors) {
        self.agent_words_in(ui, text, width, c, true);
    }

    /// `agent_words` in the look of `ui` (the island's is always dark), each paragraph in a bubble when `bubbles`.
    /// The files its words name are links to them in a tree on the canvas (filetree/link.rs).
    pub(crate) fn agent_words_in(&self, ui: &mut egui::Ui, text: &str, width: f32, c: &UiColors, bubbles: bool) {
        let dark = ui.visuals().dark_mode;
        let ctx = ui.ctx().clone();
        with_link_base(&ctx, self.chat_folder(), self.chat_agent.clone(), || {
            prose_with(ui, text, c, dark, width, bubbles, &mut |ui, seg| match seg {
                Seg::Image { src, .. } => self.chat_picture(ui, src, c),
                Seg::File { label, path: written, linked } => {
                    let path = resolve(written, self.chat_folder().as_deref());
                    // a path in backticks that isn't a file is just a word in the code colour; with its line
                    // (`src/x.rs:12`), a link to it there
                    if *linked || path.exists() {
                        self.chat_file_card(ui, &path, label, c);
                    } else {
                        code_words(ui, written, c);
                    }
                }
                _ => {}
            })
        });
    }

    /// The folder of the open chat: where a relative path in its answers starts.
    fn chat_folder(&self) -> Option<PathBuf> {
        let id = self.chat_agent.as_ref()?;
        folder(&self.chats.iter().find(|c| &c.id == id)?.cwd)
    }

    /// A picture from an address or a path: its image, up to `PICTURE_MAX`; a card when it can't be
    /// shown (an address on the web, a file that isn't there or isn't a picture).
    pub(crate) fn chat_picture(&self, ui: &mut egui::Ui, src: &str, c: &UiColors) {
        let look = Look::new(ui.visuals().dark_mode, c);
        if !is_local(src) && data_uri(src).is_none() {
            // ponytail: no network loader in the app, so a picture on the web is a link to it
            let host = src.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or(src);
            let r = self.card(ui, src, &format!("Imagem de {host}"), "Clique para abrir no navegador", false, c);
            if r.clicked() {
                open_external(src);
            }
            return;
        }
        let (key, path) = match data_uri(src) {
            Some(data) => (image_key(data), None),
            None => {
                let path = resolve(src, self.chat_folder().as_deref());
                ((0, path.to_string_lossy().into_owned()), Some(path))
            }
        };
        if let Some(path) = path.as_ref().filter(|p| !p.is_file() || !is_picture(&p.to_string_lossy())) {
            self.chat_file_card(ui, path, "", c);
            return;
        }
        // ponytail: read once; a file the agent rewrites keeps showing its first version until the
        // chat is reopened
        let pic = self
            .chat_images
            .borrow_mut()
            .entry(key.clone())
            .or_insert_with(|| match (&path, data_uri(src)) {
                (Some(path), _) => {
                    let path = path.clone();
                    decode(ui.ctx(), 1280, move || fs::read(path).ok())
                }
                (None, Some(data)) => {
                    let data = data.to_string();
                    decode(ui.ctx(), 1280, move || crate::render::base64_decode(&data))
                }
                _ => Default::default(),
            })
            .clone();
        let ready: Option<&Pic> = match pic.get() {
            Some(Some(pic)) => Some(pic),
            Some(None) => {
                // unreadable: its card
                match &path {
                    Some(path) => {
                        self.chat_file_card(ui, path, "", c);
                    }
                    None => {
                        ui.label(egui::RichText::new("Imagem ilegível").size(13.0).color(look.muted));
                    }
                }
                return;
            }
            None => None,
        };
        let size = match ready {
            Some(pic) => {
                let real = Vec2::new(pic.size[0] as f32, pic.size[1] as f32);
                let room = Vec2::new(PICTURE_MAX.x.min(ui.available_width()), PICTURE_MAX.y);
                real * (room.x / real.x).min(room.y / real.y).min(1.0)
            }
            None => Vec2::new(240.0, 150.0),
        };
        let (rect, response) = ui.allocate_exact_size(size.max(Vec2::splat(24.0)), Sense::click());
        picture(ui.painter(), ready, rect, 10, c);
        let edge = if look.dark { rgb_hex(crate::app::chat::list::ink::DARK_EDGE) } else { Color32::from_black_alpha(36) };
        ui.painter().rect_stroke(rect, 10, Stroke::new(1.0, edge), StrokeKind::Inside);
        let response = response.on_hover_cursor(CursorIcon::PointingHand).tip("Clique para ampliar");
        if response.clicked() {
            let (path, data) = (path.clone(), data_uri(src).map(str::to_string));
            let shown = Shown::new(ui.ctx(), key, move || match (path, data) {
                (Some(path), _) => fs::read(path).ok(),
                (None, Some(data)) => crate::render::base64_decode(&data),
                _ => None,
            });
            self.chat_pics.borrow_mut().zoom = Some(shown);
        }
    }

    /// A file named by the agent: a card with its name, kind and weight; a click opens it, a right click copies its
    /// path, shows its folder or opens it in the system's program. `label` empty: the file's own name.
    pub(crate) fn chat_file_card(&self, ui: &mut egui::Ui, path: &Path, label: &str, c: &UiColors) {
        let meta = fs::metadata(path).ok();
        let name =
            if label.is_empty() { path.file_name().map_or_else(|| path.to_string_lossy(), |n| n.to_string_lossy()).into_owned() } else { label.to_string() };
        let detail = match &meta {
            Some(m) if m.is_dir() => "Pasta".to_string(),
            Some(m) => {
                let ext = path.extension().map(|e| e.to_string_lossy().to_uppercase());
                [ext, Some(file_size(m.len()))].into_iter().flatten().collect::<Vec<_>>().join(" · ")
            }
            None => "Arquivo não encontrado".to_string(),
        };
        let response = self.card(ui, &path.to_string_lossy(), &name, &detail, meta.is_none(), c);
        let text = path.to_string_lossy().into_owned();
        let as_note = meta.as_ref().is_some_and(|m| m.is_file()) && crate::app::notes::file::opens_as_note(path);
        // a page, a picture or a PDF opens in the system's program; code and other text in a tree on the canvas
        let in_tree = opens_in_tree(path);
        if response.clicked() && meta.is_some() {
            if as_note {
                let ask = (path.to_path_buf(), self.chat_agent.clone());
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique(NOTE_ASK), ask));
            } else if in_tree {
                ask_open(ui.ctx(), FileLink { path: path.to_path_buf(), line: None }, self.chat_agent.clone());
            } else {
                open_external(&text);
            }
        }
        crate::app::ui::menu_look::context_menu(&response, |ui| {
            if (as_note || in_tree) && crate::app::ui::menu_look::menu_row(ui, Some(Icon::Open), "Abrir no programa padrão", "", None) {
                open_external(&text);
                ui.close();
            }
            if crate::app::ui::menu_look::menu_row(ui, Some(Icon::Duplicate), "Copiar caminho", "", None) {
                ui.ctx().copy_text(text.clone());
                ui.close();
            }
            if meta.is_some() && crate::app::ui::menu_look::menu_row(ui, Some(Icon::Open), "Mostrar na pasta", "", None) {
                open_external(&path.parent().unwrap_or(path).to_string_lossy());
                ui.close();
            }
        });
    }

    /// The card itself: radius 16, a 4.5% wash with an 8% hairline, 10 pt around a 32 pt icon,
    /// the name (13, middle-cut keeping its end) over the detail (11); dimmed when `dim`.
    fn card(&self, ui: &mut egui::Ui, id: &str, name: &str, detail: &str, dim: bool, c: &UiColors) -> egui::Response {
        let dark = ui.visuals().dark_mode;
        let look = Look::new(dark, c);
        let fade = if dim { 0.6 } else { 1.0 };
        let (ink_name, ink_detail) = (look.text.gamma_multiply(fade), look.muted.gamma_multiply(fade));
        let (name_font, detail_font) = (FontId::proportional(14.0), FontId::proportional(12.0));
        let room = ui.available_width().min(CARD_MAX) - (10.0 + ICON + 10.0 + 14.0);
        let p = ui.painter();
        let name = middle(name, |s| p.layout_no_wrap(s.to_string(), name_font.clone(), ink_name).size().x <= room);
        let name = p.layout_no_wrap(name, name_font, ink_name);
        let detail = clipped_line(p, detail, detail_font, ink_detail, room);
        let size = Vec2::new(10.0 + ICON + 10.0 + name.size().x.max(detail.size().x) + 14.0, 48.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let response = response.on_hover_cursor(CursorIcon::PointingHand);
        let p = ui.painter();
        let wash = if response.hovered() { 22 } else { 12 };
        p.rect_filled(rect, 16, tint(dark, wash).gamma_multiply(fade));
        p.rect_stroke(rect, 16, Stroke::new(1.0, tint(dark, 20).gamma_multiply(fade)), StrokeKind::Inside);
        page_icon(p, Rect::from_center_size(Pos2::new(rect.left() + 10.0 + ICON / 2.0, rect.center().y), Vec2::splat(ICON)), ink_detail);
        let x = rect.left() + 10.0 + ICON + 10.0;
        let top = rect.center().y - (name.size().y + 1.0 + detail.size().y) / 2.0;
        let below = top + name.size().y + 1.0;
        p.galley(Pos2::new(x, top), name, ink_name);
        p.galley(Pos2::new(x, below), detail, ink_detail);
        // the id keeps two cards of the same name apart for the pointer
        response.tip(id.to_string())
    }

    /// The files a run of steps wrote (Write): a picture shows as one, anything else as a card.
    pub(crate) fn step_files(&self, ui: &mut egui::Ui, steps: &[&Step], c: &UiColors) {
        for step in steps.iter().filter(|s| s.name == "Write" && s.ended.is_some() && !s.failed) {
            let Some(file) = step.files.first() else { continue };
            let path = resolve(file, self.chat_folder().as_deref());
            if is_picture(file) {
                self.chat_picture(ui, file, c);
            } else if path.is_file() {
                self.chat_file_card(ui, &path, "", c);
            }
        }
    }
}

fn clipped_line(p: &Painter, text: &str, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(10.0));
    p.layout_job(job)
}

/// A page with a folded corner, drawn in a 1.25 pt line inside `rect`.
fn page_icon(p: &Painter, rect: Rect, color: Color32) {
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * (rect.width() / 32.0);
    let line = Stroke::new(1.25, color);
    let outline = [at(8.0, 4.0), at(19.0, 4.0), at(25.0, 10.0), at(25.0, 28.0), at(8.0, 28.0), at(8.0, 4.0)];
    p.add(egui::Shape::line(outline.to_vec(), line));
    p.add(egui::Shape::line(vec![at(19.0, 4.0), at(19.0, 10.0), at(25.0, 10.0)], line));
    for y in [16.0, 20.0, 24.0] {
        p.line_segment([at(12.0, y), at(21.0, y)], line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat::list::ink;

    fn step(name: &str, file: &str) -> Step {
        Step { id: "t".into(), name: name.into(), ended: Some(1.0), files: vec![file.into()], ..Default::default() }
    }

    #[test]
    fn the_greys_are_neutral() {
        for v in [ink::DARK_BG, ink::DARK_PANEL, ink::DARK_SURFACE, ink::DARK_RAISED, ink::DARK_BAND, ink::DARK_LINE, ink::DARK_TEXT, ink::DARK_MUTED] {
            let (r, g, b) = ((v >> 16) as i32, (v >> 8 & 0xff) as i32, (v & 0xff) as i32);
            // 2 steps of blue at most: no purple cast
            assert!((b - r).abs() <= 2 && (g - r).abs() <= 2, "{v:#x}");
        }
    }

    #[test]
    fn a_written_picture_and_a_written_file_both_show_after_the_run() {
        let ctx = Context::default();
        let app = DrawingApp::default();
        let dir = std::env::temp_dir().join(format!("mago-media-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let (png, md) = (dir.join("g.png"), dir.join("r.md"));
        let mut bytes = Vec::new();
        image::DynamicImage::new_rgb8(40, 20).write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        fs::write(&png, bytes).unwrap();
        fs::write(&md, "# oi").unwrap();
        let steps = [step("Write", &png.to_string_lossy()), step("Write", &md.to_string_lossy()), step("Read", &md.to_string_lossy())];
        let refs: Vec<&Step> = steps.iter().collect();
        let mut seen = 0;
        for _ in 0..30 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 800.0))), ..Default::default() };
            let words = format!(
                "![a]({})

[r]({})

![x](https://x.dev/a.png)",
                png.to_string_lossy(),
                md.to_string_lossy()
            );
            ctx.run_ui(raw, |ui| {
                let c = app.colors();
                app.step_files(ui, &refs, &c);
                app.agent_words(ui, &words, 600.0, &c);
            })
            .textures_delta
            .clear();
            std::thread::sleep(std::time::Duration::from_millis(20));
            seen = app.chat_images.borrow().len();
            if seen > 0 && app.chat_images.borrow().values().all(|d| d.get().is_some()) {
                break;
            }
        }
        // the picture was decoded once under its path, whoever asked
        assert_eq!(seen, 1);
        assert!(app.chat_images.borrow().values().all(|d| matches!(d.get(), Some(Some(pic)) if pic.size == [40, 20])));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_click_on_the_agents_picture_shows_it_big_in_the_app_and_esc_closes_it() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        let png = std::env::temp_dir().join(format!("mago-zoom-{}.png", std::process::id()));
        let mut bytes = Vec::new();
        image::DynamicImage::new_rgb8(40, 20).write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        fs::write(&png, bytes).unwrap();
        let words = format!("![print]({})", png.to_string_lossy());
        let frame = |app: &mut DrawingApp, events: Vec<egui::Event>| {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), events, ..Default::default() };
            ctx.run_ui(raw, |ui| {
                app.chat_zoom_ui(ui.ctx());
                let c = app.colors();
                app.agent_words(ui, &words, 600.0, &c);
            })
            .textures_delta
            .clear();
        };
        for _ in 0..50 {
            frame(&mut app, vec![]);
            if app.chat_images.borrow().values().all(|d| d.get().is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let at = Pos2::new(16.0, 10.0);
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&mut app, vec![egui::Event::PointerMoved(at), press(true)]);
        frame(&mut app, vec![press(false)]);
        assert!(app.chat_pics.borrow().zoom.as_ref().is_some_and(|shown| !shown.island), "open over the app's window");
        frame(&mut app, vec![]);
        assert!(app.chat_pics.borrow().zoom.is_some(), "it stays open");
        let escape = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        frame(&mut app, vec![escape]);
        assert!(app.chat_pics.borrow().zoom.is_none(), "Esc closes it");
        let _ = fs::remove_file(png);
    }
}
