//! Open, save, export and image insertion.

use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn save_as(&mut self) {
        let path = rfd::FileDialog::new()
            .set_title("Salvar como")
            .add_filter("Desenho MagoAgent (.mago)", &["mago"])
            .set_file_name("desenho.mago")
            .save_file();
        if let Some(path) = path {
            self.save_to(path);
        }
    }

    pub(crate) fn save(&mut self) {
        match self.file_path.clone() {
            Some(path) => self.save_to(path),
            None => self.save_as(),
        }
    }

    pub(crate) fn save_to(&mut self, path: PathBuf) {
        match serde_json::to_string_pretty(&self.scene)
            .map_err(|e| e.to_string())
            .and_then(|json| fs::write(&path, json).map_err(|e| e.to_string()))
        {
            Ok(()) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.file_path = Some(path);
                self.toast(format!("Salvo em {name}"));
            }
            Err(error) => self.toast(format!("Falha ao salvar: {error}")),
        }
    }

    pub(crate) fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Abrir")
            .add_filter("Desenhos", &["mago", "rea"])
            .pick_file()
        else {
            return;
        };
        match fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|d| parse_scene(&d)) {
            Ok(scene) => {
                self.snapshot();
                self.scene = scene;
                self.file_path = Some(path);
                self.selected.clear();
                self.zoom_to_fit(false);
            }
            Err(error) => self.toast(format!("Não foi possível abrir: {error}")),
        }
    }

    /// Inserir imagem: PNG/JPEG becomes a rectangle carrying the picture, at most 600px wide,
    /// in the middle of the view.
    pub(crate) fn insert_image(&mut self) {
        if let Some(paths) = rfd::FileDialog::new().set_title("Inserir imagem").add_filter("Imagens", &IMAGE_EXTENSIONS).pick_files() {
            self.insert_image_files(paths, self.to_world(self.canvas_rect.center()));
        }
    }

    /// Images go side by side around `at`, as tall as the natural size allows: at most half
    /// the view and never more than the view minus 120px (initializeImage on the web).
    pub(crate) fn insert_image_files(&mut self, paths: Vec<PathBuf>, at: Pos2) {
        let max_height = (self.canvas_rect.height() - 120.0).max(160.0).min((self.canvas_rect.height() * 0.5).floor() / self.zoom);
        let mut elements = Vec::new();
        for path in paths {
            let loaded = fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
                let format = image::guess_format(&bytes).map_err(|e| e.to_string())?;
                let size = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
                Ok((format!("data:{};base64,{}", format.to_mime_type(), base64(&bytes)), Vec2::new(size.width() as f32, size.height() as f32)))
            });
            match loaded {
                Ok((url, natural)) => {
                    let height = natural.y.min(max_height);
                    let size = Vec2::new(height * natural.x / natural.y, height);
                    let mut element = self.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: size.x, height: size.y });
                    element.image = Some(url);
                    element.style.fill = None;
                    elements.push(element);
                }
                Err(error) => self.toast(format!("Não foi possível inserir a imagem: {error}")),
            }
        }
        // one row, 50px apart on screen, centered on `at`
        let gap = 50.0 / self.zoom;
        let row = elements.iter().map(|e| element_bounds(e).width()).sum::<f32>() + gap * elements.len().saturating_sub(1) as f32;
        let mut x = at.x - row / 2.0;
        for element in &mut elements {
            let b = element_bounds(element);
            element.move_by(Vec2::new(x, at.y - b.height() / 2.0));
            x += b.width() + gap;
        }
        self.insert_elements(elements);
    }

    pub(crate) fn export(&mut self, png: bool) {
        let (ext, title) = if png { ("png", "Imagem PNG") } else { ("svg", "Imagem SVG") };
        let Some(path) = rfd::FileDialog::new()
            .set_title("Exportar imagem")
            .add_filter(title, &[ext])
            .set_file_name(format!("desenho.{ext}"))
            .save_file()
        else {
            return;
        };
        let background = self.export_background.then_some(self.scene.background);
        let svg = scene_svg(&self.scene, background);
        let bytes = if png { render_svg_png(&svg, self.export_scale) } else { Ok(svg.into_bytes()) };
        match bytes.and_then(|b| fs::write(&path, b).map_err(|e| e.to_string())) {
            Ok(()) => {
                self.export_open = false;
                self.toast("Imagem exportada");
            }
            Err(error) => self.toast(format!("Falha ao exportar: {error}")),
        }
    }

    pub(crate) fn write_autosave(&mut self) {
        let Some(path) = self.autosave.clone() else {
            self.dirty = false;
            return;
        };
        if modified(&path) != self.synced_stamp && self.synced_stamp.is_some() {
            // an AI (MCP) wrote meanwhile: merge first, write on a later frame
            self.sync_from_file();
            return;
        }
        if let Ok(stamp) = write_scene_file(&path, &self.scene) {
            self.synced = self.scene.clone();
            self.synced_stamp = stamp;
            self.dirty = false;
        }
    }

    /// Picks up drawings made through the MCP server while the app is open.
    pub(crate) fn sync_from_file(&mut self) {
        let Some(path) = self.autosave.clone() else {
            return;
        };
        let stamp = modified(&path);
        if stamp == self.synced_stamp {
            return;
        }
        let Some((external, stamp)) = read_scene_file(&path) else {
            return;
        };
        // ponytail: an MCP write landing inside our own read-modify-write window (ms) can still lose one side
        let merged = merge_scenes(&self.synced, &self.scene, &external);
        let known: std::collections::HashSet<&str> = self.synced.elements.iter().chain(&self.scene.elements).map(|e| e.id.as_str()).collect();
        let fresh: Vec<String> = external.elements.iter().filter(|e| !known.contains(e.id.as_str())).map(|e| e.id.clone()).collect();
        if let Some(name) = &external.agent {
            if !fresh.is_empty() {
                if self.reveal.is_empty() {
                    self.reveal_since = self.now;
                }
                self.reveal.extend(fresh.iter().cloned());
                let pos = self.agent.as_ref().map_or(self.to_world(self.canvas_rect.center()), |a| a.1);
                self.agent = Some((name.clone(), pos, self.now));
            }
        }
        if merged != self.scene {
            self.snapshot(); // Cmd+Z undoes what the AI did
            self.keeping_selection(|app| app.scene = merged);
            self.editing = None;
            self.line_editor = None;
            self.multi = None;
        }
        if let Some(terminal) = &external.agent_terminal {
            self.tie_to_terminal(terminal, &fresh);
        }
        self.dirty = self.scene != external;
        self.synced = external;
        self.synced_stamp = stamp;
    }
}
