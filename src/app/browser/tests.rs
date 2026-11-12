use super::*;

#[test]
fn portal_tabs_keep_the_matching_live_pages() {
    let urls = |items: &[&str]| items.iter().map(|url| (*url).to_owned()).collect::<Vec<_>>();
    assert_eq!(match_portal_tabs(&urls(&["a", "b", "c"]), &urls(&["a", "c"])), vec![Some(0), Some(2)]);
    assert_eq!(match_portal_tabs(&urls(&["a", "b"]), &urls(&["a", "changed"])), vec![Some(0), Some(1)]);
    assert_eq!(match_portal_tabs(&urls(&["a", "b"]), &urls(&["a", "b", "c"])), vec![Some(0), Some(1), None]);
    assert_eq!(match_portal_tabs(&urls(&["a", "b", "c"]), &urls(&["b", "c"])), vec![Some(1), Some(2)]);
    assert_eq!(match_portal_tabs(&urls(&["same", "same", "tail"]), &urls(&["same", "tail"])), vec![Some(0), Some(2)]);
}

/// A click on the address turns it into a field with the keyboard, its text where the bar
/// showed it: same left edge, same vertical center.
#[test]
fn the_address_field_keeps_the_text_in_place_and_takes_the_keys() {
    let ctx = Context::default();
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    let mut portal = app.new_element(ElementKind::Rectangle { x: 40.0, y: 40.0, width: 600.0, height: 400.0 });
    portal.portal = Some(PortalSpec { name: "Docs".into(), url: "https://exemplo.com/docs".into(), ..Default::default() });
    let id = portal.id.clone();
    app.scene.elements.push(portal);
    // the url's text on screen, after a frame of the bar
    let run = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |_| app.portal_chrome(&ctx));
        out.textures_delta.clear();
        out.shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text().contains("exemplo.com") => Some(t.galley.rect.translate(t.pos.to_vec2())),
            _ => None,
        })
    };
    // a new area sizes itself unseen on its first frame
    run(&mut app, vec![]);
    let shown = run(&mut app, vec![]).expect("the bar shows the address");
    let at = shown.left_center() + Vec2::new(20.0, 0.0);
    run(
        &mut app,
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
        ],
    );
    run(&mut app, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
    assert!(app.portal_url_edit.as_ref().is_some_and(|(edited, ..)| *edited == id), "the click opens the field");
    let mut editing = None;
    for _ in 0..3 {
        editing = run(&mut app, vec![]);
    }
    let editing = editing.expect("the field shows the address");
    assert!(ctx.memory(|m| m.focused().is_some()), "the field has the keyboard");
    assert!((editing.center().y - shown.center().y).abs() < 0.5, "{editing:?} vs {shown:?}");
    assert!((editing.left() - shown.left()).abs() < 0.5, "{editing:?} vs {shown:?}");
}

#[test]
fn display_url_drops_the_scheme() {
    assert_eq!(display_url("https://docs.rs/egui"), "docs.rs/egui");
    assert_eq!(display_url("http://localhost:3000"), "localhost:3000");
    assert_eq!(display_url("about:blank"), "about:blank");
}
