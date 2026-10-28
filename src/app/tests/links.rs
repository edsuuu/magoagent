//! Connections: "Conectar" mode, the badge's list, a rope's popover and the palette's node searches.

use super::*;
use crate::app::ui::context::{Entry, Target};
use crate::app::ui::links::{LinkAction, LinksOf};
use crate::app::ui::palette::{Row, Run};

/// A terminal box at world `x`, named `name`.
fn box_at(app: &mut DrawingApp, name: &str, x: f32) -> String {
    let mut e = app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 200.0, height: 120.0 });
    e.terminal = Some(TerminalSpec { name: name.into(), ..Default::default() });
    let id = e.id.clone();
    app.scene.elements.push(e);
    id
}

#[test]
fn connect_from_a_focused_terminal_hands_the_next_click_to_the_canvas() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0));
    app.terminal_focus = Some(a.clone());
    app.selected = vec![0];
    // the bar's Conectar: the source gives up the keyboard, and the canvas gets the next press
    app.start_connect(a.clone());
    assert_eq!(app.terminal_focus, None);
    let on_b = app.to_screen(Pos2::new(500.0, 60.0));
    click(&mut app, &ctx, on_b);
    assert_eq!(app.connecting, None);
    assert_eq!(app.links()[&a], vec![b], "the node clicked is joined");
    assert_eq!(app.terminal_focus, None, "the press on it is the canvas's, not a focus");
}

#[test]
fn a_locked_connection_stays_through_disconnect() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0));
    app.connect(&a, &b);
    app.scene.elements.iter_mut().find(|e| e.rope).unwrap().locked = true;
    app.unlink(&a, &b);
    assert!(app.status.contains("bloqueada"), "says why: {}", app.status);
    app.unlink_all(std::slice::from_ref(&a));
    assert_eq!(app.links()[&a], vec![b]);
}

#[test]
fn the_badge_list_cuts_one_connection_or_all_in_one_undo_step() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b, c) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0), box_at(&mut app, "C", 800.0));
    app.connect(&a, &b);
    app.connect(&a, &c);
    // its × on b
    app.unlink(&a, &b);
    assert_eq!(app.links()[&a], vec![c.clone()]);
    app.undo();
    assert_eq!(app.links()[&a].len(), 2);
    // Remover tudo on a
    app.unlink_all(std::slice::from_ref(&a));
    assert!(app.links().is_empty());
    app.undo();
    assert_eq!(app.links()[&a].len(), 2);
    // the list itself draws: it opens below the badge and keeps its rect for the next press
    app.toggle_node_links(&a, Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(15.0, 24.0)));
    let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), ..Default::default() }, |_| {
        app.links_popover_ui(&ctx)
    });
    output.textures_delta.clear();
    assert!(app.links_popover.as_ref().is_some_and(|p| p.rect.is_positive()), "the popover drew");
}

#[test]
fn a_click_on_a_rope_opens_its_popover_and_delete_takes_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0));
    app.connect(&a, &b);
    // the middle of the rope's curve, where it sags at rest
    let rope = app.scene.elements.iter().find(|e| e.rope).unwrap();
    let path = crate::geometry::line_path(rope);
    let mid = path[path.len() / 2];
    let on_rope = app.to_screen(Pos2::new(mid[0], mid[1]));
    click(&mut app, &ctx, on_rope);
    assert!(matches!(app.links_popover.as_ref().map(|p| &p.of), Some(LinksOf::Ropes(ids)) if ids.len() == 1));
    assert_eq!(app.selected.len(), 1, "the click selected the rope");
    assert_eq!(app.hint(), None, "no hint about resizing: a rope isn't resized");
    frame(&mut app, &ctx, vec![key_press(Key::Delete)]);
    assert!(app.links().is_empty(), "Delete takes the selected rope");
}

#[test]
fn dragging_a_window_with_its_rope_selected_keeps_the_rope() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0));
    app.connect(&a, &b);
    let at = |app: &DrawingApp, id: &str| app.scene.elements.iter().position(|e| e.id == id).unwrap();
    let rope = app.scene.elements.iter().position(|e| e.rope).unwrap();
    app.selected = vec![at(&app, &a), rope];
    let b_before = app.scene.elements[at(&app, &b)].clone();
    // by A's title strip, with the ropes swinging between the frames as in the app
    let start = app.to_screen(Pos2::new(100.0, 10.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
    for step in 1..=3 {
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start + Vec2::new(0.0, 50.0 * step as f32))]);
        app.swing_ropes(0.016);
    }
    frame(&mut app, &ctx, vec![button(start + Vec2::new(0.0, 150.0), false)]);
    app.swing_ropes(0.016);
    assert_eq!(app.links()[&a], vec![b.clone()], "the rope still ties A to B");
    assert_eq!(app.scene.elements[at(&app, &b)], b_before, "B stays where it was");
    assert!(local_bounds(&app.scene.elements[at(&app, &a)]).min.y > 100.0, "A moved");
}

#[test]
fn a_rope_is_not_edited_like_an_arrow() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0));
    app.connect(&a, &b);
    let rope = app.scene.elements.iter().position(|e| e.rope).unwrap();
    app.selected = vec![rope];
    assert_eq!(app.single_line(), None, "its points follow the windows");
    assert!(app.point_handles().is_empty() && app.midpoint_handles().is_empty());
    assert!(!app.shows_transform_handles(), "no box to scale or turn it");
    assert!(!app.shows_properties(), "nor the drawing's style panel");
    assert!(!takes_label(&app.scene.elements[rope]));
    // Ctrl/Cmd + double-click on it: no line editor, no label
    let path = crate::geometry::line_path(&app.scene.elements[rope]);
    let mid = path[path.len() / 2];
    let at = app.to_screen(Pos2::new(mid[0], mid[1]));
    let cmd = Modifiers { command: true, ctrl: !cfg!(target_os = "macos"), mac_cmd: cfg!(target_os = "macos"), ..Default::default() };
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: cmd };
    let count = app.scene.elements.len();
    // the held keys come as their own event, as from a real window: a click's modifiers are its own only
    for (t, pressed) in [(1.0, true), (1.05, false), (1.15, true), (1.2, false)] {
        frame_at(&mut app, &ctx, Some(t), vec![egui::Event::ModifiersChanged(cmd), egui::Event::PointerMoved(at), press(pressed)]);
    }
    assert_eq!(app.line_editor, None);
    assert_eq!(app.scene.elements.len(), count, "no label made for it");
}

#[test]
fn a_click_finds_the_rope_where_it_is_drawn() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0));
    app.connect(&a, &b);
    app.swing_ropes(0.016);
    let rope = app.scene.elements.iter().position(|e| e.rope).unwrap();
    let path = crate::geometry::line_path(&app.scene.elements[rope]);
    let mid = Pos2::new(path[path.len() / 2][0], path[path.len() / 2][1]);
    // mid-swing: drawn 80 below its resting line
    let id = app.scene.elements[rope].id.clone();
    for p in &mut app.ropes.get_mut(&id).unwrap().points {
        p.y += 80.0;
    }
    assert_eq!(app.hit(mid), None, "nothing where the rope is not drawn");
    assert_eq!(app.hit(mid + Vec2::new(0.0, 80.0)), Some(rope), "the rope where it is drawn");
}

#[test]
fn a_click_on_crossed_ropes_lists_the_nearest_and_those_within_six_px_more() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b, c) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0), box_at(&mut app, "C", 800.0));
    app.connect(&a, &b);
    app.connect(&a, &c);
    app.swing_ropes(0.016);
    // drawn straight, 9.5 px apart at zoom 1
    let ropes: Vec<String> = app.scene.elements.iter().filter(|e| e.rope).map(|e| e.id.clone()).collect();
    for (rope, y) in ropes.iter().zip([300.0, 309.5]) {
        let points = &mut app.ropes.get_mut(rope).unwrap().points;
        let last = (points.len() - 1) as f32;
        for (i, p) in points.iter_mut().enumerate() {
            *p = Pos2::new(i as f32 * 600.0 / last, y);
        }
    }
    let listed = |app: &mut DrawingApp, y: f32| {
        app.open_rope_popover(Pos2::ZERO, Pos2::new(300.0, y));
        match app.links_popover.take().map(|p| p.of) {
            Some(LinksOf::Ropes(ids)) => ids.len(),
            _ => 0,
        }
    };
    assert_eq!(listed(&mut app, 300.0), 1, "on one: the other is 9.5 px off, past 3 + 6");
    assert_eq!(listed(&mut app, 296.5), 2, "3.5 px from one: the other at 13 px is within 8 + 6");
    assert_eq!(listed(&mut app, 280.0), 0, "past 14 px from both: nothing");
}

#[test]
fn a_note_put_on_a_resting_rope_wakes_it_and_it_moves_aside() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 600.0));
    app.connect(&a, &b);
    for _ in 0..400 {
        if !app.swing_ropes(0.05) {
            break;
        }
    }
    assert!(!app.swing_ropes(0.05), "at rest");
    let rope = app.scene.elements.iter().find(|e| e.rope).unwrap().id.clone();
    let before = app.ropes[&rope].points[10];
    // its top just above the rope's lowest point: the way out is up
    let [note, label] = app.new_note(Rect::from_min_size(before - Vec2::new(30.0, 10.0), Vec2::new(60.0, 40.0)), String::new());
    app.scene.elements.extend([note, label]);
    assert!(app.swing_ropes(0.05), "a new obstacle wakes it");
    for _ in 0..400 {
        if !app.swing_ropes(0.05) {
            break;
        }
    }
    assert!(app.ropes[&rope].points[10].y < before.y - 5.0, "and it moves aside");
}

#[test]
fn cutting_a_rope_keeps_the_rest_of_the_selection() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0));
    app.connect(&a, &b);
    let c = box_at(&mut app, "C", 800.0);
    app.selected = vec![app.scene.elements.iter().position(|e| e.id == c).unwrap()];
    app.unlink(&a, &b);
    assert_eq!(app.selected.len(), 1);
    assert_eq!(app.scene.elements[app.selected[0]].id, c, "C is still the one selected");
}

#[test]
fn the_palette_searches_the_nodes_to_join_or_cut_and_the_menu_cuts_one() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, b, _c) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0), box_at(&mut app, "C", 800.0));
    app.connect(&a, &b);
    app.selected = vec![0];
    let labels = |rows: Vec<Row>| rows.into_iter().map(|r| r.label).collect::<Vec<_>>();
    app.palette_links = Some(LinkAction::Connect);
    assert_eq!(labels(app.palette_rows()), ["C"], "not itself, not one already joined");
    app.palette_links = Some(LinkAction::Disconnect);
    assert_eq!(labels(app.palette_rows()), ["B"]);
    app.pick_link_node(&b);
    assert!(app.links().is_empty());
    assert_eq!(app.palette_links, None);
    // the node's context menu lists the nodes it is joined to, and its item cuts one
    app.connect(&a, &b);
    let entries = app.context_entries(&Target::Element(a.clone()));
    assert!(entries.iter().any(|e| matches!(e, Entry::Sub { label, .. } if label == "Desconectar")));
    app.context_action(&ctx, &Target::Element(a.clone()), &format!("disconnect:{b}"));
    assert!(app.links().is_empty());
    // "Remover todas as conexões" works on the selection
    app.connect(&a, &b);
    app.run_command(crate::app::ui::palette::Command::RemoveConnections);
    assert!(app.links().is_empty());
    assert!(matches!(app.palette_rows().first().map(|r| &r.run), Some(Run::Command(_))));
}

#[test]
fn connect_skips_arrows_and_says_why_a_locked_node_wont_join() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let (a, _b) = (box_at(&mut app, "A", 0.0), box_at(&mut app, "B", 400.0));
    app.scene.elements.push(app.new_element(ElementKind::Line { points: vec![[100.0, 400.0], [500.0, 400.0]], arrow: true }));
    // an arrow is no node: a click on it just ends the mode
    app.connecting = Some(a.clone());
    let on_arrow = app.to_screen(Pos2::new(300.0, 400.0));
    click(&mut app, &ctx, on_arrow);
    assert!(app.connecting.is_none());
    // a locked node says why it won't join, and the mode waits for another click
    app.scene.elements[1].locked = true;
    app.connecting = Some(a.clone());
    let on_b = app.to_screen(Pos2::new(500.0, 100.0));
    click(&mut app, &ctx, on_b);
    assert_eq!(app.connecting.as_ref(), Some(&a));
    assert!(app.status.contains("bloqueado"), "{}", app.status);
    assert!(app.links().get(&a).is_none_or(|l| l.is_empty()));
    // a locked frame behind is no node: a click inside it ends the mode, as on the empty canvas
    let mut back = app.new_element(ElementKind::Rectangle { x: -200.0, y: 450.0, width: 1000.0, height: 300.0 });
    back.frame = Some("Fundo".into());
    back.locked = true;
    app.scene.elements.insert(0, back);
    app.connecting = Some(a.clone());
    let inside = app.to_screen(Pos2::new(300.0, 520.0));
    click(&mut app, &ctx, inside);
    assert!(app.connecting.is_none());
}
