//! Luna dancing to what plays: the beat (BPM), the loudness and the bass are heard in the computer's own sound, since
//! Spotify closed the tempo of its tracks to new apps. While something plays and the island draws a dancer
//! (`DrawingApp::luna_groove`, each frame), a thread listens to the sound going to the speakers (`capture`: PipeWire or
//! PulseAudio on Linux, WASAPI's loopback on Windows; macOS has none and she sways by the clock), finds the beat in it
//! (`analysis`) and picks the dance (`choose`). Nothing is recorded or sent: the sound is read in small pieces, measured
//! and dropped. Two seconds without a dancer on screen, or the music paused, and the listening ends.

mod analysis;
mod capture;
mod choose;
#[cfg(test)]
mod tests;

pub(crate) use analysis::Analyzer;
pub(crate) use choose::{Chooser, ChorusWatch, Cues, DropWatch, Loudness};

use crate::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The tempo of the clock when no beat is heard (macOS, before the first one): two beats are one sway of the
/// preview's 1.58 s. Asleep, two beats are its breath of 3 s.
pub(crate) const CLOCK_BPM: f32 = 76.0;
const NAP_BPM: f32 = 40.0;
/// Without a dancer on screen for this long, the listening ends; once it failed (no pw-record nor parec, no device), it
/// is not tried again for a while.
const LINGER: Duration = Duration::from_secs(2);
const RETRY: Duration = Duration::from_secs(30);
/// Listening, but nothing heard for this long: the sound is not here (the speakers are quiet), she naps.
const STALE: Duration = Duration::from_millis(1500);
/// What is heard reaches the speakers a little later, and the frame a little after that: the beat is drawn this early.
const LEAD: f64 = 0.03;

/// Luna's dances, as the preview names them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Dance {
    /// Balanço: slow, up to 90 BPM.
    #[default]
    Sway,
    /// Passinho: 90 to 110.
    Step,
    /// Palmas: 100 to 120, clapping above the head.
    Clap,
    /// Mãos pro alto: 115 to 135.
    Pump,
    /// Onda: the chorus, the sound fuller for a while.
    Wave,
    /// Cabeçada: above 135, or a strong bass with a lot of energy.
    Bang,
    /// Giro: the drop, once.
    Spin,
    /// Cochila: paused or silent.
    Nap,
}

impl Dance {
    #[cfg(test)]
    pub(crate) const ALL: [Dance; 8] = [Dance::Sway, Dance::Step, Dance::Clap, Dance::Pump, Dance::Wave, Dance::Bang, Dance::Spin, Dance::Nap];

    /// Only these four have arms: two short tubes floating beside the body.
    pub(crate) fn arms(self) -> bool {
        matches!(self, Dance::Clap | Dance::Pump | Dance::Wave | Dance::Spin)
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Dance::Sway => "Balanço",
            Dance::Step => "Passinho",
            Dance::Clap => "Palmas",
            Dance::Pump => "Mãos pro alto",
            Dance::Wave => "Onda",
            Dance::Bang => "Cabeçada",
            Dance::Spin => "Giro",
            Dance::Nap => "Cochila",
        }
    }
}

/// How many frequency bands the sound is cut in (the island's bars).
pub(crate) const BANDS: usize = 10;

/// What Luna dances to, now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Groove {
    /// The tempo heard (the clock's when none is).
    pub(crate) bpm: f32,
    /// Where in the beat it is: 0 on the beat, toward 1 just before the next.
    pub(crate) phase: f32,
    /// The loudness, 0 to 1, against the song's own loud parts of late.
    pub(crate) energy: f32,
    /// How much of the sound is bass (under ~150 Hz), 0 to 1.
    pub(crate) bass: f32,
    /// The sound by frequency, low to high (`analysis::BAND_EDGES`), each 0 to 1 against its own loud parts of late: the
    /// island's bars. None when nothing is heard (the clock's sway, the nap, macOS).
    pub(crate) bands: Option<[f32; BANDS]>,
    pub(crate) dance: Dance,
    /// The beats counted so far (`phase` is its fraction), and the beat the dance began on: the moves longer than a beat
    /// and the spin, which happens once, are timed by them.
    pub(crate) beat: f64,
    pub(crate) since: f64,
}

impl Groove {
    /// Timed by the clock at `seconds`, at `bpm`.
    pub(crate) fn clock(seconds: f64, bpm: f32, dance: Dance) -> Groove {
        let beat = seconds * f64::from(bpm) / 60.0;
        Groove { bpm, phase: beat.fract() as f32, energy: 0.5, bass: 0.0, bands: None, dance, beat, since: 0.0 }
    }

    /// The same, `seconds` later.
    pub(crate) fn later(self, seconds: f64) -> Groove {
        let beat = self.beat + seconds.max(0.0) * f64::from(self.bpm) / 60.0;
        Groove { beat, phase: beat.fract() as f32, ..self }
    }
}

/// How many beats a move of about `seconds` takes at `bpm`: the nearest power of two (1 to 8), so it starts on a beat.
pub(crate) fn cycle_beats(seconds: f32, bpm: f32) -> f64 {
    let beats = f64::from(seconds * bpm.max(1.0) / 60.0);
    beats.log2().round().exp2().clamp(1.0, 8.0)
}

/// The spin's beats: about its 2.4 s, four or eight, so its landing (at three quarters of it) falls on a beat.
pub(crate) fn spin_beats(bpm: f32) -> f64 {
    cycle_beats(2.4, bpm).clamp(4.0, 8.0)
}

/// Seconds since the app started: the clock of the demand and of the sway with no beat.
fn clock() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// The milliseconds of `clock` (plus one: zero is never).
fn stamp() -> u64 {
    (clock() * 1000.0) as u64 + 1
}

/// Between the app and the listening thread.
pub(crate) struct Shared {
    /// When a dancer was last drawn (`stamp`); the thread ends by itself when it gets old, even if the app stops ticking.
    asked: Arc<AtomicU64>,
    stop: AtomicBool,
    ended: AtomicBool,
    /// It ended without hearing anything: no program to listen with, or no device.
    failed: AtomicBool,
    /// The song changed: the thread hears the next one from scratch (its loudness and tempo), not measured against the
    /// last one (asked: "quando troca de música a frequência fica bugada e depois de uns 20 segundos volta fluido").
    pub(crate) new_song: AtomicBool,
    latest: Mutex<Option<(Groove, Instant)>>,
}

impl Shared {
    fn new(asked: Arc<AtomicU64>) -> Shared {
        Shared {
            asked,
            stop: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            new_song: AtomicBool::new(false),
            latest: Mutex::new(None),
        }
    }

    /// Whether the thread should go on: not stopped, and a dancer drawn within `LINGER`.
    fn wanted(&self) -> bool {
        let asked = self.asked.load(Ordering::Relaxed);
        !self.stop.load(Ordering::Relaxed) && asked != 0 && stamp().saturating_sub(asked) <= LINGER.as_millis() as u64
    }

    fn publish(&self, groove: Groove) {
        if let Ok(mut latest) = self.latest.lock() {
            *latest = Some((groove, Instant::now()));
        }
    }

    fn end(&self, failed: bool) {
        self.failed.store(failed, Ordering::Relaxed);
        self.ended.store(true, Ordering::Relaxed);
    }
}

/// The app's side of the listening: the demand, the thread while it runs and what it found.
#[derive(Default)]
pub(crate) struct Dancer {
    asked: Arc<AtomicU64>,
    listening: Option<Arc<Shared>>,
    failed_at: Option<Instant>,
    /// How many times the listening started (tests: they have no thread).
    #[cfg(test)]
    pub(crate) starts: u32,
}

impl Drop for Dancer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Whether to listen: the switch on, something playing, a dancer drawn within `LINGER` (`asked`: that long ago).
pub(crate) fn should_listen(on: bool, playing: bool, asked: Option<Duration>) -> bool {
    on && playing && asked.is_some_and(|ago| ago <= LINGER) && !cfg!(target_os = "macos")
}

impl Dancer {
    /// How long ago a dancer was drawn.
    fn asked_ago(&self) -> Option<Duration> {
        let asked = self.asked.load(Ordering::Relaxed);
        (asked != 0).then(|| Duration::from_millis(stamp().saturating_sub(asked)))
    }

    /// A dancer is on screen now (the island, each frame it draws one).
    pub(crate) fn ask(&self) {
        self.asked.store(stamp(), Ordering::Relaxed);
    }

    /// Another song started: the listening forgets the last one's loudness and tempo.
    pub(crate) fn new_song(&self) {
        if let Some(shared) = &self.listening {
            shared.new_song.store(true, Ordering::Relaxed);
        }
    }

    /// Whether the thread is listening.
    pub(crate) fn listening(&self) -> bool {
        self.listening.as_ref().is_some_and(|shared| !shared.ended.load(Ordering::Relaxed))
    }

    fn stop(&mut self) {
        if let Some(shared) = self.listening.take() {
            shared.stop.store(true, Ordering::Relaxed);
        }
    }

    /// Each frame: listening starts when it should (`should_listen`) and ends when it should not.
    pub(crate) fn tick(&mut self, on: bool, playing: bool) {
        if !should_listen(on, playing, self.asked_ago()) {
            self.stop();
            return;
        }
        if let Some(shared) = &self.listening {
            if !shared.ended.load(Ordering::Relaxed) {
                return;
            }
            if shared.failed.load(Ordering::Relaxed) {
                self.failed_at = Some(Instant::now());
            }
            self.listening = None;
        }
        if self.failed_at.is_some_and(|at| at.elapsed() < RETRY) {
            return;
        }
        let shared = Arc::new(Shared::new(self.asked.clone()));
        #[cfg(test)]
        {
            self.starts += 1;
        }
        // never the real sound in a test
        if !cfg!(test) {
            let thread_shared = shared.clone();
            let spawned = std::thread::Builder::new().name("luna-batida".into()).spawn(move || capture::listen(thread_shared));
            if spawned.is_err() {
                self.failed_at = Some(Instant::now());
                return;
            }
        }
        self.listening = Some(shared);
    }

    /// What Luna dances to now, `playing` or not, asking for the listening (`ask`).
    pub(crate) fn groove(&self, playing: bool) -> Groove {
        self.ask();
        self.peek(playing)
    }

    /// The same without asking: paused, she naps; listening, what was heard carried to now (asleep when the speakers
    /// went quiet); else the clock's sway.
    pub(crate) fn peek(&self, playing: bool) -> Groove {
        if !playing {
            return Groove::clock(clock(), NAP_BPM, Dance::Nap);
        }
        let heard =
            self.listening.as_ref().filter(|shared| !shared.ended.load(Ordering::Relaxed)).map(|shared| shared.latest.lock().ok().and_then(|latest| *latest));
        match heard {
            Some(Some((groove, at))) if at.elapsed() < STALE && groove.dance != Dance::Nap => groove.later(at.elapsed().as_secs_f64() + LEAD),
            Some(Some(_)) => Groove::clock(clock(), NAP_BPM, Dance::Nap),
            _ => Groove::clock(clock(), CLOCK_BPM, Dance::Sway),
        }
    }
}

impl DrawingApp {
    /// What Luna dances to, for the island's dancer; each call also asks for the listening, so the island calls it every
    /// frame it draws one. None with "A Luna dança com a música" off, or with nothing loaded in Spotify.
    // the island calls it where it draws her; until it does, only the tests
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn luna_groove(&self) -> Option<Groove> {
        let now = self.integrations.spotify.now.as_ref()?;
        self.prefs.integrations.luna_dances.then(|| self.integrations.spotify.dancer.groove(now.playing))
    }

    /// Luna's colour while she dances: the cover's (`Spotify::tint`, asked: "a Luna trocar de cor de acordo com a capa
    /// da música"), `usual` until the cover comes.
    pub(crate) fn luna_colour(&self, usual: Color32) -> Color32 {
        self.integrations.spotify.tint.unwrap_or(usual)
    }

    /// "A Luna dança com a música" in words, for the MCP.
    pub(crate) fn luna_report(&self) -> String {
        let spotify = &self.integrations.spotify;
        let state = if self.prefs.integrations.luna_dances { "ligado" } else { "desligado" };
        let how = if cfg!(target_os = "macos") { " (no macOS ela balança no ritmo do relógio: o app não ouve o som do sistema)" } else { "" };
        let mut report = format!("A Luna dança com a música: {state}{how}.");
        if self.prefs.integrations.luna_dances
            && let Some(now) = &spotify.now
        {
            let groove = spotify.dancer.peek(now.playing);
            if spotify.dancer.listening() && groove.dance != Dance::Nap {
                report.push_str(&format!(" Agora: {}, a {:.0} BPM.", groove.dance.name(), groove.bpm));
            } else {
                report.push_str(&format!(" Agora: {}.", groove.dance.name()));
            }
        }
        report
    }
}
