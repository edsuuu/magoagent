//! The IDE node's editor: a port of Ilha, the user's PhpStorm-like IDE. One `Ide` per IDE node,
//! alive while the app runs; the canvas glue (app/ide.rs) creates it, runs `background` every
//! frame and shows `ui` while the node is enlarged.
//! Ilha's terminal and Claude Code tool windows are the canvas terminals roped to the node.

use chrome::{menu_item, named, row, tool_header};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, Frame, Id, Key, KeyboardShortcut, Margin, Modifiers, Order, Rect, Sense, Shape, Stroke, TextEdit, Ui, pos2,
    text::CCursor, text::LayoutJob, text_selection::CCursorRange, vec2,
};
pub(crate) use fileops::{FILE_MENU, FileAction, FileAsk, file_menu};
use highlight::Lang;
use lsp::path_key;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant, SystemTime};
use text::*;
use theme as t;
use tree::TreeNode;

mod branches;
pub(crate) mod card;
pub(crate) mod chrome;
mod claude;
pub(crate) mod coderef;
mod commit;

mod dbhost;
mod diffbar;
mod diffbase;
mod diffview;
mod docs;
pub(crate) mod editor;
pub(crate) mod excluded;
pub(crate) mod fileops;
pub(crate) mod find;
mod git;
#[cfg(test)]
mod gittest;
pub(crate) mod highlight;
pub(crate) mod hints;
mod history;
pub(crate) mod icons;
pub(crate) mod index;
mod inspection;
mod langs;
pub(crate) mod lsp;
pub(crate) mod media;
pub(crate) mod merge;
#[cfg(test)]
pub(crate) mod mergetest;
mod mergeview;
pub(crate) use mergeview::Conflicts;
mod search;
mod stash;
mod status;
pub(crate) mod text;
pub(crate) mod textindex;
pub(crate) mod theme;
pub(crate) mod tree;
mod unified;

pub(crate) use branches::more_git_action;
pub(crate) use find::project_search;
// git for the rest of the app (an agent's worktree, app/environments.rs)
pub(crate) use git::git as run_git;
// what the file tree node marks its rows with (app/filetree.rs)
pub(crate) use git::{DiffRow, RowKind, Status, diff_rows, parse_blame, parse_status, word_marks};
// the unified viewer's rows (app/filetree/compare.rs)
pub(crate) use unified::unified_rows;
mod files;
mod run;
mod state;
#[cfg(test)]
mod tests;
pub(crate) use state::*;

/// `Command` that never flashes a console window on Windows.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[cfg_attr(not(windows), expect(unused_mut))]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

impl Ide {
    /// The IDE of node `key` on `folder` (`~` is the home folder).
    pub(crate) fn new(ctx: egui::Context, key: &str, folder: &str) -> Self {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default();
        let root = match folder.trim().strip_prefix('~') {
            Some(rest) => home.join(rest.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(folder.trim()),
        };
        let mut ide = Ide {
            ctx,
            id: Id::unique(("ide", key)),
            folder: folder.to_string(),
            root: None,
            tree: None,
            files: vec![],
            files_rx: None,
            branch: String::new(),
            tabs: vec![],
            active: 0,
            left: Some(Left::Project),
            claude: None,
            claude_sel: None,
            git: git::Git::default(),
            toast: None,
            notice: None,
            bottom: None,
            db: Default::default(),
            palette: None,
            find_files: None,
            find_results: None,
            excluded: excluded::Excluded::default(),
            text_index: Default::default(),
            text_rx: None,
            text_gen: 0,
            files_gen: 0,
            indexed_at: Instant::now(),
            index: index::Index::default(),
            index_rx: None,
            index_gen: 0,
            code_vision: true,
            blame: Default::default(),
            blame_rx: None,
            blame_gen: 0,
            find: None,
            find_focus: false,
            insp: None,
            doc: None,
            status: String::new(),
            font_size: t::EDITOR_SIZE,
            unified_diff: false,
            focused: true,
            shift_tap: None,
            last_disk_check: Instant::now(),
            back: vec![],
            forward: vec![],
            decl_rx: None,
            decl_word: None,
            usage_word: String::new(),
            sig_line: None,
            rect: Rect::NOTHING,
            wants_canvas: false,
            wants_claude: false,
            to_agent: Vec::new(),
            code_comment: None,
            note_req: None,
            has_agent: false,
            card_scroll: 0.0,
            panel_scroll: 0.0,
            card_press: None,
            card_sel: None,
            tree_keys: false,
            file_ask: None,
            panel_mark: None,
            panel_job: Default::default(),
            // beside the drawing, like the preferences and the chats: MAGOAGENT_SCENE moves it too
            recovery: if cfg!(test) { None } else { crate::autosave_path().map(|p| p.with_file_name("recuperados")) },
            close_warned: false,
        };
        if root.is_dir() {
            ide.open_folder(root);
        } else {
            ide.status = format!("Pasta não encontrada: {}", root.display());
        }
        ide
    }

    fn open_folder(&mut self, root: PathBuf) {
        let mut tree = TreeNode::new(root.clone(), true);
        tree.open = true;
        tree.children(); // the card lists them
        self.tree = Some(tree);
        self.branch = std::fs::read_to_string(root.join(".git/HEAD"))
            .ok()
            .map(|h| h.trim().trim_start_matches("ref: refs/heads/").chars().take(40).collect())
            .unwrap_or_default();
        self.excluded = excluded::Excluded::load(&root);
        self.reload_files(&root);
        self.insp = Some(lsp::Inspections::new(self.ctx.clone(), root.clone()));
        self.claude = claude::Claude::start(self.ctx.clone(), &root).map_err(|e| self.status = format!("Integração com o Claude Code: {e}")).ok();
        self.root = Some(root);
        self.git_refresh();
    }

    /// Whether the file list or an index is still being built (Search Everywhere says so).
    pub(crate) fn indexing(&self) -> bool {
        self.files_rx.is_some() || self.index_rx.is_some() || self.text_rx.is_some()
    }

    /// Lists the project's files again (Go to File, the index) on a thread.
    fn reload_files(&mut self, root: &Path) {
        let (tx, rx) = std::sync::mpsc::channel();
        let (r, ex, ctx) = (root.to_path_buf(), self.excluded.clone(), self.ctx.clone());
        self.indexed_at = Instant::now();
        std::thread::spawn(move || {
            let _ = tx.send(tree::list_files_with(&r, &ex));
            ctx.request_repaint();
        });
        self.files_rx = Some(rx);
    }

    /// Env for terminals roped to this IDE, so `claude` started there connects to it.
    pub(crate) fn claude_env(&self) -> Vec<(String, String)> {
        self.claude.as_ref().map(|c| c.env()).unwrap_or_default()
    }

    pub(crate) fn claude_connected(&self) -> bool {
        self.claude.as_ref().is_some_and(|c| c.connected())
    }
}
