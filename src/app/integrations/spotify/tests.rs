use super::*;

fn track(title: &str) -> Track {
    Track::new(title, "Banda Exemplo", "Disco de Teste", "").unwrap()
}

fn playing(title: &str) -> Option<Now> {
    Some(Now { track: track(title), playing: true })
}

/// A 2 x 2 picture, as a player hands a cover.
fn picture() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(2, 2, image::Rgba([30, 215, 96, 255])).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[test]
fn what_busctl_says_of_spotify_is_read() {
    let root: Value = serde_json::from_str(include_str!("../fixtures/spotify_mpris.json")).unwrap();
    let now = from_mpris(&root).unwrap();
    assert!(now.playing);
    assert_eq!(now.track.title, "Canção de Teste", "one line, no control characters");
    assert_eq!((now.track.artist.as_str(), now.track.album.as_str()), ("Banda Exemplo, Convidada", "Disco de Teste"));
    assert_eq!(now.track.cover_url, "https://i.scdn.co/image/ab67616d0000b273abc");
    // paused, and a player with nothing loaded
    let paused = json!({ "type": "a{sv}", "data": [{ "PlaybackStatus": { "type": "s", "data": "Paused" }, "Metadata": { "type": "a{sv}", "data": { "xesam:title": { "type": "s", "data": "T" } } } }] });
    assert_eq!(from_mpris(&paused), Some(Now { track: Track::new("T", "", "", "").unwrap(), playing: false }));
    let empty = json!({ "type": "a{sv}", "data": [{ "PlaybackStatus": { "type": "s", "data": "Stopped" }, "Metadata": { "type": "a{sv}", "data": { "mpris:trackid": { "type": "o", "data": "/org/mpris/MediaPlayer2/TrackList/NoTrack" }, "xesam:title": { "type": "s", "data": "" } } } }] });
    assert_eq!(from_mpris(&empty), None);
    for bad in [
        json!({}),
        json!({ "data": [] }),
        json!({ "data": [{ "Metadata": 3 }] }),
        json!("texto"),
        json!({ "data": [{ "Metadata": { "data": { "xesam:title": { "data": 7 } } } }] }),
    ] {
        assert_eq!(from_mpris(&bad), None, "{bad}");
    }
}

#[test]
fn what_osascript_says_of_spotify_is_read() {
    let now = from_applescript("playing\u{1f}Canção de Teste\u{1f}Banda Exemplo\u{1f}Disco de Teste\u{1f}https://i.scdn.co/image/abc").unwrap();
    assert!(now.playing && now.track.title == "Canção de Teste" && now.track.cover_url == "https://i.scdn.co/image/abc");
    assert!(!from_applescript("paused\u{1f}T\u{1f}\u{1f}\u{1f}").unwrap().playing);
    for nothing in ["", "playing", "playing\u{1f}\u{1f}A\u{1f}B\u{1f}C", "a\u{1f}b\u{1f}c\u{1f}d\u{1f}e\u{1f}f"] {
        assert_eq!(from_applescript(nothing), None, "{nothing:?}");
    }
}

#[test]
fn the_windows_sessions_of_spotify_are_recognised() {
    assert!(is_spotify_app("Spotify.exe") && is_spotify_app("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"));
    assert!(!is_spotify_app("chrome.exe") && !is_spotify_app(""));
}

#[test]
fn a_cover_comes_only_from_spotifys_picture_servers() {
    for good in ["https://i.scdn.co/image/ab67616d0000b273abc", "https://mosaic.scdn.co/640/abc", "https://image-cdn-ak.spotifycdn.com/image/abc"] {
        assert!(cover_allowed(good), "{good}");
    }
    for bad in [
        "http://i.scdn.co/image/abc",
        "https://i.scdn.co.evil.example/image/abc",
        "https://evil.example/i.scdn.co/abc",
        "https://user@i.scdn.co/image/abc",
        "https://i.scdn.co:8443/image/abc",
        "https://i.scdn.co/image/a b",
        "file:///etc/passwd",
        "",
    ] {
        assert!(!cover_allowed(bad), "{bad}");
    }
}

#[test]
fn the_state_is_told_when_it_changes_and_the_cover_once() {
    let mut watch = Watch::default();
    let kinds = |messages: &[Message]| messages.iter().map(|message| if matches!(message, Message::Now(_)) { "now" } else { "cover" }).collect::<Vec<_>>();
    assert_eq!(kinds(&watch.step(None, |_| panic!("no track, no cover"))), ["now"], "the first answer is told, even closed");
    assert!(watch.step(None, |_| None).is_empty(), "nothing changed");
    // the cover comes a moment after the title
    assert_eq!(kinds(&watch.step(playing("A"), |_| None)), ["now"]);
    assert_eq!(kinds(&watch.step(playing("A"), |_| Some(vec![1]))), ["cover"]);
    assert!(watch.step(playing("A"), |_| panic!("already sent")).is_empty());
    // paused: the same track, the cover stays
    assert_eq!(kinds(&watch.step(Some(Now { track: track("A"), playing: false }), |_| panic!("already sent"))), ["now"]);
    // another track: its cover, asked a few polls and no more
    assert_eq!(kinds(&watch.step(playing("B"), |_| None)), ["now"]);
    let mut asked = 1;
    for _ in 0..10 {
        watch.step(playing("B"), |_| {
            asked += 1;
            None
        });
    }
    assert_eq!(asked, COVER_TRIES, "a track with no cover is not asked for ever");
}

#[test]
fn the_app_keeps_what_plays_and_the_cover_of_that_track_only() {
    let ctx = Context::default();
    let mut spotify = Spotify::default();
    spotify.take(Message::Now(playing("A")), &ctx);
    assert_eq!(spotify.report(), "Tocando: A — Banda Exemplo");
    spotify.take(Message::Cover(track("Outra"), picture()), &ctx);
    assert!(spotify.cover.is_none(), "a cover that came late for another track is dropped");
    spotify.take(Message::Cover(track("A"), b"isto nao e uma imagem".to_vec()), &ctx);
    assert!(spotify.cover.is_none(), "what is not a picture is no cover");
    spotify.take(Message::Cover(track("A"), picture()), &ctx);
    assert!(spotify.cover.is_some());
    spotify.take(Message::Now(Some(Now { track: track("A"), playing: false })), &ctx);
    assert!(spotify.cover.is_some(), "paused: the same cover");
    assert_eq!(spotify.report(), "Pausado: A — Banda Exemplo");
    spotify.take(Message::Now(playing("B")), &ctx);
    assert!(spotify.cover.is_none(), "another track: its cover is still to come");
    spotify.take(Message::Now(None), &ctx);
    assert!(spotify.report().contains("não está aberto"));
}

#[test]
fn spotify_is_on_the_island_while_it_plays_and_its_buttons_ask_the_player() {
    let mut app = DrawingApp::default();
    let ctx = Context::default();
    app.integrations.spotify.now = playing("A");
    assert!(app.integration_items().is_empty() && !app.spotify_control(Command::Next), "off: no pill, nothing asked");
    app.prefs.integrations.spotify = true;
    let item = app.integration_items().remove(0);
    assert_eq!((item.id.as_str(), item.name, item.kind.as_str()), ("integration:spotify", "Spotify", "tocando"));
    assert_eq!(item.rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(), ["A", "Banda Exemplo"]);
    assert_eq!(item.chips.iter().map(|chip| chip.0).collect::<Vec<_>>(), ["Anterior", "Pausar", "Próxima"]);
    // the card's buttons, written down by the island and done by the next tick
    app.integrations.pressed.borrow_mut().extend(item.chips.iter().map(|chip| chip.1.clone()).chain(["spotify explodir".to_string()]));
    app.integrations_tick(&ctx);
    assert_eq!(app.integrations.spotify.asked, [Command::Previous, Command::PlayPause, Command::Next]);
    app.integrations.spotify.now = Some(Now { track: track("A"), playing: false });
    assert_eq!(app.integration_items()[0].chips[1].0, "Tocar");
    // closed: no pill; off: nothing is kept
    app.integrations.spotify.now = None;
    assert!(app.integration_items().is_empty() && !app.spotify_control(Command::PlayPause));
    app.integrations.spotify.now = playing("A");
    app.prefs.integrations.spotify = false;
    app.integrations_tick(&ctx);
    assert!(app.integrations.spotify.now.is_none());
}

/// By hand, against the Spotify open on this computer (it only reads: no button is pressed):
/// `cargo test --release the_spotify_open_here_is_read -- --ignored --nocapture`
#[test]
#[ignore]
fn the_spotify_open_here_is_read() {
    let now = system::read();
    println!("{now:?}");
    if let Some(now) = &now {
        println!("cover: {:?} bytes", system::cover(&now.track).map(|bytes| bytes.len()));
    }
}

#[test]
fn not_installed_says_so_and_each_system_has_its_places() {
    let spotify = Spotify { missing: true, ..Default::default() };
    assert_eq!(spotify.report(), "O Spotify não está instalado neste computador.");
    let places = install_places(|name| Some(format!("/base-{name}")));
    let expected: &[&str] = if cfg!(windows) {
        &["Spotify.exe", "WindowsApps"]
    } else if cfg!(target_os = "macos") {
        &["/Applications/Spotify.app", "/base-HOME"]
    } else {
        &["/snap/bin/spotify", "com.spotify.Client", "/base-HOME"]
    };
    for piece in expected {
        assert!(places.iter().any(|place| place.to_string_lossy().contains(piece)), "{piece} in {places:?}");
    }
}
