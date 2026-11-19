//! Spotify on macOS, by AppleScript: it is asked only while it is open, so asking never
//! opens it. The first time, macOS asks the person whether MagoAgent may control Spotify. Its dictionary has shuffling,
//! repeating (the context's: no track repeat), the sound volume, the player position and `play track <uri>`; no queue.

use super::{Command, Now, QueueItem, Repeat, Track};
use std::process::Stdio;

/// The state, the track, then shuffling, repeating, the volume, the position (seconds), the duration (ms) and the id,
/// between unit separators; nothing when it is stopped or closed.
const READ: &str = r#"if application "Spotify" is running then
tell application "Spotify"
if player state is stopped then return ""
set t to current track
set s to character id 31
return (player state as text) & s & (name of t) & s & (artist of t) & s & (album of t) & s & (artwork url of t) & s & (shuffling as text) & s & (repeating as text) & s & (sound volume as text) & s & (player position as text) & s & (duration of t as text) & s & (id of t)
end tell
end if"#;

fn script(source: &str) -> Option<String> {
    let output = crate::editor::command("osascript").args(["-e", source]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;

    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim_end_matches(['\n', '\r']).to_string())
}

pub(super) fn read() -> Option<Now> {
    super::from_applescript(&script(READ)?)
}

/// What Spotify is told for a control; None when its dictionary has no way (a track repeat, the queue).
pub(super) fn verb(command: &Command) -> Option<String> {
    Some(match command {
        Command::PlayPause => "playpause".into(),
        Command::Next => "next track".into(),
        Command::Previous => "previous track".into(),
        Command::Shuffle(on) => format!("set shuffling to {on}"),
        Command::Repeat(Repeat::Track) => return None,
        Command::Repeat(repeat) => format!("set repeating to {}", *repeat == Repeat::Context),
        Command::Volume(percent) => format!("set sound volume to {}", percent.min(&100)),
        Command::Seek(ms) => format!("set player position to {}.{:03}", ms / 1000, ms % 1000),
        // the uri was checked (letters, digits and colons): nothing to break out of the quotes
        Command::PlayUri(uri) if super::playback::uri_allowed(uri) => format!("play track \"{uri}\""),
        Command::PlayUri(_) | Command::Enqueue(_) => return None,
    })
}

pub(super) fn control(command: &Command) -> bool {
    let Some(verb) = verb(command) else { return false };

    script(&format!("if application \"Spotify\" is running then tell application \"Spotify\" to {verb}")).is_some()
}

/// AppleScript has no queue.
pub(super) fn queue(_most: usize) -> Option<Vec<QueueItem>> {
    None
}

pub(super) fn cover(track: &Track) -> Option<Vec<u8>> {
    super::download_cover(&track.cover_url)
}
