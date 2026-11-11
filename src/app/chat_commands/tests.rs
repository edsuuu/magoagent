use super::*;

fn temp() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mago-slash-{}", uuid::Uuid::new_v4().simple()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_boxs_text_is_a_local_command_or_goes_to_the_agent() {
    assert_eq!(parse("/login", "claude"), Some(Ok(Local::Login(String::new()))));
    assert_eq!(parse("  /Login  sso ", "claude"), Some(Ok(Local::Login("sso".into()))));
    assert_eq!(parse("/model opus", "claude"), Some(Ok(Local::Model("opus".into()))));
    assert_eq!(parse("/clear", "codex"), Some(Ok(Local::Clear)));
    assert!(matches!(parse("/doctor", "codex"), Some(Err(_))));
    for pass in ["/compact", "/review 12", "/meu-comando x", "oi /login", "/usr/bin/ls", "/"] {
        assert_eq!(parse(pass, "claude"), None, "{pass}");
    }
}

#[test]
fn each_command_builds_its_cli_arguments() {
    let args = |agent, l| cli_args(agent, &l).unwrap();
    assert_eq!(args("claude", Local::Login(String::new())), ["auth", "login"]);
    assert_eq!(args("claude", Local::Login("console".into())), ["auth", "login", "--console"]);
    assert_eq!(args("claude", Local::Login("SSO".into())), ["auth", "login", "--sso"]);
    assert!(cli_args("claude", &Local::Login("x".into())).is_err());
    assert_eq!(args("claude", Local::Logout), ["auth", "logout"]);
    assert_eq!(args("claude", Local::Status), ["auth", "status", "--json"]);
    assert_eq!(args("claude", Local::Mcp), ["mcp", "list"]);
    assert_eq!(args("claude", Local::Doctor), ["doctor"]);
    assert_eq!(args("claude", Local::Update), ["update"]);
    assert_eq!(args("codex", Local::Login(String::new())), ["login"]);
    assert_eq!(args("codex", Local::Status), ["login", "status"]);
    assert!(cli_args("claude", &Local::Clear).is_err());
}

#[test]
fn the_auth_status_json_says_who_is_in() {
    let (ok, text) = describe_status(r#"{"loggedIn":true,"authMethod":"claude.ai","email":"a@b.com","orgName":"Acme","subscriptionType":"max"}"#);
    assert!(ok);
    assert_eq!(text, "Logado como a@b.com · plano max · Acme · acesso claude.ai");
    let (ok, text) = describe_status(r#"{"loggedIn":false,"authMethod":"none","apiProvider":"firstParty"}"#);
    assert!(!ok && text.contains("/login"));
    assert_eq!(describe_status("não é json").1, "não é json");
}

#[test]
fn the_menu_filters_and_lists_the_custom_commands() {
    let all = catalog("claude");
    let names = |q: &str| filter(&all, q).into_iter().map(|e| e.name).collect::<Vec<_>>();
    assert_eq!(names("lo"), ["login", "logout"]);
    assert_eq!(names("m")[..3], ["mcp", "model", "compact"], "starts-with first, then the ones that hold it");
    assert!(names("").len() == all.len() && names("zzz").is_empty());
    assert!(!catalog("codex").iter().any(|e| e.name == "doctor"));

    let (cwd, home) = (temp(), temp());
    fs::create_dir_all(cwd.join(".claude/commands/time")).unwrap();
    fs::write(cwd.join(".claude/commands/deploy.md"), "---\ndescription: Sobe o app\n---\nrode tudo").unwrap();
    fs::write(cwd.join(".claude/commands/time/revisar.md"), "# Revisa o PR\nmais").unwrap();
    fs::create_dir_all(home.join(".claude/commands")).unwrap();
    fs::write(home.join(".claude/commands/deploy.md"), "outra").unwrap();
    fs::write(home.join(".claude/commands/resumo.md"), "Resume o dia").unwrap();
    fs::create_dir_all(home.join(".claude/skills/pdf")).unwrap();
    fs::write(home.join(".claude/skills/pdf/SKILL.md"), "---\nname: pdf\ndescription: Mexe em PDF\n---\n").unwrap();
    let found = discover(&cwd, Some(&home));
    let line = |n: &str| found.iter().find(|e| e.name == n).map(|e| e.desc.clone());
    assert_eq!(line("deploy").as_deref(), Some("Sobe o app (projeto)"), "the folder's wins over the home's");
    assert_eq!(line("time:revisar").as_deref(), Some("Revisa o PR (projeto)"));
    assert_eq!(line("resumo").as_deref(), Some("Resume o dia (pessoal)"));
    assert_eq!(line("pdf").as_deref(), Some("Skill · Mexe em PDF (pessoal)"));
    assert_eq!(found.len(), 4);
    let _ = (fs::remove_dir_all(cwd), fs::remove_dir_all(home));
}

#[test]
fn model_and_effort_resolve_to_what_the_agent_is_told() {
    let all: Vec<Choice> = models("claude").into_iter().chain(more_models("claude")).collect();
    assert_eq!(resolve_model("opus", &all), "claude-opus-5-5");
    assert_eq!(resolve_model("Sonnet 4.6", &all), "claude-sonnet-4-6");
    assert_eq!(resolve_model("padrão", &all), "");
    assert_eq!(resolve_model("opus[1m]", &all), "opus[1m]");
    let list = efforts("claude", "");
    assert_eq!(resolve_effort("médio", &list).as_deref(), Some("medium"));
    assert_eq!(resolve_effort("high", &list).as_deref(), Some("high"));
    assert_eq!(resolve_effort("padrao", &list).as_deref(), Some(""));
    assert_eq!(resolve_effort("enorme", &list), None);
}

#[test]
fn clear_starts_a_new_session_and_the_commands_change_the_chat() {
    let mut app = DrawingApp::default();
    let ctx = Context::default();
    let installed = [Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
    app.create_chat(&crate::app::chat_direct::NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &installed);
    let id = app.chat_agent.clone().unwrap();
    app.chats[0].session = Some("s1".into());
    app.chat_transcript(&id).items.push(ChatItem::Agent("oi".into()));
    assert!(app.chat_submit(&ctx, &id, "/model sonnet", "/model sonnet"));
    assert!(app.chat_submit(&ctx, &id, "/effort alto", "/effort alto"));
    assert_eq!((app.chats[0].model.as_str(), app.chats[0].effort.as_str()), ("claude-sonnet-5-5", "high"));
    assert!(app.chat_submit(&ctx, &id, "/clear", "/clear"));
    assert_eq!(app.chats[0].session, None);
    assert!(matches!(&app.transcripts[&id].items[..], [ChatItem::System(note)] if note.starts_with("Conversa nova")));
    assert!(app.chat_submit(&ctx, &id, "/help", "/help"));
    assert!(matches!(app.transcripts[&id].items.last(), Some(ChatItem::System(h)) if h.contains("/login") && h.contains("/clear")));
    // the menu opens on a slash with no space after it
    app.chat_draft = "/lo".into();
    assert_eq!(app.slash_entries().map(|l| l.len()), Some(2));
    app.chat_draft = "/login sso".into();
    assert!(app.slash_entries().is_none());
}

#[test]
fn the_logins_address_is_read_from_what_it_prints() {
    let out = "Opening browser to sign in…\nIf the browser didn't open, visit: https://claude.ai/oauth/authorize?code=true&client_id=x.\nPaste code here if prompted > ";
    assert_eq!(first_url(out).as_deref(), Some("https://claude.ai/oauth/authorize?code=true&client_id=x"));
    assert_eq!(first_url("nada"), None);
}

#[test]
fn a_failed_message_is_kept_to_be_sent_again_after_the_login() {
    let mut app = DrawingApp::default();
    app.slash.sent.insert("c".into(), "roda os testes".into());
    app.slash.retry.insert("c".into());
    // Mandar de novo clears the offer even when the chat is gone
    app.chat_retry(&Context::default(), "c");
    assert!(!app.slash.retry.contains("c"));
}
