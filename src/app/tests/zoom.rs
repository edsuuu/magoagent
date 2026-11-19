//! The wheel and the zoom on the canvas (canvas/wheel.rs): a gesture latched to the canvas over the windows it passes,
//! Ctrl + wheel, the pinch and the zoom's keys whoever has the keys, an enlarged tree's own zoom; the text laid out at
//! a held zoom while the zoom moves (render/zoom.rs); the editor of a long note over its whole text box.

use super::*;
use crate::app::canvas::wheel::wheel_modifiers;

/// A window over the canvas: a scroll area drawn after it, as a tree's, a chat card's or a terminal's.
const WINDOW: Rect = Rect { min: Pos2::new(450.0, 100.0), max: Pos2::new(650.0, 300.0) };

/// One frame of the canvas (painted, its wheel taken) and of the window over it, at `time` with the pointer at `at`:
/// where the window's content is scrolled to.
fn wheel_frame(app: &mut DrawingApp, ctx: &Context, time: f64, at: Pos2, mut events: Vec<egui::Event>) -> f32 {
    events.insert(0, egui::Event::PointerMoved(at));
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let raw = egui::RawInput { screen_rect: Some(screen), time: Some(time), events, ..Default::default() };
    let mut offset = 0.0;
    let mut output = ctx.run_ui(raw, |ui| {
        app.now = time;
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::CLICK | Sense::DRAG);
            app.handle_canvas(&response, response.rect);
            app.paint_scene(&painter, response.rect);
            app.canvas_wheel(ctx, &response, response.rect);
        });
        egui::Area::new(egui::Id::unique("janela")).fixed_pos(WINDOW.min).show(ctx, |ui| {
            ui.set_width(WINDOW.width());
            ui.set_height(WINDOW.height());
            let shown = egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                for line in 0..100 {
                    ui.label(format!("linha {line}"));
                }
            });
            offset = shown.state.clamped_offset().y;
        });
    });
    output.textures_delta.clear();
    offset
}

fn wheel(delta: Vec2, modifiers: Modifiers) -> egui::Event {
    let (unit, phase, source) = (egui::MouseWheelUnit::Point, egui::TouchPhase::Move, egui::MouseWheelSource::Wheel);
    egui::Event::MouseWheel { unit, delta, phase, source, modifiers }
}

/// A notch of the wheel at `at` and the frames egui spreads it over (a third of a second): the window's scroll after.
fn notch(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2, modifiers: Modifiers) -> f32 {
    let mut offset = 0.0;
    for step in 0..20 {
        let events = if step == 0 { vec![egui::Event::ModifiersChanged(modifiers), wheel(Vec2::new(0.0, -60.0), modifiers)] } else { Vec::new() };
        offset = wheel_frame(app, ctx, *time, at, events);
        *time += 1.0 / 60.0;
    }
    offset
}

/// Frames with nothing happening for `seconds`: the gesture pauses.
fn pause(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2, seconds: f64) {
    for _ in 0..3 {
        *time += seconds / 3.0;
        wheel_frame(app, ctx, *time, at, vec![egui::Event::ModifiersChanged(Modifiers::NONE)]);
    }
}

#[test]
fn a_wheel_started_on_the_canvas_keeps_moving_it_over_a_window_until_it_pauses_or_a_click() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let (canvas, window) = (Pos2::new(200.0, 300.0), WINDOW.center());
    let mut time = 1.0;
    pause(&mut app, &ctx, &mut time, canvas, 0.1);
    // on the canvas: it pans, and the windows drawn after it no longer see the wheel
    let start = app.pan;
    assert_eq!(notch(&mut app, &ctx, &mut time, canvas, Modifiers::NONE), 0.0);
    assert!(app.pan.y < start.y - 30.0, "the canvas panned: {start:?} → {:?}", app.pan);
    // the same gesture goes on over the window: still the canvas's
    let panned = app.pan;
    assert_eq!(notch(&mut app, &ctx, &mut time, window, Modifiers::NONE), 0.0, "the window kept still");
    assert!(app.pan.y < panned.y - 30.0, "the canvas went on: {panned:?} → {:?}", app.pan);
    // a pause: the window under the pointer takes the next wheel
    pause(&mut app, &ctx, &mut time, window, 0.6);
    let still = app.pan;
    let scrolled = notch(&mut app, &ctx, &mut time, window, Modifiers::NONE);
    assert!(scrolled > 30.0, "the window scrolled: {scrolled}");
    assert_eq!(app.pan, still, "the canvas kept still");
    // a click ends the canvas's gesture at once
    notch(&mut app, &ctx, &mut time, canvas, Modifiers::NONE);
    for pressed in [true, false] {
        wheel_frame(&mut app, &ctx, time, window, vec![button(window, pressed)]);
        time += 0.02;
    }
    let (still, before) = (app.pan, scrolled);
    assert!(notch(&mut app, &ctx, &mut time, window, Modifiers::NONE) > before + 30.0);
    assert_eq!(app.pan, still);
}

#[test]
fn ctrl_wheel_and_the_pinch_zoom_the_canvas_over_a_window_whoever_has_the_keys() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let window = WINDOW.center();
    let mut time = 1.0;
    pause(&mut app, &ctx, &mut time, window, 0.1);
    // a tree has the keys (its card was clicked), the pointer is over a window
    app.card_focus = Some("arvore".into());
    assert_eq!(notch(&mut app, &ctx, &mut time, window, Modifiers::COMMAND), 0.0, "the window does not scroll");
    assert!(app.zoom < 0.95, "Ctrl + wheel zoomed the canvas out: {}", app.zoom);
    pause(&mut app, &ctx, &mut time, window, 0.6);
    let zoom = app.zoom;
    wheel_frame(&mut app, &ctx, time, window, vec![egui::Event::Zoom(1.25)]);
    assert!(app.zoom > zoom + 0.05, "the pinch zoomed it in: {zoom} → {}", app.zoom);
}

#[test]
fn the_zoom_keys_reach_the_canvas_from_a_tree_or_a_field_and_an_enlarged_tree_zooms_itself() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let ctrl = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    let mut tree = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 400.0, height: 300.0 });
    tree.filetree = Some(Default::default());
    let id = tree.id.clone();
    app.scene.elements.push(tree);
    // the tree has the keys: Ctrl + - and Ctrl + = still zoom the canvas, Ctrl + 0 brings it back
    app.card_focus = Some(id.clone());
    frame(&mut app, &ctx, vec![ctrl(Key::Minus)]);
    assert_eq!(app.zoom, 0.9);
    frame(&mut app, &ctx, vec![ctrl(Key::Equals)]);
    frame(&mut app, &ctx, vec![ctrl(Key::Equals)]);
    assert_eq!(app.zoom, 1.1);
    frame(&mut app, &ctx, vec![ctrl(Key::Num0)]);
    assert_eq!(app.zoom, 1.0);
    // a terminal reads them itself
    app.card_focus = None;
    app.terminal_focus = Some("terminal".into());
    frame(&mut app, &ctx, vec![ctrl(Key::Minus)]);
    assert_eq!(app.zoom, 1.0);
    app.terminal_focus = None;
    // enlarged, the tree has a zoom of its own: the keys and Ctrl + wheel over it change it, not the canvas's
    app.card_focus = Some(id.clone());
    app.set_elevated(Some(id.clone()));
    // risen already (each frame reads the clock)
    app.elevate_since = f64::NEG_INFINITY;
    frame(&mut app, &ctx, vec![ctrl(Key::Equals)]);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&id));
    assert!((app.lifted_zoom - 1.1).abs() < 1e-5 && app.zoom == 1.0, "{} {}", app.lifted_zoom, app.zoom);
    let (frame_rect, zoom) = app.terminal_frame(&app.scene.elements[0]);
    assert!((zoom - 1.1).abs() < 1e-5, "its code and rows are drawn at its zoom: {zoom}");
    let mut time = 10.0;
    wheel_frame(&mut app, &ctx, time, frame_rect.center(), vec![]);
    time += 0.1;
    notch(&mut app, &ctx, &mut time, frame_rect.center(), Modifiers::COMMAND);
    assert!(app.lifted_zoom < 1.0 && app.zoom == 1.0, "Ctrl + wheel zoomed the tree out, not the canvas: {} {}", app.lifted_zoom, app.zoom);
    frame(&mut app, &ctx, vec![ctrl(Key::Num0)]);
    assert_eq!(app.lifted_zoom, 1.0);
}

#[test]
fn a_wheel_over_the_window_while_another_has_the_keyboard_carries_the_keys_held() {
    let ctrl = Modifiers::COMMAND;
    let scroll = || vec![egui::Event::PointerMoved(Pos2::ZERO), wheel(Vec2::new(0.0, -40.0), Modifiers::NONE)];
    // the window has the keyboard: winit's keys are right, nothing is asked
    let mut events = scroll();
    wheel_modifiers(&mut events, true, Modifiers::NONE, || panic!("not asked"));
    assert_eq!(events, scroll());
    // another one has it: the wheel carries the keys held, told before it
    let mut events = scroll();
    wheel_modifiers(&mut events, false, Modifiers::NONE, || Some(ctrl));
    assert_eq!(events, vec![egui::Event::PointerMoved(Pos2::ZERO), egui::Event::ModifiersChanged(ctrl), wheel(Vec2::new(0.0, -40.0), ctrl)]);
    // no wheel, nothing asked; no server, nothing changes
    let mut events = vec![egui::Event::PointerMoved(Pos2::ZERO)];
    wheel_modifiers(&mut events, false, Modifiers::NONE, || panic!("not asked"));
    let mut events = scroll();
    wheel_modifiers(&mut events, false, Modifiers::NONE, || None);
    assert_eq!(events, scroll());
}

#[test]
fn while_the_zoom_moves_the_text_is_laid_out_at_a_held_zoom_and_again_at_rest() {
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    let center = Pos2::new(400.0, 300.0);
    app.now = 10.0;
    assert_eq!(app.text_zoom(), 1.0);
    // a zoom on its way: the text holds the zoom it had, until it would grow past a quarter
    for (zoom, held) in [(1.1, 1.0), (1.2, 1.0), (1.25, 1.0), (1.3, 1.3), (1.5, 1.3), (1.7, 1.7)] {
        app.now += 0.03;
        app.zoom_at(zoom, center);
        assert_eq!(app.text_zoom(), held, "at {zoom}");
    }
    // at rest: laid out at its size, crisp
    app.now += 0.3;
    assert_eq!(app.text_zoom(), 1.7);
    // a window on its way up is at its own zoom
    assert_eq!(app.text_zoom_at(1.0), 1.0);
}

/// A long note's text: a title and `lines` lines, each wrapping into rows in a 300-point note.
fn long_text(lines: usize) -> String {
    let lines = (1..=lines).map(|line| format!("Linha {line}: um texto longo para testar a edição da nota e o corte do campo."));
    std::iter::once("# Notas longas".to_string()).chain(lines).collect::<Vec<_>>().join("\n")
}

/// The texts of a frame of the canvas painted at `zoom`, at rest or (`moving`) in the middle of a zoom from `held`.
fn painted_texts(app: &mut DrawingApp, ctx: &Context, zoom: f32, held: Option<f32>) -> Vec<(String, Vec2)> {
    app.now = 50.0;
    if let Some(held) = held {
        app.zoom_at(held, Pos2::ZERO);
        app.text_zoom();
    }
    app.zoom_at(zoom, Pos2::ZERO);
    if held.is_none() {
        app.now = 60.0;
    }
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| app.paint_scene(ui.painter(), screen));
    output.textures_delta.clear();
    let mut texts = Vec::new();
    for shape in &output.shapes {
        if let Shape::Text(text) = &shape.shape {
            texts.push((text.galley.text().to_owned(), text.galley.size()));
        }
    }
    texts
}

#[test]
fn a_long_notes_lines_break_in_the_same_places_at_every_zoom_and_only_those_in_sight_are_laid_out() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(300.0, 400.0)), long_text(200));
    app.scene.elements = vec![note, label];
    let first = |texts: &[(String, Vec2)]| texts.iter().find(|(text, _)| text.starts_with("Linha 1:")).cloned().expect("the first line is painted");
    let (at_rest, size) = first(&painted_texts(&mut app, &ctx, 1.0, None));
    assert!(at_rest.contains('\n'), "the line wraps in the note: {at_rest:?}");
    for zoom in [0.37, 0.73, 1.13, 1.61, 2.9] {
        let (text, scaled) = first(&painted_texts(&mut app, &ctx, zoom, None));
        assert_eq!(text, at_rest, "the rows break where they break at 100%, at {zoom}");
        assert!((scaled.y / zoom - size.y).abs() < size.y * 0.1, "{scaled:?} at {zoom}");
    }
    // in the middle of a zoom the text laid out at the held zoom is drawn scaled to the frame's
    let (text, scaled) = first(&painted_texts(&mut app, &ctx, 1.2, Some(1.0)));
    assert_eq!(text, at_rest);
    assert!((scaled.y - size.y * 1.2).abs() < 0.01, "{scaled:?} is {size:?} × 1.2");
    // of 200 lines, the ten or so in the card's box are laid out and painted
    let painted = painted_texts(&mut app, &ctx, 1.0, None).iter().filter(|(text, _)| text.starts_with("Linha")).count();
    assert!((5..=20).contains(&painted), "{painted} lines painted");
}

#[test]
fn a_scaled_galley_is_the_same_text_grown() {
    let ctx = Context::default();
    let mut galley = None;
    let _ = ctx.run_ui(Default::default(), |ui| {
        galley = Some(ui.painter().layout_no_wrap("Notas longas".into(), FontId::proportional(20.0), Color32::BLACK));
    });
    let galley = galley.unwrap();
    let grown = crate::render::zoom::scaled_galley(&galley, 2.0);
    assert_eq!(grown.text(), galley.text());
    assert_eq!(grown.size(), galley.size() * 2.0);
    let (row, grown_row) = (&galley.rows[0], &grown.rows[0]);
    assert_eq!(grown_row.row.visuals.mesh.vertices.len(), row.row.visuals.mesh.vertices.len());
    for (vertex, grown_vertex) in row.row.visuals.mesh.vertices.iter().zip(&grown_row.row.visuals.mesh.vertices) {
        assert_eq!(grown_vertex.pos, (vertex.pos.to_vec2() * 2.0).to_pos2());
        assert_eq!(grown_vertex.uv, vertex.uv, "the same glyph pictures");
    }
    assert!(std::sync::Arc::ptr_eq(&crate::render::zoom::scaled_galley(&galley, 1.0), &galley), "at 1 the same galley");
}

#[test]
fn a_200_line_notes_editor_covers_its_text_box_and_starts_where_it_was_read_at_any_zoom() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(100.0, 80.0), Vec2::new(300.0, 400.0)), long_text(200));
    let id = note.id.clone();
    app.scene.elements = vec![note, label];
    let editor = |ctx: &Context| ctx.memory(|memory| memory.area_rect(egui::Id::unique("note_editor")));
    let mut time = 1.0;
    // typed at a small zoom first (the editor's area keeps that size), then at larger ones after a deselect
    for zoom in [0.6, 1.0, 1.4] {
        app.zoom = zoom;
        app.pan = Vec2::ZERO;
        app.note_scroll.borrow_mut().entry(id.clone()).or_default().offset = 300.0;
        app.selected.clear();
        frame_at(&mut app, &ctx, Some(time), vec![]);
        // the resting note painted: what its card hides is learned there
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        ctx.run_ui(egui::RawInput { screen_rect: Some(screen), time: Some(time), ..Default::default() }, |ui| app.paint_scene(ui.painter(), screen))
            .textures_delta
            .clear();
        let body = app.to_screen(Pos2::new(250.0, 200.0));
        double_click(&mut app, &ctx, body, time);
        for _ in 0..2 {
            time += 0.5;
            frame_at(&mut app, &ctx, Some(time), vec![]);
        }
        assert!(app.editing.is_some(), "typing at {zoom}");
        let body = note_text_box(&app.scene.elements[0]);
        let text_box = Rect::from_two_pos(app.to_screen(body.min), app.to_screen(body.max));
        let shown = editor(&ctx).expect("the editor is drawn");
        // the text box, and the note's right margin for the scroll bar
        let with_bar = text_box.with_max_x(text_box.max.x + NOTE_PAD * zoom);
        assert!((shown.min - with_bar.min).length() < 0.5 && (shown.size() - with_bar.size()).length() < 0.5, "at {zoom}: {shown:?} is not {with_bar:?}");
        // the editor opened where the note was read, 300 points down, and kept it
        let offset = app.note_scroll.borrow()[&id].offset;
        assert!((offset - 300.0).abs() < 1.0, "at {zoom}: scrolled to {offset}");
        // the wheel scrolls the text being typed
        let wheel_at = app.to_screen(Pos2::new(250.0, 300.0));
        frame_at(&mut app, &ctx, Some(time), vec![egui::Event::PointerMoved(wheel_at), wheel(Vec2::new(0.0, -200.0), Modifiers::NONE)]);
        for _ in 0..30 {
            time += 1.0 / 60.0;
            frame_at(&mut app, &ctx, Some(time), vec![]);
        }
        let offset = app.note_scroll.borrow()[&id].offset;
        assert!(offset > 330.0, "at {zoom}: the wheel scrolled the text: {offset}");
        frame_at(&mut app, &ctx, Some(time), vec![key_press(Key::Escape)]);
        frame_at(&mut app, &ctx, Some(time + 0.1), vec![]);
        assert!(app.editing.is_none());
        time += 2.0;
    }
}
