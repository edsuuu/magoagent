use super::*;

/// One frame of the terminal dialog alone, on a 1200 x 800 window.
fn dialog_frame(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.terminal_dialog_ui(ctx));
    output.textures_delta.clear();
}

fn dialog_click(app: &mut DrawingApp, ctx: &Context, at: Pos2) {
    dialog_frame(app, ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    dialog_frame(app, ctx, vec![button(at, false)]);
}

/// The sheet is 480 wide in the middle of the window, with Cancelar, the title and Criar on its bar:
/// pick a quick start, name it and press Criar with the mouse.
#[test]
fn the_new_terminal_sheet_creates_a_terminal_by_click() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.open_terminal_dialog(Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    dialog_frame(&mut app, &ctx, vec![]);
    dialog_frame(&mut app, &ctx, vec![]);
    let sheet = ctx.memory(|m| m.area_rect(egui::Id::unique(("dialog", "Novo Terminal")))).expect("the sheet is on screen");
    assert_eq!(sheet.width(), 480.0);
    assert!((sheet.center().x - 600.0).abs() < 1.0 && (sheet.center().y - 400.0).abs() < 1.0, "centered: {sheet:?}");
    // Codex, the second quick start of the row
    dialog_click(&mut app, &ctx, Pos2::new(sheet.left() + 13.0 + 1.5 * 91.45, sheet.top() + 44.0 + 13.0 + 15.0 + 4.875 + 27.0));
    assert_eq!(app.terminal_dialog.as_ref().unwrap().spec.command, "codex");
    // the name field has the keyboard; its text is replaced by typing over a cleared one
    app.terminal_dialog.as_mut().unwrap().spec.name.clear();
    dialog_frame(&mut app, &ctx, vec![egui::Event::Text("Revisor".into())]);
    assert_eq!(app.terminal_dialog.as_ref().unwrap().spec.name, "Revisor");
    dialog_click(&mut app, &ctx, Pos2::new(sheet.right() - 9.0 - 25.0, sheet.top() + 26.5));
    assert!(app.terminal_dialog.is_none(), "Criar closes the sheet");
    let terminal = app.scene.elements.last().and_then(|e| e.terminal.clone()).expect("a terminal was created");
    assert_eq!((terminal.name.as_str(), terminal.command.as_str()), ("Revisor", "codex"));
}

#[test]
fn cancelar_closes_the_sheet_and_creates_nothing() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.open_terminal_dialog(Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    dialog_frame(&mut app, &ctx, vec![]);
    dialog_frame(&mut app, &ctx, vec![]);
    let sheet = ctx.memory(|m| m.area_rect(egui::Id::unique(("dialog", "Novo Terminal")))).unwrap();
    dialog_click(&mut app, &ctx, Pos2::new(sheet.left() + 9.0 + 30.0, sheet.top() + 26.5));
    assert!(app.terminal_dialog.is_none() && app.scene.elements.is_empty());
}

/// "Editar presets…" opens Ajustes over the terminal sheet: the sheet steps aside, Ajustes answers
/// its X, and the sheet comes back.
#[test]
fn editing_presets_opens_a_settings_that_can_be_closed() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.open_terminal_dialog(Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    let frame = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), events, ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| app.floating_controls(&ctx));
        output.textures_delta.clear();
    };
    let click = |app: &mut DrawingApp, at: Pos2| {
        frame(app, vec![egui::Event::PointerMoved(at)]);
        frame(app, vec![button(at, true)]);
        frame(app, vec![button(at, false)]);
        frame(app, vec![]);
    };
    frame(&mut app, vec![]);
    frame(&mut app, vec![]);
    let sheet = ctx.memory(|m| m.area_rect(egui::Id::unique(("dialog", "Novo Terminal")))).expect("the sheet is on screen");
    click(&mut app, Pos2::new(sheet.right() - 13.0 - 30.0, sheet.top() + 44.0 + 13.0 + 7.5));
    assert!(app.settings_open, "the link opens Ajustes");
    for _ in 0..6 {
        frame(&mut app, vec![]);
    }
    // the X of Ajustes: 9 from the corner of the 760 x 640 sheet in the middle
    click(&mut app, Pos2::new(600.0 + 380.0 - 9.0 - 12.0, 400.0 - 320.0 + 9.0 + 12.0));
    assert!(!app.settings_open, "Ajustes answers its X");
    assert!(app.terminal_dialog.is_some(), "the sheet is back");
}

/// Recentes in the file tree's sheet: the menu opens over the sheet (the sheet used to climb back
/// over it every frame) and picks a folder into the field.
#[test]
fn the_recent_folders_menu_stays_over_the_new_tree_sheet() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let home = std::env::temp_dir();
    app.prefs.folders.remember(&home.to_string_lossy());
    app.open_filetree_dialog(Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    app.filetrees.dialog.as_mut().unwrap().root.clear();
    let frame = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), events, ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| app.filetree_dialog_ui(&ctx));
        output.textures_delta.clear();
    };
    frame(&mut app, vec![]);
    frame(&mut app, vec![]);
    let sheet = ctx.memory(|m| m.area_rect(egui::Id::unique(("dialog", "Nova Árvore de Arquivos")))).expect("the sheet is on screen");
    // Recentes sits left of Procurar... in the folder row, under the caption
    let at = Pos2::new(sheet.right() - 13.0 - 83.0 - 5.0 - 39.0, sheet.top() + 44.0 + 16.25 + 18.2 + 4.875 + 15.0);
    frame(&mut app, vec![egui::Event::PointerMoved(at)]);
    frame(&mut app, vec![button(at, true)]);
    frame(&mut app, vec![button(at, false)]);
    for _ in 0..3 {
        frame(&mut app, vec![]);
    }
    assert!(egui::Popup::is_any_open(&ctx), "Recentes opens its menu");
    let top = ctx.memory(|m| m.layer_ids().last()).unwrap();
    assert!(top != egui::LayerId::new(egui::Order::Foreground, egui::Id::unique(("dialog", "Nova Árvore de Arquivos"))), "the menu is over the sheet");
}
