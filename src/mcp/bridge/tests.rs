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
fn a_hook_whose_relay_hangs_up_tells_the_app() {
    let (sender, commands) = mpsc::channel();
    let asks = json!({ "cmd": "hook", "from": "ext:s1", "payload": { "hook_event_name": "PermissionRequest", "session_id": "s1" }, "wait": 30 });
    let start = Instant::now();
    // a conexão do relay caiu: nada a imprimir, e o app recebe o aviso com o número do pedido
    assert_eq!(relay_hook(&asks, &sender, &|| true), Ok(String::new()));
    assert!(start.elapsed() < Duration::from_secs(5));
    let Ok(UiCommand::Hook { payload: asked, reply: Some(_), .. }) = commands.try_recv() else { panic!("o pedido vai ao app") };
    let number = asked["mago_request"].as_str().unwrap().to_string();
    assert!(!number.is_empty());
    let Ok(UiCommand::Hook { payload: gone, reply: None, from }) = commands.try_recv() else { panic!("e depois o aviso") };
    assert_eq!(
        (from.as_str(), hook_name(&gone), gone["mago_request"].as_str(), gone["session_id"].as_str()),
        ("ext:s1", HOOK_GONE, Some(number.as_str()), Some("s1"))
    );
    // quem não espera a pessoa não ganha número nem é vigiado
    let quick = json!({ "cmd": "hook", "from": "ext:s1", "payload": { "hook_event_name": "Stop" } });
    assert_eq!(relay_hook(&quick, &sender, &|| true), Ok(String::new()));
    let Ok(UiCommand::Hook { payload: stop, reply: None, .. }) = commands.try_recv() else { panic!("o evento vai ao app") };
    assert!(stop.get("mago_request").is_none() && commands.try_recv().is_err());
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

#[test]
fn the_vault_types_only_on_the_site_itself_or_its_subdomains() {
    let portal = |id: &str, url: &str| Node { id: id.into(), kind: NodeKind::Portal, name: id.into(), text: url.into(), links: vec![], maestro: false };
    let nodes = vec![
        Node {
            id: "t".into(),
            kind: NodeKind::Terminal,
            name: "t".into(),
            text: String::new(),
            links: vec!["a".into(), "b".into(), "c".into()],
            maestro: false,
        },
        portal("a", "https://evil.com/?x=github.com"),
        portal("b", "https://github.com.evil.com/login"),
        portal("c", "https://user@evil.com?q=a.github.com"),
    ];
    // the only portal reached is on another site: no fallback to it
    assert!(portal_of_site(&nodes, "t", "github.com").is_err());
    let mut nodes = nodes;
    nodes.push(portal("d", "https://gist.github.com:443/x"));
    nodes[0].links.push("d".into());
    assert_eq!(portal_of_site(&nodes, "t", "GitHub.com").unwrap().id, "d");
}

#[test]
fn a_terminal_that_is_no_maestro_acts_only_on_its_own_trees() {
    let node = |id: &str, kind, maestro, links: &[&str]| Node {
        id: id.into(),
        kind,
        name: id.into(),
        text: String::new(),
        links: links.iter().map(|l| l.to_string()).collect(),
        maestro,
    };
    let shared = Mutex::new(Shared::default());
    shared.lock().unwrap().nodes = vec![
        node("Filho", NodeKind::Terminal, false, &["Minha"]),
        node("Chefe", NodeKind::Terminal, true, &[]),
        node("Minha", NodeKind::Tree, false, &["Filho"]),
        node("Alheia", NodeKind::Tree, false, &[]),
    ];
    let (sender, commands) = mpsc::channel();
    let run = |from: &str, action: &str, args: Value| handle(&json!({ "cmd": "act", "from": from, "action": action, "args": args }), &shared, &sender);
    // what changes the whole app, or names a target anywhere, is refused before it reaches the app
    for action in ["set_setting", "update", "close_window", "terminal_menu", "chat_send"] {
        assert!(run("Filho", action, json!({})).unwrap_err().contains("Maestro"), "{action}");
    }
    assert!(commands.try_recv().is_err());
    // a tree it is not roped to is refused; its own is aimed at, named or not
    assert!(run("Filho", "file_tree", json!({ "action": "list", "tree": "Alheia" })).is_err());
    assert!(commands.try_recv().is_err());
    let aimed = |from: &str, args: Value| {
        std::thread::scope(|scope| {
            let ask = scope.spawn(|| run(from, "file_tree", args));
            let UiCommand::Act { args, reply, .. } = commands.recv_timeout(Duration::from_secs(5)).unwrap() else { panic!("not an act") };
            let _ = reply.send(Ok("ok".into()));
            assert_eq!(ask.join().unwrap(), Ok("ok".into()));
            args["tree"].as_str().map(String::from)
        })
    };
    assert_eq!(aimed("Filho", json!({ "action": "list", "tree": "minha" })).as_deref(), Some("Minha"));
    assert_eq!(aimed("Filho", json!({ "action": "list" })).as_deref(), Some("Minha"));
    // a Maestro keeps the whole canvas
    assert_eq!(aimed("Chefe", json!({ "action": "list", "tree": "Alheia" })).as_deref(), Some("Alheia"));
}

#[test]
fn a_panic_holding_the_shared_state_does_not_poison_the_next_request() {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let held = shared.clone();
    let _ = std::thread::spawn(move || {
        let _guard = held.lock().unwrap();
        panic!("boom");
    })
    .join();
    assert!(shared.is_poisoned());
    assert!(locked(&shared).asking.is_empty());
}
