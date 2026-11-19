//! Comandos (panes/commands.rs): the terminals that run commands, off the canvas, as rows that open like an accordion.

use super::panes::{SCREEN, pane_click, pane_frame};
use super::*;
use crate::app::panes::Pane;
use crate::app::panes::commands::{CommandState, docked_grid};
use egui::accesskit::Role;

/// An app with the Luna's terminal at the origin; gives its id.
fn with_luna(app: &mut DrawingApp) -> String {
    app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let mut luna = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });

    luna.terminal = Some(TerminalSpec { name: "Luna".into(), cwd: "/p/loja".into(), maestro: true, ..Default::default() });
    let id = luna.id.clone();

    app.scene.elements.push(luna);
    id
}

fn terminal<'a>(app: &'a DrawingApp, name: &str) -> &'a Element {
    app.scene.elements.iter().find(|e| e.terminal.as_ref().is_some_and(|t| t.name == name)).unwrap()
}

/// What an agent opens to run goes to Comandos: off the canvas (not painted, not hit, its rope neither), still roped
/// to it; `canvas` true keeps the old card.
#[test]
fn a_command_an_agent_runs_goes_to_comandos_not_the_canvas() {
    let mut app = DrawingApp::default();
    let luna = with_luna(&mut app);
    let said = app.act("create_terminal", &json!({ "command": "php artisan serve", "asker": luna })).unwrap();
    let php = terminal(&app, "php").clone();

    assert!(php.docked() && said.contains("Comandos"), "{said}");
    assert_eq!(app.docked_ids(), [php.id.clone()]);
    assert!(app.panes.commands.open.contains(&php.id), "its row opens on its output");
    assert!(app.links().get(&luna).is_some_and(|links| links.contains(&php.id)), "still roped to who opened it");
    let rope = app.scene.elements.iter().find(|e| e.rope).unwrap();

    assert!(hidden_page(&app.scene.elements, &php) && hidden_page(&app.scene.elements, rope), "neither on the canvas");
    assert!(!hidden_page(&app.scene.elements, terminal(&app, "Luna")));
    assert_eq!(app.hit(element_bounds(&php).center()), None, "a click there finds nothing");
    // no shell yet: waiting; its state words
    assert_eq!(app.command_state(&php.id), CommandState::Waiting);
    assert_eq!(CommandState::Running.label(), "Rodando");

    app.act("create_terminal", &json!({ "command": "npm run dev", "asker": luna, "canvas": true })).unwrap();
    assert!(!terminal(&app, "npm").docked(), "on the canvas when asked");
}

/// Levar para o canvas puts it beside who it is tied to, on a free spot, as one undo step; Mover para Comandos (the
/// menu and the MCP's `terminal_menu` dock) takes it back.
#[test]
fn a_terminal_moves_between_the_canvas_and_comandos() {
    let mut app = DrawingApp::default();
    let luna = with_luna(&mut app);

    app.act("create_terminal", &json!({ "command": "cargo test", "asker": luna })).unwrap();
    let id = terminal(&app, "cargo").id.clone();

    // something already where it would go
    let mut box_ = app.new_element(ElementKind::Rectangle { x: 460.0, y: 0.0, width: 300.0, height: 300.0 });

    box_.id = "caixa".into();
    app.scene.elements.push(box_);
    let said = app.act("terminal_menu", &json!({ "name": "cargo", "action": "dock", "value": "off" })).unwrap();
    let card = terminal(&app, "cargo");
    let spot = local_bounds(card);

    assert!(!card.docked() && said.contains("canvas"), "{said}");
    assert!(app.scene.elements.iter().filter(|e| e.id != id && !e.rope).all(|e| !element_bounds(e).intersects(spot)), "on a free spot: {spot:?}");
    assert_eq!(app.hit(spot.center()), app.terminal_index(&id), "a click finds it now");
    assert!(app.scene.elements.iter().filter(|e| e.rope).all(|e| !hidden_page(&app.scene.elements, e)), "its rope shows again");

    app.undo();
    assert!(terminal(&app, "cargo").docked(), "one undo takes it back");

    app.redo();
    let index = app.terminal_index(&id).unwrap();

    app.terminal_focus = Some(id.clone());
    assert!(app.terminal_menu_action(&Context::default(), Some(index), "dock"));
    assert!(terminal(&app, "cargo").docked() && app.terminal_focus.is_none(), "the menu's row, and the keys leave it");
    assert!(app.terminal_entries(index).iter().any(|row| matches!(row, crate::app::ui::context::Entry::Item { id, .. } if id == "dock")));
    // the agents' list opens it in Comandos instead of gliding to where it is not
    app.panes.commands.open.clear();
    app.open_agent(&id);
    assert_eq!(app.panes.open, Some(Pane::Commands));
    assert!(app.panes.commands.open.contains(&id));
    assert!(app.act("terminal_menu", &json!({ "name": "cargo", "action": "dock", "value": "on" })).unwrap().contains("já estava"));
}

/// The panel: its button, the row with the command, folder and state; a click on the row folds it; the MCP opens it.
#[test]
fn the_panel_lists_each_command_as_a_row_that_folds() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let luna = with_luna(&mut app);

    app.act("create_terminal", &json!({ "command": "cargo test --all", "asker": luna, "name": "Testes" })).unwrap();
    let id = terminal(&app, "Testes").id.clone();

    assert_eq!(app.act("panel", &json!({ "name": "commands", "open": true })).unwrap(), "Painel Comandos aberto.");

    for _ in 0..3 {
        pane_frame(&mut app, &ctx, vec![]);
    }

    let shown = pane_frame(&mut app, &ctx, vec![]);
    let has = |shown: &[(Role, String, Rect)], text: &str| shown.iter().any(|(_, said, _)| said == text);

    assert!(has(&shown, "cargo test --all") && has(&shown, "/p/loja"), "{shown:?}");
    assert!(has(&shown, "Parar") && has(&shown, "Reiniciar") && has(&shown, "Levar para o canvas"), "open: its buttons");
    // a click on its name folds it
    let row = app.pane_rect(Rect::from_min_size(Pos2::ZERO, SCREEN)).left_top() + Vec2::new(60.0, 46.0 + 12.0 + 18.0);
    pane_click(&mut app, &ctx, row);
    let folded = pane_frame(&mut app, &ctx, vec![]);

    assert!(!app.panes.commands.open.contains(&id) && !has(&folded, "Parar"), "{folded:?}");
    // its grid fills the panel's width, 16 rows
    let (cols, rows) = docked_grid(340.0, 13.0);

    assert!(rows == 16 && cols >= 20 && docked_grid(640.0, 13.0).0 > cols);
}
