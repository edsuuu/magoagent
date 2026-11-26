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
    assert_eq!(args("claude", Local::Mcp(String::new())), ["mcp", "list"]);
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
    assert_eq!(names("m")[..3], ["mcp", "model", "permissions"], "starts-with first, then the ones that hold it");
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
    let found = discover(&cwd, Some(&home), &Known::default());
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
    // a fixed list, not the installed Claude Code's (that one changes with each update of it)
    let choice = |id: &str, name: &str| Choice { id: id.into(), name: name.into(), note: String::new() };
    let all = vec![choice("claude-opus-5-5", "Opus 5.5"), choice("claude-sonnet-4-6", "Sonnet 4.6"), choice("claude-haiku-4-5", "Haiku 4.5")];

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
    let installed = [Preset::new("Claude Code", "claude")];

    app.create_chat(&crate::app::chat::direct::NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &installed);
    let id = app.chat_agent.clone().unwrap();

    app.chats[0].session = Some("s1".into());
    app.chat_transcript(&id).items.push(ChatItem::Agent("oi".into()));
    assert!(app.chat_submit(&ctx, &id, "/model sonnet", "/model sonnet"));
    assert!(app.chat_submit(&ctx, &id, "/effort alto", "/effort alto"));
    // the agent's own default effort is kept as none (Claude Code's settings on this computer may say high)
    let high = if crate::app::chat::footer::agent_default("claude").1 == "high" { "" } else { "high" };

    assert_eq!((app.chats[0].model.as_str(), app.chats[0].effort.as_str()), ("claude-sonnet-5-5", high));
    assert!(app.chat_submit(&ctx, &id, "/clear", "/clear"));
    assert_eq!(app.chats[0].session, None);
    // headed by the new session's line, the conversation before it in the list (chat/direct/archive.rs)
    let new_session = crate::app::chat::direct::is_new_session_mark;

    assert!(matches!(&app.transcripts[&id].items[..], [ChatItem::System(mark), ChatItem::System(note)] if new_session(mark) && note.contains("Conversas")));
    assert!(app.chat_submit(&ctx, &id, "/help", "/help"));
    assert!(matches!(app.transcripts[&id].items.last(), Some(ChatItem::System(h)) if h.contains("/login") && h.contains("/clear")));
    // the menu opens on a slash with no space after it
    // the person's own skills come after (this computer's may hold "lo")
    app.chat_draft = "/lo".into();
    let first: Vec<String> = app.slash_entries().unwrap().into_iter().take(2).map(|entry| entry.name).collect();

    assert_eq!(first, ["login", "logout"]);
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

/// /btw asks on the side (Claude Code only, after the conversation began), /usage says the plan's 5 hours and 7 days,
/// /context opens its panel.
#[test]
fn btw_usage_and_context_are_the_chats_own() {
    assert_eq!(parse("/btw o que falta?", "claude"), Some(Ok(Local::Btw("o que falta?".into()))));
    assert!(matches!(parse("/btw x", "codex"), Some(Err(_))));
    assert!(matches!(parse("/context", "codex"), Some(Err(_))));
    assert_eq!(parse("/usage", "codex"), Some(Ok(Local::Usage)));
    assert!(btw_prompt("o que falta?").ends_with("o que falta?"));
    let mut app = DrawingApp::default();
    let ctx = Context::default();
    let installed = [Preset::new("Claude Code", "claude")];

    app.create_chat(&crate::app::chat::direct::NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &installed);
    let id = app.chat_agent.clone().unwrap();

    let last = |app: &DrawingApp| match app.transcripts[&id].items.last() {
        Some(ChatItem::System(note)) => note.clone(),
        other => panic!("{other:?}"),
    };

    assert!(app.chat_submit(&ctx, &id, "/btw o que falta?", "/btw o que falta?"));
    assert!(last(&app).contains("ainda não começou"), "{}", last(&app));
    assert!(!app.chat_cmds.contains_key(&id));
    // the plan not read yet: a read now
    app.usage_ui.probe.next = 99.0;
    assert!(app.chat_submit(&ctx, &id, "/usage", "/usage"));
    assert!(last(&app).contains("ainda não foi lido") && app.usage_ui.probe.next == 0.0);

    app.usage_ui.probe.slots[0].reading = Some(crate::app::usage::probe::Reading {
        plan: None,
        meters: vec![crate::app::usage::probe::Meter { title: "Sessão".into(), used: 0.16, resets_at: None }],
    });

    assert!(app.chat_submit(&ctx, &id, "/usage", "/usage"));
    assert_eq!(last(&app), "Consumo do plano\nClaude Code · 5 h 16%");
    app.prefs.chat_panel = false;
    assert!(app.chat_submit(&ctx, &id, "/context", "/context"));
    assert!(app.prefs.chat_panel);
}

/// The skills Claude Code said it has (its `init`) that no `.claude` folder shows come in the menu: a plugin's with what
/// its SKILL.md says, one synced from the account (a few folders down) by its name, Claude Code's own; none twice.
#[test]
fn the_skills_claude_code_said_it_has_come_in_the_menu() {
    let (cwd, home, plugin) = (temp(), temp(), temp());

    fs::create_dir_all(plugin.join("skills/brainstorming")).unwrap();
    fs::write(plugin.join("skills/brainstorming/SKILL.md"), "---\nname: brainstorming\ndescription: Antes de criar\n---\n").unwrap();
    fs::create_dir_all(home.join(".claude/skills/synced/conta/pdf")).unwrap();
    fs::write(home.join(".claude/skills/synced/conta/pdf/SKILL.md"), "---\ndescription: Mexe em PDF\n---\n").unwrap();
    fs::create_dir_all(home.join(".claude/skills/aws")).unwrap();
    fs::write(home.join(".claude/skills/aws/SKILL.md"), "---\ndescription: Nuvem\n---\n").unwrap();

    let init = serde_json::json!({
        "type": "system", "subtype": "init",
        "skills": ["aws", "superpowers:brainstorming", "anthropic-skills:pdf", "simplify"],
        "plugins": [{ "name": "superpowers", "path": plugin.display().to_string() }, { "name": "interno", "path": "builtin" }],
    });

    let known = Known::of_init(&init).unwrap();

    assert_eq!(known.plugins.len(), 1, "a built-in plugin has no folder");
    assert_eq!(Known::of_init(&serde_json::json!({ "type": "assistant" })), None);
    let found = discover(&cwd, Some(&home), &known);
    let line = |n: &str| found.iter().find(|e| e.name == n).map(|e| e.desc.clone());

    assert_eq!(line("superpowers:brainstorming").as_deref(), Some("Skill · Antes de criar"));
    assert_eq!(line("anthropic-skills:pdf").as_deref(), Some("Skill · Mexe em PDF"));
    assert_eq!(line("simplify").as_deref(), Some("Skill do Claude Code"));
    assert_eq!(line("aws").as_deref(), Some("Skill · Nuvem (pessoal)"), "the folder's line stays");
    assert_eq!(found.iter().filter(|e| e.name == "aws").count(), 1);
    let _ = (fs::remove_dir_all(cwd), fs::remove_dir_all(home), fs::remove_dir_all(plugin));
}

/// The arrows are taken by the box before the menu is drawn: the move is kept, and the menu scrolls to the pick once.
#[test]
fn the_arrows_leave_the_menu_a_scroll_to_the_pick() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let id = egui::Id::unique("caixa_teste");
    let entries: Vec<Entry> = (0..20).map(|n| Entry { name: format!("s{n}"), args: String::new(), desc: String::new() }).collect();
    let mut draft = "/s".to_string();

    let down = egui::Event::Key { key: Key::ArrowDown, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };

    // the box with the keys, as the chat draws it: the menu's keys first, then the box
    for events in [vec![], vec![down]] {
        let _ = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
            app.slash_keys(ui.ctx(), &mut draft, &entries, id);
            ui.add(egui::TextEdit::singleline(&mut draft).id(id)).request_focus();
        });
    }

    assert_eq!(app.slash.sel, 1);
    assert!(app.slash.moved.take(), "the menu is told to scroll");
    assert!(!app.slash.moved.get(), "once");
}

#[test]
fn the_interactive_commands_are_read_from_the_box() {
    assert_eq!(parse("/resume 6a9bb4c4", "claude"), Some(Ok(Local::Resume("6a9bb4c4".into()))));
    assert_eq!(parse("/permissions bypass", "claude"), Some(Ok(Local::Permissions("bypass".into()))));
    assert_eq!(parse("/config", "codex"), Some(Ok(Local::Config)));
    assert!(matches!(parse("/resume x", "codex"), Some(Err(_))), "a Codex thread is not resumed this way");
    assert_eq!(cli_args("claude", &Local::Mcp(String::new())).unwrap(), ["mcp", "list"]);
    assert_eq!(cli_args("claude", &Local::Mcp("magoagent".into())).unwrap(), ["mcp", "get", "magoagent"]);
    assert_eq!(pickers::resolve_mode("bypass"), Some(1));
    assert_eq!(pickers::resolve_mode("Auto"), Some(0));
    assert_eq!(pickers::resolve_mode(""), None);
    assert_eq!(pickers::resolve_mode("outro"), None);
}

/// `/resume`'s list: the folder's conversations by their first message, newest first, without those a chat holds.
#[test]
fn resume_lists_the_folders_earlier_conversations() {
    let projects = temp();
    let said = |text: &str| format!("{}\n", serde_json::json!({ "type": "user", "message": { "role": "user", "content": text } }));

    fs::write(projects.join("aaaa1111-velha.jsonl"), said("arrume o login")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(projects.join("bbbb2222-nova.jsonl"), said("suba a versão")).unwrap();
    fs::write(projects.join("cccc3333-aberta.jsonl"), said("já num chat")).unwrap();
    fs::write(projects.join("nota.txt"), "x").unwrap();
    let found = pickers::sessions(&projects, &["cccc3333-aberta".into()], std::time::SystemTime::now());
    let ids: Vec<&str> = found.iter().map(|(id, ..)| id.as_str()).collect();

    assert_eq!(ids, ["bbbb2222-nova", "aaaa1111-velha"]);
    assert_eq!(found[0].1, "suba a versão");
    let rows = pickers::resume_rows(&found);

    assert_eq!(rows[0].name, "resume bbbb2222");
    assert!(rows[0].desc.starts_with("suba a versão · "), "{}", rows[0].desc);
    assert_eq!(pickers::resume_rows(&[("dddd4444".into(), "velha".into(), 6.5 * 86400.0)])[0].desc, "velha · há 6 dias");
    let _ = fs::remove_dir_all(projects);
}

#[test]
fn mcp_and_agents_list_what_claude_code_has() {
    let user = serde_json::json!({
        "mcpServers": { "magoagent": { "command": "/x/magoagent" } },
        "projects": { "/p": { "mcpServers": { "banco": { "url": "http://localhost:9" } } }, "/outra": { "mcpServers": { "fora": {} } } },
    });

    let folder = serde_json::json!({ "mcpServers": { "banco": { "command": "repetido" }, "docs": { "command": "npx docs" } } });
    let servers = pickers::mcp_servers(Some(&user), Some(&folder), "/p");

    assert_eq!(servers, [("magoagent".into(), "/x/magoagent".into()), ("banco".into(), "http://localhost:9".into()), ("docs".into(), "npx docs".into())]);
    let claude = temp();

    fs::create_dir_all(claude.join("agents")).unwrap();
    fs::write(claude.join("agents/revisor.md"), "---\nname: revisor-de-pr\ndescription: Revisa o PR\n---\ncorpo").unwrap();
    fs::write(claude.join("agents/notas.txt"), "x").unwrap();
    let mut found = Vec::new();

    pickers::subagents(&claude, "projeto", &mut found);
    assert_eq!(found, [("revisor-de-pr".to_string(), "Revisa o PR (projeto)".to_string())]);
    assert_eq!(subagent_request("revisor-de-pr"), "Use o subagente revisor-de-pr para ");
    let _ = fs::remove_dir_all(claude);
}

/// After "/model " the menu is the models; what follows filters them; Enter on a row sends "/model id"; a mode is set
/// by /permissions.
#[test]
fn a_commands_list_is_picked_in_the_menu_and_applied() {
    let mut app = DrawingApp::default();
    let ctx = Context::default();
    let installed = [Preset::new("Claude Code", "claude")];

    app.create_chat(&crate::app::chat::direct::NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &installed);
    let id = app.chat_agent.clone().unwrap();

    app.chat_draft = "/permissions ".into();
    let rows = app.slash_entries().unwrap();

    assert_eq!(rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(), ["permissions Auto", "permissions Bypass permissions"]);
    assert_eq!(rows[0].desc, "Atual");
    app.chat_draft = "/permissions byp".into();
    let rows = app.slash_entries().unwrap();

    assert_eq!(rows.len(), 1);
    app.chat_draft = "/permissions Auto ".into();
    assert!(app.slash_entries().is_none(), "a space after the value closes the menu");
    app.chat_draft = "/model ".into();
    assert!(app.slash_entries().unwrap().iter().all(|row| row.name.starts_with("model ")));
    // Enter on a row: the box says the command with it, and it goes (the Enter is not taken)
    let box_id = egui::Id::unique("caixa_lista");
    let mut draft = "/permissions byp".to_string();
    let enter = egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    let rows = app.slash_entries_of(&id, &draft).unwrap();
    let mut taken = true;

    for events in [vec![], vec![enter]] {
        let _ = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
            taken = app.slash_keys(ui.ctx(), &mut draft, &rows, box_id);
            ui.add(egui::TextEdit::singleline(&mut draft).id(box_id)).request_focus();
        });
    }

    assert!(!taken);
    assert_eq!(draft, "/permissions Bypass permissions");
    assert!(app.chat_submit(&ctx, &id, &draft, &draft));
    assert_eq!(app.chats[0].mode, 1);
}
