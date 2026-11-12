//! Git through the git CLI (PhpStorm shells out to git too): status, blame and the line diff rows.

use super::*;
use similar::{ChangeTag, DiffTag, TextDiff};
use std::collections::HashMap;
use std::ops::Range;
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

/// A git `Command`: no console window, messages in English, and
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
