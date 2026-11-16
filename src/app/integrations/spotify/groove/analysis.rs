//! Hearing the beat. The sound, 11025 samples a second in one channel, is looked at every 128 samples (~86 looks a
//! second) through a window of 512: how much each frequency grew since the last look (the spectral flux, high at each
//! hit) is the onset envelope; its autocorrelation over the last 8 s gives the tempo (60 to 180 BPM, the octave chosen
//! by a prior around 120 and by the multiples of the period), and a comb over it where the beats fall (the phase), which
//! steers a beat clock. The window's loudness and its share under 150 Hz give the energy and the bass (`choose`).

use super::*;
use std::collections::VecDeque;

/// The rate the sound is read at, the step between two looks and the window of each look.
pub(crate) const RATE: u32 = 11025;
pub(crate) const HOP: usize = 128;
const WINDOW: usize = 512;
pub(crate) const FRAMES_PER_SECOND: f64 = RATE as f64 / HOP as f64;
/// The envelope kept (8 s); the tempo is looked for every half a second once 3 s were heard, the phase every ~0.13 s.
const HISTORY: usize = 690;
const TEMPO_EVERY: u64 = 43;
const TEMPO_AFTER: usize = 258;
const PHASE_EVERY: u64 = 11;
const SLOWEST: f64 = 60.0;
const FASTEST: f64 = 180.0;
/// Between two tempos as likely, the one nearer 120 BPM (an octave either way weighs 0.6): a click every 0.4 s is 150,
/// not 75.
const PRIOR_CENTER: f64 = 120.0;
const PRIOR_OCTAVES: f64 = 1.0;
/// The tempo found is kept over another (its double or half, or a new one) unless that one is this much stronger.
const KEEP: f32 = 0.85;
/// Under this clarity (the autocorrelation at the period) there is no beat to follow.
const CLEAR: f32 = 0.05;
/// A hit peaks in the flux a couple of looks after it starts (on its way to the window's middle): the beat is put this
/// many looks earlier.
const ONSET_DELAY: f64 = 2.0;
/// How much of the phase measured each time the beat clock takes (it would jump at every misheard hit otherwise).
const PULL: f64 = 0.3;
/// The bass: what is under this, against the whole.
const BASS_UNDER: f32 = 150.0;
/// The magnitudes are compressed by log(1 + this × magnitude), so the soft hits count too.
const COMPRESSION: f32 = 1000.0;

/// A radix-2 FFT of `WINDOW` points with a Hann window.
struct Spectrum {
    window: Vec<f32>,
    cos: Vec<f32>,
    sin: Vec<f32>,
    reversed: Vec<usize>,
    real: Vec<f32>,
    imaginary: Vec<f32>,
}

impl Spectrum {
    fn new() -> Spectrum {
        let size = WINDOW;
        let bits = size.trailing_zeros();
        let turn = |index: usize| index as f32 / size as f32 * std::f32::consts::TAU;
        Spectrum {
            window: (0..size).map(|index| 0.5 - 0.5 * turn(index).cos()).collect(),
            cos: (0..size / 2).map(|index| turn(index).cos()).collect(),
            sin: (0..size / 2).map(|index| turn(index).sin()).collect(),
            reversed: (0..size).map(|index| index.reverse_bits() >> (usize::BITS - bits)).collect(),
            real: vec![0.0; size],
            imaginary: vec![0.0; size],
        }
    }

    /// The magnitudes of `samples` (`WINDOW` of them), from 0 Hz to half the rate, into `out`: a full-scale sine is ~1.
    fn magnitudes(&mut self, samples: impl Iterator<Item = f32>, out: &mut [f32]) {
        let size = WINDOW;
        for (index, sample) in samples.take(size).enumerate() {
            self.real[self.reversed[index]] = sample * self.window[index];
            self.imaginary[self.reversed[index]] = 0.0;
        }
        let mut span = 2;
        while span <= size {
            let (half, stride) = (span / 2, size / span);
            for start in (0..size).step_by(span) {
                for offset in 0..half {
                    let (cos, sin) = (self.cos[offset * stride], self.sin[offset * stride]);
                    let (near, far) = (start + offset, start + offset + half);
                    let (real, imaginary) = (self.real[far], self.imaginary[far]);
                    let (turned_real, turned_imaginary) = (real * cos + imaginary * sin, imaginary * cos - real * sin);
                    self.real[far] = self.real[near] - turned_real;
                    self.imaginary[far] = self.imaginary[near] - turned_imaginary;
                    self.real[near] += turned_real;
                    self.imaginary[near] += turned_imaginary;
                }
            }
            span *= 2;
        }
        let scale = 4.0 / size as f32;
        for (bin, magnitude) in out.iter_mut().enumerate().take(size / 2 + 1) {
            *magnitude = self.real[bin].hypot(self.imaginary[bin]) * scale;
        }
    }
}

/// The tempo in `envelope` (looks `frames_per_second` apart), 60 to 180 BPM, and how clear it is (the autocorrelation at
/// its period, up to 1). Each period is scored by its autocorrelation and its double's, weighed by how near 120 it is;
/// `current` is kept unless another is clearly stronger. The period is then read finer at its multiples.
pub(crate) fn tempo(envelope: &[f32], frames_per_second: f64, current: Option<f64>) -> Option<(f64, f32)> {
    let count = envelope.len();
    // smoothed over five looks, so a period between two whole looks makes one wide peak, not two halves
    let smoothed: Vec<f32> = (0..count)
        .map(|index| {
            let near = |offset: usize, weight: f32| envelope.get((index + offset).wrapping_sub(2)).map_or(0.0, |value| value * weight);
            near(0, 1.0) + near(1, 2.0) + near(2, 3.0) + near(3, 2.0) + near(4, 1.0)
        })
        .collect();
    let mean = smoothed.iter().sum::<f32>() / count.max(1) as f32;
    let centered: Vec<f32> = smoothed.iter().map(|value| value - mean).collect();
    let longest = (frames_per_second * 60.0 / SLOWEST).ceil() as usize + 1;
    let shortest = ((frames_per_second * 60.0 / FASTEST).floor() as usize).max(2);
    let reach = (longest * 4).min(count / 2);
    if reach < longest + 1 {
        return None;
    }
    let mut correlation: Vec<f32> =
        (0..=reach).map(|lag| centered[lag..].iter().zip(&centered).map(|(later, earlier)| later * earlier).sum::<f32>() / (count - lag) as f32).collect();
    let energy = correlation[0];
    if energy <= 1e-12 || !energy.is_finite() {
        return None;
    }
    for value in &mut correlation {
        *value /= energy;
    }
    let at = |lag: usize| correlation.get(lag).copied().unwrap_or(0.0);
    let prior = |lag: f64| {
        let octaves = (frames_per_second * 60.0 / lag / PRIOR_CENTER).log2() / PRIOR_OCTAVES;
        (-0.5 * octaves * octaves).exp() as f32
    };
    let score = |lag: usize| (at(lag) + 0.5 * at(2 * lag - 1).max(at(2 * lag)).max(at(2 * lag + 1))) * prior(lag as f64);
    let peak = |lag: usize| at(lag) >= at(lag - 1) && at(lag) >= at(lag + 1);
    let mut best = (shortest..=longest).filter(|&lag| peak(lag)).max_by(|a, b| score(*a).total_cmp(&score(*b)))?;
    if let Some(current) = current {
        let lag = frames_per_second * 60.0 / current;
        let near = (lag.round() as usize).saturating_sub(1).max(shortest)..=(lag.round() as usize + 1).min(longest);
        if let Some(kept) = near.clone().max_by(|a, b| at(*a).total_cmp(&at(*b)))
            && near.map(score).fold(0.0, f32::max) >= KEEP * score(best)
        {
            best = kept;
        }
    }
    let clarity = at(best);
    if clarity < CLEAR {
        return None;
    }
    // finer: the peak between the lags, here and at its multiples, where an error is spread over k periods (the k-th
    // gives the period as its place / k, weighed by k: its place itself)
    let vertex = |lag: usize| {
        let (before, here, after) = (at(lag - 1), at(lag), at(lag + 1));
        let bend = before - 2.0 * here + after;
        lag as f64 + if bend < 0.0 { (0.5 * (before - after) / bend).clamp(-0.5, 0.5) as f64 } else { 0.0 }
    };
    let base = vertex(best);
    let (mut sum, mut weights) = (base, 1.0);
    for multiple in 2..=4 {
        let center = (base * multiple as f64).round() as usize;
        if center + 3 > reach {
            break;
        }
        let found = (center - 2..=center + 2).max_by(|a, b| at(*a).total_cmp(&at(*b))).unwrap_or(center);
        sum += vertex(found);
        weights += multiple as f64;
    }
    let lag = sum / weights;
    Some((frames_per_second * 60.0 / lag, clarity))
}

/// How many looks ago the last beat fell in `envelope`, the beats `period` looks apart: the offset where a comb of teeth
/// that far apart (the recent ones counting more) gathers the most onsets.
pub(crate) fn last_beat(envelope: &[f32], period: f64) -> f64 {
    let count = envelope.len();
    if count < 2 || period < 2.0 {
        return 0.0;
    }
    let mean = envelope.iter().sum::<f32>() / count as f32;
    let onset = |at: f64| {
        let index = at.floor() as usize;
        let part = (at - at.floor()) as f32;
        let value = |index: usize| (envelope[index.min(count - 1)] - mean).max(0.0);
        value(index) * (1.0 - part) + value(index + 1) * part
    };
    let gather = |offset: f64| {
        (0..16)
            .map(|tooth| (count - 1) as f64 - offset - tooth as f64 * period)
            .take_while(|at| *at >= 0.0)
            .enumerate()
            .map(|(tooth, at)| 0.8f32.powi(tooth as i32) * onset(at))
            .sum::<f32>()
    };
    let steps = period.ceil() as usize;
    let scores: Vec<f32> = (0..steps).map(|step| gather(step as f64)).collect();
    let Some(best) = (0..steps).max_by(|a, b| scores[*a].total_cmp(&scores[*b])) else { return 0.0 };
    let (before, after) = (scores[(best + steps - 1) % steps], scores[(best + 1) % steps]);
    let bend = before - 2.0 * scores[best] + after;
    let finer = if bend < 0.0 { (0.5 * (before - after) / bend).clamp(-0.5, 0.5) as f64 } else { 0.0 };
    (best as f64 + finer + ONSET_DELAY).rem_euclid(period)
}

/// The listening thread's ear: samples in, the groove out.
pub(crate) struct Analyzer {
    spectrum: Spectrum,
    window: VecDeque<f32>,
    fresh: usize,
    magnitudes: Vec<f32>,
    compressed: Vec<f32>,
    envelope: VecDeque<f32>,
    frames: u64,
    bpm: Option<f64>,
    /// A different tempo heard once, waiting to be heard again before it is taken.
    candidate: Option<f64>,
    beat: f64,
    locked: bool,
    loudness: Loudness,
    bass: f32,
    chorus: ChorusWatch,
    drops: DropWatch,
    chooser: Chooser,
}

impl Default for Analyzer {
    fn default() -> Self {
        Analyzer {
            spectrum: Spectrum::new(),
            window: VecDeque::from(vec![0.0; WINDOW]),
            fresh: 0,
            magnitudes: vec![0.0; WINDOW / 2 + 1],
            compressed: vec![0.0; WINDOW / 2 + 1],
            envelope: VecDeque::with_capacity(HISTORY + 1),
            frames: 0,
            bpm: None,
            candidate: None,
            beat: 0.0,
            locked: false,
            loudness: Loudness::default(),
            bass: 0.0,
            chorus: ChorusWatch::default(),
            drops: DropWatch::default(),
            chooser: Chooser::default(),
        }
    }
}

impl Analyzer {
    /// Samples at `RATE`, one channel, -1 to 1.
    pub(crate) fn feed(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.window.pop_front();
            self.window.push_back(if sample.is_finite() { sample.clamp(-1.0, 1.0) } else { 0.0 });
            self.fresh += 1;
            if self.fresh == HOP {
                self.fresh = 0;
                self.look();
            }
        }
    }

    /// The tempo heard, if any.
    #[cfg(test)]
    pub(crate) fn bpm(&self) -> Option<f64> {
        self.bpm
    }

    /// What was heard so far, as of the last look.
    pub(crate) fn groove(&self) -> Groove {
        let bpm = self.bpm.map_or(CLOCK_BPM, |bpm| bpm as f32);
        Groove {
            bpm,
            phase: self.beat.rem_euclid(1.0) as f32,
            energy: self.loudness.energy,
            bass: self.bass,
            dance: self.chooser.dance,
            beat: self.beat,
            since: self.chooser.since,
        }
    }

    fn look(&mut self) {
        self.frames += 1;
        let step = (1.0 / FRAMES_PER_SECOND) as f32;
        self.spectrum.magnitudes(self.window.iter().copied(), &mut self.magnitudes);
        let mut flux = 0.0;
        for (magnitude, before) in self.magnitudes.iter().zip(self.compressed.iter_mut()).skip(1) {
            let now = (1.0 + COMPRESSION * magnitude).ln();
            flux += (now - *before).max(0.0);
            *before = now;
        }
        if self.envelope.len() == HISTORY {
            self.envelope.pop_front();
        }
        self.envelope.push_back(flux);

        let newest = self.window.range(WINDOW - HOP..);
        let rms = (newest.map(|sample| sample * sample).sum::<f32>() / HOP as f32).sqrt();
        self.loudness.hear(20.0 * rms.max(1e-6).log10(), step);
        let low_bins = (BASS_UNDER / (RATE as f32 / WINDOW as f32)) as usize;
        let power = |range: std::ops::RangeInclusive<usize>| self.magnitudes[range].iter().map(|magnitude| magnitude * magnitude).sum::<f32>();
        let total = power(1..=WINDOW / 2);
        if total > 1e-9 {
            self.bass += (power(1..=low_bins) / total - self.bass) * (1.0 - (-step / 0.5).exp());
        }

        if self.frames.is_multiple_of(TEMPO_EVERY) && self.envelope.len() >= TEMPO_AFTER {
            self.listen_for_tempo();
        }
        let bpm = self.bpm.unwrap_or(f64::from(CLOCK_BPM));
        self.beat += bpm / 60.0 / FRAMES_PER_SECOND;
        if let Some(bpm) = self.bpm
            && self.frames.is_multiple_of(PHASE_EVERY)
        {
            let period = FRAMES_PER_SECOND * 60.0 / bpm;
            let measured = last_beat(self.envelope.make_contiguous(), period) / period;
            let drift = (measured - self.beat.rem_euclid(1.0) + 0.5).rem_euclid(1.0) - 0.5;
            self.beat += if self.locked { drift * PULL } else { drift };
            self.locked = true;
        }

        // the song's mean needs a while to mean something: no drop in its first 4 s, no chorus in its first 8
        let loudness = &self.loudness;
        let chorus = self.chorus.hear(loudness.short, loudness.long, step) && loudness.heard_for > 8.0;
        let drop = self.drops.hear(loudness.fast, loudness.long, step) && loudness.heard_for > 4.0;
        let cues = Cues { bpm: self.bpm.map(|bpm| bpm as f32), energy: loudness.energy, bass: self.bass, chorus, drop, silent: loudness.silent() };
        self.chooser.choose(&cues, self.beat);
    }

    fn listen_for_tempo(&mut self) {
        if self.loudness.silent() {
            return;
        }
        let Some((found, _)) = tempo(self.envelope.make_contiguous(), FRAMES_PER_SECOND, self.bpm) else { return };
        let near = |one: f64, other: f64| (one - other).abs() / other < 0.04;
        self.bpm = match (self.bpm, self.candidate) {
            (None, _) => Some(found),
            (Some(bpm), _) if near(found, bpm) => Some(bpm + (found - bpm) * 0.3),
            // a new tempo is taken when it is heard twice in a row
            (Some(_), Some(candidate)) if near(found, candidate) => {
                self.candidate = None;
                Some(found)
            }
            (Some(bpm), _) => {
                self.candidate = Some(found);
                Some(bpm)
            }
        };
    }
}
