use super::*;

#[test]
fn bridge_lists_only_what_is_connected() {
    let mut app = DrawingApp::default();
    let terminal = |app: &DrawingApp, name: &str, x: f32| {
        let mut e = app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 300.0, height: 200.0 });
        e.terminal = Some(TerminalSpec { name: name.into(), ..Default::default() });
        e
    };
    let (a, b, c) = (terminal(&app, "Shell", 0.0), terminal(&app, "Claude Code", 400.0), terminal(&app, "Sozinho", 800.0));
    let (a_id, b_id) = (a.id.clone(), b.id.clone());
    app.scene.elements = vec![a, b, c];
    app.connect(&a_id, &b_id);
    // the connection hangs like a rope: its middle sags below the straight line between its ends
    let rope = app.scene.elements.iter().find(|e| e.rope).unwrap();
    let path = line_path(rope);
    let (first, middle, last) = (path[0], path[path.len() / 2], path[path.len() - 1]);
    assert!(middle[1] > (first[1] + last[1]) / 2.0 + 10.0, "{path:?}");
    assert!(element_bounds(rope).max.y >= middle[1]);
    // it swings: right after an end moves its middle lags behind, then it settles at rest
    app.swing_ropes(0.016);
    let b_index = app.scene.elements.iter().position(|e| e.id == b_id).unwrap();
    app.scene.elements[b_index].move_by(Vec2::new(300.0, 120.0));
    app.update_bindings();
    assert!(app.swing_ropes(0.016), "moving right after the drag");
    for _ in 0..600 {
        app.swing_ropes(0.016);
    }
    assert!(!app.swing_ropes(0.016), "settled");
    app.apply_agent_command(UiCommand::CreateNote { from: a_id.clone(), text: "# Plano\n- testar".into() });
    let note = app.scene.elements.iter().find(|e| e.created.is_some()).unwrap();
    let label = app.scene.elements.iter().find(|e| e.id == format!("{}-label", note.id)).unwrap();
    assert!((local_bounds(label).center() - element_bounds(note).center()).length() < 0.1, "the label sits in the middle");
    let nodes = app.agent_nodes();
    let shared = std::sync::Mutex::new(Shared { nodes, ..Default::default() });
    let (sender, commands) = std::sync::mpsc::channel();
    let ask = |cmd: Value| handle(&cmd, &shared, &sender);
    let list = ask(json!({ "cmd": "list", "from": a_id })).unwrap();
    assert!(list.contains("Você: \"Shell\"") && list.contains("\"Claude Code\"") && list.contains("\"Plano\""), "{list}");
    assert!(!list.contains("Sozinho"), "only what a line connects: {list}");
    assert_eq!(ask(json!({ "cmd": "note_read", "from": a_id, "name": "plano" })).unwrap(), "# Plano\n- testar");
    assert!(ask(json!({ "cmd": "check", "from": a_id, "name": "Sozinho" })).unwrap_err().contains("Disponíveis: \"Claude Code\""));
    ask(json!({ "cmd": "note_write", "from": a_id, "name": "Plano", "text": "feito" })).unwrap();
    let command = commands.try_recv().unwrap();
    app.apply_agent_command(command);
    assert_eq!(app.agent_nodes().iter().find(|n| n.kind == NodeKind::Note).unwrap().text, "feito");
    // a rope needs both ends: deleting a terminal takes its rope with it on the next frame
    let ropes = |app: &DrawingApp| app.scene.elements.iter().filter(|e| e.rope).count();
    assert_eq!(ropes(&app), 2);
    app.selected = vec![app.scene.elements.iter().position(|e| e.id == b_id).unwrap()];
    app.delete_selected();
    app.swing_ropes(0.016);
    assert_eq!(ropes(&app), 1, "the note's rope stays");
    // not while an end is being dragged (the press unbinds it); dropped in the open, it goes
    app.point_drag = Some(0);
    app.scene.elements.iter_mut().find(|e| e.rope).unwrap().ends.start = None;
    app.swing_ropes(0.016);
    assert_eq!(ropes(&app), 1, "still dragging");
    app.point_drag = None;
    app.swing_ropes(0.016);
    assert_eq!(ropes(&app), 0);
    // `connect` reaches the whole canvas by name: the caller ropes itself to the lone terminal
    std::thread::scope(|s| {
        let asked = s.spawn(|| ask(json!({ "cmd": "connect", "from": a_id, "name": "sozinho" })));
        app.apply_agent_command(commands.recv().unwrap());
        assert!(asked.join().unwrap().unwrap().ends_with("conectados."));
    });
    assert_eq!(ropes(&app), 1);
    assert!(ask(json!({ "cmd": "recruit", "from": "", "name": "X" })).is_err(), "a recruit needs a terminal to sit below");
    // outside the canvas (MCP from another terminal app) everything is reachable
    assert!(ask(json!({ "cmd": "list", "from": "" })).unwrap().contains("Sozinho"));
    assert_eq!(unescape("a\\n\\x03\\e"), b"a\r\x03\x1b");
    // a real line break (the MCP's JSON "\n") is Enter too, and CR LF is one
    assert_eq!(unescape("dir\nls\r\n"), b"dir\rls\r");
    assert_eq!(note_name("## Tarefas da semana\nresto"), "Tarefas da semana");
    // the formatting bar wraps the whole line: the title drops the closing marks too
    assert_eq!(note_name("**Nota em negrito**"), "Nota em negrito");
    assert_eq!(note_name("- ~~riscada~~"), "riscada");
    assert_eq!(note_name("* item"), "item");
    // the card's markdown: block kinds per line, marks dropped from inline text
    use crate::render::markdown::{Line, Mark, inline, lines};
    assert_eq!(
        lines("## Oi\n  - [x] feito\n2. dois\n> cita\n---\n```\n*a*\n```"),
        [
            Line::Heading(2, "Oi"),
            Line::Item { depth: 1, mark: Mark::Task(true), text: "feito" },
            Line::Item { depth: 0, mark: Mark::Number("2.".into()), text: "dois" },
            Line::Quote("cita"),
            Line::Rule,
            Line::Code("*a*"),
        ]
    );
    let spans: Vec<(String, bool, bool)> = inline("a **b** `c` [d](u)", false).into_iter().map(|s| (s.text, s.bold, s.code || s.link)).collect();
    assert_eq!(
        spans,
        [
            ("a ".into(), false, false),
            ("b".into(), true, false),
            (" ".into(), false, false),
            ("c".into(), false, true),
            (" ".into(), false, false),
            ("d".into(), false, true)
        ]
    );
    // the agent's cursor is built without innerHTML (pages with Trusted Types)
    let cursor = cursor_script("IA", "#a", None);
    assert!(!cursor.contains("innerHTML") && cursor.contains("createElementNS"));
}

#[test]
fn only_the_web_goes_to_the_browser() {
    assert_eq!(normalize_url("exemplo.com/a"), "https://exemplo.com/a");
    assert_eq!(normalize_url("localhost:3000"), "http://localhost:3000");
    assert_eq!(normalize_url(" https://x.dev "), "https://x.dev");
    // a scheme only at the start; only the web leaves for the browser
    assert_eq!(normalize_url("exemplo.com/?r=http://x"), "https://exemplo.com/?r=http://x");
    assert_eq!(normalize_url("file:///C:/a.html"), "file:///C:/a.html");
    assert!(is_web("HTTPS://x.dev") && is_web("http://localhost:3000"));
    assert!(!is_web("file:///C:/a.html") && !is_web("mailto:a@b.c") && !is_web("ms-settings:") && !is_web("about:blank"));
}

/// CA-15: a rope goes with its terminal and comes back with it on Ctrl+Z.
#[test]
fn undoing_a_deleted_terminal_brings_its_rope_back() {
    let mut app = DrawingApp::default();
    let terminal = |app: &DrawingApp, name: &str, x: f32| {
        let mut e = app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 300.0, height: 200.0 });
        e.terminal = Some(TerminalSpec { name: name.into(), ..Default::default() });
        e
    };
    let (a, b) = (terminal(&app, "Shell", 0.0), terminal(&app, "Revisor", 400.0));
    let (a_id, b_id) = (a.id.clone(), b.id.clone());
    app.scene.elements = vec![a, b];
    app.connect(&a_id, &b_id);
    let ropes = |app: &DrawingApp| app.scene.elements.iter().filter(|e| e.rope).count();
    app.selected = vec![app.scene.elements.iter().position(|e| e.id == b_id).unwrap()];
    app.delete_selected();
    app.swing_ropes(0.016);
    assert_eq!(ropes(&app), 0);
    app.undo();
    app.swing_ropes(0.016);
    assert_eq!(ropes(&app), 1, "the terminal comes back with its rope");
}

#[cfg(unix)]
#[test]
fn bridge_asks_an_agent_over_tcp() {
    let bridge = Bridge::start(None).unwrap();
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(("/bin/sh", &[][..])), 10_000).unwrap();
    let node =
        |id: &str, link: &str| Node { id: id.into(), kind: NodeKind::Terminal, name: id.into(), text: String::new(), links: vec![link.into()], maestro: true };
    *bridge.shared.lock().unwrap() = Shared {
        nodes: vec![node("eu", "outro"), node("outro", "eu")],
        terminals: [("outro".to_string(), session.handle.clone())].into(),
        ..Default::default()
    };
    let request = |cmd: Value| request_to(&bridge.address, &bridge.token, "eu", cmd, std::time::Duration::from_secs(20));
    let answer = request(json!({ "cmd": "ask", "name": "outro", "text": "echo pong-$((40+2))", "timeout": 20 })).unwrap();
    assert!(answer.contains("pong-42"), "{answer}");
    // the rope between them was lit only while the ask ran
    assert!(bridge.shared.lock().unwrap().asking.is_empty());
    assert!(request(json!({ "cmd": "check", "name": "outro" })).unwrap().contains("pong-42"));
    assert!(request_to(&bridge.address, "errado", "eu", json!({ "cmd": "list" }), std::time::Duration::from_secs(5)).is_err());
}

#[test]
fn an_agent_answers_one_question_at_a_time() {
    let node = |id: &str| Node { id: id.into(), kind: NodeKind::Terminal, name: id.into(), text: String::new(), links: vec!["filho".into()], maestro: true };
    let shared = std::sync::Mutex::new(Shared {
        nodes: vec![node("pai"), node("eu"), Node { links: vec![], ..node("filho") }],
        asking: vec![("pai".into(), "filho".into())],
        ..Default::default()
    });
    let (sender, _commands) = std::sync::mpsc::channel();
    let refused = handle(&json!({ "cmd": "ask", "name": "filho", "text": "oi", "from": "eu" }), &shared, &sender).unwrap_err();
    assert_eq!(refused, "\"filho\" já está respondendo a pai: espere essa resposta ou leia a tela com check.");
    assert_eq!(shared.lock().unwrap().asking, [("pai".to_string(), "filho".to_string())], "the first ask is left as it was");
    // nor while it works on something else (an ask that timed out included); keys still go
    shared.lock().unwrap().asking.clear();
    shared.lock().unwrap().states.insert("filho".into(), crate::app::ui::agents::AgentState::Working(0.0));
    let busy = handle(&json!({ "cmd": "ask", "name": "filho", "text": "oi", "from": "eu" }), &shared, &sender).unwrap_err();
    assert_eq!(busy, "\"filho\" está trabalhando: espere ou leia a tela com check.");
    let keys = handle(&json!({ "cmd": "ask", "name": "filho", "text": "\\e", "raw": true, "from": "eu" }), &shared, &sender).unwrap_err();
    assert!(keys.contains("não está rodando"), "{keys}");
    // a question that never got typed is no question of the chat's; the event says so
    shared.lock().unwrap().states.clear();
    shared.lock().unwrap().events.clear();
    let _ = handle(&json!({ "cmd": "ask", "name": "filho", "text": " oi ", "from": "eu" }), &shared, &sender);
    assert!(shared.lock().unwrap().asked.is_empty());
    let events = std::mem::take(&mut shared.lock().unwrap().events);
    assert_eq!(events, [("filho".to_string(), "eu".to_string(), "não recebeu a pergunta".to_string())], "from the asked back to the asker");
}

#[test]
fn an_ask_reads_only_what_the_agent_wrote_after_it() {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("mago-desde-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("s.jsonl");
    let line = |kind: &str, text: &str| format!("{}\n", json!({ "type": kind, "message": { "content": [{ "type": "text", "text": text }] } }));
    fs::write(&path, line("assistant", "antiga")).unwrap();
    let mut said = crate::transcript::Transcript::from_end(path.clone());
    said.poll();
    assert_eq!(said.agent_words(), None);
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all((line("user", "pergunta") + &line("assistant", "primeira") + &line("assistant", "segunda")).as_bytes()).unwrap();
    said.poll();
    assert_eq!(said.agent_words().as_deref(), Some("primeira\n\nsegunda"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_child_agent_builds_nothing_but_still_notifies() {
    let node = |id: &str, maestro: bool| Node { id: id.into(), kind: NodeKind::Terminal, name: id.into(), text: String::new(), links: vec![], maestro };
    let shared = std::sync::Mutex::new(Shared { nodes: vec![node("pai", true), node("filho", false)], ..Default::default() });
    let (sender, _commands) = std::sync::mpsc::channel();
    let request = |cmd: Value| handle(&cmd, &shared, &sender);
    let refused = "Só um terminal Maestro pode criar terminais, navegadores ou conexões.";
    assert_eq!(request(json!({ "cmd": "recruit", "name": "Neto", "from": "filho" })).unwrap_err(), refused);
    assert_eq!(request(json!({ "cmd": "portal_create", "text": "exemplo.com", "from": "filho" })).unwrap_err(), refused);
    assert_eq!(request(json!({ "cmd": "connect", "name": "pai", "to": "filho", "from": "filho" })).unwrap_err(), refused);
    assert!(request(json!({ "cmd": "notify", "text": "oi", "from": "filho" })).is_ok());
}

#[test]
fn a_rope_leaves_from_the_edge_even_over_the_other_window() {
    let a = rect_element(0.0, 0.0, 100.0, 100.0, None);
    let center = Binding { element_id: a.id.clone(), fixed: [0.5, 0.5], inside: false };
    // the other window's center is inside this one: out through the right side, not the middle
    let p = bound_endpoint(&a, &center, Pos2::new(70.0, 50.0));
    assert!(p.x > 100.0 && (p.y - 50.0).abs() < 0.01, "{p:?}");
}

#[test]
fn agents_panel_reads_prompts_and_times() {
    use crate::{app::ui::agents::ago, terminal::asks_for_you};
    assert!(asks_for_you("Edit file src/main.rs\n Do you want to make this edit?\n ❯ 1. Yes\n   2. No"));
    assert!(asks_for_you(
        " Bash command\n Do you want to proceed?\n ❯ 1. Yes\n   2. Yes, and don't ask again\n   3. No, and tell Claude what to do differently (esc)"
    ));
    assert!(asks_for_you(" Quick safety check: trust this folder?\n ❯ No, exit\n   Yes, I trust this folder\n Enter to confirm · Esc to cancel"));
    // an answer that ends in a question, with the typing box below, isn't waiting
    assert!(!asks_for_you("⏺ Two ways:\n  1. Fast\n  2. Safe\n  Would you like me to do it?\n╭──────╮\n│ >    │\n╰──────╯\n  ? for shortcuts"));
    assert!(!asks_for_you("> \n  ? for shortcuts"));
    assert_eq!(ago(2.0), "agora");
    assert_eq!(ago(30.0), "há 30s");
    assert_eq!(ago(725.0), "há 12 min");
    assert_eq!(ago(7300.0), "há 2 h");
}

#[test]
fn a_menu_on_the_screen_gives_its_choices() {
    use crate::terminal::{Menu, menu_choices};
    let trust = "Claude Code'll be able to read, edit, and execute files here.

Security guide

 ❯ No, exit
   Yes, I trust this folder

 Enter to confirm · Esc to cancel";
    let menu = menu_choices(trust).unwrap();
    assert_eq!(menu.choices, ["No, exit", "Yes, I trust this folder"]);
    assert_eq!((menu.marked, menu.question.last().map(String::as_str)), (0, Some("Security guide")));
    // in a box, the marker further down, a choice's second line under it
    let bash = "╭────────────╮
│ Bash command │
│ Do you want to proceed? │
│   1. Yes │
│ ❯ 2. Yes, and don't ask again │
│      for this folder │
│   3. No │
╰────────────╯";
    assert_eq!(
        menu_choices(bash),
        Some(Menu {
            question: vec!["Bash command".into(), "Do you want to proceed?".into()],
            choices: vec!["1. Yes".into(), "2. Yes, and don't ask again".into(), "3. No".into()],
            marked: 1
        })
    );
    // the typing box is no menu
    assert_eq!(
        menu_choices(
            "> 
  ? for shortcuts"
        ),
        None
    );
}

#[test]
fn a_prompt_waits_while_claude_asks_or_after_it_left() {
    use crate::{app::ui::agents::prompt_gate, transcript::Session};
    let session = |status: &str, waiting: Option<&str>| Session { status: status.into(), waiting_for: waiting.map(String::from), ..Default::default() };
    // its own question first: the words would answer it
    assert!(prompt_gate(Some(&session("waiting", Some("input needed"))), true, true, 60_000, "✳ Revisão").unwrap().contains("input needed"));
    assert!(prompt_gate(Some(&session("waiting", None)), false, false, 0, "").unwrap().contains("permissão"));
    assert_eq!(prompt_gate(Some(&session("busy", None)), true, true, 60_000, "pwsh"), None);
    // made for claude, no session once started, a shell's title: it left
    assert!(prompt_gate(None, true, false, 60_000, "pwsh").unwrap().contains("shell"));
    assert_eq!(prompt_gate(None, true, false, 5_000, "pwsh"), None);
    assert_eq!(prompt_gate(None, true, false, 60_000, "✳ Revisão"), None);
    assert_eq!(prompt_gate(None, false, false, 60_000, "pwsh"), None);
    // its session went (a crash keeps the title): it left, whatever the title says
    assert!(prompt_gate(None, false, true, 1_000, "✳ Revisão").unwrap().contains("shell"));
    // a shell's own title is no mission
    use crate::app::ui::agents::shell_title;
    assert!(shell_title("C:\\Program Files\\WindowsApps\\pwsh.exe") && shell_title("eu@pc: ~/proj") && shell_title("/home/eu"));
    assert!(!shell_title("✳ Revisão do login") && !shell_title("Rodando os testes"));
}
