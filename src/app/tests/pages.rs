use super::*;

#[test]
fn a_note_is_picked_up_by_its_header_and_typed_in_below_it() {
    let mut app = DrawingApp::default();
    let [note, text] = app.new_note(Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(200.0, 100.0)), "oi".into());
    app.scene.elements.extend([note, text]);
    let at = |y: f32| Pos2::new(100.0, y);
    // at rest or worked on alike: the header, the rest is text ("Clique para editar…")
    assert!(app.on_title(0, at(15.0)) && !app.on_title(0, at(17.0)));
    app.selected = vec![0];
    assert!(app.on_title(0, at(15.0)) && !app.on_title(0, at(17.0)));
    // zoomed out, at least 16 screen px (32 here), at most 40% of the note (40 here)
    app.zoom = 0.5;
    assert!(app.on_title(0, at(31.0)) && !app.on_title(0, at(33.0)));
    app.zoom = 0.25;
    assert!(app.on_title(0, at(39.0)) && !app.on_title(0, at(41.0)));
}

#[test]
fn a_double_click_on_a_note_types_into_that_note() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [a, a_text] = app.new_note(Rect::from_min_size(Pos2::new(640.0, 60.0), Vec2::splat(260.0)), "# Plano".into());
    // long markdown lines, many of them: the label, unwrapped, is wider and taller than the card
    // and centered on it, so it reaches over A
    let long = (0..25).map(|_| "palavra ".repeat(14)).collect::<Vec<_>>().join("\n");
    let [b, b_text] = app.new_note(Rect::from_min_size(Pos2::new(940.0, 60.0), Vec2::new(300.0, 380.0)), long);
    app.scene.elements.extend([a, a_text.clone(), b, b_text]);
    let at = app.to_screen(Pos2::new(770.0, 200.0));
    for (t, pressed) in [(1.0, true), (1.05, false), (1.15, true), (1.2, false)] {
        frame_at(&mut app, &ctx, Some(t), vec![egui::Event::PointerMoved(at), button(at, pressed)]);
    }
    let editing = app.editing.map(|i| app.scene.elements[i].id.clone());
    assert_eq!(editing, Some(a_text.id), "the note under the pointer, not the other one");
    assert_eq!(app.selected, vec![0], "the card stays the selection, its text is typed in place");
}

#[test]
fn clicks_on_a_note_select_its_card_never_its_text() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // inside the 800 pt test window: a short note A and a long, tall note B beside it
    let long = (0..25).map(|_| "palavra ".repeat(14)).collect::<Vec<_>>().join("\n");
    let [a, a_text] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "# Plano".into());
    let [b, b_text] = app.new_note(Rect::from_min_size(Pos2::new(380.0, 60.0), Vec2::new(300.0, 380.0)), long);
    app.scene.elements.extend([a, a_text, b, b_text]);
    // the text of A, where its label sits: the card is what is picked
    let at = app.to_screen(Pos2::new(150.0, 150.0));
    click(&mut app, &ctx, at);
    assert_eq!(app.selected, vec![0], "the short note's card, not its label");
    // over B's label box (which reaches over A) but on A's card: A is picked
    let at = app.to_screen(Pos2::new(250.0, 200.0));
    click(&mut app, &ctx, at);
    assert_eq!(app.selected, vec![0], "a long neighbour's text does not reach over A");
    // a click on B's card picks B; no label is ever part of a selection
    let at = app.to_screen(Pos2::new(600.0, 300.0));
    click(&mut app, &ctx, at);
    assert_eq!(app.selected, vec![2]);
    assert!(app.selected.iter().all(|i| !app.is_card_text(*i)));
}

#[test]
#[cfg(not(target_os = "linux"))] // portals have no native page on Linux yet
fn portal_page_fills_the_card_under_the_bar() {
    // the native page is the card's inside, under the header and the bar, at any zoom
    for zoom in [1.0, 2.0] {
        let app = DrawingApp { zoom, canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::splat(4000.0)), ..Default::default() };
        let mut e = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 960.0, height: 680.0 });
        e.portal = Some(PortalSpec { name: "Docs".into(), url: "https://a.dev".into(), ..Default::default() });
        let b = local_bounds(&e);
        let card = Rect::from_min_max(app.to_screen(b.min), app.to_screen(b.max));
        let chrome = PORTAL_CHROME * zoom;
        assert_eq!(app.portal_page_rect(&e), Some(Rect::from_min_max(card.min + Vec2::new(1.0, chrome + 1.0), card.max - Vec2::splat(1.0))));
        // with the bar hidden the page takes the whole card
        e.portal.as_mut().unwrap().chrome_hidden = true;
        assert_eq!(app.portal_page_rect(&e), Some(card.shrink(1.0)));
    }
}

#[test]
fn an_enlarged_portal_keeps_its_bar_at_any_canvas_zoom() {
    let mut app = DrawingApp { zoom: 0.3, now: 10.0, canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0)), ..Default::default() };
    let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 960.0, height: 680.0 });
    e.portal = Some(PortalSpec { name: "Docs".into(), url: "https://a.dev".into(), ..Default::default() });
    assert!(!app.chrome_live(&e), "this small on the canvas, its bar is only drawn");
    // enlarged it is shown at its own scale, whatever the canvas zoom
    app.terminal_elevated = Some(e.id.clone());
    assert!(app.chrome_live(&e));
}

#[cfg(not(target_os = "linux"))]
#[test]
fn the_apps_floating_ui_cuts_holes_in_a_portal_page() {
    let ctx = Context::default();
    let page = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(600.0, 400.0));
    let area = |id: &str, at: Pos2, order: egui::Order| {
        egui::Area::new(egui::Id::unique(id)).fixed_pos(at).order(order).show(&ctx, |ui| ui.set_min_size(Vec2::new(200.0, 80.0)));
    };
    for _ in 0..2 {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), ..Default::default() };
        ctx.run_ui(raw, |_| {
            // a menu over the page's left edge, one clear of it, and the canvas layer under it
            area("menu", Pos2::new(50.0, 150.0), egui::Order::Foreground);
            area("clear", Pos2::new(720.0, 520.0), egui::Order::Foreground);
            area("canvas", Pos2::new(300.0, 300.0), egui::Order::Background);
        })
        .textures_delta
        .clear();
    }
    assert_eq!(DrawingApp::page_holes(&ctx, page, &[]), [Rect::from_min_max(Pos2::new(100.0, 150.0), Pos2::new(250.0, 230.0))]);
    // the selection box's outline goes through the page too: four 2-point strips around it
    let edges = crate::app::browser::marquee_edges(Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(110.0, 70.0)));
    assert_eq!(edges[0], Rect::from_min_max(Pos2::new(9.0, 19.0), Pos2::new(111.0, 21.0)));
    assert_eq!(edges[3], Rect::from_min_max(Pos2::new(109.0, 19.0), Pos2::new(111.0, 71.0)));
}
