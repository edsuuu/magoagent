//! What the players say beyond the track (shuffle, repeat, the volume, the time, where it comes from), Soloist's queue,
//! the page's commands and where each one goes.

use super::*;
use crate::app::integrations::spotify::playback::{
    CLOSED, NEEDS_ACCOUNT_QUEUE, PlayContext, Playback, Repeat, Route, System, differs, is_liked_songs, is_track, mpris_playback, route, soloist_playback,
    spotify_uri, uri_allowed,
};
use crate::app::integrations::spotify::queue::{QueueItem, from_soloist_queue, from_soloist_queue_text};

const TIMESTAMP: i64 = 1_747_654_321_000;

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn mpris_says_shuffle_repeat_the_volume_the_time_and_the_tracks_uri() {
    let now = from_mpris(&fixture(include_str!("../../fixtures/spotify_mpris.json"))).unwrap();
    let playback = &now.playback;
    assert_eq!(playback.track_uri, "spotify:track:4uLU6hMCjMI75M1A2tKUQC", "from xesam:url");
    assert_eq!((playback.shuffle, playback.repeat, playback.volume), (Some(false), Some(Repeat::Off), Some(0.42)));
    assert_eq!((playback.position_ms, playback.duration_ms), (Some(12_500), Some(213_573)), "µs to ms");
    assert!(playback.read_at_ms > 0, "the time runs from the poll");
    // LoopStatus in MPRIS's words, a volume out of range, the uri from the trackid alone, no position
    let property = |kind: &str, data: Value| json!({ "type": kind, "data": data });
    let properties = json!({ "LoopStatus": property("s", json!("Playlist")), "Volume": property("d", json!(1.7)), "Shuffle": property("b", json!(true)) });
    let metadata = json!({ "mpris:trackid": property("o", json!("/com/spotify/track/abc123")), "mpris:length": property("t", json!(0)) });
    let playback = mpris_playback(&properties, &metadata, 99);
    assert_eq!((playback.repeat, playback.volume, playback.shuffle), (Some(Repeat::Context), Some(1.0), Some(true)));
    assert_eq!((playback.track_uri.as_str(), playback.position_ms, playback.duration_ms, playback.read_at_ms), ("spotify:track:abc123", None, None, 0));
    let track = json!({ "LoopStatus": property("s", json!("Track")) });
    assert_eq!(mpris_playback(&track, &json!({}), 0).repeat, Some(Repeat::Track));
}

#[test]
fn soloist_says_its_options_the_time_and_where_it_comes_from() {
    let root = fixture(include_str!("../../fixtures/spotify_soloist_state.json"));
    let playback = soloist_playback(&root, TIMESTAMP + 5_000);
    assert_eq!((playback.shuffle, playback.repeat, playback.volume), (Some(true), Some(Repeat::Context), Some(0.64)));
    assert_eq!((playback.position_ms, playback.duration_ms, playback.read_at_ms), (Some(45_000), Some(213_573), TIMESTAMP), "its clock: near ours");
    assert_eq!(playback.context, Some(PlayContext { uri: "spotify:playlist:37i9dQZF1DXcBWIGoYBM5M".into(), name: "Lista de Domingo".into() }));
    assert_eq!((playback.track_uri.as_str(), playback.device.as_str(), playback.liked), ("spotify:track:4uLU6hMCjMI75M1A2tKUQC", "Soloist", None));
    assert_eq!(soloist_playback(&root, TIMESTAMP + 3_600_000).read_at_ms, TIMESTAMP + 3_600_000, "a clock far off: ours");
    // the flat spellings
    let flat = json!({ "shuffle": false, "repeat": "track", "position_ms": 1000, "volume": 250, "item": { "uri": "spotify:track:abc", "duration_ms": 5000 } });
    let playback = soloist_playback(&flat, 7);
    assert_eq!((playback.shuffle, playback.repeat, playback.volume), (Some(false), Some(Repeat::Track), Some(1.0)));
    assert_eq!((playback.position_ms, playback.duration_ms, playback.read_at_ms, playback.context), (Some(1000), Some(5000), 7, None));
    // the whole state, through from_soloist
    let now = from_soloist(&root).unwrap();
    assert_eq!((now.track.title.as_str(), now.track.album.as_str()), ("Canção de Teste", "Disco de Teste"));
    assert_eq!(now.playback.context.as_ref().map(PlayContext::label), Some("Lista de Domingo".into()));
}

#[test]
fn spotifast_through_its_soloist_takes_what_only_soloist_says() {
    let now = |player: Player, playback: Playback| Some(Now { track: track("Canção"), playing: true, player, playback, ..Default::default() });
    let spotifast = Playback { volume: Some(0.5), device: "Spotifast".into(), ..Default::default() };
    let context = PlayContext { uri: "spotify:playlist:abc".into(), name: "Lista".into() };
    let soloist = Playback {
        track_uri: "spotify:track:abc".into(),
        shuffle: Some(true),
        repeat: Some(Repeat::Track),
        context: Some(context.clone()),
        ..Default::default()
    };
    let one = pick(now(Player::Spotifast, spotifast), now(Player::Soloist, soloist)).unwrap();
    assert!(one.through_soloist);
    assert_eq!((one.playback.volume, one.playback.device.as_str()), (Some(0.5), "Spotifast"), "its own");
    assert_eq!((one.playback.shuffle, one.playback.repeat, one.playback.context), (Some(true), Some(Repeat::Track), Some(context)));
    assert_eq!(one.playback.track_uri, "spotify:track:abc");
}

#[test]
fn the_time_goes_on_while_it_plays_and_a_song_playing_on_says_nothing_new() {
    let at = |position: u64, read_at_ms: i64, playing: bool| Now {
        track: track("Canção"),
        playing,
        playback: Playback { position_ms: Some(position), duration_ms: Some(12_000), read_at_ms, ..Default::default() },
        ..Default::default()
    };
    assert_eq!(at(10_000, 1_000_000, true).position_at(1_001_500), Some(11_500));
    assert_eq!(at(10_000, 1_000_000, true).position_at(1_009_000), Some(12_000), "never past the end");
    assert_eq!(at(10_000, 1_000_000, false).position_at(1_009_000), Some(10_000), "paused: it stays");
    assert_eq!(Now::default().position_at(5), None);
    let older = Some(at(10_000, 1_000_000, true));
    assert!(!differs(Some(&older), &Some(at(11_000, 1_001_000, true))), "playing on");
    assert!(differs(Some(&older), &Some(at(2_000, 1_001_000, true))), "a seek");
    assert!(differs(Some(&older), &Some(at(11_000, 1_001_000, false))), "paused");
    let mut shuffled = at(11_000, 1_001_000, true);
    shuffled.playback.shuffle = Some(true);
    assert!(differs(Some(&older), &Some(shuffled)), "anything but the time");
    assert!(differs(None, &older) && differs(Some(&older), &None) && !differs(Some(&None), &None));
}

#[test]
fn only_spotifys_own_addresses_are_played() {
    assert_eq!(spotify_uri("spotify:track:4uLU6hMCjMI75M1A2tKUQC"), Some("spotify:track:4uLU6hMCjMI75M1A2tKUQC".into()));
    assert_eq!(spotify_uri("https://open.spotify.com/intl-pt/track/4uLU6hMCjMI75M1A2tKUQC?si=abc"), Some("spotify:track:4uLU6hMCjMI75M1A2tKUQC".into()));
    assert_eq!(spotify_uri("/com/spotify/episode/abc"), Some("spotify:episode:abc".into()));
    for not_a_track in ["spotify:album:abc", "https://example.com/track/abc", "spotify:track:a b", "spotify:track:", ""] {
        assert_eq!(spotify_uri(not_a_track), None, "{not_a_track:?}");
    }
    for good in [
        "spotify:track:abc",
        "spotify:episode:abc",
        "spotify:album:abc",
        "spotify:playlist:abc",
        "spotify:artist:abc",
        "spotify:show:abc",
        "spotify:user:ana.b:collection",
    ] {
        assert!(uri_allowed(good), "{good}");
    }
    for bad in [
        "spotify:track:",
        "spotify:track:abc;rm",
        "spotify:user::collection",
        "spotify:collection",
        "spotify:local:abc",
        "https://open.spotify.com/track/abc",
        "spotify:track:a:b",
    ] {
        assert!(!uri_allowed(bad), "{bad}");
    }
    assert!(is_track("spotify:track:abc") && is_track("spotify:episode:abc") && !is_track("spotify:playlist:abc"));
    assert!(is_liked_songs("spotify:collection") && is_liked_songs("spotify:user:ana:collection") && !is_liked_songs("spotify:playlist:abc"));
}

#[test]
fn where_it_comes_from_is_named() {
    let context = |uri: &str, name: &str| PlayContext { uri: uri.into(), name: name.into() };
    assert_eq!(context("spotify:user:ana:collection", "").label(), "Músicas curtidas");
    assert_eq!(context("spotify:collection", "Liked Songs").label(), "Músicas curtidas", "always in Portuguese");
    assert_eq!(context("spotify:playlist:abc", "Lista").label(), "Lista");
    assert_eq!((context("spotify:playlist:abc", "").label(), context("spotify:album:abc", "").label()), ("Playlist".into(), "Álbum".into()));
    assert_eq!(context("spotify:album:abc", "").kind(), "album");
}

#[test]
fn the_pages_words_are_commands() {
    assert_eq!(Repeat::parse("Playlist"), Some(Repeat::Context));
    assert_eq!((Repeat::parse("none"), Repeat::parse("faixa"), Repeat::parse("x")), (Some(Repeat::Off), Some(Repeat::Track), None));
    assert_eq!([Repeat::Off.next(), Repeat::Context.next(), Repeat::Track.next()], [Repeat::Context, Repeat::Track, Repeat::Off]);
    assert_eq!([Repeat::Off.mpris(), Repeat::Context.mpris(), Repeat::Track.mpris()], ["None", "Playlist", "Track"]);
    let parsed = |text: &str| Command::parse(text);
    assert_eq!(parsed("shuffle on"), Some(Command::Shuffle(true)));
    assert_eq!(parsed("shuffle false"), Some(Command::Shuffle(false)));
    assert_eq!(parsed("repeat track"), Some(Command::Repeat(Repeat::Track)));
    assert_eq!(parsed("volume 47"), Some(Command::Volume(47)));
    assert_eq!(parsed("seek 61000"), Some(Command::Seek(61_000)));
    assert_eq!(parsed("play spotify:playlist:abc"), Some(Command::PlayUri("spotify:playlist:abc".into())));
    assert_eq!(parsed("enqueue spotify:track:abc"), Some(Command::Enqueue("spotify:track:abc".into())));
    for nothing in ["volume 140", "volume -1", "seek ontem", "shuffle talvez", "play rm -rf", "enqueue spotify:album:abc", "playpause agora", "tocar"] {
        assert_eq!(parsed(nothing), None, "{nothing}");
    }
    // Soloist's words for each
    let args = |command: Command| soloist_args(&command, true).join(" ");
    assert_eq!(args(Command::Shuffle(true)), "shuffle on");
    assert_eq!(args(Command::Repeat(Repeat::Context)), "repeat context");
    assert_eq!(args(Command::Volume(40)), "volume 40");
    assert_eq!(args(Command::Seek(61_000)), "seek 61000");
    assert_eq!(args(Command::PlayUri("spotify:album:abc".into())), "play spotify:album:abc");
    assert_eq!(args(Command::Enqueue("spotify:track:abc".into())), "add-to-queue spotify:track:abc");
}

#[cfg(not(any(windows, target_os = "macos")))]
#[test]
fn mpris_gets_each_control_as_busctl_takes_it() {
    let name = "org.mpris.MediaPlayer2.spotify";
    let line = |track_id: &str, command: Command| super::super::mpris::arguments(name, track_id, &command).map(|words| words.join(" "));
    let player = format!("{name} /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player");
    assert_eq!(line("", Command::Next), Some(format!("call {player} Next")));
    assert_eq!(line("", Command::Shuffle(true)), Some(format!("set-property {player} Shuffle b true")));
    assert_eq!(line("", Command::Repeat(Repeat::Off)), Some(format!("set-property {player} LoopStatus s None")));
    assert_eq!(line("", Command::Volume(47)), Some(format!("set-property {player} Volume d 0.47")));
    assert_eq!(line("/com/spotify/track/abc", Command::Seek(61_000)), Some(format!("call {player} SetPosition ox /com/spotify/track/abc 61000000")));
    assert_eq!(line("", Command::Seek(61_000)), None, "SetPosition needs the track");
    assert_eq!(line("", Command::PlayUri("spotify:album:abc".into())), Some(format!("call {player} OpenUri s spotify:album:abc")));
    assert_eq!(line("", Command::Enqueue("spotify:track:abc".into())), None, "MPRIS has no queue");
}

#[test]
fn the_mac_says_shuffle_repeat_the_volume_and_the_time_too() {
    let fields = ["playing", "Canção", "Banda", "Disco", "https://i.scdn.co/image/abc", "true", "false", "64", "83,5", "213573", "spotify:track:abc"];
    let now = from_applescript(&fields.join("\u{1f}")).unwrap();
    let playback = &now.playback;
    assert_eq!((playback.shuffle, playback.repeat, playback.volume), (Some(true), Some(Repeat::Off), Some(0.64)));
    assert_eq!((playback.position_ms, playback.duration_ms, playback.track_uri.as_str()), (Some(83_500), Some(213_573), "spotify:track:abc"));
}

#[test]
fn each_control_goes_to_the_player_the_account_or_nowhere() {
    use Command::{Enqueue, Next, PlayPause, PlayUri, Seek, Shuffle, Volume};
    let (linux, windows, mac) = (System::Linux, System::Windows, System::Mac);
    let to = |system, player, through, linked, command: Command| route(system, player, through, linked, &command);
    let track = || "spotify:track:abc".to_string();
    // nothing open here: only the account reaches a device
    assert_eq!(to(linux, None, false, false, Next), Route::No(CLOSED));
    assert_eq!(to(linux, None, false, true, Shuffle(true)), Route::Web);
    // Soloist (and Spotifast through it) does everything itself, the queue too
    for command in [Shuffle(true), Command::Repeat(Repeat::Track), Volume(10), Seek(1), PlayUri(track()), Enqueue(track())] {
        assert_eq!(to(linux, Some(Player::Soloist), false, false, command.clone()), Route::Player, "{command:?}");
        assert_eq!(to(linux, Some(Player::Spotifast), true, false, command.clone()), Route::Player, "{command:?}");
    }
    // the three buttons always go to the player
    assert_eq!(to(windows, Some(Player::App), false, true, PlayPause), Route::Player);
    // a queue only exists in the account for the others
    assert_eq!(to(linux, Some(Player::App), false, false, Enqueue(track())), Route::No(NEEDS_ACCOUNT_QUEUE));
    assert_eq!(to(linux, Some(Player::Spotifast), false, true, Enqueue(track())), Route::Web);
    // the app: by the account when linked, else its own controls where the system has them
    assert_eq!(to(linux, Some(Player::App), false, true, Volume(10)), Route::Web);
    assert_eq!(to(linux, Some(Player::App), false, false, Volume(10)), Route::Player);
    assert_eq!(to(windows, Some(Player::App), false, false, Volume(10)), Route::Player);
    assert_eq!(to(windows, Some(Player::App), false, false, Seek(1000)), Route::Player);
    assert!(matches!(to(mac, Some(Player::App), false, false, Command::Repeat(Repeat::Track)), Route::No(_)));
    assert_eq!(to(mac, Some(Player::App), false, false, Command::Repeat(Repeat::Context)), Route::Player);
    // Spotifast's own MPRIS takes the writes; a uri goes by the account when there is one
    assert_eq!(to(linux, Some(Player::Spotifast), false, true, Shuffle(false)), Route::Player);
    assert_eq!(to(linux, Some(Player::Spotifast), false, true, PlayUri(track())), Route::Web);
    assert_eq!(to(linux, Some(Player::Spotifast), false, false, PlayUri(track())), Route::Player);
    // Liked Songs is the account's own context, even on Soloist
    assert_eq!(to(linux, Some(Player::Soloist), false, true, PlayUri("spotify:user:ana:collection".into())), Route::Web);
}

/// The media session has no volume on Windows, but the Windows mixer does: Spotify's own session there, turned by the
/// player's thread (and by the account when there is one).
#[test]
fn on_windows_the_volume_goes_by_the_player_through_the_windows_mixer() {
    use Command::Volume;
    let to = |linked: bool, percent: u8| route(System::Windows, Some(Player::App), false, linked, &Volume(percent));
    assert_eq!(to(false, 10), Route::Player);
    assert_eq!((to(false, 0), to(false, 100)), (Route::Player, Route::Player), "silence and the top too");
    assert_eq!(to(true, 40), Route::Web, "the account reaches the app's volume as well");
    // a Spotify with no sound has no session in the mixer: the page says where the volume is (the refused control)
    #[cfg(windows)]
    assert_eq!(super::super::controls::refused(&Volume(40)), "O Spotify não está tocando som agora: o volume dele aparece quando ele toca.");
}

#[test]
fn soloists_queue_is_read_from_its_json_and_from_its_text() {
    let root = fixture(include_str!("../../fixtures/spotify_soloist_queue.json"));
    let queue = from_soloist_queue(&root, 10).unwrap();
    let first = QueueItem {
        uri: "spotify:track:1111111111111111111111".into(),
        title: "Próxima da Fila".into(),
        artist: "Banda Exemplo, Convidada".into(),
        cover_url: "https://i.scdn.co/image/fila-pequena".into(),
        duration_ms: Some(180_000),
    };
    assert_eq!(queue[0], first, "upcoming only, the small cover");
    assert_eq!(queue.iter().map(|item| item.title.as_str()).collect::<Vec<_>>(), ["Próxima da Fila", "Da Playlist", "Um Episódio"], "no nameless entry");
    assert_eq!(from_soloist_queue(&root, 2).unwrap().len(), 2);
    assert_eq!(from_soloist_queue(&json!({ "type": "error" }), 10), None);
    let text = from_soloist_queue_text(include_str!("../../fixtures/spotify_soloist_queue.txt"), 10);
    let lines: Vec<(&str, &str, &str)> = text.iter().map(|item| (item.uri.as_str(), item.title.as_str(), item.artist.as_str())).collect();
    assert_eq!(
        lines,
        [("spotify:track:1111111111111111111111", "Próxima da Fila", "Banda Exemplo, Convidada"), ("", "Da Playlist", "Outra Banda"), ("", "Sem Artista", "")]
    );
    assert_eq!(from_soloist_queue_text(include_str!("../../fixtures/spotify_soloist_queue.txt"), 1).len(), 1);
    assert!(from_soloist_queue_text("Error: not logged in\n", 10).is_empty());
}

/// The bars take the cover's liveliest colour, bright enough on black, as the Dynamic Island's; a grey cover gives a
/// light grey.
#[test]
fn the_bars_take_the_covers_liveliest_colour() {
    let cover = |pixels: &[[u8; 4]]| pixels.iter().flatten().copied().collect::<Vec<u8>>();
    // mostly dark grey with a red patch: the red wins, lifted
    let red = super::super::accent(&cover(&[[30, 30, 30, 255], [30, 30, 30, 255], [30, 30, 30, 255], [150, 20, 20, 255]]));
    assert!(red.r() > 200 && red.g() < 80 && red.b() < 80, "{red:?}");
    let grey = super::super::accent(&cover(&[[90, 90, 90, 255], [120, 120, 120, 255]]));
    assert_eq!(grey, Color32::from_gray(0xa8), "a grey cover: grey dots, as the Dynamic Island's");
}

/// Asked ("as cores podem ter um degradê"): the bars go from the cover's liveliest colour to another one of it, and a
/// cover of a single colour goes to a lighter tone of it.
#[test]
fn the_bars_take_two_colours_of_the_cover() {
    let cover = |pixels: &[[u8; 4]]| pixels.iter().flatten().copied().collect::<Vec<u8>>();
    // mostly red, with a blue patch: the two colours of the bars
    let two = cover(&[[200, 20, 20, 255], [200, 20, 20, 255], [200, 20, 20, 255], [20, 20, 200, 255]]);
    let (red, blue) = super::super::accents(&two);
    assert!(red.r() > 200 && red.g() < 80 && red.b() < 80, "the liveliest first: {red:?}");
    assert!(blue.b() > 200 && blue.r() < 80 && blue.g() < 80, "another colour after it: {blue:?}");
    assert_eq!(super::super::accent(&two), red, "the first one is what Luna takes");
    // one colour only: a lighter tone of it
    let (red, light) = super::super::accents(&cover(&[[200, 20, 20, 255], [150, 30, 30, 255]]));
    assert!(light.r() >= red.r() && light.g() >= red.g() && light.b() >= red.b() && light != red, "{red:?} → {light:?}");
    // a grey cover: two greys, as the Dynamic Island's dots
    let (grey, lighter) = super::super::accents(&cover(&[[90, 90, 90, 255], [120, 120, 120, 255]]));
    assert_eq!(grey, Color32::from_gray(0xa8));
    assert!(lighter.g() > grey.g(), "a lighter grey after it: {lighter:?}");
}

/// Asked ("a transição entre músicas está ruim na ilha"): the bars' colour goes from the pair before to the new one in
/// the middle of the fade, and the cover before stays until the new one comes.
#[test]
fn the_bars_colour_goes_from_the_old_one_to_the_new_one() {
    use crate::app::integrations::spotify::{CoverFade, FADE};
    let old = Color32::from_rgb(220, 40, 40);
    let new = Color32::from_rgb(40, 60, 200);
    let fade = CoverFade { colours: Some((new, new)), before: Some((old, old)), at: 10.0, ..Default::default() };
    assert_eq!(fade.colours_at(10.0), Some((old, old)), "at once: the old one");
    assert_eq!(fade.colours_at(10.0 + FADE), Some((new, new)), "at the end: the new one");
    assert!(fade.fading(10.0 + FADE / 2.0) && !fade.fading(10.0 + FADE), "frames only while it comes in");
    let (middle, _) = fade.colours_at(10.0 + FADE / 2.0).unwrap();
    assert!(middle.r() < old.r() && middle.r() > new.r(), "between the two: {middle:?}");
    assert!(middle.b() > old.b() && middle.b() < new.b(), "between the two: {middle:?}");
    // the new cover still to come: the colours before are the ones showing
    let waiting = CoverFade { colours: None, before: Some((old, old)), previous: None, at: 20.0 };
    assert_eq!(waiting.progress(20.0), 0.0);
    assert_eq!(waiting.colours_at(20.0), Some((old, old)), "no flash while the cover comes");
}

/// A new song's cover goes over the one before it, which stays until the new one comes; the tick leaves both for the
/// pill (`CoverFade`).
#[test]
fn a_new_songs_cover_fades_in_over_the_one_before_it() {
    use crate::app::integrations::spotify::CoverFade;
    let ctx = Context::default();
    let mut spotify = Spotify::default();
    spotify.take(Message::Now(playing("A")), &ctx);
    spotify.take(Message::Cover(track("A"), picture()), &ctx);
    let first = spotify.fade();
    let colours = first.colours.expect("the cover's two colours");
    assert!(first.previous.is_none() && colours.0 != colours.1, "the first cover: no fade, its own two colours");
    spotify.take(Message::Now(playing("B")), &ctx);
    let waiting = spotify.fade();
    assert!(spotify.cover.is_none() && waiting.colours.is_none(), "the new song's cover is still to come");
    assert!(waiting.previous.is_some(), "the one before stays");
    assert_eq!(waiting.colours_at(0.0), Some(colours), "with its colours, so the bars do not flash");
    spotify.take(Message::Cover(track("B"), picture()), &ctx);
    let fading = spotify.fade();
    assert!(fading.previous.is_some() && fading.colours.is_some() && fading.before == Some(colours), "it comes over it");
    assert_eq!(fading.progress(0.0), 0.0, "from the start");
    spotify.publish(&ctx);
    assert_eq!(CoverFade::read(&ctx), fading, "the pill reads what the tick left");
}
