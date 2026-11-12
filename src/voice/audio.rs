use super::*;

/// While the agent speaks the microphone hears the speaker too: someone breaking in is a voice
/// well over that echo (2.5×, 0.05 rms at least) for a quarter of a second. The first half second
/// only learns the echo's level.
// ponytail: by level, no echo cancellation; a loud speaker next to the microphone may still cut
// itself (headphones never do); WebRTC's echo canceller if that bites
#[derive(Default)]
pub(crate) struct BargeIn {
    pub(super) echo: f32,
    pub(super) blocks: usize,
    pub(super) loud: usize,
    /// The last 0.3 s, for the words said while it was being noticed.
    pub(crate) kept: VecDeque<Vec<f32>>,
}

impl BargeIn {
    /// True when `block`, with those before it, is someone talking over the agent.
    pub(crate) fn feed(&mut self, block: &[f32]) -> bool {
        let rms = (block.iter().map(|s| s * s).sum::<f32>() / block.len().max(1) as f32).sqrt();
        self.kept.push_back(block.to_vec());
        if self.kept.len() > 15 {
            self.kept.pop_front();
        }
        self.blocks += 1;
        let learning = self.blocks <= 25;
        let loud = !learning && rms > (self.echo * 2.5).max(0.05);
        // the echo's envelope: up fast, down slow, never up to a voice over it
        if !loud {
            self.echo = if rms > self.echo { self.echo * 0.8 + rms * 0.2 } else { self.echo * 0.995 + rms * 0.005 };
        }
        self.loud = if loud { self.loud + 1 } else { 0 };
        self.loud >= 12
    }
}

/// Speech found by level, 20 ms blocks at a time: a stretch that rises over the noise floor for
/// 60 ms starts it, the pause under it ends it; 0.3 s before the rise comes along. Gives each
/// stretch of at least half a second, 60 s at most.
pub(crate) struct Segmenter {
    pub(super) floor: f32,
    pub(super) speech: Vec<f32>,
    pub(super) pre: VecDeque<Vec<f32>>,
    pub(super) loud: usize,
    pub(super) quiet: usize,
    /// Blocks over the floor in the stretch so far: a click alone isn't speech.
    pub(super) voiced: usize,
    pub(crate) in_speech: bool,
    /// Quiet blocks that end it: a pause shorter than this is part of what is said.
    pub(super) end: usize,
}

impl Default for Segmenter {
    fn default() -> Self {
        Segmenter::with_pause(2.0)
    }
}

impl Segmenter {
    /// Ends speech after `seconds` of quiet (0.6 s at least).
    pub(crate) fn with_pause(seconds: f32) -> Self {
        let end = (seconds.max(0.6) * SAMPLE_RATE as f32 / BLOCK as f32) as usize;
        Segmenter { floor: 0.01, speech: Vec::new(), pre: VecDeque::new(), loud: 0, quiet: 0, voiced: 0, in_speech: false, end }
    }

    pub(crate) fn feed(&mut self, block: &[f32]) -> Option<Vec<f32>> {
        let rms = (block.iter().map(|s| s * s).sum::<f32>() / block.len().max(1) as f32).sqrt();
        // the floor follows quiet quickly and noise slowly, so a voice doesn't become the floor
        self.floor = if rms < self.floor { self.floor * 0.9 + rms * 0.1 } else { self.floor * 0.998 + rms * 0.002 };
        let loud = rms > (self.floor * 3.5).max(0.012);
        if !self.in_speech {
            self.pre.push_back(block.to_vec());
            if self.pre.len() > 15 {
                self.pre.pop_front();
            }
            self.loud = if loud { self.loud + 1 } else { 0 };
            if self.loud >= 3 {
                self.in_speech = true;
                (self.quiet, self.voiced) = (0, self.loud);
                self.speech = self.pre.drain(..).flatten().collect();
            }
            return None;
        }
        self.speech.extend_from_slice(block);
        self.voiced += loud as usize;
        self.quiet = if loud { 0 } else { self.quiet + 1 };
        if self.quiet < self.end && self.speech.len() < 60 * SAMPLE_RATE as usize {
            return None;
        }
        self.in_speech = false;
        self.loud = 0;
        let speech = std::mem::take(&mut self.speech);
        (self.voiced >= 10 && speech.len() >= SAMPLE_RATE as usize / 2).then_some(speech)
    }
}

/// Mono at 16 kHz from `channels` interleaved at `rate`, by linear interpolation.
pub(crate) fn to_16k(samples: &[f32], channels: usize, rate: u32) -> Vec<f32> {
    let channels = channels.max(1);
    let mono: Vec<f32> = samples.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect();
    if rate == SAMPLE_RATE || mono.is_empty() {
        return mono;
    }
    let step = rate as f32 / SAMPLE_RATE as f32;
    let n = ((mono.len() as f32 - 1.0) / step).floor() as usize + 1;
    (0..n)
        .map(|i| {
            let x = i as f32 * step;
            let (k, t) = (x as usize, x.fract());
            let next = mono.get(k + 1).copied().unwrap_or(mono[k]);
            mono[k] * (1.0 - t) + next * t
        })
        .collect()
}

/// A WAV file (16-bit mono) of `samples` at `rate`.
pub(crate) fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}
