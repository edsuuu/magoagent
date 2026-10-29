//! Workspaces: each one a drawing of its own beside the main one, switched in place. The one left
//! stays whole in memory this session, so its shells, IDEs and pages go on behind the other until it
//! is unloaded (Descarregar).

use crate::prelude::*;

/// A workspace left this session: its drawing and what goes with it (the camera is in `Space`).
#[derive(Default)]
pub(crate) struct Parked {
    pub(crate) scene: Scene,
    synced: Scene,
    synced_stamp: Option<SystemTime>,
    undo: Vec<Scene>,
    redo: Vec<Scene>,
    dirty: bool,
    /// The file it was opened from or saved to: Ctrl+S in another workspace must not write there.
    file_path: Option<PathBuf>,
}

#[derive(Clone)]
pub(crate) enum SpaceDialog {
    /// Novo (no id) or Editar Espaço de Trabalho.
    Edit { id: Option<String>, name: String, color: Option<[u8; 3]>, root: String },
    /// Apagar asks first.
    Delete { id: String, name: String },
    /// Novo Andar under `parent`; `busy` while git makes its worktree, `error` when it couldn't.
    Floor { parent: String, name: String, isolate: bool, branch: String, busy: bool, error: Option<String> },
}

impl DrawingApp {
    /// At startup, before the drawing loads: the workspaces beside the main drawing, and the
    /// active one's drawing and camera.
    pub(crate) fn load_spaces(&mut self) {
        let Some(main) = self.autosave.clone() else { return };
        // a workspace's own drawing (MAGOAGENT_SCENE in its terminals, opened by an app run
        // there) is no main drawing: the registry beside it is the other app's
        if main.file_name().is_some_and(|n| n.to_string_lossy().starts_with("space-")) {
            return;
        }
        let bad;
        (self.spaces, bad) = read_spaces(&main);
        if let Some(kept) = bad {
            self.toast(format!("Não foi possível ler workspaces.json: ficou guardado em {} e os espaços voltaram ao padrão.", kept.display()));
        }
        self.autosave = Some(space_scene(&main, &self.spaces.active));
        self.spaces_home = Some(main);
        self.apply_view();
    }

    /// The elements of every workspace open this session: what lives outside the drawing (shells,
    /// IDEs, pages) goes on while its workspace is hidden.
    /// Taken away by an undo, and a redo can bring it back, in any workspace open this session.
    pub(crate) fn in_redo(&self, id: &str) -> bool {
        self.redo.iter().chain(self.parked.values().flat_map(|p| &p.redo)).any(|s| s.elements.iter().any(|e| e.id == id))
    }

    pub(crate) fn all_elements(&self) -> impl Iterator<Item = &Element> {
        self.scene.elements.iter().chain(self.parked.values().flat_map(|p| &p.scene.elements))
    }

    /// An element wherever its workspace is, with the dirty flag of the drawing that keeps it.
    #[cfg_attr(target_os = "linux", allow(dead_code))] // only portals use it
    pub(crate) fn element_anywhere_mut(&mut self, id: &str) -> Option<(&mut Element, &mut bool)> {
        if let Some(e) = self.scene.elements.iter_mut().find(|e| e.id == id) {
            return Some((e, &mut self.dirty));
        }
        self.parked.values_mut().find_map(|p| Some((p.scene.elements.iter_mut().find(|e| e.id == id)?, &mut p.dirty)))
    }

    /// Whether `id` sits in a workspace not on screen.
    pub(crate) fn hidden(&self, id: &str) -> bool {
        !self.scene.elements.iter().any(|e| e.id == id) && self.parked.values().any(|p| p.scene.elements.iter().any(|e| e.id == id))
    }

    /// The hidden workspace that keeps `id`; None when it's on screen (or nowhere).
    pub(crate) fn space_holding(&self, id: &str) -> Option<String> {
        self.parked.iter().find(|(_, p)| p.scene.elements.iter().any(|e| e.id == id)).map(|(space, _)| space.clone())
    }

    /// The drawing an element's workspace keeps: where the MCP in a terminal draws.
    pub(crate) fn scene_of(&self, id: &str) -> Option<PathBuf> {
        let space = self.space_holding(id).unwrap_or_else(|| self.spaces.active.clone());
        Some(space_scene(self.spaces_home.as_ref()?, &space))
    }

    fn active_space(&self) -> Option<&Space> {
        self.spaces.spaces.iter().find(|s| s.id == self.spaces.active)
    }

    /// Where new terminals and IDEs start: the workspace's folder, else the open drawing's.
    pub(crate) fn new_folder(&self) -> String {
        match self.active_space().map(|s| s.root.trim()).filter(|r| !r.is_empty()) {
            Some(root) => root.to_string(),
            None => self.file_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        }
    }

    fn apply_view(&mut self) {
        let [x, y, zoom] = self.active_space().and_then(|s| s.view).unwrap_or([0.0, 0.0, 1.0]);
        (self.pan, self.zoom, self.glide) = (Vec2::new(x, y), zoom, None);
    }

    /// The hidden workspaces that changed while hidden (a page of theirs navigated) go to their files
    /// too, merged with what their agents wrote meanwhile.
    pub(crate) fn save_parked(&mut self) {
        let ids: Vec<String> = self.parked.keys().cloned().collect();
        for id in ids {
            self.save_parked_space(&id, false);
        }
    }

    /// Writes hidden workspace `id` if it changed (`keep`: always, with its terminals and portals
    /// unloaded): whether its file now has it all. Not when there is no file, or one this window
    /// can't read (that is never written over).
    fn save_parked_space(&mut self, id: &str, keep: bool) -> bool {
        let Some(main) = self.spaces_home.clone() else { return false };
        let Some(p) = self.parked.get_mut(id) else { return false };
        if !p.dirty && !keep {
            return true;
        }
        let path = space_scene(&main, id);
        if modified(&path) != p.synced_stamp {
            match read_scene_file(&path) {
                Ok((external, _)) => p.scene = merge_scenes(&p.synced, &p.scene, &external),
                Err(SceneError::Missing) => {}
                Err(_) => return false,
            }
        }
        // what its agents drew meanwhile is kept unloaded too; memory changes once the file has it
        let mut scene = p.scene.clone();
        for e in scene.elements.iter_mut().filter(|_| keep) {
            e.set_unloaded(true);
        }
        let Ok(stamp) = write_scene_file(&path, &scene) else { return false };
        (p.scene, p.synced, p.synced_stamp, p.dirty) = (scene.clone(), scene, stamp, false);
        true
    }

    /// Descarregar a hidden workspace: its drawing is written and let go, so its shells, pages and
    /// IDEs end on the next frame. Its undo goes with it; showing it again reads the file. Manter
    /// Descarregado (`keep`) writes its terminals and portals unloaded, so showing it again starts
    /// none of them until Recarregar.
    pub(crate) fn unload_space(&mut self, id: &str, keep: bool) {
        if !self.parked.contains_key(id) {
            return;
        }
        if self.save_parked_space(id, keep) {
            self.parked.remove(id);
        } else {
            self.toast("Não foi possível gravar o desenho desse espaço: ele continua carregado.");
        }
    }

    /// Recarregar Espaço de Trabalho / Andar: shows it, with what was kept unloaded started again
    /// (one undo step).
    // ponytail: it switches to the workspace; reloading one in the background would need its own sync
    pub(crate) fn reload_space(&mut self, id: &str) {
        self.switch_space(id);
        if self.spaces.active == id && self.scene.elements.iter().any(|e| e.unloaded()) {
            self.snapshot();
            for e in &mut self.scene.elements {
                e.set_unloaded(false);
            }
        }
    }

    /// The camera goes into the registry, which is written once it says more than the default:
    /// another space, an edited main one, or a file already there (it must not go stale).
    pub(crate) fn keep_spaces(&mut self) {
        let (pan, zoom, active) = (self.pan, self.zoom, self.spaces.active.clone());
        if let Some(space) = self.spaces.spaces.iter_mut().find(|s| s.id == active) {
            space.view = Some([pan.x, pan.y, zoom]);
        }
        let edited = |main: &std::path::Path| {
            let first = Space { view: None, ..self.spaces.spaces[0].clone() };
            self.spaces.spaces.len() > 1 || first != Spaces::default().spaces[0] || main.with_file_name("workspaces.json").exists()
        };
        if let Some(Err(error)) = self.spaces_home.as_ref().filter(|main| edited(main)).map(|main| write_spaces(main, &self.spaces)) {
            self.toast(format!("Não foi possível gravar os espaços de trabalho: {error}"));
        }
    }

    /// Shows another workspace: this one's drawing is saved and kept, with its undo, and the
    /// other comes back as it was left (read from disk the first time).
    pub(crate) fn switch_space(&mut self, id: &str) {
        if id == self.spaces.active || !self.spaces.spaces.iter().any(|s| s.id == id) {
            return;
        }
        // a drag holds indices into this drawing; Design runs in one of its pages
        if self.in_gesture() {
            self.cancel_gesture();
        }
        self.design_exit();
        self.set_elevated(None);
        self.save_on_exit();
        self.keep_spaces();
        let left = Parked {
            scene: std::mem::take(&mut self.scene),
            synced: std::mem::take(&mut self.synced),
            synced_stamp: self.synced_stamp.take(),
            undo: std::mem::take(&mut self.undo),
            redo: std::mem::take(&mut self.redo),
            dirty: std::mem::take(&mut self.dirty),
            file_path: self.file_path.take(),
        };
        self.parked.insert(std::mem::replace(&mut self.spaces.active, id.to_string()), left);
        self.autosave = self.spaces_home.as_ref().map(|main| space_scene(main, id));
        match self.parked.remove(id) {
            Some(back) => {
                (self.scene, self.synced, self.synced_stamp) = (back.scene, back.synced, back.synced_stamp);
                (self.undo, self.redo, self.dirty, self.file_path) = (back.undo, back.redo, back.dirty, back.file_path);
            }
            None => self.load_autosave(),
        }
        // what pointed into the other drawing
        self.selected.clear();
        self.selected_point = None;
        (self.editing, self.line_editor, self.multi) = (None, None, None);
        (self.terminal_focus, self.ide_focus, self.card_focus) = (None, None, None);
        (self.context, self.rename, self.links_popover, self.connecting) = (None, None, None, None);
        // a prompt being written waits for its terminal to show again
        if let Some((id, text)) = self.composer.take().filter(|(_, text)| !text.trim().is_empty()) {
            self.drafts.insert(id, text);
        }
        (self.portal_url_edit, self.agent) = (None, None);
        self.reveal.clear();
        self.apply_view();
        // what its terminals' agents drew meanwhile
        self.sync_from_file();
        self.keep_spaces();
    }

    /// Ctrl+↑ / Ctrl+↓: the workspace before or after this one in the list, round.
    pub(crate) fn step_space(&mut self, by: isize) {
        let n = self.spaces.spaces.len() as isize;
        let at = self.spaces.spaces.iter().position(|s| s.id == self.spaces.active).unwrap_or(0) as isize;
        let next = self.spaces.spaces[(at + by).rem_euclid(n) as usize].id.clone();
        self.switch_space(&next);
    }

    /// Novo Espaço de Trabalho: an empty drawing, shown at once.
    pub(crate) fn create_space(&mut self, name: &str, color: Option<[u8; 3]>, root: &str) {
        let id = new_id();
        self.spaces.spaces.push(Space { id: id.clone(), name: name.trim().into(), color, root: root.trim().into(), ..Default::default() });
        self.switch_space(&id);
    }

    /// Apagar: its shells and IDEs end, and a floor's worktree goes. The main one stays, and so
    /// does a workspace with floors (they go first) or a floor with uncommitted changes.
    // ponytail: its drawing file stays beside the main one, so nothing drawn is lost; upgrade:
    // send it to the trash once the trash is shared code
    pub(crate) fn delete_space(&mut self, id: &str) {
        if id == MAIN_SPACE || self.spaces.spaces.iter().any(|s| s.parent.as_deref() == Some(id)) {
            return;
        }
        // what its IDEs hold unsaved lands in the checkout now, where the check below sees it
        let scene = if self.spaces.active == id { Some(&self.scene) } else { self.parked.get(id).map(|p| &p.scene) };
        for e in scene.into_iter().flat_map(|s| &s.elements) {
            if let Some(ide) = self.ides.get_mut(&e.id) {
                ide.save_all();
            }
        }
        if let Some(why) = self.floor_kept(id) {
            self.toast(why);
            return;
        }
        let Some(space) = self.spaces.spaces.iter().find(|s| s.id == id).cloned() else { return };
        if self.spaces.active == id {
            // a floor whose workspace went meanwhile goes back to the main one
            let parent = space.parent.as_deref().filter(|p| self.spaces.spaces.iter().any(|s| s.id == *p));
            self.switch_space(parent.unwrap_or(MAIN_SPACE));
        }
        if let Some(gone) = self.parked.remove(id) {
            for scene in std::iter::once(&gone.scene).chain(&gone.redo) {
                self.end_sessions(scene);
            }
        }
        self.spaces.spaces.retain(|s| s.id != id);
        self.keep_spaces();
        if let Some(worktree) = space.worktree {
            self.drop_worktree(worktree);
        }
    }

    /// The terminals a workspace has running (not unloaded), when its drawing is open this session.
    fn space_terminals(&self, id: &str) -> Option<usize> {
        let scene = if id == self.spaces.active { Some(&self.scene) } else { self.parked.get(id).map(|p| &p.scene) };
        Some(scene?.elements.iter().filter(|e| e.terminal.as_ref().is_some_and(|t| !t.unloaded)).count())
    }

    /// The button beside the minimap's, and the list it opens above them.
    pub(crate) fn spaces_panel(&mut self, ctx: &Context, c: &UiColors) {
        self.space_jobs(ctx);
        // hidden with the other corner buttons while a window has the focus
        if self.focused_window().is_some() {
            self.spaces_open = false;
            return;
        }
        let button = egui::Area::new(egui::Id::unique("spaces_button"))
            .anchor(egui::Align2::RIGHT_BOTTOM, (-16.0 - 36.0 - 8.0, -16.0))
            .enabled(!self.prefs.zen)
            .show(ctx, |ui| {
                group_frame(c)
                    .show(ui, |ui| icon_button(ui, 36.0, self.spaces_open, Color32::TRANSPARENT, Icon::Sidebar, c.text, c).on_hover_text("Espaços de Trabalho"))
                    .inner
            })
            .inner;
        if button.clicked() {
            self.spaces_open = !self.spaces_open;
        }
        if !self.spaces_open {
            return;
        }
        let (mut go, mut menu, mut new, mut renamed) = (None, None, false, None);
        let rows: Vec<(Space, Option<usize>)> = self.spaces.spaces.iter().map(|s| (s.clone(), self.space_terminals(&s.id).filter(|n| *n > 0))).collect();
        let parents: std::collections::HashSet<String> = self.spaces.spaces.iter().filter_map(|s| s.parent.clone()).collect();
        // hidden and still running: Descarregar; let go, or on screen with something unloaded: Recarregar
        let alive: std::collections::HashSet<String> = self.parked.keys().cloned().collect();
        let active_unloaded = self.scene.elements.iter().any(|e| e.unloaded());
        let (active, island, rename) = (&self.spaces.active, self.island(), &mut self.space_rename);
        egui::Area::new(egui::Id::unique("spaces_panel")).order(egui::Order::Foreground).anchor(egui::Align2::RIGHT_BOTTOM, (-16.0, -16.0 - 36.0 - 8.0)).show(
            ctx,
            |ui| {
                island.corner_radius(12).inner_margin(10).show(ui, |ui| {
                    ui.set_width(260.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Espaços de Trabalho").size(13.0).strong().color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            new = icon_button(ui, 24.0, false, Color32::TRANSPARENT, Icon::Plus, c.text, c).on_hover_text("Novo espaço de trabalho").clicked();
                        });
                    });
                    ui.add_space(4.0);
                    for (space, count) in &rows {
                        let (rect, response) = ui.allocate_exact_size(Vec2::new(260.0, 30.0), Sense::click());
                        let on = space.id == *active;
                        let fill = if on {
                            c.selected
                        } else if response.hovered() {
                            c.surface_low
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(rect, 7, fill);
                        // a floor sits under its workspace
                        let indent = if space.parent.is_some() { 16.0 } else { 0.0 };
                        let dot = rect.left_center() + Vec2::new(14.0 + indent, 0.0);
                        match space.color {
                            Some([r, g, b]) => ui.painter().circle_filled(dot, 5.0, Color32::from_rgb(r, g, b)),
                            None => ui.painter().circle_stroke(dot, 4.5, Stroke::new(1.0, c.muted)),
                        };
                        if let Some(n) = *count {
                            let label = if n == 1 { "1 terminal".to_string() } else { format!("{n} terminais") };
                            ui.painter().text(
                                rect.right_center() - Vec2::new(10.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                label,
                                FontId::proportional(11.0),
                                c.muted,
                            );
                        }
                        let name_rect =
                            Rect::from_min_max(rect.min + Vec2::new(28.0 + indent, 0.0), rect.max - Vec2::new(if count.is_some() { 80.0 } else { 10.0 }, 0.0));
                        match rename {
                            Some((id, text)) if *id == space.id => {
                                let edit = ui.put(
                                    name_rect.shrink2(Vec2::new(0.0, 3.0)),
                                    egui::TextEdit::singleline(text).id(egui::Id::unique("space_rename")).font(FontId::proportional(13.0)),
                                );
                                if edit.lost_focus() {
                                    let keep = !ui.input(|i| i.key_pressed(Key::Escape)) && !text.trim().is_empty();
                                    renamed = Some(keep.then(|| (id.clone(), text.trim().to_string())));
                                }
                            }
                            _ => {
                                let weight = if on { c.text } else { c.text.gamma_multiply(0.85) };
                                ui.painter().with_clip_rect(name_rect).text(
                                    name_rect.left_center(),
                                    egui::Align2::LEFT_CENTER,
                                    &space.name,
                                    FontId::proportional(13.0),
                                    weight,
                                );
                            }
                        }
                        let response = match &space.worktree {
                            Some(w) => response.on_hover_text(format!("Branch {} · {}", w.branch, w.path)),
                            None => response,
                        };
                        if response.clicked() {
                            go = Some(space.id.clone());
                        }
                        response.context_menu(|ui| {
                            ui.set_min_width(200.0);
                            let folder = !space.root.trim().is_empty();
                            if space.parent.is_none() && ui.button("Novo Andar…").clicked() {
                                menu = Some(("floor", space.id.clone()));
                            }
                            if ui.button("Renomear").clicked() {
                                menu = Some(("rename", space.id.clone()));
                            }
                            if ui.button(if space.parent.is_some() { "Editar Andar" } else { "Editar Espaço de Trabalho" }).clicked() {
                                menu = Some(("edit", space.id.clone()));
                            }
                            ui.separator();
                            if ui.add_enabled(folder, egui::Button::new("Copiar Caminho")).clicked() {
                                menu = Some(("copy", space.id.clone()));
                            }
                            if ui.add_enabled(folder, egui::Button::new(crate::ide::REVEAL)).clicked() {
                                menu = Some(("reveal", space.id.clone()));
                            }
                            ui.separator();
                            if alive.contains(&space.id) {
                                if ui.button("Descarregar").clicked() {
                                    menu = Some(("unload", space.id.clone()));
                                }
                                if ui.button("Manter Descarregado").clicked() {
                                    menu = Some(("keep_unloaded", space.id.clone()));
                                }
                                ui.separator();
                            } else if space.id != *active || active_unloaded {
                                if ui.button(if space.parent.is_some() { "Recarregar Andar" } else { "Recarregar Espaço de Trabalho" }).clicked() {
                                    menu = Some(("reload", space.id.clone()));
                                }
                                ui.separator();
                            }
                            let floors = parents.contains(&space.id);
                            let delete = ui.add_enabled(space.id != MAIN_SPACE && !floors, egui::Button::new("Apagar"));
                            if delete.on_disabled_hover_text(if floors { "Apague os andares dele primeiro" } else { "O espaço principal fica" }).clicked() {
                                menu = Some(("delete", space.id.clone()));
                            }
                        });
                    }
                });
            },
        );
        if new {
            self.space_dialog = Some(SpaceDialog::Edit { id: None, name: String::new(), color: None, root: String::new() });
        }
        match renamed {
            Some(Some((id, name))) => {
                if let Some(space) = self.spaces.spaces.iter_mut().find(|s| s.id == id) {
                    space.name = name;
                }
                self.space_rename = None;
                self.keep_spaces();
            }
            Some(None) => self.space_rename = None,
            None => {}
        }
        if let Some(id) = go.filter(|id| self.space_rename.as_ref().is_none_or(|(r, _)| r != id)) {
            self.switch_space(&id);
        }
        let Some((action, id)) = menu else { return };
        let Some(space) = self.spaces.spaces.iter().find(|s| s.id == id).cloned() else { return };
        match action {
            "rename" => {
                self.space_rename = Some((id, space.name));
                ctx.memory_mut(|m| m.request_focus(egui::Id::unique("space_rename")));
            }
            "edit" => self.space_dialog = Some(SpaceDialog::Edit { id: Some(id), name: space.name, color: space.color, root: space.root }),
            "copy" => ctx.copy_text(space.root),
            "reveal" => crate::app::portal::open_external(&space.root),
            "floor" => {
                self.space_dialog =
                    Some(SpaceDialog::Floor { parent: id, name: String::new(), isolate: false, branch: String::new(), busy: false, error: None })
            }
            "unload" => self.unload_space(&id, false),
            "keep_unloaded" => self.unload_space(&id, true),
            "reload" => self.reload_space(&id),
            "delete" => self.space_dialog = Some(SpaceDialog::Delete { id, name: space.name }),
            _ => {}
        }
    }

    pub(crate) fn space_dialog_ui(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.space_dialog.clone() else { return };
        let c = self.colors();
        let space = |id: &str| self.spaces.spaces.iter().find(|s| s.id == id);
        let (confirm, cancel) = match &mut dialog {
            SpaceDialog::Edit { id, name, color, root } => {
                let title = match id.as_deref().and_then(space) {
                    Some(s) if s.parent.is_some() => "Editar Andar",
                    Some(_) => "Editar Espaço de Trabalho",
                    None => "Novo Espaço de Trabalho",
                };
                dialog_ui(ctx, &c, title, if id.is_some() { "Salvar" } else { "Criar" }, |ui| {
                    dialog_field(ui, "Nome", name, "Nome do espaço de trabalho", DIALOG_WIDTH, &c);
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Cor").size(11.5).color(dialog_label(&c)));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        for pick in TERMINAL_COLORS {
                            let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                            match pick {
                                Some([r, g, b]) => ui.painter().circle_filled(rect.center(), 9.0, Color32::from_rgb(r, g, b)),
                                None => ui.painter().circle_stroke(rect.center(), 8.5, Stroke::new(1.0, c.muted)),
                            };
                            if *color == pick {
                                ui.painter().circle_stroke(rect.center(), 11.0, Stroke::new(1.5, c.text));
                            }
                            if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                *color = pick;
                            }
                        }
                    });
                    dialog_folder(ui, "Diretório de trabalho", root, "Opcional", "Diretório de trabalho", &c);
                    dialog_note(ui, "Terminais e IDEs novos deste espaço começam nessa pasta.", &c);
                })
            }
            SpaceDialog::Delete { id, name } => match space(id) {
                Some(floor) if floor.parent.is_some() => dialog_ui(ctx, &c, &format!("Apagar {name}"), "Apagar", |ui| {
                    dialog_note(ui, "Exclui o andar e tudo o que está nele.", &c);
                    if let Some(w) = &floor.worktree {
                        dialog_note(
                            ui,
                            &format!(
                                "A pasta dele, um worktree do git ({}), é removida. Com alterações sem commit, nada é excluído. A branch {} fica no repositório.",
                                w.path, w.branch
                            ),
                            &c,
                        );
                    }
                    dialog_note(ui, "Isso não pode ser desfeito.", &c);
                }),
                _ => dialog_ui(ctx, &c, &format!("Excluir “{name}”?"), "Excluir", |ui| {
                    dialog_note(ui, "O espaço de trabalho e o que está nele saem do MagoAgent. A pasta do seu projeto não é alterada.", &c);
                }),
            },
            SpaceDialog::Floor { name, isolate, branch, busy, error, .. } => {
                dialog_ui(ctx, &c, "Novo Andar", if *busy { "Criando…" } else { "Criar" }, |ui| {
                    dialog_note(ui, "Crie um andar para trabalhar em um ambiente isolado ou para organizar diferentes áreas neste espaço de trabalho.", &c);
                    dialog_field(ui, "Nome do andar", name, "Nome do andar", DIALOG_WIDTH, &c);
                    ui.add_space(8.0);
                    let info = "Cada andar trabalha na sua própria cópia do projeto, na sua própria branch, para que você e seus agentes trabalhem em vários recursos ao mesmo tempo sem atrapalhar uns aos outros.";
                    dialog_check(ui, isolate, "Isolamento de repositório", &c).on_hover_text(info);
                    if *isolate {
                        let hint = if name.trim().is_empty() { "Nome da nova branch".to_string() } else { floors::branch_from(name) };
                        dialog_field(ui, "Branch", branch, &hint, DIALOG_WIDTH, &c);
                        dialog_note(ui, "Crie uma nova branch ou use uma existente. O andar ganha um worktree do git, fora do repositório.", &c);
                    }
                    if let Some(error) = error {
                        ui.label(egui::RichText::new(error.as_str()).size(11.5).color(Color32::from_rgb(0xe0, 0x50, 0x50)));
                    }
                })
            }
        };
        if cancel {
            self.space_dialog = None;
            return;
        }
        // Criar needs a name: without one, Enter and the button do nothing (nor while git works)
        let named = !matches!(&dialog, SpaceDialog::Edit { name, .. } | SpaceDialog::Floor { name, .. } if name.trim().is_empty());
        let busy = matches!(&dialog, SpaceDialog::Floor { busy: true, .. });
        if !(confirm && named) || busy {
            self.space_dialog = Some(dialog);
            return;
        }
        self.space_dialog = None;
        match dialog {
            SpaceDialog::Edit { id: None, name, color, root } => self.create_space(&name, color, &root),
            SpaceDialog::Edit { id: Some(id), name, color, root } => {
                if let Some(space) = self.spaces.spaces.iter_mut().find(|s| s.id == id) {
                    (space.name, space.color, space.root) = (name.trim().into(), color, root.trim().into());
                }
                self.keep_spaces();
            }
            SpaceDialog::Delete { id, .. } => self.delete_space(&id),
            SpaceDialog::Floor { parent, name, isolate, branch, .. } => match self.create_floor(&parent, &name, isolate, &branch) {
                Ok(()) if isolate => self.space_dialog = Some(SpaceDialog::Floor { parent, name, isolate, branch, busy: true, error: None }),
                Ok(()) => {}
                Err(why) => self.space_dialog = Some(SpaceDialog::Floor { parent, name, isolate, branch, busy: false, error: Some(why) }),
            },
        }
    }
}
