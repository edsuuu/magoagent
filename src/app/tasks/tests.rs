use super::*;

fn task(id: &str, state: TaskState, deps: &[&str]) -> Task {
    Task { id: id.into(), title: format!("tarefa {id}"), state, deps: deps.iter().map(|d| d.to_string()).collect(), ..Default::default() }
}

#[test]
fn task_states_keep_their_ids_and_names() {
    for (state, code, label) in STATES {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(code));
        assert_eq!((state.code(), state.label()), (code, label));
        assert_eq!(TaskState::parse(code), Ok(state));
        assert_eq!(TaskState::parse(&label.to_uppercase()), Ok(state));
    }
    assert!(TaskState::parse("talvez").unwrap_err().contains("in_progress"));
    // a file from before a field existed still reads
    let old: Task = serde_json::from_str(r#"{ "id": "t1", "title": "x", "state": "blocked" }"#).unwrap();
    assert_eq!((old.state, old.deps.len(), old.by_deps), (TaskState::Blocked, 0, false));
}

#[test]
fn task_transitions_follow_the_rules() {
    let tasks = vec![task("t1", TaskState::InProgress, &[]), task("t2", TaskState::Pending, &["t1"]), task("t3", TaskState::Done, &[])];
    // a dependency not done: in progress blocks, saying on what
    assert_eq!(transition(&tasks, &tasks[1], TaskState::InProgress), Ok((TaskState::Blocked, "depende de t1 (em andamento)".into())));
    assert_eq!(transition(&tasks, &tasks[1], TaskState::InReview), Ok((TaskState::InReview, String::new())));
    assert_eq!(transition(&tasks, &tasks[0], TaskState::Done), Ok((TaskState::Done, String::new())));
    // final: only reopening
    assert!(transition(&tasks, &tasks[2], TaskState::InProgress).unwrap_err().contains("só reabre para pendente"));
    assert_eq!(transition(&tasks, &tasks[2], TaskState::Pending), Ok((TaskState::Pending, String::new())));
    // all its dependencies done: a task blocked by them is free, one blocked by hand isn't
    let mut tasks = tasks;
    tasks[1].state = TaskState::Blocked;
    tasks[1].by_deps = true;
    tasks.push(Task { by_deps: false, ..task("t4", TaskState::Blocked, &["t3"]) });
    assert!(unblocked(&tasks).is_empty());
    tasks[0].state = TaskState::Done;
    assert_eq!(unblocked(&tasks), ["t2"]);
}

#[test]
fn task_links_refuse_cycles_and_missing_tasks() {
    let mut tasks = vec![task("t1", TaskState::Pending, &[]), task("t2", TaskState::Pending, &["t1"]), task("t3", TaskState::Pending, &["t2"])];
    assert_eq!(check_links(&tasks, "t4", "t1", &["t3".into()]), Ok(()));
    assert_eq!(check_links(&tasks, "t4", "t9", &[]), Err("A tarefa-mãe t9 não existe.".into()));
    assert_eq!(check_links(&tasks, "t4", "", &["t8".into()]), Err("A dependência t8 não existe.".into()));
    assert!(check_links(&tasks, "t1", "", &["t1".into()]).unwrap_err().contains("si mesma"));
    assert_eq!(check_links(&tasks, "t1", "", &["t3".into()]), Err("Ciclo de dependências: t1 → t3 → t2 → t1.".into()));
    tasks[1].parent = "t1".into();
    assert_eq!(check_links(&tasks, "t1", "t2", &[]), Err("Ciclo de tarefas-mãe: t1 → t2 → t1.".into()));
    assert!(check_links(&tasks, "t1", "t1", &[]).is_err());
}

#[test]
fn task_attention_points_come_from_the_data() {
    let now = now_ms();
    let mut tasks = vec![task("t1", TaskState::Failed, &[]), task("t2", TaskState::Cancelled, &[]), task("t3", TaskState::Blocked, &["t1", "t2"])];
    for t in &mut tasks {
        t.updated = now;
    }
    assert_eq!(task_points(&tasks, &tasks[2], now, None), ["a dependência t1 falhou", "a dependência t2 foi cancelada"], "no owner: nothing about one");
    tasks[2].updated = now - 20 * 60_000;
    (tasks[2].owner, tasks[2].owner_name) = ("a".into(), "Front".into());
    assert_eq!(task_points(&tasks, &tasks[2], now, None)[0], "parada há 20 min");
    assert!(task_points(&tasks, &tasks[2], now, None).contains(&"o responsável Front não existe mais".to_string()));
    assert!(task_points(&tasks, &tasks[2], now, Some(false)).contains(&"o responsável Front está encerrado".to_string()));
    for id in ["t4", "t5", "t6"] {
        tasks.push(Task { owner: "a".into(), ..task(id, TaskState::InProgress, &[]) });
    }
    assert!(task_points(&tasks, &tasks[2], now, Some(true)).contains(&"Front tem 3 tarefas em andamento".to_string()));
    assert!(task_points(&tasks, &tasks[3], now, Some(true)).len() == 2, "stale (never updated) and busy, nothing invented");
}

#[test]
fn tasks_are_made_moved_kept_and_handed_to_their_agent() {
    let dir = std::env::temp_dir().join(format!("mago-tarefas-{}", new_id()));
    fs::create_dir_all(&dir).unwrap();
    let mut app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };
    app.events_tick();
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    e.terminal = Some(TerminalSpec { name: "Front".into(), command: "claude".into(), ..Default::default() });
    let front = e.id.clone();
    app.scene.elements.push(e);
    let made = app.act("tasks", &json!({ "action": "create", "title": "API de login", "owner": "front" })).unwrap();
    assert_eq!(made, "Tarefa t1 criada para Front: API de login.");
    let made = app.act("tasks", &json!({ "action": "create", "title": "Tela de login", "owner": "Front", "deps": ["t1"], "send": true })).unwrap();
    assert!(made.contains("t2") && made.contains("Não entreguei: Front não está rodando"), "{made}");
    assert!(app.act("tasks", &json!({ "action": "create", "title": "x", "parent": "t9" })).unwrap_err().contains("t9"));
    assert!(app.act("tasks", &json!({ "action": "create", "title": "x", "owner": "Ninguém" })).is_err());
    assert!(app.act("tasks", &json!({ "action": "update", "id": "t1", "deps": ["t2"] })).unwrap_err().starts_with("Ciclo de dependências"));
    // started before its dependency is done: blocked, a demand with why
    let said = app.act("tasks", &json!({ "action": "update", "id": "t2", "state": "in_progress" })).unwrap();
    assert!(said.contains("Pendente → Bloqueada (depende de t1 (pendente))"), "{said}");
    let demands = app.demands();
    let d = demands.iter().find(|d| d.kind == DemandKind::Task).unwrap();
    assert_eq!((d.id.as_str(), d.what.as_str()), (front.as_str(), "tarefa t2 \"Tela de login\" bloqueada: depende de t1 (pendente)"));
    assert!(d.points.contains(&"o responsável Front está encerrado".to_string()), "{:?}", d.points);
    assert!(app.demand_line().unwrap().contains("(1 tarefa)"));
    // the agent says where it stands; its dependency done frees it
    app.act("tasks", &json!({ "action": "update", "id": "t1", "state": "in_progress", "caller": front })).unwrap();
    let said = app.act("tasks", &json!({ "action": "update", "id": "t1", "state": "done", "artifacts": ["src/login.rs"] })).unwrap();
    assert!(said.contains("A tarefa t2 voltou a pendente"), "{said}");
    assert!(app.act("tasks", &json!({ "action": "update", "id": "t1", "state": "failed" })).unwrap_err().contains("só reabre"));
    let list = app.act("tasks", &json!({ "action": "list" })).unwrap();
    let lines: Vec<&str> = list.lines().collect();
    assert!(lines[1].starts_with("t2 · Pendente · Front · Tela de login") && lines[2].starts_with("t1 · Concluída"), "{list}");
    assert!(app.act("tasks", &json!({ "action": "list", "state": "concluída" })).unwrap().starts_with("1 tarefa:"));
    assert_eq!(app.act("tasks", &json!({ "action": "list", "owner": "ninguém" })).unwrap(), "Nenhuma tarefa com esse filtro.");
    let got = app.act("tasks", &json!({ "action": "get", "id": "t1" })).unwrap();
    assert!(got.contains("Artefatos: src/login.rs") && got.contains("pediu: Maestro"), "{got}");
    // each creation and change is in the history
    let history: Vec<String> = app.agent_events.iter().filter(|e| e.kind == EventKind::Task).map(|e| e.what.clone()).collect();
    assert!(history.contains(&"tarefa t1 \"API de login\": criada por Maestro".to_string()) && history.len() >= 6, "{history:?}");
    // kept beside the drawing, read back by the next app
    let mut again = DrawingApp { autosave: app.autosave.clone(), ..Default::default() };
    again.tasks_tick();
    assert_eq!(again.tasks.list, app.tasks.list);
    // an unreadable file goes aside and the app says so
    fs::write(dir.join("tarefas.json"), "{ quebrado").unwrap();
    let mut broken = DrawingApp { autosave: app.autosave.clone(), ..Default::default() };
    broken.tasks_tick();
    assert!(broken.tasks.list.is_empty() && broken.status.contains("tarefas.json"));
    assert!(fs::read_dir(&dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("tarefas.json.bad-")));
    app.hooks.store = None;
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_task_goes_to_its_chat_with_its_id_and_how_to_report() {
    let mut app = DrawingApp::default();
    let installed = vec![Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
    app.create_chat(&crate::app::chat_direct::NewChat { agent: 0, cwd: "~/a".into(), mode: 0 }, &installed);
    let chat = app.chats[0].clone();
    app.act("tasks", &json!({ "action": "create", "title": "Revisar o PR", "owner": chat.name, "send": true })).unwrap();
    let (to, text) = &app.chat_outbox[0];
    assert_eq!(to, &chat.id);
    assert!(text.starts_with("Tarefa t1 para você: Revisar o PR.") && text.contains("action update, id t1"), "{text}");
}
