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
    assert_eq!(svg.matches("font-family=\"Excalifont, cursive\"").count(), 2, "{svg}");
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

/// The halo skips the paths whose box misses the text's: the same answer as walking every segment, with and without
/// crossings (a line through it, a point in it, a curve, a rope, freehand, nothing near).
#[test]
fn the_halo_answers_as_walking_every_segment_did() {
    fn walked(elements: &[Element], text: &Element) -> bool {
        let area = local_bounds(text).expand(2.0);
        let edges = [
            (area.left_top(), area.right_top()),
            (area.right_top(), area.right_bottom()),
            (area.right_bottom(), area.left_bottom()),
            (area.left_bottom(), area.left_top()),
        ];
        elements.iter().filter(|e| e.id != text.id).any(|e| {
            let path = match &e.kind {
                ElementKind::Line { .. } => line_path(e),
                ElementKind::FreeDraw { points } => points.clone(),
                _ => return false,
            };
            path.windows(2).any(|w| {
                let (a, b) = (Pos2::new(w[0][0], w[0][1]), Pos2::new(w[1][0], w[1][1]));
                area.contains(a) || area.contains(b) || edges.iter().any(|(c, d)| segment_intersection(a, b, [c.x, c.y], [d.x, d.y]).is_some())
            })
        })
    }
    let app = DrawingApp::default();
    let line = |points: Vec<[f32; 2]>| app.new_element(ElementKind::Line { points, arrow: false });
    let mut curve = line(vec![[0.0, 0.0], [150.0, 120.0], [300.0, 0.0]]);
    curve.style.round = true;
    let mut rope = line(vec![[0.0, 300.0], [300.0, 300.0]]);
    rope.rope = true;
    let drawn = app.new_element(ElementKind::FreeDraw { points: vec![[500.0, 0.0], [520.0, 40.0], [560.0, 60.0]] });
    let paths = [line(vec![[100.0, -50.0], [100.0, 400.0]]), line(vec![[0.0, 50.0], [300.0, 50.0]]), curve, rope, drawn];
    let (mut with, mut without) = (0, 0);
    for x in (-200..700).step_by(37) {
        for y in (-100..450).step_by(23) {
            let text = app.new_element(ElementKind::Text { x: x as f32, y: y as f32, value: "rótulo".into() });
            for path in &paths {
                let elements = vec![path.clone(), text.clone()];
                let halo = text_needs_halo(&elements, &text);
                assert_eq!(halo, walked(&elements, &text), "{x} {y} over {:?}", path.kind);
                if halo { with += 1 } else { without += 1 }
            }
        }
    }
    assert!(with > 20 && without > 20, "both cases seen: {with} with, {without} without");
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
        for (name, color) in [("danger", c.danger), ("warning", c.warning)] {
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

#[test]
fn fnv1a_gives_the_standard_values() {
    // the cookie stores on the Mac and the sketches' jitter are named by it: it must never drift
    assert_eq!(fnv1a(""), 0xcbf29ce484222325);
    assert_eq!(fnv1a("a"), 0xaf63dc4c8601ec8c);
    assert_eq!(fnv1a_from(0xcbf29ce484222325, "a"), fnv1a("a"));
}
