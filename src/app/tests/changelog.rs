//! O changelog (ui/changelog.rs e system/updates.rs): a versão nova achada pela verificação do dia abre o cartão uma vez
//! por dia local, Atualizar depois o fecha, nada abre sob um roteiro, e a versão instalada mostra as notas uma vez. Sem
//! rede: as respostas da verificação entram pelo canal, como a thread as mandaria.

use super::*;
use crate::app::system::updates::{Release, UpdateState, checks_on_opening, local_day};
use crate::app::ui::changelog::{Changelog, ChangelogChoice, notes_job};

fn release(version: &str, installer: Option<&str>) -> Release {
    Release { version: version.into(), notes: "## Novo\n- **Ajustes** maior".into(), page: String::new(), installer: installer.map(String::from), digest: None }
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
fn the_day_s_check_opens_the_card_once_and_later_closes_it() {
    // the first opening of a local day checks; the others that day don't
    assert!(checks_on_opening(0, 20_000) && checks_on_opening(19_999, 20_000) && !checks_on_opening(20_000, 20_000));
    let mut app = DrawingApp::default();
    app.updates.answer_for_test(Ok(Some(release("99.0.0", None))), true);
    app.poll_updates_on(20_000);
    assert_eq!(app.prefs.update_checked_day, 20_000);
    let card = app.changelog.clone().expect("the card");
    assert_eq!((card.version.as_str(), card.installed), ("99.0.0", false));
    let ctx = Context::default();
    crate::app::ui::install_fonts(&ctx, false);
    let texts = card_frame(&mut app, &ctx, vec![]);
    for text in ["Nova versão 99.0.0", "Atualizar agora", "Atualizar depois", "Novo"] {
        assert!(texts.iter().any(|painted| painted == text), "{text} in {texts:?}");
    }
    // Atualizar depois: closed, the update still in Ajustes › Sobre, and no other check that day
    app.changelog_choose(ChangelogChoice::Later);
    assert!(app.changelog.is_none() && matches!(app.updates.state, UpdateState::Available(_)));
    assert!(!checks_on_opening(app.prefs.update_checked_day, 20_000) && checks_on_opening(app.prefs.update_checked_day, 20_001));
    // a check asked from Ajustes or the MCP (not the day's) only updates Sobre
    app.updates.answer_for_test(Ok(Some(release("99.0.0", None))), false);
    app.poll_updates_on(20_000);
    assert!(app.changelog.is_none());
    // the local day is the UTC one moved by the zone: never more than a day apart
    let now = now_ms();
    assert!(local_day(now).abs_diff((now / 86_400_000) as u64) <= 1);
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
    app.poll_updates_on(20_000);
    assert!(app.changelog.is_none() && app.status.contains("99.0.0"), "a toast, as before");
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
