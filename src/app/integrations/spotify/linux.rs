//! Linux: the Spotify app, by MPRIS (`mpris.rs`), and Spotify's Soloist, its headless Spotify Connect receiver (the
//! music chosen on the phone or the web plays on this computer), by `soloist ctl` (`soloist.rs`). The island shows the
//! one that plays (`pick`) of those Ajustes turned on, and its buttons go to that one.

use super::{Command, Now, Player, Sources, Track, mpris, pick, soloist};
use std::cell::Cell;

thread_local! {
    /// The player shown last and whether it played: where a button goes (only the thread that asks calls these).
    static SHOWN: Cell<Option<(Player, bool)>> = const { Cell::new(None) };
}

pub(super) fn read(sources: Sources) -> Option<Now> {
    let shown = pick(sources.app.then(mpris::read).flatten(), sources.soloist.then(soloist::read).flatten());
    SHOWN.set(shown.as_ref().map(|now| (now.player, now.playing)));
    shown
}

pub(super) fn control(command: Command) -> bool {
    match SHOWN.get() {
        Some((Player::Soloist, playing)) => soloist::control(command, playing),
        _ => mpris::control(command),
    }
}

pub(super) fn cover(track: &Track) -> Option<Vec<u8>> {
    super::download_cover(&track.cover_url)
}
