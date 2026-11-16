//! Spotify on Linux, by MPRIS: the app owns `org.mpris.MediaPlayer2.spotify` on the session bus (the Snap and the Flatpak
//! too), and Spotifast (a Spotify client of its own) `org.mpris.MediaPlayer2.spotifast`, with what plays as properties
//! and PlayPause, Next and Previous as calls. `busctl` (systemd's) asks, and answers in JSON; of the two, the one that
//! plays is shown (`first_playing`), and the buttons go to it.
// ponytail: one busctl a player a poll instead of listening to the bus's signals; a D-Bus client when the two seconds bother.

use super::{Command, Now, Player, first_playing};
use std::cell::Cell;
use std::process::Stdio;

/// The bus names of the players read on Spotify's card, and which each is: the app first.
const NAMES: [(&str, Player); 2] = [("org.mpris.MediaPlayer2.spotify", Player::App), ("org.mpris.MediaPlayer2.spotifast", Player::Spotifast)];

thread_local! {
    /// Which of `NAMES` was shown last: where a button goes (only the thread that asks calls these).
    static SHOWN: Cell<usize> = const { Cell::new(0) };
}

fn busctl(name: &str, interface: &str, call: &[&str]) -> Option<String> {
    let output = crate::editor::command("busctl")
        .args(["--user", "--json=short", "call", name, "/org/mpris/MediaPlayer2", interface])
        .args(call)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn read_one((name, player): (&str, Player)) -> Option<Now> {
    let answer = busctl(name, "org.freedesktop.DBus.Properties", &["GetAll", "s", "org.mpris.MediaPlayer2.Player"])?;
    Some(Now { player, ..super::from_mpris(&serde_json::from_str(&answer).ok()?)? })
}

pub(super) fn read() -> Option<Now> {
    let (index, now) = first_playing(NAMES.map(read_one))?;
    SHOWN.set(index);
    Some(now)
}

pub(super) fn control(command: Command) -> bool {
    let method = match command {
        Command::PlayPause => "PlayPause",
        Command::Next => "Next",
        Command::Previous => "Previous",
    };
    busctl(NAMES[SHOWN.get()].0, "org.mpris.MediaPlayer2.Player", &[method]).is_some()
}
