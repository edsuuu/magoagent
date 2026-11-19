use super::act::{SCREEN_COMMANDS, command_named};
use super::model::{ALL_TOOLS, tool_from_id, tool_id};
use crate::app::hotkeys::{Act, local::APP};
use crate::prelude::*;

fn shell(app: &DrawingApp) -> Value {
    serde_json::to_value(app.shell_model()).unwrap()
}

fn verb(app: &mut DrawingApp, name: &str, args: Value) -> Result<String, String> {
    app.native_shell_act(&format!("ui.shell.{name}"), &args)
}

fn rectangle(app: &mut DrawingApp) {
    let element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 80.0, height: 60.0 });

    app.scene.elements.push(element);
}

#[test]
fn the_snapshot_says_the_window_the_egui_one_draws() {
    let app = DrawingApp::default();
    let shell = shell(&app);

    assert_eq!(shell["tool"], "select");
    assert_eq!(shell["screen"], "canvas");
    let main: Vec<&str> = shell["tools"]["main"].as_array().unwrap().iter().map(|tool| tool["id"].as_str().unwrap()).collect();

    assert_eq!(main, ["select", "hand", "terminal", "filetree", "portal", "sticky"], "the toolbar's order");
    assert_eq!(shell["tools"]["main"][0]["badge"], "V");
    assert_eq!(shell["tools"]["main"][2]["label"], "Terminal ou chat");
    assert_eq!(shell["tools"]["extra"].as_array().unwrap().len(), 5);
    assert!(shell["properties"].is_null(), "nothing to style with the selection tool and nothing selected");
    assert_eq!((shell["canUndo"].clone(), shell["zoom"].clone()), (json!(false), json!(100)));
    // the menu's keys are the app's own: Cmd+S saves, and Desfazer is Cmd+Z
    let keys = shell["shortcuts"].as_array().unwrap();
    let save = keys.iter().find(|key| key["id"] == "save").unwrap();

    assert_eq!((save["key"].clone(), save["command"].clone(), save["shift"].clone()), (json!("s"), json!(true), json!(false)));
    let redo = keys.iter().find(|key| key["id"] == "redo").unwrap();

    assert_eq!((redo["key"].clone(), redo["shift"].clone()), (json!("z"), json!(true)), "the first of the two keys");
}

#[test]
fn a_tool_picked_in_swift_is_the_tool_egui_would_pick_and_shows_its_row() {
    let mut app = DrawingApp::default();

    verb(&mut app, "tool", json!({ "id": "rectangle" })).unwrap();
    assert_eq!(app.tool, Tool::Rectangle);
    let shell = shell(&app);

    assert_eq!((shell["tool"].clone(), shell["drawing"].clone()), (json!("rectangle"), json!(true)));
    let properties = &shell["properties"];

    assert_eq!((properties["showStroke"].clone(), properties["showFill"].clone(), properties["showEdges"].clone()), (json!(true), json!(true), json!(true)));
    assert_eq!(properties["showArrow"], false);
    assert!(verb(&mut app, "tool", json!({ "id": "nada" })).is_err());

    for tool in ALL_TOOLS {
        assert_eq!(tool_from_id(&tool_id(tool)), Some(tool), "ids round-trip");
    }
}

#[test]
fn a_tools_letter_picks_it_only_while_the_keys_are_free() {
    let mut app = DrawingApp::default();

    // the drawing row is closed: only Mão and Seleção answer, as in egui
    verb(&mut app, "tool_key", json!({ "key": "h" })).unwrap();
    assert_eq!(app.tool, Tool::Hand);
    verb(&mut app, "tool_key", json!({ "key": "r" })).unwrap();
    assert_eq!(app.tool, Tool::Hand, "a stray letter arms nothing");
    verb(&mut app, "draw_toggle", json!({})).unwrap();
    verb(&mut app, "tool_key", json!({ "key": "r" })).unwrap();
    assert_eq!(app.tool, Tool::Rectangle);
    verb(&mut app, "tool_key", json!({ "key": "5" })).unwrap();
    assert_eq!(app.tool, Tool::Arrow, "the numeric key too");
    app.terminal_focus = Some("t".into());
    verb(&mut app, "tool_key", json!({ "key": "v" })).unwrap();
    assert_eq!(app.tool, Tool::Arrow, "a terminal holding the keyboard keeps its letters");
}

#[test]
fn the_row_styles_the_selection_as_one_undo_step() {
    let mut app = DrawingApp::default();

    rectangle(&mut app);
    app.selected = vec![0];
    let shell_before = shell(&app);

    assert_eq!(shell_before["properties"]["strokeWidth"], 2.0);
    assert_eq!(shell_before["properties"]["selection"], 1);
    // a drag of the opacity slider: two moves, then the release
    verb(&mut app, "style", json!({ "opacity": 70, "final": false })).unwrap();
    verb(&mut app, "style", json!({ "opacity": 40, "final": true })).unwrap();
    verb(&mut app, "style", json!({ "strokeWidth": 4, "fillStyle": "cross_hatch", "stroke": [224, 49, 49, 255], "fill": [255, 201, 201, 255] })).unwrap();
    let style = app.scene.elements[0].style;

    assert_eq!((style.opacity, style.stroke_width, style.fill_style), (40, 4.0, FillStyle::CrossHatch));
    assert_eq!((style.stroke, style.fill), ([224, 49, 49, 255], Some([255, 201, 201, 255])));
    assert_eq!(app.undo.len(), 2, "the slider is one step, the picks another");
    assert_eq!(shell(&app)["properties"]["fillStyle"], "cross_hatch");
    // a transparent pick clears the fill
    verb(&mut app, "style", json!({ "fill": [0, 0, 0, 0] })).unwrap();
    assert_eq!(app.scene.elements[0].style.fill, None);
}

#[test]
fn arrows_show_their_type_and_heads_and_take_them() {
    let mut app = DrawingApp::default();

    verb(&mut app, "tool", json!({ "id": "arrow" })).unwrap();
    let properties = &shell(&app)["properties"];

    assert_eq!((properties["showArrow"].clone(), properties["showEdges"].clone()), (json!(true), json!(false)));
    verb(&mut app, "head", json!({ "end": true, "head": "triangle" })).unwrap();
    assert_eq!(app.heads.1, Some(Head::Triangle));
    assert_eq!(shell(&app)["properties"]["endHead"], "triangle");
    verb(&mut app, "head", json!({ "end": true, "head": "none" })).unwrap();
    assert_eq!(shell(&app)["properties"]["endHead"], "none");
}

#[test]
fn novo_terminal_is_edited_field_by_field_and_confirmed() {
    let mut app = DrawingApp::default();

    app.prefs.presets = vec![
        Preset { name: "Claude".into(), command: "claude".into(), on: true, icon: String::new(), color: None },
        Preset { name: "Shell".into(), command: String::new(), on: true, icon: String::new(), color: None },
    ];

    let at = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(300.0, 200.0));

    app.terminal_dialog = Some(TerminalDialog { target: None, at, spec: TerminalSpec::default(), chat: false });
    assert_eq!(shell(&app)["dialogs"]["terminal"]["presets"].as_array().unwrap().len(), 2);
    verb(&mut app, "dialog", json!({ "dialog": "terminal", "action": "set", "field": "preset", "value": 0 })).unwrap();
    let dialog = &shell(&app)["dialogs"]["terminal"];

    assert_eq!((dialog["name"].clone(), dialog["command"].clone(), dialog["presets"][0]["selected"].clone()), (json!("Claude"), json!("claude"), json!(true)));
    // Chat narrows the quick starts to the agents a chat speaks with
    verb(&mut app, "dialog", json!({ "dialog": "terminal", "action": "set", "field": "chat", "value": true })).unwrap();
    assert_eq!(shell(&app)["dialogs"]["terminal"]["presets"].as_array().unwrap().len(), 1);
    verb(&mut app, "dialog", json!({ "dialog": "terminal", "action": "set", "field": "name", "value": "Revisor" })).unwrap();
    assert_eq!(app.terminal_dialog.as_ref().unwrap().spec.name, "Revisor");
    verb(&mut app, "dialog", json!({ "dialog": "terminal", "action": "cancel" })).unwrap();
    assert!(shell(&app)["dialogs"]["terminal"].is_null());
    assert!(verb(&mut app, "dialog", json!({ "dialog": "terminal", "action": "set", "field": "name", "value": "x" })).is_err());
}

#[test]
fn the_palette_lists_filters_and_runs_through_swift() {
    let mut app = DrawingApp::default();

    verb(&mut app, "command", json!({ "name": "palette" })).unwrap();
    assert!(app.palette_open);
    let rows = shell(&app)["dialogs"]["palette"]["rows"].as_array().unwrap().len();

    assert!(rows > 10);
    verb(&mut app, "palette", json!({ "action": "query", "query": "modo escuro" })).unwrap();
    let palette = &shell(&app)["dialogs"]["palette"];

    assert_eq!((palette["rows"].as_array().unwrap().len(), palette["rows"][0]["label"].clone()), (1, json!("Modo escuro")));
    let dark = app.dark();

    verb(&mut app, "palette", json!({ "action": "run", "index": 0 })).unwrap();
    assert_ne!(app.dark(), dark);
    assert!(!app.palette_open, "a command closes it");
}

#[test]
fn every_shortcut_of_ajustes_has_a_menu_command() {
    for def in APP.iter().filter(|def| matches!(def.act, Act::App(_))) {
        // the points' editor and the chat's queue answer in their own places
        let known = command_named(def.id).is_some() || SCREEN_COMMANDS.contains(&def.id) || ["editPoints", "sendNow"].contains(&def.id);

        assert!(known, "{} has no `ui.shell.command`", def.id);
    }

    let mut app = DrawingApp::default();

    assert!(verb(&mut app, "command", json!({ "name": "nada" })).is_err());
    verb(&mut app, "command", json!({ "name": "settings" })).unwrap();
    assert!(shell(&app)["settingsOpen"] == json!(true));
}

#[test]
fn the_sidebar_and_the_screen_switch_and_a_toast_comes_with_its_clock() {
    let mut app = DrawingApp::default();

    verb(&mut app, "sidebar", json!({ "state": "hide" })).unwrap();
    assert_eq!(shell(&app)["sidebar"]["hidden"], true);
    verb(&mut app, "screen", json!({ "state": "chats" })).unwrap();
    assert_eq!(shell(&app)["screen"], "chats");
    assert!(shell(&app)["hint"].is_null(), "no hint over the chats");
    verb(&mut app, "screen", json!({ "state": "toggle" })).unwrap();
    assert_eq!(shell(&app)["screen"], "canvas");
    app.toast("Copiado");
    assert!(shell(&app)["toast"].is_null(), "its clock starts with its first frame");
    (app.now, app.status_time) = (10.0, 9.0);
    let toast = &shell(&app)["toast"];

    assert_eq!((toast["text"].clone(), toast["id"].clone()), (json!("Copiado"), json!(9.0)));
    app.now = 20.0;
    assert!(shell(&app)["toast"].is_null(), "and it goes after its seconds");
}

#[test]
fn the_sidebar_lists_the_spaces_with_their_agents_counts() {
    let mut app = DrawingApp::default();

    app.spaces.list = vec![
        crate::app::workspaces::Workspace { id: "a".into(), name: "Loja".into(), scene: "a.mago".into(), pinned: true, ..Default::default() },
        crate::app::workspaces::Workspace { id: "b".into(), name: "Blog".into(), scene: "b.mago".into(), ..Default::default() },
    ];

    app.spaces.current = "b".into();
    let shell = shell(&app);
    let spaces = shell["sidebar"]["spaces"].as_array().unwrap();

    assert_eq!((spaces[0]["name"].clone(), spaces[0]["pinned"].clone(), spaces[1]["current"].clone()), (json!("Loja"), json!(true), json!(true)));
    assert_eq!(shell["space"]["name"], "Blog");
    assert_eq!(shell["floors"][0]["name"], "Térreo", "the ground floor is always there");
}

#[test]
fn a_right_click_opens_the_egui_menu_for_swift_and_a_pick_runs_its_row() {
    let mut app = DrawingApp::default();

    app.canvas_rect = Rect::ZERO;
    rectangle(&mut app);
    verb(&mut app, "context_open", json!({ "px": 400, "py": 400 })).unwrap();
    let menu = &shell(&app)["menu"]["entries"];

    assert_eq!((menu[0]["kind"].clone(), menu[0]["label"].clone()), (json!("sub"), json!("Adicionar")), "the empty canvas's menu");
    verb(&mut app, "menu", json!({ "id": "" })).unwrap();
    assert!(shell(&app)["menu"].is_null(), "closed without a pick");
    // on the rectangle's edge: its own menu, Apagar in red, and the pick deletes it
    verb(&mut app, "context_open", json!({ "px": 1, "py": 30 })).unwrap();
    let entries = shell(&app)["menu"]["entries"].as_array().unwrap().clone();
    let delete = entries.iter().find(|entry| entry["id"] == "delete").unwrap();

    assert_eq!((delete["label"].clone(), delete["danger"].clone()), (json!("Apagar"), json!(true)));
    verb(&mut app, "menu", json!({ "id": "delete" })).unwrap();
    assert!(app.scene.elements.is_empty());
    assert!(shell(&app)["menu"].is_null());
}

#[test]
fn renomear_shows_the_name_and_saves_the_text_swift_sends() {
    let mut app = DrawingApp::default();

    rectangle(&mut app);
    app.scene.elements[0].portal = Some(PortalSpec { name: "Docs".into(), ..Default::default() });
    let id = app.scene.elements[0].id.clone();

    // what Renomear of its menu opens (context/rename.rs::open_rename)
    app.rename = Some((id.clone(), "Docs".into()));
    let rename = &shell(&app)["dialogs"]["rename"];

    assert_eq!((rename["name"].clone(), rename["group"].clone()), (json!("Docs"), json!(false)));
    verb(&mut app, "dialog", json!({ "dialog": "rename", "action": "confirm", "value": "Manual" })).unwrap();
    assert_eq!(app.scene.elements[0].portal.as_ref().unwrap().name, "Manual");
    assert!(shell(&app)["dialogs"]["rename"].is_null());
    app.rename = Some((id, "Manual".into()));
    verb(&mut app, "dialog", json!({ "dialog": "rename", "action": "cancel" })).unwrap();
    assert!(app.rename.is_none());
}

#[test]
fn the_corner_buttons_and_the_usage_card_reach_their_places() {
    let mut app = DrawingApp::default();
    let shell_now = shell(&app);
    let buttons = shell_now["pane"]["buttons"].as_array().unwrap();

    assert!(buttons.iter().all(|button| button["count"] == 0), "no changed files, no badge");
    assert_eq!(shell_now["sidebar"]["agents"], json!([]));
    assert_eq!(shell_now["sidebar"]["usageOpen"], false, "no card without a reading");
    // Configurações… of the usage card opens Ajustes on Agentes
    verb(&mut app, "settings", json!({ "tab": "Agentes" })).unwrap();
    assert!(app.settings_open);
    assert_eq!(crate::app::ui::settings::SETTINGS_TABS[app.settings_tab], "Agentes");
    verb(&mut app, "settings", json!({ "open": false })).unwrap();
    assert!(!app.settings_open);
}
