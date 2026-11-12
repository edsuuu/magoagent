//! The canvas group ("Grupo"): the frame, its pill, and what it leaves alone (notes, piles).

use super::*;
use crate::app::ui::context::Target;

fn two_boxes_grouped(app: &mut DrawingApp) -> String {
    let a = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 100.0, height: 100.0 });
    let b = app.new_element(ElementKind::Rectangle { x: 300.0, y: 100.0, width: 100.0, height: 100.0 });
    app.scene.elements = vec![a, b];
    app.selected = vec![0, 1];
    app.group_selection();
    app.rename = None;
    app.scene.groups[0].id.clone()
}

fn pill_center(app: &DrawingApp, gid: &str) -> Pos2 {
    app.to_screen(app.group_frame(gid).unwrap().min) + Vec2::new(20.0, -17.0)
}

#[test]
fn grouping_opens_the_name_field_and_frames_the_members() {
    let mut app = DrawingApp::default();
    let a = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 100.0, height: 100.0 });
    let b = app.new_element(ElementKind::Rectangle { x: 300.0, y: 100.0, width: 100.0, height: 100.0 });
    app.scene.elements = vec![a, b];
    app.selected = vec![0, 1];
    app.group_selection();
    let gid = app.scene.groups[0].id.clone();
    assert_eq!(app.rename.as_ref().map(|r| r.0.clone()), Some(format!("group:{gid}")));
    assert_eq!(app.group_frame(&gid), Some(Rect::from_min_max(Pos2::new(84.0, 84.0), Pos2::new(416.0, 216.0))));
    // an empty name keeps the default
    app.set_name(&format!("group:{gid}"), "  ");
    assert_eq!(app.scene.groups[0].name, "Grupo");
    app.set_name(&format!("group:{gid}"), "Fluxo");
    assert_eq!(app.scene.groups[0].name, "Fluxo");
    // a lone survivor has no frame
    app.scene.elements.pop();
    assert_eq!(app.group_frame(&gid), None);
}

#[test]
fn grouping_notes_keeps_each_bound_to_its_text_and_piles_work() {
    let mut app = DrawingApp::default();
    let [n1, l1] = app.new_note(Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::splat(160.0)), "um".into());
    let [n2, l2] = app.new_note(Rect::from_min_size(Pos2::new(400.0, 100.0), Vec2::splat(160.0)), "dois".into());
    app.scene.elements = vec![n1, l1, n2, l2];
    let before: Vec<_> = app.scene.elements.iter().map(|e| e.group_id.clone()).collect();
    app.selected = vec![0, 2];
    app.group_selection();
    assert_eq!(app.scene.elements.iter().map(|e| e.group_id.clone()).collect::<Vec<_>>(), before, "the note binding is untouched");
    assert_ne!(before[0], before[2]);
    assert!(app.scene.elements.iter().all(|e| e.frame_group == app.scene.elements[0].frame_group && e.frame_group.is_some()));
    // the frame still stacks them
    app.selected = vec![0, 2];
    assert!(app.stack_notes());
    assert_eq!(app.scene.elements[0].group_id, app.scene.elements[2].group_id);
    assert_eq!(note_label(&app.scene.elements, 0), Some(1));
    assert_eq!(note_label(&app.scene.elements, 2), Some(3));
}

#[test]
fn dragging_a_member_moves_only_it_and_the_pill_moves_all() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let gid = two_boxes_grouped(&mut app);
    let origin = |app: &DrawingApp, i: usize| element_bounds(&app.scene.elements[i]).min;
    app.selected = vec![0];
    drag(&mut app, &ctx, Pos2::new(150.0, 150.0), Pos2::new(150.0, 250.0));
    assert_eq!(origin(&app, 0), Pos2::new(100.0, 200.0));
    assert_eq!(origin(&app, 1), Pos2::new(300.0, 100.0), "the other member stays");
    let at = pill_center(&app, &gid);
    assert_eq!(app.group_pill_at(at), Some(gid.clone()));
    drag(&mut app, &ctx, at, at + Vec2::new(0.0, 60.0));
    assert_eq!(origin(&app, 0), Pos2::new(100.0, 260.0));
    assert_eq!(origin(&app, 1), Pos2::new(300.0, 160.0));
}

#[test]
fn the_pill_menu_renames_colors_and_ungroups() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let gid = two_boxes_grouped(&mut app);
    let at = pill_center(&app, &gid);
    app.open_context(at, app.to_world(at));
    let menu = app.context.take().unwrap();
    assert_eq!(menu.target, Target::Group(gid.clone()));
    app.context_action(&ctx, &menu.target, "gcolor:2");
    assert_eq!(app.scene.groups[0].color, Some(NOTE_PAPERS[2].0));
    app.context_action(&ctx, &menu.target, "gcolor:none");
    assert_eq!(app.scene.groups[0].color, None);
    app.context_action(&ctx, &menu.target, "rename");
    assert_eq!(app.rename, Some((format!("group:{gid}"), "Grupo".into())));
    app.undo();
    app.context_action(&ctx, &menu.target, "ungroup");
    assert!(app.scene.groups.is_empty() && app.scene.elements.iter().all(|e| e.frame_group.is_none()));
    assert_eq!(app.group_pill_at(at), None);
}

#[test]
fn groups_survive_the_file_round_trip_and_the_merge() {
    let mut app = DrawingApp::default();
    let gid = two_boxes_grouped(&mut app);
    app.color_frame_group(&gid, Some(NOTE_PAPERS[1].0));
    let back: Scene = serde_json::from_str(&serde_json::to_string(&app.scene).unwrap()).unwrap();
    assert_eq!(back, app.scene);
    // an outside edit that doesn't touch the groups keeps the local color and name
    let base = Scene { groups: vec![FrameGroup { id: gid.clone(), name: "Grupo".into(), color: None }], ..app.scene.clone() };
    let merged = merge_scenes(&base, &app.scene, &base);
    assert_eq!(merged.groups, app.scene.groups);
    // one renamed outside wins
    let mut external = base.clone();
    external.groups[0].name = "De fora".into();
    assert_eq!(merge_scenes(&base, &app.scene, &external).groups[0].name, "De fora");
    // a group made here is not lost to an outside file that lacks it
    assert_eq!(merge_scenes(&Scene::default(), &app.scene, &Scene::default()).groups, app.scene.groups);
}

#[test]
fn the_mcp_groups_by_name_and_lists() {
    let mut app = DrawingApp::default();
    two_boxes_grouped(&mut app);
    app.act_canvas_group(&json!({ "action": "ungroup", "group": "Grupo" })).unwrap();
    let ids: Vec<String> = app.scene.elements.iter().map(|e| e.id.clone()).collect();
    app.act_canvas_group(&json!({ "action": "create", "elements": ids, "name": "Dupla" })).unwrap();
    app.act_canvas_group(&json!({ "action": "color", "group": "dupla", "color": "Verde" })).unwrap();
    assert_eq!(app.scene.groups[0].color, Some(NOTE_PAPERS[3].0));
    app.act_canvas_group(&json!({ "action": "rename", "group": "Dupla", "name": "Par" })).unwrap();
    assert!(app.act_canvas_group(&json!({ "action": "list" })).unwrap().starts_with("Par ("));
    assert!(app.act_canvas_group(&json!({ "action": "create", "elements": [ids[0].clone()] })).is_err());
    app.act_canvas_group(&json!({ "action": "ungroup", "group": "Par" })).unwrap();
    assert!(app.scene.groups.is_empty());
}

#[test]
fn copies_of_a_groups_members_form_a_new_group_and_a_lone_one_carries_none() {
    let mut app = DrawingApp::default();
    let gid = two_boxes_grouped(&mut app);
    app.rename_frame_group(&gid, "Fluxo");
    app.color_frame_group(&gid, Some(NOTE_PAPERS[1].0));
    app.selected = vec![0, 1];
    app.duplicate();
    assert_eq!(app.scene.groups.len(), 2);
    let copy = &app.scene.groups[1];
    assert_ne!(copy.id, gid);
    assert_eq!((copy.name.as_str(), copy.color), ("Fluxo", Some(NOTE_PAPERS[1].0)));
    assert!(app.scene.elements[2..].iter().all(|e| e.frame_group.as_deref() == Some(copy.id.as_str())));
    assert_eq!(app.frame_members(&gid).len(), 2, "the original keeps its own");
    // the clipboard JSON carries the group; a part leaving one member carries none
    let ctx = Context::default();
    app.selected = vec![0, 1];
    app.copy(&ctx);
    let json = ctx.output(|o| o.commands.iter().find_map(|c| if let egui::OutputCommand::CopyText(t) = c { Some(t.clone()) } else { None })).unwrap();
    app.paste(&json);
    assert_eq!(app.scene.groups.len(), 3);
    assert_eq!(app.scene.groups[2].name, "Fluxo");
    app.selected = vec![0];
    app.duplicate();
    assert_eq!(app.scene.groups.len(), 3);
    assert!(app.scene.elements.last().unwrap().frame_group.is_none());
}

#[test]
fn a_double_click_on_the_pill_opens_the_name_balloon_and_enter_saves() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let gid = two_boxes_grouped(&mut app);
    let at = pill_center(&app, &gid);
    double_click(&mut app, &ctx, at, 1.0);
    assert_eq!(app.rename.as_ref().map(|r| r.0.clone()), Some(format!("group:{gid}")));
    let run = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let mut out =
            ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), events, ..Default::default() }, |_| {
                app.rename_ui(&ctx);
            });
        out.textures_delta.clear();
    };
    run(&mut app, vec![]);
    assert!(app.rename.is_some(), "the balloon stays open");
    app.rename = Some((format!("group:{gid}"), "Novo".into()));
    run(&mut app, vec![egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    assert!(app.rename.is_none());
    assert_eq!(app.scene.groups[0].name, "Novo");
}
