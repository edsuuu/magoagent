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
}

impl Ide {
    pub(super) fn project(&mut self, ui: &mut Ui) {
        tool_header(ui, "Projeto");
        let active = self.tabs.get(self.active).map(|t| t.path.clone());
        let mut clicked = None;
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(tree) = &mut self.tree {
                tree_rows(ui, tree, 0, active.as_deref(), &self.git.status, &mut clicked);
            }
        });
        if let Some(p) = clicked {
            self.open_file(p, None);
        }
    }
}

/// Hidden like PhpStorm's default "Ignored Files and Folders".
pub(super) const HIDDEN: &[&str] = &[".git", ".hg", ".svn", ".DS_Store", "Thumbs.db", "__pycache__"];

fn tree_rows(ui: &mut Ui, node: &mut TreeNode, depth: usize, active: Option<&Path>, vcs: &std::collections::HashMap<String, git::Status>, clicked: &mut Option<PathBuf>) {
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
    if node.dir && node.open {
        for child in node.children().iter_mut().filter(|c| !HIDDEN.contains(&c.name.as_str())) {
            tree_rows(ui, child, depth + 1, active, vcs, clicked);
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

    /// A press on card row `row`: a folder opens or closes, and a file comes back to open in the panel.
    pub(crate) fn card_click(&mut self, row: usize) -> Option<PathBuf> {
        // the root counts as row -1: its children are the rows
        let mut n = row + 1;
        let node = visible_mut(self.tree.as_mut()?, &mut n)?;
        if !node.dir {
            return Some(node.path.clone());
        }
        node.open = !node.open;
        if node.open {
            node.children();
        }
        None
    }
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
