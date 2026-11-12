//! What a file tree node's editor pane shows of one file, as the reference app's: text (edited,
//! colored, saved), a picture (fitted, or at its size by a click, with its dimensions) or a PDF (its
//! pages one under the other), a commit's diff of a file (read only), or why there is nothing to show. Its
//! own, with no IDE behind it: only the IDE's pure parts are called (the picture and PDF decoder here; the
//! code's look is in filetree/code.rs).

use crate::app::filetree::code::{Click, Laid, Project, text_ui};
use crate::app::filetree::compare::{Compare, compare_ui};
use crate::app::filetree::diff::{Hunk, head_text};
use crate::app::filetree::ui::Palette;
use crate::ide::highlight::{self, Lang, St};
use crate::ide::lsp::Diag;
use crate::ide::media::{self, Media};
use crate::prelude::*;
use std::ops::Range;
use std::path::Path;

/// What the reference reads as text (2 MiB) and shows as a picture or PDF (64 MiB), at most.
/// What the reference plays or draws in its pane and the app does not: video, sound, and the picture formats left out.
const EXTERNAL: [&str; 14] = ["mp4", "m4v", "webm", "mov", "mp3", "wav", "ogg", "oga", "m4a", "flac", "aac", "opus", "ico", "avif"];
const TEXT_MAX: u64 = 2 << 20;
const MEDIA_MAX: u64 = 64 << 20;
const FOOTER: f32 = 22.0;

pub(crate) struct TextBuf {
    pub(crate) text: String,
    pub(crate) dirty: bool,
    crlf: bool,
    bom: bool,
    mtime: Option<SystemTime>,
    /// The file changed on disk while this text has unsaved changes.
    pub(crate) external: bool,
    pub(crate) lang: &'static Lang,
    /// The colored text and what is drawn around it, by its text and size: worked out again only when one changes.
    pub(crate) laid: Option<Laid>,
    /// The bracket next to the caret and its pair, by the text and the caret they were found for.
    #[allow(clippy::type_complexity)]
    pub(crate) bracket: Option<((u64, Option<usize>), Option<(Range<usize>, St, Option<Range<usize>>)>)>,
    /// Takes the keyboard on the next frame (a file opened for good).
    pub(crate) focus: bool,
    /// The line and column the caret goes to on the next frame (a Ctrl+click's declaration).
    pub(crate) jump: Option<(usize, usize)>,
    /// The file as the last commit has it (None outside git, or for a file git does not have yet): what the gutter's change
    /// markers compare the text with (filetree/diff.rs).
    // ponytail: read when the file opens or reloads; a commit made elsewhere shows once it is reopened
    pub(crate) head: Option<String>,
    /// The changed ranges of the text, by the text's key.
    pub(crate) hunks: Option<(u64, Vec<Hunk>)>,
    /// The marker whose popup is open.
    pub(crate) pop: Option<usize>,
    /// The annotation column (who wrote each line, and when) is shown left of the line numbers (filetree/annotate.rs).
    pub(crate) annotate: bool,
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
    /// What a commit changed in a file (`git show`), read only: a tab the history of a declaration opens.
    Diff(String),
    /// A table's data or an SQL console of the Banco de Dados panel (filetree/db.rs).
    Sheet(Box<crate::db::Sheet>),
    /// A file against the last commit, side by side (filetree/compare.rs).
    Compare(Box<Compare>),
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
    let lang = highlight::detect(path);
    Body::Text(TextBuf {
        text,
        dirty: false,
        crlf,
        bom,
        mtime: meta.modified().ok(),
        external: false,
        lang,
        laid: None,
        bracket: None,
        focus: false,
        jump: None,
        head: head_text(path),
        hunks: None,
        pop: None,
        annotate: false,
    })
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
        let r = ui.interact(link.intersect(ui.clip_rect()), ui.scope_id().with(("link", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
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

/// A commit's diff in `body`, as git wrote it: added lines green, removed ones red, the hunks' headers as links. It can
/// be selected and copied, not changed.
fn diff_ui(ui: &mut egui::Ui, body: Rect, mut text: &str, id: egui::Id, z: f32) {
    use crate::ide::theme as t;
    let font = t::mono(t::EDITOR_SIZE * z);
    ui.painter().with_clip_rect(body.intersect(ui.clip_rect())).rect_filled(body.shrink2(Vec2::new(1.0, 1.0)), 0, t::ISLAND);
    ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
        ui.set_clip_rect(body.intersect(ui.clip_rect()));
        ui.visuals_mut().selection.bg_fill = t::EDITOR_SELECTION;
        egui::ScrollArea::both().id_salt(id.with("scroll")).auto_shrink(false).show(ui, |ui| {
            // ponytail: colored again each frame; a diff of many thousand lines wants the job kept
            let mut layouter = |ui: &egui::Ui, b: &dyn egui::TextBuffer, _: f32| {
                let mut job = egui::text::LayoutJob::default();
                for line in b.as_str().split_inclusive('\n') {
                    let color = match line.as_bytes().first() {
                        Some(b'+') if !line.starts_with("+++") => t::STRING,
                        Some(b'-') if !line.starts_with("---") => t::ERROR,
                        Some(b'@') => t::LINK,
                        Some(b' ') => t::EDITOR_TEXT,
                        _ => t::COMMENT,
                    };
                    job.append(line, 0.0, egui::TextFormat { font_id: font.clone(), color, ..Default::default() });
                }
                ui.fonts_mut(|f| f.layout_job(job))
            };
            let margin = egui::Margin::symmetric((10.0 * z).round() as i8, (6.0 * z).round() as i8);
            egui::TextEdit::multiline(&mut text)
                .id(id)
                .frame(egui::Frame::NONE.inner_margin(margin))
                .desired_width(f32::INFINITY)
                .layouter(&mut layouter)
                .show(ui);
        });
    });
}

/// What pressing in the body asked.
#[derive(Clone)]
pub(crate) enum BodyAct {
    None,
    /// "Abrir no App Padrão".
    OpenDefault,
    /// "Tentar Novamente" after a picture or PDF that could not be read.
    Retry,
    Reveal,
    /// "Salvar o Meu" on the banner of a file that changed under unsaved text.
    KeepMine,
    /// A press on the code: a name's declaration or usages, a label's list, "Localizar nos Arquivos".
    Code(Click),
}

/// The body of the active tab in `body`; `code` is what the tree's project knows (filetree/code.rs).
#[allow(clippy::too_many_arguments)]
pub(crate) fn body_ui(
    ui: &mut egui::Ui,
    body: Rect,
    tab: &mut Body,
    path: &Path,
    z: f32,
    wrap: bool,
    c: &Palette,
    code: &Project,
    diags: &[Diag],
) -> (BodyAct, bool) {
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
        Body::Sheet(sheet) => {
            crate::app::filetree::db::sheet_ui(ui, body, sheet);
            (BodyAct::None, false)
        }
        Body::Diff(text) => {
            diff_ui(ui, body, text, egui::Id::unique(("ft_diff", path.to_string_lossy().as_ref())), z);
            (BodyAct::None, false)
        }
        Body::Compare(cmp) => {
            compare_ui(ui, body, cmp, z);
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
                let r = ui.interact(button, ui.scope_id().with("keep"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
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
            let (changed, click) = text_ui(ui, body, buf, path, id, z, wrap, code, diags);
            if changed {
                buf.dirty = true;
            }
            (click.map_or(act, BodyAct::Code), changed)
        }
    }
}
