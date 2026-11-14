use super::*;

/// A sentence being said: the engine's 16-bit mono samples played on the speaker as they come.
pub(crate) struct Player {
    pub(super) child: Child,
    pub(super) _stream: Option<cpal::Stream>,
    pub(super) samples: Arc<Mutex<VecDeque<f32>>>,
    pub(super) ended: Arc<AtomicBool>,
}

impl Player {
    /// The 16-bit mono samples at `rate` that `child` writes, played as they come.
    pub(crate) fn from_pcm(mut child: Child, rate: u32) -> Player {
        let samples: Arc<Mutex<VecDeque<f32>>> = Arc::default();
        let ended = Arc::new(AtomicBool::new(false));
        let (sink, done) = (samples.clone(), ended.clone());
        if let Some(mut stdout) = child.stdout.take() {
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = stdout.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let mut sink = sink.lock().unwrap();
                    for pair in buf[..n - n % 2].chunks(2) {
                        sink.push_back(i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0);
                    }
                }
                done.store(true, Ordering::Relaxed);
            });
        }
        let stream = play(samples.clone(), rate);
        Player { child, _stream: stream, samples, ended }
    }

    /// Said through: the program ended and nothing is left to play.
    pub(crate) fn done(&mut self) -> bool {
        if self._stream.is_none() {
            return matches!(self.child.try_wait(), Ok(Some(_)));
        }
        self.ended.load(Ordering::Relaxed) && self.samples.lock().unwrap().is_empty()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The speaker fed from `samples` (mono at `rate`), resampled to the device's rate on the fly.
pub(super) fn play(samples: Arc<Mutex<VecDeque<f32>>>, rate: u32) -> Option<cpal::Stream> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_output_device()?;
    let config = device.default_output_config().ok()?;
    let (out_rate, channels) = (config.sample_rate().0 as f32, config.channels() as usize);
    let step = rate as f32 / out_rate;
    let (mut last, mut next, mut t) = (0f32, 0f32, 1f32);
    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| {
                let mut source = samples.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    t += step;
                    while t >= 1.0 {
                        t -= 1.0;
                        last = next;
                        next = source.pop_front().unwrap_or(0.0);
                    }
                    let v = last * (1.0 - t) + next * t;
                    frame.fill(v);
                }
            },
            |e| eprintln!("alto-falante: {e}"),
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some(stream)
}
