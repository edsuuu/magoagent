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
mod actions;
mod rows;
#[cfg(test)]
mod tests;
pub use rows::*;

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
