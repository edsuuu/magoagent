//! Alterações: the files changed against the HEAD in the folders of the canvas's file trees (the git the trees already
//! read, filetree/view.rs), a group per folder with the agents working in it, each file with its lines added and
//! removed. A click shows its diff, in the code's colors; "Abrir na Árvore" opens it in the tree's editor, as the
//! tree's own Alterações panel does. A worktree picker on top (worktrees.rs), the list in a box of ten rows under a
//! search field (list.rs), the diff colored with lines to pick (diff.rs) and the review of what was picked, sent to an
//! agent (review.rs; the MCP's in actions.rs).
// ponytail: no "Descartar": the app has no action that throws a whole file's changes away (only
// file_tree's revert_hunk, one range at a time); add one to filetree/ and its button here.

mod actions;
mod comments;
pub(crate) mod diff;
pub(crate) mod list;
pub(crate) mod review;
pub(crate) mod worktrees;

use super::live;
use crate::app::filetree::diff::head_text;
use crate::app::filetree::view::TreeGit;
use crate::editor::{Status, highlight};
use crate::prelude::*;
pub(crate) use diff::{DiffLine, diff_lines};
use list::parse_changed_lines;
pub(crate) use review::{Review, ReviewComment, comment_on, rows_for, works_in};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
pub(crate) use worktrees::Worktree;

/// How often the counts of lines are read again while the panel is open, in seconds.
const RECOUNT: f64 = 5.0;

/// The lines added and removed of each changed file, by `key`.
type Counts = HashMap<String, (usize, usize)>;
/// The changed lines of each file (added and removed, as they read), by `key`: what the search looks into.
pub(crate) type Changed = HashMap<String, Vec<String>>;

#[derive(Default)]
pub(crate) struct ChangesPane {
    counts: Counts,
    changed: Changed,
    reading: Option<Receiver<Reading>>,
    pub(crate) read_at: Option<f64>,
    /// The file whose diff shows: its tree's id (empty: a worktree no tree shows) and its path, and the diff.
    pub(crate) selected: Option<(String, PathBuf)>,
    pub(crate) diff: Vec<DiffLine>,
    /// The worktrees of the repositories shown, and the one picked (none: Todos, the trees' folders).
    pub(crate) worktrees: Vec<Worktree>,
    pub(crate) worktree: Option<PathBuf>,
    /// The picked worktree's git once read: its branch and its changed files.
    pub(crate) worktree_git: Option<(String, Vec<(PathBuf, Status)>)>,
    /// What the field over the list looks for: a file's name or a stretch of its changes.
    pub(crate) search: String,
    pub(crate) review: Review,
}

/// One folder's changes: its tree's id (empty: a worktree no tree shows), the header, the folder, the files.
pub(crate) struct ChangeGroup {
    pub(crate) tree: String,
    pub(crate) title: String,
    pub(crate) root: PathBuf,
    pub(crate) files: Vec<(PathBuf, Status)>,
}

/// What the thread read: for which worktree (none: the trees' folders), the counts, the changed lines, the worktrees
/// of the repositories and, for a worktree, its git.
struct Reading {
    worktree: Option<PathBuf>,
    counts: Counts,
    changed: Changed,
    worktrees: Vec<Worktree>,
    git: Option<(String, Vec<(PathBuf, Status)>)>,
}

/// A path as a key that is the same whichever slash git or the system wrote.
pub(crate) fn key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// A folder's last name (the whole path when it has none).
pub(crate) fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
}

/// `git diff --numstat` of `root`: added and removed lines by path (binary files have none).
pub(crate) fn parse_numstat(root: &Path, out: &str) -> Counts {
    out.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let (added, removed, path) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?);

            Some((key(&root.join(path)), (added, removed)))
        })
        .collect()
}

/// The counts and changed lines of each folder, read away from the frame: the diff against the HEAD, and the lines of
/// each new file.
fn count_lines(roots: Vec<(PathBuf, Vec<PathBuf>)>) -> (Counts, Changed) {
    let (mut counts, mut changed) = (Counts::new(), Changed::new());

    for (root, untracked) in roots {
        if let Ok(out) = crate::editor::run_git(&root, &["diff", "--numstat", "--relative", "HEAD"]) {
            counts.extend(parse_numstat(&root, &out));
        }

        let lines = ["-c", "core.quotePath=false", "diff", "--relative", "-U0", "--no-color", "--no-ext-diff", "HEAD"];

        if let Ok(out) = crate::editor::run_git(&root, &lines) {
            changed.extend(parse_changed_lines(&root, &out));
        }

        for file in untracked {
            if let Ok(text) = fs::read_to_string(&file) {
                counts.insert(key(&file), (text.lines().count(), 0));
                changed.insert(key(&file), list::kept_lines(text.lines()));
            }
        }
    }

    (counts, changed)
}

/// The thread's read: a worktree's own git (its status first, then its lines), else the trees' folders; and the
/// worktrees of the repositories of `folders`.
fn read_changes(roots: Vec<(PathBuf, Vec<PathBuf>)>, folders: Vec<PathBuf>, worktree: Option<PathBuf>) -> Reading {
    let (roots, git) = match &worktree {
        Some(path) => {
            let read = TreeGit::read(path);
            let files = crate::app::filetree::changes::sorted(path, &read.files);
            let untracked = files.iter().filter(|(_, s)| *s == Status::Untracked).map(|(p, _)| p.clone()).collect();

            (vec![(path.clone(), untracked)], Some((read.branch, files)))
        }
        None => (roots, None),
    };

    let (counts, changed) = count_lines(roots);

    Reading { worktree, counts, changed, worktrees: worktrees::read_worktrees(&folders), git }
}

impl DrawingApp {
    /// The changed files of each file tree's folder (once per folder), with the agents whose
    /// folder it is.
    pub(crate) fn change_groups(&self) -> Vec<ChangeGroup> {
        let rows = self.agent_rows();
        let mut groups: Vec<ChangeGroup> = Vec::new();

        for element in &self.scene.elements {
            let Some(spec) = &element.filetree else { continue };
            let root = spec.root_path();

            if groups.iter().any(|g| key(&g.root) == key(&root)) {
                continue;
            }

            let Some(view) = self.filetrees.views.get(&element.id).filter(|v| v.git.repo) else { continue };
            let agents: Vec<&str> = rows.iter().filter(|r| works_in(&r.spec.cwd, &root)).map(|r| r.spec.label()).collect();
            let name = folder_name(&root);
            let title = if agents.is_empty() { name } else { format!("{name} · {}", agents.join(", ")) };
            let files = crate::app::filetree::changes::sorted(&root, &view.git.files);

            groups.push(ChangeGroup { tree: element.id.clone(), title, root, files });
        }

        groups
    }

    /// What the panel shows: the trees' groups, or the picked worktree's own once read (none before).
    pub(crate) fn shown_groups(&self) -> Vec<ChangeGroup> {
        let changes = &self.panes.changes;
        let Some(path) = &changes.worktree else { return self.change_groups() };
        let Some((branch, files)) = &changes.worktree_git else { return Vec::new() };
        let tree = self.scene.elements.iter().find(|e| e.filetree.as_ref().is_some_and(|s| key(&s.root_path()) == key(path)));
        let agents: Vec<String> = self.agent_rows().iter().filter(|r| works_in(&r.spec.cwd, path)).map(|r| r.spec.label().to_string()).collect();
        let mut title = folder_name(path);

        if !branch.is_empty() {
            title += &format!(" · {branch}");
        }

        if !agents.is_empty() {
            title += &format!(" · {}", agents.join(", "));
        }

        vec![ChangeGroup { tree: tree.map(|e| e.id.clone()).unwrap_or_default(), title, root: path.clone(), files: files.clone() }]
    }

    /// The branch `root` is on: its worktree's, else its tree's.
    pub(crate) fn branch_of(&self, root: &Path) -> String {
        let changes = &self.panes.changes;

        if changes.worktree.as_deref().is_some_and(|w| key(w) == key(root))
            && let Some((branch, _)) = &changes.worktree_git
        {
            return branch.clone();
        }

        if let Some(worktree) = changes.worktrees.iter().find(|w| key(&w.path) == key(root)) {
            return worktree.branch.clone();
        }

        let tree = self.scene.elements.iter().find(|e| e.filetree.as_ref().is_some_and(|s| key(&s.root_path()) == key(root)));

        tree.and_then(|e| self.filetrees.views.get(&e.id)).map_or(String::new(), |v| v.git.branch.clone())
    }

    /// The lines added and removed of `path`, once counted.
    pub(crate) fn change_line_counts(&self, path: &Path) -> Option<(usize, usize)> {
        self.panes.changes.counts.get(&key(path)).copied()
    }

    /// The changed lines of `path`, once read.
    pub(crate) fn changed_lines(&self, path: &Path) -> Option<&Vec<String>> {
        self.panes.changes.changed.get(&key(path))
    }

    /// The changed files of all the trees' folders, for the Alterações button's badge.
    pub(crate) fn changed_file_count(&self) -> usize {
        self.change_groups().iter().map(|g| g.files.len()).sum()
    }

    pub(crate) fn changes_pane(&mut self, ui: &mut egui::Ui) {
        let colors = self.colors();
        let groups = self.shown_groups();

        self.recount_changes(ui.input(|i| i.time), &groups);
        self.worktree_switcher(ui);

        if groups.iter().all(|g| g.files.is_empty()) {
            let none = match &self.panes.changes.worktree {
                Some(_) if self.panes.changes.worktree_git.is_none() => "Lendo o worktree…",
                Some(_) => "Nenhuma alteração pendente neste worktree.",
                None if groups.is_empty() => "Nenhuma Árvore de Arquivos de um repositório git no canvas.",
                None => "Nenhuma alteração pendente.",
            };

            ui.label(egui::RichText::new(none).color(colors.muted));
        } else if let Some((tree, path)) = self.changes_list(ui, &groups).filter(|_| live(ui)) {
            self.select_change(&tree, &path);
        }

        if let Some((tree, path)) = self.panes.changes.selected.clone() {
            self.diff_view(ui, &path);
            let root = groups.iter().find(|g| g.files.iter().any(|(p, _)| *p == path)).map(|g| g.root.clone());

            ui.horizontal(|ui| {
                if ui.button("Abrir na Árvore").clicked() && live(ui) {
                    self.open_change_in_tree(&tree, root.as_deref(), &path);
                }

                // the diff closed: the list alone again (as a second click on the file)
                if ui.button("Fechar diff").clicked() && live(ui) {
                    self.select_change(&tree, &path);
                }
            });
        }

        if groups.iter().any(|g| !g.files.is_empty()) || !self.panes.changes.review.comments.is_empty() {
            self.review_section(ui, &groups);
        }
    }

    /// A file of the list clicked: its diff shows under it (a second click hides it).
    pub(crate) fn select_change(&mut self, tree: &str, path: &Path) {
        let changes = &mut self.panes.changes;

        changes.review.picked = None;

        if changes.selected.as_ref().is_some_and(|(_, p)| p == path) {
            (changes.selected, changes.diff) = (None, Vec::new());

            return;
        }

        changes.diff = file_diff(path);
        changes.selected = Some((tree.to_string(), path.to_path_buf()));
    }

    /// "Abrir na Árvore": the tree enlarged with the file's diff in its editor; a worktree no tree shows gets one first.
    pub(crate) fn open_change_in_tree(&mut self, tree: &str, root: Option<&Path>, path: &Path) {
        let mut index = self.scene.elements.iter().position(|e| e.id == tree);

        if index.is_none()
            && let Some(root) = root
        {
            match self.act_tree_create(&root.to_string_lossy()) {
                Ok(_) => index = self.scene.elements.iter().rposition(|e| e.filetree.as_ref().is_some_and(|s| key(&s.root_path()) == key(root))),
                Err(why) => self.toast(why),
            }
        }

        let Some(index) = index else { return };
        let tree = self.scene.elements[index].id.clone();

        self.show_tree(index);
        self.ft_open_change(&tree, path);
    }

    /// Every `RECOUNT` seconds (or on opening, or on another worktree), what the panel shows is read again on a thread
    /// of its own: the counts and changed lines, the worktrees and, for a worktree, its files.
    fn recount_changes(&mut self, now: f64, groups: &[ChangeGroup]) {
        let folders = self.worktree_folders();
        let changes = &mut self.panes.changes;

        match changes.reading.as_ref().map(Receiver::try_recv) {
            Some(Ok(reading)) => {
                changes.reading = None;

                // read for another worktree than the one picked since: read again
                if reading.worktree == changes.worktree {
                    (changes.counts, changes.changed, changes.worktree_git) = (reading.counts, reading.changed, reading.git);
                    changes.worktrees = reading.worktrees;
                } else {
                    changes.read_at = None;
                }
            }
            Some(Err(TryRecvError::Disconnected)) => changes.reading = None,
            _ => {}
        }

        if changes.reading.is_some() || changes.read_at.is_some_and(|at| now - at < RECOUNT) {
            return;
        }

        changes.read_at = Some(now);

        let roots: Vec<(PathBuf, Vec<PathBuf>)> = groups
            .iter()
            .filter(|g| !g.files.is_empty() && changes.worktree.is_none())
            .map(|g| (g.root.clone(), g.files.iter().filter(|(_, s)| *s == Status::Untracked).map(|(p, _)| p.clone()).collect()))
            .collect();

        let worktree = changes.worktree.clone();
        let (sender, receiver) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let _ = sender.send(read_changes(roots, folders, worktree));
        });

        changes.reading = Some(receiver);
    }
}

/// The diff of `path` against the HEAD, colored as its language.
pub(crate) fn file_diff(path: &Path) -> Vec<DiffLine> {
    let old = head_text(path).unwrap_or_default();
    let new = fs::read_to_string(path).unwrap_or_default().replace("\r\n", "\n");
    let mut lines = diff_lines(&old, &new);

    diff::color_lines(&mut lines, highlight::detect(path), &old, &new);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numstat_counts_by_path_and_skips_binaries() {
        let root = Path::new("/repo");
        let counts = parse_numstat(root, "3\t1\tsrc/main.rs\n-\t-\tlogo.png\n10\t0\tREADME.md\n");

        assert_eq!(counts.get(&key(&root.join("src").join("main.rs"))), Some(&(3, 1)));
        assert_eq!(counts.get(&key(&root.join("README.md"))), Some(&(10, 0)));
        assert_eq!(counts.len(), 2, "the binary file has no lines");
    }
}
