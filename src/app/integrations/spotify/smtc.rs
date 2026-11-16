//! Spotify on Windows, by the system's media controls (the overlay the media keys show): Spotify publishes there what
//! it plays, and Windows hands that session to any app, with its cover, and play/pause, next and previous as requests.
//! No account, no key and no network.

use super::{COVER_LIMIT, Command, Now, Track, is_spotify_app};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as Properties, GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;

/// Spotify's session among the ones the system knows.
fn session() -> Option<Session> {
    let sessions = Manager::RequestAsync().ok()?.join().ok()?.GetSessions().ok()?;
    (0..sessions.Size().ok()?)
        .filter_map(|index| sessions.GetAt(index).ok())
        .find(|session| session.SourceAppUserModelId().is_ok_and(|id| is_spotify_app(&id.to_string_lossy())))
}

fn track_of(properties: &Properties) -> Option<Track> {
    let text = |value: windows::core::Result<windows::core::HSTRING>| value.map(|text| text.to_string_lossy()).unwrap_or_default();
    Track::new(&text(properties.Title()), &text(properties.Artist()), &text(properties.AlbumTitle()), "")
}

pub(super) fn read() -> Option<Now> {
    let session = session()?;
    let track = track_of(&session.TryGetMediaPropertiesAsync().ok()?.join().ok()?)?;
    let playing = session.GetPlaybackInfo().and_then(|info| info.PlaybackStatus()).is_ok_and(|status| status == Status::Playing);
    Some(Now { track, playing, ..Default::default() })
}

/// True when Windows passed the request on and the player said yes.
pub(super) fn control(command: Command) -> bool {
    let Some(session) = session() else { return false };
    let asked = match command {
        Command::PlayPause => session.TryTogglePlayPauseAsync(),
        Command::Next => session.TrySkipNextAsync(),
        Command::Previous => session.TrySkipPreviousAsync(),
    };
    asked.and_then(|asked| asked.join()).unwrap_or(false)
}

/// The cover of `track` as the session hands it (a JPEG or a PNG). None while Spotify has not published it yet (it
/// comes a moment after the title), or when another track plays by now.
pub(super) fn cover(track: &Track) -> Option<Vec<u8>> {
    let properties = session()?.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    if track_of(&properties).as_ref() != Some(track) {
        return None;
    }
    let stream = properties.Thumbnail().ok()?.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > COVER_LIMIT as u64 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    let loaded = reader.LoadAsync(size as u32).ok()?.join().ok()?;
    let mut bytes = vec![0u8; loaded as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    Some(bytes)
}
