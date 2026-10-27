//! Gestures on the canvas: what the cursor promises, what a press picks up, how a drag ends.

use super::*;

fn hover(app: &mut DrawingApp, ctx: &Context, world: Pos2) -> CursorIcon {
    let at = app.to_screen(world);
    frame(app, ctx, vec![egui::Event::PointerMoved(at)])
}

/// A box at `rect`, made into a window by `make`; gives its index.
fn window(app: &mut DrawingApp, rect: Rect, make: impl FnOnce(&mut Element)) -> usize {
    let mut e = app.new_element(ElementKind::Rectangle { x: rect.min.x, y: rect.min.y, width: rect.width(), height: rect.height() });
    make(&mut e);
    app.scene.elements.push(e);
    app.scene.elements.len() - 1
}

fn shell(e: &mut Element) {
    e.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
}

fn space(pressed: bool) -> egui::Event {
    egui::Event::Key { key: Key::Space, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE }
}

#[test]
fn the_cursor_shows_move_only_where_a_press_drags() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    window(&mut app, Rect::from_min_size(Pos2::new(40.0, 150.0), Vec2::new(300.0, 200.0)), shell);
    window(&mut app, Rect::from_min_size(Pos2::new(420.0, 150.0), Vec2::new(300.0, 200.0)), |e| {
        e.ide = Some(IdeSpec { name: "proj".into(), root: "/nonexistent-magoagent".into(), ..Default::default() });
    });
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(40.0, 400.0), Vec2::splat(160.0)), "# Plano".into());
    app.scene.elements.extend([note, label]);
    window(&mut app, Rect::from_min_size(Pos2::new(420.0, 420.0), Vec2::new(120.0, 80.0)), |_| {});
    for (world, icon, what) in [
        (Pos2::new(190.0, 160.0), CursorIcon::Move, "a terminal's title strip drags it"),
        (Pos2::new(190.0, 260.0), CursorIcon::Text, "a terminal's body takes the keys"),
        (Pos2::new(570.0, 160.0), CursorIcon::Move, "an IDE's title strip drags it"),
        (Pos2::new(570.0, 260.0), CursorIcon::Default, "an IDE's card is clicked, not dragged"),
        (Pos2::new(120.0, 406.0), CursorIcon::Move, "a note's top strip drags it"),
        (Pos2::new(120.0, 480.0), CursorIcon::Text, "a note's body types"),
        (Pos2::new(421.0, 460.0), CursorIcon::Move, "a shape drags from its outline"),
        (Pos2::new(380.0, 380.0), CursorIcon::Default, "the empty canvas"),
        (Pos2::new(340.0, 250.0), CursorIcon::ResizeHorizontal, "a window's edge resizes it"),
    ] {
        assert_eq!(hover(&mut app, &ctx, world), icon, "{what}");
    }
}

#[test]
fn space_pans_unless_a_terminal_has_the_keys() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let i = window(&mut app, Rect::from_min_size(Pos2::new(40.0, 150.0), Vec2::new(300.0, 200.0)), shell);
    let (at, to) = (Pos2::new(500.0, 400.0), Pos2::new(530.0, 420.0));
    assert_eq!(frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), space(true)]), CursorIcon::Grab);
    assert_eq!(frame(&mut app, &ctx, vec![button(at, true)]), CursorIcon::Grabbing);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
    frame(&mut app, &ctx, vec![button(to, false), space(false)]);
    assert_eq!(app.pan, to - at);
    // a focused terminal types the space: the canvas keeps its plain cursor and press
    app.terminal_focus = Some(app.scene.elements[i].id.clone());
    assert_eq!(frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), space(true)]), CursorIcon::Default);
    frame(&mut app, &ctx, vec![button(at, true)]);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
    frame(&mut app, &ctx, vec![button(to, false), space(false)]);
    assert_eq!(app.pan, to - at, "no pan this time");
}

#[test]
fn esc_mid_drag_puts_it_back_and_other_keys_wait() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    window(&mut app, Rect::from_min_size(Pos2::new(100.0, 150.0), Vec2::new(300.0, 200.0)), shell);
    let before = local_bounds(&app.scene.elements[0]);
    let start = app.to_screen(Pos2::new(250.0, 160.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start + Vec2::new(80.0, 60.0))]);
    assert_ne!(local_bounds(&app.scene.elements[0]), before, "the title strip drags it");
    // Delete and Ctrl+Z would pull the scene from under the drag
    frame(&mut app, &ctx, vec![key_press(Key::Delete)]);
    assert_eq!(app.scene.elements.len(), 1, "Delete waits for the release");
    frame(&mut app, &ctx, vec![key_press(Key::Escape)]);
    assert_eq!(local_bounds(&app.scene.elements[0]), before, "Esc puts it back");
    assert!(app.undo.is_empty(), "and leaves no undo step");
    // the rest of that press moves nothing
    let end = start + Vec2::new(120.0, 90.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(local_bounds(&app.scene.elements[0]), before);
}

#[test]
fn a_press_that_trembles_is_still_a_click() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    window(&mut app, Rect::from_min_size(Pos2::new(100.0, 150.0), Vec2::new(300.0, 200.0)), shell);
    let before = local_bounds(&app.scene.elements[0]);
    let start = app.to_screen(Pos2::new(250.0, 160.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start + Vec2::new(3.0, 2.0))]);
    frame(&mut app, &ctx, vec![button(start + Vec2::new(3.0, 2.0), false)]);
    assert_eq!(local_bounds(&app.scene.elements[0]), before);
    assert!(app.undo.is_empty(), "a click is no undo step");
    assert_eq!(app.selected, vec![0]);
}

#[test]
fn a_note_drags_by_its_top_strip_and_its_body_types() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 150.0), Vec2::splat(200.0)), "abc".into());
    app.scene.elements.extend([note, label]);
    let before = local_bounds(&app.scene.elements[0]);
    let body = app.to_screen(Pos2::new(160.0, 260.0));
    drag(&mut app, &ctx, body, body + Vec2::new(100.0, 0.0));
    assert_eq!(local_bounds(&app.scene.elements[0]), before, "the body does not drag");
    assert_eq!(app.editing, Some(1), "the press on the body put its text in focus");
    app.finish_text();
    let strip = app.to_screen(Pos2::new(160.0, 156.0));
    drag(&mut app, &ctx, strip, strip + Vec2::new(100.0, 40.0));
    assert_eq!(local_bounds(&app.scene.elements[0]), before.translate(Vec2::new(100.0, 40.0)), "the strip drags");
}

#[test]
fn a_drag_whose_release_never_came_ends_on_the_next_frame() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.panning = true;
    frame(&mut app, &ctx, vec![]);
    assert!(!app.in_gesture());
}

#[test]
fn keeping_selection_follows_the_ids() {
    let mut app = DrawingApp::default();
    for x in [0.0, 100.0, 200.0] {
        window(&mut app, Rect::from_min_size(Pos2::new(x, 0.0), Vec2::splat(50.0)), |_| {});
    }
    let last = app.scene.elements[2].id.clone();
    app.selected = vec![0, 2];
    app.keeping_selection(|app| {
        app.scene.elements.remove(0);
    });
    assert_eq!(app.selected, vec![1], "the removed one leaves, the other follows its element");
    assert_eq!(app.scene.elements[1].id, last);
}
