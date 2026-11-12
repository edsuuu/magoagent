use super::*;

#[cfg(windows)]
#[test]
fn a_prompt_is_not_typed_into_a_menu_and_the_session_file_says_the_state() {
    use crate::app::ui::agents::AgentState;
    // a permission menu on screen: the words would pick one of its options
    let menu = "Write-Host 'Do you want to proceed?'; Write-Host ' 1. Yes'; Write-Host ' 2. No'; Start-Sleep 30";
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(("powershell", &["-NoProfile", "-Command", menu][..])), 10_000).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !session.handle.screen_text().contains("2. No") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(session.handle.waiting());
    assert!(session.handle.type_prompt("oi").unwrap_err().contains("menu"));
    // a gate says why first, and lifts
    session.handle.set_gate(Some("não está com o claude aberto".into()), false);
    assert_eq!(session.handle.type_prompt("oi"), Err("não está com o claude aberto".into()));
    session.handle.set_gate(None, false);
    assert!(session.handle.type_prompt("oi").unwrap_err().contains("menu"));
    // a session that says it rests is believed over numbered lines under a question (an answer
    // can end that way); only a menu's own footer would still count
    session.handle.set_gate(None, true);
    assert_eq!(session.handle.type_prompt("oi"), Ok(()));
    // the Claude Code session's word over the screen's
    let mut app = DrawingApp::default();
    app.terminals.insert("t".into(), Ok(session));
    let says =
        |status: &str, ago_ms: i64| crate::transcript::Session { status: status.into(), status_at: crate::model::now_ms() - ago_ms, ..Default::default() };
    app.agent_sessions.insert("t".into(), says("busy", 5000));
    assert!(matches!(app.agent_state("t"), AgentState::Working(t) if (app.now - t - 5.0).abs() < 1.0), "{:?}", app.agent_state("t"));
    app.agent_sessions.insert("t".into(), says("idle", 0));
    assert!(matches!(app.agent_state("t"), AgentState::Idle(Some(_))));
    app.terminal_waiting.insert("t".into());
    assert_eq!(app.agent_state("t"), AgentState::Waiting);
    assert_eq!(app.agent_state("outro"), AgentState::Ended);
}

#[cfg(windows)]
#[test]
fn an_ask_answers_with_what_the_agent_wrote_to_its_transcript() {
    // a stand-in agent: reads the prompt, adds its answer to the transcript, shows something else
    let dir = std::env::temp_dir().join(format!("mago-pergunta-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let (transcript, answer) = (dir.join("s.jsonl"), dir.join("resposta.json"));
    let line = |text: &str| format!("{}\n", serde_json::json!({ "type": "assistant", "message": { "content": text } }));
    fs::write(&transcript, line("antiga")).unwrap();
    fs::write(&answer, line("resposta completa")).unwrap();
    let script = format!(
        "$null = [Console]::ReadLine(); Get-Content -LiteralPath '{}' | Add-Content -LiteralPath '{}'; Write-Host 'so o fim'; Start-Sleep 30",
        answer.display(),
        transcript.display()
    );
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(("powershell", &["-NoProfile", "-Command", &script][..])), 10_000).unwrap();
    // through the bridge, as `magoagent ask B pergunta` from outside the canvas
    let node =
        crate::bridge::Node { id: "b".into(), kind: crate::bridge::NodeKind::Terminal, name: "B".into(), text: String::new(), links: vec![], maestro: false };
    let shared = std::sync::Mutex::new(crate::bridge::Shared {
        nodes: vec![node],
        terminals: [("b".to_string(), session.handle.clone())].into(),
        transcripts: [("b".to_string(), transcript.clone())].into(),
        ..Default::default()
    });
    let (sender, _commands) = std::sync::mpsc::channel();
    let reply = crate::bridge::handle(&serde_json::json!({ "cmd": "ask", "name": "B", "text": "pergunta", "timeout": 30 }), &shared, &sender);
    assert_eq!(reply, Ok("resposta completa".to_string()));
    // typed, then answered: the chat's question and the events
    let shared = shared.into_inner().unwrap();
    let outside = "um agente fora do canvas".to_string();
    assert_eq!(shared.asked, [("b".to_string(), "pergunta".to_string(), outside.clone())]);
    let said = |from: &str, to: &str, what: &str| (from.to_string(), to.to_string(), what.to_string());
    assert_eq!(shared.events, [said("", "b", "pergunta: pergunta"), said("b", "", "respondeu (a quem perguntou de fora do canvas)")]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_chat_box_grows_with_its_text_up_to_its_cap() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let mut agent = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    agent.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    app.scene.elements.push(agent);
    app.chat_open = true;
    let size = Vec2::new(1000.0, 800.0);
    let height = |app: &mut DrawingApp, draft: &str| {
        app.chat_draft = draft.into();
        for _ in 0..3 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
            ctx.run_ui(raw, |_| app.chat_screen(&ctx)).textures_delta.clear();
        }
        app.chat_composer
    };
    // the card: its margins, the words and the row of buttons under them
    let one = height(&mut app, "oi");
    assert!(one > 60.0 && one < 140.0, "{one}");
    let cap = 200f32.min((size.y - TOP_ROW) * 0.35) + 12.0 + 10.0 + 32.0 + 10.0 + 2.0;
    let long = height(&mut app, &"linha\n".repeat(40));
    assert!(long > one * 2.0 && long <= cap + 1.0, "{long} up to {cap}");
}

#[cfg(windows)]
#[test]
fn the_events_are_what_the_states_prove() {
    let mut app = DrawingApp::default();
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    e.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    let session =
        TerminalSession::spawn("", "", (40, 5), None, vec![], Some(("powershell", &["-NoProfile", "-Command", "Start-Sleep 30"][..])), 10_000).unwrap();
    app.terminals.insert(id.clone(), Ok(session));
    let step = |app: &mut DrawingApp, status: &str| {
        let waiting_for = (status == "waiting").then(|| "input needed".to_string());
        app.agent_sessions
            .insert(id.clone(), crate::transcript::Session { status: status.into(), status_at: crate::model::now_ms(), waiting_for, ..Default::default() });
        app.terminal_waiting = if status == "waiting" { [id.clone()].into() } else { Default::default() };
        app.log_agent_events();
    };
    for status in ["idle", "busy", "busy", "waiting", "busy", "idle", "idle"] {
        step(&mut app, status);
    }
    // a shell that went: ended; restarted: started again
    app.terminals.remove(&id);
    app.log_agent_events();
    let again = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(("powershell", &["-NoProfile", "-Command", "Start-Sleep 30"][..])), 10_000).unwrap();
    app.terminals.insert(id.clone(), Ok(again));
    step(&mut app, "idle");
    let said: Vec<String> = app.agent_events.iter().map(|e| format!("{} {}", e.name, e.what)).collect();
    let expected = ["iniciado", "começou a trabalhar", "precisa de você (input needed)", "começou a trabalhar", "terminou", "encerrado", "iniciado"]
        .map(|what| format!("Revisor {what}"));
    assert_eq!(said, expected);
}
