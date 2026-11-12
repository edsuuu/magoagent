use super::*;

impl DrawingApp {
    /// A press on the card's body: the card takes the keys (its tree shows hover and answers the
    /// arrows), a row is selected and a folder opens or closes; a second press on the same row is
    /// the double-click, which opens a file in the panel (a folder doesn't toggle again). The
    /// panel's tabs switch, and their × closes them. Nothing enlarges the IDE: its title and the bar do.
    pub(crate) fn ide_card_press(&mut self, index: usize, world: Pos2) {
        let element = &self.scene.elements[index];
        let id = element.id.clone();
        let (tree, panel) = card::parts(local_bounds(element), element.ide.as_ref().and_then(|s| s.tree_width));
        let now = self.now;
        self.moving = false;
        self.marquee = None;
        self.selected = vec![index];
        self.card_focus = Some(id.clone());
        // the bar between the tree and the code: dragged, it moves the split
        if self.on_card_split(world) == Some(index) {
            self.card_split = Some(index);
            return;
        }
        let mut file = None;
        if let Some(ide) = self.ides.get_mut(&id) {
            // the person's click takes over from what an agent lit
            ide.panel_mark = None;
            if let Some(p) = panel.filter(|p| world.x >= p.left()) {
                match ide.panel_tabs(p).into_iter().find(|(r, _)| r.contains(world)) {
                    Some((r, i)) if card::tab_close(r).contains(world) => ide.close_tab(i),
                    Some((_, i)) => {
                        // another file starts at its top
                        if ide.card_tab() != Some(i) {
                            ide.panel_scroll = 0.0;
                        }
                        ide.active = i;
                    }
                    None => {}
                }
            } else if let Some(row) = card_row(tree, world, ide.card_scroll) {
                let again = ide.card_press.is_some_and(|(r, t)| r == row && now - t < 0.35);
                ide.card_press = Some((row, now));
                if again {
                    file = ide.card_file(row);
                } else {
                    ide.card_click(row);
                }
            }
        }
        if let Some(path) = file {
            self.card_open_file(index, path);
        }
        self.fit_card_panel(index);
    }

    /// What a right click at `world` on IDE card `index` acts on: its tree row's file or folder
    /// (selected, as the press would), or the project's own folder below the rows; true for the
    /// project's folder. None off the tree.
    pub(crate) fn card_path_at(&mut self, index: usize, world: Pos2) -> Option<(PathBuf, bool)> {
        let element = &self.scene.elements[index];
        let (tree, panel) = card::parts(local_bounds(element), element.ide.as_ref().and_then(|s| s.tree_width));
        if !tree.contains(world) || panel.is_some_and(|p| world.x >= p.left()) {
            return None;
        }
        let ide = self.ides.get_mut(&element.id)?;
        let row = card_row(tree, world, ide.card_scroll).and_then(|row| Some((row, ide.card_path(row)?.0)));
        match row {
            Some((row, path)) => {
                ide.card_sel = Some(row);
                Some((path, false))
            }
            None => ide.root.clone().map(|root| (root, true)),
        }
    }

    /// The card's tree with the keys: ↑↓→← and Enter (a file opens in the panel); Esc lets go.
    pub(crate) fn ide_card_keys(&mut self, ctx: &Context) {
        // a file tree node has its own keys
        if self.card_focus.as_ref().is_some_and(|id| self.ft_spec(id).is_some()) {
            return self.filetree_keys(ctx);
        }
        // whatever took the keys since (a terminal, the open IDE, a field) keeps them
        let taken = self.terminal_focus.is_some()
            || self.ide_focus.is_some()
            || self.terminal_elevated.is_some()
            || self.editing.is_some()
            || ctx.egui_wants_keyboard_input();
        let index = self.card_focus.as_ref().and_then(|id| self.scene.elements.iter().position(|e| &e.id == id && e.ide.is_some()));
        let Some(index) = index.filter(|_| !taken) else {
            self.card_focus = None;
            return;
        };
        // in the order they came (held arrows repeat), taken from the canvas
        const KEYS: [Key; 6] = [Key::ArrowUp, Key::ArrowDown, Key::ArrowLeft, Key::ArrowRight, Key::Enter, Key::Escape];
        let keys = ctx.input_mut(|i| {
            let mut keys = vec![];
            i.events.retain(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.is_none() && KEYS.contains(key) => {
                    keys.push(*key);
                    false
                }
                _ => true,
            });
            keys
        });
        let element = &self.scene.elements[index];
        let (tree, _) = card::parts(local_bounds(element), element.ide.as_ref().and_then(|s| s.tree_width));
        let Some(ide) = self.ides.get_mut(&element.id) else { return };
        let mut files = vec![];
        for key in keys {
            if key == Key::Escape {
                // the arrows go back to nothing: with the node still selected they would push it
                self.card_focus = None;
                self.selected.clear();
                break;
            }
            files.extend(ide.card_key(key));
            show_selected(ide, tree);
        }
        for path in files {
            self.card_open_file(index, path);
        }
    }

    /// An agent working in an IDE card (the MCP's ide_* tools), in view: the folder opens in the
    /// tree, the file in the panel at its line, with what it opened or changed lit; a search tells.
    pub(crate) fn ide_agent(&mut self, id: &str, action: IdeAction, reply: std::sync::mpsc::Sender<Result<String, String>>) {
        let Some(index) = self.scene.elements.iter().position(|e| e.id == id && e.ide.is_some()) else {
            let _ = reply.send(Err("esse IDE não está mais no canvas".into()));
            return;
        };
        let element = &self.scene.elements[index];
        let name = element.ide.as_ref().map_or(String::new(), |s| s.name.clone());
        let file = match &action {
            IdeAction::Open { path, .. } | IdeAction::Edit { path, .. } if !path.is_dir() => Some(path.clone()),
            _ => None,
        };
        // the file panel comes up first, so its tree width is set before the reveal scrolls it
        if let Some(path) = file {
            self.card_open_file(index, path);
        }
        let element = &self.scene.elements[index];
        let (tree, panel) = card::parts(local_bounds(element), element.ide.as_ref().and_then(|s| s.tree_width));
        let Some(ide) = self.ides.get_mut(id) else {
            let _ = reply.send(Err("o IDE ainda está abrindo".into()));
            return;
        };
        let root = ide.root.clone();
        let rel = |p: &Path| root.as_ref().and_then(|r| p.strip_prefix(r).ok()).unwrap_or(p).display().to_string();
        let lit = |ide: &mut Ide, line: usize| {
            let body = panel.map_or(0.0, |p| card::panel_body(p).height());
            ide.panel_scroll = (line as f32 * card::LINE - body / 3.0).max(0.0);
        };
        let result = match action {
            IdeAction::Searched { query, hits } => {
                self.toast(format!("{name}: busca por \"{query}\", {hits} resultado(s)"));
                return;
            }
            IdeAction::Open { path, line } => {
                let shown = format!("{}{}", rel(&path), line.map_or(String::new(), |l| format!(", linha {l}")));
                let row = ide.card_reveal(&path);
                if path.is_dir() {
                    row.map(|_| format!("Pasta {shown} aberta na árvore do IDE.")).ok_or(format!("{shown} não está na árvore do projeto"))
                } else if ide.active_path().as_deref() != Some(path.as_path()) {
                    Err(format!("{shown} não abriu: {}", ide.status))
                } else {
                    let line = line.unwrap_or(1).saturating_sub(1);
                    ide.open_file(path.clone(), Some((line, 0)));
                    ide.panel_mark = Some((path, line, line));
                    lit(ide, line);
                    Ok(format!("{shown} aberto no IDE."))
                }
            }
            IdeAction::Edit { path, old, new } => {
                ide.card_reveal(&path);
                ide.agent_edit(&path, &old, &new).map(|(first, last)| {
                    lit(ide, first);
                    format!("{} alterado e salvo (linhas {} a {}).", rel(&path), first + 1, last + 1)
                })
            }
        };
        show_selected(ide, tree);
        let _ = reply.send(result);
        self.fit_card_panel(index);
        // the panel grows onto its neighbours: over them, not under a note; with nothing drawn over
        // it, the order stays the person's
        let card = &self.scene.elements[index];
        let covered = self.scene.elements[index + 1..].iter().any(|e| element_bounds(e).intersects(element_bounds(card)));
        if covered && card.ide.as_ref().is_some_and(|s| s.tree_width.is_some()) {
            self.raise_card(index);
        }
    }

    /// The card to the front, the selection following by id. Only between gestures, as the file
    /// merge: a drag or an editor holds indices into the elements.
    // ponytail: mid-gesture it stays under; upgrade: raise it at the release
    pub(super) fn raise_card(&mut self, index: usize) {
        let busy = self.in_gesture() || self.multi.is_some() || self.editing.is_some() || self.line_editor.is_some() || self.note_selection.is_some();
        if busy || index + 1 >= self.scene.elements.len() {
            return;
        }
        self.keeping_selection(|app| {
            let card = app.scene.elements.remove(index);
            app.scene.elements.push(card);
        });
        self.dirty = true;
    }

    /// The panel goes with the last open file: the node back to its tree width.
    pub(crate) fn fit_card_panel(&mut self, index: usize) {
        let element = &self.scene.elements[index];
        if element.ide.as_ref().is_some_and(|s| s.tree_width.is_some()) && self.ides.get(&element.id).is_some_and(|ide| !ide.has_text_tabs()) {
            self.card_close_panel(index);
        }
    }

    /// A file opens in the panel: the tree narrows to its longest name, a third of the node at
    /// most, and the code takes the rest; a node too small for the panel grows to the right.
    // ponytail: no undo step for the node's width, so Cmd+Z can leave a panel's width behind
    pub(crate) fn card_open_file(&mut self, index: usize, path: PathBuf) {
        let element = &mut self.scene.elements[index];
        let id = element.id.clone();
        let fit = self.ides.get(&id).map_or(card::TREE_MIN, Ide::card_tree_fit);
        if let (Some(spec), ElementKind::Rectangle { width, .. }) = (element.ide.as_mut(), &mut element.kind) {
            if spec.tree_width.is_none() {
                *width = width.max(fit + card::PANEL_W);
                spec.tree_width = Some(fit.min(*width / 3.0).max(card::TREE_MIN));
            }
        }
        if let Some(ide) = self.ides.get_mut(&id) {
            if ide.active_path().as_deref() != Some(path.as_path()) {
                ide.panel_scroll = 0.0;
            }
            ide.open_file(path, None);
        }
    }

    /// No file left in the panel: the node goes back to the tree alone, at the tree's width (the
    /// node's smallest at least, or narrower when it was resized below it).
    pub(crate) fn card_close_panel(&mut self, index: usize) {
        let element = &mut self.scene.elements[index];
        if let (Some(spec), ElementKind::Rectangle { width, .. }) = (element.ide.as_mut(), &mut element.kind) {
            if let Some(tree) = spec.tree_width.take() {
                *width = tree.max(IDE_MIN.x).min(*width);
            }
        }
    }

    /// The IDE card whose bar between the tree and the code is under `world`.
    pub(crate) fn on_card_split(&self, world: Pos2) -> Option<usize> {
        let grab = 5.0 / self.zoom;
        let index = self.hit(world).filter(|i| self.scene.elements[*i].ide.is_some())?;
        let element = &self.scene.elements[index];
        let (_, panel) = card::parts(local_bounds(element), element.ide.as_ref().and_then(|s| s.tree_width));
        let panel = panel?;
        ((world.x - panel.left()).abs() <= grab && world.y > panel.top() + card::HEADER && world.y < panel.bottom() - card::FOOTER).then_some(index)
    }

    /// The bar dragged to `x`: the tree keeps room for a name, the code for a few words.
    pub(crate) fn set_tree_width(&mut self, index: usize, x: f32) {
        let element = &mut self.scene.elements[index];
        let local = local_bounds(element);
        if let Some(spec) = element.ide.as_mut().filter(|s| s.tree_width.is_some()) {
            spec.tree_width = Some((x - local.left()).clamp(card::TREE_MIN, (local.width() - 240.0).max(card::TREE_MIN)));
            self.dirty = true;
        }
    }

    /// Whether a world point is in the open panel's text (not its tabs).
    pub(crate) fn on_panel_text(&self, index: usize, world: Pos2) -> bool {
        let element = &self.scene.elements[index];
        let (_, panel) = card::parts(local_bounds(element), element.ide.as_ref().and_then(|s| s.tree_width));
        panel.is_some_and(|p| card::panel_body(p).contains(world))
    }

    /// A double-click on the panel's text: the enlarged IDE, on the file at that line.
    pub(crate) fn ide_panel_line(&mut self, index: usize, world: Pos2) {
        let element = &self.scene.elements[index];
        let (_, panel) = card::parts(local_bounds(element), element.ide.as_ref().and_then(|s| s.tree_width));
        let (Some(panel), id) = (panel, element.id.clone()) else { return };
        let Some(ide) = self.ides.get_mut(&id) else { return };
        let Some(path) = ide.active_path() else { return };
        let line = ((world.y - card::panel_body(panel).top() + ide.panel_scroll) / card::LINE).floor().max(0.0) as usize;
        ide.open_file(path, Some((line, 0)));
        self.enlarge_ide(index);
    }

    /// Wheel over a card: the tree scrolls, or the panel's text beside it. False when no card is under it.
    pub(crate) fn ide_wheel(&mut self, world: Pos2, dy: f32) -> bool {
        let zoom = self.zoom;
        let Some(index) =
            self.scene.elements.iter().rposition(|e| e.ide.is_some() && element_bounds(e).contains(world) && self.terminal_elevated.as_ref() != Some(&e.id))
        else {
            return false;
        };
        let element = &self.scene.elements[index];
        let local = local_bounds(element);
        let (tree, panel) = card::parts(local, element.ide.as_ref().and_then(|s| s.tree_width));
        let Some(ide) = self.ides.get_mut(&element.id) else { return true };
        // a wheel up (positive) shows what is above
        let delta = -dy / zoom;
        match panel.filter(|p| world.x >= p.left()) {
            Some(panel) => {
                let body = card::panel_body(panel);
                let max = (ide.panel_height(body.width()) - body.height()).max(0.0);
                ide.panel_scroll = (ide.panel_scroll + delta).clamp(0.0, max);
            }
            None => {
                let max = (ide.card_rows().len() as f32 * card::ROW - (tree.height() - card::TREE_TOP - card::FOOTER)).max(0.0);
                ide.card_scroll = (ide.card_scroll + delta).clamp(0.0, max);
            }
        }
        true
    }
}
