//! Workspaces: each one a drawing of its own beside the main one, switched in place. The one left
//! stays whole in memory this session, so its shells, IDEs and pages go on behind the other.

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
}

#[derive(Clone)]
pub(crate) enum SpaceDialog {
    /// Novo (no id) or Editar Espaço de Trabalho.
    Edit { id: Option<String>, name: String, color: Option<[u8; 3]>, root: String },
    /// Apagar asks first.
    Delete { id: String, name: String },
}

impl DrawingApp {
    /// At startup, before the drawing loads: the workspaces beside the main drawing, and the
    /// active one's drawing and camera.
    pub(crate) fn load_spaces(&mut self) {
        let Some(main) = self.autosave.clone() else { return };
        self.spaces = read_spaces(&main);
        self.autosave = Some(space_scene(&main, &self.spaces.active));
        self.spaces_home = Some(main);
        self.apply_view();
    }

    /// The elements of every workspace open this session: what lives outside the drawing (shells,
    /// IDEs, pages) goes on while its workspace is hidden.
    pub(crate) fn all_elements(&self) -> impl Iterator<Item = &Element> {
        self.scene.elements.iter().chain(self.parked.values().flat_map(|p| &p.scene.elements))
    }

    /// Whether `id` sits in a workspace not on screen.
    pub(crate) fn hidden(&self, id: &str) -> bool {
        !self.scene.elements.iter().any(|e| e.id == id) && self.parked.values().any(|p| p.scene.elements.iter().any(|e| e.id == id))
    }

    /// The drawing an element's workspace keeps: where the MCP in a terminal draws.
    pub(crate) fn scene_of(&self, id: &str) -> Option<PathBuf> {
        let space = self.parked.iter().find(|(_, p)| p.scene.elements.iter().any(|e| e.id == id)).map_or(&self.spaces.active, |(space, _)| space);
        Some(space_scene(self.spaces_home.as_ref()?, space))
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
        (self.pan, self.zoom) = (Vec2::new(x, y), zoom);
    }

    /// The camera goes into the registry, which is written once there is more than the main
    /// space (until then nothing is).
    pub(crate) fn keep_spaces(&mut self) {
        let (pan, zoom, active) = (self.pan, self.zoom, self.spaces.active.clone());
        if let Some(space) = self.spaces.spaces.iter_mut().find(|s| s.id == active) {
            space.view = Some([pan.x, pan.y, zoom]);
        }
        let several = self.spaces.spaces.len() > 1;
        if let Some(Err(error)) = self.spaces_home.as_ref().filter(|_| several).map(|main| write_spaces(main, &self.spaces)) {
            self.toast(format!("Não foi possível gravar os espaços de trabalho: {error}"));
        }
    }

    /// Shows another workspace: this one's drawing is saved and kept, with its undo, and the
    /// other comes back as it was left (read from disk the first time).
    pub(crate) fn switch_space(&mut self, id: &str) {
        if id == self.spaces.active || !self.spaces.spaces.iter().any(|s| s.id == id) {
            return;
        }
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
        };
        self.parked.insert(std::mem::replace(&mut self.spaces.active, id.to_string()), left);
        self.autosave = self.spaces_home.as_ref().map(|main| space_scene(main, id));
        match self.parked.remove(id) {
            Some(back) => {
                (self.scene, self.synced, self.synced_stamp) = (back.scene, back.synced, back.synced_stamp);
                (self.undo, self.redo, self.dirty) = (back.undo, back.redo, back.dirty);
            }
            None => self.load_autosave(),
        }
        // what pointed into the other drawing
        self.selected.clear();
        self.selected_point = None;
        (self.editing, self.line_editor, self.multi) = (None, None, None);
        (self.terminal_focus, self.ide_focus, self.card_focus) = (None, None, None);
        (self.context, self.rename, self.links_popover, self.connecting) = (None, None, None, None);
        (self.composer, self.portal_url_edit, self.design, self.agent) = (None, None, None, None);
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
        self.spaces.spaces.push(Space { id: id.clone(), name: name.trim().into(), color, root: root.trim().into(), view: None });
        self.switch_space(&id);
    }

    /// Apagar: its shells and IDEs end. The main one stays.
    // ponytail: its drawing file stays beside the main one, so nothing drawn is lost; upgrade:
    // send it to the trash once the trash is shared code
    pub(crate) fn delete_space(&mut self, id: &str) {
        if id == MAIN_SPACE {
            return;
        }
        if self.spaces.active == id {
            self.switch_space(MAIN_SPACE);
        }
        self.parked.remove(id);
        self.spaces.spaces.retain(|s| s.id != id);
        self.keep_spaces();
    }

    /// The terminals a workspace has, when its drawing is open this session.
    fn space_terminals(&self, id: &str) -> Option<usize> {
        let scene = if id == self.spaces.active { Some(&self.scene) } else { self.parked.get(id).map(|p| &p.scene) };
        Some(scene?.elements.iter().filter(|e| e.terminal.is_some()).count())
    }

    /// The button beside Agentes and Chat, and the list it opens on the left.
    pub(crate) fn spaces_panel(&mut self, ctx: &Context, c: &UiColors) {
        let button = egui::Area::new(egui::Id::unique("spaces_button"))
            .fixed_pos((MENU_LEFT + 132.0, TOP_ROW + 4.0))
            .show(ctx, |ui| icon_button(ui, 36.0, self.spaces_open, c.surface_low, Icon::Sidebar, c.text, c).on_hover_text("Espaços de Trabalho"))
            .inner;
        if button.clicked() {
            self.spaces_open = !self.spaces_open;
        }
        if !self.spaces_open {
            return;
        }
        let (mut go, mut menu, mut new, mut renamed) = (None, None, false, None);
        let rows: Vec<(Space, Option<usize>)> = self.spaces.spaces.iter().map(|s| (s.clone(), self.space_terminals(&s.id).filter(|n| *n > 0))).collect();
        let (active, island, rename) = (&self.spaces.active, self.island(), &mut self.space_rename);
        egui::Area::new(egui::Id::unique("spaces_panel")).order(egui::Order::Foreground).fixed_pos((MENU_LEFT, TOP_ROW + 52.0)).show(ctx, |ui| {
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
                    let fill = if on { c.selected } else if response.hovered() { c.surface_low } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(rect, 7, fill);
                    let dot = rect.left_center() + Vec2::new(14.0, 0.0);
                    match space.color {
                        Some([r, g, b]) => ui.painter().circle_filled(dot, 5.0, Color32::from_rgb(r, g, b)),
                        None => ui.painter().circle_stroke(dot, 4.5, Stroke::new(1.0, c.muted)),
                    };
                    if let Some(n) = *count {
                        let label = if n == 1 { "1 terminal".to_string() } else { format!("{n} terminais") };
                        ui.painter().text(rect.right_center() - Vec2::new(10.0, 0.0), egui::Align2::RIGHT_CENTER, label, FontId::proportional(11.0), c.muted);
                    }
                    let name_rect = Rect::from_min_max(rect.min + Vec2::new(28.0, 0.0), rect.max - Vec2::new(if count.is_some() { 80.0 } else { 10.0 }, 0.0));
                    match rename {
                        Some((id, text)) if *id == space.id => {
                            let edit = ui.put(name_rect.shrink2(Vec2::new(0.0, 3.0)), egui::TextEdit::singleline(text).id(egui::Id::unique("space_rename")).font(FontId::proportional(13.0)));
                            if edit.lost_focus() {
                                let keep = !ui.input(|i| i.key_pressed(Key::Escape)) && !text.trim().is_empty();
                                renamed = Some(keep.then(|| (id.clone(), text.trim().to_string())));
                            }
                        }
                        _ => {
                            let weight = if on { c.text } else { c.text.gamma_multiply(0.85) };
                            ui.painter().with_clip_rect(name_rect).text(name_rect.left_center(), egui::Align2::LEFT_CENTER, &space.name, FontId::proportional(13.0), weight);
                        }
                    }
                    if response.clicked() {
                        go = Some(space.id.clone());
                    }
                    response.context_menu(|ui| {
                        ui.set_min_width(200.0);
                        let folder = !space.root.trim().is_empty();
                        if ui.button("Renomear").clicked() {
                            menu = Some(("rename", space.id.clone()));
                        }
                        if ui.button("Editar Espaço de Trabalho").clicked() {
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
                        if ui.add_enabled(space.id != MAIN_SPACE, egui::Button::new("Apagar")).clicked() {
                            menu = Some(("delete", space.id.clone()));
                        }
                    });
                }
            });
        });
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
            _ => self.space_dialog = Some(SpaceDialog::Delete { id, name: space.name }),
        }
    }

    pub(crate) fn space_dialog_ui(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.space_dialog.clone() else { return };
        let c = self.colors();
        let (confirm, cancel) = match &mut dialog {
            SpaceDialog::Edit { id, name, color, root } => {
                let title = if id.is_some() { "Editar Espaço de Trabalho" } else { "Novo Espaço de Trabalho" };
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
            SpaceDialog::Delete { name, .. } => dialog_ui(ctx, &c, &format!("Excluir “{name}”?"), "Excluir", |ui| {
                dialog_note(ui, "O espaço de trabalho e o que está nele saem do MagoAgent. A pasta do seu projeto não é alterada.", &c);
            }),
        };
        if cancel {
            self.space_dialog = None;
            return;
        }
        // Criar needs a name: without one, Enter and the button do nothing
        let named = !matches!(&dialog, SpaceDialog::Edit { name, .. } if name.trim().is_empty());
        if !(confirm && named) {
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
        }
    }
}
