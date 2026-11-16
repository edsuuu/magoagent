//! What plays besides the track: shuffle, repeat, the volume, where in the song, from where (a playlist, Liked Songs,
//! an album) and whether the track is in Liked Songs; and which way a control goes (`route`). Each player says what it
//! can: Soloist its `playback_state`, MPRIS its properties, the system's media session on Windows and AppleScript on
//! macOS what they have; with the account linked the Web API says the rest (`web.rs`).

use super::{Command, Now, Player};
#[cfg(any(not(any(windows, target_os = "macos")), test))]
use crate::prelude::*;

/// Repeat: off, the context (the playlist or the album) or the track, as Soloist and the Web API say it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Repeat {
    #[default]
    Off,
    Context,
    Track,
}

impl Repeat {
    /// "off", "context" or "track" (Soloist's and the Web API's words); MPRIS's "None", "Playlist" and "Track" too.
    pub(crate) fn parse(name: &str) -> Option<Repeat> {
        match name.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "desligado" => Some(Repeat::Off),
            "context" | "playlist" | "list" | "contexto" => Some(Repeat::Context),
            "track" | "faixa" | "música" | "musica" => Some(Repeat::Track),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Repeat::Off => "off",
            Repeat::Context => "context",
            Repeat::Track => "track",
        }
    }

    /// The button's next state: off, the context, the track, off.
    #[allow(dead_code)] // the island's repeat button
    pub(crate) fn next(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::Context,
            Repeat::Context => Repeat::Track,
            Repeat::Track => Repeat::Off,
        }
    }

    /// MPRIS's `LoopStatus`.
    #[cfg(any(not(any(windows, target_os = "macos")), test))]
    pub(crate) fn mpris(self) -> &'static str {
        match self {
            Repeat::Off => "None",
            Repeat::Context => "Playlist",
            Repeat::Track => "Track",
        }
    }

    /// In words, for the MCP.
    pub(crate) fn words(self) -> &'static str {
        match self {
            Repeat::Off => "repetir desligado",
            Repeat::Context => "repetindo a lista",
            Repeat::Track => "repetindo a faixa",
        }
    }
}

/// Where the music comes from: a playlist, Liked Songs, an album, an artist.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PlayContext {
    /// "spotify:playlist:…", "spotify:album:…", "spotify:user:<id>:collection" (Liked Songs).
    pub(crate) uri: String,
    /// Its name when the player said it (Soloist does; the Web API only by asking for the playlist).
    pub(crate) name: String,
}

impl PlayContext {
    /// "playlist", "album", "artist", "collection" (Liked Songs) or "" out of the uri.
    pub(crate) fn kind(&self) -> &str {
        if is_liked_songs(&self.uri) {
            return "collection";
        }
        self.uri.split(':').nth(1).unwrap_or_default()
    }

    /// What the page shows after "de": the name, "Músicas curtidas", else the kind.
    pub(crate) fn label(&self) -> String {
        match (self.kind(), self.name.is_empty()) {
            ("collection", _) => "Músicas curtidas".into(),
            (_, false) => self.name.clone(),
            ("playlist", true) => "Playlist".into(),
            ("album", true) => "Álbum".into(),
            ("artist", true) => "Artista".into(),
            _ => String::new(),
        }
    }
}

/// Liked Songs as a context: `spotify:user:<id>:collection` (and the bare `spotify:collection` some players say).
pub(crate) fn is_liked_songs(uri: &str) -> bool {
    uri == "spotify:collection" || (uri.starts_with("spotify:user:") && (uri.ends_with(":collection") || uri.contains(":collection:")))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Playback {
    /// "spotify:track:…" of what plays; empty when the player did not say.
    pub(crate) track_uri: String,
    pub(crate) shuffle: Option<bool>,
    pub(crate) repeat: Option<Repeat>,
    /// 0 to 1.
    pub(crate) volume: Option<f32>,
    pub(crate) position_ms: Option<u64>,
    pub(crate) duration_ms: Option<u64>,
    /// When `position_ms` held (ms since the epoch): playing, the time goes on from there (`Now::position_at`).
    pub(crate) read_at_ms: i64,
    pub(crate) context: Option<PlayContext>,
    /// In Liked Songs; known only with the account linked.
    pub(crate) liked: Option<bool>,
    /// Where it plays: "Soloist", "Spotify", "Spotifast", or the Connect device the Web API names.
    pub(crate) device: String,
}

impl Now {
    /// Where in the song it is at `now_ms`: what the player said, gone on while it plays, never past the end.
    pub(crate) fn position_at(&self, now_ms: i64) -> Option<u64> {
        let playback = &self.playback;
        let position = playback.position_ms?;
        let gone = if self.playing && playback.read_at_ms > 0 { (now_ms - playback.read_at_ms).max(0) as u64 } else { 0 };
        Some(playback.duration_ms.map_or(position + gone, |duration| (position + gone).min(duration)))
    }
}

/// Whether `newer` says something `older` did not: anything but the time, or a time off from where `older`'s would be by
/// then (a seek, a stall). A song playing on says nothing new, so the app is not woken every poll.
pub(crate) fn differs(older: Option<&Option<Now>>, newer: &Option<Now>) -> bool {
    let Some(older) = older else { return true };
    match (older, newer) {
        (Some(older), Some(newer)) => {
            let still = |now: &Now| Now { playback: Playback { position_ms: None, read_at_ms: 0, ..now.playback.clone() }, ..now.clone() };
            if still(older) != still(newer) {
                return true;
            }
            match (older.position_at(newer.playback.read_at_ms), newer.playback.position_ms) {
                (Some(expected), Some(position)) => expected.abs_diff(position) > 2_000,
                (expected, position) => expected.is_some() != position.is_some(),
            }
        }
        (older, newer) => older != newer,
    }
}

/// "spotify:track:…" out of what a player names a track with: the uri itself, its open.spotify.com page or the
/// Spotify app's MPRIS track id (`/com/spotify/track/…`).
#[cfg(any(not(windows), test))]
pub(crate) fn spotify_uri(text: &str) -> Option<String> {
    let text = text.trim();
    let id_ok = |id: &str| !id.is_empty() && id.len() <= 64 && id.chars().all(|letter| letter.is_ascii_alphanumeric());
    let (kind, id) = if let Some(rest) = text.strip_prefix("spotify:") {
        rest.split_once(':')?
    } else if let Some(rest) = text.strip_prefix("https://open.spotify.com/") {
        // a page may carry a locale first (intl-pt) and a query after
        let rest = rest.split(['?', '#']).next().unwrap_or_default();
        let rest = if rest.starts_with("intl-") { rest.split_once('/')?.1 } else { rest };
        rest.split_once('/')?
    } else {
        // the Spotify app's MPRIS track id
        text.strip_prefix("/com/spotify/")?.split_once('/')?
    };
    (matches!(kind, "track" | "episode") && id_ok(id)).then(|| format!("spotify:{kind}:{id}"))
}

/// A uri the app hands to a player or to the Web API: `spotify:<kind>:<id>` of a kind it plays, or Liked Songs.
pub(crate) fn uri_allowed(uri: &str) -> bool {
    let id_ok = |id: &str| !id.is_empty() && id.len() <= 64 && id.chars().all(|letter| letter.is_ascii_alphanumeric());
    let parts: Vec<&str> = uri.split(':').collect();
    match parts[..] {
        ["spotify", "track" | "episode" | "album" | "playlist" | "artist" | "show", id] => id_ok(id),
        ["spotify", "user", user, "collection"] => !user.is_empty() && user.len() <= 64 && user.chars().all(|letter| letter.is_ascii_graphic()),
        _ => false,
    }
}

/// A track's uri (what a queue takes).
pub(crate) fn is_track(uri: &str) -> bool {
    uri_allowed(uri) && (uri.starts_with("spotify:track:") || uri.starts_with("spotify:episode:"))
}

/// The number at `value`, whole and not negative.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
fn whole(value: Option<&Value>) -> Option<u64> {
    let number = value?.as_f64()?;
    (number.is_finite() && number >= 0.0).then_some(number as u64)
}

/// What MPRIS's Player properties say beyond the track: `Shuffle`, `LoopStatus`, `Volume`, `Position` (µs) and the
/// metadata's `mpris:length` (µs) and `xesam:url`.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn mpris_playback(properties: &Value, metadata: &Value, now_ms: i64) -> Playback {
    let property = |name: &str| properties.get(name).and_then(|value| value.get("data"));
    let meta = |name: &str| metadata.get(name).and_then(|value| value.get("data"));
    let position_ms = whole(property("Position")).map(|micros| micros / 1000);
    let track_uri = [meta("xesam:url"), meta("mpris:trackid")].into_iter().flatten().filter_map(Value::as_str).find_map(spotify_uri);
    Playback {
        track_uri: track_uri.unwrap_or_default(),
        shuffle: property("Shuffle").and_then(Value::as_bool),
        repeat: property("LoopStatus").and_then(Value::as_str).and_then(Repeat::parse),
        volume: property("Volume").and_then(Value::as_f64).filter(|volume| volume.is_finite()).map(|volume| volume.clamp(0.0, 1.0) as f32),
        position_ms,
        duration_ms: whole(meta("mpris:length")).map(|micros| micros / 1000).filter(|duration| *duration > 0),
        read_at_ms: if position_ms.is_some() { now_ms } else { 0 },
        ..Default::default()
    }
}

/// What Soloist's `playback_state` says beyond the track (its WebSocket API's reference): `options.shuffle`,
/// `options.repeat`, `volume` (0 to 100), `position` (`position_ms` at the server's `timestamp_ms`), the item's
/// `decorations.playback.duration_ms`, and `context` (its uri and `decorations.identity.name`).
#[cfg(any(not(any(windows, target_os = "macos")), test))]
// ponytail: read off the reference's tables, never seen from a Soloist logged in here; the flat spellings (`shuffle`,
// `repeat`, `position_ms`, `duration_ms` at the top) are taken too. Fix the keys against `soloist ctl now --json`.
pub(crate) fn soloist_playback(root: &Value, now_ms: i64) -> Playback {
    let options = root.get("options").filter(|options| options.is_object()).unwrap_or(root);
    let item = &root["item"];
    let position = root.get("position").filter(|position| position.is_object());
    let position_ms = whole(position.and_then(|position| position.get("position_ms")).or(root.get("position_ms")));
    // the server's clock, when it is near ours (else ours: the time then only runs from this poll)
    let stamped = whole(position.and_then(|position| position.get("timestamp_ms"))).map(|at| at as i64).filter(|at| (at - now_ms).abs() < 600_000);
    let context = root.get("context").filter(|context| context.is_object()).and_then(|context| {
        let uri = context.get("uri").and_then(Value::as_str).unwrap_or_default().trim().to_string();
        let name = context["decorations"]["identity"]["name"].as_str().or(context.get("name").and_then(Value::as_str)).unwrap_or_default();
        let name: String = name.chars().filter(|letter| !letter.is_control()).take(200).collect();
        (!uri.is_empty() || !name.trim().is_empty()).then(|| PlayContext { uri, name: name.trim().to_string() })
    });
    let volume = whole(root.get("volume")).map(|volume| (volume.min(100) as f32) / 100.0);
    Playback {
        track_uri: item.get("uri").and_then(Value::as_str).filter(|uri| uri_allowed(uri)).unwrap_or_default().to_string(),
        shuffle: options.get("shuffle").and_then(Value::as_bool),
        repeat: options.get("repeat").and_then(Value::as_str).and_then(Repeat::parse),
        volume,
        position_ms,
        duration_ms: whole(item["decorations"]["playback"].get("duration_ms").or(item.get("duration_ms"))).filter(|duration| *duration > 0),
        read_at_ms: if position_ms.is_some() { stamped.unwrap_or(now_ms) } else { 0 },
        context,
        liked: None,
        device: "Soloist".into(),
    }
}

/// The system the app runs on, for `route`: what its own player takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum System {
    Linux,
    Windows,
    Mac,
}

impl System {
    pub(crate) const HERE: System = if cfg!(windows) {
        System::Windows
    } else if cfg!(target_os = "macos") {
        System::Mac
    } else {
        System::Linux
    };
}

/// Which way a control goes: to the player shown (its own controls), to the Web API (the account), or nowhere, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Route {
    Player,
    Web,
    No(&'static str),
}

pub(crate) const NEEDS_ACCOUNT_QUEUE: &str = "Adicionar à fila precisa da conta do Spotify conectada em Ajustes › Integrações.";
pub(crate) const NEEDS_ACCOUNT: &str = "Isso precisa da conta do Spotify conectada em Ajustes › Integrações.";
pub(crate) const CLOSED: &str = "O Spotify não está aberto neste computador (e a conta do Spotify não está conectada).";

/// Where `command` goes, with `player` the one shown (None: nothing plays here) and `linked` the account's state.
/// Soloist (and Spotifast playing through its own Soloist) does everything by `soloist ctl`. The Spotify app takes
/// shuffle, repeat, the volume, a seek and a uri by its own controls where the system has them, but with the account the
/// Web API does those (it is what really reaches the app: its MPRIS shows them and ignores the writes), and a queue
/// only exists there.
pub(crate) fn route(system: System, player: Option<Player>, through_soloist: bool, linked: bool, command: &Command) -> Route {
    let web_or = |why: &'static str| if linked { Route::Web } else { Route::No(why) };
    let Some(player) = player else { return web_or(CLOSED) };
    // Liked Songs is the account's own context: only the Web API knows it as one
    if linked && matches!(command, Command::PlayUri(uri) if is_liked_songs(uri)) {
        return Route::Web;
    }
    if player == Player::Soloist || (player == Player::Spotifast && through_soloist) {
        return Route::Player;
    }
    let basic = matches!(command, Command::PlayPause | Command::Next | Command::Previous);
    match command {
        _ if basic => Route::Player,
        Command::Enqueue(_) => web_or(NEEDS_ACCOUNT_QUEUE),
        // a uri by MPRIS's OpenUri is a guess of what the player does with it; the Web API plays it on that device
        Command::PlayUri(_) if linked => Route::Web,
        // Spotifast's own MPRIS takes the writes
        _ if player == Player::Spotifast => Route::Player,
        _ if linked => Route::Web,
        Command::Volume(_) if system == System::Windows => {
            Route::No("O Spotify do Windows não deixa mudar o volume por fora: conecte a conta do Spotify em Ajustes › Integrações.")
        }
        Command::Repeat(Repeat::Track) if system == System::Mac => {
            Route::No("O Spotify do macOS só repete a lista por fora: conecte a conta do Spotify em Ajustes › Integrações.")
        }
        _ => Route::Player,
    }
}
