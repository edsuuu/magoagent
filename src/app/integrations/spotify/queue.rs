//! A track in a list of the island's Spotify page (A seguir, Curtidas, what a search found), and what plays next as
//! Soloist says it (`soloist ctl queue <n> --json`, its `queue_changed`; the text of an older `ctl` too). With the account,
//! the Web API's queue is read in `web.rs`.

use super::Track;
use super::playback::uri_allowed;
#[cfg(any(not(any(windows, target_os = "macos")), test))]
use crate::prelude::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct QueueItem {
    /// "spotify:track:…" (empty when the player did not say: it can then only be shown).
    pub(crate) uri: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) cover_url: String,
    pub(crate) duration_ms: Option<u64>,
}

impl QueueItem {
    /// One line each, no control characters, cut (as `Track::new`); None without a title.
    pub(crate) fn new(uri: &str, title: &str, artist: &str, cover_url: &str, duration_ms: Option<u64>) -> Option<QueueItem> {
        let track = Track::new(title, artist, "", cover_url)?;
        let uri = if uri_allowed(uri.trim()) { uri.trim().to_string() } else { String::new() };

        Some(QueueItem { uri, title: track.title, artist: track.artist, cover_url: track.cover_url, duration_ms })
    }
}

/// The url of the cover of the `size` asked for (else the first) out of Soloist's `[{ url, size }]`.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn soloist_cover(covers: &Value, size: &str) -> String {
    let covers = covers.as_array();
    let cover = covers.and_then(|covers| covers.iter().find(|cover| cover["size"] == size).or(covers.first()));

    cover.and_then(|cover| cover["url"].as_str()).unwrap_or_default().to_string()
}

/// A track out of a Soloist entity (`{ uri, entity_type, decorations: { identity, creators, visual_identity, playback } }`).
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn soloist_entity(entity: &Value) -> Option<QueueItem> {
    let decorations = &entity["decorations"];
    let name = |entity: &Value| entity["decorations"]["identity"]["name"].as_str().unwrap_or_default().to_string();

    let artists: Vec<String> =
        decorations["creators"].as_array().into_iter().flatten().map(|creator| name(&creator["entity"])).filter(|name| !name.is_empty()).collect();

    let duration = decorations["playback"]["duration_ms"].as_f64().filter(|duration| duration.is_finite() && *duration > 0.0).map(|duration| duration as u64);

    QueueItem::new(
        entity["uri"].as_str().unwrap_or_default(),
        decorations["identity"]["name"].as_str()?,
        &artists.join(", "),
        &soloist_cover(&decorations["visual_identity"]["cover"], "small"),
        duration,
    )
}

/// What plays next out of `queue_changed` (`{ upcoming: [{ uid, source, item }] }`), the first first; at most `most`.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
pub(crate) fn from_soloist_queue(root: &Value, most: usize) -> Option<Vec<QueueItem>> {
    let upcoming = root.get("upcoming")?.as_array()?;

    Some(upcoming.iter().filter_map(|entry| soloist_entity(entry.get("item").unwrap_or(entry))).take(most).collect())
}

/// What an older `soloist ctl queue <n>` printed as text: a track a line, maybe numbered, maybe with its uri, the title
/// and the artists split by " — ", " – " or " - ". Lines of no track ("queue is empty", headers) are left out.
#[cfg(any(not(any(windows, target_os = "macos")), test))]
// ponytail: the text's layout is guessed (the reference doesn't show it); `--json` is asked first and this only reads
// what an older Soloist prints. Fix it against a real `ctl queue` when one is logged in.
pub(crate) fn from_soloist_queue_text(text: &str, most: usize) -> Vec<QueueItem> {
    let mut found = Vec::new();

    for line in text.lines() {
        let mut line = line.trim().to_string();

        if line.is_empty() || line.ends_with(':') || line.to_ascii_lowercase().contains("queue is empty") || line.to_ascii_lowercase().starts_with("error") {
            continue;
        }

        // the uri, wherever it is
        let uri = line.split(|letter: char| letter.is_whitespace() || matches!(letter, '(' | ')' | '[' | ']' | '<' | '>')).find(|word| uri_allowed(word));
        let uri = uri.map(str::to_string).unwrap_or_default();

        if !uri.is_empty() {
            line = line.replace(&uri, "").replace("()", "").replace("[]", "").replace("<>", "");
        }

        // a number first: "1.", "1)", "1 "
        let digits = line.chars().take_while(char::is_ascii_digit).count();

        if digits > 0 {
            line = line[digits..].trim_start_matches(['.', ')', ':']).to_string();
        }

        let line = line.trim().trim_start_matches(['-', '•', '*']).trim();
        let (title, artist) = [" — ", " – ", " - "].iter().find_map(|split| line.split_once(split)).unwrap_or((line, ""));

        if let Some(item) = QueueItem::new(&uri, title.trim(), artist.trim(), "", None) {
            found.push(item);
        }

        if found.len() == most {
            break;
        }
    }

    found
}
