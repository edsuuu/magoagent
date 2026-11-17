//! Spotify on Linux, by MPRIS: the app owns `org.mpris.MediaPlayer2.spotify` on the session bus (the Snap and the Flatpak
//! too), and Spotifast (a Spotify client of its own) `org.mpris.MediaPlayer2.spotifast`, with what plays as properties
//! (the track, `Shuffle`, `LoopStatus`, `Volume`, `Position`) and PlayPause, Next, Previous, SetPosition and OpenUri as
//! calls; shuffle, repeat and the volume are written as properties. `busctl` (systemd's) asks, and answers in JSON; of
//! the two, the one that plays is shown (`first_playing`), and the controls go to it. MPRIS has no queue.
// ponytail: one busctl a player a poll instead of listening to the bus's signals; a D-Bus client when the two seconds bother.

use super::{Command, Now, Player, first_playing, mpris_track_id};
use std::cell::{Cell, RefCell};
use std::process::Stdio;

/// The bus names of the players read on Spotify's card, and which each is: the app first.
const NAMES: [(&str, Player); 2] = [("org.mpris.MediaPlayer2.spotify", Player::App), ("org.mpris.MediaPlayer2.spotifast", Player::Spotifast)];
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

thread_local! {
    /// Which of `NAMES` was shown last: where a control goes (only the thread that asks calls these).
    static SHOWN: Cell<usize> = const { Cell::new(0) };
    /// Each player's `mpris:trackid`, as last read: what SetPosition names the track by.
    static TRACK_IDS: RefCell<[String; 2]> = const { RefCell::new([String::new(), String::new()]) };
}

/// `busctl --user --json=short <arguments>`: what it printed, when it answered.
fn busctl(arguments: &[&str]) -> Option<String> {
    let output = crate::editor::command("busctl").args(["--user", "--json=short"]).args(arguments).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn read_one(index: usize) -> Option<Now> {
    let (name, player) = NAMES[index];
    let answer = busctl(&["call", name, PATH, "org.freedesktop.DBus.Properties", "GetAll", "s", PLAYER])?;
    let root = serde_json::from_str(&answer).ok()?;
    TRACK_IDS.with_borrow_mut(|ids| ids[index] = mpris_track_id(&root).unwrap_or_default());
    let mut now = super::from_mpris(&root)?;
    now.playback.device = if player == Player::Spotifast { "Spotifast" } else { "Spotify" }.into();
    Some(Now { player, ..now })
}

pub(super) fn read() -> Option<Now> {
    let (index, now) = first_playing([0, 1].map(read_one))?;
    SHOWN.set(index);
    Some(now)
}

/// The busctl arguments of a control for the player at bus `name` whose track is `track_id` (None: it has no way).
pub(super) fn arguments(name: &str, track_id: &str, command: &Command) -> Option<Vec<String>> {
    let call = |words: &[&str]| ["call", name, PATH, PLAYER].iter().chain(words).map(|word| word.to_string()).collect::<Vec<_>>();
    let set = |property: &str, signature: &str, value: &str| {
        ["set-property", name, PATH, PLAYER, property, signature, value].iter().map(|word| word.to_string()).collect::<Vec<_>>()
    };
    Some(match command {
        Command::PlayPause => call(&["PlayPause"]),
        Command::Next => call(&["Next"]),
        Command::Previous => call(&["Previous"]),
        Command::Shuffle(on) => set("Shuffle", "b", if *on { "true" } else { "false" }),
        Command::Repeat(repeat) => set("LoopStatus", "s", repeat.mpris()),
        Command::Volume(percent) => set("Volume", "d", &format!("{:.2}", f64::from((*percent).min(100)) / 100.0)),
        // SetPosition takes µs, and only for the track it names
        Command::Seek(ms) if !track_id.is_empty() => call(&["SetPosition", "ox", track_id, &(ms.saturating_mul(1000)).to_string()]),
        Command::Seek(_) => return None,
        Command::PlayUri(uri) => call(&["OpenUri", "s", uri]),
        Command::Enqueue(_) => return None,
    })
}

pub(super) fn control(command: &Command) -> bool {
    let index = SHOWN.get();
    let track_id = TRACK_IDS.with_borrow(|ids| ids[index].clone());
    let Some(arguments) = arguments(NAMES[index].0, &track_id, command) else { return false };
    busctl(&arguments.iter().map(String::as_str).collect::<Vec<_>>()).is_some()
}
