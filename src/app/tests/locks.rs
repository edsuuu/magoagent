//! Locking: the pointer skips locked elements, the toolbar lock, the unlock button.

use super::*;

#[test]
fn locked_elements_ignore_the_pointer() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    frame(&mut app, &ctx, vec![]);
    app.selected = vec![0];
    app.toggle_lock_selected();
    assert!(app.scene.elements[0].locked && app.selected.is_empty());
    // clicking its outline or a marquee around it no longer selects it
    drag(&mut app, &ctx, Pos2::new(300.0, 250.0), Pos2::new(300.0, 250.0));
    assert!(app.selected.is_empty());
    drag(&mut app, &ctx, Pos2::new(280.0, 180.0), Pos2::new(720.0, 320.0));
    assert_eq!(app.selected, vec![1]);
    app.select_all();
    assert_eq!(app.selected, vec![1]);
    app.lock_all();
    assert!(app.scene.elements.iter().all(|e| e.locked));
    let id = app.scene.elements[1].id.clone();
    app.unlock(Some(&id));
    assert!(app.scene.elements[0].locked && !app.scene.elements[1].locked);
    app.unlock(None);
    assert!(app.scene.elements.iter().all(|e| !e.locked));
    assert_eq!(app.describe(&app.scene.elements[0]), "Retângulo");
}

#[test]
fn locking_the_selection_takes_its_group_and_card() {
    let mut app = DrawingApp::default();
    let mut a = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 50.0, height: 50.0 });
    let mut b = app.new_element(ElementKind::Rectangle { x: 100.0, y: 0.0, width: 50.0, height: 50.0 });
    (a.group_id, b.group_id) = (Some("g".into()), Some("g".into()));
    let mut card = app.new_element(ElementKind::Rectangle { x: 0.0, y: 200.0, width: 300.0, height: 200.0 });
    card.frame = Some("Frame".into());
    let inside = app.new_element(ElementKind::Rectangle { x: 20.0, y: 220.0, width: 40.0, height: 40.0 });
    let free = app.new_element(ElementKind::Rectangle { x: 500.0, y: 0.0, width: 50.0, height: 50.0 });
    app.scene.elements = vec![a, b, card, inside, free];
    // the whole group and the card's contents go with it
    app.selected = vec![0, 2];
    app.toggle_lock_selected();
    assert!(!app.tool_locked, "the tool lock is left alone");
    assert_eq!(app.scene.elements.iter().map(|e| e.locked).collect::<Vec<_>>(), [true, true, true, true, false]);
    assert!(app.selected.is_empty());
}

#[test]
fn locked_group_shows_one_unlock_button() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut a = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 100.0, height: 100.0 });
    let mut b = app.new_element(ElementKind::Rectangle { x: 300.0, y: 100.0, width: 100.0, height: 100.0 });
    (a.group_id, b.group_id) = (Some("g".into()), Some("g".into()));
    let mut card = app.new_element(ElementKind::Rectangle { x: 100.0, y: 400.0, width: 300.0, height: 200.0 });
    card.frame = Some("Frame".into());
    let inside = app.new_element(ElementKind::Rectangle { x: 150.0, y: 450.0, width: 50.0, height: 50.0 });
    app.scene.elements = vec![a, b, card, inside];
    // locking one member of a group locks the group; locking a card locks what's in it
    app.selected = vec![0];
    app.toggle_lock_selected();
    app.selected = vec![2];
    app.toggle_lock_selected();
    assert!(app.scene.elements.iter().all(|e| e.locked));
    // clicking a locked element offers the unlock button for its whole unit
    app.tool = Tool::Select;
    drag(&mut app, &ctx, Pos2::new(100.0, 150.0), Pos2::new(100.0, 150.0));
    assert_eq!(app.active_locked.as_ref().map(Vec::len), Some(2));
    drag(&mut app, &ctx, Pos2::new(600.0, 300.0), Pos2::new(600.0, 300.0));
    assert_eq!(app.active_locked, None);
    // anywhere in a locked card, even on what's inside it, means the whole card
    for p in [Pos2::new(100.0, 500.0), Pos2::new(300.0, 550.0), Pos2::new(175.0, 475.0)] {
        drag(&mut app, &ctx, p, p);
        assert_eq!(app.active_locked.as_ref().map(Vec::len), Some(2));
    }
    // the unlock button frees the whole unit at once
    app.selected = app.lock_unit(&[2]);
    app.toggle_lock_selected();
    assert!(!app.scene.elements[2].locked && !app.scene.elements[3].locked && app.scene.elements[0].locked);
}
