//! From what is heard to the dance: the loudness over a few time spans, the chorus (the sound fuller than of late for a
//! few seconds), the drop (it jumps back after two seconds or more of quiet) and the choice, which changes at most every
//! four bars except to nap and to spin.

use super::*;

/// Below this (dB of full scale, over a second) it is silence; the song's mean loudness is over this long.
const SILENCE: f32 = -50.0;
const LONG: f32 = 25.0;
/// The energy is the loudness against the loudest of late (which comes down this many dB a second) over this range.
const PEAK_FALL: f32 = 0.25;
const ENERGY_RANGE: f32 = 18.0;
/// The chorus: the last second this many dB above the song's mean for this long; over when back near the mean for this
/// long.
const CHORUS_RISE: f32 = 2.0;
const CHORUS_FALL: f32 = 0.5;
const CHORUS_AFTER: f32 = 4.0;
const CHORUS_UNTIL: f32 = 2.0;
/// The drop: this many dB under the mean for this long, then back near it within this long.
const DROP_DIP: f32 = 6.0;
const DROP_QUIET: f32 = 2.0;
const DROP_BACK: f32 = 1.5;
const DROP_WITHIN: f32 = 0.6;
/// A dance stays at least four bars of four.
pub(crate) const DWELL: f64 = 16.0;
/// A strong bass with a lot of energy is the headbang.
const BASS_STRONG: f32 = 0.72;
const ENERGY_HIGH: f32 = 0.85;

/// `value` easing toward `target` over `span` seconds, `step` seconds later.
fn ease(value: f32, target: f32, span: f32, step: f32) -> f32 {
    value + (target - value) * (1.0 - (-step / span).exp())
}

/// The loudness (dB of full scale) over a tenth of a second (`fast`, the drop), a second (`short`, the chorus and the
/// silence) and the song (`long`, the mean of what is not silence: of all of it at first, of the last ~25 s after), and
/// the energy. The short spans average the power, so a hit and the quiet after it are as loud as they sound.
pub(crate) struct Loudness {
    powers: [f32; 3],
    /// Seconds of sound heard.
    pub(crate) heard_for: f32,
    pub(crate) fast: f32,
    pub(crate) short: f32,
    smooth: f32,
    pub(crate) long: f32,
    peak: f32,
    /// 0 to 1: the last third of a second against the loudest of late.
    pub(crate) energy: f32,
    heard: bool,
}

impl Default for Loudness {
    fn default() -> Self {
        Loudness { powers: [1e-12; 3], heard_for: 0.0, fast: -120.0, short: -120.0, smooth: -120.0, long: -120.0, peak: -120.0, energy: 0.0, heard: false }
    }
}

impl Loudness {
    /// A look of `decibels`, `step` seconds after the last.
    pub(crate) fn hear(&mut self, decibels: f32, step: f32) {
        let decibels = if decibels.is_finite() { decibels.clamp(-120.0, 20.0) } else { -120.0 };
        let power = 10f32.powf(decibels / 10.0);
        if !self.heard && decibels > SILENCE {
            (self.powers, self.long, self.peak, self.heard) = ([power; 3], decibels, decibels, true);
        }
        for (average, span) in self.powers.iter_mut().zip([0.1, 1.0, 0.3]) {
            *average = ease(*average, power, span, step);
        }
        let [fast, short, smooth] = self.powers.map(|average| 10.0 * average.max(1e-12).log10());
        (self.fast, self.short, self.smooth) = (fast, short, smooth);
        if !self.silent() {
            self.heard_for += step;
            self.long = ease(self.long, self.short, self.heard_for.clamp(1.0, LONG), step);
            self.peak = (self.peak - PEAK_FALL * step).max(self.smooth);
        }
        self.energy = if self.silent() { 0.0 } else { ((self.smooth - (self.peak - ENERGY_RANGE)) / ENERGY_RANGE).clamp(0.0, 1.0) };
    }

    pub(crate) fn silent(&self) -> bool {
        self.short < SILENCE
    }
}

/// Whether it is the chorus.
#[derive(Default)]
pub(crate) struct ChorusWatch {
    above: f32,
    below: f32,
    on: bool,
}

impl ChorusWatch {
    /// `short` and `long` loudness, `step` seconds after the last look.
    pub(crate) fn hear(&mut self, short: f32, long: f32, step: f32) -> bool {
        if short > long + CHORUS_RISE {
            (self.above, self.below) = (self.above + step, 0.0);
        } else if short < long + CHORUS_FALL {
            (self.above, self.below) = (0.0, self.below + step);
        }
        if !self.on && self.above >= CHORUS_AFTER {
            self.on = true;
        } else if self.on && self.below >= CHORUS_UNTIL {
            self.on = false;
        }
        self.on
    }
}

/// Finds the drops.
#[derive(Default)]
pub(crate) struct DropWatch {
    quiet: f32,
    left: f32,
}

impl DropWatch {
    /// `fast` and `long` loudness, `step` seconds after the last look; true on the look the drop comes.
    pub(crate) fn hear(&mut self, fast: f32, long: f32, step: f32) -> bool {
        if fast < long - DROP_DIP {
            (self.quiet, self.left) = (self.quiet + step, 0.0);
            return false;
        }
        if self.quiet >= DROP_QUIET {
            self.left = DROP_WITHIN;
        }
        self.quiet = 0.0;
        if self.left > 0.0 {
            self.left -= step;
            if fast >= long - DROP_BACK {
                self.left = 0.0;
                return true;
            }
        }
        false
    }
}

/// What the choice looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Cues {
    pub(crate) bpm: Option<f32>,
    pub(crate) energy: f32,
    pub(crate) bass: f32,
    pub(crate) chorus: bool,
    /// The drop came now.
    pub(crate) drop: bool,
    pub(crate) silent: bool,
}

/// The dance and the beat it began on.
#[derive(Default)]
pub(crate) struct Chooser {
    pub(crate) dance: Dance,
    pub(crate) since: f64,
    spin_until: f64,
    /// A tempo was heard: the sway of before it is left at once.
    heard: bool,
}

impl Chooser {
    /// The dance at `beat` for `cues`: silence naps and the drop spins at once (the spin, once, goes back to the dance
    /// wanted then); the first tempo heard and the music after silence are danced at once; else a new dance waits until
    /// the last one had four bars.
    pub(crate) fn choose(&mut self, cues: &Cues, beat: f64) -> Dance {
        let wanted = self.wanted(cues);
        let first = cues.bpm.is_some() && !self.heard;
        self.heard |= cues.bpm.is_some();
        if cues.silent {
            self.set(Dance::Nap, beat);
        } else if cues.drop && self.dance != Dance::Spin {
            // from the next beat, so its landing falls on one
            self.set(Dance::Spin, beat.ceil());
            self.spin_until = self.since + spin_beats(cues.bpm.unwrap_or(CLOCK_BPM));
        } else if self.dance == Dance::Spin {
            if beat >= self.spin_until {
                self.set(wanted, beat);
            }
        } else if self.dance == Dance::Nap || first || (wanted != self.dance && beat - self.since >= DWELL) {
            self.set(wanted, beat);
        }
        self.dance
    }

    fn set(&mut self, dance: Dance, beat: f64) {
        if dance != self.dance {
            (self.dance, self.since) = (dance, beat);
        }
    }

    /// The dance `cues` ask for: the chorus waves; fast, or a strong bass with energy, headbangs; else by the tempo, a
    /// tempo in two dances' ranges keeping the one danced.
    fn wanted(&self, cues: &Cues) -> Dance {
        if cues.chorus {
            return Dance::Wave;
        }
        let Some(bpm) = cues.bpm else { return Dance::Sway };
        if bpm > 135.0 || (cues.bass >= BASS_STRONG && cues.energy >= ENERGY_HIGH) {
            return Dance::Bang;
        }
        let ranges = [(Dance::Sway, 0.0, 90.0), (Dance::Step, 90.0, 110.0), (Dance::Clap, 100.0, 120.0), (Dance::Pump, 115.0, 135.0)];
        if ranges.iter().any(|&(dance, low, high)| dance == self.dance && (low..=high).contains(&bpm)) {
            return self.dance;
        }
        match bpm {
            bpm if bpm < 90.0 => Dance::Sway,
            bpm if bpm < 105.0 => Dance::Step,
            bpm if bpm < 117.5 => Dance::Clap,
            _ => Dance::Pump,
        }
    }
}
