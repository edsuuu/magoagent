use super::flow::*;
use super::flow_act::merge_branches;
use super::*;

fn git(folder: &Path, args: &[&str]) -> String {
    crate::editor::run_git(folder, args).unwrap_or_else(|why| panic!("git {args:?}: {why}"))
}

/// A repository on `main` with a.txt, and a branch per `(name, file, text)` made from it.
fn repo(branches: &[(&str, &str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mago-fluxo-{}", new_id()));
    fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.name", "teste"]);
    git(&dir, &["config", "user.email", "teste@example.com"]);
    fs::write(dir.join("a.txt"), "um\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "inicio"]);
    for (name, file, text) in branches {
        git(&dir, &["checkout", "-q", "-b", name]);
        fs::write(dir.join(file), text).unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", name]);
        git(&dir, &["checkout", "-q", "main"]);
    }
    dir
}

fn app_in(dir: &Path) -> DrawingApp {
    DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() }
}

fn flow_act(app: &mut DrawingApp, args: Value) -> Result<String, String> {
    app.act("flow", &args)
}

/// Writes the stage's note and advances with ok.
fn pass(app: &mut DrawingApp, id: &str, extra: Value) -> Result<String, String> {
    let stage = app.tasks.list.iter().find(|t| t.id == id).and_then(|t| t.flow.as_ref()).unwrap().stage;
    app.act_binder("write", &format!("{id}-{}", STAGES[stage].code), "feito").unwrap();
    let mut args = json!({ "action": "advance", "id": id, "result": "ok", "note": "ok" });
    args.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    flow_act(app, args)
}

fn state_of(app: &DrawingApp, id: &str) -> TaskState {
    app.tasks.list.iter().find(|t| t.id == id).unwrap().state
}

#[test]
fn the_twelve_stages_come_in_order() {
    let codes: Vec<&str> = STAGES.iter().map(|s| s.code).collect();
    assert_eq!(
        codes,
        ["planejar", "analisar", "arquitetar", "duvidas", "implementar", "testes", "usos", "padrao", "ponytail", "navegador", "review-agente", "review-humano"]
    );
    // only the doubts and the human review wait for the person; 6 to 12 go back to 5, 1 to 4 repeat
    let waits: Vec<usize> = STAGES.iter().enumerate().filter(|(_, s)| s.person).map(|(i, _)| i + 1).collect();
    assert_eq!(waits, [4, 12]);
    assert!(STAGES.iter().all(|s| s.person == (s.state == TaskState::WaitingApproval)));
    assert!(STAGES[..5].iter().all(|s| s.back.is_none()) && STAGES[5..].iter().all(|s| s.back == Some(4)));
}

#[test]
fn ok_walks_to_done_and_failures_go_back_or_block() {
    let mut flow = Flow::new("x", 0);
    for index in 0..12 {
        assert_eq!((flow.stage, flow.state()), (index, STAGES[index].state));
        let step = flow.advance(true, "", "a", 1);
        assert_eq!(step, if index == 11 { Move::Finished } else { Move::Next });
    }
    assert_eq!((flow.state(), flow.current().is_none()), (TaskState::Done, true));
    assert_eq!(flow.advance(true, "", "a", 2), Move::Finished);
    // a failure in 6 goes back to 5, and the run says why
    let mut flow = Flow { stage: 5, runs: vec![Run { stage: 5, ..Default::default() }], ..Flow::new("x", 0) };
    assert_eq!(flow.advance(false, "quebrou", "a", 1), Move::Back);
    assert_eq!((flow.stage, flow.fails(5)), (4, 1));
    assert_eq!((flow.runs[0].result.as_str(), flow.runs[0].note.as_str()), ("fail", "quebrou"));
    // 1 to 4 repeat
    let mut flow = Flow::new("x", 0);
    assert_eq!((flow.advance(false, "x", "", 1), flow.stage), (Move::Repeat, 0));
    // the third failure in a row in a stage blocks, even with the implementation passing in between
    let mut flow = Flow { stage: 5, runs: vec![Run { stage: 5, ..Default::default() }], ..Flow::new("x", 0) };
    assert_eq!(flow.advance(false, "1", "", 1), Move::Back);
    assert_eq!(flow.advance(true, "", "", 2), Move::Next);
    assert_eq!(flow.advance(false, "2", "", 3), Move::Back);
    assert_eq!(flow.advance(true, "", "", 4), Move::Next);
    assert_eq!((flow.advance(false, "3", "", 5), flow.stage), (Move::Blocked, 5));
    // after blocking the count starts over
    assert_eq!(flow.advance(false, "4", "", 6), Move::Back);
    // passing the stage clears its failures
    let mut flow = Flow::new("x", 0);
    flow.advance(false, "", "", 1);
    flow.advance(false, "", "", 2);
    flow.advance(true, "", "", 3);
    assert_eq!(flow.fails(0), 0);
}

#[test]
fn the_flow_tool_walks_the_stages_asking_for_notes_and_keeping_lessons() {
    let dir = std::env::temp_dir().join(format!("mago-fluxo-app-{}", new_id()));
    fs::create_dir_all(&dir).unwrap();
    let mut app = app_in(&dir);
    app.events_tick();
    let started = flow_act(&mut app, json!({ "action": "start", "title": "Carrinho", "description": "com Pix" })).unwrap();
    assert!(started.contains("Fluxo t1 iniciado") && started.contains("etapa 1/12: Planejar"), "{started}");
    assert_eq!(state_of(&app, "t1"), TaskState::InProgress);
    // no note, no advance
    let refused = flow_act(&mut app, json!({ "action": "advance", "id": "t1", "result": "ok" })).unwrap_err();
    assert!(refused.contains("Falta a nota `t1-planejar`"), "{refused}");
    assert!(flow_act(&mut app, json!({ "action": "advance", "id": "t1", "result": "fail" })).unwrap_err().contains("motivo"));
    assert!(app.act("flow", &json!({ "action": "advance", "id": "t9", "result": "ok" })).is_err());
    let next = pass(&mut app, "t1", json!({})).unwrap();
    assert!(next.contains("segue para a etapa 2 (Analisar codebase)") && next.contains("Leia antes a nota `t1-planejar`"), "{next}");
    pass(&mut app, "t1", json!({})).unwrap();
    pass(&mut app, "t1", json!({})).unwrap();
    // the doubts wait for the person, and that is a demand
    assert_eq!(state_of(&app, "t1"), TaskState::WaitingApproval);
    assert!(app.task_demands().iter().any(|d| d.what.contains("t1")));
    pass(&mut app, "t1", json!({})).unwrap();
    let tests = pass(&mut app, "t1", json!({ "branches": ["mago/a"] })).unwrap();
    assert_eq!(state_of(&app, "t1"), TaskState::InTest);
    assert!(tests.contains("`cargo test`"), "{tests}");
    // the standardization stage names the file to read, the next two cite the skills for whoever has them
    let skill = dir.join("SKILL.md");
    fs::write(&skill, "regras").unwrap();
    app.prefs.style_skill = skill.to_string_lossy().into_owned();
    pass(&mut app, "t1", json!({})).unwrap();
    let style = pass(&mut app, "t1", json!({})).unwrap();
    assert!(style.contains(&format!("leia o arquivo {}", skill.display())), "{style}");
    assert!(app.act("flow", &json!({ "action": "status", "id": "t1" })).unwrap().contains("[>] 8. Padronização"));
    let ponytail = pass(&mut app, "t1", json!({})).unwrap();
    assert!(ponytail.contains("Revisor ponytail") && ponytail.contains("/ponytail-review"), "{ponytail}");
    pass(&mut app, "t1", json!({})).unwrap();
    let review = pass(&mut app, "t1", json!({})).unwrap();
    assert!(review.contains("/code-review"), "{review}");
    // a failure goes back to the implementation and leaves its line in the project's lessons
    let back = flow_act(&mut app, json!({ "action": "advance", "id": "t1", "result": "fail", "note": "faltou teste\ndo total" })).unwrap();
    assert!(back.contains("volta à etapa 5 (Delegação e implementação)"), "{back}");
    assert_eq!((state_of(&app, "t1"), app.tasks.list[0].note.as_str()), (TaskState::InProgress, "faltou teste\ndo total"));
    let lessons = app.act_binder("read", "licoes-projeto", "").unwrap();
    assert!(lessons.contains("[[t1-review-agente]] etapa 11 (Code review do agente): faltou teste do total"), "{lessons}");
    // 5 -> 6 -> fail, three times in a row in the same stage: blocked, a demand
    for round in 0..3 {
        pass(&mut app, "t1", json!({})).unwrap();
        let said = flow_act(&mut app, json!({ "action": "advance", "id": "t1", "result": "fail", "note": format!("rodada {round}") })).unwrap();
        assert_eq!(said.contains("falhas seguidas"), round == 2, "{said}");
    }
    assert_eq!(state_of(&app, "t1"), TaskState::Blocked);
    assert!(app.task_demands().iter().any(|d| d.what.contains("rodada 2")));
    let list = app.act("flow", &json!({ "action": "status" })).unwrap();
    assert!(list.contains("t1 · Testes unitários e de integração 6/12 · Bloqueada"), "{list}");
    assert!(app.act("flow", &json!({ "action": "start" })).is_err());
    // a task from before the flow is not one
    app.act("tasks", &json!({ "action": "create", "title": "avulsa" })).unwrap();
    assert!(app.act("flow", &json!({ "action": "advance", "id": "t2", "result": "ok" })).unwrap_err().contains("não é um fluxo"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_last_stage_joins_the_branches_and_a_conflict_blocks_without_half_a_merge() {
    // two branches that don't meet: both join, each with its own merge commit
    let project = repo(&[("mago/a", "b.txt", "b\n"), ("mago/b", "c.txt", "c\n")]);
    let (data, branches) = (std::env::temp_dir().join(format!("mago-fluxo-dados-{}", new_id())), ["mago/a".to_string(), "mago/b".to_string()]);
    fs::create_dir_all(&data).unwrap();
    let mut app = app_in(&data);
    app.events_tick();
    let folder = project.to_string_lossy().to_string();
    flow_act(&mut app, json!({ "action": "start", "title": "Junta", "folder": folder })).unwrap();
    for _ in 0..11 {
        pass(&mut app, "t1", json!({ "branches": branches })).unwrap();
    }
    assert_eq!(state_of(&app, "t1"), TaskState::WaitingApproval);
    let done = pass(&mut app, "t1", json!({})).unwrap();
    assert!(done.contains("Ramos juntos: mago/a, mago/b") && done.contains("projeto-projeto"), "{done}");
    assert_eq!(state_of(&app, "t1"), TaskState::Done);
    assert!(project.join("b.txt").is_file() && project.join("c.txt").is_file());
    assert_eq!(git(&project, &["log", "--merges", "--oneline"]).lines().count(), 2);
    assert!(flow_act(&mut app, json!({ "action": "advance", "id": "t1", "result": "ok" })).unwrap_err().contains("já terminou"));

    // the same file changed two ways: the merge is undone and the task blocked, with the files
    let project = repo(&[("mago/x", "a.txt", "x\n"), ("mago/y", "a.txt", "y\n"), ("mago/z", "z.txt", "z\n")]);
    let before = git(&project, &["rev-parse", "HEAD"]);
    let folder = project.to_string_lossy().to_string();
    let branches = ["mago/z", "mago/x", "mago/y"];
    flow_act(&mut app, json!({ "action": "start", "title": "Conflito", "folder": folder })).unwrap();
    for _ in 0..11 {
        pass(&mut app, "t2", json!({ "branches": branches })).unwrap();
    }
    let why = pass(&mut app, "t2", json!({})).unwrap_err();
    assert!(why.contains("Não juntei os ramos, nada foi mudado") && why.contains("a.txt"), "{why}");
    assert_eq!(state_of(&app, "t2"), TaskState::Blocked);
    assert!(app.tasks.list[1].note.contains("conflito ao juntar mago/y"));
    // nothing half done: back where it was, clean, no merge in progress (not even the first, clean branch)
    assert_eq!(git(&project, &["rev-parse", "HEAD"]), before);
    assert!(git(&project, &["status", "--porcelain"]).trim().is_empty());
    assert!(!project.join(".git").join("MERGE_HEAD").exists() && !project.join("z.txt").exists());
    // a branch that doesn't exist, and a dirty folder, are refused too
    assert!(merge_branches(&project, &["mago/nao-existe".into()]).unwrap_err().contains("não juntou mago/nao-existe"));
    fs::write(project.join("a.txt"), "sujo\n").unwrap();
    assert!(merge_branches(&project, &["mago/z".into()]).unwrap_err().contains("não salvas"));
    for dir in [project, data] {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn a_tasks_file_from_before_the_flow_still_reads() {
    let old: Vec<Task> = serde_json::from_str(r#"[{ "id": "t1", "title": "x", "state": "in_progress", "owner": "a", "deps": ["t0"] }]"#).unwrap();
    assert_eq!((old[0].flow.clone(), old[0].state), (None, TaskState::InProgress));
    // and one with a flow keeps it
    let task = Task { flow: Some(Flow::new("p", 5)), ..old[0].clone() };
    let again: Task = serde_json::from_str(&serde_json::to_string(&task).unwrap()).unwrap();
    assert_eq!(again, task);
}
