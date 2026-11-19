//! A apresentação de primeiro uso (ui/tour.rs): avança, volta e pula, não volta depois de vista, nada abre sob um
//! roteiro, e o recorte fica em volta do widget de verdade, seguindo-o quando ele muda de lugar.

use super::*;
use crate::app::ui::tour::{STEPS, Spot, Tour, spot_rect, tip_place};

const SIZE: Vec2 = Vec2::new(1280.0, 800.0);

/// One frame of what the tour points at (the bars, the sidebar, the corners) and the tour over them.
fn screen_frame(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) {
    // the sidebar slides by the app's clock
    app.now = time;
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), time: Some(time), events, ..Default::default() };

    let mut output = ctx.run_ui(raw, |_| {
        app.floating_controls(ctx);
        app.sidebar(ctx);

        if TITLE_BAR > 0.0 {
            app.title_bar(ctx);
        } else {
            // no drawn bar (the Mac): the panes' buttons are painted by the panes' own area
            app.panes_ui(ctx);
        }

        app.tour_ui(ctx);
    });

    output.textures_delta.clear();
}

fn step_of(app: &DrawingApp) -> Option<usize> {
    app.tour.as_ref().map(|tour| tour.step)
}

#[test]
fn the_tour_walks_back_and_forth_and_once_seen_stays_away() {
    let mut app = DrawingApp { chat_open: true, ..Default::default() };

    app.tour_on_open(None);
    assert_eq!(step_of(&app), Some(0), "the first time it opens by itself");
    assert!(!app.chat_open, "over the canvas");
    app.tour_input(true, false, false, 1.0);
    assert_eq!(step_of(&app), Some(0), "nothing before the first");
    app.tour_input(false, true, false, 1.0);
    app.tour_input(false, true, false, 2.0);
    assert_eq!(step_of(&app), Some(2));
    app.tour_input(true, false, false, 3.0);
    assert_eq!(step_of(&app), Some(1));

    for time in 0..STEPS.len() - 2 {
        app.tour_input(false, true, false, 4.0 + time as f64);
    }

    assert_eq!(step_of(&app), Some(STEPS.len() - 1));
    assert!(!app.prefs.tour_seen);
    // Concluir
    app.tour_input(false, true, false, 10.0);
    assert!(app.tour.is_none() && app.prefs.tour_seen && app.chat_open, "done, the chat back");
    app.tour_on_open(None);
    assert!(app.tour.is_none(), "seen: not by itself again");
    // Ajustes › Geral (or the MCP) opens it again; Pular ends it
    app.settings_open = true;
    app.start_tour();
    assert!(step_of(&app) == Some(0) && !app.settings_open);
    app.tour_input(false, false, true, 11.0);
    assert!(app.tour.is_none());
}

#[test]
fn nothing_opens_by_itself_under_a_roteiro_or_over_the_welcome() {
    let mut app = DrawingApp { script: Some(Script::new("wait 10")), ..Default::default() };

    app.tour_on_open(None);
    assert!(app.tour.is_none(), "MAGOAGENT_SCRIPT");
    app.tour_on_open(Some(3));
    assert_eq!(step_of(&app), Some(3), "unless a test scene asks for a step (MAGOAGENT_TOUR)");
    let mut app = DrawingApp { welcome: Some(Default::default()), ..Default::default() };

    app.tour_on_open(None);
    assert!(app.tour.is_none(), "the welcome first; its end opens the tour");
}

#[test]
fn the_mcp_opens_the_tour_on_a_step() {
    let mut app = DrawingApp::default();
    let said = app.act("tour", &json!({ "step": 2 })).unwrap();

    assert!(said.contains("passo 3 de 7"), "{said}");
    assert_eq!(step_of(&app), Some(2));
    assert!(crate::mcp::app_tools::is_action("tour"));
}

#[test]
fn the_hole_is_around_the_real_widgets_and_follows_them() {
    let ctx = Context::default();

    crate::app::ui::install_fonts(&ctx, false);
    let mut app = DrawingApp { tour: Some(Tour::default()), ..Default::default() };
    let mut time = 1.0;

    screen_frame(&mut app, &ctx, time, vec![]);
    assert_eq!(app.tour.as_ref().unwrap().hole, None, "the first step is in the middle, with no hole");

    for (index, step) in STEPS.iter().enumerate().skip(1) {
        app.tour_input(false, true, false, time);

        // past the slide from the last hole
        for _ in 0..3 {
            time += 0.2;
            screen_frame(&mut app, &ctx, time, vec![]);
        }

        let spot = step.spot.unwrap();
        let widget = spot_rect(&ctx, spot).unwrap_or_else(|| panic!("{spot:?} was painted"));
        let hole = app.tour.as_ref().unwrap().hole.unwrap();

        assert_eq!(step_of(&app), Some(index));
        // 6 around it, kept inside the window (the sidebar runs along its edge)
        let screen = Rect::from_min_size(Pos2::ZERO, SIZE);

        assert_eq!(hole, widget.expand(6.0).intersect(screen), "{spot:?}");
    }

    // the toolbar centers over the canvas past the sidebar: folding the sidebar moves it, and the hole goes with it
    app.tour.as_mut().unwrap().go(1, time);

    for _ in 0..3 {
        time += 0.2;
        screen_frame(&mut app, &ctx, time, vec![]);
    }

    let before = app.tour.as_ref().unwrap().hole.unwrap();

    app.prefs.sidebar_hidden = true;

    for _ in 0..6 {
        time += 0.2;
        screen_frame(&mut app, &ctx, time, vec![]);
    }

    let after = app.tour.as_ref().unwrap().hole.unwrap();
    let toolbar = spot_rect(&ctx, Spot::Toolbar).unwrap();

    assert!(after.center().x < before.center().x - 50.0 && after.contains_rect(toolbar), "{before:?} → {after:?}");
    // a target off the screen (the folded sidebar): the step in the middle, no hole
    app.tour.as_mut().unwrap().go(3, time);

    for _ in 0..3 {
        time += 0.2;
        screen_frame(&mut app, &ctx, time, vec![]);
    }

    assert_eq!(spot_rect(&ctx, Spot::Sidebar), None);
    assert_eq!(app.tour.as_ref().unwrap().hole, None);
}

#[test]
fn the_balloon_buttons_take_clicks_and_nothing_under_them_does() {
    let ctx = Context::default();

    crate::app::ui::install_fonts(&ctx, false);
    let mut app = DrawingApp { tour: Some(Tour::default()), ..Default::default() };
    let mut time = 1.0;

    let mut press = |app: &mut DrawingApp, label: &str| {
        // the balloon where this step has it
        for _ in 0..2 {
            time += 0.05;
            screen_frame(app, &ctx, time, vec![]);
        }

        let button = ctx.read_response(egui::Id::unique("tour").with(label)).unwrap_or_else(|| panic!("{label} on the balloon")).rect.center();

        for events in [vec![egui::Event::PointerMoved(button)], vec![super::button(button, true)], vec![super::button(button, false)]] {
            time += 0.05;
            screen_frame(app, &ctx, time, events);
        }
    };

    press(&mut app, "Começar");
    assert_eq!(step_of(&app), Some(1));
    press(&mut app, "Voltar");
    assert_eq!(step_of(&app), Some(0));
    press(&mut app, "Pular");
    assert!(app.tour.is_none() && app.prefs.tour_seen);
}

#[test]
fn the_hole_slides_between_targets() {
    let mut tour = Tour::default();
    let first = Rect::from_min_size(Pos2::new(100.0, 10.0), Vec2::new(200.0, 40.0));
    let second = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(260.0, 800.0));

    assert_eq!(tour.hole_at(Some(first), 0.0), Some(first));
    tour.hole = Some(first);
    tour.go(1, 1.0);
    let middle = tour.hole_at(Some(second), 1.1).unwrap();

    assert!(middle != first && middle != second && middle.width() > first.width() && middle.width() < second.width());
    assert_eq!(tour.hole_at(Some(second), 1.5), Some(second), "then on the target, wherever it goes");
    let moved = second.translate(Vec2::new(30.0, 0.0));

    assert_eq!(tour.hole_at(Some(moved), 2.0), Some(moved));
    assert_eq!(tour.hole_at(None, 2.0), None);
}

#[test]
fn the_balloon_stays_in_the_middle_unless_it_would_cover_its_hole() {
    let screen = Rect::from_min_size(Pos2::ZERO, SIZE);
    let size = Vec2::new(320.0, 160.0);
    // the toolbar at the top, the corner at the bottom right, the sidebar: the balloon stays in the middle, only the
    // hole moves
    let toolbar = Rect::from_min_size(Pos2::new(500.0, 10.0), Vec2::new(280.0, 44.0));
    let corner = Rect::from_min_size(Pos2::new(1000.0, 740.0), Vec2::new(270.0, 50.0));
    let sidebar = Rect::from_min_size(Pos2::ZERO, Vec2::new(260.0, 800.0));

    for hole in [None, Some(toolbar), Some(corner), Some(sidebar)] {
        assert_eq!(tip_place(hole, size, screen).center(), screen.center(), "{hole:?}");
    }

    // a hole in the middle: the balloon moves out of its way, under it, inside the window
    let middle = Rect::from_center_size(screen.center(), Vec2::new(200.0, 60.0));
    let tip = tip_place(Some(middle), size, screen);

    assert!(!tip.intersects(middle) && screen.contains_rect(tip));
}
