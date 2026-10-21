use crate::app::ui::palette::*;
use crate::prelude::*;

fn rect_element(x: f32, y: f32, width: f32, height: f32, fill: Option<[u8; 4]>) -> Element {
    let mut e = DrawingApp::default().new_element(ElementKind::Rectangle { x, y, width, height });
    e.style.fill = fill;
    e
}

/// Through the element JSON the clipboard and the MCP server use.
pub(crate) fn round_trip(elements: &[Element]) -> Vec<Element> {
    from_interop_elements(&json!(interop_elements(elements)))
}

#[test]
fn save_format_round_trips_scene() {
    let scene = Scene { elements: vec![rect_element(5.0, 8.0, 40.0, 30.0, None)], ..Default::default() };
    let decoded: Scene = serde_json::from_str(&serde_json::to_string(&scene).unwrap()).unwrap();
    assert_eq!(decoded, scene);
}

#[test]
fn legacy_rea_files_still_load() {
    let old = r#"{"version":1,"elements":[{"id":"a","kind":{"type":"rectangle","x":1,"y":2,"width":3,"height":4},"stroke":[1,2,3,255],"fill":null,"stroke_width":4}]}"#;
    let scene = parse_scene(old).unwrap();
    assert_eq!(scene.elements[0].style.stroke_width, 4.0);
    assert_eq!(scene.elements[0].style.roughness, 1);
}

#[test]
fn interop_format_round_trips() {
    let mut app = DrawingApp::default();
    app.scene.elements = vec![
        rect_element(10.0, 20.0, 50.0, 40.0, Some([255, 201, 201, 255])),
        app.new_element(ElementKind::Line { points: vec![[0.0, 0.0], [30.0, 40.0]], arrow: true }),
        app.new_element(ElementKind::Text { x: 5.0, y: 6.0, value: "Olá\nmundo".into() }),
    ];
    app.scene.elements[0].style.fill_style = FillStyle::Hachure;
    app.scene.elements[0].angle = 0.5;
    app.scene.elements[1].style.stroke_style = StrokeStyle::Dashed;
    let back = round_trip(&app.scene.elements);
    assert_eq!(back.len(), 3);
    assert_eq!(back[0].kind, app.scene.elements[0].kind);
    assert_eq!(back[0].style.fill_style, FillStyle::Hachure);
    assert!((back[0].angle - 0.5).abs() < 1e-6);
    assert_eq!(back[1].kind, app.scene.elements[1].kind);
    assert_eq!(back[1].style.stroke_style, StrokeStyle::Dashed);
    assert_eq!(back[2].kind, app.scene.elements[2].kind);
}

#[test]
fn svg_export_contains_drawn_shape() {
    let scene = Scene { elements: vec![rect_element(10.0, 20.0, 50.0, 40.0, Some([240, 240, 250, 255]))], ..Default::default() };
    let svg = scene_svg(&scene, Some(WHITE));
    assert!(svg.contains("<polygon") && svg.contains("<polyline"));
    assert!(svg.contains("width=\"70\""));
}

#[test]
fn png_export_is_a_valid_image() {
    let png = render_svg_png(&scene_svg(&Scene::default(), Some(WHITE)), 2.0).unwrap();
    let image = image::load_from_memory(&png).unwrap();
    assert_eq!(image.width(), 360);
}

#[test]
fn grouping_and_ungrouping_keeps_elements_together() {
    let mut app = DrawingApp::default();
    app.scene.elements = vec![rect_element(0.0, 0.0, 10.0, 10.0, None), rect_element(20.0, 0.0, 10.0, 10.0, None)];
    app.selected = vec![0, 1];
    app.group_selection();
    assert_eq!(app.scene.elements[0].group_id, app.scene.elements[1].group_id);
    app.ungroup_selection();
    assert!(app.scene.elements.iter().all(|e| e.group_id.is_none()));
}

#[test]
fn layering_moves_selection() {
    let mut app = DrawingApp::default();
    app.scene.elements = (0..4).map(|i| rect_element(i as f32, 0.0, 1.0, 1.0, None)).collect();
    let ids: Vec<String> = app.scene.elements.iter().map(|e| e.id.clone()).collect();
    app.selected = vec![1];
    app.reorder(Layer::Front);
    assert_eq!(app.scene.elements[3].id, ids[1]);
    assert_eq!(app.selected, vec![3]);
    app.reorder(Layer::Backward);
    assert_eq!(app.scene.elements[2].id, ids[1]);
    app.reorder(Layer::Back);
    assert_eq!(app.scene.elements[0].id, ids[1]);
}

#[test]
fn duplicate_and_paste_create_fresh_elements() {
    let mut app = DrawingApp::default();
    app.scene.elements = vec![rect_element(0.0, 0.0, 10.0, 10.0, None)];
    app.selected = vec![0];
    app.duplicate();
    assert_eq!(app.scene.elements.len(), 2);
    assert_ne!(app.scene.elements[0].id, app.scene.elements[1].id);
    assert_eq!(element_bounds(&app.scene.elements[1]).min, Pos2::new(10.0, 10.0));
    let clip = json!({ "type": "magoagent/clipboard", "elements": interop_elements(&app.scene.elements[..1]) });
    app.paste(&clip.to_string());
    assert_eq!(app.scene.elements.len(), 3);
    app.paste("texto colado");
    assert!(matches!(&app.scene.elements[3].kind, ElementKind::Text { value, .. } if value == "texto colado"));
}

#[test]
fn bindings_and_heads_round_trip_through_interop_format() {
    let mut app = DrawingApp::default();
    app.scene.elements = vec![rect_element(0.0, 0.0, 100.0, 100.0, None), rect_element(300.0, 0.0, 100.0, 100.0, None)];
    app.tool = Tool::Arrow;
    app.heads = (Some(Head::Circle), Some(Head::TriangleOutline));
    let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[95.0, 50.0], [305.0, 50.0]], arrow: true }));
    app.scene.elements.push(arrow);
    let file = json!(interop_elements(&app.scene.elements));
    assert_eq!(file[0]["boundElements"][0]["id"], app.scene.elements[2].id);
    assert_eq!(file[2]["startArrowhead"], "circle");
    assert_eq!(file[2]["endBinding"]["mode"], "orbit");
    let back = from_interop_elements(&file);
    assert_eq!(back[2].ends, app.scene.elements[2].ends);
    // legacy .rea arrows without "ends" keep their end arrowhead
    let mut legacy = serde_json::to_value(&app.scene.elements[2]).unwrap();
    legacy.as_object_mut().unwrap().remove("ends");
    let old: Element = serde_json::from_value(legacy).unwrap();
    assert_eq!((old.ends.start_head, old.ends.end_head), (None, Some(Head::Arrow)));
}

#[test]
fn arrowhead_shapes_match_reference_sizes() {
    let app = DrawingApp::default();
    let mut e = app.new_element(ElementKind::Line { points: vec![[0.0, 0.0], [200.0, 0.0]], arrow: true });
    let heads = arrowheads(&e);
    assert_eq!(heads.len(), 1);
    let (pts, head) = &heads[0];
    assert_eq!(*head, Head::Arrow);
    // size 25 at 20°: the barbs end 25 * cos(20°) behind the tip
    assert!((pts[0][0] - (200.0 - 25.0 * 20f32.to_radians().cos())).abs() < 0.01);
    e.ends = ArrowEnds { start_head: Some(Head::Bar), end_head: None, ..Default::default() };
    let heads = arrowheads(&e);
    assert_eq!(heads.len(), 1);
    assert!((heads[0].0[0][0] - 0.0).abs() < 0.01, "bar is perpendicular at the start tip");
}

#[test]
fn palette_search_filters_without_accents() {
    let mut app = DrawingApp::default();
    app.palette_query = "zoom".into();
    let found: Vec<&str> = app.palette_items().iter().map(|c| c.label()).collect();
    assert_eq!(found, ["Aumentar zoom", "Diminuir zoom", "Redefinir zoom"], "selection-only commands stay hidden");
    app.palette_query = "retangulo".into();
    assert_eq!(app.palette_items(), [Command::Tool(Tool::Rectangle)]);
    app.palette_query = "salvar comp".into();
    assert_eq!(app.palette_items(), [Command::Save]);
    app.selected = vec![0];
    app.palette_query = "zoom".into();
    assert_eq!(app.palette_items().len(), 4);
    assert_eq!(shortcut_keys("Cmd++"), ["Cmd", "+"]);
    assert_eq!(shortcut_keys("Cmd+Shift+E"), ["Cmd", "Shift", "E"]);
    app.run_command(Command::Tool(Tool::Ellipse));
    assert_eq!(app.tool, Tool::Ellipse);
}

#[test]
fn merge_keeps_both_sides_changes() {
    let a = rect_element(0.0, 0.0, 10.0, 10.0, None);
    let b = rect_element(50.0, 0.0, 10.0, 10.0, None);
    let base = Scene { elements: vec![a.clone(), b.clone()], ..Default::default() };
    // user moved `a` and drew `c`; the AI recolored `b` and added `d`
    let mut local = base.clone();
    local.elements[0].move_by(Vec2::new(5.0, 0.0));
    let c = rect_element(100.0, 0.0, 10.0, 10.0, None);
    local.elements.push(c.clone());
    let mut external = base.clone();
    external.elements[1].style.stroke = [255, 0, 0, 255];
    let d = rect_element(200.0, 0.0, 10.0, 10.0, None);
    external.elements.push(d.clone());
    let merged = merge_scenes(&base, &local, &external);
    let ids: Vec<&str> = merged.elements.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, [a.id.as_str(), b.id.as_str(), d.id.as_str(), c.id.as_str()]);
    assert_eq!(merged.elements[0], local.elements[0]);
    assert_eq!(merged.elements[1].style.stroke, [255, 0, 0, 255]);
    // the AI deleting an element the user didn't touch removes it
    let external = Scene { elements: vec![a.clone()], ..Default::default() };
    let merged = merge_scenes(&base, &base, &external);
    assert_eq!(merged.elements.len(), 1);
}

#[test]
fn minimap_covers_drawing_and_centers_on_click() {
    let mut app = DrawingApp::default();
    app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    app.scene.elements = vec![rect_element(3000.0, 2000.0, 100.0, 100.0, None)];
    let world = app.minimap_world();
    assert!(world.contains_rect(element_bounds(&app.scene.elements[0])));
    assert!(world.contains(Pos2::ZERO), "the visible area is part of the map");
    app.zoom = 2.0;
    app.pan_to(Pos2::new(3050.0, 2050.0));
    assert_eq!(app.to_screen(Pos2::new(3050.0, 2050.0)), app.canvas_rect.center());
}

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
fn dark_mode_flips_lightness() {
    let black = themed(rgba(BLACK), true);
    let white = themed(Color32::WHITE, true);
    assert!(black.r() > 200 && white.r() < 30);
    let red = themed(rgba([0xe0, 0x31, 0x31, 255]), true);
    assert!(red.r() > red.g() && red.r() > red.b());
}
