//! Spotify's Web API with the account linked in Ajustes (`account.rs`): what no player says. Each call is blocking and
//! runs on a thread of its own (`library.rs` spawns them); what reads the answers is plain functions, tested on recorded
//! answers. The endpoints as Spotify's reference has them in 2026: `GET /search` (limit at most 10), `GET /me/playlists`
//! (the count in `items.total`, `tracks.total` before), `GET /me/tracks` (Liked Songs), `GET /me/library/contains`,
//! `PUT` and `DELETE /me/library` with `uris` (they replaced `/me/tracks/contains` and `PUT`/`DELETE /me/tracks`),
//! `GET` and `POST /me/player/queue`, `PUT /me/player/play` with `context_uri` or `uris`, and the player's controls.

use super::account::{self, Shared, encode};
use super::library::{Device, Found, PlaylistItem};
use super::playback::{PlayContext, Repeat, is_liked_songs, is_track, uri_allowed};
use super::{Command, QueueItem, cover_allowed};
use crate::app::integrations::http::{self, Request};
use crate::prelude::*;

const API: &str = "https://api.spotify.com/v1";
/// Each call's limit: under the MCP's ten seconds of waiting, a renewal of the token included.
const TIMEOUT: u64 = 8;
/// What a search brings of each kind (Spotify's most since 2026), and of the lists.
const SEARCH_MOST: usize = 10;
const LIST_MOST: usize = 50;

/// Why a call failed, in a short line; `unauthorized`: the account must be linked again.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Failure {
    pub(crate) message: String,
    pub(crate) unauthorized: bool,
    /// The HTTP status (0: no answer).
    pub(crate) status: u16,
}

/// Who the account is.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Profile {
    pub(crate) id: String,
    pub(crate) name: String,
}

/// What the Web API says plays (`GET /me/player`), wherever it plays.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WebPlayer {
    pub(crate) track_uri: String,
    pub(crate) title: String,
    pub(crate) playing: bool,
    pub(crate) shuffle: Option<bool>,
    pub(crate) repeat: Option<Repeat>,
    pub(crate) progress_ms: Option<u64>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) context: Option<PlayContext>,
    pub(crate) device: Option<Device>,
}

/// A text of the answer as the app shows it: one line, no control characters, at most `most` characters.
fn text(value: &Value, key: &str, most: usize) -> String {
    let text: String = value.get(key).and_then(Value::as_str).unwrap_or_default().chars().filter(|letter| !letter.is_control()).take(most).collect();
    text.trim().to_string()
}

fn whole(value: Option<&Value>) -> Option<u64> {
    value?.as_u64()
}

/// The picture to show small out of Spotify's `images` (largest first): the smallest at least 64 px wide, else the
/// last; only one of Spotify's own picture servers.
pub(crate) fn small_image(images: &Value) -> String {
    let images: Vec<&Value> = images.as_array().into_iter().flatten().filter(|image| image["url"].as_str().is_some_and(cover_allowed)).collect();
    let wide_enough =
        images.iter().filter(|image| image["width"].as_u64().is_none_or(|width| width >= 64)).min_by_key(|image| image["width"].as_u64().unwrap_or(u64::MAX));
    wide_enough.or(images.last()).and_then(|image| image["url"].as_str()).unwrap_or_default().to_string()
}

/// A track (or an episode) object: its uri, name, artists (an episode's show), cover and duration.
pub(crate) fn parse_track(value: &Value) -> Option<QueueItem> {
    let artists: Vec<String> =
        value["artists"].as_array().into_iter().flatten().map(|artist| text(artist, "name", 100)).filter(|name| !name.is_empty()).collect();
    let artist = if artists.is_empty() { text(&value["show"], "name", 200) } else { artists.join(", ") };
    let images = if value["album"]["images"].is_array() { &value["album"]["images"] } else { &value["images"] };
    QueueItem::new(value["uri"].as_str().unwrap_or_default(), &text(value, "name", 200), &artist, &small_image(images), whole(value.get("duration_ms")))
}

/// A playlist object: its uri, name, owner, cover and how many tracks (`items.total`, or the older `tracks.total`).
pub(crate) fn parse_playlist(value: &Value) -> Option<PlaylistItem> {
    let uri = value["uri"].as_str().filter(|uri| uri_allowed(uri) && uri.starts_with("spotify:playlist:"))?;
    let name = text(value, "name", 200);
    let total = whole(value["items"].get("total")).or(whole(value["tracks"].get("total")));
    (!name.is_empty()).then(|| PlaylistItem {
        uri: uri.into(),
        name,
        owner: text(&value["owner"], "display_name", 100),
        cover_url: small_image(&value["images"]),
        total,
    })
}

/// `GET /search`'s answer: the tracks and the playlists (an entry may come as null).
pub(crate) fn parse_search(root: &Value, query: &str) -> Found {
    let items = |kind: &str| root[kind]["items"].as_array().cloned().unwrap_or_default();
    Found {
        query: query.to_string(),
        tracks: items("tracks").iter().filter_map(parse_track).take(SEARCH_MOST).collect(),
        playlists: items("playlists").iter().filter_map(parse_playlist).take(SEARCH_MOST).collect(),
    }
}

/// `GET /me/playlists`'s page.
pub(crate) fn parse_playlists(root: &Value) -> Vec<PlaylistItem> {
    root["items"].as_array().into_iter().flatten().filter_map(parse_playlist).collect()
}

/// `GET /me/tracks`'s page (Liked Songs): `items[].track` (`item` in the newer spelling of the lists).
pub(crate) fn parse_saved(root: &Value) -> Vec<QueueItem> {
    root["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| parse_track(entry.get("track").filter(|track| track.is_object()).unwrap_or(&entry["item"])))
        .collect()
}

/// `GET /me/player/queue`: what plays next (`queue[]`), the first first.
pub(crate) fn parse_queue(root: &Value) -> Vec<QueueItem> {
    root["queue"].as_array().into_iter().flatten().filter_map(parse_track).collect()
}

/// `GET /me/library/contains`: one boolean a uri asked.
pub(crate) fn parse_contains(root: &Value) -> Option<bool> {
    root.as_array()?.first()?.as_bool()
}

pub(crate) fn parse_device(value: &Value) -> Option<Device> {
    let id = text(value, "id", 100);
    let name = text(value, "name", 100);
    (!id.is_empty() && !name.is_empty()).then(|| Device {
        id,
        name,
        kind: text(value, "type", 40),
        active: value["is_active"].as_bool().unwrap_or(false),
        volume: whole(value.get("volume_percent")).map(|volume| volume.min(100) as f32 / 100.0),
    })
}

/// `GET /me/player/devices`.
pub(crate) fn parse_devices(root: &Value) -> Vec<Device> {
    root["devices"].as_array().into_iter().flatten().filter_map(parse_device).collect()
}

/// `GET /me/player` (None: nothing plays anywhere, a 204).
pub(crate) fn parse_player(root: &Value) -> Option<WebPlayer> {
    let item = root.get("item").filter(|item| item.is_object());
    let context = root.get("context").filter(|context| context.is_object()).and_then(|context| {
        let uri = context["uri"].as_str().unwrap_or_default().trim().to_string();
        (!uri.is_empty()).then_some(PlayContext { uri, name: String::new() })
    });
    let device = parse_device(&root["device"]);
    if item.is_none() && device.is_none() {
        return None;
    }
    Some(WebPlayer {
        track_uri: item.and_then(|item| item["uri"].as_str()).filter(|uri| uri_allowed(uri)).unwrap_or_default().to_string(),
        title: item.map(|item| text(item, "name", 200)).unwrap_or_default(),
        playing: root["is_playing"].as_bool().unwrap_or(false),
        shuffle: root["shuffle_state"].as_bool(),
        repeat: root["repeat_state"].as_str().and_then(Repeat::parse),
        progress_ms: whole(root.get("progress_ms")),
        duration_ms: item.and_then(|item| whole(item.get("duration_ms"))),
        context,
        device,
    })
}

pub(crate) fn parse_profile(root: &Value) -> Option<Profile> {
    let id = text(root, "id", 100);
    (!id.is_empty()).then(|| Profile { name: Some(text(root, "display_name", 100)).filter(|name| !name.is_empty()).unwrap_or_else(|| id.clone()), id })
}

/// The Web API's answer: its JSON (None when empty, as a 204), or why not in words. `path` tells a player's call.
pub(crate) fn read_answer(path: &str, status: u16, body: &str) -> Result<Option<Value>, Failure> {
    let root: Option<Value> = serde_json::from_str(body).ok();
    if (200..300).contains(&status) {
        return Ok(root);
    }
    let reason = root.as_ref().and_then(|root| root["error"]["reason"].as_str()).unwrap_or_default();
    let player = path.starts_with("/me/player");
    let message = match (status, reason) {
        (401, _) => account::RELINK.to_string(),
        (_, "NO_ACTIVE_DEVICE") => "Nenhum aparelho do Spotify ativo: toque algo no Spotify (ou no Soloist) primeiro.".into(),
        (_, "PREMIUM_REQUIRED") => "Isso precisa do Spotify Premium.".into(),
        (404, _) if player => "Nenhum aparelho do Spotify ativo: toque algo no Spotify (ou no Soloist) primeiro.".into(),
        (404, _) => "O Spotify não achou isso.".into(),
        (403, _) => "O Spotify recusou: a sua conta precisa estar entre os usuários do app (Development Mode) e o dono do app precisa do Premium.".into(),
        (429, _) => "Pedidos demais ao Spotify agora: tente daqui a pouco.".into(),
        (400, _) => "O Spotify não entendeu o pedido.".into(),
        (500.., _) => format!("O Spotify falhou agora ({status}): tente de novo."),
        _ => format!("O Spotify respondeu {status}."),
    };
    Err(Failure { message, unauthorized: status == 401, status })
}

/// One call with the token of the moment (renewed once when refused). A call that changes something always carries a
/// body (an empty one: Spotify wants its length).
fn call(shared: &Shared, method: &str, path: &str, body: Option<Value>) -> Result<Option<Value>, Failure> {
    let url = format!("{API}{path}");
    for attempt in 0..2 {
        let token = account::bearer(shared)?;
        let mut headers = vec![format!("Authorization: Bearer {token}"), "Accept: application/json".into(), "User-Agent: MagoAgent".into()];
        let body = match (&body, method) {
            (Some(json), _) => {
                headers.push("Content-Type: application/json".into());
                Some(json.to_string())
            }
            (None, "GET") => None,
            (None, _) => Some(String::new()),
        };
        let request = Request { method, url: &url, headers, body, timeout_seconds: TIMEOUT };
        let answer = http::send(&request).map_err(|_| Failure { message: "Sem conexão com o Spotify.".into(), unauthorized: false, status: 0 })?;
        if answer.status == 401 && attempt == 0 {
            account::expire(shared);
            continue;
        }
        return read_answer(path, answer.status, &answer.body);
    }
    Err(Failure { message: account::RELINK.into(), unauthorized: true, status: 401 })
}

fn get(shared: &Shared, path: &str) -> Result<Value, Failure> {
    Ok(call(shared, "GET", path, None)?.unwrap_or(Value::Null))
}

/// Who an access token is, before it is in a session (the login's last step).
pub(crate) fn profile_with(access: &str) -> Result<Profile, Failure> {
    let request = Request {
        method: "GET",
        url: &format!("{API}/me"),
        headers: vec![format!("Authorization: Bearer {access}"), "User-Agent: MagoAgent".into()],
        body: None,
        timeout_seconds: TIMEOUT,
    };
    let answer = http::send(&request).map_err(|_| Failure { message: "Sem conexão com o Spotify.".into(), unauthorized: false, status: 0 })?;
    let root = read_answer("/me", answer.status, &answer.body)?.unwrap_or_default();
    parse_profile(&root).ok_or(Failure { message: "A resposta do Spotify veio sem a conta.".into(), unauthorized: false, status: answer.status })
}

pub(crate) fn profile(shared: &Shared) -> Result<Profile, Failure> {
    parse_profile(&get(shared, "/me")?).ok_or(Failure { message: "A resposta do Spotify veio sem a conta.".into(), unauthorized: false, status: 200 })
}

pub(crate) fn search(shared: &Shared, query: &str) -> Result<Found, Failure> {
    let root = get(shared, &format!("/search?q={}&type=track,playlist&limit={SEARCH_MOST}", encode(query)))?;
    Ok(parse_search(&root, query))
}

pub(crate) fn playlists(shared: &Shared) -> Result<Vec<PlaylistItem>, Failure> {
    Ok(parse_playlists(&get(shared, &format!("/me/playlists?limit={LIST_MOST}"))?))
}

pub(crate) fn liked(shared: &Shared) -> Result<Vec<QueueItem>, Failure> {
    Ok(parse_saved(&get(shared, &format!("/me/tracks?limit={LIST_MOST}"))?))
}

pub(crate) fn contains(shared: &Shared, uri: &str) -> Result<bool, Failure> {
    let root = get(shared, &format!("/me/library/contains?uris={}", encode(uri)))?;
    parse_contains(&root).ok_or(Failure { message: "A resposta do Spotify não disse se a música é curtida.".into(), unauthorized: false, status: 200 })
}

/// The heart: the track into Liked Songs, or out.
pub(crate) fn save(shared: &Shared, uri: &str, liked: bool) -> Result<(), Failure> {
    call(shared, if liked { "PUT" } else { "DELETE" }, &format!("/me/library?uris={}", encode(uri)), None).map(drop)
}

pub(crate) fn queue(shared: &Shared) -> Result<Vec<QueueItem>, Failure> {
    Ok(parse_queue(&get(shared, "/me/player/queue")?))
}

pub(crate) fn player(shared: &Shared) -> Result<Option<WebPlayer>, Failure> {
    Ok(call(shared, "GET", "/me/player", None)?.as_ref().and_then(parse_player))
}

pub(crate) fn devices(shared: &Shared) -> Result<Vec<Device>, Failure> {
    Ok(parse_devices(&get(shared, "/me/player/devices")?))
}

/// The path of a page of `uri`'s songs from `offset`: Liked Songs' (`/me/tracks`) or a playlist's (`/items`, which
/// replaced `/tracks`); None for anything else.
pub(crate) fn songs_path(uri: &str, offset: usize) -> Option<String> {
    if is_liked_songs(uri) {
        return Some(format!("/me/tracks?limit={LIST_MOST}&offset={offset}"));
    }
    let id = uri.strip_prefix("spotify:playlist:").filter(|_| uri_allowed(uri))?;
    Some(format!("/playlists/{id}/items?limit={LIST_MOST}&offset={offset}"))
}

/// A page of songs: what it brings, and whether more follow (`next`).
pub(crate) fn parse_songs(root: &Value) -> (Vec<QueueItem>, bool) {
    (parse_saved(root), root["next"].is_string())
}

/// A page of 50 of `uri`'s songs (a playlist, or Liked Songs) from `offset`, and whether more follow.
pub(crate) fn songs(shared: &Shared, uri: &str, offset: usize) -> Result<(Vec<QueueItem>, bool), Failure> {
    let path = songs_path(uri, offset).ok_or(Failure { message: "Essa lista não abre aqui.".into(), unauthorized: false, status: 0 })?;
    let root = match get(shared, &path) {
        // the older spelling, where `/items` is not known yet
        Err(failure) if failure.status == 404 && path.contains("/items?") => get(shared, &path.replace("/items?", "/tracks?"))?,
        other => other?,
    };
    Ok(parse_songs(&root))
}

/// A playlist's name, for "de onde" (`fields=name`: nothing else comes).
pub(crate) fn playlist_name(shared: &Shared, uri: &str) -> Result<String, Failure> {
    let id = uri.strip_prefix("spotify:playlist:").filter(|_| uri_allowed(uri)).ok_or(Failure { message: String::new(), unauthorized: false, status: 0 })?;
    Ok(text(&get(shared, &format!("/playlists/{id}?fields=name"))?, "name", 200))
}

/// The body of `PUT /me/player/play` for `uri`: a track or an episode by `uris`, the rest as the context.
pub(crate) fn play_body(uri: &str) -> Value {
    if is_track(uri) { json!({ "uris": [uri] }) } else { json!({ "context_uri": uri }) }
}

/// Liked Songs' context uri as the Web API takes it: the user id percent-encoded (a raw one with odd letters is refused).
pub(crate) fn liked_songs_uri(user_id: &str) -> String {
    format!("spotify:user:{}:collection", encode(user_id))
}

/// `?device_id=` for a call, when there is one to name.
fn on_device(path: &str, device: Option<&str>) -> String {
    match device {
        Some(id) => format!("{path}{}device_id={}", if path.contains('?') { '&' } else { '?' }, encode(id)),
        None => path.to_string(),
    }
}

/// A player's call; when no device is active (404), again on the one to wake: the active one, else a computer, else
/// the first the account has.
fn on_a_device(shared: &Shared, method: &str, path: &str, body: Option<Value>) -> Result<(), Failure> {
    match call(shared, method, path, body.clone()) {
        Err(failure) if failure.status == 404 => {
            let devices = devices(shared)?;
            let chosen = devices.iter().find(|device| device.active).or(devices.iter().find(|device| device.kind == "Computer")).or(devices.first());
            let Some(device) = chosen else { return Err(failure) };
            call(shared, method, &on_device(path, Some(&device.id)), body).map(drop)
        }
        other => other.map(drop),
    }
}

/// Plays `uri`; Liked Songs as its context, else (that context refused) as its first 50 tracks.
pub(crate) fn play(shared: &Shared, uri: &str) -> Result<(), Failure> {
    match on_a_device(shared, "PUT", "/me/player/play", Some(play_body(uri))) {
        Err(failure) if failure.status == 400 && is_liked_songs(uri) => {
            let uris: Vec<String> = liked(shared)?.into_iter().map(|item| item.uri).filter(|uri| !uri.is_empty()).collect();
            if uris.is_empty() {
                return Err(failure);
            }
            on_a_device(shared, "PUT", "/me/player/play", Some(json!({ "uris": uris })))
        }
        other => other,
    }
}

/// The body of `PUT /me/player/play` for a song of the queue: `context` from that song, so the list goes on after it.
pub(crate) fn play_in_body(track: &str, context: &str) -> Value {
    json!({ "context_uri": context, "offset": { "uri": track } })
}

/// A song of the queue at once, not skipped to: in `context` from it, else (no context, or not in it, as a song put in
/// the queue by hand) alone.
pub(crate) fn play_in(shared: &Shared, track: &str, context: Option<&str>) -> Result<(), Failure> {
    let Some(context) = context else { return play(shared, track) };
    match on_a_device(shared, "PUT", "/me/player/play", Some(play_in_body(track, context))) {
        Err(failure) if !failure.unauthorized => play(shared, track),
        other => other,
    }
}

/// A control on the account's active device; `playing`: whether it played (play and pause are two calls).
pub(crate) fn control(shared: &Shared, command: &Command, playing: bool) -> Result<(), Failure> {
    let (method, path) = match command {
        Command::PlayPause if playing => ("PUT", "/me/player/pause".to_string()),
        Command::PlayPause => ("PUT", "/me/player/play".to_string()),
        Command::Next => ("POST", "/me/player/next".to_string()),
        Command::Previous => ("POST", "/me/player/previous".to_string()),
        Command::Shuffle(on) => ("PUT", format!("/me/player/shuffle?state={on}")),
        Command::Repeat(repeat) => ("PUT", format!("/me/player/repeat?state={}", repeat.name())),
        Command::Volume(percent) => ("PUT", format!("/me/player/volume?volume_percent={}", percent.min(&100))),
        Command::Seek(ms) => ("PUT", format!("/me/player/seek?position_ms={ms}")),
        Command::PlayUri(uri) => return play(shared, uri),
        Command::Enqueue(uri) => ("POST", format!("/me/player/queue?uri={}", encode(uri))),
    };
    on_a_device(shared, method, &path, None)
}

/// The music to another device (Spotify Connect), playing there when it played here.
pub(crate) fn transfer(shared: &Shared, device_id: &str, playing: bool) -> Result<(), Failure> {
    call(shared, "PUT", "/me/player", Some(json!({ "device_ids": [device_id], "play": playing }))).map(drop)
}
