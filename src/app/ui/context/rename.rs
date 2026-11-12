use super::*;

impl DrawingApp {
    /// A terminal from a quick start, centered on the click (the new terminal dialog's own path).
    pub(super) fn new_terminal_at(&mut self, at: Pos2, preset: &Preset) {
        let spec = TerminalSpec { name: preset.name.clone(), cwd: self.new_folder(), command: preset.command.clone(), ..Default::default() };
        self.terminal_dialog = Some(TerminalDialog { target: None, at: Rect::from_center_size(at, terminal_size(80, 24)), spec, tab: 0 });
        self.confirm_terminal_dialog();
    }

    /// The edit dialog of a terminal, portal or tree (the node's bar opens the same ones).
    pub(super) fn open_edit(&mut self, i: usize) {
        let e = self.scene.elements[i].clone();
        if let Some(spec) = e.terminal {
            self.terminal_dialog = Some(TerminalDialog { target: Some(e.id), at: Rect::NOTHING, spec, tab: 0 });
        } else if let Some(spec) = e.portal {
            self.portal_dialog = Some(PortalDialog { target: Some(e.id), at: Rect::NOTHING, spec });
        } else if let Some(spec) = e.filetree {
            self.filetrees.dialog = Some(FileTreeDialog { target: Some(e.id), at: Rect::NOTHING, root: spec.root });
        }
    }

    pub(super) fn open_rename(&mut self, i: usize) {
        let e = &self.scene.elements[i];
        let name = match (&e.terminal, &e.portal) {
            (Some(t), _) => t.name.clone(),
            (_, Some(p)) => p.name.clone(),
            _ => note_name(&self.note_text(i)),
        };
        self.rename = Some((e.id.clone(), name));
    }

    /// Gives the element its new name: a terminal's (kept unique), a portal's, or a note's first
    /// line (the name a note shows).
    pub(crate) fn set_name(&mut self, id: &str, name: &str) {
        if let Some(gid) = id.strip_prefix("group:") {
            self.rename_frame_group(gid, name);
            return;
        }
        let name = name.trim();
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return };
        if self.scene.elements[i].terminal.is_some() {
            let name = match name {
                "" => self.command_name(&self.scene.elements[i].terminal.as_ref().map_or(String::new(), |t| t.command.clone())),
                name => name.to_string(),
            };
            let unique = self.unique_terminal_name(&name, Some(id));
            self.snapshot();
            if let Some(t) = self.scene.elements[i].terminal.as_mut() {
                t.name = unique;
            }
        } else if self.scene.elements[i].portal.is_some() {
            self.snapshot();
            if let Some(p) = self.scene.elements[i].portal.as_mut() {
                p.name = name.into();
            }
        } else if self.scene.elements[i].created.is_some() && !name.is_empty() {
            if let Some(label) = note_label(&self.scene.elements, i) {
                self.snapshot();
                if let ElementKind::Text { value, .. } = &mut self.scene.elements[label].kind {
                    *value = rename_note_text(value, name);
                }
            }
        }
    }

    /// The Renomear dialog: a name field, Cancelar and Salvar. A group's name is asked in a balloon
    /// hung from its pill instead.
    pub(crate) fn rename_ui(&mut self, ctx: &Context) {
        let Some((id, mut name)) = self.rename.clone() else { return };
        let c = self.colors();
        if let Some(gid) = id.strip_prefix("group:") {
            let anchor = self.scene.groups.iter().find(|g| g.id == gid).and_then(|g| self.group_pill(g));
            let Some(anchor) = anchor else {
                ctx.data_mut(|d| d.remove::<bool>(egui::Id::unique("group_balloon_open")));
                self.rename = None;
                return;
            };
            let first_id = egui::Id::unique("group_balloon_open");
            let first = !ctx.data(|d| d.get_temp::<bool>(first_id).unwrap_or(false));
            ctx.data_mut(|d| d.insert_temp(first_id, true));
            match name_balloon(ctx, &c, anchor, "Renomear grupo", "Nome do grupo", &mut name, first) {
                None => self.rename = Some((id, name)),
                Some(save) => {
                    ctx.data_mut(|d| d.remove::<bool>(first_id));
                    self.rename = None;
                    if save {
                        self.set_name(&id, &name);
                    }
                }
            }
            return;
        }
        let (confirm, cancel) = dialog_ui(ctx, &c, "Renomear", "Salvar", |ui| {
            dialog_field(ui, "Nome", &mut name, "Nome", DIALOG_WIDTH, &c);
        });
        if cancel {
            self.rename = None;
        } else if confirm {
            self.rename = None;
            self.set_name(&id, &name);
        } else {
            self.rename = Some((id, name));
        }
    }
}
