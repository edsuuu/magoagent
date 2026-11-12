//! File tree nodes on the canvas: one folder's tree with its git marks, like the reference app's.
//! The node is a rectangle with a `FileTreeSpec`; what lives only while the app runs (open
//! folders, scroll, the listings, read off the UI thread) is a `TreeView` in `FileTrees`. The
//! node's face is in filetree_ui.rs, the file operations in filetree_ops.rs.

use crate::app::chat_direct::folder;
use crate::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub(crate) use crate::app::filetree_view::*;

/// A file tree node's size when a click makes it, and the smallest it can be resized to.
pub(crate) const FILETREE_SIZE: Vec2 = Vec2::new(300.0, 300.0);
pub(crate) const FILETREE_MIN: Vec2 = Vec2::new(300.0, 300.0);
/// A row, and the header and search bar (`h-10`: 2.5 rem at the reference app's 13 px).
pub(crate) const FT_ROW: f32 = 24.0;
pub(crate) const FT_BAR: f32 = 32.5;

/// How a folder's entries are ordered (folders always first).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SortBy {
    #[default]
    Name,
    Kind,
    DateModified,
    DateCreated,
    Size,
}

impl SortBy {
    pub(crate) const ALL: [SortBy; 5] = [SortBy::Name, SortBy::Kind, SortBy::DateModified, SortBy::DateCreated, SortBy::Size];

    pub(crate) fn label(self) -> &'static str {
        match self {
            SortBy::Name => "Nome",
            SortBy::Kind => "Tipo",
            SortBy::DateModified => "Data de Modificação",
            SortBy::DateCreated => "Data de Criação",
            SortBy::Size => "Tamanho",
        }
    }

    /// The word the MCP tool uses.
    pub(crate) fn key(self) -> &'static str {
        match self {
            SortBy::Name => "name",
            SortBy::Kind => "kind",
            SortBy::DateModified => "modified",
            SortBy::DateCreated => "created",
            SortBy::Size => "size",
        }
    }
}

/// A folder's tree on the canvas: the root, the folder shown now (the person went into it) and
/// how it is listed.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct FileTreeSpec {
    pub(crate) root: String,
    /// The folder shown, inside `root`; empty is the root itself.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(crate) current: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) hide_hidden: bool,
    pub(crate) sort: SortBy,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) reversed: bool,
    /// The editor pane is shown, and how wide the tree is beside it (None: all but the default pane).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) editor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tree_width: Option<f32>,
    /// The "Banco de Dados" panel is shown over the file rows.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) db: bool,
    /// The "Alterações" panel (the files changed against the HEAD) is shown over the file rows.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) changes: bool,
}

impl FileTreeSpec {
    pub(crate) fn root_path(&self) -> PathBuf {
        folder(&self.root).unwrap_or_default()
    }

    /// The folder the rows start from.
    pub(crate) fn shown(&self) -> PathBuf {
        if self.current.is_empty() { self.root_path() } else { PathBuf::from(&self.current) }
    }

    /// The header's name: the shown folder's.
    pub(crate) fn name(&self) -> String {
        let shown = self.shown();
        shown.file_name().map_or_else(|| shown.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
    }
}

/// The new / edit file tree dialog: the root folder to show.
#[derive(Clone, Debug)]
pub(crate) struct FileTreeDialog {
    /// The tree being edited; None makes one in `at`.
    pub(crate) target: Option<String>,
    pub(crate) at: Rect,
    pub(crate) root: String,
}

/// What a file operation asks first, in a small dialog.
#[derive(Clone, Debug)]
pub(crate) enum TreeAsk {
    /// A name for a new file or folder in `at`, or the new name of `at` (`Rename`).
    Name { tree: String, at: PathBuf, kind: NameKind, text: String },
    /// Send these to the trash, once confirmed.
    Trash { tree: String, paths: Vec<PathBuf> },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum NameKind {
    NewFile,
    NewFolder,
    Rename,
}

/// Every file tree's live view, and what is asked of the person.
#[derive(Default)]
pub(crate) struct FileTrees {
    pub(crate) views: HashMap<String, TreeView>,
    pub(crate) dialog: Option<FileTreeDialog>,
    pub(crate) ask: Option<TreeAsk>,
    /// Copied or cut paths (cut moves them on paste).
    pub(crate) clipboard: Option<(Vec<PathBuf>, bool)>,
    /// The editor of each tree that opened a file (filetree_pane.rs), and the context they draw in.
    pub(crate) editors: HashMap<String, crate::app::filetree_pane::Editor>,
    pub(crate) ctx: Option<Context>,
    /// The "Conflitos" window and the merge window (ide/mergeview.rs) of the tree that asked for them.
    pub(crate) conflicts: crate::ide::Conflicts,
    /// The database panel of each tree (its sources, sessions and what it asked), and where it saves them (None: the app's own file).
    pub(crate) dbs: HashMap<String, crate::db::Db>,
    pub(crate) db_store: Option<PathBuf>,
}

impl DrawingApp {
    /// Click: a file tree centered there; drag: that box. Asks for the folder first.
    pub(crate) fn open_filetree_dialog(&mut self, start: Pos2, end: Pos2) {
        let at = if start.distance(end) * self.zoom < 10.0 {
            Rect::from_center_size(start, FILETREE_SIZE)
        } else {
            let r = Rect::from_two_pos(start, end);
            Rect::from_min_size(r.min, r.size().max(FILETREE_MIN))
        };
        self.filetrees.dialog = Some(FileTreeDialog { target: None, at, root: self.new_folder() });
        self.tool = Tool::Select;
    }

    /// Makes (or re-roots) the tree the dialog describes; a folder that isn't there keeps the dialog open.
    pub(crate) fn confirm_filetree_dialog(&mut self) {
        let Some(FileTreeDialog { target, at, root }) = self.filetrees.dialog.clone() else { return };
        let root = root.trim().to_string();
        if let Err(why) = self.check_root(&root) {
            self.toast(match target {
                Some(_) => format!("Não foi possível alterar o local: {why}"),
                None => format!("Não foi possível criar a árvore de arquivos: {why}"),
            });
            return;
        }
        self.filetrees.dialog = None;
        self.prefs.folders.remember(&root);
        self.snapshot();
        if let Some(id) = target {
            self.ft_set_root(&id, root);
            return;
        }
        let mut node = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        // what exports and the AI's view show; the app paints the tree itself
        node.style = node_style(node.style);
        node.filetree = Some(FileTreeSpec { root, ..Default::default() });
        self.scene.elements.push(node);
        self.selected = vec![self.scene.elements.len() - 1];
    }

    fn check_root(&self, root: &str) -> Result<(), String> {
        match folder(root) {
            Some(p) if p.is_dir() => Ok(()),
            _ => Err("a pasta não existe.".into()),
        }
    }

    /// Points tree `id` at another root.
    pub(crate) fn ft_set_root(&mut self, id: &str, root: String) {
        if let Some(spec) = self.scene.elements.iter_mut().find(|e| e.id == id).and_then(|e| e.filetree.as_mut()) {
            (spec.root, spec.current) = (root, String::new());
        }
        if let Some(view) = self.filetrees.views.get_mut(id) {
            *view = TreeView::new();
        }
    }

    /// A view for each file tree node, and the readers' news; views of nodes that went are dropped.
    pub(crate) fn sync_filetrees(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        self.filetrees.ctx = Some(ctx.clone());
        self.filetrees.editors.retain(|id, _| self.scene.elements.iter().any(|e| &e.id == id && e.filetree.is_some()));
        self.filetrees.dbs.retain(|id, _| self.scene.elements.iter().any(|e| &e.id == id && e.filetree.is_some()));
        for db in self.filetrees.dbs.values_mut() {
            db.background();
        }
        for ed in self.filetrees.editors.values_mut() {
            ed.poll_sheets(ctx);
        }
        let views = &mut self.filetrees.views;
        views.retain(|id, _| self.scene.elements.iter().any(|e| &e.id == id && e.filetree.is_some()));
        for e in &self.scene.elements {
            let Some(spec) = &e.filetree else { continue };
            let view = views.entry(e.id.clone()).or_insert_with(TreeView::new);
            view.poll(spec, ctx, now);
            if view.dirty {
                view.rebuild(spec);
            }
        }
    }

    pub(crate) fn ft_spec(&self, id: &str) -> Option<&FileTreeSpec> {
        self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.filetree.as_ref())
    }

    pub(crate) fn ft_spec_mut(&mut self, id: &str) -> Option<&mut FileTreeSpec> {
        self.scene.elements.iter_mut().find(|e| e.id == id).and_then(|e| e.filetree.as_mut())
    }

    /// Opens or closes a folder of tree `id`.
    pub(crate) fn ft_toggle(&mut self, id: &str, path: &Path) {
        let Some(view) = self.filetrees.views.get_mut(id) else { return };
        if !view.expanded.remove(path) {
            view.expanded.insert(path.to_path_buf());
            // read before the next frame shows it
            view.read_now(path);
            view.stale = true;
        }
        view.dirty = true;
    }

    /// Goes into folder `path` (inside the root); "Voltar" goes up again.
    pub(crate) fn ft_navigate(&mut self, id: &str, path: PathBuf) {
        let root = self.ft_spec(id).map(FileTreeSpec::root_path).unwrap_or_default();
        if !path.starts_with(&root) || !path.is_dir() {
            return;
        }
        if let Some(spec) = self.ft_spec_mut(id) {
            spec.current = if path == root { String::new() } else { path.to_string_lossy().into_owned() };
        }
        if let Some(view) = self.filetrees.views.get_mut(id) {
            (view.forward, view.selected, view.scroll, view.stale, view.dirty) = (Vec::new(), None, 0.0, true, true);
        }
    }

    /// The folder above the shown one, while it is inside the root; "Avançar" comes back.
    pub(crate) fn ft_back(&mut self, id: &str) {
        let Some(spec) = self.ft_spec(id) else { return };
        let (root, shown) = (spec.root_path(), spec.shown());
        let Some(parent) = shown.parent().filter(|p| shown != root && p.starts_with(&root)).map(Path::to_path_buf) else { return };
        let mut forward = self.filetrees.views.get(id).map(|v| v.forward.clone()).unwrap_or_default();
        forward.push(shown);
        self.ft_navigate(id, parent);
        if let Some(view) = self.filetrees.views.get_mut(id) {
            view.forward = forward;
        }
    }

    pub(crate) fn ft_forward(&mut self, id: &str) {
        let Some(next) = self.filetrees.views.get_mut(id).and_then(|v| v.forward.pop()) else { return };
        let rest = self.filetrees.views.get(id).map(|v| v.forward.clone()).unwrap_or_default();
        self.ft_navigate(id, next);
        if let Some(view) = self.filetrees.views.get_mut(id) {
            view.forward = rest;
        }
    }

    pub(crate) fn ft_collapse_all(&mut self, id: &str) {
        if let Some(view) = self.filetrees.views.get_mut(id) {
            (view.dirty, view.scroll) = (true, 0.0);
            view.expanded.clear();
        }
    }

    /// Double-click, Enter or "Abrir": a folder opens or closes, a file opens (kept) in the editor
    /// pane of the node, which grows to show it.
    pub(crate) fn ft_activate(&mut self, id: &str, path: PathBuf) {
        if path.is_dir() {
            return self.ft_toggle(id, &path);
        }
        // a SQLite file is a data source: it joins the Banco de Dados panel
        if crate::db::sqlite_file(&path) {
            return self.ft_db_add(id, path);
        }
        // a file left in conflict by a merge opens in the merge window, mine | result | theirs
        let root = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.filetree.as_ref()).map(|t| t.root_path());
        let conflicted = root.as_ref().is_some_and(|root| {
            crate::ide::merge::unmerged(root)
                .is_ok_and(|files| files.iter().any(|(f, _)| crate::ide::lsp::path_key(&root.join(f)) == crate::ide::lsp::path_key(&path)))
        });
        if let (true, Some(root)) = (conflicted, root) {
            if let Err(why) = self.filetrees.conflicts.merge(&root, &path) {
                self.toast(why);
            }
            return;
        }
        self.ft_open(id, path, true);
    }

    /// Marks tree `id` to be read again at once (something changed on disk by our hand).
    pub(crate) fn ft_refresh(&mut self, id: &str) {
        if let Some(view) = self.filetrees.views.get_mut(id) {
            (view.stale, view.dirty) = (true, true);
        }
    }

    /// The tree an agent means: by id, root folder's name or its path; unnamed, the selected one or the only one.
    pub(crate) fn tree_named(&self, name: &str) -> Result<String, String> {
        let trees: Vec<&Element> = self.scene.elements.iter().filter(|e| e.filetree.is_some()).collect();
        let wanted = name.trim().trim_end_matches(['/', '\\']).to_lowercase();
        let pick = if wanted.is_empty() {
            let selected = self.selected.iter().filter_map(|i| self.scene.elements.get(*i)).find(|e| e.filetree.is_some());
            selected.or(if trees.len() == 1 { trees.first().copied() } else { None })
        } else {
            trees.iter().copied().find(|e| {
                let spec = e.filetree.as_ref().unwrap();
                e.id == name.trim() || spec.name().to_lowercase() == wanted || spec.root.trim_end_matches(['/', '\\']).to_lowercase() == wanted
            })
        };
        pick.map(|e| e.id.clone())
            .ok_or_else(|| if trees.is_empty() { "Não há árvore de arquivos no canvas.".to_string() } else { "Diga qual árvore de arquivos.".to_string() })
    }
}
