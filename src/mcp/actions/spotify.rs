//! The MCP's `integrations` for Spotify beyond its switch and the three buttons, as the island's Spotify page has them:
//! shuffle, repeat, the volume, a seek, the queue, a search, play (a uri, Liked Songs or the first hit of a search),
//! enqueue, the heart, the playlists, Liked Songs, the devices, and the account (its Client ID, Conectar, Desconectar;
//! the token never comes through here). What waits for the Web API runs off the app's thread (`act_later`): the agent
//! waits for the answer, the app does not.

use crate::app::integrations::spotify::controls::{LIKED_SONGS, player_queue_now};
use crate::app::integrations::spotify::library::{LISTS_NEED_ACCOUNT, QUEUE_MOST, QUEUE_NEEDS_ACCOUNT, Reply};
use crate::app::integrations::spotify::playback::{is_track, uri_allowed};
use crate::app::integrations::spotify::{Command, Player, PlaylistItem, QueueItem, Repeat, account, web};
use crate::prelude::*;

/// What an action leaves to a thread: its answer to the agent.
pub(crate) type Job = Box<dyn FnOnce() -> Result<String, String> + Send>;

/// The Spotify actions of the MCP beyond on, off, status and the three buttons.
pub(crate) const ACTIONS: [&str; 20] = [
    "shuffle",
    "repeat",
    "volume",
    "seek",
    "queue",
    "search",
    "play",
    "enqueue",
    "like",
    "unlike",
    "playlists",
    "liked",
    "songs",
    "play_in",
    "devices",
    "device",
    "client_id",
    "connect",
    "disconnect",
    "account",
];

/// "3:07".
fn clock(ms: u64) -> String {
    format!("{}:{:02}", ms / 60_000, ms / 1000 % 60)
}

/// A list of tracks as the agent reads it: "1. Título — Artista (3:07) spotify:track:…".
pub(crate) fn track_lines(title: &str, items: &[QueueItem]) -> String {
    if items.is_empty() {
        return format!("{title}: nada.");
    }

    let lines = items.iter().enumerate().map(|(index, item)| {
        let by = if item.artist.is_empty() { String::new() } else { format!(" — {}", item.artist) };
        let time = item.duration_ms.map(|ms| format!(" ({})", clock(ms))).unwrap_or_default();

        format!("{}. {}{by}{time} {}", index + 1, item.title, item.uri).trim_end().to_string()
    });

    format!("{title}:\n{}", lines.collect::<Vec<_>>().join("\n"))
}

pub(crate) fn playlist_lines(title: &str, items: &[PlaylistItem]) -> String {
    if items.is_empty() {
        return format!("{title}: nenhuma.");
    }

    let lines = items.iter().enumerate().map(|(index, playlist)| {
        let owner = if playlist.owner.is_empty() { String::new() } else { format!(" (de {})", playlist.owner) };
        let total = playlist.total.map(|total| format!(" · {total} músicas")).unwrap_or_default();

        format!("{}. {}{owner}{total} {}", index + 1, playlist.name, playlist.uri)
    });

    format!("{title}:\n{}", lines.collect::<Vec<_>>().join("\n"))
}

/// "83", "1:23" or "83.5" (seconds) as ms.
fn seconds(text: &str) -> Option<u64> {
    let text = text.trim();

    let value = match text.split_once(':') {
        Some((minutes, seconds)) => minutes.trim().parse::<u64>().ok()? as f64 * 60.0 + seconds.trim().parse::<f64>().ok()?,
        None => text.parse::<f64>().ok()?,
    };

    (value.is_finite() && value >= 0.0).then_some((value * 1000.0) as u64)
}

fn flag(text: &str) -> Option<bool> {
    match text.trim().to_lowercase().as_str() {
        "on" | "true" | "ligado" | "ligar" | "sim" | "1" => Some(true),
        "off" | "false" | "desligado" | "desligar" | "não" | "nao" | "0" => Some(false),
        _ => None,
    }
}

impl DrawingApp {
    /// An action of `integrations` whose answer waits for the network, or the setting `register_mcp`: its work, to run on
    /// a thread (None: it answers at once, by `act`). An Err is the answer itself (no account, no query).
    pub(crate) fn act_later(&mut self, action: &str, args: &Value) -> Option<Job> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_lowercase();

        // reading every process's size takes a while (a `ps` per process on macOS)
        if action == "memory" {
            return Some(self.memory_job());
        }

        // the agent's CLI takes seconds (system/mcp_register.rs)
        if action == "set_setting" && text("name") == "register_mcp" {
            let agent = text("value");

            return Some(Box::new(move || crate::app::system::mcp_register::mcp_register(&agent)));
        }

        let soloist_elsewhere = text("integration") == "soloist" && cfg!(any(windows, target_os = "macos"));

        if action != "integrations" || !matches!(text("integration").as_str(), "spotify" | "soloist") || soloist_elsewhere {
            return None;
        }

        match self.spotify_job(&text("action"), args)? {
            Ok(job) => Some(job),
            Err(why) => Some(Box::new(move || Err(why))),
        }
    }

    /// The work of a Spotify action that asks the Web API (or Soloist's queue); None for the others.
    fn spotify_job(&mut self, action: &str, args: &Value) -> Option<Result<Job, String>> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let (query, uri, value) = (text("query"), text("uri"), text("value"));

        let wants = match action {
            "queue" | "search" | "playlists" | "liked" | "songs" | "devices" | "device" => true,
            // with a uri they answer at once
            "play" | "enqueue" => uri.is_empty() && !query.is_empty(),
            _ => false,
        };

        if !wants {
            return None;
        }

        if self.spotify_sources() == crate::app::integrations::spotify::Sources::default() {
            return Some(Err("o Spotify está desligado em Ajustes › Integrações: ligue antes (action on)".into()));
        }

        let spotify = &mut self.integrations.spotify;
        let linked = spotify.account.linked();
        let shared = spotify.account.shared.clone();
        let soloist = spotify.now.as_ref().is_some_and(|now| now.player == Player::Soloist || now.through_soloist);

        if action == "queue" && soloist {
            return Some(Ok(Box::new(|| {
                // never a real Soloist in a test
                if cfg!(test) {
                    return Ok("teste: a fila do Soloist".into());
                }

                player_queue_now(QUEUE_MOST).map(|queue| track_lines("A seguir", &queue)).ok_or_else(|| "O Soloist não deu a fila agora.".to_string())
            })));
        }

        if !linked {
            return Some(Err(if action == "queue" { QUEUE_NEEDS_ACCOUNT } else { LISTS_NEED_ACCOUNT }.into()));
        }

        if matches!(action, "search" | "play" | "enqueue") && query.is_empty() {
            return Some(Err("diga o que buscar em `query`".into()));
        }

        // a playlist's songs: `uri` the playlist (or `value` liked), `value` from where (a number)
        let list = if uri.is_empty() && value.eq_ignore_ascii_case("liked") { LIKED_SONGS.to_string() } else { uri.clone() };
        let offset = value.parse::<usize>().unwrap_or(0);

        if action == "songs" && web::songs_path(&list, 0).is_none() {
            return Some(Err("diga em `uri` a playlist (spotify:playlist:…) ou `value` liked para as Músicas curtidas".into()));
        }

        if action == "device" && value.is_empty() {
            return Some(Err("diga o aparelho em `value` (o nome ou o id, como devices lista)".into()));
        }

        let (generation, sender) = spotify.library.sender();
        let playlist = text("kind").eq_ignore_ascii_case("playlist");
        let playing = spotify.now.as_ref().is_some_and(|now| now.playing);
        let failure = |failure: web::Failure| failure.message;
        let action = action.to_string();

        // never the network in a test: what was asked is said
        if cfg!(test) {
            return Some(Ok(Box::new(move || Ok(format!("teste: {action} {query}{value}").trim().to_string()))));
        }

        Some(Ok(Box::new(move || match action.as_str() {
            "queue" => web::queue(&shared).map(|queue| track_lines("A seguir", &queue)).map_err(failure),
            "playlists" => web::playlists(&shared).map(|playlists| playlist_lines("As suas playlists", &playlists)).map_err(failure),
            "liked" => web::liked(&shared).map(|liked| track_lines("Músicas curtidas (as últimas 50)", &liked)).map_err(failure),
            "songs" => web::songs(&shared, &list, offset)
                .map(|(songs, more)| {
                    let title = format!("Músicas {} a {}", offset + 1, offset + songs.len());

                    let next = if more {
                        format!(
                            "
Tem mais: peça songs com `value` {}.",
                            offset + songs.len()
                        )
                    } else {
                        String::new()
                    };

                    format!("{}{next}", track_lines(&title, &songs))
                })
                .map_err(failure),
            "search" => web::search(&shared, &query)
                .map(|found| format!("{}\n{}", track_lines("Músicas", &found.tracks), playlist_lines("Playlists", &found.playlists)))
                .map_err(failure),
            "devices" => web::devices(&shared)
                .map(|devices| {
                    let lines: Vec<String> = devices
                        .iter()
                        .map(|device| format!("- {} ({}){} id {}", device.name, device.kind, if device.active { " · ativo" } else { "" }, device.id))
                        .collect();

                    if lines.is_empty() { "Nenhum aparelho do Spotify aberto.".into() } else { format!("Aparelhos:\n{}", lines.join("\n")) }
                })
                .map_err(failure),
            "device" => {
                let devices = web::devices(&shared).map_err(failure)?;
                let wanted = value.to_lowercase();

                let device = devices
                    .iter()
                    .find(|device| device.id == value || device.name.to_lowercase() == wanted)
                    .or_else(|| devices.iter().find(|device| device.name.to_lowercase().contains(&wanted)))
                    .ok_or_else(|| format!("Nenhum aparelho do Spotify chamado {value}."))?;

                web::transfer(&shared, &device.id, playing).map_err(failure)?;
                Ok(format!("A música foi para {}.", device.name))
            }
            // "play" and "enqueue" with a query: the first hit, done by the app as a press on the page would
            _ => {
                let found = web::search(&shared, &query).map_err(failure)?;

                let (uri, words) = match (playlist && action == "play", found.tracks.iter().find(|track| is_track(&track.uri)), found.playlists.first()) {
                    (true, _, Some(list)) | (false, None, Some(list)) if action == "play" => (list.uri.clone(), format!("a playlist {}", list.name)),
                    (_, Some(track), _) => {
                        (track.uri.clone(), if track.artist.is_empty() { track.title.clone() } else { format!("{} — {}", track.title, track.artist) })
                    }
                    _ => return Err(format!("Nada encontrado para {query}.")),
                };

                let command = if action == "play" { Command::PlayUri(uri.clone()) } else { Command::Enqueue(uri.clone()) };

                sender.send((generation, Reply::Then(command))).map_err(|_| "o app não respondeu".to_string())?;
                Ok(format!("{} {words} ({uri}).", if action == "play" { "Tocando" } else { "Na fila:" }))
            }
        })))
    }

    /// `integrations spotify` (or `soloist`) with one of `ACTIONS`.
    pub(super) fn act_spotify(&mut self, action: &str, args: &Value) -> Result<String, String> {
        if let Some(job) = self.spotify_job(action, args) {
            let job = job?;

            return job();
        }

        let text = |key: &str| args[key].as_str().map(str::to_string).or_else(|| args[key].as_f64().map(|number| number.to_string())).unwrap_or_default();
        let value = text("value");
        let value = value.trim();
        let uri = text("uri");

        match action {
            "shuffle" => {
                let on = flag(value).or(args["value"].as_bool()).ok_or("diga `value` on ou off")?;

                self.spotify_set_shuffle(on)?;
                Ok(format!("Embaralhar {}.", if on { "ligado" } else { "desligado" }))
            }
            "repeat" => {
                let repeat = Repeat::parse(value).ok_or("diga `value` off, context (a lista) ou track (a música)")?;

                self.spotify_set_repeat(repeat)?;
                Ok(format!("Agora {}.", repeat.words()))
            }
            "volume" => {
                let percent =
                    value.trim_end_matches('%').parse::<f32>().ok().filter(|percent| (0.0..=100.0).contains(percent)).ok_or("diga `value` de 0 a 100")?;

                self.spotify_set_volume(percent / 100.0)?;
                Ok(format!("Volume em {}%.", percent.round()))
            }
            "seek" => {
                let ms = seconds(value).ok_or("diga `value` em segundos, ou como 1:23")?;

                self.spotify_seek(ms)?;
                Ok(format!("Indo para {}.", clock(ms)))
            }
            "play" => {
                let uri = if uri.is_empty() && matches!(value.to_lowercase().as_str(), "liked" | "curtidas" | "músicas curtidas") {
                    LIKED_SONGS.to_string()
                } else {
                    uri
                };

                if uri.is_empty() {
                    return Err(
                        "diga `uri` (spotify:track:…, spotify:playlist:…, liked para as Músicas curtidas) ou `query` para tocar o primeiro achado".into()
                    );
                }

                self.spotify_play_uri(&uri)?;
                Ok("Pedido ao Spotify.".into())
            }
            "play_in" => {
                let list = if value.eq_ignore_ascii_case("liked") { LIKED_SONGS } else { value };

                if !is_track(&uri) || web::songs_path(list, 0).is_none() {
                    return Err("diga `uri` a música (spotify:track:…) e `value` a playlist dela (spotify:playlist:… ou liked)".into());
                }

                self.spotify_play_in_list(&format!("{list} {uri}"))?;
                Ok("Tocando a música na playlist; a lista segue depois dela.".into())
            }
            "enqueue" => {
                if !is_track(&uri) {
                    return Err("diga `uri` de uma música (spotify:track:…) ou `query` para pôr o primeiro achado na fila".into());
                }

                self.spotify_add_to_queue(&uri)?;
                Ok("Na fila.".into())
            }
            "like" | "unlike" => {
                if !uri.is_empty() && !uri_allowed(&uri) {
                    return Err("`uri` precisa ser de uma música (spotify:track:…)".into());
                }

                self.spotify_set_liked(&uri, action == "like")?;
                Ok(if action == "like" { "Nas Músicas curtidas." } else { "Fora das Músicas curtidas." }.into())
            }
            "client_id" => {
                if !account::client_id_looks_right(value) {
                    return Err("o Client ID tem 32 letras e números (o do app no painel do Spotify)".into());
                }

                self.prefs.integrations.spotify_client_id = value.to_string();
                Ok(format!("Client ID guardado. Cadastre {} como Redirect URI no app do Spotify e peça para conectar.", account::REDIRECT_URI))
            }
            "connect" => self.spotify_connect(),
            "disconnect" => self.spotify_disconnect(),
            "account" => Ok(match &self.integrations.spotify.account.link {
                account::Link::Linked => format!(
                    "Conta do Spotify conectada{}.",
                    Some(&self.integrations.spotify.account.name).filter(|name| !name.is_empty()).map(|name| format!(" como {name}")).unwrap_or_default()
                ),
                account::Link::Waiting => "Esperando o login do Spotify no navegador.".into(),
                account::Link::Failed(why) => format!("Conta do Spotify não conectada: {why}"),
                account::Link::Off => format!(
                    "Sem conta do Spotify. Para conectar: o Client ID do app da pessoa no painel do Spotify (client_id), com {} cadastrado como Redirect URI nesse app; depois connect.",
                    account::REDIRECT_URI
                ),
            }),
            _ => Err(format!("ação desconhecida do Spotify: {action}")),
        }
    }
}
