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
fn terminal_look_round_trips_and_old_files_default_it() {
    let mut plain = rect_element(0.0, 0.0, 300.0, 200.0, None);
    plain.terminal = Some(TerminalSpec { name: "a".into(), command: "claude".into(), ..Default::default() });
    let mut tinted = plain.clone();
    tinted.terminal = Some(TerminalSpec { icon: "eye".into(), color: Some([1, 2, 3]), quiet: true, maestro: false, ..plain.terminal.clone().unwrap() });
    // no tint is null on the wire and None back
    assert_eq!(interop_elements(&[plain.clone()])[0]["color"], Value::Null);
    let back = round_trip(&[plain.clone(), tinted.clone()]);
    assert_eq!((&back[0].terminal, &back[1].terminal), (&plain.terminal, &tinted.terminal));
    // scenes and prefs saved before the fields existed: monitoring on, the default quick starts
    let old: TerminalSpec = serde_json::from_str(r#"{"name":"a","cwd":"","command":"claude"}"#).unwrap();
    assert_eq!((old.icon.as_str(), old.color, old.quiet), ("", None, false));
    assert!(old.maestro, "scenes saved before Maestro are Maestros");
    assert_eq!(serde_json::from_str::<Prefs>("{}").unwrap().presets, default_presets());
}

#[test]
fn ide_element_round_trips_through_both_formats() {
    let mut e = rect_element(10.0, 20.0, 720.0, 460.0, None);
    e.ide = Some(IdeSpec { name: "loja".into(), root: "/tmp/loja".into(), ..Default::default() });
    assert!(e.is_node());
    let scene = Scene { elements: vec![e.clone()], ..Default::default() };
    let saved = serde_json::to_string(&scene).unwrap();
    assert_eq!(parse_scene(&saved).unwrap(), scene);
    let back = round_trip(&[e.clone()]);
    assert_eq!((back[0].ide.as_ref(), back[0].kind.clone()), (e.ide.as_ref(), e.kind.clone()));
    // plain shapes don't grow the field in the saved file
    assert!(!serde_json::to_string(&rect_element(0.0, 0.0, 1.0, 1.0, None)).unwrap().contains("\"ide\""));
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
    // off the Mac the labels say Ctrl and Alt, the keys egui's `command` and `alt` take there
    let mac = cfg!(target_os = "macos");
    assert_eq!(keys_label("Cmd+Option+]"), if mac { "Cmd+Option+]" } else { "Ctrl+Alt+]" });
    assert_eq!(keys_label("Chat — ⌘1"), if mac { "Chat — ⌘1" } else { "Chat — Ctrl+1" });
    app.run_command(Command::Tool(Tool::Ellipse));
    assert_eq!(app.tool, Tool::Ellipse);
}

#[test]
fn tool_badges_come_from_their_keys() {
    let badges: Vec<&str> = Tool::TOOLBAR.iter().chain(&Tool::DRAWING).chain(&Tool::EXTRA).map(|t| t.badge()).collect();
    assert_eq!(badges, ["", "V", "", "", "", "N", "T", "R", "D", "O", "A", "L", "P", "E", "F", "Shift+X", "K", "B", ""]);
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

#[cfg(windows)]
#[test]
fn clearing_a_windows_terminal_has_the_shell_clear_too() {
    // ConPTY would paint the old screen back on resize: Limpar buffer also sends ^L to the program
    let program = ("powershell", &["-NoProfile", "-Command", "Write-Host ('tecla ' + [int][Console]::ReadKey($true).KeyChar)"][..]);
    let session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program)).unwrap();
    session.handle.clear();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !session.exited() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(session.exited());
    assert_eq!(session.handle.screen_text(), "tecla 12");
}

#[cfg(windows)]
#[test]
fn a_windows_terminal_opens_powershell_7_when_it_is_installed() {
    // Windows PowerShell's default policy stops npm's claude.ps1; PowerShell 7's lets it run
    let path = std::env::var_os("PATH").unwrap_or_default();
    let major = if std::env::split_paths(&path).any(|dir| dir.join("pwsh.exe").is_file()) { 7 } else { 5 };
    let command = "Write-Host \"versao $($PSVersionTable.PSVersion.Major)\"; exit";
    let session = TerminalSession::spawn("", command, (60, 20), None, vec![], None).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !session.exited() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(session.exited());
    let screen = session.handle.screen_text();
    assert!(screen.contains(&format!("versao {major}")), "{screen}");
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
    let note = app.scene.elements.iter().find(|e| e.created.is_some()).unwrap();
    let label = app.scene.elements.iter().find(|e| e.id == format!("{}-label", note.id)).unwrap();
    assert!((local_bounds(label).center() - element_bounds(note).center()).length() < 0.1, "the label sits in the middle");
    let nodes = app.agent_nodes();
    let shared = std::sync::Mutex::new(Shared { nodes, terminals: Default::default(), asking: Vec::new() });
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
    // not while an end is being dragged (the press unbinds it); dropped in the open, it goes
    app.point_drag = Some(0);
    app.scene.elements.iter_mut().find(|e| e.rope).unwrap().ends.start = None;
    app.swing_ropes(0.016);
    assert_eq!(ropes(&app), 1, "still dragging");
    app.point_drag = None;
    app.swing_ropes(0.016);
    assert_eq!(ropes(&app), 0);
    // `connect` reaches the whole canvas by name: the caller ropes itself to the lone terminal
    std::thread::scope(|s| {
        let asked = s.spawn(|| ask(json!({ "cmd": "connect", "from": a_id, "name": "sozinho" })));
        app.apply_agent_command(commands.recv().unwrap());
        assert!(asked.join().unwrap().unwrap().ends_with("conectados."));
    });
    assert_eq!(ropes(&app), 1);
    assert!(ask(json!({ "cmd": "recruit", "from": "", "name": "X" })).is_err(), "a recruit needs a terminal to sit below");
    // outside the canvas (MCP from another terminal app) everything is reachable
    assert!(ask(json!({ "cmd": "list", "from": "" })).unwrap().contains("Sozinho"));
    assert_eq!(unescape("a\\n\\x03\\e"), b"a\r\x03\x1b");
    assert_eq!(note_name("## Tarefas da semana\nresto"), "Tarefas da semana");
    // the formatting bar wraps the whole line: the title drops the closing marks too
    assert_eq!(note_name("**Nota em negrito**"), "Nota em negrito");
    assert_eq!(note_name("- ~~riscada~~"), "riscada");
    assert_eq!(note_name("* item"), "item");
    // the card's markdown: block kinds per line, marks dropped from inline text
    use crate::render::markdown::{Line, Mark, inline, lines};
    assert_eq!(lines("## Oi\n  - [x] feito\n2. dois\n> cita\n---\n```\n*a*\n```"), [
        Line::Heading(2, "Oi"),
        Line::Item { depth: 1, mark: Mark::Task(true), text: "feito" },
        Line::Item { depth: 0, mark: Mark::Number("2.".into()), text: "dois" },
        Line::Quote("cita"),
        Line::Rule,
        Line::Code("*a*"),
    ]);
    let spans: Vec<(String, bool, bool)> = inline("a **b** `c` [d](u)", false).into_iter().map(|s| (s.text, s.bold, s.code || s.link)).collect();
    assert_eq!(spans, [("a ".into(), false, false), ("b".into(), true, false), (" ".into(), false, false), ("c".into(), false, true), (" ".into(), false, false), ("d".into(), false, true)]);
    assert_eq!(normalize_url("exemplo.com/a"), "https://exemplo.com/a");
    assert_eq!(normalize_url("localhost:3000"), "http://localhost:3000");
    assert_eq!(normalize_url(" https://x.dev "), "https://x.dev");
    // a scheme only at the start; only the web leaves for the browser
    assert_eq!(normalize_url("exemplo.com/?r=http://x"), "https://exemplo.com/?r=http://x");
    assert_eq!(normalize_url("file:///C:/a.html"), "file:///C:/a.html");
    assert!(is_web("HTTPS://x.dev") && is_web("http://localhost:3000"));
    assert!(!is_web("file:///C:/a.html") && !is_web("mailto:a@b.c") && !is_web("ms-settings:") && !is_web("about:blank"));
    // the agent's cursor is built without innerHTML (pages with Trusted Types)
    let cursor = cursor_script("IA", "#a", None);
    assert!(!cursor.contains("innerHTML") && cursor.contains("createElementNS"));
}

#[cfg(unix)]
#[test]
fn bridge_asks_an_agent_over_tcp() {
    let bridge = Bridge::start(None).unwrap();
    let session = TerminalSession::spawn("", "", (60, 10), None, vec![], Some(("/bin/sh", &[][..]))).unwrap();
    let node = |id: &str, link: &str| Node { id: id.into(), kind: NodeKind::Terminal, name: id.into(), text: String::new(), links: vec![link.into()], maestro: true, hidden: false };
    *bridge.shared.lock().unwrap() = Shared {
        nodes: vec![node("eu", "outro"), node("outro", "eu")],
        terminals: [("outro".to_string(), session.handle.clone())].into(),
        asking: Vec::new(),
    };
    let request = |cmd: Value| request_to(&bridge.address, &bridge.token, "eu", cmd, std::time::Duration::from_secs(20));
    let answer = request(json!({ "cmd": "ask", "name": "outro", "text": "echo pong-$((40+2))", "timeout": 20 })).unwrap();
    assert!(answer.contains("pong-42"), "{answer}");
    // the rope between them was lit only while the ask ran
    assert!(bridge.shared.lock().unwrap().asking.is_empty());
    assert!(request(json!({ "cmd": "check", "name": "outro" })).unwrap().contains("pong-42"));
    assert!(request_to(&bridge.address, "errado", "eu", json!({ "cmd": "list" }), std::time::Duration::from_secs(5)).is_err());
}

#[test]
fn a_child_agent_builds_nothing_but_still_notifies() {
    let node = |id: &str, maestro: bool| Node { id: id.into(), kind: NodeKind::Terminal, name: id.into(), text: String::new(), links: vec![], maestro, hidden: false };
    let shared = std::sync::Mutex::new(Shared { nodes: vec![node("pai", true), node("filho", false)], ..Default::default() });
    let (sender, _commands) = std::sync::mpsc::channel();
    let request = |cmd: Value| handle(&cmd, &shared, &sender);
    let refused = "Só um terminal Maestro pode criar terminais, portais ou conexões.";
    assert_eq!(request(json!({ "cmd": "recruit", "name": "Neto", "from": "filho" })).unwrap_err(), refused);
    assert_eq!(request(json!({ "cmd": "portal_create", "text": "exemplo.com", "from": "filho" })).unwrap_err(), refused);
    assert_eq!(request(json!({ "cmd": "connect", "name": "pai", "to": "filho", "from": "filho" })).unwrap_err(), refused);
    assert!(request(json!({ "cmd": "notify", "text": "oi", "from": "filho" })).is_ok());
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
    assert!(asks_for_you(" Bash command\n Do you want to proceed?\n ❯ 1. Yes\n   2. Yes, and don't ask again\n   3. No, and tell Claude what to do differently (esc)"));
    assert!(asks_for_you(" Quick safety check: trust this folder?\n ❯ No, exit\n   Yes, I trust this folder\n Enter to confirm · Esc to cancel"));
    // an answer that ends in a question, with the typing box below, isn't waiting
    assert!(!asks_for_you("⏺ Two ways:\n  1. Fast\n  2. Safe\n  Would you like me to do it?\n╭──────╮\n│ >    │\n╰──────╯\n  ? for shortcuts"));
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
fn a_script_clicks_over_two_frames_then_types_and_presses_keys() {
    let ctx = Context::default();
    let mut script = Script::new("click 10 20; type olá; key J cmd");
    let mut frame = || {
        let mut raw = egui::RawInput::default();
        script.feed(&ctx, &mut raw);
        raw.events
    };
    assert!(matches!(frame()[1], egui::Event::PointerButton { pressed: true, .. }));
    assert!(matches!(frame()[1], egui::Event::PointerButton { pressed: false, .. }));
    assert!(matches!(&frame()[0], egui::Event::Text(t) if t == "olá"));
    assert!(matches!(frame()[0], egui::Event::Key { key: Key::J, pressed: true, modifiers, .. } if modifiers.command));
}

#[test]
fn the_chat_reads_claude_code_files() {
    use crate::transcript::{ChatItem, Session, Transcript, assign, chat_items, project_folder};
    assert_eq!(project_folder("/Users/eu/.claude/my_proj"), "-Users-eu--claude-my-proj");
    let line = |v: Value| chat_items(&v);
    assert_eq!(line(json!({"type": "user", "message": {"content": "oi"}})), vec![ChatItem::User("oi".into())]);
    let reply = json!({"type": "assistant", "message": {"content": [
        {"type": "thinking", "thinking": "x"},
        {"type": "text", "text": "Feito."},
        {"type": "tool_use", "name": "Bash", "input": {"command": "ls -la\npwd", "description": "lista"}},
    ]}});
    assert_eq!(line(reply), vec![ChatItem::Agent("Feito.".into()), ChatItem::Tool("Bash".into(), "ls -la".into())]);
    // tool results, side conversations and bookkeeping lines stay out
    assert!(line(json!({"type": "user", "message": {"content": [{"type": "tool_result", "content": "ok"}]}})).is_empty());
    assert!(line(json!({"type": "assistant", "isSidechain": true, "message": {"content": "x"}})).is_empty());
    assert!(line(json!({"type": "api-request", "message": {"content": "x"}})).is_empty());

    let dir = std::env::temp_dir().join(format!("mago-chat-{}", std::process::id()));
    let sessions = dir.join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let write = |name: &str, v: Value| std::fs::write(sessions.join(name), v.to_string()).unwrap();
    write("1.json", json!({"sessionId": "velha", "cwd": "/p", "startedAt": 1000, "status": "idle"}));
    write("2.json", json!({"sessionId": "nova", "cwd": "/p", "startedAt": 9000, "status": "busy", "entrypoint": "cli"}));
    write("3.json", json!({"sessionId": "outra", "cwd": "/q", "startedAt": 9500}));
    // the desktop app's session in the same folder is not in any terminal
    write("4.json", json!({"sessionId": "mesa", "cwd": "/p", "startedAt": 9999, "entrypoint": "claude-desktop"}));
    let found = crate::transcript::sessions(&sessions);
    assert_eq!(found.len(), 3);
    let session = |id: &str, cwd: &str, started| Session { id: id.into(), cwd: cwd.into(), started };
    // two Claude terminals in one folder: the newer takes the newer session, the older the other
    let terminals = [("velho".to_string(), "/p".to_string(), 500), ("novo".to_string(), "/p".to_string(), 8000)];
    let mut pairs = assign(&terminals, &found);
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(pairs, vec![("novo".into(), session("nova", "/p", 9000)), ("velho".into(), session("velha", "/p", 1000))]);
    // nothing started in that folder since the terminal opened: no conversation
    assert!(assign(&[("t".to_string(), "/p".to_string(), 20000)], &found).is_empty());
    // the terminal's folder as typed, the session's as Claude Code resolves it (/tmp → /private/tmp)
    let typed = std::env::temp_dir().to_str().unwrap().to_string();
    let resolved = std::env::temp_dir().canonicalize().unwrap().to_str().unwrap().to_string();
    let real = assign(&[("t".to_string(), typed, 0)], &[session("real", &resolved, 1)]);
    assert_eq!(real.len(), 1);

    // read from where it stopped; an unfinished line waits for the rest, even cut inside a character
    let file = dir.join("t.jsonl");
    let head = b"{\"type\":\"ai-title\",\"aiTitle\":\"Plano\"}\n{\"type\":\"user\",\"message\":{\"content\":\"um\"}}\n{\"type\":\"user\",\"message\":{\"content\":\"a";
    std::fs::write(&file, [head.as_slice(), &"ç".as_bytes()[..1]].concat()).unwrap();
    let mut transcript = Transcript::new(file.clone());
    transcript.poll();
    assert_eq!((transcript.title.as_deref(), transcript.items.len()), (Some("Plano"), 1));
    let mut more = std::fs::OpenOptions::new().append(true).open(&file).unwrap();
    std::io::Write::write_all(&mut more, &[&"ç".as_bytes()[1..], "ão\"}}\n".as_bytes()].concat()).unwrap();
    transcript.poll();
    assert_eq!(transcript.items.last(), Some(&ChatItem::User("ação".into())));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn portal_tabs() {
    // one page shows at a time, but the data keeps every tab (and the bar's hidden state)
    let mut portal = PortalSpec { name: "Docs".into(), url: "https://a.dev".into(), tabs: vec!["https://a.dev".into(), "https://b.dev".into()], active: 1, chrome_hidden: true };
    assert_eq!((portal.tab_urls(), portal.active_tab()), (vec!["https://a.dev".to_string(), "https://b.dev".into()], 1));
    portal.set_tab_url(1, "https://b.dev/x".into());
    assert_eq!((portal.active_tab(), portal.url.as_str()), (1, "https://b.dev/x"));
    let mut e = DrawingApp::default().new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 9.0, height: 9.0 });
    e.portal = Some(portal.clone());
    assert_eq!(round_trip(&[e])[0].portal.as_ref(), Some(&portal));
}

#[test]
fn text_box_follows_its_glyphs() {
    // narrow letters take less room than wide ones: the box is the painted width, not a char count
    let width = |value: &str| {
        let e = DrawingApp::default().new_element(ElementKind::Text { x: 0.0, y: 0.0, value: value.into() });
        local_bounds(&e).width()
    };
    assert!(width("iiiiiiiiii") < width("mmmmmmmmmm") * 0.6, "{} vs {}", width("iiiiiiiiii"), width("mmmmmmmmmm"));
    assert_eq!(text_advance("", 20.0), 0.0);
}
