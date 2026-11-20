use super::*;

mod arrange;
mod arrows;
mod changelog;
mod changes_pane;
mod chat;
mod chat_card;
mod chat_images;
mod chat_node;
mod chat_queue;
mod chats_view;
mod commands;
mod conflicts;
mod dialogs;
mod doubts;
mod drops;
mod file_links;
mod filetree;
mod filetree_annotate;
mod filetree_branch;
mod filetree_changes;
mod filetree_code;
mod filetree_diff;
mod filetree_link;
mod filetree_problems;
mod frames;
mod gestures;
mod groups;
mod island;
mod island_agents;
mod island_buttons;
mod island_music;
mod island_open;
mod island_rest;
mod layout;
mod links;
mod locks;
mod mascots;
mod menu;
mod mini;
mod minimap;
mod nodes;
mod notes;
mod notes_rail;
mod notes_reading;
mod outside;
mod outside_message;
mod pages;
mod panes;
mod persist;
mod props;
mod session;
mod shapes;
mod sidebar;
mod spaces_away;
mod stacking;
mod stop_all;
mod team;
mod terminals;
mod tools;
mod tour;
mod typing;
mod usage_glance;
mod windows;
mod zoom;

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
    // the toolbar's dialog opens on Chat: its Terminal side
    let dialog = app.terminal_dialog.as_mut().unwrap();

    (dialog.chat, dialog.spec.command) = (false, "claude".into());
    app.confirm_terminal_dialog();
    app.select_tool(Tool::Select);
    app.scene.elements.last().unwrap().id.clone()
}

fn key_press(key: Key) -> egui::Event {
    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// The app on a drawing of its own in a fresh folder `mago-<name>-<pid>` of the temp dir, with its spaces loaded; the folder.
pub(crate) fn app_in_folder(name: &str) -> (DrawingApp, PathBuf) {
    let dir = std::env::temp_dir().join(format!("mago-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    fs::create_dir_all(&dir).unwrap();
    // today's drawing already there: with none, it is a first run and there is no space yet
    fs::write(dir.join("autosave.mago"), r#"{"version":1,"elements":[]}"#).unwrap();
    let mut app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };

    app.load_spaces();
    app.load_autosave();
    (app, dir)
}

/// The screen the island's tests run it on.
const ISLAND_SCREEN: Vec2 = Vec2::new(1200.0, 800.0);

/// A chat of the app's list, in folder C:/p.
fn chat(id: &str, agent: &str, name: &str) -> crate::app::chat::direct::Chat {
    crate::app::chat::direct::Chat {
        id: id.into(),
        agent: agent.into(),
        name: name.into(),
        cwd: "C:/p".into(),
        session: None,
        mode: 0,
        model: String::new(),
        effort: String::new(),
        recruit: false,
        named: false,
        space: String::new(),
        draft: false,
    }
}

/// A Claude chat named `name` (its id the name in lower case) put on the app's list; its id.
fn push_chat(app: &mut DrawingApp, name: &str) -> String {
    app.chats.push(crate::app::chat::direct::Chat { cwd: String::new(), ..chat(&name.to_lowercase(), "claude", name) });
    name.to_lowercase()
}

/// One frame of the island alone; the texts it painted and where.
fn island_frame(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, ISLAND_SCREEN)), time: Some(time), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.island_ui(ctx));

    output.textures_delta.clear();

    output
        .shapes
        .iter()
        .filter_map(|shape| if let egui::Shape::Text(text) = &shape.shape { Some((text.galley.text().to_string(), text.visual_bounding_rect())) } else { None })
        .collect()
}

/// One frame of the island alone; the texts it painted.
fn island_texts(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) -> Vec<String> {
    island_frame(app, ctx, time, events).into_iter().map(|(text, _)| text).collect()
}

/// The island as drawn: hung from the top centre.
fn island_rect(app: &DrawingApp) -> Rect {
    let size = app.island.size();

    Rect::from_min_size(Pos2::new((ISLAND_SCREEN.x / 2.0 - size.x / 2.0).round(), 0.0), size)
}

/// A header button of the island, counted from the right: 0 the close, 1 the sound, 2 the settings, 3 the voice.
/// The island header's buttons at its right, right to left: Recolher, sound, Ajustes, Chat por voz (while the call is on
/// in the code, `voice::CALL_ON`).
const RIGHT_BUTTONS: usize = 3 + crate::app::voice::CALL_ON as usize;

fn right_button(app: &DrawingApp, index: usize) -> Pos2 {
    let rect = island_rect(app);

    Pos2::new(rect.right() - 25.0 - index as f32 * 32.0, rect.top() + 25.0)
}

/// The app with `chats`, its island opened by clicking on the compact pill, and the time it is at.
fn island_opened(chats: Vec<crate::app::chat::direct::Chat>) -> (DrawingApp, Context, f64) {
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.prefs.island = true;
    app.chats = chats;
    let mut time = 1.0;
    let notch = Pos2::new(ISLAND_SCREEN.x / 2.0, 10.0);

    while time < 2.0 {
        island_frame(&mut app, &ctx, time, vec![egui::Event::PointerMoved(notch)]);
        time += 0.05;
    }

    assert!((app.island.size().x - crate::app::island::COMPACT_W).abs() < 4.0, "compact on hover: {}", app.island.size().x);
    island_frame(&mut app, &ctx, time, vec![button(notch, true)]);
    time += 0.05;
    island_frame(&mut app, &ctx, time, vec![button(notch, false)]);
    time += 0.05;

    while time < 3.0 {
        island_frame(&mut app, &ctx, time, vec![egui::Event::PointerMoved(notch)]);
        time += 0.05;
    }

    assert!((app.island.size().x - crate::app::island::EXPANDED_W).abs() < 1.0, "clicked to open: {}", app.island.size().x);
    (app, ctx, time)
}

/// One pass that only paints the canvas; what it painted, in shapes.
fn painted(app: &DrawingApp, ctx: &Context) -> usize {
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| app.paint_scene(ui.painter(), ui.max_rect()));

    out.textures_delta.clear();
    out.shapes.len()
}
