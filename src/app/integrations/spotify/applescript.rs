//! Spotify on macOS, by AppleScript (as Coucou's SpotifyController): it is asked only while it is open, so asking never
//! opens it. The first time, macOS asks the person whether MagoAgent may control Spotify.

use super::{Command, Now, Track};
use std::process::Stdio;

/// The state and the track, between unit separators; nothing when it is stopped or closed.
const READ: &str = r#"if application "Spotify" is running then
tell application "Spotify"
if player state is stopped then return ""
set t to current track
set s to character id 31
return (player state as text) & s & (name of t) & s & (artist of t) & s & (album of t) & s & (artwork url of t)
end tell
end if"#;

fn script(source: &str) -> Option<String> {
    let output = crate::editor::command("osascript").args(["-e", source]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim_end_matches(['\n', '\r']).to_string())
}

pub(super) fn read() -> Option<Now> {
    super::from_applescript(&script(READ)?)
}

pub(super) fn control(command: Command) -> bool {
    let verb = match command {
        Command::PlayPause => "playpause",
        Command::Next => "next track",
        Command::Previous => "previous track",
    };
    script(&format!("if application \"Spotify\" is running then tell application \"Spotify\" to {verb}")).is_some()
}

pub(super) fn cover(track: &Track) -> Option<Vec<u8>> {
    super::download_cover(&track.cover_url)
}
