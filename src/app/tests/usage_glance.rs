use crate::app::island::plan::fitting;
use crate::app::usage::foot::{glance, glance_height, glance_tip};
use crate::app::usage::probe::{Agent, Meter, Reading};

fn reading(meters: &[(&str, f64)]) -> Reading {
    Reading { plan: Some("Max".into()), meters: meters.iter().map(|(title, used)| Meter { title: title.to_string(), used: *used, resets_at: None }).collect() }
}

/// The chat screen and the island say the 5-hour session and the week in short ("5 h", "7 dias"), Codex Go's month as
/// "30 dias"; a special limit stays in the sidebar's popover. The chat list's foot is as tall as what it says.
#[test]
fn the_plan_glance_says_the_session_and_the_week() {
    let claude = reading(&[("Sessão", 0.14), ("Semanal", 0.32), ("Opus", 0.5)]);

    assert_eq!(glance(&claude), vec![("5 h", 0.14), ("7 dias", 0.32)]);
    assert_eq!(glance(&reading(&[("Mensal", 1.0)])), vec![("30 dias", 1.0)]);
    assert_eq!(glance_tip(Agent::Claude, &claude, 0), "Claude Code · 5 h 14% · 7 dias 32%");
    assert_eq!(glance_height(&[]), 0.0);
    assert!(glance_height(&[(Agent::Claude, claude.clone())]) < glance_height(&[(Agent::Claude, claude.clone()), (Agent::Codex, claude)]));
}

/// The island's header shows the windows that fit between its tabs and its buttons, the session first.
#[test]
fn the_island_shows_the_windows_that_fit() {
    let plan = [("5 h", 0.14), ("7 dias", 0.32)];
    let width = |_: &str, _: f64| 80.0;

    assert_eq!(fitting(&plan, 0.0, 200.0, width), 2);
    assert_eq!(fitting(&plan, 0.0, 150.0, width), 1, "the week's 80 and the gap don't fit");
    assert_eq!(fitting(&plan, 0.0, 50.0, width), 0);
}

/// The open island says the plan's use in its header, between the tabs and the buttons.
#[test]
fn the_open_island_says_the_plan_in_its_header() {
    let (mut app, ctx, time) = super::island_opened(vec![]);

    app.usage_ui.probe.slots[Agent::Claude as usize].reading = Some(reading(&[("Sessão", 0.16), ("Semanal", 0.71)]));
    let texts = super::island_frame(&mut app, &ctx, time, vec![]);
    let rect = super::island_rect(&app);
    let buttons = super::right_button(&app, super::RIGHT_BUTTONS - 1).x - 12.0;

    for word in ["5 h", "16%", "7 dias", "71%"] {
        let (_, at) = texts.iter().find(|(text, _)| text == word).unwrap_or_else(|| panic!("{word} not in {texts:?}"));

        assert!(at.bottom() < rect.top() + 42.0 && at.right() < buttons, "{word} at {at:?}, in the header before {buttons}");
    }
}
