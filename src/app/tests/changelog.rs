//! O changelog (ui/changelog.rs e system/updates.rs): a versão nova achada pelo app (ao abrir e a cada hora) abre o
//! cartão num momento calmo, Atualizar depois o adia por 6 h ou até o computador reiniciar, Pular esta versão a deixa de
//! lado, nada abre sob um roteiro, e a versão instalada mostra as notas uma vez. Sem rede: as respostas da verificação
//! entram pelo canal, como a thread as mandaria.

use super::*;
use crate::app::system::updates::{CHECK_EVERY_MS, LATER_MS, RETRY_MS, Release, UpdateState, later_holds};
use crate::app::ui::changelog::{Changelog, ChangelogChoice, notes_job};

fn release(version: &str, installer: Option<&str>) -> Release {
    Release {
        version: version.into(),
        notes: "## Novo\n- **Ajustes** maior".into(),
        page: String::new(),
        installer: installer.map(String::from),
        digest: None,
        size: None,
    }
}

/// One frame of the card; the texts it painted.
fn card_frame(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) -> Vec<String> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))), events, ..Default::default() };
    // twice: an area's first frame only measures it
    let mut output = ctx.run_ui(raw.clone(), |_| app.changelog_ui(ctx));
    if app.changelog.is_some() {
        output = ctx.run_ui(egui::RawInput { events: vec![], ..raw }, |_| app.changelog_ui(ctx));
    }
    output.textures_delta.clear();
    output.shapes.iter().filter_map(|shape| if let egui::Shape::Text(text) = &shape.shape { Some(text.galley.text().to_string()) } else { None }).collect()
}

#[test]
fn the_apps_check_opens_the_card_and_later_puts_it_off_for_six_hours() {
    let now = now_ms();
    let mut app = DrawingApp::default();
    app.updates.answer_for_test(Ok(Some(release("99.0.0", None))), true);
    app.poll_updates_at(now);
    assert_eq!(app.updates.next_check, Some(now + CHECK_EVERY_MS), "with Sempre, again in an hour");
    assert!(app.changelog.is_none() && app.updates.offer.is_some(), "it waits for a quiet frame");
    let ctx = Context::default();
    crate::app::ui::install_fonts(&ctx, false);
    let texts = card_frame(&mut app, &ctx, vec![]);
    let card = app.changelog.clone().expect("the card, nothing going on");
    assert_eq!((card.version.as_str(), card.installed), ("99.0.0", false));
    for text in ["Nova versão 99.0.0", "Atualizar agora", "Atualizar depois", "Pular esta versão", "Novo"] {
        assert!(texts.iter().any(|painted| painted == text), "{text} in {texts:?}");
    }
    assert!(texts.iter().any(|painted| painted.starts_with("Você está na ")), "{texts:?}");
    // Atualizar depois: closed for 6 h, the update still in Ajustes › Sobre
    app.changelog_choose(ChangelogChoice::Later);
    assert!(app.changelog.is_none() && matches!(app.updates.state, UpdateState::Available(_)));
    let later = app.prefs.update_later.expect("put off");
    app.updates.answer_for_test(Ok(Some(release("99.0.0", None))), true);
    app.poll_updates_at(later + CHECK_EVERY_MS);
    assert!(app.updates.offer.is_none(), "the next hour's check keeps quiet");
    app.updates.answer_for_test(Ok(Some(release("99.0.0", None))), true);
    app.poll_updates_at(later + LATER_MS);
    assert!(app.updates.offer.is_some(), "six hours later it comes back");
    // a check asked from Ajustes or the MCP only updates Sobre
    app.updates.offer = None;
    app.updates.answer_for_test(Ok(Some(release("99.0.0", None))), false);
    app.poll_updates_at(later + LATER_MS);
    assert!(app.updates.offer.is_none() && app.changelog.is_none());
    // a failed check is tried again in half an hour
    app.updates.answer_for_test(Err("sem rede".into()), true);
    app.poll_updates_at(now);
    assert_eq!(app.updates.next_check, Some(now + RETRY_MS));
}

#[test]
fn later_ends_after_six_hours_or_a_restart_of_the_computer() {
    let at = 1_000_000_000;
    assert!(later_holds(Some(at), at + LATER_MS - 1, Some(at - 5_000)), "same boot, within 6 h");
    assert!(!later_holds(Some(at), at + LATER_MS, Some(at - 5_000)), "6 h on");
    assert!(!later_holds(Some(at), at + 60_000, Some(at + 30_000)), "the computer restarted after it");
    assert!(later_holds(Some(at), at + 60_000, None), "the boot unknown: the 6 h only");
    assert!(!later_holds(None, at, None));
    assert!(crate::app::system::updates::boot_ms().is_none_or(|boot| boot < now_ms()));
}

#[test]
fn skipping_a_version_keeps_its_card_away_until_a_newer_one() {
    let now = now_ms();
    let mut app = DrawingApp::default();
    app.updates.state = UpdateState::Available(release("99.0.0", None));
    app.changelog = Some(Changelog { version: "99.0.0".into(), notes: String::new(), installed: false });
    app.changelog_choose(ChangelogChoice::Skip);
    assert!(app.changelog.is_none() && app.prefs.update_skipped == "99.0.0");
    app.updates.answer_for_test(Ok(Some(release("99.0.0", None))), true);
    app.poll_updates_at(now);
    assert!(app.updates.offer.is_none(), "skipped");
    app.updates.answer_for_test(Ok(Some(release("99.0.1", None))), true);
    app.poll_updates_at(now);
    assert_eq!(app.updates.offer.as_ref().map(|release| release.version.as_str()), Some("99.0.1"), "a newer one is offered");
    // the MCP's later and skip go the same way
    assert!(app.update_put_off(false).unwrap().contains("6 horas") && app.prefs.update_later.is_some());
    assert!(DrawingApp::default().update_put_off(true).is_err(), "nothing waiting");
}

#[test]
fn typing_holds_the_card_and_it_stays_put_when_it_grows() {
    let mut app = DrawingApp::default();
    app.updates.state = UpdateState::Available(release("99.0.0", None));
    app.updates.offer = Some(release("99.0.0", None));
    let ctx = Context::default();
    crate::app::ui::install_fonts(&ctx, false);
    card_frame(&mut app, &ctx, vec![egui::Event::Text("a".into())]);
    assert!(app.changelog.is_none(), "a key was just pressed");
    if cfg!(windows) {
        // the card's top stays where it opened while the warning makes it taller
        app.updates.offer = None;
        app.updates.state = UpdateState::Available(release("99.0.0", Some("https://github.com/edsuuu/magoagent/releases/download/v99.0.0/x.msi")));
        app.changelog = Some(Changelog { version: "99.0.0".into(), notes: String::new(), installed: false });
        let top = |ctx: &Context| ctx.memory(|memory| memory.area_rect(egui::Id::unique("changelog"))).map(|rect| rect.top());
        card_frame(&mut app, &ctx, vec![]);
        let before = top(&ctx).expect("the card");
        app.changelog_choose(ChangelogChoice::Now);
        let texts = card_frame(&mut app, &ctx, vec![]);
        assert!(texts.iter().any(|text| text.contains("fecha e reabre")) && !texts.iter().any(|text| text.contains('`')), "{texts:?}");
        assert_eq!(top(&ctx), Some(before), "it doesn't jump");
    }
}

#[test]
fn esc_on_the_card_is_later() {
    let mut app = DrawingApp { changelog: Some(Changelog { version: "99.0.0".into(), notes: String::new(), installed: false }), ..Default::default() };
    let ctx = Context::default();
    crate::app::ui::install_fonts(&ctx, false);
    assert!(card_frame(&mut app, &ctx, vec![]).iter().any(|text| text == "Essa release não traz notas."));
    card_frame(&mut app, &ctx, vec![key_press(Key::Escape)]);
    assert!(app.changelog.is_none());
}

#[test]
fn atualizar_agora_warns_first_where_the_app_closes_and_reopens() {
    let mut app = DrawingApp::default();
    app.changelog = Some(Changelog { version: "99.0.0".into(), notes: String::new(), installed: false });
    // nothing waiting (the state moved on): it says so and closes
    app.changelog_choose(ChangelogChoice::Now);
    assert!(app.changelog.is_none() && app.status.contains("não há atualização"), "{}", app.status);
    if cfg!(windows) {
        // the MSI: the warning, then Cancelar keeps the card; nothing is downloaded here
        app.changelog = Some(Changelog { version: "99.0.0".into(), notes: String::new(), installed: false });
        app.updates.state = UpdateState::Available(release("99.0.0", Some("https://github.com/edsuuu/magoagent/releases/download/v99.0.0/x.msi")));
        app.changelog_choose(ChangelogChoice::Now);
        assert!(app.updates.confirming && app.changelog.is_some());
        let ctx = Context::default();
        crate::app::ui::install_fonts(&ctx, false);
        let texts = card_frame(&mut app, &ctx, vec![]);
        assert!(texts.iter().any(|text| text == "Atualizar e reiniciar") && texts.iter().any(|text| text.contains("fecha e reabre")), "{texts:?}");
        app.changelog_choose(ChangelogChoice::Cancel);
        assert!(!app.updates.confirming && app.changelog.is_some() && matches!(app.updates.state, UpdateState::Available(_)));
    }
}

#[test]
fn nothing_opens_under_a_roteiro() {
    let mut app = DrawingApp { script: Some(Script::new("wait 10")), ..Default::default() };
    app.updates.answer_for_test(Ok(Some(release("99.0.0", None))), true);
    app.poll_updates_at(now_ms());
    assert!(app.changelog.is_none() && app.updates.offer.is_none() && app.status.contains("99.0.0"), "a toast, as before");
    app.prefs.whats_new = Some((env!("CARGO_PKG_VERSION").into(), "notas".into()));
    app.show_whats_new(env!("CARGO_PKG_VERSION"));
    assert!(app.changelog.is_none() && app.prefs.whats_new.is_none());
}

#[test]
fn the_installed_version_shows_its_notes_once() {
    let mut app = DrawingApp::default();
    // a later version's notes wait for it (the update failed or is still to come)
    app.prefs.whats_new = Some(("0.0.30".into(), "trinta".into()));
    app.show_whats_new("0.0.29");
    assert!(app.changelog.is_none() && app.prefs.whats_new.is_some());
    app.show_whats_new("0.0.30");
    assert_eq!(app.changelog, Some(Changelog { version: "0.0.30".into(), notes: "trinta".into(), installed: true }));
    assert!(app.prefs.whats_new.is_none());
    let ctx = Context::default();
    crate::app::ui::install_fonts(&ctx, false);
    let texts = card_frame(&mut app, &ctx, vec![]);
    assert!(texts.iter().any(|text| text == "O que há de novo na 0.0.30") && texts.iter().any(|text| text == "Fechar"), "{texts:?}");
    assert!(!texts.iter().any(|text| text == "Atualizar agora"));
    app.changelog_choose(ChangelogChoice::Later);
    assert!(app.changelog.is_none());
    // an earlier version's notes just go
    app.prefs.whats_new = Some(("0.0.2".into(), "velhas".into()));
    app.show_whats_new("0.0.30");
    assert!(app.changelog.is_none() && app.prefs.whats_new.is_none());
}

#[test]
fn the_notes_are_simple_markdown() {
    let spans = crate::render::markdown::inline("um **forte** e `código`", false);
    let job = notes_job(&spans, 13.5, Color32::WHITE, 400.0);
    let fonts: Vec<_> = job.sections.iter().map(|section| section.format.font_id.family.clone()).collect();
    assert_eq!(job.text, "um forte e código");
    assert_eq!(fonts[0], egui::FontFamily::Proportional);
    assert!(matches!(&fonts[1], egui::FontFamily::Name(name) if name.as_ref() == crate::app::ui::SYSTEM_SEMIBOLD));
    assert_eq!(fonts[3], egui::FontFamily::Monospace);
}
