//! What the agent's words carry besides words: pictures (`![alt](path)`, a data address or a file
//! it wrote) drawn in the conversation and files as cards; a click opens either with the system.

use crate::app::browser::open_external;
use crate::app::chat::direct::folder;
use crate::app::chat::images::{Pic, decode, picture};
use crate::app::chat::list::Look;
use crate::app::chat::markdown::prose_with;
use crate::app::chat::segments::{Seg, data_uri, file_size, is_local, is_picture, middle, resolve};
use crate::app::chat::{image_key, tint};
use crate::app::ui::Tip;
use crate::prelude::*;
use crate::transcript::Step;
use std::path::Path;

/// The file card: 360 wide at most, a 32 pt page icon, the name over its kind and weight.
const CARD_MAX: f32 = 360.0;
const ICON: f32 = 32.0;
/// A picture in the conversation: its widest and tallest.
const PICTURE_MAX: Vec2 = Vec2::new(420.0, 320.0);

impl DrawingApp {
    /// The agent's words: markdown in bubbles, with its pictures and files drawn in between.
    pub(crate) fn agent_words(&self, ui: &mut egui::Ui, text: &str, width: f32, c: &UiColors) {
        prose_with(ui, text, c, self.dark(), width, true, &mut |ui, seg| match seg {
            Seg::Image { src, .. } => self.chat_picture(ui, src, c),
            Seg::File { label, path, linked } => {
                let path = resolve(path, self.chat_folder().as_deref());
                // a path in backticks that isn't a file is just a word
                if *linked || path.exists() {
                    self.chat_file_card(ui, &path, label, c);
                } else {
                    ui.label(egui::RichText::new(label).monospace().size(12.5).color(Look::new(self.dark(), c).soft));
                }
            }
            _ => {}
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
        let look = Look::new(self.dark(), c);
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
                        ui.label(egui::RichText::new("Imagem ilegível").size(12.0).color(look.muted));
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
        let edge = if look.dark {
            {
                let v = crate::app::chat::list::ink::DARK_EDGE;
                Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
            }
        } else {
            Color32::from_black_alpha(36)
        };
        ui.painter().rect_stroke(rect, 10, Stroke::new(1.0, edge), StrokeKind::Inside);
        let response = response.on_hover_cursor(CursorIcon::PointingHand).tip("Clique para abrir");
        if response.clicked() {
            // ponytail: a data-address picture has no file to open; the viewer (chat/viewer.rs) shows the images sent as such
            if let Some(path) = &path {
                open_external(&path.to_string_lossy());
            }
        }
    }

    /// A file named by the agent: a card with its name, kind and weight; a click opens it, a right
    /// click copies its path or shows its folder. `label` empty: the file's own name.
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
        if response.clicked() && meta.is_some() {
            open_external(&text);
        }
        crate::app::ui::menu_look::context_menu(&response, |ui| {
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
        let (dark, look) = (self.dark(), Look::new(self.dark(), c));
        let fade = if dim { 0.6 } else { 1.0 };
        let (ink_name, ink_detail) = (look.text.gamma_multiply(fade), look.muted.gamma_multiply(fade));
        let (name_font, detail_font) = (FontId::proportional(13.0), FontId::proportional(11.0));
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
        response.on_hover_text(id.to_string())
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
        let mut app = DrawingApp::default();
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
}
