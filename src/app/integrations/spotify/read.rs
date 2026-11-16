//! What each player says, read away from the players (the tests feed them recorded answers): the system's media session
//! on Windows, AppleScript on macOS, MPRIS and Soloist on Linux; which of them is shown; Soloist's commands and where it
//! lives; and which covers may be downloaded.

#[cfg(any(target_os = "macos", test))]
use super::playback::{Repeat, spotify_uri};
#[cfg(any(not(any(windows, target_os = "macos")), test))]
use super::playback::{mpris_playback, soloist_playback};
#[cfg(any(not(any(windows, target_os = "macos")), test))]
use super::{Command, Player};
#[cfg(any(not(windows), test))]
use super::{Now, Track};
#[cfg(any(not(windows), test))]
use crate::prelude::*;

/// The media sessions Spotify registers on Windows: "Spotify.exe" (the installer) or "SpotifyAB.SpotifyMusic_…!Spotify"
/// (the Store).
#[cfg(any(windows, test))]
pub(crate) fn is_spotify_app(app_id: &str) -> bool {
    app_id.to_ascii_lowercase().contains("spotify")
}

/// What `osascript` printed: the state, the title, the artist, the album and the cover's address between unit
/// separators, then (a newer script) shuffling, repeating, the sound volume (0 to 100), the player position (seconds)
/// the duration (ms) and the track's id; empty when Spotify is stopped or closed.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn from_applescript(output: &str) -> Option<Now> {
    let parts: Vec<&str> = output.split('\u{1f}').collect();
    let (state, title, artist, album, cover, rest) = match parts[..] {
        [state, title, artist, album, cover] => (state, title, artist, album, cover, None),
        [state, title, artist, album, cover, shuffling, repeating, volume, position, duration, id] => {
            (state, title, artist, album, cover, Some([shuffling, repeating, volume, position, duration, id]))
        }
        _ => return None,
    };
    let mut now = Now { track: Track::new(title, artist, album, cover)?, playing: state.trim() == "playing", ..Default::default() };
    if let Some([shuffling, repeating, volume, position, duration, id]) = rest {
        // a real may come with the locale's comma
        let number = |text: &str| text.trim().replace(',', ".").parse::<f64>().ok().filter(|number| number.is_finite() && *number >= 0.0);
        let flag = |text: &str| match text.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
        let playback = &mut now.playback;
        playback.shuffle = flag(shuffling);
        // AppleScript's `repeating` is the context's repeat; it has no track repeat
        playback.repeat = flag(repeating).map(|on| if on { Repeat::Context } else { Repeat::Off });
        playback.volume = number(volume).map(|volume| (volume.min(100.0) / 100.0) as f32);
        playback.position_ms = number(position).map(|seconds| (seconds * 1000.0) as u64);
        playback.read_at_ms = if playback.position_ms.is_some() { now_ms() } else { 0 };
        playback.duration_ms = number(duration).map(|duration| duration as u64).filter(|duration| *duration > 0);
        playback.track_uri = spotify_uri(id).unwrap_or_default();
        playback.device = "Spotify".into();
    }
    Some(now)
}

/// MPRIS's Player properties as `busctl --json=short` prints them: `{"type":"a{sv}","data":[{name: {type, data}}]}`.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn from_mpris(root: &Value) -> Option<Now> {
    // a map comes as an object, or as a list with the object in it
    let map = |value: &Value| value.get("data").map(|data| if data.is_array() { data[0].clone() } else { data.clone() });
    let properties = map(root)?;
    let metadata = map(properties.get("Metadata")?)?;
    let text = |key: &str| match metadata.get(key).and_then(|value| value.get("data")) {
        Some(Value::String(text)) => text.clone(),
        // xesam:artist is a list
        Some(Value::Array(list)) => list.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
        _ => String::new(),
    };
    // a player with nothing loaded says /org/mpris/MediaPlayer2/TrackList/NoTrack
    if text("mpris:trackid").ends_with("/NoTrack") {
        return None;
    }
    let track = Track::new(&text("xesam:title"), &text("xesam:artist"), &text("xesam:album"), &text("mpris:artUrl"))?;
    let playback = mpris_playback(&properties, &metadata, now_ms());
    Some(Now { track, playing: properties["PlaybackStatus"]["data"] == "Playing", playback, ..Default::default() })
}

/// The `mpris:trackid` of MPRIS's answer (what `SetPosition` names the track by).
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn mpris_track_id(root: &Value) -> Option<String> {
    let map = |value: &Value| value.get("data").map(|data| if data.is_array() { data[0].clone() } else { data.clone() });
    let id = map(&map(root)?["Metadata"])?["mpris:trackid"]["data"].as_str()?.to_string();
    // a D-Bus object path: /a/b, letters, digits and _
    let path = id.starts_with('/') && id.len() < 400 && id.chars().all(|letter| letter.is_ascii_alphanumeric() || letter == '_' || letter == '/');
    path.then_some(id)
}

/// Of the players that answered (in order of preference), the first that plays, else the first; with its index.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn first_playing<const N: usize>(answers: [Option<Now>; N]) -> Option<(usize, Now)> {
    let found = || answers.iter().enumerate().filter_map(|(index, now)| Some((index, now.as_ref()?)));
    found().find(|(_, now)| now.playing).or_else(|| found().next()).map(|(index, now)| (index, now.clone()))
}

/// Of what the Spotify app (or Spotifast) and Soloist say, the one shown: the one that plays; when both or neither do,
/// the app (it is the one on the screen), else Soloist. Spotifast and a Soloist on the same song are Spotifast playing
/// through its Soloist: one, on both cards, with what Soloist says of it where Spotifast said nothing (the context).
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn pick(app: Option<Now>, soloist: Option<Now>) -> Option<Now> {
    match (app, soloist) {
        (Some(mut app), Some(soloist)) if app.player == Player::Spotifast && app.track.title == soloist.track.title => {
            let (mine, its) = (&mut app.playback, soloist.playback);
            mine.context = mine.context.take().or(its.context);
            mine.shuffle = mine.shuffle.or(its.shuffle);
            mine.repeat = mine.repeat.or(its.repeat);
            if mine.track_uri.is_empty() {
                mine.track_uri = its.track_uri;
            }
            Some(Now { through_soloist: true, ..app })
        }
        (Some(app), Some(soloist)) if soloist.playing && !app.playing => Some(soloist),
        (Some(app), _) => Some(app),
        (None, soloist) => soloist,
    }
}

/// Soloist's `playback_state` (`soloist ctl now --json`, the event of its WebSocket API): the item's name, its creators'
/// (the artists), its parent's (the album) and its cover of the "default" size (else the first), with what plays beyond
/// it (`soloist_playback`); None when it is idle or has nothing loaded. Buffering counts as playing.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn from_soloist(root: &Value) -> Option<Now> {
    let status = root.get("status").and_then(Value::as_str).unwrap_or_default();
    if status == "idle" {
        return None;
    }
    let decorations = &root.get("item")?["decorations"];
    let name = |entity: &Value| entity["decorations"]["identity"]["name"].as_str().unwrap_or_default().to_string();
    let artists = decorations["creators"]
        .as_array()
        .map(|creators| creators.iter().map(|creator| name(&creator["entity"])).filter(|name| !name.is_empty()).collect::<Vec<_>>());
    let track = Track::new(
        decorations["identity"]["name"].as_str()?,
        &artists.unwrap_or_default().join(", "),
        &name(&decorations["parent"]["entity"]),
        &super::queue::soloist_cover(&decorations["visual_identity"]["cover"], "default"),
    )?;
    let playback = soloist_playback(root, now_ms());
    Some(Now { track, playing: matches!(status, "playing" | "buffering"), player: Player::Soloist, playback, ..Default::default() })
}

/// The `soloist ctl` arguments for a control: play or pause by whether it plays (`playing`), next, prev, shuffle on|off,
/// repeat off|context|track, volume 0-100, seek <ms>, play <uri>, add-to-queue <uri>.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn soloist_args(command: &Command, playing: bool) -> Vec<String> {
    let words: Vec<String> = match command {
        Command::PlayPause if playing => vec!["pause".into()],
        Command::PlayPause => vec!["play".into()],
        Command::Next => vec!["next".into()],
        Command::Previous => vec!["prev".into()],
        Command::Shuffle(on) => vec!["shuffle".into(), if *on { "on" } else { "off" }.into()],
        Command::Repeat(repeat) => vec!["repeat".into(), repeat.name().into()],
        Command::Volume(percent) => vec!["volume".into(), percent.min(&100).to_string()],
        Command::Seek(ms) => vec!["seek".into(), ms.to_string()],
        Command::PlayUri(uri) => vec!["play".into(), uri.clone()],
        Command::Enqueue(uri) => vec!["add-to-queue".into(), uri.clone()],
    };
    words
}

/// Soloist's data folder, as its download sets it (`XDG_DATA_HOME` or `~/.local/share`, then `soloist`), and where its
/// download puts the program (`~/.local/bin/soloist`).
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn soloist_data(env: impl Fn(&str) -> Option<String>) -> Option<std::path::PathBuf> {
    let base = env("XDG_DATA_HOME").filter(|base| !base.is_empty()).map(std::path::PathBuf::from);
    Some(base.or_else(|| env("HOME").map(|home| std::path::Path::new(&home).join(".local/share")))?.join("soloist"))
}

#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn soloist_program(env: impl Fn(&str) -> Option<String>) -> Option<std::path::PathBuf> {
    env("HOME").map(|home| std::path::Path::new(&home).join(".local/bin/soloist"))
}

/// Where Spotifast keeps the Soloist it starts (`XDG_STATE_HOME` or `~/.local/state`, then `spotifast/soloist`): a data
/// folder for each account in `accounts`, and the program it downloaded in `builds/<hash>/soloist`.
// ponytail: not the Flatpak's (~/.var/app); look there too when someone uses it.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn spotifast_soloist(env: impl Fn(&str) -> Option<String>) -> Option<std::path::PathBuf> {
    let base = env("XDG_STATE_HOME").filter(|base| !base.is_empty()).map(std::path::PathBuf::from);
    Some(base.or_else(|| env("HOME").map(|home| std::path::Path::new(&home).join(".local/state")))?.join("spotifast/soloist"))
}

/// A cover's address is one of Spotify's own picture servers, over https, and nothing else.
pub(crate) fn cover_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split('/').next().unwrap_or_default().to_ascii_lowercase();
    let plain = !host.is_empty() && host.chars().all(|letter| letter.is_ascii_alphanumeric() || letter == '.' || letter == '-');
    plain
        && (host == "i.scdn.co" || host.ends_with(".scdn.co") || host.ends_with(".spotifycdn.com"))
        && !url.chars().any(|letter| letter.is_control() || letter == ' ')
}
