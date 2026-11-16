//! Listening to the computer's own sound, what goes to the speakers: on Linux `pw-record` (PipeWire) on the default
//! output's monitor, else `parec` (PulseAudio) on `@DEFAULT_MONITOR@`, one channel at 11025 a second, raw on its output;
//! on Windows WASAPI's loopback, an input stream on the default output device (cpal); macOS has no way without a driver
//! of its own, and does not listen. The pieces are analysed and dropped: nothing is kept or sent.

use super::analysis::RATE;
use super::*;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};

/// What listens: the program (Linux), or the stream (Windows). Dropping it ends it.
pub(super) enum Capture {
    #[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
    Program(std::process::Child),
    #[cfg_attr(not(windows), allow(dead_code))]
    Stream(cpal::Stream),
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Capture::Program(child) = self {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The programs that listen on Linux, in order: the arguments of each.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
pub(super) fn programs() -> [(&'static str, Vec<String>); 2] {
    let rate = RATE.to_string();
    [
        (
            "pw-record",
            ["-P", "{ stream.capture.sink=true node.name=magoagent-batida media.name=batida }", "--raw", "--rate", &rate, "--channels", "1", "--format", "s16"]
                .into_iter()
                .chain(["--latency", "20ms", "-"])
                .map(String::from)
                .collect(),
        ),
        (
            "parec",
            ["-d", "@DEFAULT_MONITOR@", "--raw", "--channels=1", "--format=s16le", "--latency-msec=20", "--client-name=MagoAgent", "--stream-name=batida"]
                .into_iter()
                .map(String::from)
                .chain([format!("--rate={rate}")])
                .collect(),
        ),
    ]
}

/// Little-endian 16-bit samples to -1..1; an odd byte left waits in `carry` for its pair.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
pub(super) fn samples_of(bytes: &[u8], carry: &mut Option<u8>) -> Vec<f32> {
    let mut joined = Vec::with_capacity(bytes.len() + 1);
    joined.extend(carry.take());
    joined.extend_from_slice(bytes);
    if joined.len() % 2 == 1 {
        *carry = joined.pop();
    }
    joined.as_chunks::<2>().0.iter().map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32768.0).collect()
}

/// The `attempt`-th way to listen, sending pieces of sound to `sink`, and its rate.
#[cfg(not(any(windows, target_os = "macos")))]
fn open(attempt: usize, sink: Sender<Vec<f32>>) -> Option<(Capture, u32)> {
    use std::io::Read;
    use std::process::Stdio;
    let (program, arguments) = programs().into_iter().nth(attempt)?;
    let spawned = crate::editor::command(program).args(&arguments).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
    // not installed: the next one
    let Ok(mut child) = spawned else { return open(attempt + 1, sink) };
    let output = child.stdout.take();
    // from here, a way out kills it
    let capture = Capture::Program(child);
    let mut output = output?;
    std::thread::Builder::new()
        .name("luna-som".into())
        .spawn(move || {
            let (mut bytes, mut carry) = (vec![0u8; 1024], None);
            while let Ok(read) = output.read(&mut bytes) {
                if read == 0 || sink.send(samples_of(&bytes[..read], &mut carry)).is_err() {
                    break;
                }
            }
        })
        .ok()?;
    Some((capture, RATE))
}

#[cfg(windows)]
fn open(attempt: usize, sink: Sender<Vec<f32>>) -> Option<(Capture, u32)> {
    (attempt == 0).then(|| loopback(sink)).flatten()
}

#[cfg(target_os = "macos")]
fn open(_attempt: usize, _sink: Sender<Vec<f32>>) -> Option<(Capture, u32)> {
    None
}

/// WASAPI's loopback: cpal turns an input stream built on an output device into one (on Windows only; elsewhere this
/// fails, and is not asked).
#[cfg_attr(not(windows), allow(dead_code))]
fn loopback(sink: Sender<Vec<f32>>) -> Option<(Capture, u32)> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_output_device()?;
    let config = device.default_output_config().ok()?;
    let (rate, channels) = (config.sample_rate().0, usize::from(config.channels()).max(1));
    let error = |error| eprintln!("batida da Luna: {error}");
    // each frame's channels, as one
    fn mono(data: impl Iterator<Item = f32>, channels: usize) -> Vec<f32> {
        let data: Vec<f32> = data.collect();
        data.chunks(channels).map(|frame| frame.iter().sum::<f32>() / frame.len() as f32).collect()
    }
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let _ = sink.send(mono(data.iter().copied(), channels));
            },
            error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                let _ = sink.send(mono(data.iter().map(|sample| f32::from(*sample) / 32768.0), channels));
            },
            error,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _| {
                let _ = sink.send(mono(data.iter().map(|sample| (f32::from(*sample) - 32768.0) / 32768.0), channels));
            },
            error,
            None,
        ),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some((Capture::Stream(stream), rate))
}

/// From any rate down to `RATE`: each output sample is the mean of the input ones it covers.
pub(super) struct Downsample {
    ratio: f64,
    sum: f32,
    count: u32,
    covered: f64,
}

impl Downsample {
    pub(super) fn new(from: u32) -> Downsample {
        Downsample { ratio: (f64::from(from) / f64::from(RATE)).max(1.0), sum: 0.0, count: 0, covered: 0.0 }
    }

    pub(super) fn feed(&mut self, samples: &[f32], out: &mut Vec<f32>) {
        for &sample in samples {
            self.sum += sample;
            self.count += 1;
            self.covered += 1.0;
            if self.covered >= self.ratio {
                out.push(self.sum / self.count as f32);
                (self.sum, self.count) = (0.0, 0);
                self.covered -= self.ratio;
            }
        }
    }
}

/// The listening thread: hears while it is wanted (`Shared::wanted`), publishing the groove after each piece; when a
/// way to listen ends before anything was heard, the next is tried. Ends `failed` when none heard anything.
pub(super) fn listen(shared: Arc<Shared>) {
    let mut analyzer = Analyzer::default();
    let mut heard = false;
    for attempt in 0.. {
        let (sink, pieces): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = channel();
        let Some((capture, rate)) = open(attempt, sink) else { break };
        let mut downsample = Downsample::new(rate);
        let mut resampled = Vec::new();
        loop {
            if !shared.wanted() {
                drop(capture);
                return shared.end(false);
            }
            match pieces.recv_timeout(Duration::from_millis(100)) {
                Ok(piece) => {
                    heard = true;
                    let piece = if rate == RATE {
                        piece
                    } else {
                        resampled.clear();
                        downsample.feed(&piece, &mut resampled);
                        std::mem::take(&mut resampled)
                    };
                    analyzer.feed(&piece);
                    shared.publish(analyzer.groove());
                }
                // the speakers are quiet (a suspended output sends nothing): wait
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(capture);
        if heard {
            return shared.end(false);
        }
    }
    shared.end(!heard);
}
