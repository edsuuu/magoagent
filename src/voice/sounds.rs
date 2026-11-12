use super::*;

/// A sine of a call sound: from `hz` gliding to `to` over `glide` s, starting `at` s in, swelling
/// for `attack` s, then fading by exp(-decay·t).
pub(crate) struct Partial {
    pub(super) hz: f32,
    pub(super) to: f32,
    pub(super) glide: f32,
    pub(super) at: f32,
    pub(super) attack: f32,
    pub(super) decay: f32,
    pub(super) gain: f32,
}

pub(super) const fn partial(hz: f32, to: f32, glide: f32, at: f32, attack: f32, decay: f32, gain: f32) -> Partial {
    Partial { hz, to, glide, at, attack, decay, gain }
}

pub(crate) type Sound = &'static [Partial];

/// The call's sounds, shaped as the voice-chat ones measured: a low swell rising a fifth when it
/// connects, a "dum" when it ends, two notes on an error, quiet ticks for the microphone, and
/// a rising one when an answer waits on standby.
pub(crate) const CALL_START: Sound = &[partial(220.0, 330.0, 0.2, 0.0, 0.15, 10.0, 0.22), partial(440.0, 660.0, 0.2, 0.0, 0.15, 14.0, 0.05)];
pub(crate) const CALL_END: Sound = &[partial(220.0, 220.0, 0.0, 0.0, 0.1, 8.0, 0.22), partial(440.0, 440.0, 0.0, 0.0, 0.03, 16.0, 0.12)];
pub(crate) const CALL_ERROR: Sound = &[partial(293.7, 293.7, 0.0, 0.0, 0.01, 14.0, 0.2), partial(415.3, 415.3, 0.0, 0.1, 0.01, 12.0, 0.2)];
pub(crate) const CALL_MUTE: Sound = &[partial(2531.0, 2531.0, 0.0, 0.0, 0.002, 60.0, 0.07)];
pub(crate) const CALL_UNMUTE: Sound = &[partial(1031.0, 1031.0, 0.0, 0.0, 0.003, 40.0, 0.05)];
pub(crate) const CALL_WAITING: Sound = &[partial(217.0, 440.0, 0.15, 0.0, 0.08, 10.0, 0.18)];

/// How long `sound` is heard, in seconds (until about -50 dB).
pub(crate) fn sound_length(sound: Sound) -> f32 {
    sound.iter().map(|p| p.at + p.attack + 6.0 / p.decay).fold(0.0, f32::max)
}

/// The samples of `sound` at `rate`.
pub(crate) fn sound_samples(sound: Sound, rate: u32) -> Vec<f32> {
    let mut out = vec![0f32; (sound_length(sound) * rate as f32) as usize];
    for p in sound {
        let mut phase = 0f32;
        for (i, s) in out[(p.at * rate as f32) as usize..].iter_mut().enumerate() {
            let t = i as f32 / rate as f32;
            let hz = p.hz + (p.to - p.hz) * (t / p.glide.max(1e-6)).min(1.0);
            phase += hz / rate as f32 * std::f32::consts::TAU;
            let envelope = if t < p.attack { t / p.attack } else { (-(t - p.attack) * p.decay).exp() };
            *s += phase.sin() * envelope * p.gain;
        }
    }
    out
}

/// `sound` through the speaker; the stream plays while kept (`sound_length`).
pub(crate) fn chime(sound: Sound) -> Option<cpal::Stream> {
    const RATE: u32 = 22_050;
    play(Arc::new(Mutex::new(sound_samples(sound, RATE).into())), RATE)
}
