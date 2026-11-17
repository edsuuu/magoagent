use super::analysis::{FRAMES_PER_SECOND, RATE};
use super::capture::{Downsample, programs, samples_of};
use super::*;

/// A small xorshift: the same noise every run.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 % 20_001) as f32 / 10_000.0 - 1.0
    }
}

/// `seconds` of clicks at `bpm` (a 1.5 kHz ping of ~10 ms), the first at `first` s, under white noise of `noise`.
fn clicks(bpm: f64, seconds: f64, first: f64, noise: f32) -> Vec<f32> {
    let mut random = Noise(0x2545_f491);
    let period = 60.0 / bpm;
    (0..(seconds * f64::from(RATE)) as usize)
        .map(|index| {
            let time = index as f64 / f64::from(RATE);
            let since = (time - first).rem_euclid(period);
            let click =
                if time >= first && since < 0.03 { 0.8 * (-since / 0.004).exp() as f32 * (since * 1500.0 * std::f64::consts::TAU).sin() as f32 } else { 0.0 };
            click + noise * random.next()
        })
        .collect()
}

/// `samples` heard in pieces of 40 ms, as the capture hands them.
fn hear(samples: &[f32]) -> Analyzer {
    let mut analyzer = Analyzer::default();
    for piece in samples.chunks(441) {
        analyzer.feed(piece);
    }
    analyzer
}

#[test]
fn click_tracks_under_noise_give_their_tempo_without_octave_errors() {
    for bpm in [85.0, 120.0, 150.0] {
        for noise in [0.0, 0.05, 0.2] {
            let analyzer = hear(&clicks(bpm, 10.0, 0.1, noise));
            let heard = analyzer.bpm().unwrap_or_else(|| panic!("{bpm} BPM with noise {noise}: no tempo"));
            assert!((heard - bpm).abs() <= 3.0, "{bpm} BPM with noise {noise}: heard {heard}");
        }
    }
}

#[test]
fn a_kick_on_the_beat_and_hats_between_are_the_kicks_tempo() {
    // 85 BPM: a low thump on each beat and a soft hi-hat on each half beat; 170 would be the octave error
    let mut random = Noise(7);
    let period = 60.0 / 85.0;
    let samples: Vec<f32> = (0..10 * RATE as usize)
        .map(|index| {
            let time = index as f64 / f64::from(RATE);
            let beat = time.rem_euclid(period);
            let half = time.rem_euclid(period / 2.0);
            let kick = 0.9 * (-beat / 0.06).exp() as f32 * (beat * 70.0 * std::f64::consts::TAU).sin() as f32;
            let hat = 0.15 * (-half / 0.01).exp() as f32 * random.next();
            kick + hat
        })
        .collect();
    let heard = hear(&samples).bpm().expect("a tempo");
    assert!((heard - 85.0).abs() <= 3.0, "heard {heard}");
}

#[test]
fn the_beat_clock_locks_on_the_clicks() {
    // clicks from 0.137 s at 120 BPM; the last look is at the end of the sound
    let (bpm, first, seconds) = (120.0, 0.137, 12.0);
    let analyzer = hear(&clicks(bpm, seconds, first, 0.02));
    let groove = analyzer.groove();
    let looked = (seconds * FRAMES_PER_SECOND).floor() / FRAMES_PER_SECOND;
    let expected = ((looked - first) * bpm / 60.0).rem_euclid(1.0) as f32;
    let off = ((groove.phase - expected + 0.5).rem_euclid(1.0) - 0.5).abs();
    assert!(off < 0.08, "phase {} against {expected}", groove.phase);
    assert!((groove.bpm - 120.0).abs() <= 3.0);
}

#[test]
fn energy_and_bass_follow_the_sound() {
    let tone = |hertz: f64, level: f32, seconds: f64| -> Vec<f32> {
        (0..(seconds * f64::from(RATE)) as usize).map(|index| level * (index as f64 / f64::from(RATE) * hertz * std::f64::consts::TAU).sin() as f32).collect()
    };
    let low = hear(&tone(60.0, 0.5, 3.0)).groove();
    let high = hear(&tone(2000.0, 0.5, 3.0)).groove();
    assert!(low.bass > 0.9 && high.bass < 0.1, "{} {}", low.bass, high.bass);
    // loud, then 12 dB quieter: the energy falls with it, against the loud part
    let mut sound = tone(440.0, 0.5, 4.0);
    sound.extend(tone(440.0, 0.125, 3.0));
    let (loud, quiet) = (hear(&sound[..4 * RATE as usize]).groove().energy, hear(&sound).groove().energy);
    assert!(loud > 0.9 && quiet < 0.5, "{loud} {quiet}");
    // silence naps, with no energy
    let silence = hear(&vec![0.0; 3 * RATE as usize]).groove();
    assert!(silence.dance == Dance::Nap && silence.energy == 0.0);
}

/// A loudness envelope looked at `FRAMES_PER_SECOND` times a second: (seconds, dB) stretches.
fn envelope(stretches: &[(f32, f32)]) -> Vec<f32> {
    stretches.iter().flat_map(|&(seconds, decibels)| std::iter::repeat_n(decibels, (seconds * FRAMES_PER_SECOND as f32) as usize)).collect()
}

/// When each watcher fired (seconds), over `decibels`.
fn watch(decibels: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let step = (1.0 / FRAMES_PER_SECOND) as f32;
    let (mut loudness, mut chorus, mut drops) = (Loudness::default(), ChorusWatch::default(), DropWatch::default());
    let (mut choruses, mut dropped, mut was) = (Vec::new(), Vec::new(), false);
    for (index, &level) in decibels.iter().enumerate() {
        loudness.hear(level, step);
        let on = chorus.hear(loudness.short, loudness.long, step);
        if on && !was {
            choruses.push(index as f32 * step);
        }
        was = on;
        if drops.hear(loudness.fast, loudness.long, step) {
            dropped.push(index as f32 * step);
        }
    }
    (choruses, dropped)
}

#[test]
fn the_chorus_is_the_sound_fuller_for_a_few_seconds() {
    // a verse, a chorus 5 dB up for 15 s, the verse again
    let (choruses, drops) = watch(&envelope(&[(30.0, -20.0), (15.0, -15.0), (20.0, -20.0)]));
    assert_eq!(choruses.len(), 1, "{choruses:?}");
    assert!((33.5..37.0).contains(&choruses[0]), "a few seconds into it: {choruses:?}");
    assert!(drops.is_empty(), "{drops:?}");
    // a louder second is no chorus
    let (choruses, _) = watch(&envelope(&[(30.0, -20.0), (1.5, -14.0), (20.0, -20.0)]));
    assert!(choruses.is_empty(), "{choruses:?}");
}

#[test]
fn the_drop_is_the_sound_jumping_back_after_seconds_of_quiet() {
    // 20 s of song, 3 s of breakdown 12 dB down, then all of it at once
    let (_, drops) = watch(&envelope(&[(20.0, -15.0), (3.0, -27.0), (10.0, -14.0)]));
    assert_eq!(drops.len(), 1, "{drops:?}");
    assert!((23.0..23.6).contains(&drops[0]), "{drops:?}");
    // a quiet second is no drop
    let (_, drops) = watch(&envelope(&[(20.0, -15.0), (1.0, -27.0), (10.0, -14.0)]));
    assert!(drops.is_empty(), "{drops:?}");
    // nor a slow climb back
    let mut climb = envelope(&[(20.0, -15.0), (3.0, -27.0)]);
    let rising = (4.0 * FRAMES_PER_SECOND) as usize;
    climb.extend((0..rising).map(|index| -27.0 + 13.0 * index as f32 / rising as f32));
    climb.extend(envelope(&[(5.0, -14.0)]));
    let (_, drops) = watch(&climb);
    assert!(drops.is_empty(), "{drops:?}");
}

/// The dances chosen over `beats` beats of the same `cues`, from `beat`.
fn dance_for(chooser: &mut Chooser, cues: Cues, beat: &mut f64, beats: f64) -> Dance {
    let end = *beat + beats;
    while *beat < end {
        chooser.choose(&cues, *beat);
        *beat += 0.1;
    }
    chooser.dance
}

#[test]
fn each_tempo_has_its_dance_and_a_new_one_waits_four_bars() {
    let tempo = |bpm: f32| Cues { bpm: Some(bpm), energy: 0.6, bass: 0.3, ..Default::default() };
    for (bpm, dance) in [(70.0, Dance::Sway), (95.0, Dance::Step), (112.0, Dance::Clap), (126.0, Dance::Pump), (150.0, Dance::Bang)] {
        let (mut chooser, mut beat) = (Chooser::default(), 0.0);
        assert_eq!(dance_for(&mut chooser, tempo(bpm), &mut beat, 1.0), dance, "{bpm} BPM, at once as the first tempo is heard");
    }
    let (mut chooser, mut beat) = (Chooser::default(), 0.0);
    assert_eq!(dance_for(&mut chooser, Cues::default(), &mut beat, 4.0), Dance::Sway, "no tempo yet: the sway");
    assert_eq!(dance_for(&mut chooser, tempo(112.0), &mut beat, 1.0), Dance::Clap);
    // faster: still clapping until four bars passed
    assert_eq!(dance_for(&mut chooser, tempo(128.0), &mut beat, 12.0), Dance::Clap);
    assert_eq!(dance_for(&mut chooser, tempo(128.0), &mut beat, 4.0), Dance::Pump);
    // 116 is in both ranges: the dance danced stays
    assert_eq!(dance_for(&mut chooser, tempo(116.0), &mut beat, 40.0), Dance::Pump);
    // a strong bass with energy headbangs; the chorus waves over it
    let heavy = Cues { energy: 0.95, bass: 0.8, ..tempo(116.0) };
    assert_eq!(dance_for(&mut chooser, heavy, &mut beat, 17.0), Dance::Bang);
    assert_eq!(dance_for(&mut chooser, Cues { chorus: true, ..heavy }, &mut beat, 17.0), Dance::Wave);
}

#[test]
fn the_drop_spins_once_and_silence_naps_at_once() {
    let tempo = Cues { bpm: Some(126.0), energy: 0.6, bass: 0.3, ..Default::default() };
    let (mut chooser, mut beat) = (Chooser::default(), 0.0);
    assert_eq!(dance_for(&mut chooser, tempo, &mut beat, 2.0), Dance::Pump);
    // the drop comes off the beat: the spin starts on the next one and lasts four
    beat = 2.3;
    assert_eq!(chooser.choose(&Cues { drop: true, ..tempo }, beat), Dance::Spin);
    assert_eq!(chooser.since, 3.0);
    assert_eq!(dance_for(&mut chooser, tempo, &mut beat, 4.5), Dance::Spin);
    assert_eq!(dance_for(&mut chooser, tempo, &mut beat, 0.5), Dance::Pump, "once, back to the dance");
    assert_eq!(chooser.choose(&Cues { silent: true, ..tempo }, beat), Dance::Nap, "silence: at once");
    assert_eq!(chooser.choose(&tempo, beat + 0.1), Dance::Pump, "and back as it plays");
}

#[test]
fn the_moves_take_whole_beats_and_the_spin_lands_on_one() {
    assert_eq!(cycle_beats(1.58, 76.0), 2.0, "the sway at the clock's tempo: two beats, the preview's 1.58 s");
    assert_eq!(cycle_beats(0.556, 108.0), 1.0, "a hop a beat");
    assert_eq!(cycle_beats(0.952, 126.0), 2.0);
    assert_eq!(cycle_beats(0.4, 90.0), 1.0, "never less than a beat");
    for bpm in [60.0, 100.0, 140.0, 180.0] {
        let beats = spin_beats(bpm);
        assert!((beats * 0.75).fract() == 0.0 && (4.0..=8.0).contains(&beats), "{bpm}: {beats}");
    }
    let groove = Groove::clock(10.0, 120.0, Dance::Sway);
    assert_eq!((groove.beat, groove.phase), (20.0, 0.0));
    let later = groove.later(0.25);
    assert!((later.beat - 20.5).abs() < 1e-9 && (later.phase - 0.5).abs() < 1e-6);
}

#[test]
fn listening_starts_only_while_it_plays_and_a_dancer_asks() {
    assert!(!should_listen(true, true, None), "nobody asked");
    assert!(!should_listen(true, false, Some(Duration::ZERO)), "nothing plays");
    assert!(!should_listen(false, true, Some(Duration::ZERO)), "the switch is off");
    assert!(!should_listen(true, true, Some(Duration::from_secs(3))), "asked too long ago");
    assert_eq!(should_listen(true, true, Some(Duration::from_millis(500))), !cfg!(target_os = "macos"), "macOS never listens");

    use crate::app::integrations::spotify::{Now, Track};
    let mut app = DrawingApp::default();
    let ctx = Context::default();
    app.prefs.integrations.spotify = true;
    let paused = Now { track: Track::new("A", "B", "", "").unwrap(), playing: false, ..Default::default() };
    // nothing loaded, then paused: no listening even with a dancer drawn
    app.integrations_tick(&ctx);
    assert!(app.luna_groove().is_none(), "nothing loaded: no dancer");
    app.integrations.spotify.now = Some(paused.clone());
    assert_eq!(app.luna_groove().map(|groove| groove.dance), Some(Dance::Nap), "paused: she naps");
    app.integrations_tick(&ctx);
    assert_eq!(app.integrations.spotify.dancer.starts, 0, "a dancer asked, but nothing plays");
    // playing, but no dancer drawn
    app.integrations.spotify.dancer = Dancer::default();
    app.integrations.spotify.now = Some(Now { playing: true, ..paused.clone() });
    app.integrations_tick(&ctx);
    assert_eq!(app.integrations.spotify.dancer.starts, 0, "nobody asked");
    // a dancer drawn: it listens (not on macOS); before anything is heard, the clock's sway
    assert_eq!(app.luna_groove().map(|groove| groove.dance), Some(Dance::Sway));
    app.integrations_tick(&ctx);
    let listens = u32::from(!cfg!(target_os = "macos"));
    assert_eq!(app.integrations.spotify.dancer.starts, listens);
    assert_eq!(app.integrations.spotify.dancer.listening(), listens == 1);
    app.integrations_tick(&ctx);
    assert_eq!(app.integrations.spotify.dancer.starts, listens, "once");
    // paused: it ends; the switch off and the island off (its bars listen too): no dancer and no listening
    app.integrations.spotify.now = Some(paused.clone());
    app.integrations_tick(&ctx);
    assert!(!app.integrations.spotify.dancer.listening());
    (app.prefs.integrations.luna_dances, app.prefs.island) = (false, false);
    app.integrations.spotify.now = Some(Now { playing: true, ..paused });
    assert!(app.luna_groove().is_none());
    app.integrations_tick(&ctx);
    assert!(!app.integrations.spotify.dancer.listening() && app.integrations.spotify.dancer.starts == listens);
    assert!(DrawingApp::default().prefs.integrations.luna_dances, "on by default");
    let old: crate::app::integrations::IntegrationPrefs = serde_json::from_str(r#"{"spotify":true}"#).unwrap();
    assert!(old.luna_dances, "an old preferences.json without it dances too");
}

#[test]
fn the_sound_is_read_as_the_programs_hand_it() {
    let mut carry = None;
    assert_eq!(samples_of(&[0x00, 0x40, 0x00], &mut carry), [0.5]);
    assert_eq!(carry, Some(0x00));
    assert_eq!(samples_of(&[0xc0], &mut carry), [-0.5], "the odd byte joins the next");
    assert_eq!(carry, None);
    let mut out = Vec::new();
    let mut downsample = Downsample::new(RATE * 4);
    downsample.feed(&[1.0, 1.0, 0.0, 0.0, -1.0, -1.0, -1.0, -1.0], &mut out);
    assert_eq!(out, [0.5, -1.0]);
    // one channel, at 11025, 16 bits, raw, on the default output's monitor
    let [(pipewire, pipewire_arguments), (pulse, pulse_arguments)] = programs();
    assert_eq!((pipewire, pulse), ("pw-record", "parec"));
    for wanted in ["--raw", "11025", "1", "s16", "-"] {
        assert!(pipewire_arguments.iter().any(|argument| argument == wanted), "{wanted}");
    }
    assert!(pipewire_arguments.iter().any(|argument| argument.contains("stream.capture.sink=true")));
    for wanted in ["@DEFAULT_MONITOR@", "--raw", "--rate=11025", "--channels=1", "--format=s16le"] {
        assert!(pulse_arguments.iter().any(|argument| argument == wanted), "{wanted}");
    }
}

/// By hand, on this computer's sound (it only listens, for 6 s; nothing is kept): what it hears, and what it costs.
/// `cargo test --release luna_listens_here -- --ignored --nocapture`
#[test]
#[ignore]
fn luna_listens_here() {
    let cpu = |pid: &str| -> f64 {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        let fields: Vec<&str> = stat.rsplit(')').next().unwrap_or_default().split_whitespace().collect();
        let ticks = |index: usize| fields.get(index).and_then(|field| field.parse::<f64>().ok()).unwrap_or(0.0);
        (ticks(11) + ticks(12)) / 100.0
    };
    let dancer = Dancer::default();
    dancer.ask();
    let shared = Arc::new(Shared::new(dancer.asked.clone()));
    let thread_shared = shared.clone();
    let (start, started) = (cpu("self"), Instant::now());
    let thread = std::thread::spawn(move || capture::listen(thread_shared));
    let mut children = 0.0;
    while started.elapsed() < Duration::from_secs(6) {
        std::thread::sleep(Duration::from_millis(500));
        dancer.ask();
        if let Some((groove, _)) = *shared.latest.lock().unwrap() {
            println!("{:>5.1} BPM  fase {:.2}  energia {:.2}  grave {:.2}  {}", groove.bpm, groove.phase, groove.energy, groove.bass, groove.dance.name());
        }
        let output = std::process::Command::new("pgrep").args(["-P", &std::process::id().to_string()]).output().unwrap();
        for pid in String::from_utf8_lossy(&output.stdout).split_whitespace() {
            children = f64::max(children, cpu(pid));
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    shared.stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
    println!(
        "CPU: o app {:.1}% de um núcleo, o programa que ouve {:.1}%; falhou: {}",
        (cpu("self") - start) / seconds * 100.0,
        children / seconds * 100.0,
        shared.failed.load(Ordering::Relaxed)
    );
    // the analysis with a beat to follow (silence skips the tempo): 30 s of clicks, timed
    let sound = clicks(120.0, 30.0, 0.1, 0.05);
    let started = Instant::now();
    let analyzer = hear(&sound);
    println!("análise com batida: {:.2}% de um núcleo ({:.0} BPM)", started.elapsed().as_secs_f64() / 30.0 * 100.0, analyzer.groove().bpm);
}

/// The island's bars are the sound by frequency: a low tone lifts the first band over the last, a high one the last
/// over the first, and silence lets them all fall.
#[test]
fn the_bands_follow_where_the_sound_is() {
    let tone = |hertz: f64| -> Vec<f32> {
        (0..(2.0 * f64::from(RATE)) as usize).map(|index| 0.5 * (index as f64 / f64::from(RATE) * hertz * std::f64::consts::TAU).sin() as f32).collect()
    };
    let low = hear(&tone(70.0)).groove().bands.unwrap();
    assert!(low[0] > low[BANDS - 1] + 0.3, "a 70 Hz tone is bass: {low:?}");
    let high = hear(&tone(3000.0)).groove().bands.unwrap();
    assert!(high[BANDS - 1] > high[0] + 0.3, "a 3 kHz tone is treble: {high:?}");
    let mut analyzer = hear(&tone(3000.0));
    analyzer.feed(&vec![0.0; RATE as usize]);
    assert!(analyzer.groove().bands.unwrap().iter().all(|level| *level < 0.05), "silence: {:?}", analyzer.groove().bands);
}
