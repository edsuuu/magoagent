//! Files dropped on the canvas (or handed to `canvas_drop`) and pictures pasted onto it: a text note
//! becomes a note, a web page a portal, a folder a file tree, a picture a picture, and any other
//! file a card with its name and size and the buttons that open it.

use crate::app::chat_direct::folder;
use crate::prelude::*;
use std::path::Path;

/// Notes and pages are this size when they come from a file (portals the reference app's 600×400).
const NOTE_DROP: Vec2 = Vec2::new(300.0, 250.0);
const PAGE_DROP: Vec2 = Vec2::new(600.0, 400.0);
const FILE_DROP: Vec2 = Vec2::new(260.0, 80.0);
/// Each file of a drop lands this far down and right of the one before.
const CASCADE: f32 = 30.0;
/// A text file bigger than this is a file card, not a note.
const NOTE_MAX: u64 = 1 << 20;
/// Folders that are really one file (macOS packages).
const PACKAGES: [&str; 8] = ["app", "framework", "bundle", "kext", "plugin", "xcodeproj", "xcworkspace", "photoslibrary"];
/// Programs: `open_default` never starts them.
const PROGRAMS: [&str; 11] = ["exe", "bat", "cmd", "com", "ps1", "sh", "app", "msi", "vbs", "scr", "command"];

fn ext_of(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Pictures the canvas takes: the image formats, and SVG (drawn to a picture when it goes in).
pub(crate) fn is_canvas_image(path: &Path) -> bool {
    let ext = ext_of(path);
    ext == "svg" || IMAGE_EXTENSIONS.contains(&ext.as_str())
}

impl DrawingApp {
    /// Each path becomes what the reference app makes of it, cascading from `at` (world). Back: what was made.
    pub(crate) fn drop_on_canvas(&mut self, paths: &[PathBuf], at: Pos2) -> Result<String, String> {
        let missing: Vec<String> = paths.iter().filter(|p| !p.exists()).map(|p| p.display().to_string()).collect();
        if !missing.is_empty() {
            return Err(format!("não existe: {}", missing.join(", ")));
        }
        let (mut made, mut images, mut elements) = (vec![], vec![], vec![]);
        for (k, path) in paths.iter().enumerate() {
            let center = at + Vec2::splat(CASCADE * k as f32);
            let (name, ext) = (path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), ext_of(path));
            let rect = |size: Vec2| Rect::from_center_size(center, size);
            if path.is_dir() && !PACKAGES.contains(&ext.as_str()) {
                let r = rect(FILETREE_SIZE);
                let mut node = self.new_element(ElementKind::Rectangle { x: r.min.x, y: r.min.y, width: r.width(), height: r.height() });
                node.style = node_style(node.style);
                node.filetree = Some(FileTreeSpec { root: path.to_string_lossy().into(), ..Default::default() });
                self.prefs.folders.remember(&path.to_string_lossy());
                elements.push(node);
                made.push(format!("{name}: árvore de arquivos"));
            } else if is_canvas_image(path) {
                images.push(path.clone());
                made.push(format!("{name}: imagem"));
            } else if matches!(ext.as_str(), "html" | "htm") {
                let r = rect(PAGE_DROP);
                let mut node = self.new_element(ElementKind::Rectangle { x: r.min.x, y: r.min.y, width: r.width(), height: r.height() });
                node.style = node_style(node.style);
                let url = format!("file:///{}", path.to_string_lossy().replace('\\', "/").trim_start_matches('/'));
                node.portal = Some(PortalSpec { name: name.rsplit_once('.').map_or(name.clone(), |(stem, _)| stem.into()), url, ..Default::default() });
                elements.push(node);
                made.push(format!("{name}: navegador"));
            } else if let Some(text) = Some(&ext).filter(|e| matches!(e.as_str(), "md" | "markdown" | "txt")).and_then(|_| note_text(path)) {
                let [mut note, label] = self.new_note(rect(NOTE_DROP), text);
                // only markdown is formatted; a .txt shows as typed
                note.note.raw = ext == "txt";
                elements.extend([note, label]);
                made.push(format!("{name}: nota"));
            } else {
                elements.extend(self.file_card(path, &name, rect(FILE_DROP)));
                made.push(format!("{name}: arquivo"));
            }
        }
        self.insert_elements(elements);
        if !images.is_empty() {
            self.insert_image_files(images, at);
        }
        Ok(made.join("\n"))
    }

    /// A note that names the file and its size, with the buttons that open it (in the IDE of its project, or in the
    /// system's program for it).
    fn file_card(&self, path: &Path, name: &str, rect: Rect) -> [Element; 2] {
        let size = std::fs::metadata(path).map_or(0, |m| m.len());
        let full = path.to_string_lossy().to_string();
        let buttons = vec![
            NoteButton { label: "Abrir no IDE".into(), action: "open_path".into(), args: json!({ "path": full }) },
            NoteButton { label: "Abrir no programa padrão".into(), action: "open_default".into(), args: json!({ "path": full }) },
        ];
        let rect = Rect::from_min_size(rect.min, rect.size() + Vec2::new(0.0, crate::app::note_buttons::height(&buttons, rect.width())));
        let [mut note, label] = self.new_note(rect, format!("{name}\n{}", crate::app::chat_images::size_label(size)));
        // the text as it is: a name with `_` or `*` is not markdown
        (note.note.raw, note.note.buttons) = (true, buttons);
        [note, label]
    }

    /// An image pasted with Ctrl+V and no text on the clipboard: a picture at the pointer, or the middle of the view.
    pub(crate) fn paste_image(&mut self, image: &egui::ColorImage) {
        let Some(path) = crate::app::pointer::save_pasted(image, &std::env::temp_dir()) else { return };
        let at = self.last_pointer.filter(|p| self.canvas_rect.contains(*p)).unwrap_or(self.canvas_rect.center());
        self.insert_image_files(vec![path.clone()], self.to_world(at));
        let _ = fs::remove_file(path);
    }

    /// `open_default`: the file in the system's program for it. Programs are not started from here.
    pub(crate) fn act_open_default(&mut self, path: &str) -> Result<String, String> {
        let full = folder(path).filter(|p| p.exists()).ok_or_else(|| format!("{path} não existe."))?;
        let name = full.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        // ponytail: judged by the extension; a Linux binary with none opens as any file
        if !full.is_dir() && PROGRAMS.contains(&ext_of(&full).as_str()) {
            return Err(format!("{name} é um programa: o MagoAgent não o executa."));
        }
        #[cfg(not(test))]
        crate::app::portal::open_external(&full.to_string_lossy());
        Ok(format!("{name} aberto no programa padrão."))
    }
}

/// A note file's text, unless it is too big or not text.
fn note_text(path: &Path) -> Option<String> {
    let small = fs::metadata(path).is_ok_and(|m| m.len() <= NOTE_MAX);
    small.then(|| fs::read(path).ok().and_then(|b| String::from_utf8(b).ok()))?.map(|t| t.replace("\r\n", "\n").trim_end().to_string())
}
