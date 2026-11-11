//! The changes of a file against the last commit in a file tree node's editor: the ranges the gutter marks, the rollback of
//! one, the tab that compares side by side and the agent's `diff`, `hunks` and `revert_hunk`.

use super::filetree::{git, temp, tree_frame, tree_on};
use super::*;
use crate::app::filetree_compare::Compare;
use crate::app::filetree_diff::{Change, Hunk, hunks, rollback};
use crate::app::filetree_editor::Body;
use std::path::Path;

#[test]
fn ranges_are_added_modified_or_deleted_and_a_rollback_puts_one_back() {
    let head = "a\nb\nc\nd\ne\nf\n";
    // "b" changed, "d" gone, "x" and "y" new after "f"
    let text = "a\nB\nc\ne\nf\nx\ny\n";
    let list = hunks(head, text);
    assert_eq!(list, vec![Hunk { old: 1..2, new: 1..2 }, Hunk { old: 3..4, new: 3..3 }, Hunk { old: 6..6, new: 5..7 }]);
    assert_eq!(list.iter().map(Hunk::change).collect::<Vec<_>>(), [Change::Modified, Change::Deleted, Change::Added]);
    assert!(hunks(head, head).is_empty());
    assert_eq!(rollback(text, head, &list[0]), "a\nb\nc\ne\nf\nx\ny\n");
    assert_eq!(rollback(text, head, &list[1]), "a\nB\nc\nd\ne\nf\nx\ny\n");
    assert_eq!(rollback(text, head, &list[2]), "a\nB\nc\ne\nf\n");
    // all three, the last one first so the earlier keep their numbers
    let all = list.iter().rev().fold(text.to_string(), |t, h| rollback(&t, head, h));
    assert_eq!(all, head);
    // a last line without its line end
    let (head, text) = ("a\nb", "a\nb\nc");
    assert_eq!(
        rollback(text, head, &hunks(head, text)[0]),
        "a
b"
    );
}

#[test]
fn the_compare_tab_pairs_rows_and_walks_the_changes() {
    let mut c = Compare::new(Path::new("x.rs"), "a\nb\nc\nd\n".into(), "a\nB\nc\nd\nz\n".into());
    assert_eq!(c.count(), 2);
    assert_eq!(c.cur, 0);
    c.go(1);
    c.go(1);
    assert_eq!(c.cur, 1, "the last change stops the walk");
    c.go(-5);
    assert_eq!(c.cur, 0);
    c.set_unified(true);
    assert_eq!(c.count(), 2, "unified has the same changes");
    c.set_new("a\nb\nc\nd\n".into());
    assert_eq!(c.count(), 0);
}

fn repo(name: &str) -> PathBuf {
    let dir = temp(name);
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "t"]);
    fs::write(dir.join("a.rs"), "fn um() {}\nfn dois() {}\nfn tres() {}\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "primeiro"]);
    dir
}

#[test]
fn an_agent_lists_the_ranges_opens_the_diff_and_rolls_one_back() {
    if crate::ide::command("git").arg("--version").output().is_err() {
        return;
    }
    let dir = repo("diff-mcp");
    let a = dir.join("a.rs");
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    // nothing changed: nothing to compare
    assert!(app.act_file_tree(&json!({ "action": "diff", "path": "a.rs" })).is_err());
    fs::write(&a, "fn um() {}\nfn DOIS() {}\nfn tres() {}\nfn quatro() {}\n").unwrap();
    let said = app.act_file_tree(&json!({ "action": "hunks", "path": "a.rs" })).unwrap();
    assert_eq!(said, "a.rs: 2 mudança(s) contra o HEAD\n1. modificado (linhas 2 a 2, antes 2 a 2)\n2. adicionado (linhas 4 a 4)\n");
    assert!(app.filetrees.editors.get(&tree).is_none_or(|ed| ed.tabs.is_empty()), "listing opens nothing");
    // the tab, side by side
    let said = app.act_file_tree(&json!({ "action": "diff", "path": "a.rs" })).unwrap();
    assert!(said.contains("2 mudança(s)"), "{said}");
    let ed = &app.filetrees.editors[&tree];
    let tab = ed.tabs.iter().find(|t| ed.active.as_ref() == Some(&t.path)).unwrap();
    assert_eq!(tab.path.file_name().unwrap().to_string_lossy(), "HEAD ↔ a.rs");
    assert!(matches!(&tab.body, Body::Compare(c) if c.count() == 2 && !c.unified) && app.ft_spec(&tree).unwrap().editor);
    // the rollback of the first range: the file on disk is written, the second stays
    assert!(app.act_file_tree(&json!({ "action": "revert_hunk", "path": "a.rs" })).is_err());
    assert!(app.act_file_tree(&json!({ "action": "revert_hunk", "path": "a.rs", "index": 3 })).is_err());
    app.act_file_tree(&json!({ "action": "revert_hunk", "path": "a.rs", "index": 1 })).unwrap();
    assert_eq!(fs::read_to_string(&a).unwrap(), "fn um() {}\nfn dois() {}\nfn tres() {}\nfn quatro() {}\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_editor_marks_the_text_against_the_commit_follows_edits_and_reverts_from_the_popup() {
    if crate::ide::command("git").arg("--version").output().is_err() {
        return;
    }
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let dir = repo("diff-gutter");
    let a = dir.join("a.rs");
    fs::write(&a, "fn um() {}\nfn DOIS() {}\nfn tres() {}\n").unwrap();
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    app.ft_open(&tree, a.clone(), true);
    for i in 0..5 {
        tree_frame(&mut app, &ctx, 100.0 + i as f64 * 0.02, vec![]);
    }
    let marks = |app: &DrawingApp| match &app.filetrees.editors[&tree].tabs[0].body {
        Body::Text(t) => t.hunks.as_ref().map(|h| h.1.clone()),
        _ => None,
    };
    assert_eq!(marks(&app), Some(vec![Hunk { old: 1..2, new: 1..2 }]));
    // an edit moves the ranges on the next frame
    if let Body::Text(t) = &mut app.filetrees.editors.get_mut(&tree).unwrap().tabs[0].body {
        t.text.push_str("fn novo() {}\n");
        t.dirty = true;
    }
    for i in 0..3 {
        tree_frame(&mut app, &ctx, 101.0 + i as f64 * 0.02, vec![]);
    }
    assert_eq!(marks(&app).map(|h| h.len()), Some(2));
    // a file git does not know has no markers
    let b = dir.join("b.rs");
    fs::write(&b, "novo\n").unwrap();
    app.ft_open(&tree, b, true);
    for i in 0..3 {
        tree_frame(&mut app, &ctx, 102.0 + i as f64 * 0.02, vec![]);
    }
    assert!(matches!(&app.filetrees.editors[&tree].tabs[1].body, Body::Text(t) if t.head.is_none() && t.hunks.is_none()));
    // Ctrl+D in the code opens the same tab
    let ed = app.filetrees.editors.get_mut(&tree).unwrap();
    ed.active = Some(a.clone());
    if let Body::Text(t) = &mut ed.tabs[0].body {
        t.focus = true;
    }
    tree_frame(&mut app, &ctx, 102.3, vec![]);
    let ctrl_d = egui::Event::Key { key: Key::D, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    tree_frame(&mut app, &ctx, 102.5, vec![ctrl_d]);
    tree_frame(&mut app, &ctx, 102.6, vec![]);
    assert!(app.filetrees.editors[&tree].tabs.iter().any(|t| matches!(t.body, Body::Compare(_))), "Ctrl+D opened the compare tab");
    app.ft_pane_apply(&tree, crate::app::filetree_pane::PaneAct::Close(a.with_file_name("HEAD ↔ a.rs")));
    // Mostrar Diff over the unsaved text, which the tab then follows
    assert_eq!(app.ft_show_diff(&tree, &a), Ok(2));
    let ed = app.filetrees.editors.get_mut(&tree).unwrap();
    if let Body::Text(t) = &mut ed.tabs[0].body {
        t.text = "fn um() {}\nfn dois() {}\nfn tres() {}\n".into();
    }
    ed.sync_compares();
    let n = ed.tabs.iter().find_map(|t| if let Body::Compare(c) = &t.body { Some(c.count()) } else { None });
    assert_eq!(n, Some(0), "back to the commit's text: no changes left");
    let _ = fs::remove_dir_all(&dir);
}
