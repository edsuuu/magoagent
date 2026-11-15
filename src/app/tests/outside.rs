//! Claude Code sessions outside the app (maestro/outside.rs): the hook's way to the bridge and the island's card, what
//! the hook prints, the queue, the click guard and the badge. Nothing here touches the person's own settings.

use super::*;
use crate::app::maestro::outside::{Badge, MAX_ASKS};
use crate::app::maestro::relay;
use crate::mcp::bridge::{Bridge, UiCommand};
use std::sync::mpsc;
use std::time::Duration;

const SIZE: Vec2 = Vec2::new(1200.0, 800.0);

fn questions() -> Value {
    json!({ "questions": [
        { "question": "Qual cor?", "header": "Cor", "multiSelect": false, "options": [{ "label": "Azul", "description": "fria" }, { "label": "Verde", "description": "" }] },
        { "question": "Quais frutas?", "header": "Frutas", "multiSelect": true, "options": [{ "label": "Maçã", "description": "" }, { "label": "Uva", "description": "" }] },
    ] })
}

fn answers(value: Value) -> serde_json::Map<String, Value> {
    value.as_object().unwrap().clone()
}

fn hook(event: &str, tool: &str, input: Value) -> Value {
    json!({ "hook_event_name": event, "session_id": "s1", "cwd": "C:/Users/eu/loja", "tool_name": tool, "tool_input": input })
}

fn desktop(name: &str) -> Option<String> {
    (name == "CLAUDE_CODE_ENTRYPOINT").then(|| "claude-desktop".to_string())
}

fn island_app() -> DrawingApp {
    let mut app = DrawingApp::default();
    app.prefs.island = true;
    app
}

/// A hook process that waits for the app: what it prints comes back on the channel.
fn hook_process(endpoint: (String, String), payload: Value, may_wait: bool) -> mpsc::Receiver<String> {
    let (printed, output) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = printed.send(relay::send(endpoint, payload, may_wait, &desktop));
    });
    output
}

/// The app takes what the hook sent over the bridge, as its loop does.
fn take(app: &mut DrawingApp, bridge: &Bridge) {
    let Ok(UiCommand::Hook { from, payload, reply }) = bridge.commands.recv_timeout(Duration::from_secs(5)) else { panic!("the hook did not arrive") };
    app.take_hook(&from, &payload, reply);
}

fn endpoint(bridge: &Bridge) -> (String, String) {
    (bridge.address.clone(), bridge.token.clone())
}

fn first_ask_id(app: &DrawingApp) -> String {
    app.person_asks.borrow()[0].chat.clone()
}

#[test]
fn the_hook_prints_what_claude_code_takes() {
    let input = questions();
    let both = answers(json!({ "Qual cor?": "Verde", "Quais frutas?": ["Maçã", "Uva"] }));
    let printed: Value = serde_json::from_str(&question_answer("PermissionRequest", &input, &both)).unwrap();
    let decision = &printed["hookSpecificOutput"]["decision"];
    assert_eq!((printed["hookSpecificOutput"]["hookEventName"].as_str(), decision["behavior"].as_str()), (Some("PermissionRequest"), Some("allow")));
    assert_eq!(decision["updatedInput"]["answers"], json!({ "Qual cor?": "Verde", "Quais frutas?": ["Maçã", "Uva"] }));
    assert_eq!(decision["updatedInput"]["questions"], input["questions"], "the questions go back as they came");
    let printed: Value = serde_json::from_str(&question_answer("PreToolUse", &input, &both)).unwrap();
    assert_eq!(printed["hookSpecificOutput"]["permissionDecision"], "allow");
    assert_eq!(printed["hookSpecificOutput"]["updatedInput"]["answers"]["Qual cor?"], "Verde");
    // what does not fit prints nothing, and the terminal asks: an unknown label, a missing answer, a list for one choice, repeats, a string for many
    for wrong in [
        json!({ "Qual cor?": "Roxo", "Quais frutas?": ["Maçã"] }),
        json!({ "Qual cor?": "Azul" }),
        json!({ "Qual cor?": ["Azul"], "Quais frutas?": ["Maçã"] }),
        json!({ "Qual cor?": "Azul", "Quais frutas?": ["Maçã", "Maçã"] }),
        json!({ "Qual cor?": "Azul", "Quais frutas?": "Maçã" }),
        json!({ "Qual cor?": "Azul", "Quais frutas?": [] }),
        json!({ "Qual cor?": "Azul", "Quais frutas?": ["Maçã"], "Extra?": "x" }),
    ] {
        assert_eq!(question_answer("PermissionRequest", &input, &answers(wrong.clone())), "", "{wrong}");
    }
    assert_eq!(question_answer("PermissionRequest", &json!({ "command": "ls" }), &answers(json!({}))), "", "no questions, no answer");
    // permissions
    let allow: Value = serde_json::from_str(&permission_answer(true, "")).unwrap();
    assert_eq!(allow["hookSpecificOutput"]["decision"], json!({ "behavior": "allow" }));
    let deny: Value = serde_json::from_str(&refusal("PermissionRequest", "não")).unwrap();
    assert_eq!(deny["hookSpecificOutput"]["decision"], json!({ "behavior": "deny", "message": "não" }));
    let deny: Value = serde_json::from_str(&refusal("PreToolUse", "não")).unwrap();
    assert_eq!(deny["hookSpecificOutput"], json!({ "hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": "não" }));
}

#[test]
fn a_hook_sends_its_origin_and_not_the_weight() {
    let mut payload = hook("PostToolUse", "Bash", json!({ "command": "x".repeat(5000) }));
    payload["tool_response"] = json!("saída enorme");
    payload["transcript_path"] = json!("C:/t.jsonl");
    relay::tag(&mut payload, &desktop);
    assert_eq!(payload["mago_origin"], "claude-desktop");
    assert!(payload.get("tool_response").is_none() && payload.get("transcript_path").is_none());
    assert_eq!(payload["tool_input"]["command"].as_str().unwrap().len(), 2000);
    // a question's input goes whole: it is what comes back with the answers
    let mut ask = hook("PermissionRequest", "AskUserQuestion", json!({ "questions": [{ "question": "q".repeat(3000), "options": [] }] }));
    relay::tag(&mut ask, &|name| (name == "CURSOR_TRACE_ID").then(|| "1".to_string()));
    assert_eq!(ask["mago_origin"], "cursor");
    assert_eq!(ask["tool_input"]["questions"][0]["question"].as_str().unwrap().len(), 3000);
    let mut plain = hook("Stop", "", Value::Null);
    relay::tag(&mut plain, &|_| None);
    assert_eq!(plain["mago_origin"], "");
}

#[test]
fn a_question_from_claude_desktop_reaches_the_island_and_the_answer_goes_back() {
    let bridge = Bridge::start(None).unwrap();
    let mut app = island_app();
    let input = questions();
    let output = hook_process(endpoint(&bridge), hook("PreToolUse", "AskUserQuestion", input.clone()), true);
    take(&mut app, &bridge);
    let id = first_ask_id(&app);
    {
        let asks = app.person_asks.borrow();
        assert_eq!(asks.len(), 1);
        let source = asks[0].source.as_ref().unwrap();
        assert_eq!((source.label.as_str(), source.event.as_str(), source.session.as_str()), ("Claude Desktop", "PreToolUse", "s1"));
        assert_eq!(asks[0].questions.len(), 2);
    }
    let pills = app.outside_pills();
    assert_eq!((pills[0].name.as_str(), pills[0].color), ("Claude Desktop", Color32::from_rgb(0xd9, 0x77, 0x57)));
    assert_eq!(pills[0].badge, Some(Badge::Approval), "the dot while something waits");
    assert_eq!(pills[0].state, crate::app::ui::agents::AgentState::Waiting);
    assert!(output.recv_timeout(Duration::from_millis(200)).is_err(), "the hook waits");
    {
        let mut asks = app.person_asks.borrow_mut();
        asks[0].picked = vec![vec![1], vec![0, 1]];
    }
    app.answer_person(&id, true);
    let printed: Value = serde_json::from_str(&output.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
    assert_eq!(printed["hookSpecificOutput"]["updatedInput"]["answers"], json!({ "Qual cor?": "Verde", "Quais frutas?": ["Maçã", "Uva"] }));
    assert!(app.person_asks.borrow().is_empty());
}

#[test]
fn a_permission_is_allowed_or_denied_and_the_terminal_answers_nothing() {
    let bridge = Bridge::start(None).unwrap();
    let mut app = island_app();
    for (allow, behavior) in [(true, "allow"), (false, "deny")] {
        let output = hook_process(endpoint(&bridge), hook("PermissionRequest", "Bash", json!({ "command": "rm -rf build" })), false);
        take(&mut app, &bridge);
        assert_eq!(app.person_asks.borrow()[0].summary(), "rm -rf build");
        app.answer_person(&first_ask_id(&app), allow);
        let printed: Value = serde_json::from_str(&output.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
        assert_eq!(printed["hookSpecificOutput"]["decision"]["behavior"], behavior);
    }
    // "Responder no terminal": nothing printed
    let output = hook_process(endpoint(&bridge), hook("PermissionRequest", "Bash", json!({ "command": "ls" })), false);
    take(&mut app, &bridge);
    app.person_asks.borrow_mut().remove(0).send_terminal();
    assert_eq!(output.recv_timeout(Duration::from_secs(5)).unwrap(), "");
    // events that wait for nobody come back at once, and an app that is not there is no wait at all
    let output = hook_process(endpoint(&bridge), hook("Stop", "", Value::Null), false);
    take(&mut app, &bridge);
    assert_eq!(output.recv_timeout(Duration::from_secs(5)).unwrap(), "");
    assert_eq!(relay::send(("127.0.0.1:1".into(), "x".into()), hook("PermissionRequest", "Bash", Value::Null), false, &|_| None), "");
}

#[test]
fn only_the_hook_that_waits_holds_a_question_and_it_answers_in_its_own_shape() {
    let bridge = Bridge::start(None).unwrap();
    let mut app = island_app();
    // o PreToolUse geral e o PermissionRequest da pergunta voltam na hora e nem chegam ao app
    for (event, may_wait) in [("PreToolUse", false), ("PermissionRequest", false), ("PermissionRequest", true)] {
        assert_eq!(relay::send(endpoint(&bridge), hook(event, "AskUserQuestion", questions()), may_wait, &desktop), "", "{event}");
        assert!(bridge.commands.try_recv().is_err(), "{event}: nada foi ao app");
    }
    // um relay antigo que ainda os mande: nem cartão, nem pílula trabalhando, e o hook é solto na hora
    let (reply, output) = mpsc::channel();
    app.take_hook("ext:s1", &hook("PermissionRequest", "AskUserQuestion", questions()), Some(reply));
    app.take_hook("ext:s1", &hook("PreToolUse", "AskUserQuestion", questions()), None);
    assert!(output.recv().is_err(), "solto sem resposta");
    assert!(app.person_asks.borrow().is_empty() && app.outside_pills().is_empty());
    let output = hook_process(endpoint(&bridge), hook("PreToolUse", "AskUserQuestion", questions()), true);
    take(&mut app, &bridge);
    assert_eq!(app.person_asks.borrow().len(), 1);
    app.person_asks.borrow_mut()[0].picked = vec![vec![0], vec![1]];
    app.answer_person(&first_ask_id(&app), true);
    let printed: Value = serde_json::from_str(&output.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
    assert_eq!(printed["hookSpecificOutput"]["permissionDecision"], "allow");
    assert_eq!(printed["hookSpecificOutput"]["updatedInput"]["answers"], json!({ "Qual cor?": "Azul", "Quais frutas?": ["Uva"] }));
    // "Responder no terminal": nada impresso, e o Claude Code pergunta lá
    let output = hook_process(endpoint(&bridge), hook("PreToolUse", "AskUserQuestion", questions()), true);
    take(&mut app, &bridge);
    app.person_asks.borrow_mut().remove(0).send_terminal();
    assert_eq!(output.recv_timeout(Duration::from_secs(5)).unwrap(), "");
    assert!(app.person_asks.borrow().is_empty());
}

#[test]
fn the_queue_holds_five_and_the_oldest_is_the_card() {
    let mut app = island_app();
    let mut outputs = Vec::new();
    for count in 0..MAX_ASKS + 1 {
        let (reply, output) = mpsc::channel();
        let mut payload = hook("PermissionRequest", "Bash", json!({ "command": format!("cmd {count}") }));
        payload["session_id"] = format!("s{count}").into();
        app.take_hook(&format!("ext:s{count}"), &payload, Some(reply));
        outputs.push(output);
    }
    assert_eq!(app.person_asks.borrow().len(), MAX_ASKS);
    assert_eq!(outputs[MAX_ASKS].try_recv(), Ok(Ok(String::new())), "the sixth goes back to the terminal");
    assert!(outputs[0].try_recv().is_err(), "the others wait");
    // FIFO: the card is the first that came; answered, the next is the card
    assert_eq!(app.person_asks.borrow()[0].summary(), "cmd 0");
    app.answer_person(&first_ask_id(&app), true);
    assert_eq!(app.person_asks.borrow()[0].summary(), "cmd 1");
    assert!(outputs[0].try_recv().unwrap().unwrap().contains("allow"));
    // the island shows one card and says how many wait behind it
    let ctx = Context::default();
    let texts = settle(&mut app, &ctx);
    assert!(texts.iter().any(|(text, _)| text == "+3 na fila"), "{texts:?}");
    assert!(texts.iter().any(|(text, _)| text.contains("cmd 1")) && !texts.iter().any(|(text, _)| text.contains("cmd 2")));
}

/// One island frame at `time`: what it drew, as text and where.
fn frame(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), time: Some(time), events, ..Default::default() };
    let output = ctx.run_ui(raw, |_| app.island_ui(ctx));
    output
        .shapes
        .iter()
        .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.visual_bounding_rect())) } else { None })
        .collect()
}

/// The island for two seconds: open and settled; what the last frame drew.
fn settle(app: &mut DrawingApp, ctx: &Context) -> Vec<(String, Rect)> {
    (0..40).map(|step| frame(app, ctx, 1.0 + step as f64 * 0.05, vec![])).last().unwrap()
}

#[test]
fn a_new_card_ignores_clicks_for_600_ms() {
    let ctx = Context::default();
    let mut app = island_app();
    let (reply, output) = mpsc::channel();
    app.take_hook("ext:s1", &hook("PermissionRequest", "Bash", json!({ "command": "ls" })), Some(reply));
    let at = settle(&mut app, &ctx).iter().find(|(text, _)| text == "Responder no terminal").map(|(_, rect)| rect.center()).expect("the button");
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let click = |app: &mut DrawingApp, from: f64| {
        for (step, events) in [vec![egui::Event::PointerMoved(at)], vec![press(true)], vec![press(false)]].into_iter().enumerate() {
            frame(app, &ctx, from + step as f64 * 0.05, events);
        }
    };
    // the card is 0.15 s old at the release: nothing happens
    app.person_asks.borrow_mut()[0].shown = Some(3.0 + 0.1 - 0.05);
    click(&mut app, 3.0);
    assert!(output.try_recv().is_err(), "too soon: nothing answered");
    assert_eq!(app.person_asks.borrow().len(), 1);
    // a card that has been there is answered by the click
    app.person_asks.borrow_mut()[0].shown = Some(0.0);
    click(&mut app, 3.5);
    assert_eq!(output.try_recv(), Ok(Ok(String::new())));
    assert!(app.person_asks.borrow().is_empty());
    assert_eq!(app.outside.declined.borrow().len(), 1, "a question given back is remembered");
}

#[test]
fn the_turn_going_on_or_ending_drops_the_card_and_the_badge_comes_and_goes() {
    let mut app = island_app();
    let (reply, output) = mpsc::channel();
    app.take_hook("ext:s1", &hook("PreToolUse", "Bash", json!({ "command": "ls" })), None);
    assert_eq!(app.outside_pills()[0].state, crate::app::ui::agents::AgentState::Working(0.0));
    assert_eq!(app.outside_pills()[0].name, "Claude Code · loja");
    app.take_hook("ext:s1", &hook("PermissionRequest", "Bash", json!({ "command": "ls" })), Some(reply));
    assert_eq!(app.person_asks.borrow().len(), 1);
    // answered in the terminal: the same call finished
    app.take_hook("ext:s1", &hook("PostToolUse", "Edit", json!({})), None);
    assert_eq!(app.person_asks.borrow().len(), 1, "another tool says nothing of it");
    app.take_hook("ext:s1", &hook("PostToolUse", "Bash", json!({ "command": "pwd" })), None);
    assert_eq!(app.person_asks.borrow().len(), 1, "nem outra chamada da mesma ferramenta (um subagente ao lado)");
    app.take_hook("ext:s1", &hook("PostToolUse", "Bash", json!({ "command": "ls" })), None);
    assert!(app.person_asks.borrow().is_empty());
    assert!(output.recv().is_err(), "the hook is let go");
    // the turn ends: the green dot, which the pointer on the pill clears
    app.take_hook("ext:s1", &hook("Stop", "", Value::Null), None);
    assert_eq!(app.outside_pills()[0].badge, Some(Badge::Finished));
    assert!(app.outside_focus("pill:code:C:/Users/eu/loja"));
    assert_eq!(app.outside_pills()[0].badge, None, "seen");
    // an error shows red, and a new event clears it
    app.take_hook("ext:s1", &hook("StopFailure", "", Value::Null), None);
    assert_eq!(app.outside_pills()[0].badge, Some(Badge::Error));
    app.take_hook("ext:s1", &hook("UserPromptSubmit", "", Value::Null), None);
    assert_eq!(app.outside_pills()[0].badge, None);
    // 5.2 s after a Stop the pill leaves; a session that ended goes at once
    app.take_hook("ext:s1", &hook("Stop", "", Value::Null), None);
    app.outside_tick();
    assert_eq!(app.outside_pills().len(), 1);
    app.outside.expire_badges();
    app.outside_tick();
    assert!(app.outside_pills().is_empty());
    app.take_hook("ext:s2", &hook("SessionStart", "", Value::Null), None);
    assert_eq!(app.outside_pills().len(), 1);
    app.take_hook("ext:s2", &hook("SessionEnd", "", Value::Null), None);
    assert!(app.outside_pills().is_empty());
}

#[test]
fn the_island_off_or_a_plan_goes_back_to_the_terminal() {
    let mut app = DrawingApp::default();
    let (reply, output) = mpsc::channel();
    app.take_hook("ext:s1", &hook("PermissionRequest", "Bash", json!({ "command": "ls" })), Some(reply));
    assert_eq!(output.try_recv(), Ok(Ok(String::new())), "no island, no wait");
    app.prefs.island = true;
    let (reply, output) = mpsc::channel();
    app.take_hook("ext:s1", &hook("PermissionRequest", "ExitPlanMode", json!({ "plan": "x" })), Some(reply));
    assert_eq!(output.try_recv(), Ok(Ok(String::new())), "a plan needs the terminal's own screen");
    let (reply, output) = mpsc::channel();
    app.take_hook("ext:s1", &hook("PreToolUse", "AskUserQuestion", json!({ "questions": [] })), Some(reply));
    assert_eq!(output.try_recv(), Ok(Ok(String::new())), "a question with nothing to pick");
}

#[test]
fn a_notification_is_a_question_or_the_plan_limit_and_subagents_show_in_the_activity() {
    use crate::app::ui::agents::AgentState;
    let mut app = island_app();
    let notification = |message: &str| {
        let mut payload = hook("Notification", "", Value::Null);
        payload["message"] = message.into();
        payload
    };
    let mut prompt = hook("UserPromptSubmit", "", Value::Null);
    prompt["prompt"] = "arruma o teste\ne roda de novo".into();
    app.take_hook("ext:s1", &prompt, None);
    assert_eq!((app.outside_pills()[0].activity.as_str(), app.outside_pills()[0].state), ("arruma o teste", AgentState::Working(0.0)));
    app.take_hook("ext:s1", &hook("PreToolUse", "Bash", json!({ "command": "cargo test" })), None);
    assert_eq!(app.outside_pills()[0].activity, "Bash: cargo test");
    // o `demands` do MCP conta as sessões de fora
    assert!(app.act_demands("", "", "").unwrap().ends_with("Sessões fora do app:\n- Claude Code · loja — Trabalhando — Bash: cargo test"));
    // um subagente aparece na atividade e não muda o estado
    app.take_hook("ext:s1", &hook("SubagentStart", "", Value::Null), None);
    assert_eq!((app.outside_pills()[0].activity.as_str(), app.outside_pills()[0].state), ("+ subagente", AgentState::Working(0.0)));
    app.take_hook("ext:s1", &hook("SubagentStop", "", Value::Null), None);
    assert_eq!(app.outside_pills()[0].activity, "• subagente terminou");
    // espera de entrada sem pergunta: nada; com pergunta, a pílula espera
    app.take_hook("ext:s1", &notification("Claude is waiting for your input"), None);
    assert_eq!(app.outside_pills()[0].state, AgentState::Working(0.0));
    app.take_hook("ext:s1", &notification("Posso apagar a pasta build?"), None);
    assert_eq!((app.outside_pills()[0].activity.as_str(), app.outside_pills()[0].state), ("Posso apagar a pasta build?", AgentState::Waiting));
    // o limite do plano: estado próprio, que some quando a sessão volta a trabalhar
    app.take_hook("ext:s1", &notification("You've hit your usage limit · resets 3pm"), None);
    let pills = app.outside_pills();
    assert!(pills[0].limited && pills[0].state == AgentState::Idle(None));
    app.take_hook("ext:s1", &hook("PostToolUseFailure", "Bash", json!({ "command": "cargo test" })), None);
    let pills = app.outside_pills();
    assert!(!pills[0].limited);
    assert_eq!(pills[0].activity, "falhou: Bash: cargo test");
    // a vez acaba com a última frase da resposta; o ajudante que o Claude Code roda depois (agent_type vazio) não reabre a pílula
    let mut stop = hook("Stop", "", Value::Null);
    stop["last_assistant_message"] = "Pronto: os 12 testes passam.".into();
    app.take_hook("ext:s1", &stop, None);
    let mut helper = hook("SubagentStop", "", Value::Null);
    helper["agent_type"] = "".into();
    app.take_hook("ext:s1", &helper, None);
    let pills = app.outside_pills();
    assert_eq!((pills[0].activity.as_str(), pills[0].badge, pills[0].state), ("Pronto: os 12 testes passam.", Some(Badge::Finished), AgentState::Idle(None)));
    // um prompt que o próprio Claude Code injeta (em tags) não vira atividade
    prompt["prompt"] = "<task-notification>x</task-notification>".into();
    app.take_hook("ext:s1", &prompt, None);
    assert_eq!(app.outside_pills()[0].activity, "Pronto: os 12 testes passam.");
}

#[test]
fn a_relay_that_hangs_up_takes_its_card_away() {
    use std::io::Write;
    let bridge = Bridge::start(None).unwrap();
    let mut app = island_app();
    // o relay de verdade é um processo; aqui, a conexão dele
    let ask = |command: &str| {
        let mut relay = std::net::TcpStream::connect(&bridge.address).unwrap();
        let payload = hook("PermissionRequest", "Bash", json!({ "command": command }));
        writeln!(relay, "{}", json!({ "cmd": "hook", "token": bridge.token, "from": "ext:s1", "payload": payload, "wait": 60 })).unwrap();
        relay
    };
    let answered_in_terminal = ask("rm -rf build");
    take(&mut app, &bridge);
    let waiting = ask("ls");
    take(&mut app, &bridge);
    assert_eq!(app.person_asks.borrow().len(), 2);
    assert!(!app.person_asks.borrow()[0].source.as_ref().unwrap().request.is_empty(), "a ponte numerou o pedido");
    // respondido no terminal: o Claude Code encerra o relay, a ponte vê a conexão cair e o cartão dele sai; o outro fica
    drop(answered_in_terminal);
    take(&mut app, &bridge);
    assert_eq!(app.person_asks.borrow().len(), 1);
    assert_eq!(app.person_asks.borrow()[0].summary(), "ls");
    assert_eq!(app.outside_pills()[0].activity, "respondido fora da ilha");
    // o que ainda espera é respondido pela ilha como sempre
    app.answer_person(&first_ask_id(&app), true);
    let mut printed = String::new();
    std::io::BufRead::read_line(&mut std::io::BufReader::new(waiting), &mut printed).unwrap();
    assert!(printed.contains("allow"), "{printed}");
}
