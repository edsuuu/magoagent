//! Spotify's Soloist on Linux. It has no MPRIS, but started with its local API (`--ws`, as `-w 127.0.0.1:0`) it answers
//! `soloist ctl` on this computer, with no account and no key of ours: `now --json` prints its `playback_state`
//! (`from_soloist`), and play, pause, next and prev control it. Its data folder holds `ws.port` only while it runs with
//! that API: a closed Soloist costs a look at a file, not a process. The one its download starts keeps its folder in
//! `~/.local/share/soloist`; the one Spotifast starts (always with that API), one for each account in Spotifast's own.

use super::{Command, Now, first_playing, soloist_command, soloist_data, soloist_program, spotifast_soloist};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Stdio;

thread_local! {
    /// The data folder of the Soloist shown last: where a button goes (only the thread that asks calls these).
    static SHOWN: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// The folders in `folder`, sorted (Spotifast's accounts and builds).
fn inside(folder: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
    found.sort();
    found
}

/// Its program: on the PATH, where its download goes (`~/.local/bin`), or the one Spotifast downloaded (any of them
/// talks to any Soloist's local API).
pub(super) fn program() -> Option<PathBuf> {
    let spotifast = || spotifast_soloist(env).map(|root| inside(&root.join("builds"))).unwrap_or_default().into_iter().map(|build| build.join("soloist"));
    crate::editor::lsp::which("soloist")
        .or_else(|| soloist_program(env).filter(|program| program.is_file()))
        .or_else(|| spotifast().find(|program| program.is_file()))
}

/// The data folders of the Soloists running with their local API: its download's, then Spotifast's.
fn running() -> Vec<PathBuf> {
    let spotifast = spotifast_soloist(env).map(|root| inside(&root.join("accounts"))).unwrap_or_default();
    soloist_data(env).into_iter().chain(spotifast).filter(|data| data.join("ws.port").is_file()).collect()
}

/// What `soloist ctl <args>` printed for the Soloist of `data`, when it ran and answered.
fn ctl(data: &Path, args: &[&str]) -> Option<String> {
    let output = crate::editor::command(program()?).arg("ctl").args(args).arg("-D").arg(data).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn read_one(data: &Path) -> Option<Now> {
    super::from_soloist(&serde_json::from_str(&ctl(data, &["now", "--json"])?).ok()?)
}

/// Of the Soloists running, the first that plays, else the first with something loaded.
pub(super) fn read() -> Option<Now> {
    let folders = running();
    // ponytail: the first three; more Soloists at once when someone has them
    let [first, second, third] = [0, 1, 2].map(|index| folders.get(index).and_then(|data| read_one(data)));
    let (index, now) = first_playing([first, second, third])?;
    SHOWN.set(folders.get(index).cloned());
    Some(now)
}

/// `playing`: whether it played when last read (play and pause are two commands).
pub(super) fn control(command: Command, playing: bool) -> bool {
    SHOWN.with_borrow(|data| data.as_deref().and_then(|data| ctl(data, &[soloist_command(command, playing)]))).is_some()
}
