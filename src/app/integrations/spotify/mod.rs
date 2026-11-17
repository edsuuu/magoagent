//! Spotify on the island, as the reference's (MIT) Spotify pill and its Spotify page: what plays (the title, the artist,
//! the cover, where it comes from, the time, shuffle, repeat, the volume) and its controls. No account and no key: the
//! Spotify open on this computer is asked, by the system's media controls on Windows (`smtc`), AppleScript on macOS
//! (`applescript`) and on Linux MPRIS (`mpris`, the app and Spotifast), and Spotify's Soloist there too, by its local
//! API (`soloist`, its own or the one Spotifast starts; `linux` shows the one that plays). With the Spotify account
//! linked in Ajustes (`account`), the Web API (`web`) adds what no player says: the queue of the app, Liked Songs, the
//! playlists, a search, the heart. The page's lists and its polls are in `library`, the controls and where each goes in
//! `controls`. Off, nothing is asked; with Spotify closed, there is no pill.
// ponytail: asked every two seconds instead of listening to the player's events; listen when the delay bothers.

use super::{IslandItem, ItemRow};
use crate::prelude::*;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};

pub(crate) mod account;
#[cfg(target_os = "macos")]
mod applescript;
pub(crate) mod controls;
pub(crate) mod fake;
pub(crate) mod groove;
pub(crate) mod library;
#[cfg(not(any(windows, target_os = "macos")))]
mod linux;
#[cfg(not(any(windows, target_os = "macos")))]
mod mpris;
pub(crate) mod playback;
pub(crate) mod queue;
mod read;
pub(crate) mod setup;
#[cfg(windows)]
mod smtc;
#[cfg(not(any(windows, target_os = "macos")))]
mod soloist;
pub(crate) mod web;
#[cfg(target_os = "macos")]
use applescript as system;
#[cfg(not(any(windows, target_os = "macos")))]
use linux as system;
#[cfg(windows)]
use smtc as system;
#[cfg(test)]
mod tests;

// what the island's Spotify page reads (island/), named here
pub(crate) use library::PlaylistItem;
pub(crate) use playback::{Playback, Repeat};
pub(crate) use queue::QueueItem;
pub(crate) use read::*;

/// Between two questions to the player, and how long it takes to say what a request changed.
const EVERY: std::time::Duration = std::time::Duration::from_secs(2);
const SETTLE: std::time::Duration = std::time::Duration::from_millis(350);
/// The biggest cover read (Spotify's are 640 px JPEGs, about 100 KB), and how many polls a track's cover is asked for.
pub(crate) const COVER_LIMIT: usize = 3 * 1024 * 1024;
const COVER_TRIES: u8 = 4;
const GREEN: Color32 = Color32::from_rgb(0x1e, 0xd7, 0x60);
/// How long the cover now takes to replace the one before it on the island's pill (asked: the change between songs was
/// abrupt), and how long that one is kept while the new cover is still to come (past it, a song without a cover shows
/// the placeholder, as it always did).
const FADE: f64 = 0.3;
const COVER_WAIT: f64 = 4.0;

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
    /// Shuffle, repeat, the volume, the time, where it comes from and the heart, as far as the player said.
    pub(crate) playback: Playback,
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

/// What is asked of the player shown (or, by `playback::route`, of the Web API).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    PlayPause,
    Next,
    Previous,
    Shuffle(bool),
    Repeat(Repeat),
    /// 0 to 100.
    Volume(u8),
    /// Where in the song, ms.
    Seek(u64),
    /// A track, an album, a playlist or Liked Songs, now.
    PlayUri(String),
    /// A track to the queue.
    Enqueue(String),
}

impl Command {
    /// "playpause", "next" or "previous" (the island's chips and the MCP), and the page's "shuffle on|off", "repeat
    /// off|context|track", "volume <0-100>", "seek <ms>", "play <uri>" and "enqueue <track uri>".
    pub(crate) fn parse(text: &str) -> Option<Command> {
        let text = text.trim();
        let (name, value) = text.split_once(' ').map_or((text, ""), |(name, value)| (name, value.trim()));
        match (name, value) {
            ("playpause", "") => Some(Command::PlayPause),
            ("next", "") => Some(Command::Next),
            ("previous", "") => Some(Command::Previous),
            ("shuffle", "on" | "true") => Some(Command::Shuffle(true)),
            ("shuffle", "off" | "false") => Some(Command::Shuffle(false)),
            ("repeat", repeat) => Repeat::parse(repeat).map(Command::Repeat),
            ("volume", percent) => percent.parse::<u8>().ok().filter(|percent| *percent <= 100).map(Command::Volume),
            ("seek", ms) => ms.parse::<u64>().ok().map(Command::Seek),
            ("play", uri) if playback::uri_allowed(uri) => Some(Command::PlayUri(uri.to_string())),
            ("enqueue", uri) if playback::is_track(uri) => Some(Command::Enqueue(uri.to_string())),
            _ => None,
        }
    }
}

/// The cover at `url`, when it is Spotify's and of a sane size.
pub(crate) fn download_cover(url: &str) -> Option<Vec<u8>> {
    if !cover_allowed(url) {
        return None;
    }
    let request = super::http::Request { method: "GET", url, headers: vec!["User-Agent: MagoAgent".into()], body: None, timeout_seconds: 10 };
    let (status, bytes) = super::http::send_bytes(&request).ok()?;
    (status == 200 && !bytes.is_empty() && bytes.len() <= COVER_LIMIT).then_some(bytes)
}

/// What the app asks of the thread: a control for the player shown, or what plays next (Soloist's queue, `most` tracks).
enum Ask {
    Control(Command),
    Queue(usize),
}

enum Message {
    Now(Option<Now>),
    Cover(Track, Vec<u8>),
    /// What plays next, from the player; None when it has no queue to give (MPRIS, the system's session) or failed.
    Queue(Option<Vec<QueueItem>>),
    /// A control the player did not take.
    Refused(Command),
}

/// What the thread remembers between two questions: what it last told, and the cover it still owes.
#[derive(Default)]
struct Watch {
    told: Option<Option<Now>>,
    covered: Option<Track>,
    tries: u8,
    /// A control was sent: the next state is told even when it looks the same (the page showed the change already).
    dirty: bool,
}

impl Watch {
    /// What to tell the app after the player said `now`: the state when it changed (a song playing on is no change), and
    /// the track's cover once (`cover` fetches it; it may come a moment after the title, so it is asked for a few polls).
    fn step(&mut self, now: Option<Now>, cover: impl FnOnce(&Track) -> Option<Vec<u8>>) -> Vec<Message> {
        let mut messages = Vec::new();
        if std::mem::take(&mut self.dirty) || playback::differs(self.told.as_ref(), &now) {
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

/// The thread that asks the player while the integration is on; it ends when the app drops its side of `asks`.
fn watch(asks: Receiver<Ask>, messages: Sender<Message>, ctx: Context, sources: Sources) {
    let mut watch = Watch::default();
    let send = |message: Message| {
        let sent = messages.send(message).is_ok();
        ctx.request_repaint();
        sent
    };
    loop {
        #[cfg(not(any(windows, target_os = "macos")))]
        let now = system::read(sources);
        #[cfg(any(windows, target_os = "macos"))]
        let now = system::read().filter(|_| sources.app);
        for message in watch.step(now, system::cover) {
            if !send(message) {
                return;
            }
        }
        match asks.recv_timeout(EVERY) {
            Ok(Ask::Control(command)) => {
                if !system::control(&command) && !send(Message::Refused(command)) {
                    return;
                }
                watch.dirty = true;
                std::thread::sleep(SETTLE);
            }
            Ok(Ask::Queue(most)) => {
                if !send(Message::Queue(system::queue(most))) {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// The wheel cut in this many hues, each 30°: the pixels are gathered by them, and the two colours of the bars are the
/// two that weigh most.
const HUE_BUCKETS: usize = 12;
/// The grey of a cover with no colour to give, as the Dynamic Island's dots are for it.
const GREY: Color32 = Color32::from_gray(0xa8);

/// The liveliest colour of a cover (its RGBA pixels), made bright enough for the black island: each pixel weighs by how
/// vivid and how light it is, and the liveliest hue's own colour wins; a grey cover gives grey, as the Dynamic Island's
/// dots are for it. It is the first of the pair `accents` gives (the bars start with it and Luna takes it, through
/// `tint`), kept for whoever reads one colour at a time — the tests today.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn accent(rgba: &[u8]) -> Color32 {
    accents(rgba).0
}

/// The two liveliest colours of a cover, left to right for the island's bars: the liveliest hue (what `accent` gives)
/// and the liveliest one at least 60° from it — a lighter tone of the first when the cover has only one colour to give.
pub(crate) fn accents(rgba: &[u8]) -> (Color32, Color32) {
    let (mut weight, mut sums, mut weights) = (0.0f32, [[0.0f32; 3]; HUE_BUCKETS], [0.0f32; HUE_BUCKETS]);
    for pixel in rgba.as_chunks::<4>().0 {
        let colour = [pixel[0], pixel[1], pixel[2]].map(f32::from);
        let Some(each) = vividness(colour, pixel[3]) else { continue };
        let around = (hue(colour) / (360.0 / HUE_BUCKETS as f32)) as usize % HUE_BUCKETS;
        for (total, channel) in sums[around].iter_mut().zip(colour) {
            *total += channel * each;
        }
        (weight, weights[around]) = (weight + each, weights[around] + each);
    }
    if weight < 0.02 * (rgba.len() / 4).max(1) as f32 {
        return (GREY, lighter(GREY));
    }
    let mean = |around: usize| lifted(sums[around].map(|channel| channel / weights[around]));
    let liveliest = (0..HUE_BUCKETS).max_by(|one, other| weights[*one].total_cmp(&weights[*other])).unwrap_or(0);
    let first = mean(liveliest);
    let first_hue = hue(sums[liveliest]);
    // another colour: a hue with some presence in the cover, at least 60° from the first one's
    let second = (0..HUE_BUCKETS)
        .filter(|around| weights[*around] >= 0.05 * weight && apart(*around, first_hue))
        .max_by(|one, other| weights[*one].total_cmp(&weights[*other]))
        .map(mean);
    (first, second.unwrap_or_else(|| lighter(first)))
}

/// How much a pixel counts for its colour: by how vivid it is, how light and how opaque; None for a pixel too dark or
/// too grey to tell a colour.
fn vividness([red, green, blue]: [f32; 3], alpha: u8) -> Option<f32> {
    let (most, least) = (red.max(green).max(blue), red.min(green).min(blue));
    (most >= 1.0).then(|| {
        let vivid = (most - least) / most;
        vivid * vivid * (most / 255.0) * f32::from(alpha) / 255.0
    })
}

/// Where a colour sits on the wheel, in degrees, 0 to 360.
fn hue([red, green, blue]: [f32; 3]) -> f32 {
    let (most, least) = (red.max(green).max(blue), red.min(green).min(blue));
    let span = most - least;
    if span <= 0.0 {
        return 0.0;
    }
    let turn = if most == red {
        ((green - blue) / span).rem_euclid(6.0)
    } else if most == green {
        (blue - red) / span + 2.0
    } else {
        (red - green) / span + 4.0
    };
    turn * 60.0
}

/// Whether the `around`-th hue of the wheel is at least 60° from `degrees`: another colour, not a shade of that one.
fn apart(around: usize, degrees: f32) -> bool {
    let middle = (around as f32 + 0.5) * (360.0 / HUE_BUCKETS as f32);
    let angle = (middle - degrees).abs().rem_euclid(360.0);
    angle.min(360.0 - angle) >= 60.0
}

/// A colour made bright enough for the black island: its channels lifted so its brightest comes to 235.
fn lifted(mean: [f32; 3]) -> Color32 {
    let lift = 235.0 / mean.iter().copied().fold(1.0, f32::max);
    let [red, green, blue] = mean.map(|channel| (channel * lift).clamp(0.0, 255.0) as u8);
    Color32::from_rgb(red, green, blue)
}

/// A lighter tone of `colour`, for a cover with a single colour to give.
fn lighter(colour: Color32) -> Color32 {
    colour.lerp_to_gamma(Color32::WHITE, 0.45)
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
    /// The cover's liveliest colour (`accent`), for the island's bars as the Dynamic Island colours them.
    pub(crate) tint: Option<Color32>,
    /// The cover's two liveliest colours (`accents`), left to right for the island's bars.
    pub(crate) colours: Option<(Color32, Color32)>,
    /// The cover of the song before and its colours: they stay while the new song's cover is still to come, and go
    /// under it while it fades in (`fade`, `island/pill.rs`).
    previous: Option<egui::TextureHandle>,
    before: Option<(Color32, Color32)>,
    /// When the cover now came, and when the song changed: the fade runs from the first, and the cover before it is let
    /// go `COVER_WAIT` after the second when the new one never comes.
    replaced_at: f64,
    waiting_since: Option<f64>,
    /// The Spotify app (or Spotifast) and Soloist are not installed here: looked once, when the integration turned on.
    pub(crate) missing: bool,
    pub(crate) soloist_missing: bool,
    /// The island's Spotify page: its lists (A seguir, Playlists, Curtidas), what a search found, the devices and the
    /// last error; asked only while the page is open (`DrawingApp::spotify_page_open`).
    pub(crate) library: library::Library,
    /// The Spotify account (the Web API), linked in Ajustes; it outlives the switches.
    pub(crate) account: account::Account,
    /// The players the thread asks (Ajustes' two switches).
    sources: Sources,
    /// Luna dancing to what plays: the beat heard in the computer's sound while the island draws her (`groove`).
    pub(crate) dancer: groove::Dancer,
    /// The thread's two ends, while the integration is on: dropping them ends it.
    thread: Option<(Sender<Ask>, Receiver<Message>)>,
    /// What was asked of the player, for the tests (they have no thread).
    #[cfg(test)]
    pub(crate) asked: Vec<Command>,
}

/// What the island's pill (`island/pill.rs`) draws of the cover beyond what the island hands it (the cover and the
/// first colour): the cover of the song before, under the new one while it fades in, the bars' two colours and the pair
/// they replace, and when the new one came. `spotify_tick` leaves it in the context every frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CoverFade {
    /// The cover of the song before: drawn while the new one is still to come, and under it as it fades in.
    pub(crate) previous: Option<egui::TextureId>,
    /// The bars' two colours (`accents`), left to right; None while the cover is still to come.
    pub(crate) colours: Option<(Color32, Color32)>,
    /// The colours of the song before, and the instant the new ones came: the bars go from one pair to the other.
    pub(crate) before: Option<(Color32, Color32)>,
    pub(crate) at: f64,
}

impl CoverFade {
    /// Where the tick leaves it for the pill.
    fn slot() -> egui::Id {
        egui::Id::unique("spotify-cover-fade")
    }

    /// What the tick left for the pill; the default (no cover, no colours) before it runs.
    pub(crate) fn read(ctx: &Context) -> CoverFade {
        ctx.data(|data| data.get_temp(Self::slot())).unwrap_or_default()
    }

    /// How far the cover now is in, 0 to 1 at `time` (the island's clock); 0 while it is still to come.
    pub(crate) fn progress(&self, time: f64) -> f32 {
        match (self.colours, self.before) {
            (None, _) => 0.0,
            (Some(_), None) => 1.0,
            (Some(_), Some(_)) => ((time - self.at) / FADE).clamp(0.0, 1.0) as f32,
        }
    }

    /// Whether the cover and its colours are still coming in at `time`: the pill asks for frames meanwhile.
    pub(crate) fn fading(&self, time: f64) -> bool {
        self.colours.is_some() && self.before.is_some() && self.progress(time) < 1.0
    }

    /// The bars' two colours at `time`: the ones before while the new cover is still to come, and then the new pair, on
    /// its way from the old one while it fades in.
    pub(crate) fn colours_at(&self, time: f64) -> Option<(Color32, Color32)> {
        let Some(colours) = self.colours else { return self.before };
        let Some(before) = self.before else { return Some(colours) };
        let progress = self.progress(time);
        Some((before.0.lerp_to_gamma(colours.0, progress), before.1.lerp_to_gamma(colours.1, progress)))
    }
}

impl Spotify {
    /// The integration turned off (or its players changed): the thread ends and nothing of what played stays; the
    /// account stays linked.
    fn stop(&mut self) {
        let account = std::mem::take(&mut self.account);
        *self = Spotify { account, ..Default::default() };
    }

    /// What the pill draws of the cover: the one before, under the new one while it fades in, and the two colours of the
    /// bars with the pair they replace.
    pub(crate) fn fade(&self) -> CoverFade {
        CoverFade { previous: self.previous.as_ref().map(egui::TextureHandle::id), colours: self.colours, before: self.before, at: self.replaced_at }
    }

    /// Nothing of a cover stays: the placeholder and Spotify's green, as with a song that never had one.
    fn cover_gone(&mut self) {
        (self.cover, self.tint, self.colours, self.previous, self.before, self.waiting_since) = (None, None, None, None, None, None);
    }

    fn take(&mut self, message: Message, ctx: &Context) {
        match message {
            Message::Now(now) => {
                let changed = now.as_ref().map(|now| &now.track) != self.now.as_ref().map(|now| &now.track);
                if changed {
                    self.dancer.new_song();
                }
                match &now {
                    // closed: nothing of the cover stays
                    None => self.cover_gone(),
                    // another song: the cover and the colours before stay, under the new song's once it comes (asked:
                    // the change between songs was abrupt)
                    Some(_) if changed => {
                        (self.previous, self.before) = (self.cover.take(), self.colours.take());
                        // the colour before stays too (the bars without the fade fall back to it), until the new cover gives its own
                        self.waiting_since = Some(ctx.input(|input| input.time));
                    }
                    Some(_) => {}
                }
                self.now = now;
                self.enrich();
            }
            Message::Queue(queue) => self.library.take_player_queue(queue),
            Message::Refused(command) => self.library.fail(controls::refused(&command), now_ms()),
            Message::Cover(track, bytes) if self.now.as_ref().is_some_and(|now| now.track == track) && bytes.len() <= COVER_LIMIT => {
                // what the player handed is read as any picture from outside: decoded, never run
                if let Ok(picture) = image::load_from_memory(&bytes) {
                    let picture = picture.thumbnail(96, 96).into_rgba8();
                    let size = [picture.width() as usize, picture.height() as usize];
                    let colours = accents(&picture);
                    // the cover that was showing goes under the new one, and the fade starts here
                    if let Some(cover) = self.cover.take() {
                        (self.previous, self.before) = (Some(cover), self.colours);
                    }
                    (self.tint, self.colours) = (Some(colours.0), Some(colours));
                    self.cover =
                        Some(ctx.load_texture("spotify-cover", egui::ColorImage::from_rgba_unmultiplied(size, &picture), egui::TextureOptions::LINEAR));
                    (self.replaced_at, self.waiting_since) = (ctx.input(|input| input.time), None);
                }
            }
            Message::Cover(..) => {}
        }
    }

    /// Leaves the pill what it draws of the cover (`CoverFade`), letting go of the cover before it once the fade is
    /// over, or once the new one has taken too long to come.
    fn publish(&mut self, ctx: &Context) {
        let time = ctx.input(|input| input.time);
        if self.waiting_since.is_some_and(|since| time - since >= COVER_WAIT) {
            self.cover_gone();
        }
        if self.colours.is_some() && self.before.is_some() && time - self.replaced_at >= FADE {
            (self.previous, self.before) = (None, None);
        }
        let fade = self.fade();
        ctx.data_mut(|data| data.insert_temp(CoverFade::slot(), fade));
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

    /// What plays beyond the title, for the MCP: "de Músicas curtidas · 0:46 de 2:47 · embaralhar ligado · repetindo a
    /// lista · volume 47% · curtida"; empty when the player said none of it.
    pub(crate) fn details(&self) -> String {
        let Some(now) = &self.now else { return String::new() };
        let playback = &now.playback;
        let clock = |ms: u64| format!("{}:{:02}", ms / 60_000, ms / 1000 % 60);
        let mut parts = Vec::new();
        if let Some(context) = &playback.context
            && !context.label().is_empty()
        {
            parts.push(format!("de {}", context.label()));
        }
        match (now.position_at(now_ms()), playback.duration_ms) {
            (Some(position), Some(duration)) => parts.push(format!("{} de {}", clock(position), clock(duration))),
            (None, Some(duration)) => parts.push(format!("duração {}", clock(duration))),
            _ => {}
        }
        if let Some(shuffle) = playback.shuffle {
            parts.push(format!("embaralhar {}", if shuffle { "ligado" } else { "desligado" }));
        }
        if let Some(repeat) = playback.repeat {
            parts.push(repeat.words().into());
        }
        if let Some(volume) = playback.volume {
            parts.push(format!("volume {}%", (volume * 100.0).round()));
        }
        match playback.liked {
            Some(true) => parts.push("nas Músicas curtidas".into()),
            Some(false) => parts.push("fora das Músicas curtidas".into()),
            None => {}
        }
        if !playback.track_uri.is_empty() {
            parts.push(playback.track_uri.clone());
        }
        parts.join(" · ")
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
    /// switch turned on or off starts it again, asking the players that are on. What the pill draws of the cover is
    /// left in the context for it (`CoverFade`, `island/pill.rs`).
    pub(crate) fn spotify_tick(&mut self, ctx: &Context) {
        self.spotify_ask(ctx);
        self.integrations.spotify.publish(ctx);
    }

    /// The players asked, and what they said.
    fn spotify_ask(&mut self, ctx: &Context) {
        let sources = self.spotify_sources();
        let spotify = &mut self.integrations.spotify;
        if sources == Sources::default() || (spotify.thread.is_some() && spotify.sources != sources) {
            if spotify.thread.is_some() || spotify.now.is_some() {
                spotify.stop();
            }
            if sources == Sources::default() {
                return;
            }
        }
        // a test scene's: made up here, nothing else is asked (and nothing heard)
        if fake::active() {
            if spotify.now.is_none() {
                spotify.now = Some(fake::now());
                fake::fill(&mut spotify.library);
            }
            return;
        }
        // never the real player in a test
        if spotify.thread.is_none() && !cfg!(test) {
            let ((asks, inbox), (outbox, messages)) = (channel(), channel());
            let ctx = ctx.clone();
            std::thread::spawn(move || watch(inbox, outbox, ctx, sources));
            spotify.thread = Some((asks, messages));
            (spotify.sources, spotify.missing, spotify.soloist_missing) = (sources, !installed(), !soloist_installed());
        }
        while let Some(message) = spotify.thread.as_ref().and_then(|(_, messages)| messages.try_recv().ok()) {
            spotify.take(message, ctx);
        }
        let playing = spotify.now.as_ref().is_some_and(|now| now.playing);
        // the island's bars listen too, not only Luna's dance
        spotify.dancer.tick(self.prefs.integrations.luna_dances || self.prefs.island, playing);
        self.spotify_account_tick();
        self.spotify_library_tick(ctx);
    }

    /// Spotify in words, for the MCP's `integrations`.
    pub(crate) fn spotify_report(&self) -> String {
        if self.prefs.integrations.spotify { self.spotify_report_of(Player::App) } else { "desligado.".into() }
    }

    /// Soloist in words, for the MCP's `integrations` (only on Linux).
    pub(crate) fn soloist_report(&self) -> String {
        if self.spotify_sources().soloist { self.spotify_report_of(Player::Soloist) } else { "desligado.".into() }
    }

    /// A card's report, what plays beyond the title, and the account when there is one.
    fn spotify_report_of(&self, card: Player) -> String {
        let spotify = &self.integrations.spotify;
        let mut lines = vec![spotify.report_of(card)];
        // what plays, wherever: the agent asking either card wants to know it
        if spotify.now.is_some() {
            lines.push(spotify.details());
        }
        let account = &spotify.account;
        lines.push(match &account.link {
            account::Link::Linked if account.name.is_empty() => "Conta do Spotify conectada.".into(),
            account::Link::Linked => format!("Conta do Spotify conectada como {}.", account.name),
            account::Link::Waiting => "Conta do Spotify: esperando o login no navegador.".into(),
            account::Link::Failed(why) => format!("Conta do Spotify: {why}"),
            account::Link::Off => String::new(),
        });
        lines.retain(|line| !line.is_empty());
        lines.join("\n")
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
