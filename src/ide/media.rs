//! Pictures and PDFs in the IDE: a tab shows the picture, or the PDF's pages one under the other.
//! Decoded off the UI thread; `background` hands them to the GPU, and the enlarged editor and the
//! card's panel both paint them. A PDF's pages are rendered as they scroll into view.

use super::*;
use eframe::egui::Vec2;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};

/// What opens as a picture: the formats the image crate is built with, and SVG.
const IMAGES: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "svg"];
/// The longest side a picture keeps: bigger ones are scaled down to it.
const LONGEST: u32 = 4096;
const PDF_SCALE: f32 = 1.5;
/// Pages kept as textures beyond the ones in view (a page is ~4.5 MB on the GPU).
const KEEP: usize = 2;
/// An animated GIF plays up to this many frames and pixels in all; past it, its first frame stands.
const GIF_FRAMES: usize = 300;
const GIF_PIXELS: usize = 32 << 20;
/// Between a PDF's pages, and around them.
const GAP: f32 = 12.0;

type Slot = Arc<Mutex<Option<Result<Decoded, String>>>>;
type Ready = Arc<Mutex<Vec<(usize, egui::ColorImage)>>>;

#[derive(Default)]
pub(crate) struct Media {
    /// What the decoding thread gave back, taken once on the UI thread.
    decoded: Slot,
    pages: Vec<Page>,
    pub(crate) error: Option<String>,
    /// The picture's real size (the texture may be scaled down).
    natural: Option<[usize; 2]>,
    /// An animated GIF: every frame and how long it stays (seconds).
    frames: Vec<(egui::TextureHandle, f64)>,
    pdf: Option<PdfJob>,
}

struct Page {
    /// Laid out at this size (a PDF page at its render scale).
    size: Vec2,
    tex: Option<egui::TextureHandle>,
}

enum Decoded {
    Images { images: Vec<egui::ColorImage>, delays: Vec<f64>, natural: [usize; 2] },
    Pdf(Vec<Vec2>),
}

/// The thread rendering a PDF's pages on request.
struct PdfJob {
    /// Pages the view wants, nearest first; the thread takes them one at a time.
    want: Arc<Mutex<Vec<usize>>>,
    /// The page being rendered now.
    busy: Arc<AtomicUsize>,
    ready: Ready,
    wake: mpsc::Sender<()>,
    asked: std::cell::RefCell<Vec<usize>>,
    /// First and last page in view (with a screen of margin).
    seen: std::cell::Cell<(usize, usize)>,
}

/// Whether `path` opens as a picture or a PDF rather than as text.
pub(crate) fn is_media(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    ext == "pdf" || IMAGES.contains(&ext.as_str())
}

impl Media {
    pub(crate) fn open(path: &Path, ctx: &egui::Context) -> Self {
        let mut media = Media::default();
        let (out, path, ctx) = (media.decoded.clone(), path.to_path_buf(), ctx.clone());
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            let (want, busy, ready) = (Arc::new(Mutex::new(vec![])), Arc::new(AtomicUsize::new(usize::MAX)), Ready::default());
            let (wake, woken) = mpsc::channel();
            media.pdf =
                Some(PdfJob { want: want.clone(), busy: busy.clone(), ready: ready.clone(), wake, asked: Default::default(), seen: Default::default() });
            std::thread::spawn(move || pdf_thread(&path, &ctx, &out, (want, busy, ready), woken));
        } else {
            std::thread::spawn(move || {
                *out.lock().unwrap() = Some(decode(&path));
                ctx.request_repaint();
            });
        }
        media
    }

    /// A file the editor won't open: `why` is what the panel says.
    pub(crate) fn refused(why: String) -> Self {
        Media { error: Some(why), ..Default::default() }
    }

    /// The decoded pages to the GPU, once they're there (`background`, every frame).
    pub(crate) fn upload(&mut self, ctx: &egui::Context) {
        let done = self.decoded.lock().unwrap().take();
        match done {
            Some(Ok(Decoded::Images { images, delays, natural })) => {
                self.natural = Some(natural);
                let textures: Vec<_> =
                    images.into_iter().enumerate().map(|(i, image)| ctx.load_texture(format!("ide_media_{i}"), image, egui::TextureOptions::LINEAR)).collect();
                self.pages = vec![Page { size: textures[0].size_vec2(), tex: Some(textures[0].clone()) }];
                if textures.len() > 1 {
                    self.frames = textures.into_iter().zip(delays).collect();
                }
            }
            Some(Ok(Decoded::Pdf(sizes))) => self.pages = sizes.into_iter().map(|size| Page { size, tex: None }).collect(),
            Some(Err(e)) => self.error = Some(e),
            None => {}
        }
        if let Some(pdf) = &self.pdf {
            for (i, image) in std::mem::take(&mut *pdf.ready.lock().unwrap()) {
                if let Some(page) = self.pages.get_mut(i) {
                    page.tex = Some(ctx.load_texture(format!("ide_media_{i}"), image, egui::TextureOptions::LINEAR));
                }
            }
            // far from view, a page's texture goes
            let (first, last) = pdf.seen.get();
            for (i, page) in self.pages.iter_mut().enumerate() {
                if i + KEEP < first || i > last + KEEP {
                    page.tex = None;
                }
            }
        }
    }

    /// The pages laid out `width` wide: each fitted to it (never blown up), under the one before.
    /// Their rects from `top`, and the height of them all.
    pub(crate) fn layout(&self, left: f32, top: f32, width: f32) -> (Vec<Rect>, f32) {
        let mut y = top + GAP;
        let room = (width - 2.0 * GAP).max(1.0);
        let rects = self
            .pages
            .iter()
            .map(|page| {
                let size = page.size * (room / page.size.x).min(1.0);
                let rect = Rect::from_min_size(pos2(left + (width - size.x) / 2.0, y), size);
                y += size.y + GAP;
                rect
            })
            .collect();
        (rects, y - top)
    }

    /// Paints the pages at `rects` (from `layout`, moved by `map`) that are in view, and asks for the
    /// textures of a PDF's.
    pub(crate) fn paint(&self, painter: &egui::Painter, rects: &[Rect], map: impl Fn(Rect) -> Rect) {
        let clip = painter.clip_rect();
        let now = painter.ctx().input(|i| i.time);
        // a screen of margin each way, so the next page is there before it scrolls in
        let near = clip.expand2(vec2(0.0, clip.height()));
        let mut seen = vec![];
        for (i, rect) in rects.iter().enumerate() {
            let rect = map(*rect);
            if !rect.intersects(near) {
                continue;
            }
            seen.push(i);
            match self.texture(i, now, painter.ctx()) {
                Some(id) => painter.image(id, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE),
                None => painter.rect_filled(rect, 0, Color32::WHITE.gamma_multiply(0.06)),
            };
        }
        if let (Some(pdf), Some((&first, &last))) = (&self.pdf, seen.first().zip(seen.last())) {
            pdf.seen.set((first, last));
            let need: Vec<usize> = seen.into_iter().filter(|&i| self.pages[i].tex.is_none() && pdf.busy.load(Ordering::Relaxed) != i).collect();
            if need != *pdf.asked.borrow() {
                *pdf.want.lock().unwrap() = need.clone();
                *pdf.asked.borrow_mut() = need;
                pdf.wake.send(()).ok();
            }
        }
    }

    /// Page `i`'s texture: for an animated GIF, the frame of the moment.
    fn texture(&self, i: usize, now: f64, ctx: &egui::Context) -> Option<egui::TextureId> {
        if i == 0 && !self.frames.is_empty() {
            let mut at = now % self.frames.iter().map(|f| f.1).sum::<f64>();
            let frame = self
                .frames
                .iter()
                .find(|f| {
                    at -= f.1;
                    at < 0.0
                })
                .unwrap_or(&self.frames[0]);
            ctx.request_repaint_after(Duration::from_secs_f64(frame.1));
            return Some(frame.0.id());
        }
        self.pages[i].tex.as_ref().map(|t| t.id())
    }

    /// What to say instead of the pages: still opening, or why it didn't.
    pub(crate) fn note(&self) -> Option<String> {
        match (&self.error, self.pages.is_empty()) {
            (Some(e), _) => Some(e.clone()),
            (None, true) => Some("Abrindo…".into()),
            _ => None,
        }
    }

    /// "12 páginas", or a picture's real size, for the status line.
    pub(crate) fn summary(&self) -> String {
        match (self.pages.len(), self.natural) {
            (_, Some([w, h])) => format!("{w} × {h} px"),
            (1, None) => "1 página".into(),
            (n, None) => format!("{n} páginas"),
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
            self.paint(ui.painter(), &pages, |r| r);
        });
        // its size, or how many pages
        ui.painter().text(rect.right_bottom() - vec2(16.0, 10.0), Align2::RIGHT_BOTTOM, self.summary(), t::ui(12.0), t::MUTED);
    }
}

/// Opens the PDF, tells the page sizes, then renders the pages asked for until the `Media` is gone.
fn pdf_thread(path: &Path, ctx: &egui::Context, out: &Slot, (want, busy, ready): (Arc<Mutex<Vec<usize>>>, Arc<AtomicUsize>, Ready), woken: mpsc::Receiver<()>) {
    let opened = std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|b| hayro::hayro_syntax::Pdf::new(b).map_err(|_| "PDF ilegível (ou protegido por senha)".to_string()));
    let pdf = match opened {
        Ok(pdf) if pdf.pages().is_empty() => return *out.lock().unwrap() = Some(Err("PDF sem páginas".into())),
        Ok(pdf) => pdf,
        Err(e) => return *out.lock().unwrap() = Some(Err(e)),
    };
    let sizes = pdf.pages().iter().map(|page| page.render_dimensions()).map(|(w, h)| vec2((w * PDF_SCALE).ceil(), (h * PDF_SCALE).ceil())).collect();
    *out.lock().unwrap() = Some(Ok(Decoded::Pdf(sizes)));
    ctx.request_repaint();
    let (cache, settings) = (hayro::RenderCache::new(), hayro::hayro_interpret::InterpreterSettings::default());
    let pixmap = hayro::PixmapSettings { x_scale: PDF_SCALE, y_scale: PDF_SCALE, bg_color: hayro::vello_cpu::color::palette::css::WHITE };
    while woken.recv().is_ok() {
        loop {
            let next = {
                let mut want = want.lock().unwrap();
                (!want.is_empty()).then(|| want.remove(0))
            };
            let Some(i) = next else { break };
            busy.store(i, Ordering::Relaxed);
            if let Some(page) = pdf.pages().get(i) {
                let p = hayro::render(page, &cache, &settings, &hayro::RenderSettings::default(), &pixmap);
                let image = egui::ColorImage::from_rgba_premultiplied([p.width() as usize, p.height() as usize], p.data_as_u8_slice());
                ready.lock().unwrap().push((i, image));
            }
            busy.store(usize::MAX, Ordering::Relaxed);
            ctx.request_repaint();
        }
    }
}

fn decode(path: &Path) -> Result<Decoded, String> {
    use image::AnimationDecoder;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    let one = |image: egui::ColorImage, natural: [usize; 2]| Ok(Decoded::Images { images: vec![image], delays: vec![], natural });
    if ext == "svg" {
        let tree = resvg::usvg::Tree::from_data(&bytes, &resvg::usvg::Options::default()).map_err(|e| format!("SVG ilegível: {e}"))?;
        let size = tree.size();
        // drawn at twice its size, sharp when enlarged, within the longest side
        let scale = (2.0f32).min(LONGEST as f32 / size.width().max(size.height()));
        let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
        let mut pixmap = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("SVG sem tamanho")?;
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
        return one(
            egui::ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data()),
            [size.width().ceil() as usize, size.height().ceil() as usize],
        );
    }
    if ext == "gif"
        && let Ok(decoder) = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&bytes))
    {
        let mut pixels = 0;
        let frames: Vec<_> = decoder
            .into_frames()
            .take(GIF_FRAMES)
            .map_while(|f| f.ok())
            .take_while(|f| {
                pixels += (f.buffer().width() * f.buffer().height()) as usize;
                pixels <= GIF_PIXELS
            })
            .collect();
        if frames.len() > 1 {
            let (w, h) = frames[0].buffer().dimensions();
            let seconds = |f: &image::Frame| {
                let (n, d) = f.delay().numer_denom_ms();
                (n as f64 / d as f64 / 1000.0).max(0.02)
            };
            let images = frames.iter().map(|f| egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], f.buffer())).collect();
            return Ok(Decoded::Images { images, delays: frames.iter().map(seconds).collect(), natural: [w as usize, h as usize] });
        }
    }
    let image = image::load_from_memory(&bytes).map_err(|e| format!("Imagem ilegível: {e}"))?;
    let natural = [image.width() as usize, image.height() as usize];
    let image = if image.width().max(image.height()) > LONGEST { image.resize(LONGEST, LONGEST, image::imageops::FilterType::Triangle) } else { image };
    let rgba = image.to_rgba8();
    one(egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba), natural)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Waits for `done` to hold, uploading what the threads decoded.
    fn until(media: &mut Media, ctx: &egui::Context, done: impl Fn(&Media) -> bool) {
        for _ in 0..500 {
            media.upload(ctx);
            if done(media) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("media never got there");
    }

    fn size_of(decoded: Decoded) -> [usize; 2] {
        match decoded {
            Decoded::Images { images, .. } => images[0].size,
            Decoded::Pdf(_) => unreachable!(),
        }
    }

    /// `n` empty Letter pages.
    fn pdf_of(n: usize) -> String {
        let kids: String = (0..n).map(|i| format!("{} 0 R ", 3 + i)).collect();
        let pages: String = (0..n).map(|i| format!("{} 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >> endobj\n", 3 + i)).collect();
        format!(
            "%PDF-1.4\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [{kids}] /Count {n} >> endobj\n{pages}trailer << /Root 1 0 R >>\n%%EOF\n"
        )
    }

    /// A picture and a PDF become their pages, laid out to the width without being blown up.
    #[test]
    fn pictures_and_pdfs_open_as_pages() {
        let dir = std::env::temp_dir().join(format!("magoagent-media-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        image::RgbaImage::new(300, 100).save(&png).unwrap();
        let ico = dir.join("i.ico");
        image::RgbaImage::new(32, 32).save(&ico).unwrap();
        let svg = dir.join("b.svg");
        std::fs::write(&svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="red"/></svg>"#).unwrap();
        assert!(is_media(&png) && is_media(&svg) && is_media(&ico) && is_media(Path::new("x.PDF")) && !is_media(Path::new("main.rs")));
        assert_eq!(size_of(decode(&png).unwrap()), [300, 100]);
        assert_eq!(size_of(decode(&ico).unwrap()), [32, 32]);
        assert_eq!(size_of(decode(&svg).unwrap()), [80, 40], "SVG at twice its size");
        let ctx = egui::Context::default();
        // a PDF that isn't one says so
        std::fs::write(dir.join("c.pdf"), "não é pdf").unwrap();
        let mut bad = Media::open(&dir.join("c.pdf"), &ctx);
        until(&mut bad, &ctx, |m| m.error.is_some());
        assert_eq!(bad.error.as_deref(), Some("PDF ilegível (ou protegido por senha)"));
        // in the view: fitted to the width, not blown up past its size
        let mut media = Media::open(&png, &ctx);
        until(&mut media, &ctx, |m| !m.pages.is_empty());
        let (rects, height) = media.layout(0.0, 0.0, 174.0);
        assert_eq!((rects[0].size(), height), (vec2(150.0, 50.0), 50.0 + 2.0 * GAP));
        assert_eq!(media.layout(0.0, 0.0, 1000.0).0[0].size(), vec2(300.0, 100.0));
        assert_eq!(media.summary(), "300 × 100 px");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Past 40 pages: every page is there, and only the ones asked for get rendered (Letter at 1.5x).
    #[test]
    fn a_long_pdf_renders_its_pages_on_demand() {
        let dir = std::env::temp_dir().join(format!("magoagent-media-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("longo.pdf");
        std::fs::write(&path, pdf_of(60)).unwrap();
        let ctx = egui::Context::default();
        let mut media = Media::open(&path, &ctx);
        until(&mut media, &ctx, |m| !m.pages.is_empty());
        assert_eq!((media.pages.len(), media.summary()), (60, "60 páginas".to_string()));
        assert!(media.pages.iter().all(|p| p.tex.is_none() && p.size == vec2(918.0, 1188.0)));
        // the last page comes into view
        let pdf = media.pdf.as_ref().unwrap();
        *pdf.want.lock().unwrap() = vec![59];
        pdf.wake.send(()).unwrap();
        pdf.seen.set((58, 59));
        until(&mut media, &ctx, |m| m.pages[59].tex.is_some());
        assert_eq!(media.pages[59].tex.as_ref().unwrap().size(), [918, 1188]);
        assert!(media.pages[..59].iter().all(|p| p.tex.is_none()));
        // and goes again once it is far from view
        media.pdf.as_ref().unwrap().seen.set((0, 1));
        media.upload(&ctx);
        assert!(media.pages[59].tex.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// An animated GIF keeps its frames; a refused file is only its message.
    #[test]
    fn gifs_animate_and_refusals_say_why() {
        let dir = std::env::temp_dir().join(format!("magoagent-media-gif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.gif");
        let mut gif = image::codecs::gif::GifEncoder::new(std::fs::File::create(&path).unwrap());
        for shade in [0u8, 255] {
            let frame = image::RgbaImage::from_pixel(8, 6, image::Rgba([shade, 0, 0, 255]));
            gif.encode_frame(image::Frame::from_parts(frame, 0, 0, image::Delay::from_numer_denom_ms(100, 1))).unwrap();
        }
        drop(gif);
        let ctx = egui::Context::default();
        let mut media = Media::open(&path, &ctx);
        until(&mut media, &ctx, |m| !m.pages.is_empty());
        assert_eq!((media.frames.len(), media.summary()), (2, "8 × 6 px".to_string()));
        let refused = Media::refused("Este não é um arquivo de texto, então não há nada para editar aqui.".into());
        assert_eq!(refused.note().as_deref(), Some("Este não é um arquivo de texto, então não há nada para editar aqui."));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A binary, a UTF-16 file and one too big are refused with the editor's message in their own tab.
    #[test]
    fn refused_files_say_why_in_their_tab() {
        let dir = std::env::temp_dir().join(format!("magoagent-refused-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let files = [
            ("bin.dat", vec![0u8, 159, 146, 150], "Este não é um arquivo de texto, então não há nada para editar aqui."),
            ("utf16.txt", vec![0xFF, 0xFE, b'a', 0, b'b', 0], "Este não é um arquivo de texto, então não há nada para editar aqui."),
            ("grande.rs", vec![b'a'; MAX_TEXT as usize + 1], "Este arquivo tem 256 KB, além do que o editor embutido comporta."),
        ];
        let mut ide = Ide::new(egui::Context::default(), "refused", &dir.to_string_lossy());
        for (name, bytes, why) in files {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            ide.open_file(path.clone(), None);
            let tab = &ide.tabs[ide.active];
            assert_eq!((tab.path.clone(), tab.media.as_ref().and_then(|m| m.note()).as_deref()), (path, Some(why)), "{name}");
            assert!(!tab.is_text() && ide.status == why);
        }
        // text up to the limit still opens
        let ok = dir.join("ok.rs");
        std::fs::write(&ok, vec![b'a'; MAX_TEXT as usize]).unwrap();
        ide.open_file(ok, None);
        assert!(ide.tabs[ide.active].is_text());
        std::fs::remove_dir_all(&dir).ok();
    }
}
