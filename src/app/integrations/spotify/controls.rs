//! The controls of what plays, as the island's page, the pill, Ajustes and the MCP ask them: each goes where
//! `playback::route` says (the player shown, by its own thread; the Web API, by a thread of its own; or nowhere, with
//! why), and the page shows the change at once (the player's next answer confirms it or puts it back).

use super::library::{LISTS_NEED_ACCOUNT, Reply};
use super::playback::{self, NEEDS_ACCOUNT, Route, System, is_liked_songs, is_track, uri_allowed};
use super::web;
use super::{Ask, Command, QueueItem, Repeat, Sources, Spotify};
use crate::prelude::*;

/// Liked Songs, as the page and the MCP name it: the account's own uri (`spotify:user:<id>:collection`) is put in its
/// place.
pub(crate) const LIKED_SONGS: &str = "spotify:collection";

/// What the Soloist that answers plays next, asked from any thread (only Linux has a player with a queue).
pub(crate) fn player_queue_now(most: usize) -> Option<Vec<QueueItem>> {
    #[cfg(not(any(windows, target_os = "macos")))]
    return super::soloist::queue_now(most);

    #[cfg(any(windows, target_os = "macos"))]
    {
        let _ = most;

        None
    }
}

/// What the page says when the player did not take a control.
pub(crate) fn refused(command: &Command) -> String {
    match command {
        Command::PlayPause | Command::Next | Command::Previous => "O player não respondeu.",
        Command::Shuffle(_) => "O player não aceitou mudar o embaralhar.",
        Command::Repeat(_) => "O player não aceitou mudar o repetir.",
        // Windows: the volume is the Windows mixer's own, and Spotify only has a session there while it plays a sound
        Command::Volume(_) if cfg!(windows) => "O Spotify não está tocando som agora: o volume dele aparece quando ele toca.",
        Command::Volume(_) => "O player não aceitou mudar o volume.",
        Command::Seek(_) => "O player não aceitou mudar o ponto da música.",
        Command::PlayUri(_) => "O player não conseguiu tocar isso.",
        Command::Enqueue(_) => "O player não aceitou pôr na fila.",
    }
    .into()
}

impl Spotify {
    /// What plays as the Web API takes it for a song of the queue (Liked Songs as the account's own uri); None when
    /// nothing says.
    pub(crate) fn play_context(&self) -> Option<String> {
        self.context_uri(self.now.as_ref()?.playback.context.as_ref()?.uri.trim())
    }

    /// `uri` as the Web API takes a context: Liked Songs by the account's own uri; None when it is not one to play.
    pub(crate) fn context_uri(&self, uri: &str) -> Option<String> {
        let context = if is_liked_songs(uri) { web::liked_songs_uri(&self.account.user_id) } else { uri.to_string() };

        uri_allowed(&context).then_some(context)
    }

    /// The page shows a control's change before the player answers.
    fn show_change(&mut self, command: &Command) {
        let Some(now) = self.now.as_mut() else { return };
        let playback = &mut now.playback;

        match command {
            Command::Shuffle(on) => playback.shuffle = Some(*on),
            Command::Repeat(repeat) => playback.repeat = Some(*repeat),
            Command::Volume(percent) => playback.volume = Some(f32::from((*percent).min(100)) / 100.0),
            Command::Seek(ms) => {
                playback.position_ms = Some(playback.duration_ms.map_or(*ms, |duration| (*ms).min(duration)));
                playback.read_at_ms = now_ms();
            }
            _ => {}
        }
    }
}

impl DrawingApp {
    /// A control for what plays, sent where `route` says; Err in words when it goes nowhere (the integration off,
    /// nothing open and no account, a queue without the account…).
    pub(crate) fn spotify_do(&mut self, command: Command) -> Result<(), String> {
        if self.spotify_sources() == Sources::default() {
            return Err("O Spotify está desligado em Ajustes › Integrações.".into());
        }

        let spotify = &mut self.integrations.spotify;
        let linked = spotify.account.linked();

        // Liked Songs: the account's own uri
        let command = match command {
            Command::PlayUri(uri) if uri == LIKED_SONGS || is_liked_songs(&uri) => {
                if spotify.account.user_id.is_empty() {
                    return Err(NEEDS_ACCOUNT.into());
                }

                Command::PlayUri(web::liked_songs_uri(&spotify.account.user_id))
            }
            command => command,
        };

        if super::fake::active() {
            spotify.show_change(&command);

            if let (Command::PlayPause, Some(now)) = (&command, spotify.now.as_mut()) {
                now.playing = !now.playing;
            }

            return Ok(());
        }

        let shown = spotify.now.as_ref().map(|now| (now.player, now.through_soloist, now.playing));
        let route = playback::route(System::HERE, shown.map(|shown| shown.0), shown.is_some_and(|shown| shown.1), linked, &command);

        match route {
            Route::No(why) => return Err(why.into()),
            Route::Player => {
                #[cfg(test)]
                spotify.asked.push(command.clone());

                spotify.show_change(&command);

                if let Some((asks, _)) = &spotify.thread {
                    let _ = asks.send(Ask::Control(command));
                }
            }
            Route::Web => {
                spotify.show_change(&command);
                let playing = shown.is_some_and(|shown| shown.2);

                spotify.spawn_web("control", move |shared| Reply::Done(web::control(shared, &command, playing)));
                spotify.soon();
            }
        }

        Ok(())
    }

    pub(crate) fn spotify_set_shuffle(&mut self, on: bool) -> Result<(), String> {
        self.spotify_do(Command::Shuffle(on))
    }

    pub(crate) fn spotify_set_repeat(&mut self, repeat: Repeat) -> Result<(), String> {
        self.spotify_do(Command::Repeat(repeat))
    }

    /// `volume` from 0 to 1.
    pub(crate) fn spotify_set_volume(&mut self, volume: f32) -> Result<(), String> {
        let volume = if volume.is_finite() { volume.clamp(0.0, 1.0) } else { 0.0 };

        self.spotify_do(Command::Volume((volume * 100.0).round() as u8))
    }

    /// Where in the song, ms.
    pub(crate) fn spotify_seek(&mut self, ms: u64) -> Result<(), String> {
        self.spotify_do(Command::Seek(ms))
    }

    /// A track, an album, a playlist (`spotify:…`) or Liked Songs (`LIKED_SONGS`), now.
    pub(crate) fn spotify_play_uri(&mut self, uri: &str) -> Result<(), String> {
        let uri = uri.trim();

        if uri != LIKED_SONGS && !uri_allowed(uri) {
            return Err("Isso não é um endereço do Spotify que dê para tocar (spotify:track:…, spotify:playlist:…).".into());
        }

        self.spotify_do(Command::PlayUri(uri.to_string()))
    }

    /// A song of the queue (A seguir), now: with the account by the Web API in what plays from it, not skipping to it;
    /// without, as any `play`.
    pub(crate) fn spotify_jump(&mut self, uri: &str) -> Result<(), String> {
        let context = self.integrations.spotify.play_context();

        self.spotify_play_from(uri, context)
    }

    /// A song of an opened playlist (`words`: the playlist's uri and the song's), now, with the list going on after it.
    pub(crate) fn spotify_play_in_list(&mut self, words: &str) -> Result<(), String> {
        let (list, uri) = words.trim().split_once(' ').ok_or_else(String::new)?;
        let context = self.integrations.spotify.context_uri(list);

        self.spotify_play_from(uri, context)
    }

    /// A song now in `context`: with the account one Web API call; without, as any `play`.
    fn spotify_play_from(&mut self, uri: &str, context: Option<String>) -> Result<(), String> {
        let uri = uri.trim().to_string();

        if !self.integrations.spotify.account.linked() || !is_track(&uri) || super::fake::active() || self.spotify_sources() == Sources::default() {
            return self.spotify_play_uri(&uri);
        }

        self.integrations.spotify.spawn_web("control", move |shared| Reply::Done(web::play_in(shared, &uri, context.as_deref())));
        self.integrations.spotify.soon();
        Ok(())
    }

    /// A track to the queue (Soloist's own, else the account's).
    pub(crate) fn spotify_add_to_queue(&mut self, uri: &str) -> Result<(), String> {
        let uri = uri.trim();

        if !is_track(uri) {
            return Err("Só uma música vai para a fila (spotify:track:…).".into());
        }

        self.spotify_do(Command::Enqueue(uri.to_string()))
    }

    /// The heart of a track (`uri`; empty: the one playing): into Liked Songs, or out. Only with the account.
    pub(crate) fn spotify_set_liked(&mut self, uri: &str, liked: bool) -> Result<(), String> {
        let spotify = &mut self.integrations.spotify;

        if !spotify.account.linked() {
            return Err(NEEDS_ACCOUNT.into());
        }

        let uri =
            if uri.trim().is_empty() { spotify.now.as_ref().map(|now| now.playback.track_uri.clone()).unwrap_or_default() } else { uri.trim().to_string() };

        if !is_track(&uri) {
            return Err("Não sei qual música é esta: o player não disse o endereço dela.".into());
        }

        spotify.library.hearts.insert(uri.clone(), liked);
        spotify.enrich();
        spotify.spawn_web("save", move |shared| Reply::Saved(uri.clone(), liked, web::save(shared, &uri, liked)));
        Ok(())
    }

    /// A search of tracks and playlists (the answer lands in `library.found`); an empty `query` clears it.
    pub(crate) fn spotify_search(&mut self, query: &str) -> Result<(), String> {
        let query: String = query.chars().filter(|letter| !letter.is_control()).take(200).collect::<String>().trim().to_string();
        let spotify = &mut self.integrations.spotify;

        if query.is_empty() {
            (spotify.library.found, spotify.library.searching) = (None, false);

            return Ok(());
        }

        if !spotify.account.linked() {
            return Err(LISTS_NEED_ACCOUNT.into());
        }

        spotify.library.searching = true;
        spotify.library.set_query(&query);
        spotify.spawn_web("search", move |shared| Reply::Found(web::search(shared, &query)));
        Ok(())
    }

    /// The music to another of the account's devices (the "Soloist ▾" menu).
    pub(crate) fn spotify_transfer(&mut self, device_id: &str) -> Result<(), String> {
        let spotify = &mut self.integrations.spotify;

        if !spotify.account.linked() {
            return Err(NEEDS_ACCOUNT.into());
        }

        let device_id = device_id.trim().to_string();

        if !spotify.library.devices.iter().any(|device| device.id == device_id) {
            return Err("Esse aparelho não está entre os da conta do Spotify.".into());
        }

        let playing = spotify.now.as_ref().is_some_and(|now| now.playing);

        spotify.spawn_web("control", move |shared| Reply::Done(web::transfer(shared, &device_id, playing)));
        Ok(())
    }

    /// What a press on the island's Spotify page does (written down as "spotify <words>"): a control ("shuffle on",
    /// "repeat track", "volume 40", "seek 61000", "play <uri>", "enqueue <uri>", "playpause"…), "like" or "unlike" (the
    /// track playing, or a uri after it), "search <words>", "device <id>", "jump <uri>" (a song of the queue). What fails shows on the page.
    pub(crate) fn spotify_press(&mut self, words: &str) {
        let words = words.trim();
        let (name, rest) = words.split_once(' ').map_or((words, ""), |(name, rest)| (name, rest.trim()));

        let result = match name {
            "like" | "unlike" => self.spotify_set_liked(rest, name == "like"),
            "search" => self.spotify_search(rest),
            "device" => self.spotify_transfer(rest),
            "jump" => self.spotify_jump(rest),
            "playin" => self.spotify_play_in_list(rest),
            "open" => self.integrations.spotify.open_list(rest).ok_or_else(|| "Essa playlist não está mais na lista.".to_string()),
            "more" => {
                self.integrations.spotify.more_songs();
                Ok(())
            }
            "close" => {
                self.integrations.spotify.library.opened = None;
                Ok(())
            }
            "play" if rest == LIKED_SONGS => self.spotify_play_uri(rest),
            _ => match Command::parse(words) {
                Some(command) => self.spotify_do(command),
                None => Err(String::new()),
            },
        };

        if let Err(why) = result {
            self.integrations.spotify.library.fail(why, now_ms());
        }
    }
}
