//! Spotify's Soloist on Linux. It has no MPRIS, but started with its local API (`--ws`, as `-w 127.0.0.1:0`) it answers
//! `soloist ctl` on this computer, with no account and no key of ours: `now --json` prints its `playback_state`
//! (`from_soloist`), `queue <n> --json` its `queue_changed`, and play, pause, next, prev, seek, volume, shuffle, repeat,
//! `play <uri>` and `add-to-queue` control it. Its data folder holds `ws.port` only while it runs with that API: a closed
//! Soloist costs a look at a file, not a process. The one its download starts keeps its folder in
//! `~/.local/share/soloist`; the one Spotifast starts (always with that API), one for each account in Spotifast's own.

use super::queue::{from_soloist_queue, from_soloist_queue_text};
use super::{Command, Now, QueueItem, first_playing, soloist_args, soloist_data, soloist_program, spotifast_soloist};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::process::Stdio;

thread_local! {
    /// The data folder of the Soloist shown last: where a control goes (only the thread that asks calls these).
    static SHOWN: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    /// This Soloist's `ctl queue` printed text where JSON was asked (an older one): text from then on.
    static QUEUE_TEXT: Cell<bool> = const { Cell::new(false) };
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
pub(super) fn control(command: &Command, playing: bool) -> bool {
    let args = soloist_args(command, playing);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    SHOWN.with_borrow(|data| data.as_deref().and_then(|data| ctl(data, &args))).is_some()
}

/// What the first Soloist that answers plays next, from any thread (the MCP's `queue`).
pub(super) fn queue_now(most: usize) -> Option<Vec<QueueItem>> {
    running().iter().find_map(|data| queue_of(data, most))
}

/// What the Soloist shown plays next, `most` tracks.
pub(super) fn queue(most: usize) -> Option<Vec<QueueItem>> {
    queue_of(&SHOWN.with_borrow(Clone::clone)?, most)
}

/// What the Soloist of `data` plays next: its `queue_changed` (`--json`), else the text an older one prints.
fn queue_of(data: &Path, most: usize) -> Option<Vec<QueueItem>> {
    let most_text = most.max(1).to_string();
    if !QUEUE_TEXT.get() {
        let answer = ctl(data, &["queue", &most_text, "--json"]);
        if let Some(found) = answer.as_deref().and_then(|answer| serde_json::from_str(answer).ok()).and_then(|root| from_soloist_queue(&root, most)) {
            return Some(found);
        }
        if answer.is_none() {
            // not logged in, or `--json` unknown: the text says which
            let text = ctl(data, &["queue", &most_text])?;
            QUEUE_TEXT.set(true);
            return Some(from_soloist_queue_text(&text, most));
        }
        QUEUE_TEXT.set(true);
    }
    Some(from_soloist_queue_text(&ctl(data, &["queue", &most_text])?, most))
}
