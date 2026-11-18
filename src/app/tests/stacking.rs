//! The cards (windows and notes) stack in the drawing's order, over the drawings, and a press brings one to the front,
//! its live parts and a portal's page following it. Asked: "olha o bug aqui quando eu tenho um terminal aberto, pode ver
//! também no repo ali como funciona a hierarquia de elemento no canvas e corrigir por favor" (a note selected over a
//! browser under its bar and page; a browser over a terminal with its bar and page over the terminal's text but its
//! header under it).

use super::*;

/// A portal; over its right half a terminal, then a note over both; then a box drawn over the note.
fn stacked(app: &mut DrawingApp) -> [String; 4] {
    let mut portal = app.new_element(ElementKind::Rectangle { x: 60.0, y: 100.0, width: 400.0, height: 300.0 });
    portal.portal = Some(PortalSpec { name: "Docs".into(), url: "about:blank".into(), ..Default::default() });
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 300.0, y: 150.0, width: 300.0, height: 200.0 });
    terminal.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(200.0, 330.0), Vec2::splat(160.0)), "oi".into());
    let drawing = app.new_element(ElementKind::Rectangle { x: 220.0, y: 400.0, width: 60.0, height: 60.0 });
    let ids = [portal.id.clone(), terminal.id.clone(), note.id.clone(), drawing.id.clone()];
    app.scene.elements = vec![portal, terminal, note, label, drawing];
    ids
}

fn index_of(app: &DrawingApp, id: &str) -> usize {
    app.scene.elements.iter().position(|element| element.id == id).unwrap()
}

#[test]
fn a_card_in_front_of_a_portal_is_cut_out_of_its_page_and_over_the_drawings() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [portal, terminal, note, _] = stacked(&mut app);
    frame(&mut app, &ctx, vec![]);
    let frame_of = |id: &str| app.terminal_frame(&app.scene.elements[index_of(&app, id)]).0;
    // the terminal and the note are in front of the portal: its page leaves both uncovered
    let fronts = app.cards_in_front(index_of(&app, &portal));
    assert_eq!(fronts, vec![frame_of(&terminal), frame_of(&note)]);
    let page = app.portal_page_rect(&app.scene.elements[0]).expect("on the canvas");
    let holes = DrawingApp::page_holes(&ctx, page, &[], &fronts);
    let cut = |front: Rect| holes.contains(&front.intersect(page));
    // on macOS a hole hides the whole page, which has no picture to stand in for it there: it stays whole, over them
    assert_eq!([cut(frame_of(&terminal)), cut(frame_of(&note))], [!cfg!(target_os = "macos"); 2], "{holes:?}");
    // nothing in front of the note
    assert!(app.cards_in_front(index_of(&app, &note)).is_empty());
    // a click finds what is drawn on top: the note over the portal, and over the box drawn after it
    assert_eq!(app.hit(Pos2::new(250.0, 380.0)), Some(index_of(&app, &note)), "over the portal");
    assert_eq!(app.hit(Pos2::new(240.0, 420.0)), Some(index_of(&app, &note)), "over the box drawn later");
    assert_eq!(app.hit(Pos2::new(330.0, 200.0)), Some(index_of(&app, &terminal)), "the terminal over the portal");
}

#[test]
fn a_press_brings_a_covered_card_to_the_front_with_its_text() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [portal, terminal, note, drawing] = stacked(&mut app);
    frame(&mut app, &ctx, vec![]);
    // the portal's header, clear of the others: the portal goes in front of the terminal and the note
    let header = app.to_screen(Pos2::new(100.0, 110.0));
    click(&mut app, &ctx, header);
    let order: Vec<&str> = app.scene.elements.iter().map(|element| element.id.as_str()).collect();
    assert_eq!(order.last(), Some(&portal.as_str()));
    assert!(app.cards_in_front(index_of(&app, &portal)).is_empty(), "its page no longer has holes for them");
    assert_eq!(app.selected, vec![index_of(&app, &portal)], "and it stays selected where it went");
    assert!(app.dirty);
    // now the note, covered by the portal: it comes back over it, its text along
    let below = app.to_screen(Pos2::new(300.0, 470.0));
    click(&mut app, &ctx, below);
    let note_index = index_of(&app, &note);
    assert_eq!(app.with_card_text(&[note_index]), vec![note_index, note_index + 1]);
    assert_eq!(note_index + 2, app.scene.elements.len());
    // the terminal and the box drawn over the note keep their place among the rest
    assert!(index_of(&app, &terminal) < index_of(&app, &portal) && index_of(&app, &drawing) < index_of(&app, &portal));
    // a card nobody covers stays where it is
    app.dirty = false;
    let before: Vec<String> = app.scene.elements.iter().map(|element| element.id.clone()).collect();
    assert_eq!(app.raise_card(note_index), note_index);
    assert!(!app.dirty && app.scene.elements.iter().map(|element| element.id.clone()).collect::<Vec<_>>() == before);
}

#[test]
fn a_portals_bar_and_a_focused_terminals_grid_stack_with_their_cards() {
    let mut app = DrawingApp::default();
    let [portal, terminal, ..] = stacked(&mut app);
    let middle = |id: egui::Id| egui::LayerId::new(egui::Order::Middle, id);
    // the bar is the area browser/chrome.rs shows
    assert_eq!(app.live_layers(&app.scene.elements[0]), vec![middle(egui::Id::unique(("portal_chrome", portal.as_str())))]);
    // a terminal has a live grid only while it has the keyboard (terminal/overlay.rs)
    assert!(app.live_layers(&app.scene.elements[1]).is_empty());
    app.terminal_focus = Some(terminal);
    assert_eq!(app.live_layers(&app.scene.elements[1]), vec![middle(egui::Id::unique("terminal_overlay"))]);
}

/// Asked: "cliquei em editar a nota e o texto saiu em cima do terminal". However its typing begins (here Enter on the
/// selected note, with no press on it), the note typed in comes in front of the terminal over it: its card and text are
/// drawn where the editor is, over the terminal.
#[test]
fn a_note_typed_in_comes_in_front_of_the_card_covering_it() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(100.0, 200.0), Vec2::splat(200.0)), "texto".into());
    let (note_id, label_id) = (note.id.clone(), label.id.clone());
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 50.0, y: 120.0, width: 400.0, height: 150.0 });
    terminal.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
    app.scene.elements = vec![note, label, terminal];
    app.selected = vec![0];
    frame(&mut app, &ctx, vec![key_press(Key::Enter)]);
    let (note_index, label_index) = (index_of(&app, &note_id), index_of(&app, &label_id));
    assert_eq!((note_index, label_index), (1, 2), "the note and its text after the terminal");
    assert_eq!(app.editing, Some(label_index), "still typing its text");
    assert_eq!(app.selected, vec![note_index]);
    assert_eq!(app.hit(Pos2::new(200.0, 230.0)), Some(note_index), "where the terminal covered it, the note is on top");
    // typing on, nothing moves again
    app.dirty = false;
    frame(&mut app, &ctx, vec![]);
    assert!(!app.dirty && app.editing == Some(label_index));
}

/// A frame as the app runs it, the terminal's grid included after the editor (update.rs): the terminal with the keyboard
/// asks for it again every frame.
fn frame_with_terminal(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), time: Some(time), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |ui| {
        app.screen_keys(ctx);
        app.handle_shortcuts(ctx);
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let (response, _) = ui.allocate_painter(ui.available_size(), Sense::CLICK | Sense::DRAG);
            app.handle_canvas(&response, response.rect);
        });
        app.text_editor(ctx);
        app.terminal_overlay(ctx);
    });
    output.textures_delta.clear();
}

/// A note in front of the terminal with the keyboard, inside its frame: a click on the note takes the keys from the
/// terminal, and what is typed in the note stays in the note.
#[test]
fn a_click_on_a_note_in_front_of_the_terminal_with_the_keyboard_takes_the_keys() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    frame_with_terminal(&mut app, &ctx, 0.0, vec![]);
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 50.0, y: 100.0, width: 500.0, height: 400.0 });
    terminal.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
    let terminal_id = terminal.id.clone();
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(200.0, 200.0), Vec2::splat(200.0)), String::new());
    let label_id = label.id.clone();
    app.scene.elements = vec![terminal, note, label];
    app.terminal_focus = Some(terminal_id);
    frame_with_terminal(&mut app, &ctx, 0.1, vec![]);
    // a click on the note's text, inside the terminal's frame
    let at = app.to_screen(Pos2::new(300.0, 320.0));
    for (time, pressed) in [(1.0, true), (1.05, false)] {
        frame_with_terminal(&mut app, &ctx, time, vec![egui::Event::PointerMoved(at), button(at, pressed)]);
    }
    assert_eq!(app.terminal_focus, None);
    assert_eq!(app.selected, vec![1]);
    for (time, events) in [(1.3, vec![]), (1.31, vec![egui::Event::Text("oi".into())]), (1.32, vec![])] {
        frame_with_terminal(&mut app, &ctx, time, events);
    }
    let label = app.scene.elements.iter().find(|element| element.id == label_id).unwrap();
    assert!(matches!(&label.kind, ElementKind::Text { value, .. } if value == "oi"), "{:?}", label.kind);
    assert_eq!(app.terminal_focus, None);
}

/// A card brought to the front from its live parts (a tree's rows, a portal's bar, a terminal's grid) while an arrow's
/// points are edited or a line is drawn point by point: they stay on that arrow and that line.
#[test]
fn a_card_brought_to_the_front_keeps_the_arrow_and_line_worked_on() {
    let mut app = DrawingApp::default();
    let mut portal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    portal.portal = Some(PortalSpec { name: "Docs".into(), url: "about:blank".into(), ..Default::default() });
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 200.0, y: 100.0, width: 400.0, height: 300.0 });
    terminal.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
    let arrow = app.new_element(ElementKind::Line { points: vec![[0.0, 500.0], [300.0, 500.0]], arrow: true });
    let line = app.new_element(ElementKind::Line { points: vec![[0.0, 600.0], [300.0, 600.0]], arrow: false });
    let (arrow_id, line_id) = (arrow.id.clone(), line.id.clone());
    app.scene.elements = vec![portal, terminal, arrow, line];
    (app.line_editor, app.multi) = (Some(2), Some(3));
    assert_eq!(app.raise_card(0), 3, "the portal over the terminal");
    assert_eq!(app.line_editor.map(|index| app.scene.elements[index].id.clone()), Some(arrow_id));
    assert_eq!(app.multi.map(|index| app.scene.elements[index].id.clone()), Some(line_id));
}
