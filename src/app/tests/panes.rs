//! The panels on the right (panes/): the title bar's buttons open and close them, and each one's
//! gestures (a diff opened; the Tarefas panel went).

use super::filetree::{git, temp, tree_on};
use super::*;
use crate::app::panes::{PANE_WIDTH, PANES, Pane};
use egui::accesskit::Role;

pub(super) const SCREEN: Vec2 = Vec2::new(1200.0, 800.0);

/// One frame of the title bar and the panel, with the canvas's keys; gives what it showed, from
/// its accessibility tree: each widget's role, text and rect.
pub(super) fn pane_frame(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) -> Vec<(Role, String, Rect)> {
    pane_input(app, ctx, events, Modifiers::NONE)
}

/// `pane_frame` with keys held (Shift on a click).
pub(super) fn pane_input(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>, modifiers: Modifiers) -> Vec<(Role, String, Rect)> {
    ctx.enable_accesskit();
    let time = Some(ctx.input(|i| i.time) + 0.05);
    let events = [egui::Event::ModifiersChanged(modifiers)].into_iter().chain(events).collect();
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time, events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| {
        app.screen_keys(ctx);
        app.handle_shortcuts(ctx);
        if TITLE_BAR > 0.0 {
            app.title_bar(ctx);
        }
        app.panes_ui(ctx);
    });
    output.textures_delta.clear();
    let nodes = output.platform_output.accesskit_update.map(|update| update.nodes).unwrap_or_default();
    nodes
        .into_iter()
        .filter_map(|(_, node)| {
            let bounds = node.bounds()?;
            let text = node.label().or(node.value()).unwrap_or_default().to_string();
            Some((node.role(), text, Rect::from_min_max(Pos2::new(bounds.x0 as f32, bounds.y0 as f32), Pos2::new(bounds.x1 as f32, bounds.y1 as f32))))
        })
        .collect()
}

pub(super) fn pane_click(app: &mut DrawingApp, ctx: &Context, at: Pos2) -> Vec<(Role, String, Rect)> {
    pane_frame(app, ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    let shown = pane_frame(app, ctx, vec![button(at, false)]);
    // the pointer leaves, so no tooltip covers the next click
    pane_frame(app, ctx, vec![egui::Event::PointerGone]);
    shown
}

/// The first frames: the title bar's areas take their size before they take clicks.
fn settle(app: &mut DrawingApp, ctx: &Context) {
    for _ in 0..4 {
        pane_frame(app, ctx, vec![]);
    }
}

/// A press on the title bar's panel button `k`.
fn press_pane(app: &mut DrawingApp, ctx: &Context, k: usize) {
    let at = app.pane_button(SCREEN.x, k).center();
    pane_click(app, ctx, at);
}

/// Where the widget of `role` whose text starts with `text` is.
pub(super) fn find(shown: &[(Role, String, Rect)], role: Role, text: &str) -> Option<Rect> {
    shown.iter().find(|(r, t, _)| *r == role && t.starts_with(text)).map(|(_, _, rect)| *rect)
}

pub(super) fn texts(shown: &[(Role, String, Rect)]) -> Vec<String> {
    shown.iter().map(|(_, text, _)| text.clone()).collect()
}

/// The panel opened by its button, settled.
pub(super) fn opened(pane: Pane) -> (Context, DrawingApp, Vec<(Role, String, Rect)>) {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    settle(&mut app, &ctx);
    let k = PANES.iter().position(|p| p.0 == pane).unwrap();
    press_pane(&mut app, &ctx, k);
    assert_eq!(app.panes.open, Some(pane));
    let shown = pane_frame(&mut app, &ctx, vec![]);
    (ctx, app, shown)
}

#[test]
fn each_button_opens_its_panel_and_closes_it_one_at_a_time() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    settle(&mut app, &ctx);
    assert_eq!(app.pane_width(), 0.0);
    for (k, (pane, ..)) in PANES.iter().enumerate() {
        press_pane(&mut app, &ctx, k);
        assert_eq!(app.panes.open, Some(*pane), "button {k}");
        // the canvas gives it its width, and the corners move out of its way
        assert_eq!((app.pane_width(), app.corner_inset().1), (PANE_WIDTH, PANE_WIDTH));
    }
    // the open one's button closes it
    press_pane(&mut app, &ctx, PANES.len() - 1);
    assert_eq!(app.panes.open, None);
    // the X on its header
    press_pane(&mut app, &ctx, 0);
    let close = Rect::from_center_size(Pos2::new(SCREEN.x - 22.0, 60.0 + 23.0), Vec2::splat(10.0));
    pane_click(&mut app, &ctx, close.center());
    assert_eq!(app.panes.open, None, "the X closes it");
    // and Esc
    press_pane(&mut app, &ctx, 0);
    app.selected = vec![0];
    pane_frame(&mut app, &ctx, vec![key_press(Key::Escape)]);
    assert_eq!(app.panes.open, None, "Esc closes it first");
    assert_eq!(app.selected, vec![0], "and only it");
    // over the chat the buttons and the panel are hidden
    app.set_pane(Some(Pane::Changes));
    app.chat_open = true;
    pane_frame(&mut app, &ctx, vec![]);
    assert_eq!((app.panes.buttons, app.pane_width()), (None, 0.0));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn the_buttons_are_no_place_to_move_the_window() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    settle(&mut app, &ctx);
    let at = app.pane_button(SCREEN.x, 0).center();
    let mut raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        events: vec![egui::Event::PointerMoved(at), button(at, true)],
        ..Default::default()
    };
    app.caption_drag(&ctx, &mut raw);
    assert_eq!(raw.events.len(), 2, "the press stays the button's");
}

/// Asked: "se eu tô na intro e clico e arrasto a janela no meio ela deve deixar eu arrastar, mesma coisa com os botões
/// … eles nem devem aparecer na intro". On the welcome the panels' buttons are gone and the bar's middle moves the window.
#[cfg(not(target_os = "macos"))]
#[test]
fn on_the_welcome_the_panel_buttons_hide_and_the_top_moves_the_window() {
    let ctx = Context::default();
    let mut app = DrawingApp { welcome: Some(Default::default()), ..Default::default() };
    settle(&mut app, &ctx);
    assert_eq!(app.panes.buttons, None, "no panel buttons on the welcome");
    let middle = Pos2::new(SCREEN.x / 2.0, 20.0);
    let mut raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        events: vec![egui::Event::PointerMoved(middle), button(middle, true)],
        ..Default::default()
    };
    app.caption_drag(&ctx, &mut raw);
    assert_eq!(raw.events.len(), 1, "the press in the middle of the top moves the window");
}

/// Asked: "a aba de tarefas, ali no canvas remove por favor". Only Alterações and Nota rápida have a button; the tasks
/// still reach the agents.
#[test]
fn the_tasks_panel_is_gone_and_the_tasks_stay() {
    assert_eq!(PANES.map(|pane| pane.2), ["Alterações", "Nota rápida"]);
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    settle(&mut app, &ctx);
    let shown = pane_frame(&mut app, &ctx, vec![]);
    assert!(!texts(&shown).iter().any(|text| text == "Tarefas"), "{:?}", texts(&shown));
    app.act("tasks", &json!({ "action": "create", "title": "Revisar o login" })).unwrap();
    assert_eq!(app.tasks.list.len(), 1);
}

#[test]
fn a_changed_file_shows_its_lines_and_diff_and_opens_in_the_tree() {
    if crate::editor::command("git").arg("--version").output().is_err() {
        return;
    }
    let dir = temp("panes-changes");
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "t"]);
    fs::write(dir.join("kept.txt"), "um\ndois\ntrês\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "first"]);
    fs::write(dir.join("kept.txt"), "um\n2\ntrês\nquatro\n").unwrap();
    let (ctx, mut app, _) = opened(Pane::Changes);
    let id = tree_on(&mut app, &dir);
    app.filetrees.views.get_mut(&id).unwrap().git = crate::app::filetree::view::TreeGit::read(&dir);
    assert_eq!(app.changed_file_count(), 1, "the badge's count");
    // the lines are counted away from the frame
    let mut shown = pane_frame(&mut app, &ctx, vec![]);
    for _ in 0..300 {
        if app.change_line_counts(&dir.join("kept.txt")).is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        shown = pane_frame(&mut app, &ctx, vec![]);
    }
    assert_eq!(app.change_line_counts(&dir.join("kept.txt")), Some((2, 1)));
    let row = find(&shown, Role::Button, "kept.txt").expect("the file's row");
    pane_click(&mut app, &ctx, row.center());
    let diff: Vec<(char, &str)> = app.panes.changes.diff.iter().map(|l| (l.mark, l.text.as_str())).collect();
    assert!(diff.contains(&('-', "dois")) && diff.contains(&('+', "2")) && diff.contains(&('+', "quatro")), "{diff:?}");
    let shown = pane_frame(&mut app, &ctx, vec![]);
    assert!(find(&shown, Role::Button, "Descartar").is_none(), "no such action in the app");
    pane_click(&mut app, &ctx, find(&shown, Role::Button, "Abrir na Árvore").unwrap().center());
    assert_eq!(app.terminal_elevated.as_deref(), Some(id.as_str()), "the tree enlarged");
    assert!(app.filetrees.editors[&id].tabs.iter().any(|t| t.path.to_string_lossy().contains("HEAD ↔ kept.txt")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_narrow_window_keeps_the_toolbar_clear_of_the_panel_buttons() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    let narrow = Vec2::new(1000.0, 700.0);
    for _ in 0..4 {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, narrow)), ..Default::default() };
        let _ = ctx.run_ui(raw, |_| app.floating_controls(&ctx));
    }
    let tools = crate::app::ui::tour::spot_rect(&ctx, crate::app::ui::tour::Spot::Toolbar).expect("the toolbar was painted");
    let buttons = app.pane_button(narrow.x, 0);
    assert!(tools.right() < buttons.left(), "{tools:?} runs into {buttons:?}");
    assert!(tools.left() > app.sidebar_width(), "still over the canvas: {tools:?}");
}

/// Asked: "uma nota rápida de fácil acesso, ali no botão de alteração … um botão a gente leva essa nota para dentro do
/// canvas … não quero que esteja em markdown". Its button opens it ready to type; what is typed stays as typed (the
/// canvas's letter keys don't take it); Levar para o canvas makes it a note shown as typed, selected, and empties it.
#[test]
fn nota_rapida_keeps_plain_text_and_goes_to_the_canvas_as_a_note() {
    let (ctx, mut app, shown) = opened(Pane::QuickNote);
    assert!(texts(&shown).iter().any(|text| text == "Vazia"));
    let tool = app.tool;
    pane_frame(&mut app, &ctx, vec![egui::Event::Text("# lista h".into())]);
    pane_frame(&mut app, &ctx, vec![key_press(Key::Enter), egui::Event::Text("- **leite**".into())]);
    assert_eq!(app.prefs.quick_note, "# lista h\n- **leite**");
    assert_eq!(app.tool, tool, "the canvas's keys stay out of it");
    let shown = pane_frame(&mut app, &ctx, vec![]);
    assert!(texts(&shown).iter().any(|text| text == "2 linhas"));
    let take = find(&shown, Role::Button, "Levar para o canvas").expect("Levar para o canvas");
    pane_click(&mut app, &ctx, take.center());
    let note = app.scene.elements.iter().position(|element| element.note.raw).expect("a note shown as typed");
    let label = &app.scene.elements[note + 1];
    assert!(matches!(&label.kind, ElementKind::Text { value, .. } if value == "# lista h\n- **leite**"));
    assert_eq!((app.selected.as_slice(), app.prefs.quick_note.as_str()), ([note].as_slice(), ""));
    // empty, there is nothing to take
    assert!(app.quick_note_to_canvas().unwrap_err().contains("vazia"));
}

/// The MCP's `quick_note`, as the person uses it.
#[test]
fn the_mcp_reads_writes_and_takes_the_nota_rapida() {
    let mut app = DrawingApp::default();
    let act = |app: &mut DrawingApp, args: Value| app.act("quick_note", &args);
    assert_eq!(act(&mut app, json!({ "action": "get" })), Ok("A nota rápida está vazia.".into()));
    act(&mut app, json!({ "action": "set", "text": "comprar pão" })).unwrap();
    act(&mut app, json!({ "action": "append", "text": "ligar para a Bia" })).unwrap();
    assert_eq!(act(&mut app, json!({ "action": "get" })), Ok("comprar pão\nligar para a Bia".into()));
    act(&mut app, json!({ "action": "show" })).unwrap();
    assert_eq!(app.panes.open, Some(Pane::QuickNote));
    assert_eq!(app.act("panel", &json!({ "name": "note", "open": false })), Ok("Painel Nota rápida fechado.".into()));
    act(&mut app, json!({ "action": "to_canvas" })).unwrap();
    assert!(app.scene.elements.iter().any(|element| matches!(&element.kind, ElementKind::Text { value, .. } if value == "comprar pão\nligar para a Bia")));
    assert!(act(&mut app, json!({ "action": "apagar" })).unwrap_err().contains("show, get, set, append ou to_canvas"));
}
