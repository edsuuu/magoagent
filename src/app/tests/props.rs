use super::*;

/// The row's buttons and its popover's, where the last frame drew them.
fn props(ctx: &Context) -> (Vec<Rect>, Vec<Rect>) {
    ctx.data(|d| d.get_temp(egui::Id::unique("props_buttons"))).unwrap_or_default()
}

fn open(ctx: &Context) -> Option<&'static str> {
    ctx.data(|d| d.get_temp(egui::Id::unique("props_open")))
}

fn row_shown(ctx: &Context) -> bool {
    ctx.memory(|m| m.areas().visible_last_frame(&egui::LayerId::new(egui::Order::Middle, egui::Id::unique("properties"))))
}

/// Frames far enough apart for the row's entrance to be over.
fn settle(app: &mut DrawingApp, ctx: &Context) {
    for t in [10.0, 11.0] {
        frame_at(app, ctx, Some(t), vec![]);
    }
}

#[test]
fn a_selected_shape_shows_the_options_row_and_its_color_changes_the_shape() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    app.tool = Tool::Rectangle;
    for _ in 0..2 {
        frame(&mut app, &ctx, vec![]);
    }
    drag(&mut app, &ctx, Pos2::new(300.0, 300.0), Pos2::new(500.0, 450.0));
    settle(&mut app, &ctx);
    assert!(app.shows_properties() && row_shown(&ctx));
    // 36 tall, centered over the canvas (past the sidebar), under the toolbars
    let row = ctx.memory(|m| m.area_rect(egui::Id::unique("properties"))).unwrap();
    let middle = (800.0 + app.sidebar_width()) / 2.0;
    assert_eq!(row.height(), GLASS_HEIGHT);
    assert!((row.center().x - middle).abs() < 1.0 && row.top() >= TOOLBAR_BOTTOM, "{row:?}");
    assert!(row.left() > app.sidebar_width(), "it does not cover the sidebar: {row:?}");
    // the stroke color: a dot that opens a popover, whose swatches paint the shape
    assert_eq!(open(&ctx), None);
    click(&mut app, &ctx, props(&ctx).0[0].center());
    assert_eq!(open(&ctx), Some("stroke"));
    for _ in 0..2 {
        frame(&mut app, &ctx, vec![]);
    }
    click(&mut app, &ctx, props(&ctx).1[1].center());
    assert_eq!(app.scene.elements[0].style.stroke, STROKE_PICKS[1]);
    assert_eq!(app.style.stroke, STROKE_PICKS[1], "and the next shapes");
    // Esc puts the popover away and keeps the selection
    let esc = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame(&mut app, &ctx, vec![esc]);
    assert_eq!((open(&ctx), app.selected.len()), (None, 1));
    // a press anywhere else too
    click(&mut app, &ctx, props(&ctx).0[0].center());
    assert_eq!(open(&ctx), Some("stroke"));
    for _ in 0..2 {
        frame(&mut app, &ctx, vec![]);
    }
    let away = Pos2::new(600.0, 520.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(away), button(away, true)]);
    assert_eq!(open(&ctx), None);
}

#[test]
fn a_choice_group_is_buttons_or_one_popover_and_both_set_the_style() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    app.tool = Tool::Rectangle;
    for _ in 0..2 {
        frame(&mut app, &ctx, vec![]);
    }
    drag(&mut app, &ctx, Pos2::new(300.0, 300.0), Pos2::new(500.0, 450.0));
    settle(&mut app, &ctx);
    // beside the sidebar there is no room for every choice: one button per group, its choices in a popover
    let folded = props(&ctx).0;
    app.prefs.sidebar_hidden = true;
    for t in [12.0, 13.0] {
        frame_at(&mut app, &ctx, Some(t), vec![]);
    }
    let buttons = props(&ctx).0;
    assert!(folded.len() < buttons.len(), "{} {}", folded.len(), buttons.len());
    app.prefs.sidebar_hidden = false;
    for t in [14.0, 15.0] {
        frame_at(&mut app, &ctx, Some(t), vec![]);
    }
    let folded = props(&ctx).0;
    // color, background, then the width group's button; its third choice is "Muito espesso"
    click(&mut app, &ctx, folded[2].center());
    assert_eq!(open(&ctx), Some("width"));
    for _ in 0..2 {
        frame(&mut app, &ctx, vec![]);
    }
    click(&mut app, &ctx, props(&ctx).1[2].center());
    assert_eq!((app.scene.elements[0].style.stroke_width, open(&ctx)), (4.0, None), "a choice sets and closes");
}

#[test]
fn nothing_selected_and_no_drawing_tool_leaves_no_row() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    settle(&mut app, &ctx);
    assert!(!app.shows_properties() && !row_shown(&ctx));
    app.tool = Tool::Ellipse;
    settle(&mut app, &ctx);
    assert!(row_shown(&ctx));
    app.tool = Tool::Select;
    settle(&mut app, &ctx);
    assert!(!row_shown(&ctx));
}

#[test]
fn the_node_options_bar_stays_under_the_toolbar_with_the_sidebar_open_or_folded() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    let mut note = app.new_element(ElementKind::Rectangle { x: 300.0, y: 300.0, width: 260.0, height: 160.0 });
    note.created = Some(0);
    app.scene.elements.push(note);
    app.selected = vec![0];
    for folded in [false, true] {
        app.prefs.sidebar_hidden = folded;
        settle(&mut app, &ctx);
        let bar = ctx.memory(|m| m.area_rect(egui::Id::unique("context_bar"))).unwrap();
        let tools = app.toolbar_span;
        assert!((bar.center().x - tools.center().x).abs() < 1.0, "folded {folded}: {bar:?} under {tools:?}");
    }
}
