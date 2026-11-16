//! The island's Spotify page, the state it reads: A seguir (the queue), Playlists, Curtidas (Liked Songs), what a search
//! found, the account's devices, the heart of the track, where it comes from, the covers of the lists, and the last error
//! in a short line. Nothing is asked while the page is closed: the island marks it open each frame it draws it
//! (`DrawingApp::spotify_page_open`), and then the queue and the player are asked every five seconds, the lists every two
//! minutes. The Web API's calls (`web.rs`) run on threads of their own and answer by a channel.

use super::account::{Link, Shared};
use super::playback::{self, PlayContext};
use super::web::{self, Failure, Profile, WebPlayer};
use super::{Ask, Command, Player, QueueItem, Spotify};
use crate::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};

/// While the page is open: the queue and the player, and the lists.
pub(crate) const PAGE_EVERY_MS: i64 = 5_000;
const LISTS_EVERY_MS: i64 = 120_000;
/// The page counts as open this long after it was last drawn.
const PAGE_GRACE_MS: i64 = 1_500;
/// How long an error stays.
pub(crate) const ERROR_FOR_MS: i64 = 6_000;
/// What the queue shows.
pub(crate) const QUEUE_MOST: usize = 10;
/// The covers kept for the lists, and how many come down at once.
const THUMBS_MOST: usize = 200;
const THUMBS_AT_ONCE: usize = 4;
const THUMB_SIZE: u32 = 64;
pub(crate) const QUEUE_NEEDS_ACCOUNT: &str = "A fila deste player só aparece com a conta do Spotify conectada em Ajustes › Integrações.";
pub(crate) const LISTS_NEED_ACCOUNT: &str = "Playlists, Curtidas e a busca aparecem com a conta do Spotify conectada em Ajustes › Integrações.";

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PlaylistItem {
    /// "spotify:playlist:…".
    pub(crate) uri: String,
    pub(crate) name: String,
    pub(crate) owner: String,
    pub(crate) cover_url: String,
    /// How many tracks, when Spotify said.
    pub(crate) total: Option<u64>,
}

/// What a search found.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Found {
    pub(crate) query: String,
    pub(crate) tracks: Vec<QueueItem>,
    pub(crate) playlists: Vec<PlaylistItem>,
}

/// One of the account's Spotify Connect devices (this computer's app, Soloist, the phone…).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Device {
    pub(crate) id: String,
    pub(crate) name: String,
    /// "Computer", "Smartphone", "Speaker"…
    pub(crate) kind: String,
    pub(crate) active: bool,
    pub(crate) volume: Option<f32>,
}

/// What a Web API thread answers.
pub(crate) enum Reply {
    Queue(Result<Vec<QueueItem>, Failure>),
    Playlists(Result<Vec<PlaylistItem>, Failure>),
    Liked(Result<Vec<QueueItem>, Failure>),
    Found(Result<Found, Failure>),
    Contains(String, Result<bool, Failure>),
    Saved(String, bool, Result<(), Failure>),
    Player(Result<Option<WebPlayer>, Failure>),
    Devices(Result<Vec<Device>, Failure>),
    ContextName(String, Result<String, Failure>),
    Profile(Result<Profile, Failure>),
    /// A control done by the Web API.
    Done(Result<(), Failure>),
    /// Something to play or queue as a press on the page would (the MCP's "the first hit").
    Then(Command),
}

/// The covers of the lists, by address: asked by the page as it draws (`DrawingApp::spotify_thumbnail`), downloaded on
/// threads, made textures by the next tick.
#[derive(Default)]
struct Thumbs {
    ready: HashMap<String, egui::TextureHandle>,
    wanted: RefCell<Vec<String>>,
    asked: HashSet<String>,
    loading: usize,
    channel: Option<(Sender<(String, Option<Vec<u8>>)>, Receiver<(String, Option<Vec<u8>>)>)>,
}

#[derive(Default)]
pub(crate) struct Library {
    /// A seguir: what plays next, from Soloist or from the account.
    pub(crate) queue: Vec<QueueItem>,
    /// Why the queue is empty when no one can give it (the app's player without the account).
    pub(crate) queue_note: Option<String>,
    /// The account's playlists and Liked Songs (the last 50 each).
    pub(crate) playlists: Vec<PlaylistItem>,
    pub(crate) liked: Vec<QueueItem>,
    /// Why Playlists, Curtidas and the search are empty: no account.
    pub(crate) lists_note: Option<String>,
    /// What the last search found, and whether one is out.
    pub(crate) found: Option<Found>,
    pub(crate) searching: bool,
    /// The account's devices (the "Soloist ▾" menu); empty without the account.
    pub(crate) devices: Vec<Device>,
    /// The last thing that went wrong, in a short pt-BR line, and when (ms): shown a few seconds.
    pub(crate) error: Option<(String, i64)>,
    /// What the Web API says plays: where it comes from, the device, the volume where no player says them.
    pub(crate) web_player: Option<WebPlayer>,
    /// Whether each track is in Liked Songs, as the account said (and as a press changed it).
    pub(crate) hearts: HashMap<String, bool>,
    /// Playlists' names by uri, for "de onde".
    context_names: HashMap<String, String>,
    /// The query the page waits for (an older search's answer is dropped).
    query: String,
    /// When the page was last drawn (ms).
    page_seen: Cell<i64>,
    next_page: i64,
    next_lists: i64,
    next_profile: i64,
    busy: HashSet<&'static str>,
    thumbs: Thumbs,
    /// Counts the accounts: an answer for an earlier one is dropped.
    generation: u64,
    channel: Option<(Sender<(u64, Reply)>, Receiver<(u64, Reply)>)>,
    /// The Web API's calls asked, for the tests (they reach no network).
    #[cfg(test)]
    pub(crate) web_asked: Vec<&'static str>,
}

impl Library {
    pub(crate) fn page_open(&self, now: i64) -> bool {
        now - self.page_seen.get() < PAGE_GRACE_MS
    }

    /// Something went wrong: the page says it a moment.
    pub(crate) fn fail(&mut self, message: impl Into<String>, now: i64) {
        let message = message.into();
        if !message.is_empty() {
            self.error = Some((message, now));
        }
    }

    /// The account changed (linked, unlinked): what it brought goes, and the lists are asked again.
    pub(crate) fn reset_lists(&mut self) {
        self.generation += 1;
        (self.playlists, self.liked, self.found, self.devices, self.web_player) = (Vec::new(), Vec::new(), None, Vec::new(), None);
        (self.hearts, self.context_names, self.searching) = (HashMap::new(), HashMap::new(), false);
        self.busy.clear();
        (self.next_page, self.next_lists) = (0, 0);
    }

    /// Soloist's queue, from the player's thread (None: it gave none).
    pub(super) fn take_player_queue(&mut self, queue: Option<Vec<QueueItem>>) {
        self.busy.remove("queue");
        if let Some(queue) = queue {
            (self.queue, self.queue_note) = (queue, None);
        }
    }

    /// The search whose answer the page waits for.
    pub(super) fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
    }

    /// The answer channel's sender and the account's generation, for a thread.
    pub(crate) fn sender(&mut self) -> (u64, Sender<(u64, Reply)>) {
        (self.generation, self.channel.get_or_insert_with(channel).0.clone())
    }
}

impl Spotify {
    /// A Web API call on a thread of its own, unless one of its `kind` is out; never in a test (it is written down).
    pub(super) fn spawn_web(&mut self, kind: &'static str, work: impl FnOnce(&Shared) -> Reply + Send + 'static) {
        // a control, a heart and a search go every time (an older search's answer is dropped); a poll once at a time
        let polled = !matches!(kind, "control" | "save" | "search");
        if polled && !self.library.busy.insert(kind) {
            return;
        }
        #[cfg(test)]
        {
            self.library.web_asked.push(kind);
            self.library.busy.remove(kind);
            let _ = work;
        }
        #[cfg(not(test))]
        {
            let (generation, sender) = self.library.sender();
            let shared = self.account.shared.clone();
            std::thread::spawn(move || drop(sender.send((generation, work(&shared)))));
        }
    }

    /// What plays, with what the account knows and the player did not say: the heart, where it comes from (and its
    /// name), and from the Web API's view of the same track shuffle, repeat, the duration and the volume.
    pub(super) fn enrich(&mut self) {
        let library = &self.library;
        let Some(now) = self.now.as_mut() else { return };
        let playback = &mut now.playback;
        let same = |web: &&WebPlayer| {
            if playback.track_uri.is_empty() { !web.title.is_empty() && web.title == now.track.title } else { web.track_uri == playback.track_uri }
        };
        if let Some(web) = library.web_player.as_ref().filter(same) {
            if playback.track_uri.is_empty() {
                playback.track_uri = web.track_uri.clone();
            }
            playback.context = playback.context.take().or(web.context.clone());
            playback.shuffle = playback.shuffle.or(web.shuffle);
            playback.repeat = playback.repeat.or(web.repeat);
            playback.duration_ms = playback.duration_ms.or(web.duration_ms);
            if let Some(device) = &web.device {
                playback.volume = playback.volume.or(device.volume);
                // the name the account gives this device ("Soloist" stays: it is the one shown on the island)
                if playback.device.is_empty() || playback.device == "Spotify" {
                    playback.device = device.name.clone();
                }
            }
        }
        if let Some(context) = playback.context.as_mut()
            && context.name.is_empty()
        {
            context.name = match context.kind() {
                "album" => now.track.album.clone(),
                _ => library.context_names.get(&context.uri).cloned().unwrap_or_default(),
            };
        }
        if let Some(liked) = library.hearts.get(&playback.track_uri) {
            playback.liked = Some(*liked);
        } else if !self.account.linked() {
            playback.liked = None;
        }
    }

    /// An answer of a Web API thread; what it asks next (the MCP's first hit) comes back.
    pub(super) fn take_reply(&mut self, reply: Reply, now: i64) -> Option<Command> {
        let library = &mut self.library;
        let mut failed = None;
        match reply {
            Reply::Queue(result) => {
                library.busy.remove("queue");
                match result {
                    Ok(queue) => (library.queue, library.queue_note) = (queue, None),
                    Err(failure) => failed = Some(failure),
                }
            }
            Reply::Playlists(result) => {
                library.busy.remove("playlists");
                match result {
                    Ok(playlists) => {
                        for playlist in &playlists {
                            library.context_names.insert(playlist.uri.clone(), playlist.name.clone());
                        }
                        library.playlists = playlists;
                    }
                    Err(failure) => failed = Some(failure),
                }
            }
            Reply::Liked(result) => {
                library.busy.remove("liked");
                match result {
                    Ok(liked) => {
                        for item in liked.iter().filter(|item| !item.uri.is_empty()) {
                            library.hearts.insert(item.uri.clone(), true);
                        }
                        library.liked = liked;
                    }
                    Err(failure) => failed = Some(failure),
                }
            }
            Reply::Found(result) => {
                library.busy.remove("search");
                match result {
                    Ok(found) if found.query == library.query => (library.found, library.searching) = (Some(found), false),
                    Ok(_) => {}
                    Err(failure) => {
                        library.searching = false;
                        failed = Some(failure);
                    }
                }
            }
            Reply::Contains(uri, result) => {
                library.busy.remove("contains");
                match result {
                    Ok(liked) => {
                        library.hearts.insert(uri, liked);
                    }
                    Err(failure) => failed = Some(failure),
                }
            }
            Reply::Saved(uri, liked, result) => match result {
                Ok(()) => {
                    library.hearts.insert(uri.clone(), liked);
                    if liked {
                        library.next_lists = 0;
                    } else {
                        library.liked.retain(|item| item.uri != uri);
                    }
                }
                Err(failure) => {
                    library.hearts.insert(uri, !liked);
                    failed = Some(failure);
                }
            },
            Reply::Player(result) => {
                library.busy.remove("player");
                match result {
                    Ok(player) => library.web_player = player,
                    Err(failure) => failed = Some(failure),
                }
            }
            Reply::Devices(result) => {
                library.busy.remove("devices");
                match result {
                    Ok(devices) => library.devices = devices,
                    Err(failure) => failed = Some(failure),
                }
            }
            Reply::ContextName(uri, result) => {
                library.busy.remove("context");
                // a playlist Spotify does not show (an editorial one) keeps the plain "Playlist"
                library.context_names.insert(uri, result.unwrap_or_default());
            }
            Reply::Profile(result) => {
                library.busy.remove("profile");
                match result {
                    Ok(profile) => (self.account.name, self.account.user_id) = (profile.name, profile.id),
                    Err(failure) => failed = Some(failure),
                }
            }
            Reply::Done(result) => failed = result.err(),
            Reply::Then(command) => return Some(command),
        }
        if let Some(failure) = failed {
            if failure.unauthorized {
                // the refresh token was refused: no more calls until it is linked again (the keychain keeps it)
                self.account.link = Link::Failed(failure.message.clone());
                if let Ok(mut held) = self.account.shared.lock() {
                    *held = None;
                }
            }
            self.library.fail(failure.message, now);
        }
        None
    }
}

impl DrawingApp {
    /// The island draws the Spotify page: call it each frame it does. While it is drawn (and a moment after), the queue,
    /// the player and the lists are asked; closed, nothing is.
    #[allow(dead_code)] // the island's Spotify page (island/) calls it
    pub(crate) fn spotify_page_open(&self) {
        self.integrations.spotify.library.page_seen.set(now_ms());
    }

    /// A list's cover, once it came down (it is asked the first time it is wanted); None meanwhile, or when the address
    /// is not one of Spotify's picture servers.
    #[allow(dead_code)] // the island's Spotify page (island/) calls it
    pub(crate) fn spotify_thumbnail(&self, url: &str) -> Option<egui::TextureId> {
        let thumbs = &self.integrations.spotify.library.thumbs;
        if let Some(texture) = thumbs.ready.get(url) {
            return Some(texture.id());
        }
        if super::cover_allowed(url) && !thumbs.asked.contains(url) {
            let mut wanted = thumbs.wanted.borrow_mut();
            if !wanted.iter().any(|known| known == url) && wanted.len() < 64 {
                wanted.push(url.to_string());
            }
        }
        None
    }

    /// The covers asked for come down a few at a time; what came becomes a texture.
    fn spotify_thumbs_tick(&mut self, ctx: &Context) {
        let thumbs = &mut self.integrations.spotify.library.thumbs;
        while let Some((url, bytes)) = thumbs.channel.as_ref().and_then(|(_, inbox)| inbox.try_recv().ok()) {
            thumbs.loading = thumbs.loading.saturating_sub(1);
            if let Some(picture) = bytes.and_then(|bytes| image::load_from_memory(&bytes).ok()) {
                let picture = picture.thumbnail(THUMB_SIZE, THUMB_SIZE).into_rgba8();
                let size = [picture.width() as usize, picture.height() as usize];
                let image = egui::ColorImage::from_rgba_unmultiplied(size, &picture);
                thumbs.ready.insert(url, ctx.load_texture("spotify-thumb", image, egui::TextureOptions::LINEAR));
            }
        }
        if thumbs.ready.len() > THUMBS_MOST {
            // ponytail: all of them go at once past the most (the page asks again what it shows); an LRU if it flickers
            thumbs.ready.clear();
            thumbs.asked.clear();
        }
        let wanted = std::mem::take(thumbs.wanted.get_mut());
        for url in wanted {
            if thumbs.loading >= THUMBS_AT_ONCE {
                thumbs.wanted.get_mut().push(url);
                continue;
            }
            if !thumbs.asked.insert(url.clone()) || cfg!(test) {
                continue;
            }
            thumbs.loading += 1;
            let sender = thumbs.channel.get_or_insert_with(channel).0.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let bytes = super::download_cover(&url);
                let _ = sender.send((url, bytes));
                ctx.request_repaint();
            });
        }
    }

    /// Each frame the integration is on: the Web API's answers taken, the covers, and while the page is open the polls
    /// that are due.
    pub(crate) fn spotify_library_tick(&mut self, ctx: &Context) {
        let now = now_ms();
        let mut then = Vec::new();
        let spotify = &mut self.integrations.spotify;
        while let Some((generation, reply)) = spotify.library.channel.as_ref().and_then(|(_, inbox)| inbox.try_recv().ok()) {
            if generation == spotify.library.generation {
                then.extend(spotify.take_reply(reply, now));
            }
        }
        if spotify.library.error.as_ref().is_some_and(|(_, at)| now - at > ERROR_FOR_MS) {
            spotify.library.error = None;
        }
        let linked = spotify.account.linked();
        // who the account is: once (again a minute after a failure), for Ajustes and Liked Songs' uri
        if linked && spotify.account.user_id.is_empty() && now >= spotify.library.next_profile && spotify.account.link == Link::Linked {
            spotify.library.next_profile = now + 60_000;
            spotify.spawn_web("profile", |shared| Reply::Profile(web::profile(shared)));
        }
        let open = spotify.library.page_open(now);
        if open {
            // the time moves on the page
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
            spotify.library.lists_note = (!linked).then(|| LISTS_NEED_ACCOUNT.to_string());
            if now >= spotify.library.next_page {
                spotify.library.next_page = now + PAGE_EVERY_MS;
                spotify.poll_page(linked);
            }
            if linked && now >= spotify.library.next_lists {
                spotify.library.next_lists = now + LISTS_EVERY_MS;
                spotify.spawn_web("playlists", |shared| Reply::Playlists(web::playlists(shared)));
                spotify.spawn_web("liked", |shared| Reply::Liked(web::liked(shared)));
            }
        }
        spotify.enrich();
        self.spotify_thumbs_tick(ctx);
        for command in then {
            if let Err(why) = self.spotify_do(command) {
                self.integrations.spotify.library.fail(why, now);
            }
        }
    }
}

impl Spotify {
    /// The page's poll: the queue (Soloist's, else the account's), and with the account the player, the devices, the
    /// heart of the track and the name of where it comes from.
    pub(super) fn poll_page(&mut self, linked: bool) {
        let soloist = self.now.as_ref().is_some_and(|now| now.player == Player::Soloist || now.through_soloist);
        if soloist {
            #[cfg(test)]
            self.library.web_asked.push("soloist queue");
            if self.library.busy.insert("queue") {
                match &self.thread {
                    Some((asks, _)) => drop(asks.send(Ask::Queue(QUEUE_MOST))),
                    None => drop(self.library.busy.remove("queue")),
                }
            }
        } else if linked {
            self.spawn_web("queue", |shared| Reply::Queue(web::queue(shared)));
        } else {
            self.library.queue.clear();
            self.library.queue_note = Some(QUEUE_NEEDS_ACCOUNT.into());
        }
        if !linked {
            return;
        }
        self.spawn_web("player", |shared| Reply::Player(web::player(shared)));
        self.spawn_web("devices", |shared| Reply::Devices(web::devices(shared)));
        let uri = self.now.as_ref().map(|now| now.playback.track_uri.clone()).unwrap_or_default();
        if playback::is_track(&uri) && !self.library.hearts.contains_key(&uri) {
            self.spawn_web("contains", move |shared| Reply::Contains(uri.clone(), web::contains(shared, &uri)));
        }
        let context = self.now.as_ref().and_then(|now| now.playback.context.clone()).or(self.library.web_player.as_ref().and_then(|web| web.context.clone()));
        if let Some(PlayContext { uri, .. }) = context.filter(|context| context.kind() == "playlist" && context.name.is_empty())
            && !self.library.context_names.contains_key(&uri)
        {
            self.spawn_web("context", move |shared| Reply::ContextName(uri.clone(), web::playlist_name(shared, &uri)));
        }
    }
}
