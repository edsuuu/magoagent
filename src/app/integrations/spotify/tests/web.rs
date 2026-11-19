//! The Web API's answers (fixtures in the shapes of Spotify's reference), what a refusal says, and what the island's
//! page gets from the account: the polls while it is open, the replies, the controls and the presses.

use super::*;
use crate::app::integrations::spotify::account::{Account, Link, RELINK};
use crate::app::integrations::spotify::controls::LIKED_SONGS;
use crate::app::integrations::spotify::library::{LISTS_NEED_ACCOUNT, PlaylistItem, QUEUE_NEEDS_ACCOUNT, Reply};
use crate::app::integrations::spotify::playback::{NEEDS_ACCOUNT, PlayContext, Playback, Repeat, uri_allowed};
use crate::app::integrations::spotify::queue::QueueItem;
use crate::app::integrations::spotify::web::{
    Failure, Profile, liked_songs_uri, parse_contains, parse_devices, parse_player, parse_playlists, parse_profile, parse_queue, parse_saved, parse_search,
    parse_songs, play_body, play_in_body, read_answer, songs_path,
};

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

const TRACK: &str = "spotify:track:4uLU6hMCjMI75M1A2tKUQC";

#[test]
fn a_search_brings_tracks_and_playlists() {
    let found = parse_search(&fixture(include_str!("../../fixtures/spotify_web_search.json")), "teste");

    assert_eq!(found.query, "teste");

    let first = QueueItem {
        uri: TRACK.into(),
        title: "Canção de Teste".into(),
        artist: "Banda Exemplo, Convidada".into(),
        cover_url: "https://i.scdn.co/image/miuda".into(),
        duration_ms: Some(213_573),
    };

    assert_eq!(found.tracks[0], first, "the smallest picture of at least 64 px");
    assert_eq!((found.tracks.len(), found.tracks[1].cover_url.as_str()), (2, ""), "a picture from elsewhere is left out");

    let playlist = PlaylistItem {
        uri: "spotify:playlist:37i9dQZF1DXcBWIGoYBM5M".into(),
        name: "Lista de Domingo".into(),
        owner: "Ana Exemplo".into(),
        cover_url: "https://mosaic.scdn.co/640/abc".into(),
        total: Some(42),
    };

    assert_eq!(found.playlists, [playlist], "a null entry and what is not a playlist are left out");
}

#[test]
fn the_playlists_liked_songs_and_the_queue_are_read() {
    let playlists = parse_playlists(&fixture(include_str!("../../fixtures/spotify_web_playlists.json")));
    let summary: Vec<(&str, &str, Option<u64>)> = playlists.iter().map(|list| (list.name.as_str(), list.owner.as_str(), list.total)).collect();

    assert_eq!(summary, [("Para Programar", "Ana Exemplo", Some(120)), ("Sem Capa", "", Some(7))], "tracks.total and the newer items.total");
    assert_eq!(playlists[0].cover_url, "https://image-cdn-ak.spotifycdn.com/image/abc");
    let liked = parse_saved(&fixture(include_str!("../../fixtures/spotify_web_saved.json")));

    assert_eq!(liked.iter().map(|item| item.title.as_str()).collect::<Vec<_>>(), ["Canção de Teste", "Na Grafia Nova"], "track or item, no null");
    assert_eq!(liked[0].cover_url, "https://i.scdn.co/image/media");
    let queue = parse_queue(&fixture(include_str!("../../fixtures/spotify_web_queue.json")));

    assert_eq!(queue.len(), 2, "what plays now is not in the queue");
    assert_eq!((queue[0].artist.as_str(), queue[0].cover_url.as_str()), ("Banda Exemplo, Convidada", "https://i.scdn.co/image/fila-miuda"));
    assert_eq!((queue[1].uri.as_str(), queue[1].artist.as_str()), ("spotify:episode:3333333333333333333333", "Podcast de Teste"), "an episode: its show");
}

#[test]
fn the_player_the_devices_the_heart_and_who_are_read() {
    let player = parse_player(&fixture(include_str!("../../fixtures/spotify_web_player.json"))).unwrap();

    assert_eq!((player.track_uri.as_str(), player.title.as_str(), player.playing), (TRACK, "Canção de Teste", true));
    assert_eq!((player.shuffle, player.repeat, player.progress_ms, player.duration_ms), (Some(false), Some(Repeat::Track), Some(30_000), Some(213_573)));
    let context = player.context.clone().unwrap();

    assert_eq!((context.uri.as_str(), context.label()), ("spotify:user:ana:collection", "Músicas curtidas".into()), "Liked Songs as a context");
    let device = player.device.unwrap();

    assert_eq!(
        (device.id.as_str(), device.name.as_str(), device.kind.as_str(), device.active, device.volume),
        ("dev-soloist", "Soloist", "Computer", true, Some(0.55))
    );

    assert_eq!(parse_player(&json!({})), None, "nothing plays anywhere");
    let devices = parse_devices(&fixture(include_str!("../../fixtures/spotify_web_devices.json")));

    assert_eq!(
        devices.iter().map(|device| (device.name.as_str(), device.active, device.volume)).collect::<Vec<_>>(),
        [("Soloist", true, Some(0.55)), ("Celular da Ana", false, None)]
    );

    assert_eq!((parse_contains(&json!([true])), parse_contains(&json!([false])), parse_contains(&json!({}))), (Some(true), Some(false), None));
    assert_eq!(parse_profile(&json!({ "id": "ana", "display_name": "Ana Exemplo" })), Some(Profile { id: "ana".into(), name: "Ana Exemplo".into() }));
    assert_eq!(parse_profile(&json!({ "id": "ana", "display_name": null })).map(|profile| profile.name), Some("ana".into()));
    assert_eq!(parse_profile(&json!({})), None);
}

#[test]
fn what_to_play_is_a_track_or_a_context_and_liked_songs_is_the_accounts() {
    assert_eq!(play_body(TRACK), json!({ "uris": [TRACK] }));
    assert_eq!(play_body("spotify:playlist:abc"), json!({ "context_uri": "spotify:playlist:abc" }));
    assert_eq!(liked_songs_uri("ana"), "spotify:user:ana:collection");
    assert_eq!(liked_songs_uri("ana b.c"), "spotify:user:ana%20b.c:collection");
    assert!(uri_allowed(&liked_songs_uri("ana b.c")));
}

#[test]
fn a_refusal_says_why_in_a_short_line() {
    let refused = |path: &str, status: u16, body: &str| read_answer(path, status, body).unwrap_err();

    assert_eq!(read_answer("/me", 200, r#"{"id":"ana"}"#).unwrap(), Some(json!({ "id": "ana" })));
    assert_eq!(read_answer("/me/player", 204, "").unwrap(), None, "nothing plays");
    let unauthorized = refused("/me/playlists", 401, r#"{"error":{"status":401,"message":"The access token expired"}}"#);

    assert_eq!(unauthorized, Failure { message: RELINK.into(), unauthorized: true, status: 401 });
    assert!(refused("/me/player/next", 404, "").message.contains("Nenhum aparelho"));

    assert!(
        refused("/me/player/play", 404, r#"{"error":{"status":404,"message":"Player command failed: No active device found","reason":"NO_ACTIVE_DEVICE"}}"#)
            .message
            .contains("Nenhum aparelho")
    );

    assert!(refused("/playlists/abc", 404, "").message.contains("não achou"));

    assert!(
        refused("/me/player/shuffle", 403, r#"{"error":{"status":403,"message":"Player command failed: Premium required","reason":"PREMIUM_REQUIRED"}}"#)
            .message
            .contains("Premium")
    );

    assert!(refused("/me/tracks", 403, "").message.contains("Development Mode"));
    assert!(refused("/search", 429, "").message.contains("daqui a pouco"));
    assert!(refused("/search", 502, "<html>").message.contains("502"));
    assert!(!refused("/search", 502, "").unauthorized);
}

/// An app with Spotify on, `now` playing on `player`, and an account linked as "ana" when `linked`.
fn app_with(player: Player, linked: bool) -> DrawingApp {
    let mut app = DrawingApp::default();

    app.prefs.integrations.spotify = true;
    let playback = Playback { track_uri: TRACK.into(), duration_ms: Some(213_573), ..Default::default() };

    app.integrations.spotify.now = Some(Now { track: track("Canção de Teste"), playing: true, player, playback, ..Default::default() });

    if linked {
        app.integrations.spotify.account = Account::with_session("ana");
    }

    app
}

#[test]
fn the_page_is_polled_only_while_it_is_open() {
    let ctx = Context::default();
    let mut closed = app_with(Player::App, true);

    closed.spotify_library_tick(&ctx);
    assert!(closed.integrations.spotify.library.web_asked.is_empty(), "closed: nothing is asked");
    // open, with the account: the queue, the player, the devices, the heart and the lists
    let mut app = app_with(Player::App, true);

    app.spotify_page_open();
    app.spotify_library_tick(&ctx);
    let asked = app.integrations.spotify.library.web_asked.clone();

    for kind in ["queue", "player", "devices", "contains", "playlists", "liked"] {
        assert!(asked.contains(&kind), "{kind} in {asked:?}");
    }

    assert!(!asked.contains(&"profile"), "who it is is known");
    app.spotify_library_tick(&ctx);
    assert_eq!(app.integrations.spotify.library.web_asked.len(), asked.len(), "not again before five seconds");
    // Soloist gives its own queue; without the account the lists say why they are empty
    let mut soloist = app_with(Player::Soloist, false);

    soloist.spotify_page_open();
    soloist.spotify_library_tick(&ctx);
    assert_eq!(soloist.integrations.spotify.library.web_asked, ["soloist queue"]);
    assert_eq!(soloist.integrations.spotify.library.lists_note.as_deref(), Some(LISTS_NEED_ACCOUNT));
    // the app's own player without the account has no queue to show
    let mut alone = app_with(Player::App, false);

    alone.spotify_page_open();
    alone.spotify_library_tick(&ctx);
    assert_eq!(alone.integrations.spotify.library.queue_note.as_deref(), Some(QUEUE_NEEDS_ACCOUNT));
    assert!(alone.integrations.spotify.library.web_asked.is_empty());
}

#[test]
fn what_the_account_says_fills_what_the_player_did_not() {
    let mut app = app_with(Player::App, true);
    let spotify = &mut app.integrations.spotify;
    let player = parse_player(&fixture(include_str!("../../fixtures/spotify_web_player.json")));

    assert_eq!(spotify.take_reply(Reply::Player(Ok(player)), 0), None);
    spotify.take_reply(Reply::Liked(Ok(parse_saved(&fixture(include_str!("../../fixtures/spotify_web_saved.json"))))), 0);
    spotify.enrich();
    let playback = &spotify.now.as_ref().unwrap().playback;

    assert_eq!(playback.context.as_ref().map(PlayContext::label), Some("Músicas curtidas".into()));
    assert_eq!((playback.shuffle, playback.repeat, playback.volume), (Some(false), Some(Repeat::Track), Some(0.55)));
    assert_eq!((playback.liked, playback.device.as_str()), (Some(true), "Soloist"), "the heart and the account's name of the device");
    assert!(spotify.details().contains("de Músicas curtidas") && spotify.details().contains("nas Músicas curtidas"), "{}", spotify.details());
    // a playlist's name comes from the account's playlists
    spotify.now.as_mut().unwrap().playback.context = Some(PlayContext { uri: "spotify:playlist:1aaaaaaaaaaaaaaaaaaaaa".into(), name: String::new() });
    spotify.take_reply(Reply::Playlists(Ok(parse_playlists(&fixture(include_str!("../../fixtures/spotify_web_playlists.json"))))), 0);
    spotify.enrich();
    assert_eq!(spotify.now.as_ref().unwrap().playback.context.as_ref().map(PlayContext::label), Some("Para Programar".into()));
    // the MCP's first hit comes back to be played
    let then = Reply::Then(Command::PlayUri(TRACK.into()));

    assert_eq!(spotify.take_reply(then, 0), Some(Command::PlayUri(TRACK.into())));
    // a refused token: no more calls until it is linked again, and the page says why
    let refused = Failure { message: RELINK.into(), unauthorized: true, status: 401 };

    spotify.take_reply(Reply::Queue(Err(refused)), 0);
    assert!(!spotify.account.linked() && matches!(spotify.account.link, Link::Failed(_)));
    assert_eq!(spotify.library.error.as_ref().map(|(why, _)| why.as_str()), Some(RELINK));
    spotify.enrich();
    assert_eq!(spotify.now.as_ref().unwrap().playback.liked, Some(true), "what it knew stays until it is unlinked");
}

#[test]
fn the_controls_go_to_soloist_or_to_the_account() {
    let mut app = app_with(Player::Soloist, false);

    app.spotify_set_shuffle(true).unwrap();
    app.spotify_set_repeat(Repeat::Track).unwrap();
    app.spotify_set_volume(0.474).unwrap();
    app.spotify_seek(999_999).unwrap();
    app.spotify_play_uri("spotify:playlist:abc").unwrap();
    app.spotify_add_to_queue(TRACK).unwrap();

    let asked = [
        Command::Shuffle(true),
        Command::Repeat(Repeat::Track),
        Command::Volume(47),
        Command::Seek(999_999),
        Command::PlayUri("spotify:playlist:abc".into()),
        Command::Enqueue(TRACK.into()),
    ];

    assert_eq!(app.integrations.spotify.asked, asked);
    let playback = &app.integrations.spotify.now.as_ref().unwrap().playback;

    assert_eq!(
        (playback.shuffle, playback.repeat, playback.volume, playback.position_ms),
        (Some(true), Some(Repeat::Track), Some(0.47), Some(213_573)),
        "shown before it answers"
    );

    // what goes nowhere says why
    assert!(app.spotify_add_to_queue("spotify:album:abc").is_err() && app.spotify_play_uri("rm -rf").is_err());
    assert_eq!(app.spotify_play_uri(LIKED_SONGS), Err(NEEDS_ACCOUNT.into()));
    assert_eq!(app.spotify_set_liked("", true), Err(NEEDS_ACCOUNT.into()));
    assert_eq!(app.spotify_search("lista"), Err(LISTS_NEED_ACCOUNT.into()));
    assert!(app.integrations.spotify.library.web_asked.is_empty());
    app.prefs.integrations.spotify = false;
    assert!(app.spotify_set_shuffle(false).unwrap_err().contains("desligado"));
    // with the account: Liked Songs, the heart, the search and another device by the Web API
    let mut app = app_with(Player::Soloist, true);

    app.spotify_play_uri(LIKED_SONGS).unwrap();
    app.spotify_set_liked("", true).unwrap();
    assert_eq!(app.integrations.spotify.now.as_ref().unwrap().playback.liked, Some(true), "the heart fills at once");
    app.spotify_search("  lista  ").unwrap();
    assert!(app.integrations.spotify.library.searching);
    assert!(app.spotify_transfer("dev-phone").is_err(), "not one of the account's devices");
    app.integrations.spotify.library.devices = parse_devices(&fixture(include_str!("../../fixtures/spotify_web_devices.json")));
    app.spotify_transfer("dev-phone").unwrap();
    assert_eq!(app.integrations.spotify.library.web_asked, ["control", "save", "search", "control"]);
    assert!(app.integrations.spotify.asked.is_empty(), "nothing to Soloist");
    app.spotify_search("").unwrap();
    assert!(!app.integrations.spotify.library.searching && app.integrations.spotify.library.found.is_none(), "an empty search clears it");
}

#[test]
fn a_song_of_the_queue_plays_at_once_in_what_plays() {
    // the Web API starts it in the playlist (the list goes on after it), not by skipping to it
    let playlist = "spotify:playlist:1aaaaaaaaaaaaaaaaaaaaa";

    assert_eq!(play_in_body(TRACK, playlist), json!({ "context_uri": playlist, "offset": { "uri": TRACK } }));
    let mut app = app_with(Player::App, true);

    assert_eq!(app.integrations.spotify.play_context(), None, "nothing says where it plays");
    app.integrations.spotify.now.as_mut().unwrap().playback.context = Some(PlayContext { uri: playlist.into(), name: String::new() });
    assert_eq!(app.integrations.spotify.play_context().as_deref(), Some(playlist));
    app.integrations.spotify.now.as_mut().unwrap().playback.context = Some(PlayContext { uri: "spotify:collection".into(), name: String::new() });
    assert_eq!(app.integrations.spotify.play_context(), Some(liked_songs_uri("ana")), "Liked Songs as the account's own");
    app.spotify_press(&format!("jump {TRACK}"));
    assert_eq!(app.integrations.spotify.library.web_asked, ["control"], "one call");
    assert!(app.integrations.spotify.asked.is_empty(), "no next");
    // without the account the player plays it
    let mut soloist = app_with(Player::Soloist, false);

    soloist.spotify_press(&format!("jump {TRACK}"));
    assert_eq!(soloist.integrations.spotify.asked, [Command::PlayUri(TRACK.into())]);
}

#[test]
fn an_opened_playlist_brings_its_songs_fifty_at_a_time() {
    let playlist = "spotify:playlist:1aaaaaaaaaaaaaaaaaaaaa";

    assert_eq!(songs_path(playlist, 50).as_deref(), Some("/playlists/1aaaaaaaaaaaaaaaaaaaaa/items?limit=50&offset=50"));
    assert_eq!(songs_path(LIKED_SONGS, 0).as_deref(), Some("/me/tracks?limit=50&offset=0"));
    assert_eq!(songs_path("spotify:track:abc", 0), None, "only a list opens");

    // a page: its songs (`item` or the older `track`), and whether more follow
    let page = fixture(&format!(
        r#"{{"items":[{{"item":{{"uri":"{TRACK}","name":"Canção","artists":[{{"name":"Banda"}}]}}}}],"next":"https://api.spotify.com/v1/…"}}"#
    ));

    let (songs, more) = parse_songs(&page);

    assert_eq!((songs.len(), songs[0].title.as_str(), more), (1, "Canção", true));
    assert!(!parse_songs(&fixture(r#"{"items":[],"next":null}"#)).1);
    // opened from the lists: the first page is asked once, the answer goes in, an old one is dropped
    let mut app = app_with(Player::App, true);
    let list = PlaylistItem { uri: playlist.into(), name: "Foco".into(), ..Default::default() };

    app.integrations.spotify.library.playlists = vec![list];
    app.spotify_press(&format!("open {playlist}"));
    let spotify = &mut app.integrations.spotify;

    assert_eq!(spotify.library.web_asked, ["songs"]);
    spotify.more_songs();
    assert_eq!(spotify.library.web_asked, ["songs"], "one page at a time");
    let song = |index: usize| QueueItem::new(&format!("spotify:track:{index}"), &format!("Música {index}"), "Banda", "", None).unwrap();

    spotify.take_reply(Reply::Songs(playlist.into(), 0, Ok(((0..50).map(song).collect(), true))), 0);
    spotify.take_reply(Reply::Songs(playlist.into(), 0, Ok((vec![song(99)], false))), 0);
    let opened = spotify.library.opened.as_ref().unwrap();

    assert_eq!((opened.songs.len(), opened.more, opened.loading), (50, true, false), "the repeated page is dropped");
    // the end of the scroll asks the next, from 50
    spotify.more_songs();
    assert_eq!(spotify.library.web_asked, ["songs", "songs"]);
    spotify.take_reply(Reply::Songs(playlist.into(), 50, Ok((vec![song(50)], false))), 0);
    assert_eq!(spotify.library.opened.as_ref().unwrap().songs.len(), 51);
    spotify.more_songs();
    assert_eq!(spotify.library.web_asked.len(), 2, "none follow");
    // a song pressed plays in the list, with one call; Voltar closes it
    app.spotify_press(&format!("playin {playlist} spotify:track:7"));
    assert_eq!(app.integrations.spotify.library.web_asked.last(), Some(&"control"));
    assert!(app.integrations.spotify.asked.is_empty(), "no next");
    app.spotify_press("close");
    assert!(app.integrations.spotify.library.opened.is_none());
    // the end of the list asks a little before it is in sight
    let clip = Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 200.0));

    assert!(crate::app::island::music::wants_more(clip, Rect::from_min_size(Pos2::new(0.0, 250.0), Vec2::new(300.0, 1.0))));
    assert!(!crate::app::island::music::wants_more(clip, Rect::from_min_size(Pos2::new(0.0, 900.0), Vec2::new(300.0, 1.0))));
}

#[test]
fn the_pages_presses_are_done_by_the_next_tick() {
    let ctx = Context::default();
    let mut app = app_with(Player::Soloist, false);

    let presses =
        ["spotify shuffle off", "spotify repeat context", "spotify volume 30", "spotify enqueue spotify:track:abc", "spotify volume 300", "spotify explodir"];

    app.integrations.pressed.borrow_mut().extend(presses.map(String::from));
    app.integrations_tick(&ctx);
    let asked = [Command::Shuffle(false), Command::Repeat(Repeat::Context), Command::Volume(30), Command::Enqueue("spotify:track:abc".into())];

    assert_eq!(app.integrations.spotify.asked, asked);
    assert_eq!(app.integrations.spotify.library.error, None, "a word it does not know is no error");
    app.integrations.pressed.borrow_mut().push(format!("spotify play {LIKED_SONGS}"));
    app.integrations_tick(&ctx);
    assert_eq!(app.integrations.spotify.library.error.as_ref().map(|(why, _)| why.as_str()), Some(NEEDS_ACCOUNT), "the page says why");
}
