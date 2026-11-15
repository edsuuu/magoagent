//! The island's sounds, made in code (nothing is copied from Coucou's own files): a short soft tone or chord for each
//! moment Coucou has one (core/sound.ts, `STATE_SOUND`): it peeks, opens, closes, a question or permission comes, one
//! is answered, something finishes or fails, and the clicks of its buttons (a hover makes none). Low in volume.

use crate::prelude::*;
use crate::voice::{Sound, partial};

pub(super) const PEEK: Sound = &[partial(1320.0, 1320.0, 0.0, 0.0, 0.004, 38.0, 0.05)];
pub(super) const OPEN: Sound = &[partial(523.0, 784.0, 0.12, 0.0, 0.01, 14.0, 0.09), partial(784.0, 1175.0, 0.12, 0.04, 0.01, 14.0, 0.05)];
pub(super) const CLOSE: Sound = &[partial(784.0, 523.0, 0.12, 0.0, 0.01, 16.0, 0.08)];
pub(super) const BLIP: Sound = &[partial(1760.0, 1760.0, 0.0, 0.0, 0.002, 60.0, 0.05)];
/// A question or a permission arrives: two notes, the second higher.
pub(super) const ASK: Sound = &[partial(659.0, 659.0, 0.0, 0.0, 0.01, 10.0, 0.1), partial(880.0, 880.0, 0.0, 0.11, 0.01, 9.0, 0.1)];
pub(super) const APPROVE: Sound = &[partial(880.0, 1320.0, 0.06, 0.0, 0.005, 22.0, 0.08)];
/// Everything finished: a rising arpeggio.
pub(super) const FINISH: Sound =
    &[partial(523.0, 523.0, 0.0, 0.0, 0.01, 7.0, 0.08), partial(659.0, 659.0, 0.0, 0.09, 0.01, 7.0, 0.08), partial(784.0, 784.0, 0.0, 0.18, 0.01, 6.0, 0.08)];

/// The entrance: a few bright notes rising, like sparkles.
pub(super) const GREET: Sound = &[
    partial(784.0, 784.0, 0.0, 0.0, 0.01, 8.0, 0.07),
    partial(988.0, 988.0, 0.0, 0.1, 0.01, 8.0, 0.07),
    partial(1319.0, 1319.0, 0.0, 0.2, 0.01, 7.0, 0.07),
    partial(1568.0, 1760.0, 0.1, 0.32, 0.01, 5.0, 0.06),
];
/// A failure: two soft notes, the second lower.
pub(super) const ERROR: Sound = &[partial(392.0, 392.0, 0.0, 0.0, 0.01, 9.0, 0.1), partial(262.0, 233.0, 0.05, 0.12, 0.01, 7.0, 0.1)];
/// A poke on the wizard: a short squeak up; dizzy from too many, it wobbles.
pub(super) const POKE: Sound = &[partial(700.0, 1100.0, 0.2, 0.0, 0.003, 45.0, 0.07)];
pub(super) const DIZZY: Sound = &[partial(900.0, 500.0, 0.5, 0.0, 0.01, 8.0, 0.08), partial(500.0, 800.0, 0.5, 0.14, 0.01, 8.0, 0.08)];

impl DrawingApp {
    /// Plays `sound` when the island's sounds are on (the tests stay quiet); the stream plays while kept.
    pub(super) fn island_chime(&mut self, sound: Sound, time: f64) {
        #[cfg(test)]
        {
            self.island.chimes += 1;
        }
        if cfg!(test) || !self.prefs.island_sound {
            return;
        }
        self.island.tones.retain(|(_, until)| *until > time);
        let until = time + crate::voice::sound_length(sound) as f64;
        if let Some(stream) = crate::voice::chime_at_volume(sound, self.prefs.island_volume) {
            self.island.tones.push((stream, until));
        }
    }
}
