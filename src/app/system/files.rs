//! Open, save, export and image insertion.

use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn save_as(&mut self) {
        let path = rfd::FileDialog::new().set_title("Salvar como").add_filter("Desenho MagoAgent (.mago)", &["mago"]).set_file_name("desenho.mago").save_file();

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
            .map_err(|error| error.to_string())
            .and_then(|json| write_atomic(&path, json).map_err(|error| error.to_string()))
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
        let Some(path) = rfd::FileDialog::new().set_title("Abrir").add_filter("Desenhos", &["mago", "rea"]).pick_file() else {
            return;
        };

        match fs::read_to_string(&path).map_err(|error| error.to_string()).and_then(|text| parse_scene(&text).map_err(|error| error.to_string())) {
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
        if let Some(paths) =
            rfd::FileDialog::new().set_title("Inserir imagem").add_filter("Imagens", &[IMAGE_EXTENSIONS.as_slice(), &["svg"]].concat()).pick_files()
        {
            self.insert_image_files(paths, self.to_world(self.canvas_rect.center()));
        }
    }

    /// Images go side by side around `at`, as tall as the natural size allows: at most half
    /// the view and never more than the view minus 120px (initializeImage on the web).
    pub(crate) fn insert_image_files(&mut self, paths: Vec<PathBuf>, at: Pos2) {
        let max_height = (self.canvas_rect.height() - 120.0).max(160.0).min((self.canvas_rect.height() * 0.5).floor() / self.zoom);
        let mut elements = Vec::new();

        for path in paths {
            let loaded = fs::read(&path).map_err(|error| error.to_string()).and_then(|bytes| {
                // a drawing of lines (SVG) goes in as the picture it draws, at twice its size
                let bytes = if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("svg")) {
                    render_svg_png(&String::from_utf8_lossy(&bytes), 2.0)?
                } else {
                    bytes
                };

                let format = image::guess_format(&bytes).map_err(|error| error.to_string())?;
                let size = image::load_from_memory(&bytes).map_err(|error| error.to_string())?;

                Ok((format!("data:{};base64,{}", format.to_mime_type(), base64(&bytes)), Vec2::new(size.width() as f32, size.height() as f32)))
            });

            match loaded {
                Ok((url, natural)) => {
                    let height = natural.y.min(max_height);
                    let size = Vec2::new(height * natural.x / natural.y, height);
                    let mut element = self.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: size.x, height: size.y });

                    element.image = Some(url.into());
                    element.style.fill = None;
                    elements.push(element);
                }
                Err(error) => self.toast(format!("Não foi possível inserir a imagem: {error}")),
            }
        }

        // one row, 50px apart on screen, centered on `at`
        let gap = 50.0 / self.zoom;
        let row = elements.iter().map(|element| element_bounds(element).width()).sum::<f32>() + gap * elements.len().saturating_sub(1) as f32;
        let mut x = at.x - row / 2.0;

        for element in &mut elements {
            let bounds = element_bounds(element);

            element.move_by(Vec2::new(x, at.y - bounds.height() / 2.0));
            x += bounds.width() + gap;
        }

        self.insert_elements(elements);
    }

    pub(crate) fn export(&mut self, png: bool) {
        let (extension, title) = if png { ("png", "Imagem PNG") } else { ("svg", "Imagem SVG") };

        let Some(path) =
            rfd::FileDialog::new().set_title("Exportar imagem").add_filter(title, &[extension]).set_file_name(format!("desenho.{extension}")).save_file()
        else {
            return;
        };

        let background = self.export_background.then_some(self.scene.background);
        let svg = scene_svg(&self.scene, background);
        let bytes = if png { render_svg_png(&svg, self.export_scale) } else { Ok(svg.into_bytes()) };

        match bytes.and_then(|data| fs::write(&path, data).map_err(|error| error.to_string())) {
            Ok(()) => {
                self.export_open = false;
                self.toast("Imagem exportada");
            }
            Err(error) => self.toast(format!("Falha ao exportar: {error}")),
        }
    }

    /// A write in flight on the writer thread (the drawing on disk is not yet what `synced` says).
    pub(crate) fn autosave_busy(&self) -> bool {
        self.autosave_writer.as_ref().is_some_and(|writer| writer.busy)
    }

    /// Takes the answer of the write in flight (waiting for it when `wait`): its scene is what is on disk now.
    pub(crate) fn finish_autosave(&mut self, wait: bool) {
        let Some(writer) = self.autosave_writer.as_mut().filter(|writer| writer.busy) else { return };
        let answer = if wait { writer.done.recv().map_err(|_| std::sync::mpsc::TryRecvError::Disconnected) } else { writer.done.try_recv() };

        let (path, scene, result) = match answer {
            Ok(done) => done,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            // the thread is gone: nothing is in flight any more, and the write is asked again
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.autosave_writer = None;
                self.dirty = true;

                return;
            }
        };

        writer.busy = false;

        match result {
            Ok(stamp) if self.autosave.as_ref() == Some(&path) => (self.synced, self.synced_stamp) = (scene, stamp),
            Ok(_) => {}
            Err(_) => self.dirty = true,
        }
    }

    /// Like `write_autosave`, but the serializing and the disk go to the writer thread, one write at a time and in order;
    /// the next frame picks up its answer. A change made meanwhile stays dirty for the next write.
    pub(crate) fn write_autosave_in_background(&mut self) {
        self.finish_autosave(false);

        if self.autosave_busy() {
            return;
        }

        let Some(path) = self.autosave.clone() else {
            self.dirty = false;

            return;
        };

        if modified(&path).is_some_and(|stamp| Some(stamp) != self.synced_stamp) {
            self.sync_from_file();

            return;
        }

        let writer = self.autosave_writer.get_or_insert_with(AutosaveWriter::start);

        if writer.jobs.send((path, self.scene.clone())).is_ok() {
            writer.busy = true;
            self.dirty = false;
        } else {
            self.autosave_writer = None;
        }
    }

    /// Writes now, after the write in flight (so the last one on disk is always the last one asked).
    pub(crate) fn write_autosave(&mut self) {
        self.finish_autosave(true);

        let Some(path) = self.autosave.clone() else {
            self.dirty = false;

            return;
        };

        // a file that is gone just gets written again; one this window can't read never is
        if modified(&path).is_some_and(|stamp| Some(stamp) != self.synced_stamp) {
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

    /// The preferences on disk once they stopped changing: 0.5 s after the first change not written (2 s while the
    /// pointer is held, a window or a slider being dragged), on the writer's thread. Before, a drag wrote them (fsync
    /// and the keychain) every frame.
    pub(crate) fn save_prefs_when_due(&mut self, ctx: &Context) {
        if self.prefs == self.saved_prefs {
            self.prefs_changed_at = None;

            return;
        }

        let now = ctx.input(|input| input.time);
        let since = *self.prefs_changed_at.get_or_insert(now);
        let due = if ctx.input(|input| input.pointer.any_down()) { 2.0 } else { 0.5 };

        if now - since >= due {
            let _ = self.save_prefs(false);
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(due - (now - since)));
        }
    }

    /// Writes the preferences beside this drawing (a test with a drawing of its own never writes the person's), on
    /// the writer's thread; with `wait`, once they are on disk.
    pub(crate) fn save_prefs(&mut self, wait: bool) -> Result<(), String> {
        let written = match self.autosave.as_ref().map(|autosave| autosave.with_file_name("preferences.json")) {
            Some(path) => write_prefs(path, self.prefs.clone(), self.saved_prefs.clone(), wait),
            None => Ok(()),
        };

        self.saved_prefs = self.prefs.clone();
        self.prefs_changed_at = None;
        written
    }

    pub(crate) fn save_on_exit(&mut self) {
        // the preferences not yet written, and the ones on their way
        if self.prefs != self.saved_prefs {
            let _ = self.save_prefs(true);
        } else {
            prefs_written();
        }

        self.finish_autosave(true);
        self.finish_text();

        // what was typed in a file tree's editor pane
        for editor in self.filetrees.editors.values_mut() {
            editor.save_all();
        }

        // twice: the first one may only merge what an AI wrote meanwhile
        for _ in 0..2 {
            if self.dirty {
                self.write_autosave();
            }
        }
    }

    /// The live drawing at startup. An unreadable one is set aside and a fresh one starts. One
    /// that can't be used (newer, locked) stays as it is: with no stamp of it, `write_autosave`
    /// only ever merges into it, so this window never writes over it.
    pub(crate) fn load_autosave(&mut self) {
        let Some(path) = self.autosave.clone() else {
            return;
        };

        match read_scene_file(&path) {
            Ok((scene, stamp)) => {
                self.scene = scene.clone();
                self.synced = scene;
                self.synced_stamp = stamp;
            }
            Err(SceneError::Missing) => {}
            Err(SceneError::Unreadable(error)) => match set_aside(&path) {
                Ok(kept) => self.toast(format!("O desenho salvo estava ilegível ({error}). Ele ficou na mesma pasta, como {}.", kept_name(&kept))),
                // the MCP set it aside first
                Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {}
                Err(io_error) => self.toast(format!("O desenho salvo está ilegível e não pôde ser guardado ({io_error}): esta janela não grava por cima dele")),
            },
            Err(SceneError::Newer(scene)) => {
                self.scene = scene.unwrap_or_default();
                self.toast("Desenho salvo por uma versão mais nova do MagoAgent: aberto só para leitura");
            }
            Err(error) => self.toast(format!("O desenho salvo não abriu ({error}): esta janela não grava por cima dele")),
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

        let (external, stamp) = match read_scene_file(&path) {
            Ok(read) => read,
            // gone (deleted, set aside by the MCP): this window writes its drawing back
            Err(SceneError::Missing) => {
                // nothing on disk is known now: should the MCP make a file first, the merge keeps
                // both drawings instead of taking ours as deleted there
                self.synced = Scene::default();
                self.synced_stamp = None;
                self.dirty = true;

                return;
            }
            // ponytail: a file left unreadable or newer while the app is open is never written
            // over, so edits made since live only in this window; upgrade: set it aside once it
            // stays bad for a few polls, and say so
            Err(_) => return,
        };

        // ponytail: an MCP write landing inside our own read-modify-write window (ms) can still lose one side
        let merged = merge_scenes(&self.synced, &self.scene, &external);
        let known: std::collections::HashSet<&str> = self.synced.elements.iter().chain(&self.scene.elements).map(|element| element.id.as_str()).collect();
        let fresh: Vec<String> = external.elements.iter().filter(|element| !known.contains(element.id.as_str())).map(|element| element.id.clone()).collect();

        if let Some(name) = &external.agent
            && !fresh.is_empty()
        {
            if self.reveal.is_empty() {
                self.reveal_since = self.now;
            }

            self.reveal.extend(fresh.iter().cloned());
            let pos = self.agent.as_ref().map_or(self.to_world(self.canvas_rect.center()), |agent| agent.1);

            self.agent = Some((name.clone(), pos, self.now));
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

/// The writer thread of the autosave: it takes (path, drawing) jobs one by one and answers with the drawing it wrote.
pub(crate) struct AutosaveWriter {
    jobs: std::sync::mpsc::Sender<(PathBuf, Scene)>,
    done: std::sync::mpsc::Receiver<(PathBuf, Scene, std::io::Result<Option<SystemTime>>)>,
    busy: bool,
}

impl AutosaveWriter {
    fn start() -> Self {
        let (jobs, job_queue) = std::sync::mpsc::channel::<(PathBuf, Scene)>();
        let (answer, done) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            for (path, scene) in job_queue {
                let result = write_scene_file(&path, &scene);

                if answer.send((path, scene, result)).is_err() {
                    break;
                }
            }
        });

        Self { jobs, done, busy: false }
    }
}
