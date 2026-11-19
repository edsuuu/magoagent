use super::*;

mod account;
mod player;
mod web;

fn track(title: &str) -> Track {
    Track::new(title, "Banda Exemplo", "Disco de Teste", "").unwrap()
}

fn playing(title: &str) -> Option<Now> {
    Some(Now { track: track(title), playing: true, ..Default::default() })
}

/// A 2 x 2 picture, as a player hands a cover.
fn picture() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());

    image::RgbaImage::from_pixel(2, 2, image::Rgba([30, 215, 96, 255])).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[test]
fn what_busctl_says_of_spotify_is_read() {
    let root: Value = serde_json::from_str(include_str!("../../fixtures/spotify_mpris.json")).unwrap();
    let now = from_mpris(&root).unwrap();

    assert!(now.playing);
    assert_eq!(now.track.title, "Canção de Teste", "one line, no control characters");
    assert_eq!((now.track.artist.as_str(), now.track.album.as_str()), ("Banda Exemplo, Convidada", "Disco de Teste"));
    assert_eq!(now.track.cover_url, "https://i.scdn.co/image/ab67616d0000b273abc");
    // paused, and a player with nothing loaded
    let paused = json!({ "type": "a{sv}", "data": [{ "PlaybackStatus": { "type": "s", "data": "Paused" }, "Metadata": { "type": "a{sv}", "data": { "xesam:title": { "type": "s", "data": "T" } } } }] });

    assert_eq!(from_mpris(&paused), Some(Now { track: Track::new("T", "", "", "").unwrap(), playing: false, ..Default::default() }));
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
    assert_eq!(watch.wait(None), QUICK, "its cover owed: asked again soon");
    assert_eq!(kinds(&watch.step(playing("A"), |_| Some(vec![1]))), ["cover"]);
    assert_eq!(watch.wait(None), EVERY, "the cover came: at the usual pace");
    assert_eq!(watch.wait(Some(std::time::Instant::now() + QUICK_FOR)), QUICK, "after a control: soon");
    assert!(watch.step(playing("A"), |_| panic!("already sent")).is_empty());
    // paused: the same track, the cover stays
    assert_eq!(kinds(&watch.step(Some(Now { track: track("A"), playing: false, ..Default::default() }), |_| panic!("already sent"))), ["now"]);
    // another track: its cover, asked a few polls and no more
    assert_eq!(kinds(&watch.step(playing("B"), |_| None)), ["now"]);
    let mut asked = 1;

    for _ in 0..40 {
        watch.step(playing("B"), |_| {
            asked += 1;
            None
        });
    }

    assert_eq!(asked, COVER_TRIES, "a track with no cover is not asked for ever");
    assert_eq!(watch.wait(None), EVERY, "and then at the usual pace");
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
    spotify.take(Message::Now(Some(Now { track: track("A"), playing: false, ..Default::default() })), &ctx);
    assert!(spotify.cover.is_some(), "paused: the same cover");
    assert_eq!(spotify.report(), "Pausado: A — Banda Exemplo");
    spotify.take(Message::Now(playing("B")), &ctx);
    assert!(spotify.cover.is_none(), "another track: its cover is still to come");
    spotify.take(Message::Now(None), &ctx);
    assert!(spotify.report().contains("não está aberto"));
}

#[test]
fn a_song_paused_for_half_a_minute_leaves_the_island() {
    let ctx = Context::default();
    let mut spotify = Spotify::default();
    let paused = || Message::Now(Some(Now { track: track("A"), playing: false, ..Default::default() }));

    spotify.take(Message::Now(playing("A")), &ctx);
    assert!(spotify.shown(1000.0).is_some(), "playing: always shown");
    spotify.take(paused(), &ctx);
    assert!(spotify.shown(10.0).is_some(), "just paused: still shown");
    spotify.take(paused(), &ctx);
    assert!(spotify.shown(29.0).is_some(), "the same pause told again does not restart it");
    assert!(spotify.shown(31.0).is_none(), "paused for 30 s: gone");
    spotify.take(Message::Now(playing("A")), &ctx);
    assert!(spotify.shown(1000.0).is_some(), "playing again: back");
}

#[test]
fn spotify_is_on_the_island_while_it_plays_and_its_buttons_ask_the_player() {
    let mut app = DrawingApp::default();
    let ctx = Context::default();

    app.integrations.spotify.now = playing("A");
    assert!(app.integration_items().is_empty() && app.spotify_do(Command::Next).is_err(), "off: no pill, nothing asked");
    app.prefs.integrations.spotify = true;
    let item = app.integration_items().remove(0);

    assert_eq!((item.id.as_str(), item.name, item.kind.as_str()), ("integration:spotify", "Spotify", "tocando"));
    assert_eq!(item.rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(), ["A", "Banda Exemplo"]);
    assert_eq!(item.chips.iter().map(|chip| chip.0).collect::<Vec<_>>(), ["Anterior", "Pausar", "Próxima"]);
    // the card's buttons, written down by the island and done by the next tick
    app.integrations.pressed.borrow_mut().extend(item.chips.iter().map(|chip| chip.1.clone()).chain(["spotify explodir".to_string()]));
    app.integrations_tick(&ctx);
    assert_eq!(app.integrations.spotify.asked, [Command::Previous, Command::PlayPause, Command::Next]);
    app.integrations.spotify.now = Some(Now { track: track("A"), playing: false, ..Default::default() });
    assert_eq!(app.integration_items()[0].chips[1].0, "Tocar");
    // closed: no pill; off: nothing is kept
    app.integrations.spotify.now = None;
    assert!(app.integration_items().is_empty() && app.spotify_do(Command::PlayPause).is_err());
    app.integrations.spotify.now = playing("A");
    app.prefs.integrations.spotify = false;
    app.integrations_tick(&ctx);
    assert!(app.integrations.spotify.now.is_none());
}

/// By hand, against the Spotify open on this computer, and Soloist on Linux (it only reads: no button is pressed):
/// `cargo test --release the_spotify_open_here_is_read -- --ignored --nocapture`
#[test]
#[ignore]
fn the_spotify_open_here_is_read() {
    #[cfg(not(any(windows, target_os = "macos")))]
    let now = {
        // each switch alone, then both
        println!("só o Spotify: {:?}", system::read(Sources { app: true, soloist: false }));
        println!("só o Soloist: {:?}", system::read(Sources { app: false, soloist: true }));
        system::read(Sources { app: true, soloist: true })
    };

    #[cfg(any(windows, target_os = "macos"))]
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

/// Soloist's `playback_state`, shaped as its WebSocket API's reference spells it (made-up names).
fn soloist_state(status: &str) -> Value {
    let named = |name: &str| json!({ "decorations": { "identity": { "name": name } } });

    json!({
        "type": "playback_state",
        "status": status,
        "item": {
            "uri": "spotify:track:abc",
            "entity_type": "track",
            "decorations": {
                "identity": { "name": "Canção" },
                "creators": [{ "entity": named("Banda") }, { "entity": named("Convidada") }],
                "parent": { "entity": named("Disco") },
                "visual_identity": { "cover": [
                    { "url": "https://i.scdn.co/image/pequena", "size": "small" },
                    { "url": "https://i.scdn.co/image/padrao", "size": "default" }
                ] },
                "playback": { "duration_ms": 200000 }
            }
        },
        "position": { "position_ms": 45000, "timestamp_ms": 1747654321000u64, "speed": 1.0 },
        "volume": 70,
        "is_active": true
    })
}

#[test]
fn what_soloist_plays_is_read_from_its_playback_state() {
    let now = from_soloist(&soloist_state("playing")).unwrap();

    assert_eq!(now.track, Track::new("Canção", "Banda, Convidada", "Disco", "https://i.scdn.co/image/padrao").unwrap(), "the default-size cover");
    assert!(now.playing && now.player == Player::Soloist);
    assert!(!from_soloist(&soloist_state("paused")).unwrap().playing);
    assert!(from_soloist(&soloist_state("buffering")).unwrap().playing, "about to play");
    assert_eq!(from_soloist(&soloist_state("idle")), None, "idle: nothing to show");
    assert_eq!(from_soloist(&json!({ "status": "paused" })), None, "nothing loaded");
    let mut bare = soloist_state("playing");

    bare["item"]["decorations"] = json!({ "identity": { "name": "Só o nome" } });
    assert_eq!(from_soloist(&bare).unwrap().track, Track::new("Só o nome", "", "", "").unwrap(), "no creators, album or cover");
}

#[test]
fn the_spotify_that_plays_is_shown_and_the_app_when_both_or_neither_do() {
    let now = |title: &str, playing: bool| Some(Now { track: Track::new(title, "", "", "").unwrap(), playing, ..Default::default() });
    let soloist = |title: &str, playing: bool| now(title, playing).map(|now| Now { player: Player::Soloist, ..now });
    let shown = |app, soloist| pick(app, soloist).map(|now| (now.player, now.track.title));

    assert_eq!(shown(now("app", false), soloist("soloist", true)), Some((Player::Soloist, "soloist".into())));
    assert_eq!(shown(now("app", true), soloist("soloist", true)), Some((Player::App, "app".into())));
    assert_eq!(shown(now("app", false), soloist("soloist", false)), Some((Player::App, "app".into())));
    assert_eq!(shown(None, soloist("soloist", false)), Some((Player::Soloist, "soloist".into())));
    assert_eq!(shown(now("app", false), None), Some((Player::App, "app".into())));
    assert_eq!(shown(None, None), None);
    // Spotifast sends its music to its own Soloist: the same song from both is one, shown on both cards
    let spotifast = |title: &str, playing: bool| now(title, playing).map(|now| Now { player: Player::Spotifast, ..now });
    let one = pick(spotifast("canção", true), soloist("canção", true)).unwrap();

    assert!(one.player == Player::Spotifast && one.through_soloist && one.on(Player::App) && one.on(Player::Soloist), "{one:?}");
    let two = pick(spotifast("outra", false), soloist("canção", true)).unwrap();

    assert!(two.player == Player::Soloist && !two.on(Player::App), "different songs: the one that plays");
    assert!(!pick(now("canção", true), soloist("canção", true)).unwrap().through_soloist, "the app never goes through Soloist");
}

#[test]
fn each_card_says_which_spotify_plays() {
    let now = |player: Player, through_soloist: bool| Now {
        track: Track::new("Canção", "Banda", "", "").unwrap(),
        playing: true,
        player,
        through_soloist,
        ..Default::default()
    };

    let spotify = |now: Now| Spotify { now: Some(now), ..Default::default() };

    assert_eq!(spotify(now(Player::App, false)).report(), "Tocando: Canção — Banda");
    assert_eq!(spotify(now(Player::Spotifast, false)).report(), "Tocando: Canção — Banda (Spotifast)");
    assert_eq!(spotify(now(Player::Spotifast, true)).report(), "Tocando: Canção — Banda (Spotifast, no Soloist)");
    assert_eq!(spotify(now(Player::Soloist, false)).report(), "Tocando: Canção — Banda (Soloist)");
    // the Spotify card shows the app and Spotifast; Soloist's, what plays through a Soloist
    let through = spotify(now(Player::Spotifast, true));

    assert_eq!(through.report_of(Player::App), through.report_of(Player::Soloist));
    assert!(spotify(now(Player::Spotifast, false)).report_of(Player::Soloist).contains("não está tocando"));
    assert!(spotify(now(Player::Soloist, false)).report_of(Player::App).contains("(o Soloist está)"));
    let soloist_missing = Spotify { soloist_missing: true, ..Default::default() };

    assert!(soloist_missing.report_of(Player::Soloist).contains("pelo Spotifast"));
}

#[test]
fn soloists_buttons_and_where_it_lives() {
    let args = |command: Command, playing: bool| soloist_args(&command, playing).join(" ");

    assert_eq!(args(Command::PlayPause, true), "pause");
    assert_eq!(args(Command::PlayPause, false), "play");
    assert_eq!((args(Command::Next, true), args(Command::Previous, true)), ("next".into(), "prev".into()));
    let env = |pairs: &'static [(&'static str, &'static str)]| move |name: &str| pairs.iter().find(|(key, _)| *key == name).map(|(_, value)| value.to_string());
    let home = env(&[("HOME", "/home/ana")]);

    assert_eq!(soloist_data(home), Some("/home/ana/.local/share/soloist".into()));
    assert_eq!(soloist_data(env(&[("HOME", "/home/ana"), ("XDG_DATA_HOME", "/dados")])), Some("/dados/soloist".into()));
    assert_eq!(soloist_data(env(&[("HOME", "/home/ana"), ("XDG_DATA_HOME", "")])), Some("/home/ana/.local/share/soloist".into()), "empty: the default");
    assert_eq!(soloist_program(home), Some("/home/ana/.local/bin/soloist".into()));
    assert_eq!(soloist_data(env(&[])), None);
    assert_eq!(spotifast_soloist(home), Some("/home/ana/.local/state/spotifast/soloist".into()), "the Soloist Spotifast starts");
    assert_eq!(spotifast_soloist(env(&[("HOME", "/home/ana"), ("XDG_STATE_HOME", "/estado")])), Some("/estado/spotifast/soloist".into()));
    assert_eq!(spotifast_soloist(env(&[])), None);
}

#[test]
fn of_the_spotify_app_and_spotifast_the_one_that_plays_is_shown() {
    let now = |title: &str, playing: bool| Some(Now { track: Track::new(title, "", "", "").unwrap(), playing, ..Default::default() });
    let shown = |answers: [Option<Now>; 2]| first_playing(answers).map(|(index, now)| (index, now.track.title));

    assert_eq!(shown([now("app", false), now("spotifast", true)]), Some((1, "spotifast".into())), "the one that plays");
    assert_eq!(shown([now("app", true), now("spotifast", true)]), Some((0, "app".into())), "both: the app");
    assert_eq!(shown([now("app", false), now("spotifast", false)]), Some((0, "app".into())), "neither: the app");
    assert_eq!(shown([None, now("spotifast", false)]), Some((1, "spotifast".into())), "only Spotifast open");
    assert_eq!(shown([None, None]), None);
}
