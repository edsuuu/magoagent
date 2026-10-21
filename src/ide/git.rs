//! Git through the git CLI (PhpStorm shells out to git too): status, HEAD content, line diff,
//! commit/push/pull/checkout and the branch popup. The Commit window and the diff are in commit.rs.

use super::*;
use similar::{DiffTag, TextDiff};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflict,
}

impl Status {
    /// FILESTATUS_* colours from the Islands Dark scheme.
    pub fn color(self) -> Color32 {
        let v: u32 = match self {
            Status::Modified | Status::Renamed => 0x70AEFF,
            Status::Added => 0x73BD79,
            Status::Deleted => 0x6F737A,
            Status::Untracked => 0xE88F89,
            Status::Conflict => 0xDE6A66,
        };
        Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
}

pub struct Change {
    pub path: PathBuf,
    pub status: Status,
}

/// `git status --porcelain=v1 -z` -> changes. Renames carry the old path as an extra NUL field.
pub fn parse_status(root: &Path, out: &str) -> Vec<Change> {
    let mut v = vec![];
    let mut it = out.split('\0');
    while let Some(e) = it.next() {
        if e.len() < 4 {
            continue;
        }
        let (x, y) = (e.as_bytes()[0], e.as_bytes()[1]);
        let status = match (x, y) {
            (b'?', b'?') => Status::Untracked,
            (b'U', _) | (_, b'U') | (b'A', b'A') | (b'D', b'D') => Status::Conflict,
            (b'R' | b'C', _) => {
                it.next();
                Status::Renamed
            }
            (b'A', _) => Status::Added,
            (b'D', _) | (_, b'D') => Status::Deleted,
            _ => Status::Modified,
        };
        v.push(Change { path: root.join(&e[3..]), status });
    }
    v
}

pub fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = command("git").args(args).current_dir(root).output().map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn head_text(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
    git(root, &["show", &format!("HEAD:{rel}")]).ok().map(|s| s.replace("\r\n", "\n"))
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MarkKind {
    Added,
    Modified,
    Deleted,
}

/// Gutter change markers: [start, end) line ranges of the new text.
pub type Mark = (usize, usize, MarkKind);

pub fn line_marks(old: &str, new: &str) -> Vec<Mark> {
    TextDiff::from_lines(old, new)
        .ops()
        .iter()
        .filter_map(|op| {
            let (tag, _, n) = op.as_tag_tuple();
            match tag {
                DiffTag::Equal => None,
                DiffTag::Insert => Some((n.start, n.end, MarkKind::Added)),
                DiffTag::Delete => Some((n.start, n.start, MarkKind::Deleted)),
                DiffTag::Replace => Some((n.start, n.end, MarkKind::Modified)),
            }
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RowKind {
    Same,
    Insert,
    Delete,
    Change,
}

/// One aligned row of the side-by-side viewer (line indexes into old/new).
#[derive(Debug, PartialEq)]
pub struct DiffRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub kind: RowKind,
}

pub fn diff_rows(old: &str, new: &str) -> Vec<DiffRow> {
    let mut out = vec![];
    for op in TextDiff::from_lines(old, new).ops() {
        let (tag, o, n) = op.as_tag_tuple();
        let kind = match tag {
            DiffTag::Equal => RowKind::Same,
            DiffTag::Insert => RowKind::Insert,
            DiffTag::Delete => RowKind::Delete,
            DiffTag::Replace => RowKind::Change,
        };
        for i in 0..o.len().max(n.len()) {
            out.push(DiffRow { left: (i < o.len()).then(|| o.start + i), right: (i < n.len()).then(|| n.start + i), kind });
        }
    }
    out
}

pub struct Diff {
    pub old: Vec<String>,
    pub new: Vec<String>,
    pub rows: Vec<DiffRow>,
    /// first row of every changed block, for "next/previous difference"
    pub blocks: Vec<usize>,
    pub current: usize,
    pub scroll_to: Option<usize>,
    /// tab title given by Claude Code's openDiff
    pub title: Option<String>,
    /// pending Claude Code openDiff: reply channel + proposed text
    pub claude: Option<(std::sync::mpsc::Sender<Vec<String>>, String)>,
}

impl Diff {
    pub fn new(old: &str, new: &str) -> Self {
        let rows = diff_rows(old, new);
        let blocks = (0..rows.len()).filter(|&i| rows[i].kind != RowKind::Same && (i == 0 || rows[i - 1].kind == RowKind::Same)).collect::<Vec<_>>();
        let scroll_to = blocks.first().copied();
        Diff { old: old.lines().map(String::from).collect(), new: new.lines().map(String::from).collect(), rows, blocks, current: 0, scroll_to, title: None, claude: None }
    }
}

enum Msg {
    Status { repo: bool, changes: Vec<Change>, branches: Vec<String>, branch: String },
    Done(String, Result<String, String>),
}

#[derive(Default)]
pub struct Git {
    pub repo: bool,
    pub changes: Vec<Change>,
    /// path key -> status, including parent directories of changed files
    pub status: HashMap<String, Status>,
    pub branches: Vec<String>,
    pub message: String,
    pub amend: bool,
    /// tracked changes the user unticked / untracked files the user ticked
    pub(super) unchecked: HashSet<PathBuf>,
    pub(super) checked_untracked: HashSet<PathBuf>,
    pub(super) selected: Option<PathBuf>,
    rx: Option<Receiver<Msg>>,
    pub busy: Option<String>,
}

impl Git {
    pub(super) fn included(&self, c: &Change) -> bool {
        if c.status == Status::Untracked { self.checked_untracked.contains(&c.path) } else { !self.unchecked.contains(&c.path) }
    }
}

/// Gutter markers for a tab (HEAD vs buffer), cached by text hash. `root` is None outside a repo.
pub fn marks(root: Option<&Path>, tab: &mut Tab) -> Vec<Mark> {
    let Some(root) = root.filter(|_| tab.diff.is_none()) else { return vec![] };
    let head = tab.head.get_or_insert_with(|| head_text(root, &tab.path));
    let Some(head) = head else { return vec![] };
    let hash = hash(&tab.text, 0.0);
    if tab.marks.as_ref().is_none_or(|m| m.0 != hash) {
        tab.marks = Some((hash, line_marks(head, &tab.text)));
    }
    tab.marks.as_ref().unwrap().1.clone()
}

// ---------------------------------------------------------------- App glue

impl Ide {
    pub(crate) fn git_refresh(&mut self) {
        let Some(root) = self.root.clone() else { return };
        if self.git.busy.is_some() {
            return; // one channel for both: a refresh now would drop the running command's Done (Done refreshes anyway)
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let st = git(&root, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
            let msg = match st {
                Ok(out) => Msg::Status {
                    repo: true,
                    changes: parse_status(&root, &out),
                    branches: git(&root, &["branch", "--format=%(refname:short)"]).unwrap_or_default().lines().map(String::from).collect(),
                    branch: git(&root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default().trim().to_string(),
                },
                Err(_) => Msg::Status { repo: false, changes: vec![], branches: vec![], branch: String::new() },
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
        self.git.rx = Some(rx);
    }

    /// Runs git commands in order on a background thread, stopping at the first failure.
    pub(crate) fn git_run(&mut self, label: &str, cmds: Vec<Vec<String>>) {
        let Some(root) = self.root.clone() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let (ctx, label) = (self.ctx.clone(), label.to_string());
        self.git.busy = Some(format!("{label}…"));
        std::thread::spawn(move || {
            let mut res = Ok(String::new());
            for c in &cmds {
                let args: Vec<&str> = c.iter().map(String::as_str).collect();
                res = git(&root, &args);
                if res.is_err() {
                    break;
                }
            }
            let _ = tx.send(Msg::Done(label, res));
            ctx.request_repaint();
        });
        self.git.rx = Some(rx);
    }

    pub(crate) fn git_poll(&mut self) {
        let Some(rx) = &self.git.rx else { return };
        let Ok(msg) = rx.try_recv() else { return };
        self.git.rx = None;
        match msg {
            Msg::Status { repo, changes, branches, branch } => {
                self.git.repo = repo;
                self.git.status.clear();
                if let Some(root) = &self.root {
                    for c in &changes {
                        self.git.status.insert(path_key(&c.path), c.status);
                        // PhpStorm also tints directories that contain changes
                        let mut p = c.path.parent();
                        while let Some(d) = p.filter(|d| d.starts_with(root) && *d != root.as_path()) {
                            self.git.status.entry(path_key(d)).or_insert(Status::Modified);
                            p = d.parent();
                        }
                    }
                }
                self.git.changes = changes;
                self.git.branches = branches;
                if !branch.is_empty() {
                    self.branch = branch;
                }
            }
            Msg::Done(label, res) => {
                self.git.busy = None;
                match res {
                    Ok(_) => {
                        self.notify(format!("{label}: concluído"));
                        if label.starts_with("Commit") {
                            self.git.message.clear();
                            self.git.amend = false;
                        }
                        for tab in &mut self.tabs {
                            tab.head = None; // HEAD moved: reload the diff base
                        }
                    }
                    Err(e) => self.notify(format!("{label} falhou: {}", e.lines().last().unwrap_or(&e))),
                }
                self.git_refresh();
            }
        }
    }

    pub(crate) fn file_status(&self, p: &Path) -> Option<Status> {
        self.git.status.get(&path_key(p)).copied()
    }

    pub(crate) fn open_diff(&mut self, path: PathBuf) {
        let Some(root) = self.root.clone() else { return };
        if let Some(i) = self.tabs.iter().position(|t| t.path == path && t.diff.is_some()) {
            self.active = i;
            return;
        }
        let old = head_text(&root, &path).unwrap_or_default();
        let new = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        let mut tab = Tab::new(path, String::new(), false);
        tab.diff = Some(Box::new(Diff::new(&old, &new)));
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    pub(crate) fn branch_menu(&mut self, ui: &mut Ui) {
        ui.set_min_width(260.0);
        if menu_item(ui, "Atualizar projeto…", "Ctrl+T") {
            self.git_pull();
        }
        if menu_item(ui, "Commit…", "Ctrl+K") {
            self.left = Some(Left::Commit);
        }
        if menu_item(ui, "Push…", "Ctrl+Shift+K") {
            self.git_push();
        }
        ui.separator();
        ui.label(egui::RichText::new("Local").color(t::SECONDARY).small());
        let current = self.branch.clone();
        for b in self.git.branches.clone() {
            let label = if b == current { format!("★ {b}") } else { b.clone() };
            if ui.button(label).clicked() && b != current {
                self.save_all();
                self.git_run(&format!("Checkout {b}"), vec![vec!["checkout".into(), b.clone()]]);
                ui.close();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_parsing() {
        let root = Path::new("/r");
        let out = " M app/User.php\0?? new.txt\0R  b.php\0a.php\0D  gone.php\0UU c.php\0";
        let v: Vec<_> = parse_status(root, out).into_iter().map(|c| (c.path, c.status)).collect();
        assert_eq!(v, vec![
            (root.join("app/User.php"), Status::Modified),
            (root.join("new.txt"), Status::Untracked),
            (root.join("b.php"), Status::Renamed),
            (root.join("gone.php"), Status::Deleted),
            (root.join("c.php"), Status::Conflict),
        ]);
    }

    #[test]
    fn marks_and_rows() {
        let old = "a\nb\nc\nd\n";
        let new = "a\nB\nc\nx\nd\n";
        assert_eq!(line_marks(old, new), vec![(1, 2, MarkKind::Modified), (3, 4, MarkKind::Added)]);
        assert_eq!(line_marks("a\nb\n", "a\n"), vec![(1, 1, MarkKind::Deleted)]);
        let rows = diff_rows(old, new);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[1], DiffRow { left: Some(1), right: Some(1), kind: RowKind::Change });
        assert_eq!(rows[3], DiffRow { left: None, right: Some(3), kind: RowKind::Insert });
        assert_eq!(Diff::new(old, new).blocks, vec![1, 3]);
    }
}
