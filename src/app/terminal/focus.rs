use super::*;

impl DrawingApp {
    /// Click: an 80×24 terminal centered there; drag: that box. Asks for its shell first.

    pub(crate) fn open_terminal_dialog(&mut self, start: Pos2, end: Pos2) {
        let at = if start.distance(end) * self.zoom < 10.0 {
            Rect::from_center_size(start, self.new_terminal_size())
        } else {
            let r = Rect::from_two_pos(start, end);
            Rect::from_min_size(r.min, r.size().max(TERMINAL_MIN))
        };
        let spec = TerminalSpec { name: self.command_name(""), cwd: self.new_folder(), command: String::new(), ..Default::default() };
        self.terminal_dialog = Some(TerminalDialog { target: None, at, spec, tab: 0 });
        self.tool = Tool::Select;
    }

    /// Creates the dialog's terminal, or applies it to the one being edited (restarting it
    /// when the folder or command changed).
    pub(crate) fn confirm_terminal_dialog(&mut self) {
        let Some(TerminalDialog { target, at, mut spec, .. }) = self.terminal_dialog.take() else { return };
        self.prefs.folders.remember(&spec.cwd);
        let name = match spec.name.trim() {
            "" => self.command_name(&spec.command),
            name => name.to_string(),
        };
        spec.name = self.unique_terminal_name(&name, target.as_deref());
        self.snapshot();
        if let Some(id) = target {
            if let Some(e) = self.scene.elements.iter_mut().find(|e| e.id == id) {
                let restart = e.terminal.as_ref().is_some_and(|old| old.cwd != spec.cwd || old.command != spec.command || old.look.shell != spec.look.shell);
                e.terminal = Some(spec);
                if restart {
                    self.terminals.remove(&id);
                }
            }
            return;
        }
        let mut terminal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        terminal.style = node_style(terminal.style);
        let chat = spec.chat;
        terminal.terminal = Some(spec);
        self.terminal_focus = Some(terminal.id.clone());
        let id = terminal.id.clone();
        self.scene.elements.push(terminal);
        self.selected = vec![self.scene.elements.len() - 1];
        if chat {
            self.chat_on(&id);
        }
    }

    /// Descarregar (`on`) or Recarregar terminal or portal `i`, as one undo step: false when it is
    /// neither or already so. The next frame ends or starts its shell or page.
    pub(crate) fn unload_node(&mut self, i: usize, on: bool) -> bool {
        let e = &self.scene.elements[i];
        if (e.terminal.is_none() && e.portal.is_none()) || e.unloaded() == on {
            return false;
        }
        self.snapshot();
        self.scene.elements[i].set_unloaded(on)
    }

    /// A press on a terminal's body: it takes the keyboard (the overlay types into it from here on)
    /// and restarts its shell if that had ended. Stays selected, so its bar stays up.
    pub(crate) fn focus_terminal(&mut self, index: usize) {
        let id = self.scene.elements[index].id.clone();
        if self.terminals.get(&id).is_some_and(|s| s.as_ref().map_or(true, |s| s.exited())) {
            self.terminals.remove(&id);
        }
        self.moving = false;
        self.marquee = None;
        self.selected = vec![index];
        if self.scene.elements[index].terminal.as_ref().is_some_and(|t| t.look.blurred) {
            return;
        }
        // "Iniciar no modo de chat": its chat takes the click; Abrir no canvas or Ampliar still type into it
        if self.scene.elements[index].terminal.as_ref().is_some_and(|t| t.chat) && self.terminal_elevated.as_ref() != Some(&id) {
            self.chat_on(&id);
            return;
        }
        self.terminal_focus = Some(id);
    }

    /// Double-click on the title strip enlarges it (or puts it back); anywhere else focuses it.
    pub(crate) fn double_click_terminal(&mut self, index: usize, world: Pos2) {
        let id = self.scene.elements[index].id.clone();
        let on_header = world.y - local_bounds(&self.scene.elements[index]).min.y < TERMINAL_HEADER;
        if on_header {
            self.set_elevated(if self.terminal_elevated.as_ref() == Some(&id) { None } else { Some(id.clone()) });
        }
        self.focus_terminal(index);
    }

    /// Enlarges a terminal (None puts it back), with a short zoom animation either way.
    pub(crate) fn set_elevated(&mut self, id: Option<String>) {
        if id == self.terminal_elevated {
            return;
        }
        // a note going back ends its typing
        if self.elevated_index().is_some_and(|i| self.scene.elements[i].created.is_some()) {
            self.finish_text();
        }
        if let Some(old) = self.terminal_elevated.take() {
            self.leaving = Some((old, self.now));
        }
        self.elevate_since = self.now;
        // each enlarging starts in the middle
        self.dock = None;
        self.dock_drag = None;
        // a double-click on the title also makes the bar's Ampliar icon hop
        self.hop = Some(("expand", self.now));
        self.terminal_elevated = id;
    }

    /// The enlarged window's element, if one is.
    pub(crate) fn elevated_index(&self) -> Option<usize> {
        let id = self.terminal_elevated.as_ref()?;
        self.scene.elements.iter().position(|e| &e.id == id)
    }

    /// Double-click on a window's title: enlarged above the canvas and in focus, or back on the
    /// canvas. A note comes up with its text ready to type.
    pub(crate) fn toggle_lift(&mut self, index: usize) {
        let id = self.scene.elements[index].id.clone();
        if self.terminal_elevated.as_ref() == Some(&id) {
            self.set_elevated(None);
            return;
        }
        self.moving = false;
        self.marquee = None;
        self.selected = vec![index];
        self.set_elevated(Some(id));
        if self.scene.elements[index].created.is_some() {
            self.begin_text_edit(index);
        }
    }

    /// Enlarges the node above the canvas, from a command.
    pub(crate) fn enlarge(&mut self, index: usize) {
        self.moving = false;
        self.marquee = None;
        self.selected = vec![index];
        self.set_elevated(Some(self.scene.elements[index].id.clone()));
    }

    /// ⌘3: the file tree screen. The enlarged tree, else the first on the canvas, opened as a double-click
    /// opens it; already open, it only takes the keyboard back.
    pub(crate) fn open_tree(&mut self) {
        self.finish_text();
        let is_tree = |e: &Element| e.filetree.is_some();
        let elevated = self.terminal_elevated.as_ref().and_then(|id| self.scene.elements.iter().position(|e| &e.id == id && is_tree(e)));
        let Some(index) = elevated.or_else(|| self.scene.elements.iter().position(is_tree)) else {
            self.toast("Nenhuma Árvore de Arquivos no canvas: crie uma com a ferramenta da barra");
            return;
        };
        self.show_tree(index);
    }
}
