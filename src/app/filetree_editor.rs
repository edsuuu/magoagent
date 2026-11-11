//! What a file tree node's editor pane shows of one file, as the reference app's: text (edited,
//! colored, saved), a picture (fitted, or at its size by a click, with its dimensions) or a PDF (its
//! pages one under the other), or why there is nothing to show. Its own, with no IDE behind it: only
//! the IDE's pure parts are called (the highlighter's tokens, the picture and PDF decoder).

use crate::app::filetree_ui::Palette;
use crate::ide::highlight::{self, Lang, St};
use crate::ide::media::{self, Media};
use crate::prelude::*;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// What the reference reads as text (2 MiB) and shows as a picture or PDF (64 MiB), at most.
/// What the reference plays or draws in its pane and the app does not: video, sound, and the picture formats left out.
const EXTERNAL: [&str; 14] = ["mp4", "m4v", "webm", "mov", "mp3", "wav", "ogg", "oga", "m4a", "flac", "aac", "opus", "ico", "avif"];
const TEXT_MAX: u64 = 2 << 20;
const MEDIA_MAX: u64 = 64 << 20;
/// A line of the code, at the reference's 12 px text; the gutter holds three digits at least.
const LINE: f32 = 17.0;
const FOOTER: f32 = 22.0;

pub(crate) struct TextBuf {
    pub(crate) text: String,
    pub(crate) dirty: bool,
    crlf: bool,
    bom: bool,
    mtime: Option<SystemTime>,
    /// The file changed on disk while this text has unsaved changes.
    pub(crate) external: bool,
    lang: &'static Lang,
    /// The colored text, by its text and size: tokenized again only when one changes.
    job: Option<(u64, egui::text::LayoutJob)>,
    /// Takes the keyboard on the next frame (a file opened for good).
    pub(crate) focus: bool,
}

pub(crate) struct MediaBuf {
    media: Media,
    pdf: bool,
    dims: Option<(u32, u32)>,
    size: u64,
    /// A click showed the picture at its size, scrolled.
    actual: bool,
}

pub(crate) enum Body {
    Text(TextBuf),
    Media(MediaBuf),
    /// Why there is nothing to show.
    Refused(String),
    /// A video or sound (or a picture format the app cannot draw): only the system's program shows it.
    External(String),
}

pub(crate) fn bytes(n: u64) -> String {
    match n {
        0..1024 => format!("{n} B"),
        1024..0x10_0000 => format!("{:.1} KB", n as f32 / 1024.0).replace('.', ","),
        _ => format!("{:.1} MB", n as f32 / 1_048_576.0).replace('.', ","),
    }
}

/// The file as the pane shows it: read now, off nothing (a file of 2 MiB at most).
pub(crate) fn load(path: &Path, ctx: &Context) -> Body {
    let meta = match fs::metadata(path) {
        Ok(m) if m.is_file() => m,
        Ok(_) => return Body::Refused("Não foi possível abrir este arquivo.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Body::Refused("Este arquivo não está mais no disco.".into()),
        Err(e) => return Body::Refused(format!("Não foi possível abrir este arquivo. {e}")),
    };
    let size = meta.len();
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if EXTERNAL.contains(&ext.as_str()) {
        return Body::External("O MagoAgent não mostra este tipo de arquivo aqui. O programa padrão do sistema mostra.".into());
    }
    if media::is_media(path) {
        if size > MEDIA_MAX {
            return Body::Refused(format!("Este arquivo tem {}, além do que a pré-visualização comporta.", bytes(size)));
        }
        let pdf = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        let dims = if pdf { None } else { dimensions(path) };
        return Body::Media(MediaBuf { media: Media::open(path, ctx), pdf, dims, size, actual: false });
    }
    if size > TEXT_MAX {
        return Body::Refused(format!("Este arquivo tem {}, além do que o editor embutido comporta.", bytes(size)));
    }
    let binary = || Body::Refused("Este não é um arquivo de texto, então não há nada para editar aqui.".into());
    let raw = match fs::read(path) {
        Ok(raw) => raw,
        Err(e) => return Body::Refused(format!("Não foi possível abrir este arquivo. {e}")),
    };
    if raw[..raw.len().min(8000)].contains(&0) {
        return binary();
    }
    let Ok(text) = String::from_utf8(raw) else { return binary() };
    let bom = text.starts_with('\u{feff}');
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let crlf = text.contains("\r\n");
    let text = if crlf { text.replace("\r\n", "\n") } else { text.to_string() };
    Body::Text(TextBuf { text, dirty: false, crlf, bom, mtime: meta.modified().ok(), external: false, lang: highlight::detect(path), job: None, focus: false })
}

/// A picture's own size in pixels (read from its header, not the decoded copy, which is cut to 4096).
fn dimensions(path: &Path) -> Option<(u32, u32)> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
        let tree = resvg::usvg::Tree::from_data(&fs::read(path).ok()?, &resvg::usvg::Options::default()).ok()?;
        return Some((tree.size().width().round() as u32, tree.size().height().round() as u32));
    }
    image::image_dimensions(path).ok()
}

impl TextBuf {
    /// Writes the text back as it came (line ends, BOM); false with why when it could not.
    pub(crate) fn save(&mut self, path: &Path) -> Result<(), String> {
        let text = if self.crlf { self.text.replace('\n', "\r\n") } else { self.text.clone() };
        let text = if self.bom { format!("\u{feff}{text}") } else { text };
        fs::write(path, text).map_err(|e| format!("Não foi possível salvar {}: {e}", path.display()))?;
        self.dirty = false;
        self.external = false;
        self.mtime = fs::metadata(path).and_then(|m| m.modified()).ok();
        Ok(())
    }

    /// The disk's news, now and then: a clean text follows the file, an edited one is told it changed.
    pub(crate) fn check_disk(&mut self, path: &Path) {
        let now = fs::metadata(path).and_then(|m| m.modified()).ok();
        if now == self.mtime {
            return;
        }
        if self.dirty {
            self.external = true;
        } else if let Body::Text(fresh) = load(path, &Context::default()) {
            let focus = self.focus;
            *self = fresh;
            self.focus = focus;
        }
    }
}

/// The reference's `--syntax-*` colors.
fn token_color(s: St, dark: bool, c: &Palette) -> Color32 {
    let hex = |d: u32, l: u32| {
        let h = if dark { d } else { l };
        Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
    };
    match s {
        St::Keyword | St::Tag => hex(0xff7ab2, 0xa82387),
        St::Str => hex(0xff8170, 0xc41a16),
        St::Number => hex(0xd9c97c, 0x1c00cf),
        St::Comment | St::Doc => hex(0x8495a5, 0x00733c),
        St::Function | St::Method => hex(0xdcdcaa, 0x795e26),
        St::Field => hex(0x9cdcfe, 0x0451a5),
        St::Meta => hex(0x6bdfff, 0x2b807e),
        St::Constant => hex(0xffa14f, 0x802387),
        St::Text | St::Bracket(_) => c.fg,
    }
}

fn job_of(buf: &TextBuf, font: &FontId, line: f32, dark: bool, c: &Palette) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let fmt =
        |color: Color32| egui::text::TextFormat { font_id: font.clone(), color, line_height: Some(line), valign: egui::Align::Center, ..Default::default() };
    let src = &buf.text;
    let mut at = 0;
    for (a, b, s) in highlight::tokens(buf.lang, src) {
        if a > at {
            job.append(&src[at..a], 0.0, fmt(c.fg));
        }
        if b > a {
            job.append(&src[a..b], 0.0, fmt(token_color(s, dark, c)));
        }
        at = at.max(b);
    }
    if at < src.len() {
        job.append(&src[at..], 0.0, fmt(c.fg));
    }
    job
}

/// The text in `body`: line numbers at the left, the colored text edited. Back: whether it changed.
fn text_ui(ui: &mut egui::Ui, body: Rect, buf: &mut TextBuf, id: egui::Id, z: f32, dark: bool, wrap: bool, c: &Palette) -> bool {
    let font = FontId::monospace(12.0 * z);
    let line = LINE * z;
    let lines = buf.text.matches('\n').count() + 1;
    let digit = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
    let gutter = Rect::from_min_size(body.min, Vec2::new(lines.to_string().len().max(3) as f32 * digit + 19.5 * z, body.height()));
    let area = Rect::from_min_max(Pos2::new(gutter.right(), body.top()), body.max);
    let key = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (buf.text.len(), z.to_bits(), dark).hash(&mut h);
        buf.text.hash(&mut h);
        h.finish()
    };
    if buf.job.as_ref().is_none_or(|(k, _)| *k != key) {
        buf.job = Some((key, job_of(buf, &font, line, dark, c)));
    }
    let job = buf.job.as_ref().map(|(_, j)| j.clone()).unwrap_or_default();
    let mut layouter = |ui: &egui::Ui, _: &dyn egui::TextBuffer, width: f32| {
        let mut job = job.clone();
        job.wrap.max_width = if wrap { width } else { f32::INFINITY };
        ui.painter().layout_job(job)
    };
    let want_focus = std::mem::take(&mut buf.focus);
    let (mut changed, mut origin, mut rows) = (false, Pos2::ZERO, vec![]);
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        ui.set_clip_rect(area.intersect(ui.clip_rect()));
        ui.visuals_mut().selection.bg_fill = Color32::from_rgba_unmultiplied(0x0a, 0x84, 0xff, 0x55);
        ui.visuals_mut().selection.stroke = Stroke::new(1.0, c.fg);
        let out = egui::ScrollArea::both().id_salt(id.with("scroll")).auto_shrink(false).show(ui, |ui| {
            let edit = egui::TextEdit::multiline(&mut buf.text)
                .id(id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::symmetric(0, 0))
                .font(font.clone())
                .layouter(&mut layouter)
                .lock_focus(true)
                .desired_width(if wrap { area.width() } else { f32::INFINITY })
                .desired_rows(1);
            let out = edit.show(ui);
            if want_focus {
                out.response.request_focus();
            }
            (out.response.changed(), out.galley_pos, out.galley.rows.iter().map(|r| (r.min_y(), r.height(), r.ends_with_newline)).collect::<Vec<_>>())
        });
        (changed, origin, rows) = out.inner;
    });
    // the line numbers: one per paragraph, level with it, in a column that stays when the text goes sideways
    let p = ui.painter().with_clip_rect(body.intersect(ui.clip_rect()));
    let mut n = 1;
    let mut starts = true;
    for (min_y, h, ends) in rows {
        let y = origin.y + min_y + h / 2.0;
        if starts && y > body.top() - line && y < body.bottom() + line {
            let color = c.muted;
            p.text(Pos2::new(gutter.right() - 6.5 * z, y), egui::Align2::RIGHT_CENTER, n.to_string(), font.clone(), color);
        }
        if starts {
            n += 1;
        }
        starts = ends;
    }
    changed
}

/// A picture, or a PDF's pages, in `body`; its dimensions and size under it.
fn media_ui(ui: &mut egui::Ui, body: Rect, m: &mut MediaBuf, z: f32, c: &Palette) {
    // what the decoder finished goes to the GPU
    m.media.upload(ui.ctx());
    let footer = Rect::from_min_max(Pos2::new(body.left(), body.bottom() - FOOTER * z), body.max);
    let view = Rect::from_min_max(body.min, Pos2::new(body.right(), footer.top()));
    let p = ui.painter().with_clip_rect(body.intersect(ui.clip_rect()));
    p.hline(footer.x_range(), footer.top(), Stroke::new(1.0, c.border));
    let text = match m.dims {
        Some((w, h)) => format!("{w} × {h} · {}", bytes(m.size)),
        None => bytes(m.size),
    };
    p.text(footer.center(), egui::Align2::CENTER_CENTER, text, FontId::proportional(11.0 * z), c.muted);
    if let Some(note) = m.media.note() {
        p.text(view.center(), egui::Align2::CENTER_CENTER, note, FontId::proportional(12.0 * z), c.muted);
        return;
    }
    if m.pdf {
        // the PDF viewer's gray, the pages on it, scrolled
        p.rect_filled(view, 0, Color32::from_rgb(0x52, 0x56, 0x59));
        ui.scope_builder(egui::UiBuilder::new().max_rect(view), |ui| {
            ui.set_clip_rect(view.intersect(ui.clip_rect()));
            egui::ScrollArea::vertical().id_salt("ft_pdf").auto_shrink(false).show(ui, |ui| {
                let (pages, height) = m.media.layout(view.left(), ui.max_rect().top(), view.width());
                ui.allocate_space(Vec2::new(view.width(), height));
                m.media.paint(ui.painter(), &pages, |r| r);
            });
        });
        return;
    }
    // a picture: all of it in the pane, or at its own size (scrolled) after a click
    let (pages, _) = m.media.layout(0.0, 0.0, 1.0e6);
    let Some(natural) = pages.first().map(|r| r.size()) else { return };
    let natural = m.dims.map_or(natural, |(w, h)| Vec2::new(w as f32, h as f32)) * z;
    let pad = 6.5 * z;
    ui.scope_builder(egui::UiBuilder::new().max_rect(view), |ui| {
        ui.set_clip_rect(view.intersect(ui.clip_rect()));
        egui::ScrollArea::both().id_salt("ft_img").auto_shrink(false).show(ui, |ui| {
            let room = Vec2::new(view.width() - 2.0 * pad, view.height() - 2.0 * pad).max(Vec2::splat(1.0));
            let size = if m.actual { natural } else { natural * (room.x / natural.x).min(room.y / natural.y).min(1.0) };
            let content = Vec2::new((size.x + 2.0 * pad).max(view.width()), (size.y + 2.0 * pad).max(view.height()));
            let (rect, response) = ui.allocate_exact_size(content, Sense::click());
            let at = Rect::from_center_size(rect.center(), size);
            m.media.paint(ui.painter(), &[at], |r| r);
            let response = response.on_hover_cursor(if m.actual { CursorIcon::ZoomOut } else { CursorIcon::ZoomIn });
            if response.clicked() {
                m.actual = !m.actual;
            }
        });
    });
}

/// The message in the middle of the pane, and the "Mostrar na Pasta" link under it. Back: whether the link was pressed.
pub(crate) fn message_ui(ui: &mut egui::Ui, body: Rect, why: &str, links: &[(&str, BodyAct)], z: f32, c: &Palette) -> BodyAct {
    let p = ui.painter().with_clip_rect(body.intersect(ui.clip_rect()));
    let galley = p.layout(why.to_string(), FontId::proportional(12.0 * z), c.muted, (body.width() - 26.0 * z).max(10.0));
    let top = body.center().y - galley.size().y / 2.0 - 12.0 * z;
    p.galley(Pos2::new(body.center().x - galley.size().x / 2.0, top), galley.clone(), c.muted);
    let mut pressed = BodyAct::None;
    for (i, (label, act)) in links.iter().enumerate() {
        let link = Rect::from_center_size(Pos2::new(body.center().x, top + galley.size().y + (16.0 + 24.0 * i as f32) * z), Vec2::new(190.0 * z, 22.0 * z));
        let r = ui.interact(link.intersect(ui.clip_rect()), ui.id().with(("link", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        if r.hovered() {
            p.rect_filled(link, 6, c.fg.gamma_multiply(0.1));
        }
        let width = p.text(link.center(), egui::Align2::CENTER_CENTER, *label, FontId::proportional(12.0 * z), c.fg).width();
        p.hline(link.center().x - width / 2.0..=link.center().x + width / 2.0, link.center().y + 7.0 * z, Stroke::new(1.0, c.fg));
        if r.clicked() {
            pressed = act.clone();
        }
    }
    pressed
}

/// What pressing in the body asked.
#[derive(Clone, Copy)]
pub(crate) enum BodyAct {
    None,
    /// "Abrir no App Padrão".
    OpenDefault,
    /// "Tentar Novamente" after a picture or PDF that could not be read.
    Retry,
    Reveal,
    /// "Salvar o Meu" on the banner of a file that changed under unsaved text.
    KeepMine,
}

/// The body of the active tab in `body`.
pub(crate) fn body_ui(ui: &mut egui::Ui, body: Rect, tab: &mut Body, path: &Path, z: f32, dark: bool, wrap: bool, c: &Palette) -> (BodyAct, bool) {
    match tab {
        Body::Refused(why) => (message_ui(ui, body, why, &[("Mostrar na Pasta", BodyAct::Reveal)], z, c), false),
        Body::External(why) => {
            (message_ui(ui, body, why, &[("Abrir no App Padrão", BodyAct::OpenDefault), ("Mostrar na Pasta", BodyAct::Reveal)], z, c), false)
        }
        Body::Media(m) if m.media.error.is_some() => (
            message_ui(ui, body, "Não foi possível ler este arquivo.", &[("Tentar Novamente", BodyAct::Retry), ("Mostrar na Pasta", BodyAct::Reveal)], z, c),
            false,
        ),
        Body::Media(m) => {
            media_ui(ui, body, m, z, c);
            (BodyAct::None, false)
        }
        Body::Text(buf) => {
            let mut act = BodyAct::None;
            let mut body = body;
            if buf.external {
                // the banner the reference puts over a file that changed under unsaved text
                let band = Rect::from_min_size(body.min, Vec2::new(body.width(), 28.0 * z));
                ui.painter().rect_filled(band, 0, c.hover);
                ui.painter().hline(band.x_range(), band.bottom(), Stroke::new(1.0, c.border));
                ui.painter().text(
                    Pos2::new(band.left() + 8.125 * z, band.center().y),
                    egui::Align2::LEFT_CENTER,
                    "Este arquivo mudou no disco enquanto você editava.",
                    FontId::proportional(11.0 * z),
                    c.fg,
                );
                let button = Rect::from_center_size(Pos2::new(band.right() - 50.0 * z, band.center().y), Vec2::new(84.0 * z, 20.0 * z));
                let r = ui.interact(button, ui.id().with("keep"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                if r.hovered() {
                    ui.painter().rect_filled(button, 6, c.fg.gamma_multiply(0.1));
                }
                ui.painter().text(button.center(), egui::Align2::CENTER_CENTER, "Salvar o Meu", FontId::proportional(11.0 * z), c.fg);
                if r.clicked() {
                    act = BodyAct::KeepMine;
                }
                body = Rect::from_min_max(Pos2::new(body.left(), band.bottom() + 1.0), body.max);
            }
            let id = egui::Id::unique(("ft_text", path.to_string_lossy().as_ref()));
            let changed = text_ui(ui, body, buf, id, z, dark, wrap, c);
            if changed {
                buf.dirty = true;
            }
            (act, changed)
        }
    }
}
