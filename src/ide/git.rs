//! Git through the git CLI (PhpStorm shells out to git too): status, HEAD content, line diff,
//! commit/push/pull/checkout and the branch popup. The Commit window and the diff are in commit.rs.

use super::*;
use similar::{ChangeTag, DiffTag, TextDiff};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

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

/// An edit made in a diff's right side.
pub enum DiffEdit {
    /// » on a block (true: Ctrl, keep yours and add HEAD's after)
    Revert(usize, bool),
    /// a line typed over
    Line(usize, String),
    Undo,
    Redo,
}

/// One aligned row of the side-by-side viewer (line indexes into old/new).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiffRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub kind: RowKind,
}

/// `new` with its `block`-th change (counted like `Diff::blocks`) put back as it is in `old`:
/// PhpStorm's » between the two sides of a diff. `append` (Ctrl+») keeps the new lines and puts
/// the old ones after them.
pub fn revert(old: &str, new: &str, block: usize, append: bool) -> String {
    // the differ's own lines (a lone \r ends one too), so the ops index the right slices
    let diff = TextDiff::from_lines(old, new);
    let (o, n) = (diff.old_slices(), diff.new_slices());
    let (mut out, mut seen, mut in_change) = (String::new(), 0, false);
    let mut tail = String::new(); // the old lines of an appended block, after its new ones
    for op in diff.ops() {
        let (tag, a, b) = op.as_tag_tuple();
        if tag == DiffTag::Equal {
            seen += in_change as usize;
            in_change = false;
            out.push_str(&tail);
            tail.clear();
            out.extend(n[b].iter().copied());
        } else {
            in_change = true;
            let target = seen == block;
            if target && append {
                let old_lines = &o[a];
                out.extend(n[b].iter().copied());
                if !old_lines.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                tail.extend(old_lines.iter().copied());
            } else {
                out.extend(if target { &o[a] } else { &n[b] }.iter().copied());
            }
        }
    }
    out.push_str(&tail);
    out
}

/// Byte ranges of the words that differ between two paired lines: (old side, new side).
pub fn word_marks(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let (mut o, mut n, mut at_o, mut at_n) = (vec![], vec![], 0, 0);
    for c in TextDiff::from_words(old, new).iter_all_changes() {
        let len = c.value().len();
        match c.tag() {
            ChangeTag::Equal => {
                at_o += len;
                at_n += len;
            }
            ChangeTag::Delete => {
                o.push(at_o..at_o + len);
                at_o += len;
            }
            ChangeTag::Insert => {
                n.push(at_n..at_n + len);
                at_n += len;
            }
        }
    }
    (o, n)
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
    /// what the person wants to say about this change, sent to the agent
    pub comment: String,
    /// HEAD's text this diff is against: the buffer's diff is recomputed from it on every edit
    pub base: String,
    /// the buffer's text this diff was last computed from (a hash), so an edit elsewhere shows here
    pub seen: u64,
    /// the buffer before each » or line edit made here, newest last, for ⌘Z; ⌘⇧Z walks `redo` back
    pub undo: Vec<String>,
    pub redo: Vec<String>,
    /// the right-side line being typed into: its index and its text so far
    pub editing: Option<(usize, String)>,
}

impl Diff {
    /// The current block's lines (1-based, inclusive) and their text: what a comment about "this
    /// change" points at. The new side, or the old side when the block only removes lines
    /// (`removed`).
    pub fn current_lines(&self) -> Option<(usize, usize, Vec<&str>, bool)> {
        let start = *self.blocks.get(self.current)?;
        let block: Vec<&DiffRow> = self.rows[start..].iter().take_while(|r| r.kind != RowKind::Same).collect();
        let (lines, text, removed): (Vec<usize>, &[String], bool) = match block.iter().filter_map(|r| r.right).collect::<Vec<_>>() {
            l if !l.is_empty() => (l, &self.new, false),
            _ => (block.iter().filter_map(|r| r.left).collect(), &self.old, true),
        };
        let (first, last) = (*lines.first()?, *lines.last()?);
        Some((first + 1, last + 1, lines.iter().filter_map(|&l| text.get(l).map(String::as_str)).collect(), removed))
    }

    /// The rows of block `k`: from its first row up to the next unchanged one.
    pub fn block_rows(&self, k: usize) -> Range<usize> {
        let start = self.blocks[k];
        let end = self.rows[start..].iter().position(|r| r.kind == RowKind::Same).map_or(self.rows.len(), |p| start + p);
        start..end
    }

    pub fn new(old: &str, new: &str) -> Self {
        let rows = diff_rows(old, new);
        let blocks = (0..rows.len()).filter(|&i| rows[i].kind != RowKind::Same && (i == 0 || rows[i - 1].kind == RowKind::Same)).collect::<Vec<_>>();
        let scroll_to = blocks.first().copied();
        Diff { old: old.lines().map(String::from).collect(), new: new.lines().map(String::from).collect(), rows, blocks, current: 0, scroll_to, title: None, claude: None, comment: String::new(), base: old.to_string(), seen: 0, undo: Vec::new(), redo: Vec::new(), editing: None }
    }

    /// The rows and blocks again after the buffer became `new`; the current block stays if it exists.
    /// What the buffer `text` becomes after `edit` made in the diff, keeping the ⌘Z / ⌘⇧Z history;
    /// None when nothing changes.
    pub fn apply(&mut self, text: &str, edit: DiffEdit) -> Option<String> {
        let next = match edit {
            DiffEdit::Revert(k, append) => revert(&self.base, text, k, append),
            DiffEdit::Line(n, line) => {
                let mut lines: Vec<&str> = text.split('\n').collect();
                if lines.get(n).is_none_or(|l| *l == line) {
                    return None;
                }
                lines[n] = &line;
                lines.join("\n")
            }
            DiffEdit::Undo => return self.undo.pop().inspect(|_| self.redo.push(text.to_string())),
            DiffEdit::Redo => return self.redo.pop().inspect(|_| self.undo.push(text.to_string())),
        };
        if next == text {
            return None;
        }
        self.undo.push(text.to_string());
        self.redo.clear();
        Some(next)
    }

    pub fn recompute(&mut self, new: &str) {
        let fresh = Diff::new(&self.base, new);
        self.current = self.current.min(fresh.blocks.len().saturating_sub(1));
        (self.old, self.new, self.rows, self.blocks) = (fresh.old, fresh.new, fresh.rows, fresh.blocks);
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
    let Some(root) = root.filter(|_| tab.is_text()) else { return vec![] };
    let head = tab.head.get_or_insert_with(|| head_text(root, &tab.path));
    let Some(head) = head else { return vec![] };
    let hash = hash(&tab.text, 0.0);
    if tab.marks.as_ref().is_none_or(|m| m.0 != hash) {
        tab.marks = Some((hash, line_marks(head, &tab.text)));
    }
    tab.marks.as_ref().unwrap().1.clone()
}

/// The author of each line from `git blame --porcelain` (None for uncommitted lines). A commit's
/// author is printed only with its first line, so names are kept per sha.
pub fn parse_blame(out: &str) -> Vec<Option<String>> {
    let mut names: HashMap<&str, &str> = HashMap::new();
    let mut shas: Vec<&str> = vec![];
    let mut sha = "";
    for l in out.lines() {
        if let Some(name) = l.strip_prefix("author ") {
            names.insert(sha, name);
        } else if l.starts_with('\t') {
            shas.push(sha);
        } else if let Some((first, _)) = l.split_once(' ')
            && first.len() == 40
            && first.bytes().all(|b| b.is_ascii_hexdigit())
        {
            sha = first;
        }
    }
    shas.iter().map(|s| if s.bytes().all(|b| b == b'0') { None } else { names.get(s).map(|n| n.to_string()) }).collect()
}

// ---------------------------------------------------------------- App glue

impl Ide {
    /// Starts `git blame` for the active file unless it has an answer; one run at a time. The answer
    /// goes to `blame` (empty outside git or for an uncommitted file).
    pub(crate) fn blame_request(&mut self) {
        let Some(tab) = self.tabs.get(self.active) else { return };
        if !self.git.repo || self.blame_rx.is_some() || self.blame.contains_key(&tab.path) || tab.lang.kind != highlight::Kind::Code || tab.diff.is_some() || tab.sheet.is_some() {
            return;
        }
        let (Some(root), path) = (self.root.clone(), tab.path.clone()) else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let rel = path.strip_prefix(&root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            let lines = git(&root, &["blame", "--porcelain", "--", rel.as_str()]).map(|out| parse_blame(&out)).unwrap_or_default();
            let _ = tx.send((path, lines));
            ctx.request_repaint();
        });
        self.blame_rx = Some(rx);
    }

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

    /// The file's own tab, whose text the diff's right side edits; opened again when it was closed.
    pub(crate) fn buffer_tab(&mut self, path: &Path) -> Option<usize> {
        if let Some(i) = self.tabs.iter().position(|t| t.is_text() && t.path == path) {
            return Some(i);
        }
        let (at, n) = (self.active, self.tabs.len());
        self.open_file(path.to_path_buf(), None);
        self.active = at;
        (self.tabs.len() > n).then_some(n)
    }

    pub(crate) fn open_diff(&mut self, path: PathBuf) {
        let Some(root) = self.root.clone() else { return };
        if let Some(i) = self.tabs.iter().position(|t| t.path == path && t.diff.is_some()) {
            self.active = i;
            return;
        }
        let Some(buffer) = self.buffer_tab(&path) else { return };
        let (old, new) = (head_text(&root, &path).unwrap_or_default(), self.tabs[buffer].text.clone());
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
    fn a_comment_points_at_the_current_change() {
        let diff = Diff::new("a\nb\nc\nd\n", "a\nB\nB2\nc\nd\nE\n");
        // first change: line 2 became two lines (2–3)
        assert_eq!(diff.current_lines(), Some((2, 3, vec!["B", "B2"], false)));
        let next = Diff { current: 1, ..Diff::new("a\nb\nc\nd\n", "a\nB\nB2\nc\nd\nE\n") };
        assert_eq!(next.current_lines(), Some((6, 6, vec!["E"], false)));
        // only removed: the old lines, by their old numbers
        assert_eq!(Diff::new("a\nb\nc\nd\n", "a\nd\n").current_lines(), Some((2, 3, vec!["b", "c"], true)));
        assert_eq!(Diff::new("x\n", "x\n").current_lines(), None);
    }

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
    fn the_arrow_puts_one_block_back() {
        let (old, new) = ("a\nb\nc\nd\n", "a\nB\nB2\nc\nd\nE\n");
        assert_eq!(revert(old, new, 0, false), "a\nb\nc\nd\nE\n");
        assert_eq!(revert(old, new, 1, false), "a\nB\nB2\nc\nd\n");
        assert_eq!(revert(old, new, 2, false), new, "no such block: nothing changes");
        // Ctrl+»: the new lines stay and the old ones come after them
        assert_eq!(revert(old, new, 0, true), "a\nB\nB2\nb\nc\nd\nE\n");
        assert_eq!(revert("a\nb", "a\nx", 0, true), "a\nx\nb");
        // a removed block comes back where it was; one without the last newline too
        assert_eq!(revert("a\nb\nc\n", "a\nc\n", 0, false), "a\nb\nc\n");
        assert_eq!(revert("a\nb", "a\nx", 0, false), "a\nb");
        assert_eq!(revert("a\rb\n", "a\rx\n", 0, false), "a\rb\n", "a lone \\r is a line end to the differ");
        assert_eq!(Diff::new(old, &revert(old, new, 0, false)).blocks.len(), 1);
    }

    #[test]
    fn edits_in_the_diff_undo_and_redo() {
        let base = "a\nb\nc\n";
        let mut d = Diff::new(base, "a\nB\nc\nX\n");
        let mine = "a\nB\nc\nX\n".to_string();
        // » on the first block puts b back; a typed line, then two undos and a redo
        let one = d.apply(&mine, DiffEdit::Revert(0, false)).unwrap();
        assert_eq!(one, "a\nb\nc\nX\n");
        let two = d.apply(&one, DiffEdit::Line(3, "Y".into())).unwrap();
        assert_eq!(two, "a\nb\nc\nY\n");
        assert_eq!(d.apply(&two, DiffEdit::Line(3, "Y".into())), None, "the same line changes nothing");
        let back = d.apply(&two, DiffEdit::Undo).unwrap();
        assert_eq!(back, one);
        let back = d.apply(&back, DiffEdit::Undo).unwrap();
        assert_eq!(back, mine);
        assert_eq!(d.apply(&back, DiffEdit::Undo), None);
        assert_eq!(d.apply(&back, DiffEdit::Redo).unwrap(), one);
    }

    #[test]
    fn an_edit_recomputes_the_blocks() {
        let mut d = Diff::new("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!(d.blocks.len(), 1);
        d.recompute("a\nb\nc\n");
        assert!(d.blocks.is_empty());
        d.recompute("a\nb\nc\nX\n");
        assert_eq!((d.blocks.len(), d.base.as_str()), (1, "a\nb\nc\n"));
    }

    #[test]
    fn changed_words_of_a_line() {
        assert_eq!(word_marks("foo bar baz", "foo qux baz"), (vec![4..7], vec![4..7]));
        assert_eq!(word_marks("same", "same"), (vec![], vec![]));
        // an added word marks only the new side
        assert!(word_marks("a", "a b").0.is_empty());
    }

    #[test]
    fn both_sides_walk_the_same_rows() {
        // the viewer scrolls one list of rows: every line of each side shows up once, in order
        let (old, new) = ("a\nb\nc\nd\n", "a\nX\nY\nd\ne\n");
        let rows = diff_rows(old, new);
        let left: Vec<usize> = rows.iter().filter_map(|r| r.left).collect();
        let right: Vec<usize> = rows.iter().filter_map(|r| r.right).collect();
        assert_eq!((left, right), (vec![0, 1, 2, 3], vec![0, 1, 2, 3, 4]));
        let d = Diff::new(old, new);
        assert_eq!(d.blocks, vec![1, 4]);
        assert_eq!((d.block_rows(0), d.block_rows(1)), (1..3, 4..5));
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

    #[test]
    fn blame_porcelain_names_each_line() {
        // the author comes once per commit; uncommitted lines (all-zero sha) have none
        let a = "1".repeat(40);
        let z = "0".repeat(40);
        let out = format!("{a} 1 1 2\nauthor edsuuu\nauthor-mail <x@x>\nsummary first\nfilename f.js\n\tfn f() {{\n{a} 2 2\n\tx();\n{z} 3 3 1\nauthor Not Committed Yet\n\ty();\n");
        assert_eq!(parse_blame(&out), vec![Some("edsuuu".to_string()), Some("edsuuu".to_string()), None]);
    }
}
