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
    // it opens on Chat; these are the terminal's
    app.terminal_dialog.as_mut().unwrap().chat = false;
    dialog_frame(&mut app, &ctx, vec![]);
    dialog_frame(&mut app, &ctx, vec![]);
    let sheet = ctx.memory(|m| m.area_rect(egui::Id::unique(("dialog", "Novo Terminal")))).expect("the sheet is on screen");

    assert_eq!(sheet.width(), 480.0);
    assert!((sheet.center().x - 600.0).abs() < 1.0 && (sheet.center().y - 400.0).abs() < 1.0, "centered: {sheet:?}");
    // Codex, the second quick start of the row, under the Terminal | Chat tabs (29 and 13 under them)
    // the row (Claude Code, Codex, OpenCode, Shell) sits in the middle of the 454 wide column
    let tile = (454.0 - 4.0 * 3.25) / 5.0;
    let row_left = sheet.left() + 13.0 + (454.0 - (4.0 * tile + 3.0 * 3.25)) / 2.0;

    dialog_click(&mut app, &ctx, Pos2::new(row_left + 1.5 * tile + 3.25, sheet.top() + 44.0 + 13.0 + 42.0 + 15.0 + 4.875 + 27.0));
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
    // it opens on Chat; these are the terminal's
    app.terminal_dialog.as_mut().unwrap().chat = false;
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
    // it opens on Chat; these are the terminal's
    app.terminal_dialog.as_mut().unwrap().chat = false;

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

    click(&mut app, Pos2::new(sheet.right() - 13.0 - 30.0, sheet.top() + 44.0 + 13.0 + 42.0 + 7.5));
    assert!(app.settings_open, "the link opens Ajustes");

    for _ in 0..6 {
        frame(&mut app, vec![]);
    }

    // the X of Ajustes: 28 square, 18 from the corner of the 880 x 688 window (86% of 800) in the middle
    click(&mut app, Pos2::new(600.0 + 440.0 - 18.0 - 14.0, 400.0 - 344.0 + 18.0 + 14.0));
    assert!(!app.settings_open, "Ajustes answers its X");
    assert!(app.terminal_dialog.is_some(), "the sheet is back");
}

/// The folder field of the file tree's sheet: a click in it lists the recent folders under it, inside the
/// sheet (no button to press, no menu out of it), and a click on one puts it in the field.
#[test]
fn a_click_in_the_folder_field_lists_the_recent_ones_inside_the_sheet() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let home = std::env::temp_dir().to_string_lossy().trim_end_matches(['/', '\\']).to_string();

    app.prefs.folders.remember(&home);
    app.open_filetree_dialog(Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    app.filetrees.dialog.as_mut().unwrap().root.clear();
    // the time moves on, for the list's unfolding
    let clock = std::cell::Cell::new(0.0);

    let frame = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        clock.set(clock.get() + 0.05);

        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            time: Some(clock.get()),
            events,
            ..Default::default()
        };

        let mut output = ctx.run_ui(raw, |_| app.filetree_dialog_ui(&ctx));

        output.textures_delta.clear();
    };

    let press = |app: &mut DrawingApp, at: Pos2| {
        frame(app, vec![egui::Event::PointerMoved(at)]);
        frame(app, vec![button(at, true)]);
        frame(app, vec![button(at, false)]);

        for _ in 0..6 {
            frame(app, vec![]);
        }
    };

    frame(&mut app, vec![]);
    frame(&mut app, vec![]);
    let sheet = || ctx.memory(|m| m.area_rect(egui::Id::unique(("dialog", "Nova Árvore de Arquivos")))).expect("the sheet is on screen");
    let before = sheet();
    // the field, under the caption, left of Procurar...
    let field_y = before.top() + 44.0 + 16.25 + 18.2 + 4.875 + 15.0;

    press(&mut app, Pos2::new(before.left() + 13.0 + 60.0, field_y));
    let after = sheet();

    assert!(after.height() > before.height() + 40.0, "the list is in the sheet, which grew: {before:?} {after:?}");
    assert!(!egui::Popup::is_any_open(&ctx), "no menu over it");
    // its one row, under the field (the sheet, grown, is centered again)
    let field_y = after.top() + 44.0 + 16.25 + 18.2 + 4.875 + 15.0;

    press(&mut app, Pos2::new(after.left() + 13.0 + 120.0, field_y + 15.0 + 6.0 + 4.0 + 20.0));
    assert_eq!(app.filetrees.dialog.as_ref().unwrap().root, home);
}

/// A press on Ajustes' veil that drags instead of clicking (it doesn't close) leaves the sheet on top:
/// its tabs still answer (the veil came up over it and took every click).
#[test]
fn a_drag_on_the_settings_veil_leaves_its_buttons_answering() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.settings_open = true;

    let frame = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), events, ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| app.floating_controls(&ctx));

        output.textures_delta.clear();
    };

    for _ in 0..8 {
        frame(&mut app, vec![]);
    }

    // pressed outside the sheet, dragged, let go: no click, Ajustes stays
    frame(&mut app, vec![egui::Event::PointerMoved(Pos2::new(60.0, 60.0))]);
    frame(&mut app, vec![button(Pos2::new(60.0, 60.0), true)]);

    for step in 1..=5 {
        frame(&mut app, vec![egui::Event::PointerMoved(Pos2::new(60.0 + 12.0 * step as f32, 60.0 + 8.0 * step as f32))]);
    }

    frame(&mut app, vec![button(Pos2::new(120.0, 100.0), false)]);
    frame(&mut app, vec![]);
    assert!(app.settings_open, "a drag on the veil doesn't close it");
    // Terminal, the second row of the sidebar (34 high, 2 apart, from 68 under the top of the 880 x 688 window)
    let tab = Pos2::new(600.0 - 440.0 + 60.0, 400.0 - 344.0 + 68.0 + 36.0 + 17.0);

    frame(&mut app, vec![egui::Event::PointerMoved(tab)]);
    frame(&mut app, vec![button(tab, true)]);
    frame(&mut app, vec![button(tab, false)]);
    frame(&mut app, vec![]);
    assert_eq!(crate::app::ui::settings::SETTINGS_TABS[app.settings_tab], "Terminal", "the sheet's tabs answer");
}
