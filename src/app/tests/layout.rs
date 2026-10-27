//! Laying things out: Alinhar/Distribuir, snapping while dragging, how small a window gets.

use super::*;
use crate::app::editing::align_deltas;

fn rect(app: &DrawingApp, x: f32, y: f32, width: f32, height: f32) -> Element {
    app.new_element(ElementKind::Rectangle { x, y, width, height })
}

fn tied_to(id: &str) -> Option<Binding> {
    Some(Binding { element_id: id.into(), fixed: [0.5, 0.5], inside: false })
}

#[test]
fn align_deltas_line_up_edges_and_spread_evenly() {
    let r = |x: f32, w: f32| Rect::from_min_size(Pos2::new(x, 0.0), Vec2::new(w, 10.0));
    let boxes = [r(0.0, 10.0), r(50.0, 10.0), r(12.0, 10.0)];
    assert_eq!(align_deltas(&boxes, 2).unwrap(), [Vec2::new(50.0, 0.0), Vec2::ZERO, Vec2::new(38.0, 0.0)]);
    // spread across: 0..60 holds 30 of boxes, so two gaps of 15, in the order they lie
    assert_eq!(align_deltas(&boxes, 6).unwrap(), [Vec2::ZERO, Vec2::ZERO, Vec2::new(13.0, 0.0)]);
    assert_eq!(align_deltas(&boxes[..1], 0), None);
}

#[test]
fn aligning_moves_a_group_whole_and_skips_ropes_and_locks() {
    let mut app = DrawingApp::default();
    let mut a = rect(&app, 0.0, 0.0, 100.0, 50.0);
    let mut b = rect(&app, 150.0, 100.0, 50.0, 50.0);
    a.group_id = Some("g".into());
    b.group_id = Some("g".into());
    let c = rect(&app, 400.0, 300.0, 80.0, 40.0);
    let mut locked = rect(&app, 600.0, 500.0, 60.0, 60.0);
    locked.locked = true;
    // a rope reaching far right: counted, it would set the right edge
    let mut rope = app.new_element(ElementKind::Line { points: vec![[0.0, 0.0], [900.0, 300.0]], arrow: false });
    rope.rope = true;
    (rope.ends.start, rope.ends.end) = (tied_to(&a.id), tied_to(&c.id));
    app.scene.elements = vec![a, b, c, locked, rope];
    app.selected = (0..5).collect();
    app.align_selection(2);
    let x = |app: &DrawingApp, i: usize| element_bounds(&app.scene.elements[i]).min.x;
    // the right edge is c's (480): the group's box (0..200) goes 280 right, in one piece
    assert_eq!((x(&app, 0), x(&app, 1), x(&app, 2)), (280.0, 430.0, 400.0));
    assert_eq!(x(&app, 3), 600.0, "the locked box stays");
}

#[test]
fn snapping_skips_what_moves_along() {
    let mut app = DrawingApp::default();
    app.prefs.snap_objects = true;
    let mut frame_box = rect(&app, 0.0, 0.0, 400.0, 300.0);
    frame_box.frame = Some("Frame".into());
    // inside the frame, 5 from its left edge; and an arrow tied to it 3 from that edge
    let inside = rect(&app, 5.0, 100.0, 50.0, 50.0);
    let mut arrow = app.new_element(ElementKind::Line { points: vec![[3.0, 400.0], [3.0, 600.0]], arrow: true });
    arrow.ends.start = tied_to(&frame_box.id);
    app.scene.elements = vec![frame_box, inside, arrow];
    app.selected = vec![0];
    app.move_origin = Some((vec![0, 1], app.scene.elements[..2].to_vec(), Rect::NOTHING));
    let moved = Rect::from_min_size(Pos2::new(1.0, 0.0), Vec2::new(400.0, 300.0));
    assert_eq!(app.snap_box(moved).0, Vec2::ZERO);
    // nor the frame's label, centered on it again each frame
    let mut label = app.new_element(ElementKind::Text { x: 4.0, y: 700.0, value: "rótulo".into() });
    label.id = format!("{}-label", app.scene.elements[0].id);
    app.scene.elements.push(label);
    assert_eq!(app.snap_box(moved).0, Vec2::ZERO);
    // a box that stays is still a target
    app.scene.elements.push(rect(&app, 6.0, 900.0, 20.0, 20.0));
    assert_eq!(app.snap_box(moved).0, Vec2::new(5.0, 0.0));
}

#[test]
fn a_window_stops_at_its_smallest_and_never_turns_inside_out() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut terminal = rect(&app, 60.0, 150.0, 300.0, 200.0);
    terminal.terminal = Some(TerminalSpec { name: "t".into(), ..Default::default() });
    let mut ide = rect(&app, 420.0, 150.0, 320.0, 380.0);
    ide.ide = Some(IdeSpec { name: "proj".into(), root: "/nonexistent-magoagent".into(), ..Default::default() });
    app.scene.elements = vec![terminal, ide];
    frame(&mut app, &ctx, vec![]);
    // the corner dragged past the opposite one: the terminal keeps its corner and its minimum
    let corner = app.to_screen(Pos2::new(360.0, 350.0));
    let past = app.to_screen(Pos2::new(10.0, 20.0));
    drag(&mut app, &ctx, corner, past);
    assert_eq!(local_bounds(&app.scene.elements[0]), Rect::from_min_size(Pos2::new(60.0, 150.0), TERMINAL_MIN));
    // the IDE's right edge pulled far left, and above its top: only the width moves, down to its floor
    let edge = app.to_screen(Pos2::new(740.0, 340.0));
    let far = app.to_screen(Pos2::new(380.0, 60.0));
    drag(&mut app, &ctx, edge, far);
    assert_eq!(local_bounds(&app.scene.elements[1]), Rect::from_min_size(Pos2::new(420.0, 150.0), Vec2::new(IDE_MIN.x, 380.0)));
    // one already under its floor (an older build, an AI): an edge leaves the other axis alone
    let mut short = rect(&app, 60.0, 150.0, 300.0, 90.0);
    short.terminal = Some(TerminalSpec { name: "t".into(), ..Default::default() });
    app.scene.elements[0] = short;
    let (edge, right) = (app.to_screen(Pos2::new(360.0, 195.0)), app.to_screen(Pos2::new(500.0, 195.0)));
    drag(&mut app, &ctx, edge, right);
    assert_eq!(local_bounds(&app.scene.elements[0]), Rect::from_min_size(Pos2::new(60.0, 150.0), Vec2::new(440.0, 90.0)));
}

#[test]
fn a_note_never_turns() {
    let mut app = DrawingApp::default();
    let [mut note, label] = app.new_note(Rect::from_min_size(Pos2::ZERO, Vec2::splat(160.0)), "oi".into());
    assert!(!note.rotatable());
    // turned with a group, it goes round the pivot upright
    let pivot = Pos2::new(500.0, 500.0);
    note.rotate_around(pivot, 0.5);
    assert_eq!(note.angle, 0.0);
    let center = pivot + rot(Pos2::new(80.0, 80.0) - pivot, 0.5);
    assert!((note.center() - center).length() < 0.01, "{:?}", note.center());
    // no handle for it, with its text or alone
    app.scene.elements = vec![note.clone(), label];
    app.selected = vec![0, 1];
    assert_eq!(app.rotation_handle(), None);
    // one an older build turned can be set upright again
    note.angle = 0.3;
    assert!(note.rotatable());
}
