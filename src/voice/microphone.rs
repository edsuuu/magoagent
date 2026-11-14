use super::*;

pub(crate) fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

/// A virtual cable or the speakers' loopback, by its name: an input that is no microphone (VB-Audio's
/// CABLE Output as the system's input hears only silence).
pub(crate) fn virtual_input(name: &str) -> bool {
    let name = name.to_lowercase();
    ["cable", "virtual", "vb-audio", "voicemeeter", "stereo mix", "mixagem est", "loopback"].iter().any(|v| name.contains(v))
}

/// Of the inputs `names` (the system's first), the one to hear: `wanted` when there, else the system's unless it is
/// a virtual one, else the first that isn't.
pub(crate) fn pick_input<'a>(names: &'a [String], default: Option<&'a str>, wanted: &str) -> Option<&'a str> {
    let found = |n: &str| names.iter().map(String::as_str).find(|m| *m == n);
    (!wanted.is_empty())
        .then(|| found(wanted))
        .flatten()
        .or(default.filter(|d| !virtual_input(d)))
        .or_else(|| names.iter().map(String::as_str).find(|n| !virtual_input(n)))
        .or(default)
}

/// The microphones' names, as Ajustes › Voz lists them (read again every 5 s at most).
pub(crate) fn input_names() -> Vec<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    static SEEN: Mutex<Option<(std::time::Instant, Vec<String>)>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap();
    if seen.as_ref().is_none_or(|(at, _)| at.elapsed().as_secs() >= 5) {
        let names = cpal::default_host().input_devices().map(|d| d.filter_map(|d| d.name().ok()).collect()).unwrap_or_default();
        *seen = Some((std::time::Instant::now(), names));
    }
    seen.as_ref().map(|(_, n)| n.clone()).unwrap_or_default()
}

/// The microphone, until `stop`: what is said is written down by `hear` and goes as `Heard`. `mic` is the one
/// chosen by name (`pick_input`). Runs on its own thread.
pub(crate) fn listen(shared: VoiceShared, tx: Sender<VoiceEvent>, hear: impl Fn(&[f32]) -> Result<String, String>, pause: f32, mic: &str) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let devices: Vec<(String, cpal::Device)> = host.input_devices().map(|d| d.filter_map(|d| Some((d.name().ok()?, d))).collect()).unwrap_or_default();
    let names: Vec<String> = devices.iter().map(|(n, _)| n.clone()).collect();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    let chosen = pick_input(&names, default.as_deref(), mic).map(str::to_string);
    let device = chosen.and_then(|c| devices.into_iter().find(|(n, _)| *n == c).map(|(_, d)| d)).or_else(|| host.default_input_device());
    let Some(device) = device else {
        let _ = tx.send(VoiceEvent::Failed("Nenhum microfone encontrado.".into()));
        return;
    };
    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(VoiceEvent::Failed(format!("Microfone: {e}")));
            return;
        }
    };
    let (rate, channels) = (config.sample_rate().0, config.channels() as usize);
    let raw: Arc<Mutex<Vec<f32>>> = Arc::default();
    let sink = raw.clone();
    let err = |e| eprintln!("microfone: {e}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(&config.into(), move |data: &[f32], _| sink.lock().unwrap().extend_from_slice(data), err, None),
        cpal::SampleFormat::I16 => {
            device.build_input_stream(&config.into(), move |data: &[i16], _| sink.lock().unwrap().extend(data.iter().map(|s| *s as f32 / 32768.0)), err, None)
        }
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _| sink.lock().unwrap().extend(data.iter().map(|s| (*s as f32 - 32768.0) / 32768.0)),
            err,
            None,
        ),
        other => {
            let _ = tx.send(VoiceEvent::Failed(format!("Microfone em formato {other} não é lido.")));
            return;
        }
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(VoiceEvent::Failed(format!("Microfone: {e}")));
            return;
        }
    };
    if let Err(e) = stream.play() {
        let _ = tx.send(VoiceEvent::Failed(format!("Microfone: {e}")));
        return;
    }
    let _ = tx.send(VoiceEvent::Ready);
    let mut segmenter = Segmenter::with_pause(pause);
    let mut pending: Vec<f32> = Vec::new();
    let mut was_hearing = false;
    // while the agent speaks, only someone talking over it gets through (and stops it)
    let (mut barge, mut broke_in) = (BargeIn::default(), false);
    while !shared.stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let taken = std::mem::take(&mut *raw.lock().unwrap());
        pending.extend(to_16k(&taken, channels, rate));
        let muted = shared.muted.load(Ordering::Relaxed);
        let hold = shared.hold.load(Ordering::Relaxed);
        if !hold {
            (barge, broke_in) = (BargeIn::default(), false);
        }
        while pending.len() >= BLOCK {
            let block: Vec<f32> = pending.drain(..BLOCK).collect();
            let peak = block.iter().fold(0f32, |m, s| m.max(s.abs()));
            shared.level.store(peak.to_bits(), Ordering::Relaxed);
            if hold && !broke_in && !muted && barge.feed(&block) {
                broke_in = true;
                let _ = tx.send(VoiceEvent::Interrupt);
                // what was said while it was noticed starts the stretch
                segmenter = Segmenter::with_pause(pause);
                for kept in barge.kept.drain(..) {
                    segmenter.feed(&kept);
                }
                continue;
            }
            if muted || (hold && !broke_in) {
                segmenter = Segmenter::with_pause(pause);
                continue;
            }
            let heard = segmenter.feed(&block);
            if segmenter.in_speech != was_hearing {
                was_hearing = segmenter.in_speech;
                let _ = tx.send(VoiceEvent::Hearing(was_hearing));
            }
            if let Some(samples) = heard {
                let _ = tx.send(VoiceEvent::Status("Entendendo…".into()));
                match hear(&samples) {
                    Ok(text) if !text.is_empty() && !text.starts_with('[') && !text.starts_with('(') => {
                        let _ = tx.send(VoiceEvent::Heard(text));
                    }
                    Ok(_) => {
                        let _ = tx.send(VoiceEvent::Status(String::new()));
                    }
                    Err(e) => {
                        let _ = tx.send(VoiceEvent::Status(format!("Não entendi: {e}")));
                    }
                }
            }
        }
    }
    drop(stream);
}

/// Starts the call's ears on a thread: the microphone, hearing on the VoiceStudio server (checked
/// first, so a server that isn't up says how to bring it up) or on OpenAI's API. With neither
/// there is nothing to hear with, and the why comes as `Failed`; nothing is ever downloaded.
pub(crate) fn start(shared: VoiceShared, setup: VoiceSetup) -> Receiver<VoiceEvent> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let language = setup.language.clone();
        if let Some(studio) = setup.studio.clone() {
            match studio.voices() {
                Ok(_) => listen(shared, tx, |samples| studio.transcribe(&language, samples), setup.pause, &setup.mic),
                Err(why) => {
                    let _ = tx.send(VoiceEvent::Failed(why));
                }
            }
        } else if let Some(key) = setup.openai.clone() {
            listen(shared, tx, |samples| crate::voice::openai::transcribe(&key, &language, samples), setup.pause, &setup.mic);
        } else {
            let _ = tx.send(VoiceEvent::Failed(NO_ENGINE.into()));
        }
    });
    rx
}

/// What to do when no engine hears: the two there are, both set in Ajustes › Voz.
pub(crate) const NO_ENGINE: &str = "Para ouvir, suba o VoiceStudio ou ponha a chave da OpenAI em Ajustes › Voz. O app não baixa modelos de voz.";
