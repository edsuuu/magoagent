//! Spotify on the island, as the reference's (MIT) Spotify pill: what plays (the title, the artist, the cover) and
//! play/pause, next and previous. No account and no key: the Spotify open on this computer is asked, by the system's
//! media controls on Windows (`smtc`), AppleScript on macOS (`applescript`) and on Linux MPRIS (`mpris`, the app and
//! Spotifast), and Spotify's Soloist there too, by its local API (`soloist`, its own or the one Spotifast starts;
//! `linux` shows the one that plays). Off, nothing is asked; with Spotify closed, there is no pill.
// ponytail: asked every two seconds instead of listening to the player's events; listen when the delay bothers.

use super::{IslandItem, ItemRow};
use crate::prelude::*;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};

#[cfg(target_os = "macos")]
mod applescript;
#[cfg(not(any(windows, target_os = "macos")))]
mod linux;
#[cfg(not(any(windows, target_os = "macos")))]
mod mpris;
#[cfg(windows)]
mod smtc;
#[cfg(not(any(windows, target_os = "macos")))]
mod soloist;
#[cfg(target_os = "macos")]
use applescript as system;
#[cfg(not(any(windows, target_os = "macos")))]
use linux as system;
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

/// What Spotify plays now, and which one plays it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Now {
    pub(crate) track: Track,
    pub(crate) playing: bool,
    pub(crate) player: Player,
    /// Spotifast plays it through its own Soloist (the same song on both): it shows on both cards.
    pub(crate) through_soloist: bool,
}

impl Now {
    /// Whether it shows on a card of Ajustes: Spotify's (`App`: the app and Spotifast) or Soloist's.
    pub(crate) fn on(&self, card: Player) -> bool {
        match card {
            Player::Soloist => self.player == Player::Soloist || self.through_soloist,
            Player::App | Player::Spotifast => self.player != Player::Soloist,
        }
    }

    /// Where it plays, on the island's pill: nothing for the app.
    fn place(&self) -> &'static str {
        match self.player {
            Player::App => "",
            Player::Spotifast => " no Spotifast",
            Player::Soloist => " no Soloist",
        }
    }

    /// Where it plays, after the title: nothing for the app.
    fn from(&self) -> &'static str {
        match self.player {
            Player::App => "",
            Player::Spotifast if self.through_soloist => " (Spotifast, no Soloist)",
            Player::Spotifast => " (Spotifast)",
            Player::Soloist => " (Soloist)",
        }
    }
}

/// Which Spotify plays: the app, Spotifast (a Spotify client of its own) or Soloist (Spotify's Connect receiver); the
/// last two only on Linux.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
pub(crate) enum Player {
    #[default]
    App,
    Spotifast,
    Soloist,
}

/// The players Ajustes turned on (Soloist only on Linux): the thread asks only these.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Sources {
    pub(crate) app: bool,
    pub(crate) soloist: bool,
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
    Some(Now { track: Track::new(title, artist, album, cover)?, playing: state.trim() == "playing", ..Default::default() })
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
    Some(Now { track, playing: properties["PlaybackStatus"]["data"] == "Playing", ..Default::default() })
}

/// Of the players that answered (in order of preference), the first that plays, else the first; with its index.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn first_playing<const N: usize>(answers: [Option<Now>; N]) -> Option<(usize, Now)> {
    let found = || answers.iter().enumerate().filter_map(|(index, now)| Some((index, now.as_ref()?)));
    found().find(|(_, now)| now.playing).or_else(|| found().next()).map(|(index, now)| (index, now.clone()))
}

/// Of what the Spotify app (or Spotifast) and Soloist say, the one shown: the one that plays; when both or neither do,
/// the app (it is the one on the screen), else Soloist. Spotifast and a Soloist on the same song are Spotifast playing
/// through its Soloist: one, on both cards.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn pick(app: Option<Now>, soloist: Option<Now>) -> Option<Now> {
    match (app, soloist) {
        (Some(app), Some(soloist)) if app.player == Player::Spotifast && app.track.title == soloist.track.title => Some(Now { through_soloist: true, ..app }),
        (Some(app), Some(soloist)) if soloist.playing && !app.playing => Some(soloist),
        (Some(app), _) => Some(app),
        (None, soloist) => soloist,
    }
}

/// Soloist's `playback_state` (`soloist ctl now --json`, the event of its WebSocket API): the item's name, its creators'
/// (the artists), its parent's (the album) and its cover of the "default" size (else the first); None when it is idle
/// or has nothing loaded. Buffering counts as playing.
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
    let covers = decorations["visual_identity"]["cover"].as_array();
    let cover = covers.and_then(|covers| covers.iter().find(|cover| cover["size"] == "default").or(covers.first())).and_then(|cover| cover["url"].as_str());
    let track = Track::new(
        decorations["identity"]["name"].as_str()?,
        &artists.unwrap_or_default().join(", "),
        &name(&decorations["parent"]["entity"]),
        cover.unwrap_or_default(),
    )?;
    Some(Now { track, playing: matches!(status, "playing" | "buffering"), player: Player::Soloist, ..Default::default() })
}

/// The `soloist ctl` command for a button: play or pause by whether it plays (`playing`), next, prev.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn soloist_command(command: Command, playing: bool) -> &'static str {
    match command {
        Command::PlayPause if playing => "pause",
        Command::PlayPause => "play",
        Command::Next => "next",
        Command::Previous => "prev",
    }
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
fn watch(commands: Receiver<Command>, messages: Sender<Message>, ctx: Context, sources: Sources) {
    let mut watch = Watch::default();
    loop {
        #[cfg(not(any(windows, target_os = "macos")))]
        let now = system::read(sources);
        #[cfg(any(windows, target_os = "macos"))]
        let now = system::read().filter(|_| sources.app);
        for message in watch.step(now, system::cover) {
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

/// Where each system puts the app: Windows (installer or Store), macOS (.app), Linux (PATH, snap, Flatpak).
pub(crate) fn install_places(env: impl Fn(&str) -> Option<String>) -> Vec<std::path::PathBuf> {
    let at = |name: &str, rest: &str| env(name).map(|base| std::path::Path::new(&base).join(rest));
    if cfg!(windows) {
        [at("APPDATA", "Spotify/Spotify.exe"), at("LOCALAPPDATA", "Microsoft/WindowsApps/Spotify.exe")].into_iter().flatten().collect()
    } else if cfg!(target_os = "macos") {
        [Some("/Applications/Spotify.app".into()), at("HOME", "Applications/Spotify.app")].into_iter().flatten().collect()
    } else {
        [Some("/snap/bin/spotify".into()), Some("/var/lib/flatpak/app/com.spotify.Client".into()), at("HOME", ".local/share/flatpak/app/com.spotify.Client")]
            .into_iter()
            .flatten()
            .collect()
    }
}

/// Whether Soloist's program is on this computer (only Linux has it).
fn soloist_installed() -> bool {
    #[cfg(not(any(windows, target_os = "macos")))]
    return soloist::program().is_some();
    #[cfg(any(windows, target_os = "macos"))]
    false
}

/// The Spotify app, or on Linux Spotifast (on the PATH or in `~/.local/bin`).
fn installed() -> bool {
    let linux = !cfg!(any(windows, target_os = "macos"));
    let spotifast = || {
        crate::editor::lsp::which("spotifast").is_some()
            || std::env::var("HOME").is_ok_and(|home| std::path::Path::new(&home).join(".local/bin/spotifast").is_file())
    };
    (linux && (crate::editor::lsp::which("spotify").is_some() || spotifast()))
        || install_places(|name| std::env::var(name).ok()).iter().any(|place| place.exists())
}

#[derive(Default)]
pub(crate) struct Spotify {
    /// What plays; None: Spotify is closed, or has nothing loaded.
    pub(crate) now: Option<Now>,
    /// The cover of what plays, once it came.
    pub(crate) cover: Option<egui::TextureHandle>,
    /// The Spotify app (or Spotifast) and Soloist are not installed here: looked once, when the integration turned on.
    pub(crate) missing: bool,
    pub(crate) soloist_missing: bool,
    /// The players the thread asks (Ajustes' two switches).
    sources: Sources,
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
                format!("{}: {}{by}{}", if now.playing { "Tocando" } else { "Pausado" }, now.track.title, now.from())
            }
            None if self.missing => "O Spotify não está instalado neste computador.".into(),
            None => "O Spotify não está aberto neste computador (ou não tem nada carregado).".into(),
        }
    }

    /// What one card's players play (Spotify's: `App`, the app and Spotifast; Soloist's), for Ajustes and the MCP: the
    /// report while one of them is shown, else why not.
    pub(crate) fn report_of(&self, card: Player) -> String {
        let linux = !cfg!(any(windows, target_os = "macos"));
        match (&self.now, card) {
            (Some(now), _) if now.on(card) => self.report(),
            (_, Player::Soloist) if self.soloist_missing => "O Soloist não está instalado (em ~/.local/bin, no PATH ou pelo Spotifast).".into(),
            (_, Player::Soloist) => "O Soloist não está tocando, ou foi aberto sem a API local (-w 127.0.0.1:0).".into(),
            (_, _) if self.missing && linux => "Nem o Spotify nem o Spotifast estão instalados neste computador.".into(),
            (_, _) if self.missing => "O Spotify não está instalado neste computador.".into(),
            (Some(_), _) => "O Spotify não está tocando neste computador (o Soloist está).".into(),
            (None, _) if linux => "O Spotify (ou o Spotifast) não está aberto neste computador (ou não tem nada carregado).".into(),
            (None, _) => "O Spotify não está aberto neste computador (ou não tem nada carregado).".into(),
        }
    }
}

impl DrawingApp {
    /// The players Ajustes turned on: the Spotify app, and Soloist (only on Linux).
    pub(crate) fn spotify_sources(&self) -> Sources {
        let integrations = &self.prefs.integrations;
        Sources { app: integrations.spotify, soloist: integrations.soloist && cfg!(not(any(windows, target_os = "macos"))) }
    }

    /// Each frame: on, the thread that asks the players runs and its news are taken; off, it ends and nothing stays. A
    /// switch turned on or off starts it again, asking the players that are on.
    pub(crate) fn spotify_tick(&mut self, ctx: &Context) {
        let sources = self.spotify_sources();
        let spotify = &mut self.integrations.spotify;
        if sources == Sources::default() || (spotify.thread.is_some() && spotify.sources != sources) {
            if spotify.thread.is_some() || spotify.now.is_some() {
                *spotify = Spotify::default();
            }
            if sources == Sources::default() {
                return;
            }
        }
        // never the real player in a test
        if spotify.thread.is_none() && !cfg!(test) {
            let ((commands, inbox), (outbox, messages)) = (channel(), channel());
            let ctx = ctx.clone();
            std::thread::spawn(move || watch(inbox, outbox, ctx, sources));
            spotify.thread = Some((commands, messages));
            (spotify.sources, spotify.missing, spotify.soloist_missing) = (sources, !installed(), !soloist_installed());
        }
        while let Some(message) = spotify.thread.as_ref().and_then(|(_, messages)| messages.try_recv().ok()) {
            spotify.take(message, ctx);
        }
    }

    /// Spotify in words, for the MCP's `integrations`.
    pub(crate) fn spotify_report(&self) -> String {
        if self.prefs.integrations.spotify { self.integrations.spotify.report_of(Player::App) } else { "desligado.".into() }
    }

    /// Soloist in words, for the MCP's `integrations` (only on Linux).
    pub(crate) fn soloist_report(&self) -> String {
        if self.spotify_sources().soloist { self.integrations.spotify.report_of(Player::Soloist) } else { "desligado.".into() }
    }

    /// Play/pause, next or previous, asked of the player; false when the integration is off or Spotify is closed.
    pub(crate) fn spotify_control(&mut self, command: Command) -> bool {
        let on = self.spotify_sources() != Sources::default();
        let spotify = &mut self.integrations.spotify;
        if !on || spotify.now.is_none() {
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
            kind: format!("{}{}", if now.playing { "tocando" } else { "pausado" }, now.place()),
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
