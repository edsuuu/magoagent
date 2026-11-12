use super::*;

#[test]
fn grouping_and_ungrouping_keeps_elements_together() {
    let mut app = DrawingApp::default();
    app.scene.elements = vec![rect_element(0.0, 0.0, 10.0, 10.0, None), rect_element(20.0, 0.0, 10.0, 10.0, None)];
    app.selected = vec![0, 1];
    app.group_selection();
    assert!(app.scene.elements[0].frame_group.is_some() && app.scene.elements[0].frame_group == app.scene.elements[1].frame_group);
    assert!(app.scene.elements.iter().all(|e| e.group_id.is_none()), "the note binding is another field");
    app.ungroup_selection();
    assert!(app.scene.elements.iter().all(|e| e.frame_group.is_none()) && app.scene.groups.is_empty());
}

#[test]
fn the_menu_brings_to_front_sends_to_back_selects_all_and_hides_a_portals_bar() {
    use crate::app::ui::context::Target;
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.scene.elements = (0..3).map(|i| rect_element(i as f32 * 10.0, 0.0, 5.0, 5.0, None)).collect();
    app.scene.elements[0].portal = Some(PortalSpec::default());
    let first = app.scene.elements[0].id.clone();
    app.selected = vec![0];
    app.context_action(&ctx, &Target::Element(first.clone()), "front");
    assert_eq!(app.scene.elements[2].id, first);
    app.context_action(&ctx, &Target::Element(first.clone()), "back");
    assert_eq!(app.scene.elements[0].id, first);
    app.context_action(&ctx, &Target::Element(first.clone()), "chrome");
    assert!(app.scene.elements[0].portal.as_ref().unwrap().chrome_hidden);
    app.context_action(&ctx, &Target::Canvas(Pos2::ZERO), "select_all");
    assert_eq!(app.selected.len(), 3);
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
    assert_eq!(badges, ["V", "", "", "", "", "N", "T", "R", "D", "O", "A", "L", "P", "E", "F", "Shift+X", "K", "B", ""]);
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
    let mut node = rect_element(3000.0, 2000.0, 100.0, 100.0, None);
    node.terminal = Some(TerminalSpec::default());
    app.scene.elements = vec![node];
    let world = app.minimap_world();
    assert!(world.contains_rect(element_bounds(&app.scene.elements[0])));
    assert!(world.contains(Pos2::ZERO), "the visible area is part of the map");
    app.zoom = 2.0;
    app.pan_to(Pos2::new(3050.0, 2050.0));
    assert_eq!(app.to_screen(Pos2::new(3050.0, 2050.0)), app.canvas_rect.center());
}
