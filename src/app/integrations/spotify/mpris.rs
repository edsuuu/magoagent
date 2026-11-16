//! Spotify on Linux, by MPRIS: it owns `org.mpris.MediaPlayer2.spotify` on the session bus (the Snap and the Flatpak
//! too), with what it plays as properties and PlayPause, Next and Previous as calls. `busctl` (systemd's) asks, and
//! answers in JSON.
// ponytail: one busctl a poll instead of listening to the bus's signals; a D-Bus client when the two seconds bother.

use super::{Command, Now};
use std::process::Stdio;

fn busctl(interface: &str, call: &[&str]) -> Option<String> {
    let output = crate::editor::command("busctl")
        .args(["--user", "--json=short", "call", "org.mpris.MediaPlayer2.spotify", "/org/mpris/MediaPlayer2", interface])
        .args(call)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

pub(super) fn read() -> Option<Now> {
    let answer = busctl("org.freedesktop.DBus.Properties", &["GetAll", "s", "org.mpris.MediaPlayer2.Player"])?;
    super::from_mpris(&serde_json::from_str(&answer).ok()?)
}

pub(super) fn control(command: Command) -> bool {
    let method = match command {
        Command::PlayPause => "PlayPause",
        Command::Next => "Next",
        Command::Previous => "Previous",
    };
    busctl("org.mpris.MediaPlayer2.Player", &[method]).is_some()
}
