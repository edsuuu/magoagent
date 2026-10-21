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

#[test]
fn terminal_keys_and_paste() {
    let none = Modifiers::NONE;
    assert_eq!(key_bytes(Key::Enter, none, false), Some(b"\r".to_vec()));
    assert_eq!(key_bytes(Key::C, Modifiers::CTRL, false), Some(vec![3]));
    assert_eq!(key_bytes(Key::ArrowUp, none, true), Some(b"\x1bOA".to_vec()));
    assert_eq!(key_bytes(Key::ArrowUp, Modifiers::SHIFT, false), Some(b"\x1b[1;2A".to_vec()));
    assert_eq!(key_bytes(Key::Tab, Modifiers::SHIFT, false), Some(b"\x1b[Z".to_vec()));
    assert_eq!(key_bytes(Key::A, none, false), None, "printable keys come as Event::Text");
    assert_eq!(paste_bytes("a\nb", false), b"a\rb".to_vec());
    assert_eq!(paste_bytes("a\x1b[201~b", true), b"\x1b[200~a[201~b\x1b[201~".to_vec());
    // the grid's cell is the monospace font's real advance
    let ctx = Context::default();
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    let job = egui::text::LayoutJob::simple_singleline("M".repeat(100), FontId::monospace(TERMINAL_FONT), Color32::WHITE);
    let advance = ctx.fonts_mut(|f| f.layout_job(job)).size().x / 100.0;
    assert!((advance - CELL.x).abs() < 0.01, "{advance}");
    assert_eq!(grid_size(terminal_size(80, 24)), (80, 24));
    // block elements are drawn as exact cell parts: full, lower half, left 3/8, a quadrant mix
    assert_eq!(block_parts('█'), Some((vec![[0.0, 0.0, 1.0, 1.0]], 1.0)));
    assert_eq!(block_parts('▄'), Some((vec![[0.0, 0.5, 1.0, 1.0]], 1.0)));
    assert_eq!(block_parts('▍'), Some((vec![[0.0, 0.0, 0.375, 1.0]], 1.0)));
    assert_eq!(block_parts('▛').map(|(p, _)| p.len()), Some(3));
    assert_eq!(block_parts('a'), None);
}

#[cfg(unix)]
#[test]
fn terminal_runs_a_program_and_shows_its_output() {
    let program = ("/bin/sh", &["-c", "printf 'ol\\303\\241 \\033[31mmundo\\033[0m'"][..]);
    let session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program)).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !session.exited() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(session.exited());
    assert_eq!(session.handle.screen_text(), "olá mundo");
}

#[test]
fn bridge_lists_only_what_is_connected() {
    let mut app = DrawingApp::default();
    let terminal = |app: &DrawingApp, name: &str, x: f32| {
        let mut e = app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 300.0, height: 200.0 });
        e.terminal = Some(TerminalSpec { name: name.into(), ..Default::default() });
        e
    };
    let (a, b, c) = (terminal(&app, "Shell", 0.0), terminal(&app, "Claude Code", 400.0), terminal(&app, "Sozinho", 800.0));
    let (a_id, b_id) = (a.id.clone(), b.id.clone());
    app.scene.elements = vec![a, b, c];
    app.connect(&a_id, &b_id);
    // the connection hangs like a rope: its middle sags below the straight line between its ends
    let rope = app.scene.elements.iter().find(|e| e.rope).unwrap();
    let path = line_path(rope);
    let (first, middle, last) = (path[0], path[path.len() / 2], path[path.len() - 1]);
    assert!(middle[1] > (first[1] + last[1]) / 2.0 + 10.0, "{path:?}");
    assert!(element_bounds(rope).max.y >= middle[1]);
    // it swings: right after an end moves its middle lags behind, then it settles at rest
    app.swing_ropes(0.016);
    let b_index = app.scene.elements.iter().position(|e| e.id == b_id).unwrap();
    app.scene.elements[b_index].move_by(Vec2::new(300.0, 120.0));
    app.update_bindings();
    assert!(app.swing_ropes(0.016), "moving right after the drag");
    for _ in 0..600 {
        app.swing_ropes(0.016);
    }
    assert!(!app.swing_ropes(0.016), "settled");
    app.apply_agent_command(UiCommand::CreateNote { from: a_id.clone(), text: "# Plano\n- testar".into() });
    let nodes = app.agent_nodes();
    let shared = std::sync::Mutex::new(Shared { nodes, terminals: Default::default() });
    let (sender, commands) = std::sync::mpsc::channel();
    let ask = |cmd: Value| handle(&cmd, &shared, &sender);
    let list = ask(json!({ "cmd": "list", "from": a_id })).unwrap();
    assert!(list.contains("Você: \"Shell\"") && list.contains("\"Claude Code\"") && list.contains("\"Plano\""), "{list}");
    assert!(!list.contains("Sozinho"), "only what a line connects: {list}");
    assert_eq!(ask(json!({ "cmd": "note_read", "from": a_id, "name": "plano" })).unwrap(), "# Plano\n- testar");
    assert!(ask(json!({ "cmd": "check", "from": a_id, "name": "Sozinho" })).unwrap_err().contains("Disponíveis: \"Claude Code\""));
    ask(json!({ "cmd": "note_write", "from": a_id, "name": "Plano", "text": "feito" })).unwrap();
    let command = commands.try_recv().unwrap();
    app.apply_agent_command(command);
    assert_eq!(app.agent_nodes().iter().find(|n| n.kind == NodeKind::Note).unwrap().text, "feito");
    // a rope needs both ends: deleting a terminal takes its rope with it on the next frame
    let ropes = |app: &DrawingApp| app.scene.elements.iter().filter(|e| e.rope).count();
    assert_eq!(ropes(&app), 2);
    app.selected = vec![app.scene.elements.iter().position(|e| e.id == b_id).unwrap()];
    app.delete_selected();
    app.swing_ropes(0.016);
    assert_eq!(ropes(&app), 1, "the note's rope stays");
    // outside the canvas (MCP from another terminal app) everything is reachable
    assert!(ask(json!({ "cmd": "list", "from": "" })).unwrap().contains("Sozinho"));
    assert_eq!(unescape("a\\n\\x03\\e"), b"a\r\x03\x1b");
    assert_eq!(note_name("## Tarefas da semana\nresto"), "Tarefas da semana");
    assert_eq!(normalize_url("exemplo.com/a"), "https://exemplo.com/a");
    assert_eq!(normalize_url("localhost:3000"), "http://localhost:3000");
    assert_eq!(normalize_url(" https://x.dev "), "https://x.dev");
}

#[cfg(unix)]
#[test]
fn bridge_asks_an_agent_over_tcp() {
    let bridge = Bridge::start(None).unwrap();
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(("/bin/sh", &[][..]))).unwrap();
    let node = |id: &str, link: &str| Node { id: id.into(), kind: NodeKind::Terminal, name: id.into(), text: String::new(), links: vec![link.into()] };
    *bridge.shared.lock().unwrap() = Shared {
        nodes: vec![node("eu", "outro"), node("outro", "eu")],
        terminals: [("outro".to_string(), session.handle.clone())].into(),
    };
    let request = |cmd: Value| request_to(&bridge.address, &bridge.token, "eu", cmd, std::time::Duration::from_secs(20));
    let answer = request(json!({ "cmd": "ask", "name": "outro", "text": "echo pong-$((40+2))", "timeout": 20 })).unwrap();
    assert!(answer.contains("pong-42"), "{answer}");
    assert!(request(json!({ "cmd": "check", "name": "outro" })).unwrap().contains("pong-42"));
    assert!(request_to(&bridge.address, "errado", "eu", json!({ "cmd": "list" }), std::time::Duration::from_secs(5)).is_err());
}

#[test]
fn a_rope_leaves_from_the_edge_even_over_the_other_window() {
    let a = rect_element(0.0, 0.0, 100.0, 100.0, None);
    let center = Binding { element_id: a.id.clone(), fixed: [0.5, 0.5], inside: false };
    // the other window's center is inside this one: out through the right side, not the middle
    let p = bound_endpoint(&a, &center, Pos2::new(70.0, 50.0));
    assert!(p.x > 100.0 && (p.y - 50.0).abs() < 0.01, "{p:?}");
}

#[test]
fn agents_panel_reads_prompts_and_times() {
    use crate::app::ui::agents::{ago, asks_for_you};
    assert!(asks_for_you("Edit file src/main.rs\n Do you want to make this edit?\n ❯ 1. Yes\n   2. No"));
    assert!(!asks_for_you("> \n  ? for shortcuts"));
    assert_eq!(ago(2.0), "agora");
    assert_eq!(ago(30.0), "há 30s");
    assert_eq!(ago(725.0), "há 12 min");
    assert_eq!(ago(7300.0), "há 2 h");
}

#[test]
fn clicked_icon_jumps_bounces_and_rests() {
    assert_eq!(hop_height(0.0), 0.0);
    assert!((hop_height(0.11) - 4.0).abs() < 0.01);
    assert!((hop_height(0.29) - 1.5).abs() < 0.01);
    // 1.0 is what an icon that never hopped gets
    assert_eq!(hop_height(0.5), 0.0);
    assert_eq!(hop_height(1.0), 0.0);
}

#[test]
fn portal_tabs() {
    let mut portal = PortalSpec { name: "Docs".into(), url: "https://a.dev".into(), ..Default::default() };
    assert_eq!(portal.tab_urls(), ["https://a.dev"]);
    portal.open_tab("https://b.dev".into());
    portal.open_tab("https://c.dev".into());
    assert_eq!((portal.active_tab(), portal.url.as_str()), (2, "https://c.dev"));
    portal.select_tab(1);
    portal.set_tab_url(1, "https://b.dev/x".into());
    assert_eq!(portal.url, "https://b.dev/x");
    // closing the open tab shows the one that took its place; the last tab never closes
    portal.close_tab(1);
    assert_eq!((portal.active_tab(), portal.url.as_str()), (1, "https://c.dev"));
    portal.close_tab(1);
    portal.close_tab(0);
    assert_eq!(portal.tab_urls(), ["https://a.dev"]);
    let mut e = DrawingApp::default().new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 9.0, height: 9.0 });
    portal.open_tab("https://d.dev".into());
    e.portal = Some(portal.clone());
    assert_eq!(round_trip(&[e])[0].portal.as_ref(), Some(&portal));
}
