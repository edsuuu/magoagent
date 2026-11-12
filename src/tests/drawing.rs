use super::*;

#[test]
fn symbols_render_in_png_instead_of_boxes() {
    if !std::path::Path::new(SYMBOL_FONTS[0]).exists() {
        return; // not macOS
    }
    let line = |text: &str| {
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"60\" height=\"40\">{}</svg>", svg_text_line(5.0, 30.0, false, 28.0, "#000", None, text))
    };
    let missing = render_svg_png(&line("\u{E123}"), 1.0).unwrap();
    for symbol in ["→", "✓", "★", "⇒"] {
        assert_ne!(render_svg_png(&line(symbol), 1.0).unwrap(), missing, "{symbol} fell back to a box");
    }
    // only the symbol leaves the hand font, the words around it stay hand-drawn
    let svg = svg_text_line(0.0, 0.0, false, 20.0, "#000", None, "Login → Painel");
    assert_eq!(svg.matches("font-family=\"MagoHand, cursive\"").count(), 2, "{svg}");
    assert!(svg.contains(">Login <") && svg.contains(">Painel<"), "{svg}");
}

#[test]
fn text_over_lines_gets_an_outline() {
    let app = DrawingApp::default();
    let line = app.new_element(ElementKind::Line { points: vec![[0.0, 50.0], [300.0, 50.0]], arrow: true });
    let over = app.new_element(ElementKind::Text { x: 100.0, y: 40.0, value: "lê · grava".into() });
    let away = app.new_element(ElementKind::Text { x: 100.0, y: 200.0, value: "longe".into() });
    let elements = vec![line, over.clone(), away.clone()];
    assert!(text_needs_halo(&elements, &over));
    assert!(!text_needs_halo(&elements, &away));
    assert!(svg_text_line(0.0, 0.0, false, 20.0, "#000", Some("#ffffff"), "x").contains("paint-order=\"stroke\""));
}

#[test]
fn hachure_stays_inside_shape() {
    let mut e = rect_element(0.0, 0.0, 100.0, 60.0, Some([255, 0, 0, 255]));
    e.style.fill_style = FillStyle::CrossHatch;
    e.style.roughness = 0;
    let lines = fill_strokes(&e);
    assert!(lines.len() > 10);
    let area = element_bounds(&e).expand(1.0);
    assert!(lines.iter().flatten().all(|p| area.contains(Pos2::new(p[0], p[1]))));
}

#[test]
fn the_contrast_of_text_holds_in_both_themes() {
    // WCAG: sRGB's relative luminance, then (lighter + 0.05) / (darker + 0.05); 4.5 for body text
    let luminance = |c: Color32| {
        let linear = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    };
    let contrast = |a: Color32, b: Color32| {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    };
    for dark in [true, false] {
        let c = crate::app::ui::UiColors::new(dark);
        assert!(contrast(c.text, c.island) >= 4.5, "the panels' text, dark = {dark}");
    }
    assert!(contrast(TERMINAL_FG, TERMINAL_BG) >= 4.5, "the terminal's, in any theme");
    // the agents' state colors also read as text on the panels: 4.5 on light, 3 on dark
    for dark in [true, false] {
        let c = crate::app::ui::UiColors::new(dark);
        let min = if dark { 3.0 } else { 4.5 };
        for (name, color) in [("danger", c.danger), ("warning", c.warning), ("success", c.success)] {
            assert!(contrast(color, c.island) >= min, "{name} on the panels, dark = {dark}");
        }
    }
}

#[test]
fn dark_mode_flips_lightness() {
    let black = themed(rgba(BLACK), true);
    let white = themed(Color32::WHITE, true);
    assert!(black.r() > 200 && white.r() < 30);
    let red = themed(rgba([0xe0, 0x31, 0x31, 255]), true);
    assert!(red.r() > red.g() && red.r() > red.b());
}
