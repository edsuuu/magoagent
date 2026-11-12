//! The enlarged tree's way out, and "Vincular": roping a tree to a terminal or a chat.

use super::filetree::{temp, tree_frame, tree_on};
use super::*;

#[test]
fn a_press_on_the_bare_canvas_puts_an_enlarged_tree_back() {
    let ctx = Context::default();
    let dir = temp("leave");
    let mut app = DrawingApp::default();
    tree_frame(&mut app, &ctx, 0.0, vec![]);
    let id = tree_on(&mut app, &dir);
    app.set_elevated(Some(id.clone()));
    tree_frame(&mut app, &ctx, 1.0, vec![]);
    assert_eq!(app.terminal_elevated.as_ref(), Some(&id));
    let margin = Pos2::new(3.0, 3.0);
    tree_frame(&mut app, &ctx, 2.0, vec![egui::Event::PointerMoved(margin), button(margin, true)]);
    assert!(app.terminal_elevated.is_none(), "the margin goes back to the canvas");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn vincular_ropes_a_tree_to_an_agent_and_again_cuts_it() {
    let ctx = Context::default();
    let dir = temp("link");
    let mut app = DrawingApp::default();
    tree_frame(&mut app, &ctx, 0.0, vec![]);
    let tree = tree_on(&mut app, &dir);
    assert!(app.link_rows(&tree).is_empty(), "no terminal or chat to pick from");
    let mut shell = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 300.0, height: 200.0 });
    shell.terminal = Some(TerminalSpec { name: "Dev".into(), ..Default::default() });
    let dev = shell.id.clone();
    app.scene.elements.push(shell);
    assert_eq!(app.link_rows(&tree).len(), 1);
    app.context_action(&ctx, &crate::app::ui::context::Target::Element(tree.clone()), &format!("link:{dev}"));
    assert!(app.links().get(&tree).is_some_and(|l| l.contains(&dev)), "roped");
    app.context_action(&ctx, &crate::app::ui::context::Target::Element(tree.clone()), &format!("link:{dev}"));
    assert!(app.links().get(&tree).is_none_or(|l| !l.contains(&dev)), "cut by the second pick");
    let _ = fs::remove_dir_all(&dir);
}
