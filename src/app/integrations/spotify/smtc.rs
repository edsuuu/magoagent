//! Spotify on Windows, by the system's media controls (the overlay the media keys show): Spotify publishes there what
//! it plays, and Windows hands that session to any app, with its cover, shuffle, repeat and the timeline, and
//! play/pause, next, previous, shuffle, repeat and a seek as requests. The volume is not there (the Web API does it with
//! the account), nor a queue. No account, no key and no network.

use super::{COVER_LIMIT, Command, Now, QueueItem, Repeat, Track, is_spotify_app};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as Properties, GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Media::MediaPlaybackAutoRepeatMode as RepeatMode;
use windows::Storage::Streams::DataReader;

/// A TimeSpan's ticks (100 ns) in a ms.
const TICKS_PER_MS: i64 = 10_000;

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
    let info = session.GetPlaybackInfo().ok();
    let playing = info.as_ref().and_then(|info| info.PlaybackStatus().ok()).is_some_and(|status| status == Status::Playing);
    let mut now = Now { track, playing, ..Default::default() };
    let playback = &mut now.playback;
    playback.device = "Spotify".into();
    if let Some(info) = &info {
        playback.shuffle = info.IsShuffleActive().and_then(|shuffle| shuffle.Value()).ok();
        playback.repeat = info.AutoRepeatMode().and_then(|mode| mode.Value()).ok().map(|mode| match mode {
            RepeatMode::Track => Repeat::Track,
            RepeatMode::List => Repeat::Context,
            _ => Repeat::Off,
        });
    }
    if let Ok(timeline) = session.GetTimelineProperties() {
        let ms = |span: windows::core::Result<windows::Foundation::TimeSpan>| span.ok().map(|span| span.Duration / TICKS_PER_MS).filter(|ms| *ms >= 0);
        let (start, end) = (ms(timeline.StartTime()).unwrap_or(0), ms(timeline.EndTime()));
        playback.duration_ms = end.map(|end| (end - start).max(0) as u64).filter(|duration| *duration > 0);
        playback.position_ms = ms(timeline.Position()).map(|position| (position - start).max(0) as u64);
        // ponytail: the position as of this poll, not as of the timeline's LastUpdatedTime (a FILETIME); a second off at
        // most, since it is asked every two
        playback.read_at_ms = if playback.position_ms.is_some() { crate::model::now_ms() } else { 0 };
    }
    Some(now)
}

/// True when Windows passed the request on and the player said yes.
pub(super) fn control(command: &Command) -> bool {
    let Some(session) = session() else { return false };
    let asked = match command {
        Command::PlayPause => session.TryTogglePlayPauseAsync(),
        Command::Next => session.TrySkipNextAsync(),
        Command::Previous => session.TrySkipPreviousAsync(),
        Command::Shuffle(on) => session.TryChangeShuffleActiveAsync(*on),
        Command::Repeat(repeat) => session.TryChangeAutoRepeatModeAsync(match repeat {
            Repeat::Off => RepeatMode::None,
            Repeat::Context => RepeatMode::List,
            Repeat::Track => RepeatMode::Track,
        }),
        Command::Seek(ms) => session.TryChangePlaybackPositionAsync((*ms as i64).saturating_mul(TICKS_PER_MS)),
        // the uri opens in the Spotify app (`spotify:` is its scheme): it shows it, and a track starts
        Command::PlayUri(uri) => {
            crate::app::browser::open_external(uri);
            return true;
        }
        Command::Volume(_) | Command::Enqueue(_) => return false,
    };
    asked.and_then(|asked| asked.join()).unwrap_or(false)
}

/// The session has no queue.
pub(super) fn queue(_most: usize) -> Option<Vec<QueueItem>> {
    None
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
