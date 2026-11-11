//! Git through the git CLI (PhpStorm shells out to git too): status, HEAD content, line diff,
//! commit/push/pull/checkout and the branch popup. The Commit window and the diff are in commit.rs.

use super::branches::{Ask, Snapshot, explain};
use super::diffbar::Ignore;
use super::diffbase::rev_text;
use super::history::History;
use super::stash::StashForm;
use super::*;
use similar::{ChangeTag, DiffTag, TextDiff};
use std::collections::{BTreeSet, HashMap, HashSet};
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

/// The `git` to run: the one on PATH, else the usual install places (a GUI app on the Mac and
/// on Windows often starts without the shell's PATH). Found once.
fn git_program() -> &'static std::ffi::OsString {
    static PROGRAM: std::sync::OnceLock<std::ffi::OsString> = std::sync::OnceLock::new();
    PROGRAM.get_or_init(|| {
        let works = |p: &std::ffi::OsStr| command(p).arg("--version").output().is_ok_and(|o| o.status.success());
        let usual: &[&str] = if cfg!(windows) {
            &["C:/Program Files/Git/cmd/git.exe", "C:/Program Files (x86)/Git/cmd/git.exe"]
        } else if cfg!(target_os = "macos") {
            &["/usr/bin/git", "/opt/homebrew/bin/git", "/usr/local/bin/git"]
        } else {
            &["/usr/bin/git", "/usr/local/bin/git"]
        };
        std::iter::once("git").chain(usual.iter().copied()).map(std::ffi::OsString::from).find(|p| works(p)).unwrap_or_else(|| "git".into())
    })
}

/// A git `Command`: no console window, messages in English (`branches::explain` reads them), and
/// a remote that wants a password fails instead of waiting on a tty nobody sees.
fn git_command(root: &Path) -> std::process::Command {
    let mut c = command(git_program());
    c.current_dir(root).env("GIT_TERMINAL_PROMPT", "0").env("LANGUAGE", "C").env("LC_MESSAGES", "C");
    c
}

/// What a failed run says: git's stderr, or its stdout when stderr is empty (a merge that stops on
/// conflicts says so there).
fn git_failure(out: &std::process::Output) -> String {
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if err.is_empty() { String::from_utf8_lossy(&out.stdout).trim().to_string() } else { err }
}

pub fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = git_command(root).args(args).output().map_err(|e| format!("git: {e}"))?;
    if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).into_owned()) } else { Err(git_failure(&out)) }
}

/// `git` with `input` on its stdin and, when given, `index` as its index file (a commit of
/// some blocks of a file, built aside so the real index and the disk stay as they are).
fn git_with(root: &Path, args: &[&str], input: Option<&str>, index: Option<&Path>) -> Result<String, String> {
    use std::io::Write;
    let mut c = git_command(root);
    c.args(args).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    if let Some(index) = index {
        c.env("GIT_INDEX_FILE", index);
    }
    c.stdin(if input.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() });
    let mut child = c.spawn().map_err(|e| format!("git: {e}"))?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(text.as_bytes()).map_err(|e| format!("git: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("git: {e}"))?;
    if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).into_owned()) } else { Err(git_failure(&out)) }
}

/// The file as it goes into a partial commit: `disk` with the `excluded` blocks (of HEAD vs
/// disk) put back as in HEAD. The last block first, so the earlier ones keep their numbers.
pub fn partial_text(head: &str, disk: &str, excluded: &BTreeSet<usize>) -> String {
    excluded.iter().rev().fold(disk.to_string(), |text, &k| revert(head, &text, k, false))
}

/// A commit where `partial` files (relative path, text to commit) go in with only some of their
/// blocks: PhpStorm's "Include into commit" per block. The tree is built in a temporary index
/// from HEAD (whole files added, partial ones written as blobs), committed, and the real index
/// then takes the new HEAD for those paths; the disk is not touched, the left-out blocks stay
/// as local changes.
pub fn commit_parts(root: &Path, whole: &[String], partial: &[(String, String)], commit: &[String]) -> Result<String, String> {
    let index = PathBuf::from(git(root, &["rev-parse", "--git-path", "magoagent-index"])?.trim());
    let index = if index.is_absolute() { index } else { root.join(index) };
    let run = || -> Result<String, String> {
        git_with(root, &["read-tree", "HEAD"], None, Some(&index))?;
        if !whole.is_empty() {
            let mut add = vec!["add", "-A", "--"];
            add.extend(whole.iter().map(String::as_str));
            git_with(root, &add, None, Some(&index))?;
        }
        for (rel, text) in partial {
            let sha = git_with(root, &["hash-object", "-w", "--stdin", "--path", rel], Some(text), Some(&index))?;
            let mode = git(root, &["ls-tree", "HEAD", "--", rel])?.split_whitespace().next().unwrap_or("100644").to_string();
            git_with(root, &["update-index", "--add", "--cacheinfo", &format!("{mode},{},{rel}", sha.trim())], None, Some(&index))?;
        }
        let args: Vec<&str> = commit.iter().map(String::as_str).collect();
        let out = git_with(root, &args, None, Some(&index))?;
        // the real index catches up with the new HEAD for these paths only
        let mut reset = vec!["reset", "-q", "--"];
        reset.extend(whole.iter().chain(partial.iter().map(|(rel, _)| rel)).map(String::as_str));
        git(root, &reset)?;
        Ok(out)
    };
    let res = run();
    let _ = std::fs::remove_file(&index);
    res
}

pub fn head_text(root: &Path, path: &Path) -> Option<String> {
    rev_text(root, "HEAD", path)
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

/// The rows with the lines compared as `ignore` says (the lines themselves stay as they are).
pub fn diff_rows_ignoring(old: &str, new: &str, ignore: Ignore) -> Vec<DiffRow> {
    if ignore == Ignore::None {
        return diff_rows(old, new);
    }
    let keyed = |s: &str| {
        s.lines()
            .map(|l| {
                ignore.key(l)
                    + "
"
            })
            .collect::<String>()
    };
    diff_rows(&keyed(old), &keyed(new))
}

/// The first row of every changed block.
fn blocks_of(rows: &[DiffRow]) -> Vec<usize> {
    (0..rows.len()).filter(|&i| rows[i].kind != RowKind::Same && (i == 0 || rows[i - 1].kind == RowKind::Same)).collect()
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
    /// a commit's diff (history.rs): the names of its two sides, both read-only
    pub sides: Option<(String, String)>,
    /// what the differ leaves out when it compares lines (the toolbar's "Não ignorar" menu)
    pub ignore: Ignore,
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
        Self::build(old, new, Ignore::None)
    }

    fn build(old: &str, new: &str, ignore: Ignore) -> Self {
        let rows = diff_rows_ignoring(old, new, ignore);
        let blocks = blocks_of(&rows);
        let scroll_to = blocks.first().copied();
        Diff {
            old: old.lines().map(String::from).collect(),
            new: new.lines().map(String::from).collect(),
            rows,
            blocks,
            current: 0,
            scroll_to,
            title: None,
            claude: None,
            comment: String::new(),
            base: old.to_string(),
            seen: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            editing: None,
            sides: None,
            ignore,
        }
    }

    /// The working copy's diff against its base: the buffer is its right side and can be edited.
    pub fn is_plain(&self) -> bool {
        self.claude.is_none() && self.sides.is_none()
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
        let fresh = Diff::build(&self.base, new, self.ignore);
        self.current = self.current.min(fresh.blocks.len().saturating_sub(1));
        (self.old, self.new, self.rows, self.blocks) = (fresh.old, fresh.new, fresh.rows, fresh.blocks);
    }
}

impl Diff {
    /// The rows again under another ignore policy, from the lines on show.
    pub fn set_ignore(&mut self, ignore: Ignore) {
        self.ignore = ignore;
        let fresh = Diff::build(
            &self.old.join(
                "
",
            ),
            &self.new.join(
                "
",
            ),
            ignore,
        );
        self.current = self.current.min(fresh.blocks.len().saturating_sub(1));
        (self.rows, self.blocks) = (fresh.rows, fresh.blocks);
        self.scroll_to = self.blocks.get(self.current).copied();
    }
}

pub(super) enum Msg {
    Status { repo: bool, changes: Vec<Change>, branches: Vec<String>, branch: String, snap: Box<Snapshot> },
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
    /// blocks left out of the commit, per file: the hash of the text they were picked on (the
    /// buffer), how many blocks it had, and which. Picked on other text, they no longer count.
    pub(super) excluded: HashMap<PathBuf, (u64, usize, BTreeSet<usize>)>,
    pub(super) rx: Option<Receiver<Msg>>,
    pub busy: Option<String>,
    /// branches with their upstreams, remotes, merge in progress and stashes (branches.rs)
    pub snap: Snapshot,
    /// the question being asked (a new branch's name, a delete, a merge) and the branch popup's filter
    pub(super) ask: Option<Ask>,
    pub(super) filter: String,
    /// the History panel (history.rs) and its stash form (stash.rs)
    pub(super) history: History,
    pub(super) stash: StashForm,
    /// the revision the diffs compare with (HEAD unless the person picks another)
    pub(super) base: super::diffbase::Base,
}

impl Git {
    /// The blocks of `path` left out of the commit, if picked on the text whose hash is `seen`.
    pub(super) fn left_out(&self, path: &Path, seen: u64) -> Option<&BTreeSet<usize>> {
        self.excluded.get(path).filter(|(h, _, set)| *h == seen && !set.is_empty()).map(|e| &e.2)
    }

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
        if !self.git.repo
            || self.blame_rx.is_some()
            || self.blame.contains_key(&tab.path)
            || tab.lang.kind != highlight::Kind::Code
            || tab.diff.is_some()
            || tab.sheet.is_some()
        {
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
        // another base: its list follows the working copy too (unless a new base is on its way)
        if !self.git.base.is_head() && self.git.base.rx_idle() {
            let rev = self.git.base.name.clone();
            self.set_diff_base(&rev);
        }
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
                    snap: Box::new(Snapshot::read(&root)),
                },
                Err(_) => Msg::Status { repo: false, changes: vec![], branches: vec![], branch: String::new(), snap: Default::default() },
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

    /// An agent's commit (the MCP's ide_git): the tabs saved first; `files` (relative to the
    /// project) go in alone, else only the changes of files git already tracks (`add -u`). Back:
    /// what goes in and the untracked files left out, for the agent to name them if they belong.
    pub(crate) fn agent_commit(&mut self, message: &str, files: &[String]) -> Result<String, String> {
        let root = self.root.clone().ok_or("o IDE não está numa pasta")?;
        if let Some(busy) = &self.git.busy {
            return Err(format!("o git do IDE está ocupado ({busy}): espere terminar"));
        }
        self.save_all();
        if let Some(tab) = self.tabs.iter().find(|t| t.is_text() && t.dirty) {
            return Err(format!("{} não pôde ser salvo (mudou no disco ou a gravação falhou): resolva no IDE antes do commit", file_name(&tab.path)));
        }
        let rel = |p: &Path| p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/");
        let (mut tracked, mut untracked) = (vec![], vec![]);
        for c in parse_status(&root, &git(&root, &["status", "--porcelain=v1", "-z", "--untracked-files=all"])?) {
            (if c.status == Status::Untracked { &mut untracked } else { &mut tracked }).push(rel(&c.path));
        }
        let s = |v: &[&str]| v.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        let (cmds, going) = if files.is_empty() {
            (vec![s(&["add", "-u"]), s(&["commit", "-m", message])], tracked)
        } else {
            // exactly the files named, the way the Commit panel commits its ticked ones
            let named: Vec<String> = files.iter().map(|f| f.trim().replace('\\', "/").trim_start_matches("./").to_string()).collect();
            if let Some(f) = named.iter().find(|f| !tracked.contains(f) && !untracked.contains(f)) {
                return Err(format!("{f} não tem mudanças no projeto (o caminho é relativo à pasta do IDE)"));
            }
            untracked.retain(|f| !named.contains(f));
            let (mut add, mut commit) = (s(&["add", "-A", "--"]), s(&["commit", "-m", message, "--"]));
            add.extend(named.iter().cloned());
            commit.extend(named.iter().cloned());
            (vec![add, commit], named)
        };
        let list = |v: &[String]| if v.is_empty() { "nenhum".to_string() } else { v.join(", ") };
        if going.is_empty() {
            return Err(format!("nada versionado mudou; não versionados (nomeie em files para incluir): {}", list(&untracked)));
        }
        self.git_run("Commit", cmds);
        Ok(format!(
            "Commit começou no IDE com: {}. Não versionados que ficaram de fora (nomeie em files para incluir): {}. O resultado aparece no IDE.",
            list(&going),
            list(&untracked)
        ))
    }

    /// `git_run` for a commit with some blocks left out (`commit_parts`), then `after` (a push).
    pub(crate) fn git_commit_parts(&mut self, label: &str, whole: Vec<String>, partial: Vec<(String, String)>, commit: Vec<String>, after: Vec<Vec<String>>) {
        let Some(root) = self.root.clone() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let (ctx, label) = (self.ctx.clone(), label.to_string());
        self.git.busy = Some(format!("{label}…"));
        std::thread::spawn(move || {
            let mut res = commit_parts(&root, &whole, &partial, &commit);
            for c in &after {
                if res.is_err() {
                    break;
                }
                let args: Vec<&str> = c.iter().map(String::as_str).collect();
                res = git(&root, &args);
            }
            let _ = tx.send(Msg::Done(label, res));
            ctx.request_repaint();
        });
        self.git.rx = Some(rx);
    }

    pub(crate) fn git_poll(&mut self) {
        self.base_poll();
        self.history_poll();
        let Some(rx) = &self.git.rx else { return };
        let Ok(msg) = rx.try_recv() else { return };
        self.git.rx = None;
        match msg {
            Msg::Status { repo, changes, branches, branch, snap } => {
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
                self.git.snap = *snap;
                if !branch.is_empty() {
                    self.branch = branch;
                }
                if self.left == Some(Left::History) {
                    self.history_load();
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
                        let rev = self.git.base.rev().to_string();
                        for tab in &mut self.tabs {
                            tab.head = None; // HEAD moved: reload the diff base
                            // an open diff compares with the new HEAD too: the blocks just committed leave it
                            if let (Some(root), Some(d)) = (&self.root, tab.diff.as_mut().filter(|d| d.is_plain())) {
                                d.base = rev_text(root, &rev, &tab.path).unwrap_or_default();
                                d.seen = 0;
                            }
                        }
                    }
                    Err(e) => self.notify(format!("{label} falhou: {}", explain(&e))),
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
        if let Some(i) = self.tabs.iter().position(|t| t.path == path && t.diff.as_ref().is_some_and(|d| d.sides.is_none())) {
            self.active = i;
            return;
        }
        let Some(buffer) = self.buffer_tab(&path) else { return };
        let (old, new) = (rev_text(&root, self.git.base.rev(), &path).unwrap_or_default(), self.tabs[buffer].text.clone());
        let mut tab = Tab::new(path, String::new(), false);
        tab.diff = Some(Box::new(Diff::new(&old, &new)));
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
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
        assert_eq!(
            v,
            vec![
                (root.join("app/User.php"), Status::Modified),
                (root.join("new.txt"), Status::Untracked),
                (root.join("b.php"), Status::Renamed),
                (root.join("gone.php"), Status::Deleted),
                (root.join("c.php"), Status::Conflict),
            ]
        );
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
    fn a_partial_commit_takes_only_the_included_blocks() {
        let dir = std::env::temp_dir().join(format!("magoagent-partial-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (file, other) = (dir.join("a.php"), dir.join("b.php"));
        let head = "1\n2\n3\n4\n5\n6\n7\n";
        std::fs::write(&file, head).unwrap();
        std::fs::write(&other, "x\n").unwrap();
        let id = ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        git(&dir, &["init", "-q"]).unwrap();
        git(&dir, &["add", "."]).unwrap();
        git(&dir, &[&id[..], &["commit", "-qm", "base"]].concat()).unwrap();
        // two blocks in a.php; the second one stays out
        let disk = "1\nDOIS\n3\n4\n5\nSEIS\n7\n";
        std::fs::write(&file, disk).unwrap();
        std::fs::write(&other, "y\n").unwrap();
        let text = partial_text(head, disk, &BTreeSet::from([1]));
        assert_eq!(text, "1\nDOIS\n3\n4\n5\n6\n7\n");
        let commit: Vec<String> = [&id[..], &["commit", "-qm", "parcial"]].concat().into_iter().map(String::from).collect();
        commit_parts(&dir, &["b.php".into()], &[("a.php".into(), text.clone())], &commit).unwrap();
        assert_eq!(git(&dir, &["show", "HEAD:a.php"]).unwrap(), text);
        assert_eq!(git(&dir, &["show", "HEAD:b.php"]).unwrap(), "y\n");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), disk, "the disk keeps both blocks");
        // what is left is the excluded block, unstaged
        assert_eq!(git(&dir, &["diff", "--cached", "--name-only"]).unwrap(), "");
        assert_eq!(git(&dir, &["diff", "--name-only"]).unwrap(), "a.php\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_open_diff_compares_with_head_after_a_commit() {
        let dir = std::env::temp_dir().join(format!("magoagent-diffhead-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, "1\n2\n").unwrap();
        let id = ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        git(&dir, &["init", "-q"]).unwrap();
        git(&dir, &["add", "."]).unwrap();
        git(&dir, &[&id[..], &["commit", "-qm", "base"]].concat()).unwrap();
        std::fs::write(&file, "1\nDOIS\n").unwrap();
        let mut ide = Ide::new(egui::Context::default(), "h", &dir.to_string_lossy());
        ide.open_diff(file.clone());
        git(&dir, &[&id[..], &["commit", "-qam", "dois"]].concat()).unwrap();
        // the IDE's own commit finished
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Msg::Done("Commit".into(), Ok(String::new()))).unwrap();
        ide.git.rx = Some(rx);
        ide.git_poll();
        assert_eq!(ide.tabs[ide.active].diff.as_ref().unwrap().base, "1\nDOIS\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_agent_commit_saves_first_and_takes_untracked_files_only_when_named() {
        let dir = std::env::temp_dir().join(format!("magoagent-agentcommit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        std::fs::write(&a, "a\n").unwrap();
        std::fs::write(&b, "b\n").unwrap();
        git(&dir, &["init", "-q"]).unwrap();
        for (k, v) in [("user.name", "t"), ("user.email", "t@t"), ("commit.gpgsign", "false")] {
            git(&dir, &["config", k, v]).unwrap();
        }
        git(&dir, &["add", "."]).unwrap();
        git(&dir, &["commit", "-qm", "base"]).unwrap();
        std::fs::write(dir.join("segredo.env"), "TOKEN=x\n").unwrap();
        let mut ide = Ide::new(egui::Context::default(), "c", &dir.to_string_lossy());
        let wait = |ide: &mut Ide| {
            let start = Instant::now();
            while ide.git.busy.is_some() && start.elapsed() < Duration::from_secs(20) {
                std::thread::sleep(Duration::from_millis(20));
                ide.git_poll();
            }
            assert!(ide.status.contains("concluído"), "{}", ide.status);
        };
        let names = |dir: &Path| git(dir, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
        // b.txt only in the editor; a.txt typed in while someone rewrote it on disk
        ide.open_file(b.clone(), None);
        (ide.tabs[0].text, ide.tabs[0].dirty) = ("b2\n".into(), true);
        ide.open_file(a.clone(), None);
        (ide.tabs[1].text, ide.tabs[1].dirty) = ("a2\n".into(), true);
        std::fs::write(&a, "deles\n").unwrap();
        std::fs::File::options().write(true).open(&a).unwrap().set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
        ide.git.busy = Some("Push…".into());
        assert!(ide.agent_commit("dois", &[]).unwrap_err().contains("ocupado"));
        ide.git.busy = None;
        let err = ide.agent_commit("dois", &[]).unwrap_err();
        assert!(err.contains("a.txt"), "{err}");
        assert_eq!(git(&dir, &["rev-list", "--count", "HEAD"]).unwrap().trim(), "1", "nothing committed");
        // the person keeps theirs: the tracked changes go in, saved; the untracked one is named as left out
        ide.save(1, true);
        let said = ide.agent_commit("dois", &[]).unwrap();
        assert!(said.contains("a.txt, b.txt") && said.contains("segredo.env"), "{said}");
        wait(&mut ide);
        assert_eq!((names(&dir), git(&dir, &["show", "HEAD:b.txt"]).unwrap()), ("a.txt\nb.txt\n".into(), "b2\n".into()));
        // named, it goes in alone
        assert!(ide.agent_commit("x", &["nada.txt".into()]).is_err());
        std::fs::write(&b, "b3\n").unwrap();
        ide.agent_commit("segredo", &["./segredo.env".into()]).unwrap();
        wait(&mut ide);
        assert_eq!(names(&dir), "segredo.env\n");
        drop(ide);
        let _ = std::fs::remove_dir_all(&dir);
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
        let out = format!(
            "{a} 1 1 2\nauthor edsuuu\nauthor-mail <x@x>\nsummary first\nfilename f.js\n\tfn f() {{\n{a} 2 2\n\tx();\n{z} 3 3 1\nauthor Not Committed Yet\n\ty();\n"
        );
        assert_eq!(parse_blame(&out), vec![Some("edsuuu".to_string()), Some("edsuuu".to_string()), None]);
    }
}
