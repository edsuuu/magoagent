//! Linux: the Spotify app and Spotifast, by MPRIS (`mpris.rs`), and Spotify's Soloist, its headless Spotify Connect
//! receiver (the music chosen on the phone or the web plays on this computer), by `soloist ctl` (`soloist.rs`). The
//! island shows the one that plays (`pick`) of those Ajustes turned on, and its controls go to that one.

use super::{Command, Now, Player, QueueItem, Sources, Track, mpris, pick, soloist};
use std::cell::Cell;

thread_local! {
    /// The player shown last, whether it played and whether Spotifast played through its own Soloist: where a control
    /// goes (only the thread that asks calls these).
    static SHOWN: Cell<Option<(Player, bool, bool)>> = const { Cell::new(None) };
}

pub(super) fn read(sources: Sources) -> Option<Now> {
    let shown = pick(sources.app.then(mpris::read).flatten(), sources.soloist.then(soloist::read).flatten());
    SHOWN.set(shown.as_ref().map(|now| (now.player, now.playing, now.through_soloist)));
    shown
}

pub(super) fn control(command: &Command) -> bool {
    match SHOWN.get() {
        Some((Player::Soloist, playing, _)) => soloist::control(command, playing),
        // Spotifast through its Soloist: what its MPRIS lacks (a uri to play, the queue) goes to that Soloist
        Some((_, playing, true)) if matches!(command, Command::PlayUri(_) | Command::Enqueue(_)) => soloist::control(command, playing),
        _ => mpris::control(command),
    }
}

/// What plays next, when a Soloist plays it (MPRIS has no queue).
pub(super) fn queue(most: usize) -> Option<Vec<QueueItem>> {
    match SHOWN.get() {
        Some((Player::Soloist, ..) | (_, _, true)) => soloist::queue(most),
        _ => None,
    }
}

pub(super) fn cover(track: &Track) -> Option<Vec<u8>> {
    super::download_cover(&track.cover_url)
}
