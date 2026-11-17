//! Spotify on Windows, by the system's media controls (the overlay the media keys show): Spotify publishes there what
//! it plays, and Windows hands that session to any app, with its cover, shuffle, repeat and the timeline, and
//! play/pause, next, previous, shuffle, repeat and a seek as requests. The volume is not there: it comes from the
//! Windows mixer, Spotify's own session there (as the system's volume slider shows it). Nor a queue. No account, no key
//! and no network.

use super::{COVER_LIMIT, Command, Now, QueueItem, Repeat, Track, is_spotify_app};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as Properties, GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Media::MediaPlaybackAutoRepeatMode as RepeatMode;
use windows::Storage::Streams::DataReader;
use windows::Win32::Foundation::{CloseHandle, RPC_E_CHANGED_MODE};
use windows::Win32::Media::Audio::{
    IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eMultimedia, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows::core::{Interface, PWSTR};

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
    playback.volume = volume();
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
        // the mixer has what the media session does not: Spotify's own volume
        Command::Volume(percent) => return set_volume(*percent),
        Command::Enqueue(_) => return false,
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

/// COM is for the thread, and this one (the watch's) lives while the integration is on: initialized once. S_FALSE
/// (already initialized) and another apartment (RPC_E_CHANGED_MODE) still leave it usable here.
fn com_ready() -> bool {
    thread_local! {
        static READY: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    }
    READY.with(|ready| match ready.get() {
        Some(ready) => ready,
        None => {
            let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            let usable = result.is_ok() || result == RPC_E_CHANGED_MODE;
            ready.set(Some(usable));
            usable
        }
    })
}

/// The program's file name behind `pid` ("Spotify.exe").
fn process_name(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut path = [0u16; 512];
    let mut length = path.len() as u32;
    let asked = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(path.as_mut_ptr()), &mut length) };
    let _ = unsafe { CloseHandle(process) };
    asked.ok()?;
    let path = String::from_utf16_lossy(&path[..(length as usize).min(path.len())]);
    Some(path.rsplit(['\\', '/']).next().unwrap_or_default().to_string())
}

/// The Windows mixer's sessions of Spotify: one for each of its windows that plays a sound, and there may be more than
/// one. Core Audio has no counterpart in the media session, so Spotify's own volume comes from here.
fn sessions() -> Option<Vec<ISimpleAudioVolume>> {
    if !com_ready() {
        return None;
    }
    // SAFETY: COM is up on this thread (com_ready), and each call's answer is checked before the next one
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None).ok()?;
        let list = manager.GetSessionEnumerator().ok()?;
        let mut found = Vec::new();
        for index in 0..list.GetCount().ok()? {
            let Ok(control) = list.GetSession(index) else { continue };
            let Ok(control) = control.cast::<IAudioSessionControl2>() else { continue };
            let spotify = control.GetProcessId().ok().and_then(process_name).is_some_and(|name| name.eq_ignore_ascii_case("Spotify.exe"));
            if spotify && let Ok(volume) = control.cast::<ISimpleAudioVolume>() {
                found.push(volume);
            }
        }
        Some(found)
    }
}

/// Spotify's volume in the mixer, 0 to 1: the first of its sessions (the one the mixer's slider shows); None when it
/// has no session (it plays no sound now).
fn volume() -> Option<f32> {
    let level = unsafe { sessions()?.first()?.GetMasterVolume() }.ok()?;
    Some(level.clamp(0.0, 1.0))
}

/// Turns every one of Spotify's sessions in the mixer to `percent`; false without a session of it, or when none of them
/// took it.
fn set_volume(percent: u8) -> bool {
    let level = f32::from(percent.min(100)) / 100.0;
    let Some(sessions) = sessions().filter(|sessions| !sessions.is_empty()) else { return false };
    let mut turned = false;
    for session in sessions {
        turned |= unsafe { session.SetMasterVolume(level, std::ptr::null()) }.is_ok();
    }
    turned
}
