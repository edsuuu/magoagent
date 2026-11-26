use super::*;

fn app_on(tab: &str) -> DrawingApp {
    let mut app = DrawingApp::default();

    app.settings_open = true;
    app.settings_tab = position(tab);
    app
}

fn position(tab: &str) -> usize {
    super::super::SETTINGS_TABS.iter().position(|name| *name == tab).unwrap()
}

fn values(snapshot: &Snapshot) -> Vec<(&str, &Value)> {
    snapshot.groups.iter().flat_map(|group| &group.items).filter(|item| !item.id.is_empty()).map(|item| (item.id.as_str(), &item.value)).collect()
}

fn titles(app: &DrawingApp, tab: &str) -> Vec<String> {
    app.settings_snapshot(position(tab)).groups.into_iter().map(|group| group.title).collect()
}

#[test]
fn the_pages_are_the_sidebar_s_in_order_and_sobre_has_no_credits() {
    let app = app_on("Geral");
    let snapshot = app.settings_snapshot(0);

    assert_eq!(
        snapshot.tabs,
        ["Geral", "Ilha", "Canvas", "Navegação e editor", "Terminal", "Agentes", "Chat", "Skills", "Integrações", "Atalhos", "Dados", "Sobre"]
    );

    for name in snapshot.tabs {
        let snapshot = app.settings_snapshot(position(name));
        let json = serde_json::to_string(&snapshot).unwrap();

        assert!(!json.contains("CRÉDITOS") && !json.contains("Ver os avisos"), "{}", snapshot.title);
        assert!(!snapshot.groups.is_empty(), "{}", snapshot.title);
    }
}

#[test]
fn each_page_has_the_egui_page_s_groups() {
    let app = app_on("Geral");

    assert_eq!(titles(&app, "Geral"), ["APARÊNCIA", "INÍCIO"]);
    assert_eq!(titles(&app, "Ilha"), ["ILHA DO MAESTRO"]);
    assert_eq!(titles(&app, "Canvas"), ["CANVAS"]);
    assert_eq!(titles(&app, "Navegação e editor"), ["NAVEGAÇÃO E EDITOR"]);
    assert_eq!(titles(&app, "Terminal"), ["INÍCIO RÁPIDO", "APARÊNCIA", "SHELL E COMPORTAMENTO"]);
    assert!(titles(&app, "Agentes").ends_with(&["LIMITES DO MAESTRO".into(), "ONDE OS AGENTES TRABALHAM".into()]));
    assert_eq!(titles(&app, "Chat"), ["CHAT", "CONTEXTO"]);
    assert_eq!(titles(&app, "Skills"), ["SKILL DE ESTILO", "SKILLS DOS AGENTES DO APP"]);
    assert!(titles(&app, "Integrações").starts_with(&["CLAUDE CODE FORA DO APP".into(), "CODEX FORA DO APP".into(), "MCP EM OUTROS AGENTES".into(), "SERVIÇOS".into()]));
    assert!(titles(&app, "Sobre").ends_with(&["AUTOR".into(), "PRIVACIDADE".into()]));
}

#[test]
fn a_row_set_comes_back_in_the_snapshot() {
    for page in ["Geral", "Ilha", "Canvas", "Navegação e editor", "Terminal", "Agentes", "Chat", "Skills"] {
        let mut app = app_on(page);
        let tab = |app: &DrawingApp| app.settings_snapshot(app.settings_tab);

        // every switch flips, every choice moves to its last option, a stepper and a field take their value
        for (id, kind, last, min) in tab(&app)
            .groups
            .iter()
            .flat_map(|group| &group.items)
            .map(|item| (item.id.clone(), item.kind, item.options.len(), item.min))
            .collect::<Vec<_>>()
        {
            // the login item is the system's and the island's place is the screen's: never touched by a test
            if id.is_empty() || id == "run_at_login" || id == "island_place" {
                continue;
            }

            let wanted = match kind {
                "toggle" => json!(!tab(&app).groups.iter().flat_map(|group| &group.items).find(|item| item.id == id).unwrap().value.as_bool().unwrap()),
                "choice" | "select" => json!(last - 1),
                "stepper" => json!(min + 1),
                "text" => json!("x"),
                _ => continue,
            };

            app.native_settings_act("set", &json!({ "id": id, "value": wanted })).unwrap();
            let snapshot = tab(&app);
            let now = values(&snapshot).into_iter().find(|(each, _)| *each == id).unwrap().1.clone();

            assert_eq!(now, wanted, "{page}: {id}");
        }
    }

    assert!(app_on("Geral").native_settings_act("set", &json!({ "id": "nada", "value": 1 })).is_err());
}

#[test]
fn voz_and_ok_mago_are_not_offered_and_agentes_has_the_temp_folder_switch() {
    let mut app = app_on("Geral");

    for hidden in super::super::HIDDEN_TABS {
        assert!(app.native_settings_act("tab", &json!({ "name": hidden })).is_err(), "{hidden}");
    }

    assert_eq!(super::super::SETTINGS_TABS[app.settings_tab], "Geral");
    app.native_settings_act("tab", &json!({ "name": "Agentes" })).unwrap();
    let on = app.prefs.work_outside_temp;

    app.native_settings_act("set", &json!({ "id": "work_outside_temp", "value": !on })).unwrap();
    assert_eq!(app.prefs.work_outside_temp, !on);
    assert!(values(&app.settings_snapshot(app.settings_tab)).contains(&("work_outside_temp", &json!(!on))));
    assert!(app.native_settings_act("set", &json!({ "id": "voice_engine", "value": 1 })).is_err(), "the voice's rows went with its page");
}

#[test]
fn the_mcp_is_under_integracoes_with_a_row_per_agent() {
    let app = app_on("Integrações");
    let snapshot = app.settings_snapshot(app.settings_tab);
    let mcp = snapshot.groups.iter().find(|group| group.title == "MCP EM OUTROS AGENTES").unwrap();
    let rows: Vec<&str> = mcp.items.iter().filter(|item| item.kind == "button").map(|item| item.id.as_str()).collect();

    assert_eq!(rows, ["mcp.claude", "mcp.codex"]);
    assert!(!titles(&app, "Agentes").contains(&"MCP EM OUTROS AGENTES".into()));
    assert!(app_on("Integrações").native_settings_act("press", &json!({ "id": "mcp_remove", "arg": "nada" })).is_err());
}

#[test]
fn the_skills_read_show_as_rows() {
    let app = app_on("Skills");

    *SKILLS.lock().unwrap() = Some(vec![Skill { name: "estilo".into(), description: "O seu estilo.".into() }]);
    let snapshot = app.settings_snapshot(app.settings_tab);
    let found = snapshot.groups[1].items.iter().any(|item| item.kind == "info" && item.label == "estilo" && item.hint == "O seu estilo.");

    *SKILLS.lock().unwrap() = None;
    assert!(found);
}

#[test]
fn a_secret_never_leaves_for_swift() {
    let field = text("github_token", "Token", "", "ghp_segredo", "", 250.0, true);

    assert_eq!(field.value, json!(""));
}

#[test]
fn the_recorder_waits_for_salvar_and_refuses_keys_another_shortcut_has() {
    let mut app = app_on("Atalhos");
    let hotkey = |app: &mut DrawingApp, args: Value| app.native_settings_act("hotkey", &args);

    let keys_of = |app: &DrawingApp, id: &str| {
        let snapshot = app.settings_snapshot(app.settings_tab);

        snapshot.groups.into_iter().flat_map(|group| group.items).find(|item| item.id == id).unwrap().keys.unwrap()
    };

    hotkey(&mut app, json!({ "op": "record", "id": "save" })).unwrap();
    assert!(keys_of(&app, "save").editing);
    // Cmd+Z is Desfazer's: shown, refused
    hotkey(&mut app, json!({ "op": "key", "id": "save", "key": "Z", "cmd": true })).unwrap();
    let taken = keys_of(&app, "save");

    assert!(taken.pending && taken.problem);
    hotkey(&mut app, json!({ "op": "save", "id": "save" })).unwrap();
    assert!(keys_of(&app, "save").editing, "still waiting");
    // Cmd+Alt+F5 is free: Salvar applies it
    hotkey(&mut app, json!({ "op": "key", "id": "save", "key": "F5", "cmd": true, "alt": true })).unwrap();
    assert!(!keys_of(&app, "save").problem);
    hotkey(&mut app, json!({ "op": "save", "id": "save" })).unwrap();
    let saved = keys_of(&app, "save");

    assert!(!saved.editing && saved.changed && saved.text.contains("F5"), "{}", saved.text);
    hotkey(&mut app, json!({ "op": "clear", "id": "save" })).unwrap();
    assert!(!keys_of(&app, "save").can_clear);
    hotkey(&mut app, json!({ "op": "default", "id": "save" })).unwrap();
    assert!(!keys_of(&app, "save").changed);
    // Esc alone ends the recording
    hotkey(&mut app, json!({ "op": "record", "id": "save" })).unwrap();
    hotkey(&mut app, json!({ "op": "key", "id": "save", "key": "Escape" })).unwrap();
    assert!(!keys_of(&app, "save").editing);
}

#[test]
fn the_presets_move_go_and_are_edited_one_at_a_time() {
    let mut app = app_on("Terminal");
    let count = app.prefs.presets.len();
    let first = app.prefs.presets[0].name.clone();

    app.native_settings_act("press", &json!({ "id": "preset.0.down" })).unwrap();
    assert_eq!(app.prefs.presets[1].name, first);
    app.native_settings_act("press", &json!({ "id": "preset.1.remove" })).unwrap();
    assert_eq!(app.prefs.presets.len(), count - 1);
    // a new one opens for editing, as the egui page's
    app.native_settings_act("press", &json!({ "id": "presets_add" })).unwrap();
    assert_eq!(app.prefs.presets.len(), count);
    assert_eq!(app.settings_edit, Some(count - 1));
    app.native_settings_act("press", &json!({ "id": "preset.0.edit" })).unwrap();

    let editing = |app: &DrawingApp| {
        let snapshot = app.settings_snapshot(app.settings_tab);

        snapshot.groups[0].items.iter().filter(|item| item.kind == "preset" && item.value["editing"] == json!(true)).count()
    };

    assert_eq!((app.settings_edit, editing(&app)), (Some(0), 1));
    app.native_settings_act("set", &json!({ "id": "preset.0.color", "value": 2 })).unwrap();
    assert_eq!(app.prefs.presets[0].color, crate::app::ui::TERMINAL_COLORS[2]);
    app.native_settings_act("press", &json!({ "id": "preset.0.edit" })).unwrap();
    assert_eq!(editing(&app), 0);
}

#[test]
fn closing_clears_the_questions_and_the_tab_moves() {
    let mut app = app_on("Dados");

    app.wipe_confirm = true;
    assert!(app.settings_snapshot(app.settings_tab).wipe.is_some());
    app.native_settings_act("tab", &json!({ "name": "Sobre" })).unwrap();
    assert_eq!(app.settings_tab, position("Sobre"));
    app.native_settings_act("close", &json!({})).unwrap();
    assert!(!app.settings_open && !app.wipe_confirm);
}
