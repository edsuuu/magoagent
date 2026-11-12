use super::*;

mod arrows;
mod chat;
mod chat_node;
mod conflicts;
mod dialogs;
mod drops;
mod filetree;
mod filetree_annotate;
mod filetree_branch;
mod filetree_changes;
mod filetree_code;
mod filetree_db;
mod filetree_diff;
mod filetree_link;
mod filetree_problems;
mod frames;
mod gestures;
mod groups;
mod layout;
mod links;
mod locks;
mod menu;
mod mini;
mod minimap;
mod nodes;
mod notes;
mod notes_rail;
mod pages;
mod persist;
mod props;
mod session;
mod shapes;
mod terminals;
mod tools;
mod typing;
mod windows;

/// One frame of the canvas; gives the cursor it asked for.
fn frame(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) -> egui::CursorIcon {
    frame_at(app, ctx, None, events)
}

/// A frame at a given time, for gestures that depend on timing (double-click).
fn frame_at(app: &mut DrawingApp, ctx: &Context, time: Option<f64>, events: Vec<egui::Event>) -> egui::CursorIcon {
    let mut output =
        ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), time, events, ..Default::default() }, |ui| {
            app.screen_keys(ctx);
            app.handle_shortcuts(ctx);
            egui::CentralPanel::no_frame().show(ui, |ui| {
                let (response, _) = ui.allocate_painter(ui.available_size(), Sense::CLICK | Sense::DRAG);
                app.handle_canvas(&response, response.rect);
                app.update_cursor(ctx, &response);
            });
            app.text_editor(ctx);
            app.note_buttons_ui(ctx);
            app.floating_controls(ctx);
        });
    output.textures_delta.clear();
    output.platform_output.cursor_icon
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

fn drag(app: &mut DrawingApp, ctx: &Context, start: Pos2, end: Pos2) {
    frame(app, ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
    frame(app, ctx, vec![egui::Event::PointerMoved(start + Vec2::new(12.0, 12.0))]);
    frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
    frame(app, ctx, vec![button(end, false)]);
}

fn click(app: &mut DrawingApp, ctx: &Context, pos: Pos2) {
    frame(app, ctx, vec![egui::Event::PointerMoved(pos), button(pos, true)]);
    frame(app, ctx, vec![button(pos, false)]);
}

/// Two presses inside 0.2 s on `at`, from time `t`.
fn double_click(app: &mut DrawingApp, ctx: &Context, at: Pos2, t: f64) {
    for (dt, pressed) in [(0.0, true), (0.05, false), (0.15, true), (0.2, false)] {
        frame_at(app, ctx, Some(t + dt), vec![egui::Event::PointerMoved(at), button(at, pressed)]);
    }
}

fn two_boxes(app: &mut DrawingApp) {
    app.scene.elements = vec![
        app.new_element(ElementKind::Rectangle { x: 300.0, y: 200.0, width: 100.0, height: 100.0 }),
        app.new_element(ElementKind::Rectangle { x: 600.0, y: 200.0, width: 100.0, height: 100.0 }),
    ];
}

/// A terminal made the way the dialog makes one, for the keyboard tests below.
fn terminal_at(app: &mut DrawingApp, ctx: &Context, at: Pos2) -> String {
    app.select_tool(Tool::Terminal);
    drag(app, ctx, at, at + Vec2::new(1.0, 0.0));
    app.terminal_dialog.as_mut().unwrap().spec.command = "claude".into();
    app.confirm_terminal_dialog();
    app.select_tool(Tool::Select);
    app.scene.elements.last().unwrap().id.clone()
}

fn key_press(key: Key) -> egui::Event {
    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// One pass that only paints the canvas; what it painted, in shapes.
fn painted(app: &DrawingApp, ctx: &Context) -> usize {
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| app.paint_scene(ui.painter(), ui.max_rect()));
    out.textures_delta.clear();
    out.shapes.len()
}
