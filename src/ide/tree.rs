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
    pub(super) fn project(&mut self, ui: &mut Ui) {
        tool_header(ui, "Projeto");
        let active = self.tabs.get(self.active).map(|t| t.path.clone());
        let (mut clicked, mut menu) = (None, None);
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(tree) = &mut self.tree {
                tree_rows(ui, tree, 0, active.as_deref(), &self.git.status, &mut clicked, &mut menu);
            }
        });
        if let Some(p) = clicked {
            self.open_file(p, None);
        }
        if let Some((path, action)) = menu {
            self.file_action(&path, action);
        }
    }
}

/// Hidden like PhpStorm's default "Ignored Files and Folders".
pub(super) const HIDDEN: &[&str] = &[".git", ".hg", ".svn", ".DS_Store", "Thumbs.db", "__pycache__"];

fn tree_rows(
    ui: &mut Ui,
    node: &mut TreeNode,
    depth: usize,
    active: Option<&Path>,
    vcs: &std::collections::HashMap<String, git::Status>,
    clicked: &mut Option<PathBuf>,
    menu: &mut Option<(PathBuf, FileAction)>,
) {
    let selected = active == Some(node.path.as_path());
    // the file shown in the editor: inactive selection colour, as the tree doesn't own focus
    let (resp, rect) = row_colored(ui, if selected { t::SELECTION_INACTIVE } else { Color32::TRANSPARENT }, 24.0);
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
    let g = ui.painter().text(pos2(x + 36.0, y), Align2::LEFT_CENTER, &node.name, t::ui(t::UI_SIZE), color);
    if depth == 0 {
        let full = node.path.display().to_string();
        ui.painter().text(pos2(g.right() + 8.0, y), Align2::LEFT_CENTER, full, t::ui(t::UI_SIZE), t::SECONDARY);
    }
    if resp.clicked() {
        if node.dir { node.open = !node.open } else { *clicked = Some(node.path.clone()) }
    }
    resp.context_menu(|ui| {
        if let Some(action) = file_menu(ui, depth == 0) {
            *menu = Some((node.path.clone(), action));
        }
    });
    if node.dir && node.open {
        for child in node.children().iter_mut().filter(|c| !HIDDEN.contains(&c.name.as_str())) {
            tree_rows(ui, child, depth + 1, active, vcs, clicked, menu);
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
