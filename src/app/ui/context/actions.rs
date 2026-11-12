use super::*;

impl DrawingApp {
    /// Runs a row of a menu on its target.
    pub(crate) fn context_action(&mut self, ctx: &Context, target: &Target, id: &str) {
        if let Target::Group(gid) = target {
            self.group_menu_action(gid, id);
            return;
        }
        let index = match target {
            Target::Element(eid) => self.scene.elements.iter().position(|e| &e.id == eid),
            Target::Canvas(_) | Target::Group(_) => None,
        };
        let at = match target {
            Target::Canvas(p) => *p,
            Target::Element(_) | Target::Group(_) => Pos2::ZERO,
        };
        if self.terminal_menu_action(ctx, index, id) || self.chat_menu_action(target, id) {
            return;
        }
        match id {
            "rename" => {
                if let Some(i) = index {
                    self.open_rename(i);
                }
            }
            "edit" => {
                if let Some(i) = index {
                    self.open_edit(i);
                }
            }
            "monitor" => {
                if let Some(i) = index.filter(|i| self.scene.elements[*i].terminal.is_some()) {
                    self.snapshot();
                    if let Some(t) = self.scene.elements[i].terminal.as_mut() {
                        t.quiet = !t.quiet;
                    }
                }
            }
            "unfocus" => self.terminal_focus = None,
            "clear" => {
                if let Some(Ok(session)) = index.and_then(|i| self.terminals.get(&self.scene.elements[i].id)) {
                    session.handle.clear();
                }
            }
            "unload" => {
                if let Some(i) = index {
                    self.unload_node(i, true);
                }
            }
            "reload" => {
                // an unloaded one starts again; else the page reloads or the shell starts over
                if let Some(i) = index
                    && !self.unload_node(i, false)
                {
                    let eid = self.scene.elements[i].id.clone();
                    if self.scene.elements[i].portal.is_some() {
                        self.portal_reload(&eid);
                    } else {
                        // the next frame starts the shell again
                        self.terminals.remove(&eid);
                    }
                }
            }
            "copy_el" => self.copy(ctx),
            "dup" => self.duplicate(),
            "select_all" => self.select_all(),
            "tidy" => self.tidy_selection(),
            "front" => self.reorder(Layer::Front),
            "back" => self.reorder(Layer::Back),
            "chrome" => {
                if let Some(i) = index.filter(|i| self.scene.elements[*i].portal.is_some()) {
                    self.snapshot();
                    if let Some(spec) = self.scene.elements[i].portal.as_mut() {
                        spec.chrome_hidden = !spec.chrome_hidden;
                    }
                }
            }
            "lock" => self.toggle_lock_selected(),
            "delete" => self.delete_selected(),
            "copy_content" => {
                if let Some(i) = index {
                    ctx.copy_text(self.note_text(i));
                }
            }
            "note_blur" => {
                if let Some(i) = index {
                    self.set_note_blur(i, !self.scene.elements[i].note.blurred);
                }
            }
            "note_move" => {
                if let Some(i) = index {
                    self.move_note_file_ui(i);
                }
            }
            "lock_content" => {
                if let Some(i) = index {
                    self.snapshot();
                    self.scene.elements[i].note.content_locked = !self.scene.elements[i].note.content_locked;
                }
            }
            "open_browser" => {
                if let Some(url) = index.and_then(|i| self.scene.elements[i].portal.as_ref()).map(|p| normalize_url(&p.url)) {
                    if is_web(&url) {
                        open_external(&url);
                    } else {
                        self.toast("Só endereços http e https abrem no navegador.");
                    }
                }
            }
            "show_folder" if index.is_some_and(|i| self.scene.elements[i].filetree.is_some()) => {
                if let Some(spec) = index.and_then(|i| self.scene.elements[i].filetree.as_ref()) {
                    open_external(&spec.shown().to_string_lossy());
                }
            }
            "show_folder" if index.is_some_and(|i| self.scene.elements[i].created.is_some()) => {
                // the note's `.md` lives in `notas/` beside the scene (file.rs)
                if let (Some(scene), Some(i)) = (&self.autosave, index) {
                    if let Some(folder) = crate::file::note_file(scene, &self.scene.elements[i]).parent() {
                        open_external(&folder.to_string_lossy());
                    }
                }
            }
            "stack" => {
                self.stack_notes();
            }
            "stack_uniform" => {
                if let Some(i) = index {
                    self.uniform_stack_color(i);
                }
            }
            "stack_delete" => {
                if let Some(i) = index {
                    self.delete_stack_page(i);
                }
            }
            "unstack" => {
                if let Some(i) = index {
                    self.unstack(i);
                }
            }
            "stack_page" => {
                if let Some(i) = index {
                    self.new_stack_page(i);
                }
            }
            "stack_remove" => {
                if let Some(i) = index {
                    self.remove_from_stack(i);
                }
            }
            "group" => self.group_selection(),
            "ungroup" => self.ungroup_selection(),
            "paste" => self.paste(""),
            "add_terminal" => self.open_terminal_dialog(at, at),
            "add_note" => self.place_sticky(at, at),
            "add_file" => self.insert_image(),
            "add_portal" => self.open_portal_dialog(at, at),
            "add_text" => self.start_text(at),
            "add_filetree" => self.open_filetree_dialog(at, at),
            _ => {
                if let Some(preset) = id.strip_prefix("preset:").and_then(|k| k.parse::<usize>().ok()).and_then(|k| self.prefs.presets.get(k)).cloned() {
                    self.new_terminal_at(at, &preset);
                } else if let (Some(op), Some(i)) = (id.strip_prefix("navegador:"), index) {
                    let node = self.scene.elements[i].id.clone();
                    self.portal_menu(&node, op);
                } else if let Some(page) = id.strip_prefix("stack_show:") {
                    if let Some(p) = self.scene.elements.iter().position(|e| e.id == page) {
                        self.show_stack_page(p);
                    }
                } else if let Some(op) = id.strip_prefix("fmt:") {
                    if let Some(i) = index {
                        self.note_format(ctx, i, op);
                    }
                } else if let Some(paper) = id.strip_prefix("color:").and_then(|k| k.parse::<usize>().ok()).and_then(|k| NOTE_PAPERS.get(k)) {
                    if let Some(i) = index {
                        self.snapshot();
                        self.scene.elements[i].style.fill = Some(paper.0);
                    }
                } else if let Some(k) = id.strip_prefix("align:").and_then(|k| k.parse::<usize>().ok()) {
                    self.align_selection(k);
                } else if let (Some(other), Some(i)) = (id.strip_prefix("link:"), index) {
                    let node = self.scene.elements[i].id.clone();
                    self.toggle_link(&node, other);
                } else if let (Some(other), Some(i)) = (id.strip_prefix("disconnect:"), index) {
                    let node = self.scene.elements[i].id.clone();
                    self.unlink(&node, other);
                }
            }
        }
    }
}
