//! IDE nodes on the canvas: the folder dialog, the card painted while one sits on the canvas, the
//! live editor (src/ide) while it is enlarged, and the Claude Code terminals roped to it.

use crate::ide::{Ide, card, icons, theme as t};
use crate::prelude::*;
use std::path::Path;

/// Scrolls a card's tree so its selected row is in view.
fn show_selected(ide: &mut Ide, tree: Rect) {
    let view = tree.height() - card::TREE_TOP - card::FOOTER;
    let top = ide.card_sel.unwrap_or(0) as f32 * card::ROW;
    ide.card_scroll = ide.card_scroll.max(top + card::ROW - view).min(top).max(0.0);
}

/// The tree row under `world` on a card's `tree` column, scrolled by `scroll` (past the last row
/// too: the caller finds no node there).
fn card_row(tree: Rect, world: Pos2, scroll: f32) -> Option<usize> {
    (world.y >= tree.top() + card::TREE_TOP && world.y < tree.bottom() - card::FOOTER)
        .then(|| ((world.y - tree.top() - card::TREE_TOP + scroll) / card::ROW).floor().max(0.0) as usize)
}

/// The new / edit IDE dialog.
#[derive(Clone, Debug)]
pub(crate) struct IdeDialog {
    /// The IDE being edited; None creates one in `at`.
    pub(crate) target: Option<String>,
    pub(crate) at: Rect,
    pub(crate) spec: IdeSpec,
}

impl DrawingApp {
    /// Click: an IDE card centered there; drag: that box. Asks for the folder first.
    pub(crate) fn open_ide_dialog(&mut self, start: Pos2, end: Pos2) {
        let at = if start.distance(end) * self.zoom < 10.0 {
            Rect::from_center_size(start, IDE_SIZE)
        } else {
            let r = Rect::from_two_pos(start, end);
            Rect::from_min_size(r.min, r.size().max(IDE_MIN))
        };
        let folder = self.file_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().to_string());
        self.ide_dialog = Some(IdeDialog { target: None, at, spec: IdeSpec { name: String::new(), root: folder.unwrap_or_default(), ..Default::default() } });
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }

    pub(crate) fn confirm_ide_dialog(&mut self) {
        let Some(IdeDialog { target, at, mut spec }) = self.ide_dialog.take() else { return };
        spec.root = spec.root.trim().to_string();
        if spec.name.trim().is_empty() {
            let folder = std::path::Path::new(&spec.root).file_name().map(|n| n.to_string_lossy().to_string());
            spec.name = folder.unwrap_or_else(|| "IDE".into());
        }
        self.snapshot();
        if let Some(id) = target {
            if let Some(e) = self.scene.elements.iter_mut().find(|e| e.id == id) {
                e.ide = Some(spec);
            }
            return;
        }
        let mut ide = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        // what exports and the AI's view show; the app paints the project card instead
        ide.style = node_style(ide.style);
        ide.ide = Some(spec);
        self.scene.elements.push(ide);
        self.selected = vec![self.scene.elements.len() - 1];
    }

    pub(crate) fn ide_dialog_ui(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.ide_dialog.clone() else { return };
        let c = self.colors();
        let editing = dialog.target.is_some();
        let title = if editing { "Editar IDE" } else { "Novo IDE" };
        let (confirm, cancel) = dialog_ui(ctx, &c, title, if editing { "Salvar" } else { "Criar" }, |ui| {
            dialog_folder(ui, "Pasta do projeto", &mut dialog.spec.root, "a pasta do projeto", "Pasta do projeto", &c);
            dialog_field(ui, "Nome", &mut dialog.spec.name, "opcional: o nome da pasta", DIALOG_WIDTH, &c);
            dialog_note(ui, "Duplo clique no título do IDE para abri-lo. Um terminal ligado a ele por Conectar roda o Claude Code \
                integrado: as mudanças propostas aparecem como diff no IDE.", &c);
        });
        if cancel {
            self.ide_dialog = None;
            return;
        }
        self.ide_dialog = Some(dialog);
        if confirm {
            self.confirm_ide_dialog();
        }
    }

    /// Folders dropped on the canvas become IDEs, side by side from where they land.
    pub(crate) fn drop_folders(&mut self, ctx: &Context, dropped: &[PathBuf]) {
        let at = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(self.canvas_rect.center());
        let mut x = self.to_world(at).x;
        for dir in dropped.iter().filter(|p| p.is_dir()) {
            let spec = IdeSpec { name: String::new(), root: dir.to_string_lossy().into(), ..Default::default() };
            let at = Rect::from_min_size(Pos2::new(x, self.to_world(at).y), IDE_SIZE);
            self.ide_dialog = Some(IdeDialog { target: None, at, spec });
            self.confirm_ide_dialog();
            x += IDE_SIZE.x + 40.0;
        }
    }

    /// One editor per IDE element: new ones open, removed or re-pointed ones save and close
    /// (`Drop`: their Claude Code lock file and language servers go with them). All of them answer
    /// Claude Code every frame.
    pub(crate) fn sync_ides(&mut self, ctx: &Context) {
        let live: Vec<(String, String)> = self.scene.elements.iter().filter_map(|e| Some((e.id.clone(), e.ide.as_ref()?.root.clone()))).collect();
        self.ides.retain(|id, ide| live.iter().any(|(live, root)| live == id && *root == ide.folder));
        for (id, root) in live {
            if !self.ides.contains_key(&id) {
                let ide = Ide::new(ctx.clone(), &id, &root);
                self.ides.insert(id, ide);
            }
        }
        let mut notice = None;
        for (id, ide) in &mut self.ides {
            ide.background(ctx);
            // what an IDE off screen says (a diff Claude Code proposes) shows on the canvas
            if let Some(message) = ide.notice.take().filter(|_| self.terminal_elevated.as_ref() != Some(id)) {
                let name = self.scene.elements.iter().find(|e| &e.id == id).and_then(|e| e.ide.as_ref()).map_or(String::new(), |s| s.name.clone());
                notice = Some(format!("{name}: {message}"));
            }
        }
        if let Some(message) = notice {
            self.toast(message);
        }
        // the last file closed (in the panel or in the open IDE) takes the card's panel with it
        for index in 0..self.scene.elements.len() {
            self.fit_card_panel(index);
        }
        if self.ide_focus.is_some() && self.ide_focus != self.terminal_elevated {
            self.ide_focus = None;
        }
    }

    /// `set_elevated` going from `old` to `new`: an IDE leaving saves its files, one arriving
    /// takes the keyboard.
    pub(crate) fn ide_elevated(&mut self, old: Option<String>, new: &Option<String>) {
        if let Some(ide) = old.and_then(|id| self.ides.get_mut(&id)) {
            ide.save_all();
        }
        self.ide_focus = new.clone().filter(|id| self.scene.elements.iter().any(|e| &e.id == id && e.ide.is_some()));
        if self.ide_focus.is_some() {
            self.terminal_focus = None;
        }
    }

    /// Enlarges the IDE above the canvas: from its title's double-click, its bar, or the panel.
    pub(crate) fn enlarge_ide(&mut self, index: usize) {
        self.moving = false;
        self.marquee = None;
        self.selected = vec![index];
        self.set_elevated(Some(self.scene.elements[index].id.clone()));
    }

    /// Claude Code in a terminal roped to an IDE connects to it. Only when the shell starts: a
    /// terminal roped afterwards needs Reiniciar (or `/ide` inside claude).
    pub(crate) fn ide_env(&self, terminal: &str) -> Vec<(String, String)> {
        let links = self.links();
        links.get(terminal).into_iter().flatten().filter_map(|id| self.ides.get(id)).flat_map(Ide::claude_env).collect()
    }

    /// The IDE's Claude Code button: the `claude` terminal roped to it, made beside it in its
    /// folder the first time (roped before it starts, so it connects), back on the canvas and typing.
    pub(crate) fn claude_for_ide(&mut self, ide: &str) {
        let Some(element) = self.scene.elements.iter().find(|e| e.id == ide).cloned() else { return };
        let is_claude = |id: &String| self.scene.elements.iter().any(|e| &e.id == id && e.terminal.as_ref().is_some_and(|t| t.command.trim_start().starts_with("claude")));
        let roped = self.links().get(ide).and_then(|l| l.iter().find(|id| is_claude(id)).cloned());
        let terminal = match roped {
            Some(id) => id,
            None => {
                let b = local_bounds(&element);
                let at = Rect::from_min_size(Pos2::new(b.max.x + 60.0, b.min.y), terminal_size(100, 30));
                let cwd = element.ide.map(|s| s.root).unwrap_or_default();
                let spec = TerminalSpec { name: "Claude Code".into(), cwd, command: "claude".into(), ..Default::default() };
                self.terminal_dialog = Some(TerminalDialog { target: None, at, spec, tab: 0 });
                self.confirm_terminal_dialog();
                let Some(id) = self.terminal_focus.clone() else { return };
                // one Cmd+Z takes back the terminal and its rope
                self.rope_between(ide, &id, false);
                id
            }
        };
        self.set_elevated(None);
        self.selected = self.scene.elements.iter().position(|e| e.id == terminal).into_iter().collect();
        self.terminal_focus = Some(terminal);
    }

    /// The card on the canvas: title strip (folder, name, branch), the folder tree, the file panel
    /// once a file is open, and the footer. Nothing here takes focus; `ide_card_press` hits it.
    pub(crate) fn paint_ide(&self, painter: &Painter, element: &Element, rect: Rect, zoom: f32) {
        let spec = element.ide.clone().unwrap_or_default();
        let ide = self.ides.get(&element.id);
        let local = local_bounds(element);
        let map = |r: Rect| Rect::from_min_size(rect.min + (r.min - local.min) * zoom, r.size() * zoom);
        let radius = (8.0 * zoom).min(255.0) as u8;
        painter.rect(rect, radius, t::ISLAND, Stroke::new(1.0, Color32::from_gray(0x3a)), StrokeKind::Inside);
        let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
        let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TERMINAL_HEADER * zoom));
        painter.rect_filled(header, egui::CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, t::FRAME);
        let folder = header.left_center() + Vec2::new(TERMINAL_PAD + 6.0, 0.0) * zoom;
        icons::paint(&painter, icons::FOLDER, Rect::from_center_size(folder, Vec2::splat(14.0 * zoom)));
        let name = if spec.name.is_empty() { "IDE" } else { &spec.name };
        let name_rect = painter.text(folder + Vec2::new(12.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, name, FontId::proportional(12.0 * zoom), t::TEXT);
        if let Some(branch) = ide.map(|i| i.branch.as_str()).filter(|b| !b.is_empty()) {
            let at = name_rect.right_center() + Vec2::new(16.0 * zoom, 0.0);
            icons::paint(&painter, icons::BRANCH, Rect::from_center_size(at, Vec2::splat(13.0 * zoom)));
            painter.text(at + Vec2::new(10.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, branch, FontId::proportional(11.0 * zoom), t::MUTED);
        }
        if self.terminal_elevated.as_ref() == Some(&element.id) {
            paint_icon(&painter, Icon::Expand, Self::restore_button(rect).center(), t::TEXT);
        }
        let (tree, panel) = card::parts(local, spec.tree_width);
        if let Some(ide) = ide {
            let focused = self.card_focus.as_ref() == Some(&element.id);
            let hover = self.last_pointer.map(|p| self.to_world(p)).filter(|w| focused && tree.contains(*w)).and_then(|w| card_row(tree, w, ide.card_scroll));
            ide.paint_tree(&painter, tree, &map, zoom, focused, hover);
            if let Some(panel) = panel {
                ide.paint_panel(&painter, panel, &map, zoom, ide.panel_scroll);
            }
        }
        let footer = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - TERMINAL_FOOTER * zoom), rect.max);
        painter.rect_filled(footer, egui::CornerRadius { nw: 0, ne: 0, sw: radius, se: radius }, t::FRAME);
        let small = FontId::proportional(11.0 * zoom);
        let note = if ide.is_some_and(Ide::claude_connected) { "✻ Claude Code conectado" } else { "Duplo clique para ampliar" };
        let note = painter.text(footer.right_center() - Vec2::new(TERMINAL_PAD * zoom, 0.0), egui::Align2::RIGHT_CENTER, note, small.clone(), t::MUTED);
        // the folder gives way to the note on its right
        let room = Rect::from_min_max(footer.min, Pos2::new(note.left() - TERMINAL_PAD * zoom, footer.bottom())).intersect(painter.clip_rect());
        painter.with_clip_rect(room).text(footer.left_center() + Vec2::new(TERMINAL_PAD * zoom, 0.0), egui::Align2::LEFT_CENTER, display_folder(&spec.root), small, t::MUTED);
    }

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
        let mut file = None;
        if let Some(ide) = self.ides.get_mut(&id) {
            // the person's click takes over from what an agent lit
            ide.panel_mark = None;
            if let Some(p) = panel.filter(|p| world.x >= p.left()) {
                match ide.panel_tabs(p).into_iter().find(|(r, _)| r.contains(world)) {
                    Some((r, i)) if card::tab_close(r).contains(world) => ide.close_tab(i),
                    Some((_, i)) => ide.active = i,
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
        // whatever took the keys since (a terminal, the open IDE, a field) keeps them
        let taken = self.terminal_focus.is_some() || self.ide_focus.is_some() || self.terminal_elevated.is_some() || self.editing.is_some() || ctx.egui_wants_keyboard_input();
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
    }

    /// The panel goes with the last open file: the node back to its tree width.
    pub(crate) fn fit_card_panel(&mut self, index: usize) {
        let element = &self.scene.elements[index];
        if element.ide.as_ref().is_some_and(|s| s.tree_width.is_some()) && self.ides.get(&element.id).is_some_and(|ide| !ide.has_text_tabs()) {
            self.card_close_panel(index);
        }
    }

    /// A file opens in the panel: the node grows to the right by the panel, keeping its tree width.
    // ponytail: no undo step for the node's width, so Cmd+Z can leave a panel's width behind
    pub(crate) fn card_open_file(&mut self, index: usize, path: PathBuf) {
        let element = &mut self.scene.elements[index];
        let id = element.id.clone();
        if let (Some(spec), ElementKind::Rectangle { width, .. }) = (element.ide.as_mut(), &mut element.kind) {
            if spec.tree_width.is_none() {
                spec.tree_width = Some(*width);
                *width += card::PANEL_W;
            }
        }
        if let Some(ide) = self.ides.get_mut(&id) {
            ide.open_file(path, None);
        }
    }

    /// No file left in the panel: the node goes back to its tree width (or stays narrower, when
    /// it was resized below it).
    pub(crate) fn card_close_panel(&mut self, index: usize) {
        let element = &mut self.scene.elements[index];
        if let (Some(spec), ElementKind::Rectangle { width, .. }) = (element.ide.as_mut(), &mut element.kind) {
            if let Some(tree) = spec.tree_width.take() {
                *width = tree.min(*width);
            }
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
        let Some(index) = self.scene.elements.iter().rposition(|e| {
            e.ide.is_some() && element_bounds(e).contains(world) && self.terminal_elevated.as_ref() != Some(&e.id)
        }) else {
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
                let max = (ide.active_lines() as f32 * card::LINE - card::panel_body(panel).height()).max(0.0);
                ide.panel_scroll = (ide.panel_scroll + delta).clamp(0.0, max);
            }
            None => {
                let max = (ide.card_rows().len() as f32 * card::ROW - (tree.height() - card::TREE_TOP - card::FOOTER)).max(0.0);
                ide.card_scroll = (ide.card_scroll + delta).clamp(0.0, max);
            }
        }
        true
    }

    /// ⌘3: the IDE screen. The enlarged IDE, else the first on the canvas, opened as a double-click
    /// opens it; already open, it only takes the keyboard back (a dialog or the chat had it).
    pub(crate) fn open_ide(&mut self) {
        self.finish_text();
        let is_ide = |e: &Element| e.ide.is_some();
        let elevated = self.terminal_elevated.as_ref().and_then(|id| self.scene.elements.iter().position(|e| &e.id == id && is_ide(e)));
        let Some(index) = elevated.or_else(|| self.scene.elements.iter().position(is_ide)) else {
            self.toast("Nenhum IDE no canvas: crie um com a ferramenta IDE da barra");
            return;
        };
        self.chat_open = false;
        self.enlarge_ide(index);
        self.ide_focus = Some(self.scene.elements[index].id.clone());
    }

    /// The enlarged IDE above the canvas: the card while it grows, then the live editor at 100%.
    /// It has the keyboard until a click lands elsewhere; a press on the canvas margin goes back.
    pub(crate) fn ide_overlay(&mut self, ctx: &Context) {
        let elevated = self.terminal_elevated.as_ref().and_then(|id| self.scene.elements.iter().find(|e| &e.id == id && e.ide.is_some()));
        let Some(element) = elevated.cloned() else { return };
        let id = element.id.clone();
        let (rect, zoom) = self.terminal_frame(&element);
        // not a pan: the middle button or Space drag the view, not away from it (a right click still
        // ends it: its menu's keys are the canvas's)
        let press = ctx.input(|i| (i.pointer.primary_pressed() || i.pointer.secondary_pressed()).then(|| i.pointer.interact_pos()).flatten()).filter(|_| !self.panning);
        if press.is_some_and(|p| !rect.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background)) {
            self.set_elevated(None);
            return;
        }
        let covered = self.palette_open || self.export_open || self.dialog_open();
        if covered {
            self.ide_focus = None;
        } else if press.is_some_and(|p| rect.contains(p)) {
            self.ide_focus = Some(id.clone());
        }
        let focused = self.ide_focus.as_ref() == Some(&id);
        let animating = self.elevate_animating();
        let has_agent = !self.roped_agents(&id).is_empty();
        egui::Area::new(egui::Id::unique(("ide_overlay", id.as_str()))).fixed_pos(rect.min).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_clip_rect(rect);
            if animating {
                ctx.request_repaint();
                self.paint_ide(ui.painter(), &element, rect, zoom);
                ui.allocate_rect(rect, Sense::hover());
            } else if let Some(ide) = self.ides.get_mut(&id) {
                ide.has_agent = has_agent;
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| ide.ui(ui, focused));
            }
        });
        let Some(ide) = self.ides.get_mut(&id) else { return };
        let notes = std::mem::take(&mut ide.to_agent);
        if std::mem::take(&mut ide.wants_claude) {
            self.claude_for_ide(&id);
        } else if std::mem::take(&mut ide.wants_canvas) {
            self.set_elevated(None);
        }
        // comments on changes go to the agents roped to this IDE
        if !notes.is_empty() {
            let agents = self.roped_agents(&id);
            if agents.is_empty() {
                self.toast("Ligue um agente a este IDE para mandar o comentário");
            }
            // DIFF_REJECTED travels by the WebSocket thread; if the paste's Enter reached Claude Code
            // while its diff prompt was still up, it would pick the highlighted "Yes".
            // ponytail: 1 s fixo, sem confirmação do CLI; se falhar, colar depois da resposta sair pelo socket
            for note in &notes {
                for agent in &agents {
                    self.send_prompt_after(agent, note, std::time::Duration::from_secs(1));
                }
            }
        }
    }
}
