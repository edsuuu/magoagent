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
    assert!(holes.contains(&frame_of(&terminal).intersect(page)) && holes.contains(&frame_of(&note).intersect(page)), "{holes:?}");
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
