use super::*;

#[test]
fn clicked_icon_jumps_bounces_and_rests() {
    assert_eq!(hop_height(0.0), 0.0);
    assert!((hop_height(0.11) - 4.0).abs() < 0.01);
    assert!((hop_height(0.29) - 1.5).abs() < 0.01);
    // 1.0 is what an icon that never hopped gets
    assert_eq!(hop_height(0.5), 0.0);
    assert_eq!(hop_height(1.0), 0.0);
}

#[test]
fn a_script_clicks_over_two_frames_then_types_and_presses_keys() {
    let ctx = Context::default();
    let mut script = Script::new("click 10 20; type olá; key J cmd");
    let mut frame = || {
        let mut raw = egui::RawInput::default();
        script.feed(&ctx, &mut raw);
        raw.events
    };
    assert!(matches!(frame()[1], egui::Event::PointerButton { pressed: true, .. }));
    assert!(matches!(frame()[1], egui::Event::PointerButton { pressed: false, .. }));
    assert!(matches!(&frame()[0], egui::Event::Text(t) if t == "olá"));
    assert!(matches!(frame()[0], egui::Event::Key { key: Key::J, pressed: true, modifiers, .. } if modifiers.command));
}

#[test]
fn the_app_of_a_scene_is_open_while_its_bridge_answers() {
    let dir = std::env::temp_dir().join(format!("magoagent-open-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let endpoint = dir.join("bridge.json");
    assert!(!app_open(&endpoint), "no bridge.json");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    fs::write(&endpoint, json!({ "address": listener.local_addr().unwrap().to_string(), "token": "t" }).to_string()).unwrap();
    assert!(app_open(&endpoint));
    // not the port just freed: a test running beside this one may bind it next. A port under 1024
    // no test can take
    drop(listener);
    fs::write(&endpoint, json!({ "address": "127.0.0.1:1", "token": "t" }).to_string()).unwrap();
    assert!(!app_open(&endpoint), "a port nobody listens on");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_script_wait_lets_the_app_idle_and_perf_logs_each_second() {
    // a step asks for the next frame at once; a `wait` only for its end
    let ctx = Context::default();
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = asked.clone();
    ctx.set_repaint_observer(move |info| seen.lock().unwrap().push(info.delay.as_secs_f64()));
    let mut script = Script::new("wait 500; quit");
    let mut feed = |time: f64| {
        script.feed(&ctx, &mut egui::RawInput { time: Some(time), ..Default::default() });
        asked.lock().unwrap().pop()
    };
    assert_eq!(feed(0.0), Some(0.0), "the step itself: the next frame at once");
    let waiting = feed(0.1).unwrap();
    assert!((waiting - 0.4).abs() < 1e-6, "waiting: a frame only at its end, not {waiting}");
    assert_eq!(feed(0.6), Some(0.0), "its end: the next step");
    // MAGOAGENT_PERF: a header, then one line a second
    let path = std::env::temp_dir().join(format!("mago-perf-{}.tsv", std::process::id()));
    let _ = fs::remove_file(&path);
    let mut perf = Perf::default();
    perf.path = path.clone();
    perf.frame(10.0, Some(0.002), &[]);
    perf.frame(10.5, Some(0.004), &[]);
    perf.frame(11.0, Some(0.006), &[]);
    let lines: Vec<String> = fs::read_to_string(&path).unwrap().lines().map(String::from).collect();
    assert_eq!(lines[0], "tempo	quadros	cpu_med_ms	cpu_max_ms	causas");
    assert_eq!(lines[1..], ["10.0	1	2.00	2.00	".to_string(), "11.0	2	5.00	6.00	".to_string()]);
    let _ = fs::remove_file(&path);
}
