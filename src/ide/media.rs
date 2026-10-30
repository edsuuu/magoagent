//! Pictures and PDFs in the IDE: a tab shows the picture, or the PDF's pages one under the other.
//! Decoded off the UI thread; `background` hands them to the GPU, and the enlarged editor and the
//! card's panel both paint them.

use super::*;
use std::sync::{Arc, Mutex};

/// What opens as a picture: the formats the image crate is built with, and SVG.
const IMAGES: [&str; 7] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"];
/// The longest side a picture keeps: bigger ones are scaled down to it.
const LONGEST: u32 = 4096;
// ponytail: a PDF's first 40 pages, rendered up front at 1.5x (~4.5 MB an A4 page); past that,
// render the pages as they scroll into view
const PDF_PAGES: usize = 40;
const PDF_SCALE: f32 = 1.5;
/// Between a PDF's pages, and around them.
const GAP: f32 = 12.0;

pub(crate) struct Media {
    /// What the decoding thread gave back, taken once on the UI thread.
    decoded: Arc<Mutex<Option<Result<Vec<egui::ColorImage>, String>>>>,
    pages: Vec<egui::TextureHandle>,
    pub(crate) error: Option<String>,
    /// The PDF has more pages than were rendered.
    more: bool,
}

/// Whether `path` opens as a picture or a PDF rather than as text.
pub(crate) fn is_media(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    ext == "pdf" || IMAGES.contains(&ext.as_str())
}

impl Media {
    pub(crate) fn open(path: &Path, ctx: &egui::Context) -> Self {
        let decoded = Arc::new(Mutex::new(None));
        let (out, path, ctx) = (decoded.clone(), path.to_path_buf(), ctx.clone());
        let more = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
            && std::fs::read(&path).ok().and_then(|b| hayro::hayro_syntax::Pdf::new(b).ok()).is_some_and(|p| p.pages().len() > PDF_PAGES);
        std::thread::spawn(move || {
            *out.lock().unwrap() = Some(decode(&path));
            ctx.request_repaint();
        });
        Media { decoded, pages: vec![], error: None, more }
    }

    /// The decoded pages to the GPU, once they're there (`background`, every frame).
    pub(crate) fn upload(&mut self, ctx: &egui::Context) {
        let Some(done) = self.decoded.lock().unwrap().take() else { return };
        match done {
            Ok(images) => {
                self.pages =
                    images.into_iter().enumerate().map(|(i, image)| ctx.load_texture(format!("ide_media_{i}"), image, egui::TextureOptions::LINEAR)).collect()
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// The pages laid out `width` wide: each fitted to it (never blown up), under the one before.
    /// Their rects from `top`, and the height of them all.
    pub(crate) fn layout(&self, left: f32, top: f32, width: f32) -> (Vec<(egui::TextureId, Rect)>, f32) {
        let mut y = top + GAP;
        let room = (width - 2.0 * GAP).max(1.0);
        let rects = self
            .pages
            .iter()
            .map(|page| {
                let size = page.size_vec2();
                let size = size * (room / size.x).min(1.0);
                let rect = Rect::from_min_size(pos2(left + (width - size.x) / 2.0, y), size);
                y += size.y + GAP;
                (page.id(), rect)
            })
            .collect();
        (rects, y - top)
    }

    /// What to say instead of the pages: still opening, or why it didn't.
    pub(crate) fn note(&self) -> Option<String> {
        match (&self.error, self.pages.is_empty()) {
            (Some(e), _) => Some(e.clone()),
            (None, true) => Some("Abrindo…".into()),
            _ => None,
        }
    }

    /// "12 páginas", for the status line; also what was left out.
    pub(crate) fn summary(&self) -> String {
        match (self.pages.len(), self.more) {
            (n, true) => format!("primeiras {n} páginas"),
            (1, false) => self.pages[0].size().map(|s| s.to_string()).join(" × ") + " px",
            (n, false) => format!("{n} páginas"),
        }
    }

    /// The enlarged editor's view: the pages in a scroll area, on the editor's background.
    pub(crate) fn view(&self, ui: &mut Ui) {
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0, t::ISLAND);
        if let Some(note) = self.note() {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, note, t::ui(14.0), t::MUTED);
            return;
        }
        egui::ScrollArea::vertical().id_salt("media").auto_shrink(false).show(ui, |ui| {
            let left = ui.max_rect().left();
            let (pages, height) = self.layout(left, ui.max_rect().top(), ui.available_width());
            ui.allocate_space(vec2(ui.available_width(), height));
            for (id, rect) in pages {
                ui.painter().image(id, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        });
        // its size, or how many pages (and that a long PDF was cut)
        ui.painter().text(rect.right_bottom() - vec2(16.0, 10.0), Align2::RIGHT_BOTTOM, self.summary(), t::ui(12.0), t::MUTED);
    }
}

fn decode(path: &Path) -> Result<Vec<egui::ColorImage>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    match ext.as_str() {
        "pdf" => {
            let pdf = hayro::hayro_syntax::Pdf::new(bytes).map_err(|_| "PDF ilegível (ou protegido por senha)".to_string())?;
            let (cache, settings) = (hayro::RenderCache::new(), hayro::hayro_interpret::InterpreterSettings::default());
            let pixmap = hayro::PixmapSettings { x_scale: PDF_SCALE, y_scale: PDF_SCALE, bg_color: hayro::vello_cpu::color::palette::css::WHITE };
            let pages: Vec<_> = pdf
                .pages()
                .iter()
                .take(PDF_PAGES)
                .map(|page| {
                    let p = hayro::render(page, &cache, &settings, &hayro::RenderSettings::default(), &pixmap);
                    egui::ColorImage::from_rgba_premultiplied([p.width() as usize, p.height() as usize], p.data_as_u8_slice())
                })
                .collect();
            if pages.is_empty() { Err("PDF sem páginas".into()) } else { Ok(pages) }
        }
        "svg" => {
            let tree = resvg::usvg::Tree::from_data(&bytes, &resvg::usvg::Options::default()).map_err(|e| format!("SVG ilegível: {e}"))?;
            let size = tree.size();
            // drawn at twice its size, sharp when enlarged, within the longest side
            let scale = (2.0f32).min(LONGEST as f32 / size.width().max(size.height()));
            let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
            let mut pixmap = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("SVG sem tamanho")?;
            resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
            Ok(vec![egui::ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data())])
        }
        _ => {
            let image = image::load_from_memory(&bytes).map_err(|e| format!("Imagem ilegível: {e}"))?;
            let image = if image.width().max(image.height()) > LONGEST { image.resize(LONGEST, LONGEST, image::imageops::FilterType::Triangle) } else { image };
            let rgba = image.to_rgba8();
            Ok(vec![egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba)])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture and a PDF become their pages, laid out to the width without being blown up.
    #[test]
    fn pictures_and_pdfs_open_as_pages() {
        let dir = std::env::temp_dir().join(format!("magoagent-media-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        image::RgbaImage::new(300, 100).save(&png).unwrap();
        let svg = dir.join("b.svg");
        std::fs::write(&svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="red"/></svg>"#).unwrap();
        assert!(is_media(&png) && is_media(&svg) && is_media(Path::new("x.PDF")) && !is_media(Path::new("main.rs")));
        assert_eq!(decode(&png).unwrap()[0].size, [300, 100]);
        assert_eq!(decode(&svg).unwrap()[0].size, [80, 40], "SVG at twice its size");
        assert!(decode(&dir.join("nada.pdf")).is_err());
        std::fs::write(dir.join("c.pdf"), "não é pdf").unwrap();
        assert_eq!(decode(&dir.join("c.pdf")).unwrap_err(), "PDF ilegível (ou protegido por senha)");
        // a one-page PDF: Letter at 1.5x
        let pdf = dir.join("d.pdf");
        std::fs::write(&pdf, MINIMAL_PDF).unwrap();
        let pages = decode(&pdf).unwrap();
        assert_eq!((pages.len(), pages[0].size), (1, [918, 1188]));
        // in the view: fitted to the width, not blown up past its size
        let ctx = egui::Context::default();
        let mut media = Media::open(&png, &ctx);
        for _ in 0..200 {
            media.upload(&ctx);
            if !media.pages.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let (rects, height) = media.layout(0.0, 0.0, 174.0);
        assert_eq!((rects[0].1.size(), height), (vec2(150.0, 50.0), 50.0 + 2.0 * GAP));
        assert_eq!(media.layout(0.0, 0.0, 1000.0).0[0].1.size(), vec2(300.0, 100.0));
        assert_eq!(media.summary(), "300 × 100 px");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// One empty Letter page.
    const MINIMAL_PDF: &str = "%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >> endobj
trailer << /Root 1 0 R >>
%%EOF
";
}
