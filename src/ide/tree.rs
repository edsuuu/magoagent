//! The Project tool window: the folder tree, plus the file list behind Go to File and the index.

use super::chrome::row_colored;
use super::*;

pub(super) struct TreeNode {
    path: PathBuf,
    pub(super) name: String,
    pub(super) dir: bool,
    pub(super) open: bool,
    children: Option<Vec<TreeNode>>,
}

impl TreeNode {
    pub(super) fn new(path: PathBuf, dir: bool) -> Self {
        let name = path.file_name().map_or_else(|| path.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned());
        TreeNode { path, name, dir, open: false, children: None }
    }
    pub(super) fn children(&mut self) -> &mut Vec<TreeNode> {
        self.children.get_or_insert_with(|| {
            let mut v: Vec<TreeNode> = std::fs::read_dir(&self.path).into_iter().flatten().flatten()
                .map(|e| TreeNode::new(e.path(), e.file_type().is_ok_and(|t| t.is_dir())))
                .collect();
            v.sort_by_cached_key(|n| (!n.dir, n.name.to_lowercase()));
            v
        })
    }

    /// Reads the open folders again (what was open stays open; a closed one is read when it
    /// opens); true when a name came or went.
    pub(super) fn sync(&mut self) -> bool {
        let Some(old) = self.children.take() else { return false };
        let mut old: std::collections::HashMap<PathBuf, TreeNode> = old.into_iter().map(|n| (n.path.clone(), n)).collect();
        let mut changed = false;
        for node in self.children() {
            match old.remove(&node.path) {
                Some(mut kept) if kept.dir == node.dir => {
                    if kept.open {
                        changed |= kept.sync();
                    } else {
                        kept.children = None;
                    }
                    *node = kept;
                }
                _ => changed = true,
            }
        }
        changed || !old.is_empty()
    }
}

impl Ide {
    pub(super) fn project(&mut self, ui: &mut Ui, focused: bool) {
        tool_header(ui, "Projeto");
        let (mut at, mut menu) = ((0, None), None);
        let sel = (self.card_sel, self.tree_keys && focused);
        let area = egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(tree) = &mut self.tree {
                tree_rows(ui, tree, 0, sel, &self.git.status, &mut at, &mut menu);
            }
        }).inner_rect;
        // the arrows are the panel's after a click in it, until a press elsewhere
        if at.1.is_some() {
            self.tree_keys = true;
        } else if ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.contains(p))) {
            self.tree_keys = false;
        }
        if let Some((row, twice)) = at.1 {
            self.card_sel = Some(row);
            if twice && let Some(p) = self.card_click(row) {
                self.open_file(p, None);
            }
        }
        if self.tree_keys && focused && ui.memory(|m| m.focused().is_none()) {
            for key in [Key::ArrowUp, Key::ArrowDown, Key::ArrowLeft, Key::ArrowRight, Key::Enter] {
                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, key)) && let Some(p) = self.card_key(key) {
                    self.open_file(p, None);
                }
            }
        }
        if let Some((path, action)) = menu {
            self.file_action(&path, action);
        }
    }
}

/// Hidden like PhpStorm's default "Ignored Files and Folders".
pub(super) const HIDDEN: &[&str] = &[".git", ".hg", ".svn", ".DS_Store", "Thumbs.db", "__pycache__"];

/// One row of the Project panel and the open ones under it. Rows are numbered as on the card (the
/// root apart, its children from 0) in `at.0`; `sel` is the selected one and whether the panel has
/// the keys. A click on a row comes back in `at.1`, with whether it was the second one.
fn tree_rows(
    ui: &mut Ui,
    node: &mut TreeNode,
    depth: usize,
    (sel, keys): (Option<usize>, bool),
    vcs: &std::collections::HashMap<String, git::Status>,
    at: &mut (usize, Option<(usize, bool)>),
    menu: &mut Option<(PathBuf, FileAction)>,
) {
    let row = (depth > 0).then(|| {
        at.0 += 1;
        at.0 - 1
    });
    let fill = match (row.is_some() && row == sel, keys) {
        (true, true) => t::SELECTION,
        (true, false) => t::SELECTION_INACTIVE,
        _ => Color32::TRANSPARENT,
    };
    let (resp, rect) = row_colored(ui, fill, 24.0);
    let x = rect.left() + 10.0 + depth as f32 * 18.0;
    let y = rect.center().y;
    if node.dir {
        let chevron = if node.open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
        icons::paint(ui.painter(), chevron, Rect::from_center_size(pos2(x + 6.0, y), vec2(14.0, 14.0)));
        icons::at(ui.painter(), icons::FOLDER, pos2(x + 23.0, y));
    } else {
        icons::file(ui.painter(), &node.path, pos2(x + 23.0, y), 1.0);
    }
    let color = vcs.get(&path_key(&node.path)).map_or(t::TEXT, |s| s.color());
    let mut name = ui.painter().text(pos2(x + 36.0, y), Align2::LEFT_CENTER, &node.name, t::ui(t::UI_SIZE), color);
    if depth == 0 {
        let full = node.path.display().to_string();
        name = ui.painter().text(pos2(name.right() + 8.0, y), Align2::LEFT_CENTER, full, t::ui(t::UI_SIZE), t::SECONDARY);
    }
    // a long name widens the content, so the panel scrolls sideways to it
    ui.expand_to_include_x(name.right() + 8.0);
    // a click selects and a second one opens; the chevron (and the root) opens and closes the folder
    let on_chevron = node.dir && resp.interact_pointer_pos().is_some_and(|p| p.x < x + 14.0);
    match row {
        Some(r) if !on_chevron && resp.clicked() => at.1 = Some((r, resp.double_clicked())),
        _ if node.dir && resp.clicked() => node.open = !node.open,
        _ => {}
    }
    resp.context_menu(|ui| {
        if let Some(action) = file_menu(ui, depth == 0) {
            *menu = Some((node.path.clone(), action));
        }
    });
    if node.dir && node.open {
        for child in node.children().iter_mut().filter(|c| !HIDDEN.contains(&c.name.as_str())) {
            tree_rows(ui, child, depth + 1, (sel, keys), vcs, at, menu);
        }
    }
}

/// All files under root for Go to File. Skips VCS/dependency caches that would drown results.
pub(super) fn list_files(root: &Path) -> Vec<PathBuf> {
    const SKIP: &[&str] = &[".git", "node_modules", "target", ".idea", "__pycache__", ".next", ".nuxt", ".cache"];
    let mut out = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = e.file_name();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                if !SKIP.iter().any(|s| name == *s) {
                    stack.push(e.path());
                }
            } else {
                out.push(e.path());
            }
            if out.len() >= 200_000 {
                return out; // ponytail: hard cap instead of a real file index
            }
        }
    }
    out
}

impl Ide {
    /// The rows the card shows, top to bottom: (depth, name, is a folder, is open). Only the open
    /// folders' children are listed; the root itself is the card's header.
    pub(crate) fn card_rows(&self) -> Vec<(usize, String, bool, bool)> {
        fn walk(node: &TreeNode, depth: usize, out: &mut Vec<(usize, String, bool, bool)>) {
            let Some(children) = &node.children else { return };
            for c in children.iter().filter(|c| !HIDDEN.contains(&c.name.as_str())) {
                out.push((depth, c.name.clone(), c.dir, c.open));
                if c.dir && c.open {
                    walk(c, depth + 1, out);
                }
            }
        }
        let mut out = vec![];
        if let Some(root) = &self.tree {
            walk(root, 0, &mut out);
        }
        out
    }

    /// A press (or Enter) on card row `row` selects it: a folder opens or closes, and a file comes
    /// back, for the double-click or Enter to open in the panel.
    pub(crate) fn card_click(&mut self, row: usize) -> Option<PathBuf> {
        // the root counts as row -1: its children are the rows
        let mut n = row + 1;
        let node = visible_mut(self.tree.as_mut()?, &mut n)?;
        self.card_sel = Some(row);
        if !node.dir {
            return Some(node.path.clone());
        }
        node.open = !node.open;
        if node.open {
            node.children();
        }
        None
    }

    /// The file on card row `row` (None for a folder): what a double-click opens.
    pub(crate) fn card_file(&mut self, row: usize) -> Option<PathBuf> {
        self.card_path(row).filter(|(_, dir)| !dir).map(|(path, _)| path)
    }

    /// What card row `row` shows, and whether it is a folder.
    pub(crate) fn card_path(&mut self, row: usize) -> Option<(PathBuf, bool)> {
        let mut n = row + 1;
        visible_mut(self.tree.as_mut()?, &mut n).map(|node| (node.path.clone(), node.dir))
    }

    /// The card's keys, as in PhpStorm's tree: ↑↓ move the selection, → opens a folder (then goes
    /// into it), ← closes it (or goes up to its folder), Enter is a press. A file comes back to open.
    pub(crate) fn card_key(&mut self, key: egui::Key) -> Option<PathBuf> {
        let rows = self.card_rows();
        let last = rows.len().checked_sub(1)?;
        let Some(sel) = self.card_sel.filter(|s| *s <= last) else {
            self.card_sel = Some(0);
            return None;
        };
        let (depth, dir, open) = (rows[sel].0, rows[sel].2, rows[sel].3);
        match key {
            egui::Key::ArrowUp => self.card_sel = Some(sel.saturating_sub(1)),
            egui::Key::ArrowDown => self.card_sel = Some((sel + 1).min(last)),
            egui::Key::ArrowRight if dir && !open => _ = self.card_click(sel),
            egui::Key::ArrowRight if dir && rows.get(sel + 1).is_some_and(|r| r.0 > depth) => self.card_sel = Some(sel + 1),
            egui::Key::ArrowLeft if dir && open => _ = self.card_click(sel),
            egui::Key::ArrowLeft => self.card_sel = rows[..sel].iter().rposition(|r| r.0 + 1 == depth).or(Some(sel)),
            egui::Key::Enter => return self.card_click(sel),
            _ => {}
        }
        None
    }
}

impl Ide {
    /// Picks up files made, renamed or removed outside (Claude Code, git, a terminal): the open
    /// folders are read again, the card's selection stays on its file, and Go to File relists.
    pub(crate) fn sync_tree(&mut self) {
        let Some(tree) = &mut self.tree else { return };
        let selected = self.card_sel.and_then(|row| visible_mut(tree, &mut (row + 1)).map(|n| n.path.clone()));
        if !tree.sync() {
            return;
        }
        let mut row = 0;
        if let Some(path) = selected
            && row_of(tree, &path, &mut row)
        {
            self.card_sel = Some(row);
        }
        if let Some(root) = self.root.clone() {
            self.reload_files(&root);
        }
    }

    /// Opens the folders down to `path` (a folder itself opens too) and selects its card row,
    /// as an agent's ide_open shows it. None when it isn't in the project's tree.
    pub(crate) fn card_reveal(&mut self, path: &Path) -> Option<usize> {
        let root = self.tree.as_mut()?;
        let rel = path.strip_prefix(&root.path).ok()?.to_path_buf();
        let mut node = root;
        for part in rel.components() {
            node = node.children().iter_mut().find(|c| c.path.file_name() == Some(part.as_os_str()))?;
            if node.dir {
                node.open = true;
                node.children();
            }
        }
        let mut row = 0;
        let found = row_of(self.tree.as_ref()?, path, &mut row);
        self.card_sel = found.then_some(row);
        self.card_sel
    }
}

/// Counts the visible rows before `path` under `node`, in the card's order; true once found.
fn row_of(node: &TreeNode, path: &Path, row: &mut usize) -> bool {
    for c in node.children.iter().flatten().filter(|c| !HIDDEN.contains(&c.name.as_str())) {
        if c.path == path {
            return true;
        }
        *row += 1;
        if c.dir && c.open && row_of(c, path, row) {
            return true;
        }
    }
    false
}

/// The `n`th visible node below `node` (0 is `node` itself), in the card's order.
fn visible_mut<'a>(node: &'a mut TreeNode, n: &mut usize) -> Option<&'a mut TreeNode> {
    if *n == 0 {
        return Some(node);
    }
    *n -= 1;
    if node.dir && node.open {
        for child in node.children().iter_mut().filter(|c| !HIDDEN.contains(&c.name.as_str())) {
            if let Some(hit) = visible_mut(child, n) {
                return Some(hit);
            }
        }
    }
    None
}
