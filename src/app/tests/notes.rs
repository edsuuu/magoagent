//! Notes and menus: the click that focuses a note, the bold button, the right-click menus.

use super::*;
use crate::app::notes::{format_text, rename_note_text};
use crate::app::ui::context::{Entry, Target};

fn ids(entries: &[Entry], out: &mut Vec<String>) {
    for entry in entries {
        match entry {
            Entry::Item { id, .. } => out.push(id.clone()),
            Entry::Sub { items, .. } => ids(items, out),
            Entry::Sep => {}
        }
    }
}

#[test]
fn markdown_buttons_wrap_the_selection_or_take_the_caret() {
    assert_eq!(format_text("abc", 0, 3, "**"), ("**abc**".into(), 2, 5));
    assert_eq!(format_text("abc", 1, 1, "**"), ("a****bc".into(), 3, 3));
    // the same mark around the selection again takes it off
    assert_eq!(format_text("**abc**", 2, 5, "**"), ("abc".into(), 0, 3));
    // line marks toggle on the lines the selection touches
    assert_eq!(format_text("um\ndois", 4, 4, "- "), ("um\n- dois".into(), 9, 9));
    assert_eq!(format_text("- um", 2, 2, "- "), ("um".into(), 2, 2));
    assert_eq!(format_text("# T", 0, 0, "## "), ("## T".into(), 4, 4));
    assert_eq!(rename_note_text("lista\nmais", "Compras"), "# Compras\nmais");
}

#[test]
fn a_right_click_on_a_terminal_opens_its_menu() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut shell = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 400.0, height: 300.0 });
    shell.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
    let id = shell.id.clone();
    app.scene.elements.push(shell);
    let at = app.to_screen(Pos2::new(300.0, 250.0));
    let secondary = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), secondary(true)]);
    let menu = app.context.as_ref().expect("the right click opens a menu");
    assert_eq!(menu.target, Target::Element(id.clone()));
    let mut shown = Vec::new();
    ids(&app.context_entries(&menu.target), &mut shown);
    // the reference app's order
    let order = ["rename", "edit", "unfocus", "clear", "reload", "monitor", "copy_el", "dup", "lock", "delete"];
    let at: Vec<usize> = order.iter().map(|w| shown.iter().position(|s| s == w).unwrap_or_else(|| panic!("terminal menu has {w}: {shown:?}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "terminal menu order: {shown:?}");
    // Desfocar lets the terminal go and keeps it selected
    app.terminal_focus = Some(id.clone());
    app.context_action(&ctx, &Target::Element(id.clone()), "unfocus");
    assert_eq!(app.terminal_focus, None);
}

#[test]
fn duplicate_from_the_menu_makes_a_copy() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let rect = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 50.0, height: 50.0 });
    let id = rect.id.clone();
    app.scene.elements.push(rect);
    app.selected = vec![0];
    app.context_action(&ctx, &Target::Element(id), "dup");
    assert_eq!(app.scene.elements.len(), 2);
    assert_ne!(app.scene.elements[0].id, app.scene.elements[1].id);
}

#[test]
fn a_click_on_a_note_focuses_it_and_typing_goes_in() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "# Plano".into());
    app.scene.elements.extend([note, label]);
    let at = app.to_screen(Pos2::new(150.0, 150.0));
    click(&mut app, &ctx, at);
    assert_eq!(app.editing, Some(1), "the click puts the note's text in focus");
    assert_eq!(app.selected, vec![0]);
}

#[test]
fn a_click_types_into_a_note_only_on_its_text_and_without_a_modifier() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "uma linha".into());
    let id = note.id.clone();
    app.scene.elements.extend([note, label]);
    // as painted (these frames don't paint): its text ends 80 below the note's top
    app.note_text_bottom.borrow_mut().insert(id, 140.0);
    let below = app.to_screen(Pos2::new(150.0, 250.0));
    click(&mut app, &ctx, below);
    assert_eq!((app.selected.clone(), app.editing), (vec![0], None), "below the text: only selected");
    let header = app.to_screen(Pos2::new(150.0, 66.0));
    click(&mut app, &ctx, header);
    assert_eq!(app.editing, None, "the header doesn't type");
    // under the 44 px strip that picks up a note at rest
    let on_text = app.to_screen(Pos2::new(150.0, 120.0));
    let ctrl = |pressed| egui::Event::PointerButton { pos: on_text, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::CTRL };
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(on_text), ctrl(true)]);
    frame(&mut app, &ctx, vec![ctrl(false)]);
    assert_eq!(app.editing, None, "nor a click with Ctrl");
    click(&mut app, &ctx, on_text);
    assert_eq!(app.editing, Some(1), "a plain click on the text does");
}

#[test]
fn bold_wraps_the_selected_text_of_the_note() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "abc".into());
    app.scene.elements.extend([note, label]);
    app.selected = vec![0];
    // the click on the bar has already taken the keys from the editor: what counts is the
    // selection it had while it held them
    app.editing = None;
    app.note_selection = Some((1, 0, 3));
    app.note_format(&ctx, 0, "**");
    let ElementKind::Text { value, .. } = &app.scene.elements[1].kind else { panic!("the note's label") };
    assert_eq!(value, "**abc**");
}
