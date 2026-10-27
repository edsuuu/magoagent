//! Gestures on the canvas: what the cursor promises, what a press picks up, how a drag ends.

use super::*;

fn hover(app: &mut DrawingApp, ctx: &Context, world: Pos2) -> CursorIcon {
    let at = app.to_screen(world);
    frame(app, ctx, vec![egui::Event::PointerMoved(at)])
}

/// A box at `rect`, made into a window by `make`; gives its index.
fn window(app: &mut DrawingApp, rect: Rect, make: impl FnOnce(&mut Element)) -> usize {
    let mut e = app.new_element(ElementKind::Rectangle { x: rect.min.x, y: rect.min.y, width: rect.width(), height: rect.height() });
    make(&mut e);
    app.scene.elements.push(e);
    app.scene.elements.len() - 1
}

fn shell(e: &mut Element) {
    e.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
}

fn space(pressed: bool) -> egui::Event {
    egui::Event::Key { key: Key::Space, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE }
}

#[test]
fn the_cursor_shows_move_only_where_a_press_drags() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    window(&mut app, Rect::from_min_size(Pos2::new(40.0, 150.0), Vec2::new(300.0, 200.0)), shell);
    window(&mut app, Rect::from_min_size(Pos2::new(420.0, 150.0), Vec2::new(300.0, 200.0)), |e| {
        e.ide = Some(IdeSpec { name: "proj".into(), root: "/nonexistent-magoagent".into(), ..Default::default() });
    });
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(40.0, 400.0), Vec2::splat(160.0)), "# Plano".into());
    app.scene.elements.extend([note, label]);
    window(&mut app, Rect::from_min_size(Pos2::new(420.0, 420.0), Vec2::new(120.0, 80.0)), |_| {});
    for (world, icon, what) in [
        (Pos2::new(190.0, 160.0), CursorIcon::Move, "a terminal's title strip drags it"),
        (Pos2::new(190.0, 260.0), CursorIcon::Text, "a terminal's body takes the keys"),
        (Pos2::new(570.0, 160.0), CursorIcon::Move, "an IDE's title strip drags it"),
        (Pos2::new(570.0, 260.0), CursorIcon::Default, "an IDE's card is clicked, not dragged"),
        (Pos2::new(120.0, 406.0), CursorIcon::Move, "a note's top strip drags it"),
        (Pos2::new(120.0, 480.0), CursorIcon::Text, "a note's body types"),
        (Pos2::new(421.0, 460.0), CursorIcon::Move, "a shape drags from its outline"),
        (Pos2::new(380.0, 380.0), CursorIcon::Default, "the empty canvas"),
        (Pos2::new(340.0, 250.0), CursorIcon::ResizeHorizontal, "a window's edge resizes it"),
    ] {
        assert_eq!(hover(&mut app, &ctx, world), icon, "{what}");
    }
}

#[test]
fn space_pans_unless_a_terminal_has_the_keys() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let i = window(&mut app, Rect::from_min_size(Pos2::new(40.0, 150.0), Vec2::new(300.0, 200.0)), shell);
    let (at, to) = (Pos2::new(500.0, 400.0), Pos2::new(530.0, 420.0));
    assert_eq!(frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), space(true)]), CursorIcon::Grab);
    assert_eq!(frame(&mut app, &ctx, vec![button(at, true)]), CursorIcon::Grabbing);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
    frame(&mut app, &ctx, vec![button(to, false), space(false)]);
    assert_eq!(app.pan, to - at);
    // a focused terminal types the space: the canvas keeps its plain cursor and press
    app.terminal_focus = Some(app.scene.elements[i].id.clone());
    assert_eq!(frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), space(true)]), CursorIcon::Default);
    frame(&mut app, &ctx, vec![button(at, true)]);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
    frame(&mut app, &ctx, vec![button(to, false), space(false)]);
    assert_eq!(app.pan, to - at, "no pan this time");
}

#[test]
fn esc_mid_drag_puts_it_back_and_other_keys_wait() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    window(&mut app, Rect::from_min_size(Pos2::new(100.0, 150.0), Vec2::new(300.0, 200.0)), shell);
    let before = local_bounds(&app.scene.elements[0]);
    let start = app.to_screen(Pos2::new(250.0, 160.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start + Vec2::new(80.0, 60.0))]);
    assert_ne!(local_bounds(&app.scene.elements[0]), before, "the title strip drags it");
    // Delete and Ctrl+Z would pull the scene from under the drag
    frame(&mut app, &ctx, vec![key_press(Key::Delete)]);
    assert_eq!(app.scene.elements.len(), 1, "Delete waits for the release");
    frame(&mut app, &ctx, vec![key_press(Key::Escape)]);
    assert_eq!(local_bounds(&app.scene.elements[0]), before, "Esc puts it back");
    assert!(app.undo.is_empty(), "and leaves no undo step");
    // the rest of that press moves nothing
    let end = start + Vec2::new(120.0, 90.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(local_bounds(&app.scene.elements[0]), before);
}

#[test]
fn a_press_that_trembles_is_still_a_click() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    window(&mut app, Rect::from_min_size(Pos2::new(100.0, 150.0), Vec2::new(300.0, 200.0)), shell);
    let before = local_bounds(&app.scene.elements[0]);
    let start = app.to_screen(Pos2::new(250.0, 160.0));
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start + Vec2::new(3.0, 2.0))]);
    frame(&mut app, &ctx, vec![button(start + Vec2::new(3.0, 2.0), false)]);
    assert_eq!(local_bounds(&app.scene.elements[0]), before);
    assert!(app.undo.is_empty(), "a click is no undo step");
    assert_eq!(app.selected, vec![0]);
}

#[test]
fn a_note_drags_by_its_top_strip_and_its_body_types() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 150.0), Vec2::splat(200.0)), "abc".into());
    app.scene.elements.extend([note, label]);
    let before = local_bounds(&app.scene.elements[0]);
    let body = app.to_screen(Pos2::new(160.0, 260.0));
    drag(&mut app, &ctx, body, body + Vec2::new(100.0, 0.0));
    assert_eq!(local_bounds(&app.scene.elements[0]), before, "the body does not drag");
    assert_eq!(app.editing, Some(1), "the press on the body put its text in focus");
    app.finish_text();
    let strip = app.to_screen(Pos2::new(160.0, 156.0));
    drag(&mut app, &ctx, strip, strip + Vec2::new(100.0, 40.0));
    assert_eq!(local_bounds(&app.scene.elements[0]), before.translate(Vec2::new(100.0, 40.0)), "the strip drags");
}

#[test]
fn a_drag_whose_release_never_came_ends_on_the_next_frame() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.panning = true;
    frame(&mut app, &ctx, vec![]);
    assert!(!app.in_gesture());
}

#[test]
fn keeping_selection_follows_the_ids() {
    let mut app = DrawingApp::default();
    for x in [0.0, 100.0, 200.0] {
        window(&mut app, Rect::from_min_size(Pos2::new(x, 0.0), Vec2::splat(50.0)), |_| {});
    }
    let last = app.scene.elements[2].id.clone();
    app.selected = vec![0, 2];
    app.keeping_selection(|app| {
        app.scene.elements.remove(0);
    });
    assert_eq!(app.selected, vec![1], "the removed one leaves, the other follows its element");
    assert_eq!(app.scene.elements[1].id, last);
}

#[test]
fn a_press_on_the_margin_of_an_enlarged_window_only_puts_it_back() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let i = window(&mut app, Rect::from_min_size(Pos2::new(300.0, 100.0), Vec2::new(300.0, 200.0)), shell);
    // a filled shape under the margin, where the press lands
    window(&mut app, Rect::from_min_size(Pos2::new(5.0, 250.0), Vec2::new(30.0, 100.0)), |e| e.style.fill = Some([200, 0, 0, 255]));
    let id = app.scene.elements[i].id.clone();
    app.set_elevated(Some(id.clone()));
    app.terminal_focus = Some(id);
    app.selected.clear();
    let margin = app.to_screen(Pos2::new(20.0, 300.0));
    click(&mut app, &ctx, margin);
    assert_eq!(app.terminal_elevated, None, "the margin puts it back");
    assert_eq!(app.terminal_focus, None);
    assert!(app.selected.is_empty(), "and the shape under the press is not picked");
    // nor the rope under it: its popover stays shut
    let top = window(&mut app, Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(40.0, 40.0)), shell);
    let bottom = window(&mut app, Rect::from_min_size(Pos2::new(0.0, 540.0), Vec2::new(40.0, 40.0)), shell);
    let (top, bottom) = (app.scene.elements[top].id.clone(), app.scene.elements[bottom].id.clone());
    app.connect(&top, &bottom);
    let rope = app.scene.elements.iter().find(|e| e.rope).unwrap();
    let near = |p: &[f32; 2]| (p[1] - 200.0).abs();
    let on_rope = crate::geometry::line_path(rope).into_iter().min_by(|a, b| near(a).total_cmp(&near(b))).unwrap();
    let on_rope = Pos2::new(on_rope[0], on_rope[1]);
    assert!(app.hit(on_rope).is_some_and(|k| app.scene.elements[k].rope), "the press lands on the rope");
    let on_rope = app.to_screen(on_rope);
    app.set_elevated(Some(app.scene.elements[i].id.clone()));
    click(&mut app, &ctx, on_rope);
    assert_eq!(app.terminal_elevated, None);
    assert!(app.links_popover.is_none());
}

#[test]
fn a_pan_press_keeps_the_enlarged_window() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let i = window(&mut app, Rect::from_min_size(Pos2::new(300.0, 100.0), Vec2::new(300.0, 200.0)), |e| {
        e.ide = Some(IdeSpec { name: "proj".into(), root: "/nonexistent-magoagent".into(), ..Default::default() });
    });
    let id = app.scene.elements[i].id.clone();
    app.set_elevated(Some(id.clone()));
    let at = app.to_screen(Pos2::new(20.0, 300.0));
    let middle = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Middle, pressed, modifiers: Modifiers::NONE };
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), middle(true)]);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at + Vec2::new(30.0, 0.0))]);
    frame(&mut app, &ctx, vec![middle(false)]);
    assert_eq!(app.terminal_elevated, Some(id.clone()), "the middle button pans, it does not put the IDE back");
    app.tool = Tool::Hand;
    click(&mut app, &ctx, at);
    assert_eq!(app.terminal_elevated, Some(id.clone()), "nor does the hand");
    // a right click on the margin still does, as the left one: the menu it opens takes the keys
    app.tool = Tool::Select;
    let right = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE };
    frame(&mut app, &ctx, vec![right(true)]);
    frame(&mut app, &ctx, vec![right(false)]);
    assert_eq!(app.terminal_elevated, None);
}

#[test]
fn the_halo_just_outside_a_terminal_picks_it_up_instead_of_typing() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let i = window(&mut app, Rect::from_min_size(Pos2::new(100.0, 150.0), Vec2::new(300.0, 200.0)), shell);
    // 6 below its bottom edge: past the resize band, inside the hit's halo
    let below = Pos2::new(250.0, 356.0);
    assert_eq!(hover(&mut app, &ctx, below), CursorIcon::Move);
    let at = app.to_screen(below);
    click(&mut app, &ctx, at);
    assert_eq!(app.terminal_focus, None, "no typing from outside the window");
    assert_eq!(app.selected, vec![i]);
}

#[test]
fn notes_and_windows_get_no_rotation_handle() {
    let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 150.0), Vec2::splat(200.0)), "abc".into());
    app.scene.elements.extend([note, label]);
    window(&mut app, Rect::from_min_size(Pos2::new(400.0, 150.0), Vec2::splat(100.0)), |_| {});
    app.selected = vec![0];
    assert_eq!(app.rotation_handle(), None, "a turned note could no longer be resized by its edges");
    app.selected = vec![2];
    assert!(app.rotation_handle().is_some(), "a shape still turns");
}

#[test]
fn esc_on_an_ide_card_lets_the_arrows_go_without_pushing_the_node() {
    let ctx = Context::default();
    let dir = std::env::temp_dir().join(format!("magoagent-esc-card-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let i = window(&mut app, Rect::from_min_size(Pos2::new(100.0, 100.0), IDE_SIZE), |e| {
        e.ide = Some(IdeSpec { name: "proj".into(), root: dir.to_string_lossy().into(), ..Default::default() });
    });
    frame(&mut app, &ctx, vec![]);
    (app.card_focus, app.selected) = (Some(app.scene.elements[i].id.clone()), vec![i]);
    let before = local_bounds(&app.scene.elements[i]);
    let key = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame(&mut app, &ctx, vec![key(Key::Escape)]);
    frame(&mut app, &ctx, vec![key(Key::ArrowRight)]);
    assert_eq!((app.card_focus.as_ref(), local_bounds(&app.scene.elements[i])), (None, before));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn tab_on_the_canvas_leaves_the_shortcuts_on() {
    use eframe::App as _;
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let tab = egui::Event::Key { key: Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    let mut raw = egui::RawInput { events: vec![tab.clone(), tab], ..Default::default() };
    app.raw_input_hook(&ctx, &mut raw);
    frame(&mut app, &ctx, raw.events);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.keyboard_taken(&ctx));
}

/// The composer's area on screen, after the frame it shows in and the one it settles in.
fn composer_areas(app: &mut DrawingApp, ctx: &Context) -> Vec<Rect> {
    for _ in 0..2 {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), ..Default::default() };
        let _ = ctx.run_ui(raw, |_| app.composer_ui(ctx));
    }
    ctx.memory(|m| m.areas().visible_layer_ids().into_iter().filter(|l| l.order == egui::Order::Foreground).filter_map(|l| m.area_rect(l.id)).collect())
}

#[test]
fn the_composer_stays_in_view_and_waits_under_the_chat() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // its terminal panned far off: it stays at the edge of the view, where its keys go
    let i = window(&mut app, Rect::from_min_size(Pos2::new(2000.0, 2000.0), Vec2::new(400.0, 300.0)), shell);
    app.composer = Some((app.scene.elements[i].id.clone(), String::new()));
    let areas = composer_areas(&mut app, &ctx);
    assert_eq!(areas.len(), 1);
    assert!(app.canvas_rect.contains_rect(areas[0]), "{:?} in {:?}", areas[0], app.canvas_rect);
    // a margin of its own, not pushed against the edge by egui
    assert!(areas[0].right() <= app.canvas_rect.right() - 6.0, "{:?}", areas[0]);
    // the chat covers the canvas: no composer over it, and it lets the keys go
    app.chat_open = true;
    assert!(composer_areas(&mut app, &ctx).is_empty());
    assert_eq!(ctx.memory(|m| m.focused()), None);
}

#[test]
fn a_right_click_on_an_ide_cards_tree_offers_what_to_do_with_its_file() {
    use crate::app::ui::context::Entry;
    let ctx = Context::default();
    let dir = std::env::temp_dir().join(format!("magoagent-card-menu-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "").unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let i = window(&mut app, Rect::from_min_size(Pos2::new(100.0, 100.0), IDE_SIZE), |e| {
        e.ide = Some(IdeSpec { name: "proj".into(), root: dir.to_string_lossy().into(), ..Default::default() });
    });
    frame(&mut app, &ctx, vec![]);
    let id = app.scene.elements[i].id.clone();
    let right_click = |app: &mut DrawingApp, at: Pos2| {
        let right = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE };
        frame(app, &ctx, vec![egui::Event::PointerMoved(at), right(true)]);
        frame(app, &ctx, vec![right(false)]);
        let items = app.context.as_ref().and_then(|m| m.items.clone()).unwrap_or_default();
        items.into_iter().filter_map(|e| if let Entry::Item { id, label, enabled, .. } = e { Some((id, label, enabled)) } else { None }).collect::<Vec<_>>()
    };
    let on_file = card_row_at(&app, i, 0);
    let menu = right_click(&mut app, on_file);
    assert_eq!(menu[0].1, "Novo Arquivo");
    assert!(menu[0].0.ends_with("a.txt"), "{}", menu[0].0);
    assert!(menu.iter().any(|r| r.1 == "Mostrar na pasta"), "then the card's own rows");
    // Renomear asks for the name, and the card's tree lets the keys go to that
    let rename = menu.iter().find(|r| r.1 == "Renomear").unwrap().0.clone();
    let target = app.context.take().unwrap().target;
    app.card_focus = Some(id.clone());
    app.context_action(&ctx, &target, &rename);
    assert!(matches!(&app.ides[&id].file_ask, Some(crate::ide::FileAsk::Name { folder: None, text, .. }) if text == "a.txt"));
    assert_eq!(app.card_focus, None);
    // below the rows: the project's own folder, which is not renamed or removed from here
    app.ides.get_mut(&id).unwrap().file_ask = None;
    let below = card_row_at(&app, i, 5);
    let menu = right_click(&mut app, below);
    assert!(menu.iter().any(|r| r.1 == "Renomear" && !r.2));
    assert!(menu.iter().any(|r| r.1 == "Novo Arquivo" && r.2));
    let _ = fs::remove_dir_all(&dir);
}
