use super::*;

#[test]
fn f2_moves_the_editor_cursor_between_diagnostics_without_wrapping() {
    use super::super::{Ide, Tab, char_at};

    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    for (name, family) in [("ide", egui::FontFamily::Proportional), ("ide-mono", egui::FontFamily::Monospace)] {
        fonts.families.insert(egui::FontFamily::Name(name.into()), fonts.families[&family].clone());
    }
    ctx.set_fonts(fonts);
    // A missing folder avoids starting servers and the IDE integration.
    let root = std::env::temp_dir().join(format!("magoagent-f2-{}", uuid::Uuid::new_v4()));
    let path = root.join("a.txt");
    let mut ide = Ide::new(ctx.clone(), "f2", &root.to_string_lossy());
    ide.code_vision = false;
    ide.tabs.push(Tab::new(path.clone(), "ação abc\numa linha\noutra linha\nfim".into(), false));
    let mut insp = Inspections::new(ctx.clone(), root);
    let diagnostics: Vec<_> = [(2, 4), (0, 3), (2, 1)].into_iter().map(|(line, col)| json!({
        "range": {"start": {"line": line, "character": col}, "end": {"line": line, "character": col + 1}},
        "severity": 1, "message": "Problema de teste"
    })).collect();
    insp.publish("test", &json!({"uri": uri(&path), "diagnostics": diagnostics}));
    ide.insp = Some(insp);

    let mut frame = |modifiers, expected| {
        let events = [true, false].into_iter().map(|pressed| egui::Event::Key {
            key: egui::Key::F2, physical_key: None, pressed, repeat: false, modifiers,
        }).collect();
        let mut out = ctx.run_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 600.0))),
            events, ..Default::default()
        }, |ui| ide.ui(ui, true));
        out.textures_delta.clear();
        let tab = &ide.tabs[0];
        assert_eq!(tab.cursor, expected);
        let c = char_at(&tab.text, expected.0, expected.1);
        assert_eq!(tab.sel, (c, c));
    };
    for at in [(0, 3), (2, 1), (2, 4), (2, 4)] {
        frame(egui::Modifiers::NONE, at);
    }
    for at in [(2, 1), (0, 3), (0, 3)] {
        frame(egui::Modifiers::SHIFT, at);
    }
}
