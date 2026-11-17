//! A test scene's Spotify (`MAGOAGENT_FAKE_SPOTIFY=1` next to a `MAGOAGENT_SCENE`): a song that plays, a queue, the
//! playlists and Liked Songs made up here, so the island's page is seen and clicked with no player, no account, no
//! network and no ear on the computer's sound. A press changes only what is shown.

use super::library::{Library, PlaylistItem};
use super::playback::{PlayContext, Playback, Repeat};
use super::queue::QueueItem;
use super::{Now, Player, Track};
use crate::prelude::*;

/// In a test scene that asked for it.
pub(crate) fn active() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("MAGOAGENT_SCENE").is_some() && std::env::var_os("MAGOAGENT_FAKE_SPOTIFY").is_some())
}

fn song(index: usize, title: &str, artist: &str, seconds: u64) -> QueueItem {
    QueueItem::new(&format!("spotify:track:cena{index}"), title, artist, "", Some(seconds * 1000)).unwrap_or_default()
}

/// What plays: a minute and a half into a song of three, from Liked Songs, on Soloist.
pub(super) fn now() -> Now {
    Now {
        track: Track::new("Mentira ou Verdade", "DJ Biel do Furduncinho, $Cristiano", "Mentira ou Verdade", "").unwrap_or_default(),
        playing: true,
        player: Player::Soloist,
        through_soloist: false,
        playback: Playback {
            track_uri: "spotify:track:cena0".into(),
            shuffle: Some(true),
            repeat: Some(Repeat::Off),
            volume: Some(0.6),
            position_ms: Some(84_000),
            duration_ms: Some(198_000),
            read_at_ms: now_ms(),
            context: Some(PlayContext { uri: "spotify:collection".into(), name: String::new() }),
            liked: Some(true),
            device: "Soloist".into(),
        },
    }
}

/// The page's lists.
pub(super) fn fill(library: &mut Library) {
    library.queue = vec![
        song(1, "Tá Vendo Aquela Lua", "Exaltasamba", 221),
        song(2, "Evidências", "Chitãozinho & Xororó", 278),
        song(3, "Deixa a Vida Me Levar", "Zeca Pagodinho", 245),
        song(4, "Trem-Bala", "Ana Vilela", 197),
    ];
    library.playlists = ["Sertanejo Raiz", "Pagode 90", "Foco"]
        .iter()
        .enumerate()
        .map(|(index, name)| PlaylistItem {
            uri: format!("spotify:playlist:cena{index}"),
            name: name.to_string(),
            owner: "Spotify".into(),
            cover_url: String::new(),
            total: Some(40 + 20 * index as u64),
        })
        .collect();
    library.liked = vec![song(5, "Mentira ou Verdade", "DJ Biel do Furduncinho", 198), song(6, "Evidências", "Chitãozinho & Xororó", 278)];
}
