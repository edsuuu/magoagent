use super::*;

#[test]
fn terminal_tokens_bind_and_revoke_identity() {
    let shared = Mutex::new(Shared::default());
    shared.lock().unwrap().terminal_tokens.insert("terminal-secret".into(), "terminal-a".into());
    let request = json!({"token":"terminal-secret", "cmd":"list"});
    assert_eq!(authorize(&request, &shared, "general").unwrap()["from"], "terminal-a");
    let forged = json!({"token":"terminal-secret", "from":"terminal-b"});
    assert_eq!(authorize(&forged, &shared, "general"), Err("este terminal não pode falar por outro".into()));
    let general = json!({"token":"general", "from":"qualquer"});
    assert_eq!(authorize(&general, &shared, "general").unwrap()["from"], "qualquer");
    shared.lock().unwrap().terminal_tokens.clear();
    assert_eq!(authorize(&request, &shared, "general"), Err("token inválido".into()));
    let unknown = json!({"token":"unknown"});
    assert_eq!(authorize(&unknown, &shared, "general"), Err("token inválido".into()));
    // a restarted shell gets a new token and the old one stops working; a closed terminal's goes too
    let ask = |token: &str| authorize(&json!({ "token": token, "cmd": "list" }), &shared, "general");
    let first = shared.lock().unwrap().terminal_token("terminal-a");
    let other = shared.lock().unwrap().terminal_token("terminal-b");
    let second = shared.lock().unwrap().terminal_token("terminal-a");
    assert_ne!(first, second);
    assert_eq!(ask(&first), Err("token inválido".into()));
    assert_eq!(ask(&second).unwrap()["from"], "terminal-a");
    shared.lock().unwrap().keep_tokens(|id| id == "terminal-a");
    assert_eq!(ask(&other), Err("token inválido".into()));
    assert_eq!(ask(&second).unwrap()["from"], "terminal-a");
}

#[test]
fn a_workspace_off_view_keeps_its_terminals_talking() {
    let node = |id: &str, kind, links: &[&str]| Node {
        id: id.into(),
        kind,
        name: id.into(),
        text: "o plano".into(),
        links: links.iter().map(|l| l.to_string()).collect(),
        maestro: true,
    };
    let shared = Mutex::new(Shared::default());
    shared.lock().unwrap().nodes = vec![node("Tela", NodeKind::Terminal, &[])];
    let away =
        vec![node("Dev", NodeKind::Terminal, &["Revisor", "Plano"]), node("Revisor", NodeKind::Terminal, &["Dev"]), node("Plano", NodeKind::Note, &["Dev"])];
    shared.lock().unwrap().away.insert("loja".into(), ("Loja".into(), away));
    let (sender, _commands) = mpsc::channel();
    let run = |cmd: &str, name: &str| handle(&json!({ "cmd": cmd, "from": "Dev", "name": name }), &shared, &sender);
    let list = run("list", "").unwrap();
    assert!(list.contains("\"Revisor\"") && list.contains("\"Plano\"") && !list.contains("Tela"), "{list}");
    assert_eq!(run("note_read", "Plano"), Ok("o plano".into()));
    // its peer runs no shell in this test: the ask gets as far as the shell
    assert_eq!(run("ask", "Revisor"), Err("esse terminal não está rodando".into()));
    let err = run("note_write", "Plano").unwrap_err();
    assert!(err.contains("\"Loja\"") && err.contains("não está aberto"), "{err}");
    // the canvas on screen is unchanged for the others
    assert!(handle(&json!({ "cmd": "list", "from": "Tela" }), &shared, &sender).unwrap().contains("Nada conectado"));
}

#[test]
fn portal_scripts_escape_selectors_and_limit_wait() {
    let script = portal_wait_script("[title=\"a\\b\"]");
    assert!(script.contains("\\\"a\\\\b\\\""));
    assert!(script.contains("getComputedStyle"));
    assert!(portal_scroll_script(Some("a\\\"b"), None).contains("\\\\\\\""));
}

#[test]
fn portal_eval_reports_exceptions_without_waiting_for_promises() {
    let script = portal_eval_script("Promise.resolve(2)");
    assert!(script.contains("(0,eval)"));
    assert!(!script.contains("await"));
    assert!(script.contains("erro: "));
}

#[test]
fn portal_wait_retries_until_element_appears() {
    let mut replies = ["false", "false", "true"].into_iter();
    let result = portal_wait("#alvo", Duration::from_secs(1), |_| Ok(replies.next().unwrap().into())).unwrap();
    assert_eq!(result, "Elemento encontrado: #alvo");
}

#[test]
fn portal_wait_reports_timeout() {
    let result = portal_wait("#alvo", Duration::from_millis(1), |_| Ok("false".into())).unwrap();
    assert_eq!(result, "Prazo esgotado ao esperar: #alvo");
}

#[test]
fn portal_wait_propagates_evaluation_error() {
    let result = portal_wait("#alvo", Duration::from_secs(1), |_| Err("falha".into()));
    assert_eq!(result, Err("falha".into()));
}

#[test]
fn portal_history_rejects_unknown_actions() {
    assert_eq!(portal_history_action("back"), Some("voltou uma página"));
    assert_eq!(portal_history_action("forward"), Some("avançou uma página"));
    assert_eq!(portal_history_action("reload"), Some("recarregou a página"));
    assert_eq!(portal_history_action("go"), None);
}

#[test]
fn list_follows_the_note_chain() {
    let node = |id: &str, kind, links: &[&str]| Node {
        id: id.into(),
        kind,
        name: id.into(),
        text: String::new(),
        links: links.iter().map(|l| l.to_string()).collect(),
        maestro: false,
    };
    // eu — Plano — Detalhes — Anexo — Plano (cycle) — Plano2 (named "Plano" too)
    // eu — Lado — Plano (roped to me and to a sibling), eu — Comum — Outro (shared note)
    // eu — Outro (terminal) — Dele (its note)
    let nodes = [
        node("eu", NodeKind::Terminal, &["Plano", "Outro", "Lado", "Comum"]),
        node("Plano", NodeKind::Note, &["eu", "Detalhes", "Lado"]),
        node("Detalhes", NodeKind::Note, &["Plano", "Anexo"]),
        node("Anexo", NodeKind::Note, &["Detalhes", "Plano", "Plano2"]),
        Node { name: "Plano".into(), ..node("Plano2", NodeKind::Note, &["Anexo"]) },
        node("Lado", NodeKind::Note, &["eu", "Plano"]),
        node("Outro", NodeKind::Terminal, &["eu", "Dele", "Comum"]),
        node("Dele", NodeKind::Note, &["Outro"]),
        node("Comum", NodeKind::Note, &["eu", "Outro"]),
    ];
    let names = |from: &str| reachable(&nodes, from).into_iter().map(|n| n.name).collect::<Vec<_>>();
    assert_eq!(names("eu"), ["Outro", "Plano", "Lado", "Comum", "Detalhes", "Anexo", "Plano"], "nearest first; a terminal's notes are not mine");
    assert_eq!(names("Outro"), ["eu", "Dele", "Comum"], "the shared note brings none of my notes along");
    assert!(names("sumiu").is_empty(), "a terminal the canvas lost reaches nothing");
    let shared = Mutex::new(Shared { nodes: nodes.to_vec(), ..Default::default() });
    let list = describe(&nodes, "eu", &shared);
    assert!(
        list.contains("Notas conectadas:\n  - \"Plano\"\n    - \"Detalhes\"\n      - \"Anexo\"\n        - \"Plano\"\n  - \"Lado\"\n  - \"Comum\""),
        "{list}"
    );
    assert!(describe(&nodes, "sumiu", &shared).contains("Nada conectado"));
    assert!(find(&nodes, "eu", "anexo", NodeKind::Note).is_ok());
    assert_eq!(find(&nodes, "eu", "Plano", NodeKind::Note).unwrap().id, "Plano", "the direct note, not the deep one");
    assert_eq!(note_tree(&nodes, "").len(), 7, "outside the canvas: every note, once");
    assert!(describe(&nodes, "", &shared).contains("  - \"Plano\"\n    - \"Detalhes\""), "still a tree from outside");
}

#[test]
fn a_hook_waits_for_the_apps_answer_only_on_a_permission() {
    let shared = Mutex::new(Shared::default());
    let (sender, commands) = mpsc::channel();
    let quick = json!({ "cmd": "hook", "from": "t1", "payload": { "hook_event_name": "Stop" } });
    assert_eq!(handle(&quick, &shared, &sender), Ok(String::new()));
    assert!(matches!(commands.try_recv(), Ok(UiCommand::Hook { reply: None, .. })));
    let app = std::thread::spawn(move || {
        for answer in [Some("sim"), None] {
            let Ok(UiCommand::Hook { reply: Some(reply), from, .. }) = commands.recv() else { panic!("a permission waits") };
            assert_eq!(from, "t1");
            // None: the app lets it go, and the hook ends at once
            if let Some(answer) = answer {
                reply.send(Ok(answer.into())).unwrap();
            }
        }
    });
    let asks = json!({ "cmd": "hook", "from": "t1", "payload": { "hook_event_name": "PermissionRequest" }, "wait": 30 });
    assert_eq!(handle(&asks, &shared, &sender), Ok("sim".into()));
    let start = Instant::now();
    assert_eq!(handle(&asks, &shared, &sender), Ok(String::new()));
    assert!(start.elapsed() < Duration::from_secs(5));
    app.join().unwrap();
}

#[test]
fn an_ide_path_stays_inside_its_project() {
    let root = if cfg!(windows) { Path::new(r"C:\Users\eu\Proj") } else { Path::new("/home/eu/Proj") };
    assert_eq!(ide_path(root, "src/a.rs").unwrap(), root.join("src").join("a.rs"));
    assert!(ide_path(root, "../x").is_err() && ide_path(root, " ").is_err());
    let sibling = root.with_file_name("Proj-segredo").join("a.rs");
    assert!(ide_path(root, &sibling.to_string_lossy()).is_err(), "a folder that only starts with the project's name");
    let other_case = root.join("src").join("a.rs").to_string_lossy().to_lowercase();
    // the same file on Windows, in the project's spelling; another folder elsewhere
    assert_eq!(ide_path(root, &other_case).ok(), cfg!(windows).then(|| root.join("src").join("a.rs")));
}
