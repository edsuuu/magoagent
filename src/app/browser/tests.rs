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

/// Linux: a page's box goes to WebKitGTK in whole pixels inside the card's box, at the screen's scale, never empty.
#[cfg(target_os = "linux")]
#[test]
fn a_linux_page_takes_whole_pixels_inside_its_card() {
    let page = Rect::from_min_max(Pos2::new(10.4, 20.0), Pos2::new(110.6, 70.9));

    assert_eq!(webkit::page_pixels(page, 1.0), [11, 20, 99, 50]);
    assert_eq!(webkit::page_pixels(page, 1.5), [17, 30, 149, 75]);
    assert_eq!(webkit::page_pixels(Rect::from_min_max(Pos2::new(5.0, 5.0), Pos2::new(5.5, 5.2)), 1.0), [5, 5, 1, 1]);
    // the canvas's zoom and the screen's scale, less what GTK already scales
    assert_eq!(webkit::page_zoom(0.5, 2.0, 2), 0.5);
    assert_eq!(webkit::page_zoom(1.0, 1.25, 1), 1.25);
    assert_eq!(webkit::page_zoom(1.0, 2.0, 0), 2.0);
}

/// Linux: the page's picture from Cairo loses its row padding, and a region of it is cut at the picture's scale.
#[cfg(target_os = "linux")]
#[test]
fn a_linux_page_picture_is_unpadded_and_cropped() {
    let padded = [1, 2, 3, 4, 0, 0, 0, 0, 5, 6, 7, 8];

    assert_eq!(webkit::rows(&padded, 1, 2, 8), Some(vec![1, 2, 3, 4, 5, 6, 7, 8]));
    assert_eq!(webkit::rows(&padded, 2, 2, 4), None);
    assert_eq!(webkit::rows(&padded, 0, 2, 8), None);
    // a 4x2 picture whose pixels are numbered
    let picture: Vec<u8> = (0..8u8).flat_map(|pixel| [pixel; 4]).collect();
    let numbers = |rgba: Vec<u8>| rgba.chunks(4).map(|pixel| pixel[0]).collect::<Vec<_>>();
    let (width, height, rgba) = webkit::crop(4, 2, &picture, [1.0, 0.0, 2.0, 2.0], 1.0).unwrap();

    assert_eq!((width, height, numbers(rgba)), (2, 2, vec![1, 2, 5, 6]));
    let (width, height, rgba) = webkit::crop(4, 2, &picture, [0.5, 0.5, 1.0, 0.5], 2.0).unwrap();

    assert_eq!((width, height, numbers(rgba)), (2, 1, vec![5, 6]));
    assert!(webkit::crop(4, 2, &picture, [10.0, 0.0, 2.0, 2.0], 1.0).is_none());
    assert!(webkit::crop(4, 2, &picture[..16], [0.0, 0.0, 2.0, 2.0], 1.0).is_none());
}

/// Moving, the page is cut away and its picture goes with the card; just stopped, it lays out unseen; then it shows over
/// its picture, which goes after.
#[test]
fn a_page_hides_while_its_card_moves_and_comes_back_unseen_first() {
    assert_eq!(motion(None), Motion::Still, "not on the canvas");
    assert_eq!(motion(Some(f64::MAX)), Motion::Still, "never moved");
    assert_eq!(motion(Some(0.0)), Motion::Moving, "the frame it moved");
    assert_eq!(motion(Some(0.1)), Motion::Moving, "a zoom still going");
    assert_eq!(motion(Some(0.2)), Motion::Unseen, "stopped: laying out at its new size, cut away");
    assert_eq!(motion(Some(0.4)), Motion::Back, "shown over its picture");
    assert_eq!(motion(Some(0.5)), Motion::Still, "settled: the picture goes");
    // a page seen the first time starts a settle ago (`sync_portals`): it lays out unseen before it shows
    assert_eq!(motion(Some(0.15)), Motion::Unseen);
}

/// The frame the card moves, the page's last picture is painted over its new page box already (the page is cut away from
/// that frame on, `sync_portals`), and where the card painted it is kept: the zoom buttons change the view after the
/// canvas painted, and the page waits a frame for its picture.
#[test]
fn a_moving_card_carries_its_pages_picture_from_the_first_frame() {
    let ctx = Context::default();
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    let mut portal = app.new_element(ElementKind::Rectangle { x: 40.0, y: 40.0, width: 600.0, height: 400.0 });

    portal.portal = Some(PortalSpec { name: "Docs".into(), url: "about:blank".into(), ..Default::default() });
    let id = portal.id.clone();

    app.scene.elements = vec![portal];
    let page = app.portal_page_rect(&app.scene.elements[0]).expect("on the canvas");
    let picture = ctx.load_texture("pagina", egui::ColorImage::filled([4, 4], Color32::RED), Default::default());

    app.portal_snaps.insert(id.clone(), (0.0, Some(picture.clone())));

    let painted = |app: &DrawingApp| {
        let output = ctx.run_ui(Default::default(), |ui| app.paint_portal(ui.painter(), &app.scene.elements[0]));

        output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == picture.id()))
    };

    // standing still where the page is: the page itself shows, not its picture
    app.portal_motion.insert(id.clone(), (page, 0.0));
    assert!(!painted(&app));
    assert_eq!(app.portal_painted.borrow().get(&id), Some(&page));
    // zoomed: the card's page box is elsewhere this frame, its picture there
    app.zoom = 1.5;
    assert!(painted(&app));
    let moved = app.portal_page_rect(&app.scene.elements[0]);

    assert_ne!(moved, Some(page));
    assert_eq!(app.portal_painted.borrow().get(&id), moved.as_ref());
}

/// A portal gone from the canvas leaves nothing kept for it: where its page box was painted, how it moved, its picture.
#[test]
fn a_portal_gone_leaves_nothing_behind() {
    let mut app = DrawingApp::default();
    let place = Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0));

    for id in ["fica", "saiu"] {
        app.portal_painted.get_mut().insert(id.into(), place);
        app.portal_motion.insert(id.into(), (place, 0.0));
        app.portal_snaps.insert(id.into(), (0.0, None));
    }

    app.forget_portals(&["fica"]);
    assert_eq!(app.portal_painted.borrow().keys().collect::<Vec<_>>(), ["fica"]);
    assert_eq!(app.portal_motion.keys().collect::<Vec<_>>(), ["fica"]);
    assert_eq!(app.portal_snaps.keys().collect::<Vec<_>>(), ["fica"]);
}
