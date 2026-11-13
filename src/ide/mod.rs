//! What the file tree's code pane and the rest of the app draw with, from the PhpStorm-like editor
//! the IDE node once had: highlight, hints, index, text, theme, language servers, git, find, merge.

use chrome::{named, row};
use eframe::egui::{
    self, Align, Align2, Color32, Frame, Id, Key, KeyboardShortcut, Margin, Modifiers, Rect, Sense, Shape, Stroke, Ui, pos2, text::LayoutJob, vec2,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use text::*;
use theme as t;

pub(crate) mod chrome;
pub(crate) mod editor;
pub(crate) mod excluded;
pub(crate) mod fileops;
pub(crate) mod find;
mod git;
pub(crate) mod highlight;
pub(crate) mod hints;
pub(crate) mod icons;
pub(crate) mod index;
mod langs;
pub(crate) mod lsp;
pub(crate) mod media;
pub(crate) mod merge;
#[cfg(test)]
pub(crate) mod mergetest;
mod mergeview;
pub(crate) use mergeview::Conflicts;
pub(crate) mod text;
pub(crate) mod textindex;
pub(crate) mod theme;
pub(crate) mod tree;
mod unified;

// git for the rest of the app (an agent's worktree, app/environments.rs)
pub(crate) use git::git as run_git;
// what the file tree node marks its rows with (app/filetree.rs)
pub(crate) use git::{DiffRow, RowKind, Status, diff_rows, parse_blame, parse_status, word_marks};
// the unified viewer's rows (app/filetree/compare.rs)
pub(crate) use unified::unified_rows;

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
