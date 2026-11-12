//! The annotation column: the author above a declaration turns it on and off, and git says who wrote each line.

use super::filetree::{git, temp, tree_frame, tree_on};
use super::*;
use crate::app::filetree::pane::PaneAct;

#[test]
fn the_author_turns_the_blame_column_on_and_off_and_git_fills_it() {
    if crate::ide::command("git").arg("--version").output().is_err() {
        return;
    }
    let ctx = Context::default();
    let dir = temp("annotate");
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Maria"]);
    fs::write(dir.join("a.rs"), "fn um() {}\nfn dois() {}\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "first"]);
    let mut app = DrawingApp::default();
    tree_frame(&mut app, &ctx, 0.0, vec![]);
    let id = tree_on(&mut app, &dir);
    let file = dir.join("a.rs");
    let shown = |app: &DrawingApp| match &app.filetrees.editors[&id].tabs[0].body {
        crate::app::filetree::editor::Body::Text(t) => t.annotate,
        _ => panic!("not a text"),
    };
    app.act_file_tree(&json!({ "action": "open", "path": file.to_string_lossy() })).unwrap();
    assert!(!shown(&app), "off until asked");
    // pressing the author: on; again: off (the pane's act, as the click sends it)
    app.ft_pane_apply(&id, PaneAct::Annotate(file.clone()));
    assert!(shown(&app));
    app.ft_pane_apply(&id, PaneAct::Annotate(file.clone()));
    assert!(!shown(&app));
    assert!(app.act_file_tree(&json!({ "action": "annotate", "path": file.to_string_lossy() })).unwrap().contains("ligada"));
    assert!(shown(&app));
    // git's answer (the column is drawn meanwhile, and once the lines are there) comes on a thread: the lines carry a date and the author
    for i in 0..100 {
        tree_frame(&mut app, &ctx, 1.0 + i as f64 * 0.1, vec![]);
        if app.filetrees.editors[&id].code.annotation(&file).is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let note = app.filetrees.editors[&id].code.annotation(&file).expect("blame arrived").clone();
    assert_eq!(note.lines.len(), 2);
    let line = note.lines[0].clone().unwrap();
    let (day, who) = line.split_once("  ").unwrap();
    assert!(who == "Maria" && day.len() == 10 && day.as_bytes()[2] == b'/' && day.as_bytes()[5] == b'/', "{line}");
    tree_frame(&mut app, &ctx, 20.0, vec![]);
    assert!(app.act_file_tree(&json!({ "action": "annotate", "path": file.to_string_lossy(), "show": false })).unwrap().contains("desligada"));
    let _ = fs::remove_dir_all(&dir);
}
