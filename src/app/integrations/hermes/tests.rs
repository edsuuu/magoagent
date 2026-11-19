use super::*;

/// A Hermes folder of the test's own, never the person's.
fn temp_home(name: &str) -> PathBuf {
    let home = std::env::temp_dir().join(format!("mago-hermes-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);

    fs::create_dir_all(&home).unwrap();
    home
}

const EXE: &str = "C:\\Programas\\MagoAgent\\magoagent.exe";

/// A config as Hermes documents it, with what is the person's around the list.
const THEIRS: &str = "model: anthropic/claude-sonnet-4.6\n# os meus plugins\nplugins:\n  enabled:\n    - disk-cleanup   # limpa o disco\n    - my-tool-plugin\n  disabled:\n    - noisy-plugin\n  hook_callback_timeout: 30\ngateway:\n  port: 8080\n";

#[test]
fn our_name_goes_into_the_list_and_comes_out_leaving_the_rest_as_written() {
    let with = merged(THEIRS, true).unwrap();

    assert_eq!(with, THEIRS.replace("    - my-tool-plugin\n", "    - my-tool-plugin\n    - magoagent\n"), "after the last of theirs, at their depth");
    assert!(enabled_in(&with) && !enabled_in(THEIRS));
    assert_eq!(merged(&with, true).unwrap(), with, "twice does not double it");
    assert_eq!(merged(&with, false).unwrap(), THEIRS, "out: byte for byte what they had");
    assert_eq!(merged(THEIRS, false).unwrap(), THEIRS, "nothing of ours: nothing changes");
    // Windows line ends and no line break at the end are kept
    let windows = THEIRS.replace('\n', "\r\n");

    assert_eq!(merged(&merged(&windows, true).unwrap(), false).unwrap(), windows);
    assert!(merged(&windows, true).unwrap().contains("    - my-tool-plugin\r\n    - magoagent\r\n") && !merged(&windows, true).unwrap().contains("t\n"));
    let cut = THEIRS.trim_end();

    assert!(!merged(cut, true).unwrap().ends_with('\n'));
}

#[test]
fn every_way_the_list_is_written_takes_our_name() {
    // (before, with our name, after taking it out again)
    let cases = [
        // the items at the key's own depth
        ("plugins:\n  enabled:\n  - a\n  disabled:\n  - b\n", "plugins:\n  enabled:\n  - a\n  - magoagent\n  disabled:\n  - b\n", None),
        // four spaces, quoted
        ("plugins:\n    enabled:\n        - \"a\"\n", "plugins:\n    enabled:\n        - \"a\"\n        - magoagent\n", None),
        // on one line, with a comment
        ("plugins:\n  enabled: [a, b]  # meus\n", "plugins:\n  enabled: [a, b, magoagent]  # meus\n", None),
        ("plugins:\n  enabled: []\nx: 1\n", "plugins:\n  enabled: [magoagent]\nx: 1\n", None),
        // an empty list said as nothing: taking ours out leaves a list, which is what Hermes iterates
        (
            "plugins:\n  enabled:\n  disabled:\n    - b\n",
            "plugins:\n  enabled:\n    - magoagent\n  disabled:\n    - b\n",
            Some("plugins:\n  enabled: []\n  disabled:\n    - b\n"),
        ),
        ("plugins:\n  enabled: null\n", "plugins:\n  enabled:\n    - magoagent\n", Some("plugins:\n  enabled: []\n")),
        // no `enabled`, no `plugins`, no file
        (
            "plugins:\n  disabled:\n    - b\n",
            "plugins:\n  enabled:\n    - magoagent\n  disabled:\n    - b\n",
            Some("plugins:\n  enabled: []\n  disabled:\n    - b\n"),
        ),
        ("plugins: {}\nx: 1\n", "plugins:\n  enabled:\n    - magoagent\nx: 1\n", Some("plugins:\n  enabled: []\nx: 1\n")),
        ("model: x\n", "model: x\n\nplugins:\n  enabled:\n    - magoagent\n", Some("model: x\n\nplugins:\n  enabled: []\n")),
        ("", "plugins:\n  enabled:\n    - magoagent\n", Some("plugins:\n  enabled: []\n")),
        // a byte order mark, and the document's own start
        ("\u{feff}---\nplugins:\n  enabled:\n    - a\n", "\u{feff}---\nplugins:\n  enabled:\n    - a\n    - magoagent\n", None),
    ];

    for (before, with, after) in cases {
        let added = merged(before, true).unwrap();

        assert_eq!(added, with, "{before:?}");
        assert!(enabled_in(&added), "{with:?}");
        assert_eq!(merged(&added, false).unwrap(), after.unwrap_or(before), "{before:?}");
    }

    // already there, however it is written
    for has in ["plugins:\n  enabled:\n    - \"magoagent\"  # a ilha\n", "plugins:\n  enabled: [a, 'magoagent']\n"] {
        assert_eq!(merged(has, true).unwrap(), has);
        assert!(enabled_in(has));
    }

    assert_eq!(merged("plugins:\n  enabled: [a, 'magoagent']\n", false).unwrap(), "plugins:\n  enabled: [a]\n");
}

#[test]
fn a_config_this_cant_change_safely_is_refused_untouched() {
    let refused = [
        "a: 1\n---\nplugins:\n  enabled: []\n",
        "{\"plugins\": {\"enabled\": []}}\n",
        "\"plugins\":\n  enabled: []\n",
        "plugins: {enabled: [a]}\n",
        "plugins:\n  enabled: [a,\n    b]\n",
        "plugins:\n  enabled: true\n",
        "plugins:\n  enabled: &lista\n    - a\n",
        "plugins:\n\tenabled:\n\t\t- a\n",
        "plugins:\n  enabled:\n    - nome: a\n      opcao: 1\n",
    ];

    for config in refused {
        for add in [true, false] {
            // nothing of ours in a file it would only read is nothing to do; the rest is refused
            match merged(config, add) {
                Err(why) => assert!(why.contains("não mexi") && why.contains("hermes plugins enable magoagent"), "{why}"),
                Ok(same) => assert_eq!((same.as_str(), add), (config, false), "{config:?}"),
            }
        }

        assert!(merged(config, true).is_err(), "{config:?}");
    }
}

#[test]
fn the_plugin_calls_this_app_and_is_python() {
    let source = plugin_source(EXE);

    assert!(source.contains(&Value::from(EXE).to_string()), "the path as a quoted string");
    assert!(source.contains(MARK) && !source.contains("__MAGOAGENT_EXE__"));
    let unusual_path = "/opt/it's \"here\"/magoagent";

    assert!(plugin_source(unusual_path).contains(&Value::from(unusual_path).to_string()));

    for hook in MANIFEST.lines().filter_map(|line| line.strip_prefix("  - ")) {
        assert!(source.contains(&format!("\"{hook}\": {hook},")), "{hook} is registered");
    }

    // only what a session is doing leaves Hermes: never a message, a command or a tool's answer
    for private in ["user_message", "assistant_response", "conversation_history", "\"args\"", "result", "\"command\""] {
        assert!(!source.contains(&format!("kwargs.get(\"{}\")", private.trim_matches('"'))), "{private} is not read");
    }

    // where there is a Python, it reads the file without a complaint
    let home = temp_home("python");
    let file = home.join("__init__.py");

    fs::write(&file, &source).unwrap();
    let mut checked = false;

    for python in ["python3", "python"] {
        let check =
            crate::editor::command(python).args(["-I", "-c", "import ast, sys; ast.parse(open(sys.argv[1], encoding='utf-8').read())"]).arg(&file).output();

        if let Ok(output) = check {
            if output.status.success() {
                checked = true;

                break;
            }

            if !output.stderr.is_empty() && !String::from_utf8_lossy(&output.stderr).contains("not found") {
                panic!("{}", String::from_utf8_lossy(&output.stderr));
            }
        }
    }

    let _ = checked;
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn installing_writes_the_plugin_and_removing_gives_the_config_back() {
    let home = temp_home("instalar");
    let config = home.join("config.yaml");

    fs::write(&config, THEIRS).unwrap();
    assert_eq!(status_at(&home, EXE), Status::NotInstalled);
    let said = install_at(&home, EXE).unwrap();

    assert!(said.contains("Plugin instalado"), "{said}");
    let folder = home.join("plugins/magoagent");

    assert_eq!(fs::read_to_string(folder.join("__init__.py")).unwrap(), plugin_source(EXE));
    assert!(fs::read_to_string(folder.join("plugin.yaml")).unwrap().starts_with("name: magoagent\n"));
    assert!(enabled_in(&fs::read_to_string(&config).unwrap()));
    assert_eq!(status_at(&home, EXE), Status::Installed);
    assert_eq!(status_at(&home, "/outro/magoagent"), Status::Stale, "installed for another copy of the app");
    let backups = || fs::read_dir(&home).unwrap().flatten().filter(|file| file.file_name().to_string_lossy().ends_with(".bak")).collect::<Vec<_>>();

    assert_eq!(backups().len(), 1, "a backup before writing");
    assert_eq!(fs::read_to_string(backups()[0].path()).unwrap(), THEIRS, "of what they had");
    // twice: the config is not written again
    install_at(&home, EXE).unwrap();
    assert_eq!(backups().len(), 1);
    // Python left its cache there
    fs::create_dir_all(folder.join("__pycache__")).unwrap();
    fs::write(folder.join("__pycache__/__init__.cpython-312.pyc"), "x").unwrap();
    let said = remove_at(&home).unwrap();

    assert!(said.contains("removido"), "{said}");
    assert_eq!(fs::read_to_string(&config).unwrap(), THEIRS, "the config is theirs again, byte for byte");
    assert!(!folder.exists());
    assert_eq!(status_at(&home, EXE), Status::NotInstalled);
    assert!(remove_at(&home).unwrap().contains("não estava instalado"));
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn nothing_is_written_without_a_hermes_or_over_what_is_not_ours() {
    // no Hermes on the machine: no folder is made for it
    let absent = std::env::temp_dir().join(format!("mago-hermes-ausente-{}", std::process::id()));
    let _ = fs::remove_dir_all(&absent);

    assert!(install_at(&absent, EXE).is_err());
    assert!(!absent.exists());
    assert_eq!(status_at(&absent, EXE), Status::NoHermes);
    // a Hermes with no config yet: one is made, with no backup of nothing
    let home = temp_home("novo");

    install_at(&home, EXE).unwrap();
    assert_eq!(fs::read_to_string(home.join("config.yaml")).unwrap(), "plugins:\n  enabled:\n    - magoagent\n");
    assert_eq!(status_at(&home, EXE), Status::Installed);
    remove_at(&home).unwrap();
    // a config it can't change safely: refused before a single file is written
    fs::write(home.join("config.yaml"), "plugins: {enabled: [a]}\n").unwrap();
    assert!(install_at(&home, EXE).unwrap_err().contains("não mexi"));
    assert!(!home.join("plugins/magoagent").exists());
    assert_eq!(fs::read_to_string(home.join("config.yaml")).unwrap(), "plugins: {enabled: [a]}\n");
    // someone else's plugin of the same name, and its other files, stay
    let folder = home.join("plugins/magoagent");

    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("__init__.py"), "def register(ctx):\n    pass\n").unwrap();
    fs::write(home.join("config.yaml"), "model: x\n").unwrap();
    assert!(install_at(&home, EXE).unwrap_err().contains("não é nosso"));
    assert_eq!(fs::read_to_string(folder.join("__init__.py")).unwrap(), "def register(ctx):\n    pass\n");
    assert!(remove_at(&home).unwrap().contains("não estava instalado"));
    assert!(folder.join("__init__.py").exists());
    // ours, with a file of theirs beside it: ours go, theirs and the folder stay
    fs::write(folder.join("__init__.py"), plugin_source(EXE)).unwrap();
    fs::write(folder.join("notas.txt"), "minhas").unwrap();
    remove_at(&home).unwrap();
    assert!(!folder.join("__init__.py").exists() && folder.join("notas.txt").exists());
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn a_test_never_sees_the_persons_own_hermes() {
    let home = home().unwrap();

    assert!(home.starts_with(std::env::temp_dir()), "{}", home.display());
    let mut app = DrawingApp::default();

    assert_eq!(app.act_hermes("status").unwrap(), Status::NoHermes.words());
    assert!(app.act_hermes("install").is_err(), "no Hermes there: nothing is made");
    assert!(!home.exists());
    assert!(app.act_hermes("explodir").is_err());
}

/// What the plugin sends is Hermes' own pill, and only while the integration is on.
#[test]
fn a_hermes_session_is_its_own_pill_while_the_integration_is_on() {
    use crate::app::maestro::outside::Badge;
    use crate::app::maestro::relay::tag;
    use crate::app::ui::agents::AgentState;
    let env = |key: &str| (key == "MAGOAGENT_ORIGIN").then(|| "hermes".to_string());

    let event = |name: &str, message: &str| {
        let mut payload = json!({ "hook_event_name": name, "session_id": "s1", "tool_name": "terminal", "message": message, "cwd": "/projeto" });

        tag(&mut payload, &env);
        payload
    };

    assert_eq!(event("PreToolUse", "")["mago_origin"], "hermes");
    let mut app = DrawingApp::default();

    app.take_outside_hook(&event("PreToolUse", ""), None);
    assert!(app.outside_pills().is_empty(), "off: what Hermes says is dropped");
    app.prefs.integrations.hermes = true;
    app.take_outside_hook(&event("SessionStart", ""), None);
    app.take_outside_hook(&event("PreToolUse", ""), None);
    let pills = app.outside_pills();

    assert_eq!((pills.len(), pills[0].id.as_str(), pills[0].name.as_str()), (1, "pill:hermes", "Hermes"));
    assert!(matches!(pills[0].state, AgentState::Working(_)), "a tool runs");
    // Hermes asks for an approval in its own window: the pill waits
    app.take_outside_hook(&event("Notification", "O Hermes pede a sua aprovação: comando perigoso?"), None);
    assert!(app.outside_pills()[0].state == AgentState::Waiting);
    app.take_outside_hook(&event("PostToolUse", ""), None);
    assert!(matches!(app.outside_pills()[0].state, AgentState::Working(_)), "approved there: it works again");
    app.take_outside_hook(&event("Stop", ""), None);
    assert!(app.outside_pills()[0].badge == Some(Badge::Finished));
    app.take_outside_hook(&event("UserPromptSubmit", ""), None);
    app.take_outside_hook(&event("StopFailure", ""), None);
    assert!(app.outside_pills()[0].badge == Some(Badge::Error));
}
