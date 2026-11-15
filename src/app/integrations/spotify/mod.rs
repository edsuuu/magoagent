//! Spotify on the island, as Coucou's (MIT) Spotify pill: what plays (the title, the artist, the cover) and play/pause,
//! next and previous. No account and no key: the Spotify open on this computer is asked, by the system's media
//! controls on Windows (`smtc`), AppleScript on macOS (`applescript`) and MPRIS on Linux (`mpris`). Off, nothing is
//! asked; with Spotify closed, there is no pill.
// ponytail: asked every two seconds instead of listening to the player's events; listen when the delay bothers.

use super::{IslandItem, ItemRow};
use crate::prelude::*;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};

#[cfg(target_os = "macos")]
mod applescript;
#[cfg(not(any(windows, target_os = "macos")))]
mod mpris;
#[cfg(windows)]
mod smtc;
#[cfg(target_os = "macos")]
use applescript as system;
#[cfg(not(any(windows, target_os = "macos")))]
use mpris as system;
#[cfg(windows)]
use smtc as system;
#[cfg(test)]
mod tests;

/// Between two questions to the player, and how long it takes to say what a request changed.
const EVERY: std::time::Duration = std::time::Duration::from_secs(2);
const SETTLE: std::time::Duration = std::time::Duration::from_millis(350);
/// The biggest cover read (Spotify's are 640 px JPEGs, about 100 KB), and how many polls a track's cover is asked for.
pub(crate) const COVER_LIMIT: usize = 3 * 1024 * 1024;
const COVER_TRIES: u8 = 4;
const GREEN: Color32 = Color32::from_rgb(0x1e, 0xd7, 0x60);

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Track {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    /// Where the cover is, when it is an address (macOS, Linux); on Windows the system hands the picture itself.
    pub(crate) cover_url: String,
}

impl Track {
    /// A track out of what the player said: one line each, no control characters, cut; None without a title (nothing
    /// is loaded).
    pub(crate) fn new(title: &str, artist: &str, album: &str, cover_url: &str) -> Option<Track> {
        let clean = |text: &str, most: usize| text.chars().filter(|letter| !letter.is_control()).take(most).collect::<String>().trim().to_string();
        let title = clean(title, 200);
        (!title.is_empty()).then(|| Track { title, artist: clean(artist, 200), album: clean(album, 200), cover_url: clean(cover_url, 400) })
    }
}

/// What Spotify plays now.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Now {
    pub(crate) track: Track,
    pub(crate) playing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    PlayPause,
    Next,
    Previous,
}

impl Command {
    /// "playpause", "next" or "previous" (the island's chips and the MCP).
    pub(crate) fn parse(name: &str) -> Option<Command> {
        match name {
            "playpause" => Some(Command::PlayPause),
            "next" => Some(Command::Next),
            "previous" => Some(Command::Previous),
            _ => None,
        }
    }
}

/// The media sessions Spotify registers on Windows: "Spotify.exe" (the installer) or "SpotifyAB.SpotifyMusic_…!Spotify"
/// (the Store).
#[cfg(any(windows, test))]
pub(crate) fn is_spotify_app(app_id: &str) -> bool {
    app_id.to_ascii_lowercase().contains("spotify")
}

/// What `osascript` printed: the state, the title, the artist, the album and the cover's address between unit
/// separators; empty when Spotify is stopped or closed.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn from_applescript(output: &str) -> Option<Now> {
    let parts: Vec<&str> = output.split('\u{1f}').collect();
    let [state, title, artist, album, cover] = parts[..] else { return None };
    Some(Now { track: Track::new(title, artist, album, cover)?, playing: state.trim() == "playing" })
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
    Some(Now { track, playing: properties["PlaybackStatus"]["data"] == "Playing" })
}

/// A cover's address is one of Spotify's own picture servers, over https, and nothing else.
#[cfg(any(not(windows), test))]
pub(crate) fn cover_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split('/').next().unwrap_or_default().to_ascii_lowercase();
    let plain = !host.is_empty() && host.chars().all(|letter| letter.is_ascii_alphanumeric() || letter == '.' || letter == '-');
    plain
        && (host == "i.scdn.co" || host.ends_with(".scdn.co") || host.ends_with(".spotifycdn.com"))
        && !url.chars().any(|letter| letter.is_control() || letter == ' ')
}

/// The cover at `url`, when it is Spotify's and of a sane size.
#[cfg(not(windows))]
fn download_cover(url: &str) -> Option<Vec<u8>> {
    if !cover_allowed(url) {
        return None;
    }
    let request = super::http::Request { method: "GET", url, headers: vec!["User-Agent: MagoAgent".into()], body: None, timeout_seconds: 10 };
    let (status, bytes) = super::http::send_bytes(&request).ok()?;
    (status == 200 && !bytes.is_empty() && bytes.len() <= COVER_LIMIT).then_some(bytes)
}

enum Message {
    Now(Option<Now>),
    Cover(Track, Vec<u8>),
}

/// What the thread remembers between two questions: what it last told, and the cover it still owes.
#[derive(Default)]
struct Watch {
    told: Option<Option<Now>>,
    covered: Option<Track>,
    tries: u8,
}

impl Watch {
    /// What to tell the app after the player said `now`: the state when it changed, and the track's cover once
    /// (`cover` fetches it; it may come a moment after the title, so it is asked for a few polls).
    fn step(&mut self, now: Option<Now>, cover: impl FnOnce(&Track) -> Option<Vec<u8>>) -> Vec<Message> {
        let mut messages = Vec::new();
        if self.told.as_ref() != Some(&now) {
            let track = |state: &Option<Now>| state.as_ref().map(|now| now.track.clone());
            if self.told.as_ref().is_none_or(|told| track(told) != track(&now)) {
                (self.covered, self.tries) = (None, 0);
            }
            self.told = Some(now.clone());
            messages.push(Message::Now(now.clone()));
        }
        if let Some(now) = now
            && self.covered.as_ref() != Some(&now.track)
            && self.tries < COVER_TRIES
        {
            self.tries += 1;
            if let Some(bytes) = cover(&now.track) {
                self.covered = Some(now.track.clone());
                messages.push(Message::Cover(now.track, bytes));
            }
        }
        messages
    }
}

/// The thread that asks the player while the integration is on; it ends when the app drops its side of `commands`.
fn watch(commands: Receiver<Command>, messages: Sender<Message>, ctx: Context) {
    let mut watch = Watch::default();
    loop {
        for message in watch.step(system::read(), system::cover) {
            if messages.send(message).is_err() {
                return;
            }
            ctx.request_repaint();
        }
        match commands.recv_timeout(EVERY) {
            Ok(command) => {
                system::control(command);
                std::thread::sleep(SETTLE);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[derive(Default)]
pub(crate) struct Spotify {
    /// What plays; None: Spotify is closed, or has nothing loaded.
    pub(crate) now: Option<Now>,
    /// The cover of what plays, once it came.
    pub(crate) cover: Option<egui::TextureHandle>,
    /// The thread's two ends, while the integration is on: dropping them ends it.
    thread: Option<(Sender<Command>, Receiver<Message>)>,
    /// What was asked of the player, for the tests (they have no thread).
    #[cfg(test)]
    pub(crate) asked: Vec<Command>,
}

impl Spotify {
    fn take(&mut self, message: Message, ctx: &Context) {
        match message {
            Message::Now(now) => {
                if now.as_ref().map(|now| &now.track) != self.now.as_ref().map(|now| &now.track) {
                    self.cover = None;
                }
                self.now = now;
            }
            Message::Cover(track, bytes) if self.now.as_ref().is_some_and(|now| now.track == track) && bytes.len() <= COVER_LIMIT => {
                // what the player handed is read as any picture from outside: decoded, never run
                if let Ok(picture) = image::load_from_memory(&bytes) {
                    let picture = picture.thumbnail(96, 96).into_rgba8();
                    let size = [picture.width() as usize, picture.height() as usize];
                    self.cover =
                        Some(ctx.load_texture("spotify-cover", egui::ColorImage::from_rgba_unmultiplied(size, &picture), egui::TextureOptions::LINEAR));
                }
            }
            Message::Cover(..) => {}
        }
    }

    /// "Tocando: Título — Artista", for the MCP and Ajustes.
    pub(crate) fn report(&self) -> String {
        match &self.now {
            Some(now) => {
                let by = if now.track.artist.is_empty() { String::new() } else { format!(" — {}", now.track.artist) };
                format!("{}: {}{by}", if now.playing { "Tocando" } else { "Pausado" }, now.track.title)
            }
            None => "O Spotify não está aberto neste computador (ou não tem nada carregado).".into(),
        }
    }
}

impl DrawingApp {
    /// Each frame: on, the thread that asks the player runs and its news are taken; off, it ends and nothing stays.
    pub(crate) fn spotify_tick(&mut self, ctx: &Context) {
        let spotify = &mut self.integrations.spotify;
        if !self.prefs.integrations.spotify {
            if spotify.thread.is_some() || spotify.now.is_some() {
                *spotify = Spotify::default();
            }
            return;
        }
        // never the real player in a test
        if spotify.thread.is_none() && !cfg!(test) {
            let ((commands, inbox), (outbox, messages)) = (channel(), channel());
            let ctx = ctx.clone();
            std::thread::spawn(move || watch(inbox, outbox, ctx));
            spotify.thread = Some((commands, messages));
        }
        while let Some(message) = spotify.thread.as_ref().and_then(|(_, messages)| messages.try_recv().ok()) {
            spotify.take(message, ctx);
        }
    }

    /// Spotify in words, for the MCP's `integrations`.
    pub(crate) fn spotify_report(&self) -> String {
        if self.prefs.integrations.spotify { self.integrations.spotify.report() } else { "desligado.".into() }
    }

    /// Play/pause, next or previous, asked of the player; false when the integration is off or Spotify is closed.
    pub(crate) fn spotify_control(&mut self, command: Command) -> bool {
        let spotify = &mut self.integrations.spotify;
        if !self.prefs.integrations.spotify || spotify.now.is_none() {
            return false;
        }
        #[cfg(test)]
        spotify.asked.push(command);
        if let Some((commands, _)) = &spotify.thread {
            let _ = commands.send(command);
        }
        true
    }

    /// Spotify's pill: the title, the artist, the cover and the three buttons. None while it is closed.
    pub(super) fn spotify_item(&self) -> Option<IslandItem> {
        let now = self.integrations.spotify.now.as_ref()?;
        let ink = Color32::from_rgb(0xf5, 0xf6, 0xf8);
        let dim = Color32::from_rgb(0x8e, 0x93, 0x9c);
        let mut rows = vec![ItemRow { text: now.track.title.clone(), color: ink, action: None }];
        if !now.track.artist.is_empty() {
            rows.push(ItemRow { text: now.track.artist.clone(), color: dim, action: None });
        }
        Some(IslandItem {
            id: format!("{}spotify", super::ITEM_PREFIX),
            name: "Spotify",
            kind: if now.playing { "tocando" } else { "pausado" }.into(),
            color: GREEN,
            badge: None,
            failed: false,
            rows,
            chips: vec![
                ("Anterior", "spotify previous".into()),
                (if now.playing { "Pausar" } else { "Tocar" }, "spotify playpause".into()),
                ("Próxima", "spotify next".into()),
            ],
            cover: self.integrations.spotify.cover.as_ref().map(egui::TextureHandle::id),
        })
    }
}
