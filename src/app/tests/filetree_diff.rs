//! The changes of a file against the last commit in a file tree node's editor: the ranges the gutter marks, the rollback of
//! one, the tab that compares side by side and the agent's `diff`, `hunks` and `revert_hunk`.

use super::filetree::{git, temp, tree_frame, tree_on};
use super::*;
use crate::app::filetree::compare::Compare;
use crate::app::filetree::diff::{Change, Hunk, hunks, rollback};
use crate::app::filetree::editor::Body;
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
    if crate::editor::command("git").arg("--version").output().is_err() {
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
    // the tab's » on the change left (now the first): the file goes back to the HEAD and the tab follows
    app.act_file_tree(&json!({ "action": "diff", "path": "a.rs" })).unwrap();
    app.ft_revert_change(&tree, &a, 0).unwrap();
    assert_eq!(fs::read_to_string(&a).unwrap(), "fn um() {}\nfn dois() {}\nfn tres() {}\n");
    assert!(app.filetrees.editors[&tree].tabs.iter().any(|t| matches!(&t.body, Body::Compare(c) if c.count() == 0)), "the tab follows");
    assert!(app.ft_revert_change(&tree, &a, 0).is_err(), "nothing left to revert");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_editor_marks_the_text_against_the_commit_follows_edits_and_reverts_from_the_popup() {
    if crate::editor::command("git").arg("--version").output().is_err() {
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
    app.ft_pane_apply(&tree, crate::app::filetree::pane::PaneAct::Close(a.with_file_name("HEAD ↔ a.rs")));
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

#[test]
fn a_line_of_the_annotation_column_tells_its_commit_and_the_files_it_changed() {
    if crate::editor::command("git").arg("--version").output().is_err() {
        return;
    }
    let dir = repo("annot_commit");
    fs::create_dir(dir.join("src")).unwrap();
    fs::write(dir.join("src").join("b.rs"), "fn b() {}\n").unwrap();
    fs::write(dir.join("a.rs"), "fn um() {}\nfn dois() { 2 }\nfn tres() {}\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "segundo"]);
    let said = crate::app::filetree::annotate::line_commit(&dir.join("a.rs"), 2).unwrap();
    assert!(said.starts_with("commit ") && said.contains("Autor: t <t@example.com>") && said.contains("\n\nsegundo\n"), "{said}");
    assert!(said.contains("Arquivos (2):\nM a.rs\nA src/b.rs\n"), "{said}");
    // the first line came from the first commit, which touched only a.rs
    let first = crate::app::filetree::annotate::line_commit(&dir.join("a.rs"), 1).unwrap();
    assert!(first.contains("primeiro") && first.contains("Arquivos (1):\nA a.rs\n"), "{first}");
    // a press on the column picks a file: its whole path, for the diff of that commit
    let hash = said.lines().next().unwrap().trim_start_matches("commit ");
    let files = crate::app::filetree::vision::commit_files(&dir.join("a.rs"), hash).unwrap();
    assert!(files.iter().any(|(_, p, rel)| rel == "src/b.rs" && p.ends_with("src/b.rs") && p.is_file()), "{files:?}");
    assert!(crate::app::filetree::vision::commit_files(&dir.join("a.rs"), "nada; rm").is_err());
    let _ = fs::remove_dir_all(&dir);
}

fn numbered(count: usize, changed: &[usize]) -> String {
    (0..count).map(|line| if changed.contains(&line) { format!("mudou {line}\n") } else { format!("linha {line}\n") }).collect()
}

#[test]
fn the_unchanged_stretches_collapse_keeping_four_lines_around_each_change() {
    use crate::app::filetree::compare::{Item, Plan, visible_plan};
    use std::collections::HashSet;
    // 20 equal rows, a change, 3 equal rows: the long stretch keeps its last 4, the short one stays whole
    let keys: Vec<Option<usize>> = (0..20).map(Some).chain([None]).chain((20..23).map(Some)).collect();
    let plan = visible_plan(&keys, &HashSet::new(), true);
    assert_eq!(plan[0], Plan::Fold { key: 0, first: 0, count: 16 });
    assert_eq!(plan[1..5], [Plan::Show(16), Plan::Show(17), Plan::Show(18), Plan::Show(19)]);
    assert_eq!(plan.len(), 1 + 4 + 1 + 3, "nothing else hides");
    // opened, or the switch off, or nothing changed: everything shows
    assert_eq!(visible_plan(&keys, &HashSet::from([0]), true).len(), keys.len());
    assert_eq!(visible_plan(&keys, &HashSet::new(), false).len(), keys.len());
    assert_eq!(visible_plan(&keys[..20], &HashSet::new(), true).len(), 20);
    // a stretch that spares fewer than two rows is not worth a marker
    let tight: Vec<Option<usize>> = (0..5).map(Some).chain([None]).collect();
    assert_eq!(visible_plan(&tight, &HashSet::new(), true).len(), 6);

    // in the tab: change line 15 of 30, the second change at 29 of the new text
    let mut compare = Compare::new(Path::new("x.rs"), numbered(30, &[]), numbered(30, &[15]));
    let hidden = |compare: &Compare, column: usize| compare.items[column].iter().filter(|item| matches!(item, Item::Fold { .. })).count();
    assert!(compare.collapse);
    assert_eq!(compare.items[0].len(), 1 + 4 + 1 + 4 + 1, "a marker, 4 lines, the change, 4 lines, a marker");
    assert_eq!(hidden(&compare, 0), 2);
    assert_eq!(compare.items[0][0], Item::Fold { key: 0, count: 11, starts: [0, 0] });
    assert_eq!(compare.items[1].len(), compare.items[0].len(), "both columns hide the same stretches");
    assert_eq!(compare.blocks[0].rows, [5..6, 5..6], "the change's row in each column counts the marker");
    // the marker opens its stretch
    compare.open_fold(0);
    assert_eq!(hidden(&compare, 0), 1);
    assert_eq!(compare.items[0].len(), 15 + 1 + 4 + 1, "the first stretch is back");
    compare.set_collapse(false);
    assert_eq!(compare.items[0].len(), 30);
    compare.set_collapse(true);
    assert_eq!(hidden(&compare, 0), 2, "back on, the opened ones close");
    // unified collapses too
    compare.set_unified(true);
    assert_eq!(compare.items[0].iter().filter(|item| matches!(item, Item::Fold { .. })).count(), 2);
    assert!(compare.items[1].is_empty());
    assert_eq!(compare.items[0].len(), 1 + 4 + 2 + 4 + 1);
}

#[test]
fn the_columns_line_up_row_for_row_and_scroll_together() {
    use crate::app::filetree::compare::Item;
    // "x" and "y" added after "a", "c" removed: each faces an empty row on the other side
    let compare = Compare::new(Path::new("x.rs"), "a\nb\nc\nd\n".into(), "a\nx\ny\nb\nd\n".into());
    assert_eq!(compare.items[0].len(), 6);
    assert_eq!(compare.items[1].len(), 6, "both columns have the same rows");
    let sides = |column: usize| -> Vec<Option<usize>> {
        compare.items[column]
            .iter()
            .map(|item| match item {
                Item::Line { row, .. } => row.left.or(row.right),
                Item::Fold { .. } => None,
            })
            .collect()
    };
    assert_eq!(sides(0), [Some(0), None, None, Some(1), Some(2), Some(3)], "the HEAD's column has two empty rows across from x and y");
    assert_eq!(sides(1), [Some(0), Some(1), Some(2), Some(3), None, Some(4)], "the file's has one across from the removed c");
    assert!(compare.blocks.iter().all(|block| block.rows[0] == block.rows[1]));
    // where the text edit puts each line: past the empty rows
    assert_eq!(compare.line_rows(0), [(0, true), (3, true), (4, true), (5, true)]);
    assert_eq!(compare.line_rows(1), [(0, true), (1, true), (2, true), (3, true), (5, true)]);
    assert_eq!(compare.totals(), (1, 2), "one line removed, two added");
    // a hidden stretch: its lines sit on the marker's row, without glyphs
    let folded = Compare::new(Path::new("x.rs"), numbered(30, &[]), numbered(30, &[15]));
    let rows = folded.line_rows(1);
    assert_eq!(rows.len(), 30);
    assert_eq!((rows[0], rows[10], rows[11]), ((0, false), (0, false), (1, true)));
    assert_eq!(folded.totals(), (1, 1));
    // the tab's own scroll: the driver is clamped and the other follows on the same row
    let mut compare = Compare::new(Path::new("x.rs"), numbered(40, &[]), format!("{}{}", "novo\n".repeat(6), numbered(40, &[])));
    compare.set_collapse(false);
    compare.scroll_driver(0, 8.0, [100.0, 100.0]);
    assert_eq!(compare.scroll(), [8.0, 8.0]);
    compare.scroll_driver(1, 1000.0, [36.0, 36.0]);
    assert_eq!(compare.scroll(), [36.0, 36.0], "clamped");
}

#[test]
fn the_changed_words_are_whole_spans_and_a_line_changed_whole_has_none() {
    use crate::app::filetree::compare::word_spans;
    use crate::editor::word_marks;
    // one word changed: one span, without the blanks around it
    let (old, new) = ("let total = soma(a, b);", "let total = conta(a, b);");
    let (removed, added) = word_marks(old, new);
    assert_eq!(word_spans(old, removed).iter().map(|span| &old[span.clone()]).collect::<Vec<_>>(), ["soma(a,"]);
    assert_eq!(word_spans(new, added).iter().map(|span| &new[span.clone()]).collect::<Vec<_>>(), ["conta(a,"]);
    // words next to each other with spaces between: one span, no holes at the spaces
    let line = "$a = 'um' . $b['c'] . fim();";
    let start = line.find("'um'").unwrap();
    let end = line.find("fim").unwrap() - 3;
    let marks = vec![start..start + 5, start + 5..start + 7, start + 7..end];
    assert_eq!(word_spans(line, marks), vec![start..end]);
    // the line changed nearly whole (the user's case): its row's color says it, no word spans
    let (old, new) = ("$sentSummary[] = $item;", "$sentSummary[] = 'resultado da função: ' . $item['output'];");
    let (_, added) = word_marks(old, new);
    assert!(word_spans(new, added).is_empty(), "never word by word");
    let (old, new) = ("return x;", "throw new Erro('falhou');");
    let (removed, added) = word_marks(old, new);
    assert!(word_spans(old, removed).is_empty() && word_spans(new, added).is_empty());
    // blanks only, or past the end, give nothing
    assert!(word_spans("a  b", std::iter::once(1..3).collect()).is_empty());
    assert!(word_spans("ab", std::iter::once(5..9).collect()).is_empty());
}

#[test]
fn an_agent_turns_the_collapse_of_the_diff_on_and_off() {
    if crate::editor::command("git").arg("--version").output().is_err() {
        return;
    }
    let dir = repo("diff-collapse");
    let a = dir.join("a.rs");
    fs::write(&a, numbered(30, &[])).unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "segundo"]);
    fs::write(&a, numbered(30, &[15])).unwrap();
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    let state = |app: &DrawingApp| {
        let ed = &app.filetrees.editors[&tree];
        ed.tabs.iter().find_map(|t| if let Body::Compare(c) = &t.body { Some((c.collapse, c.items[0].len())) } else { None }).unwrap()
    };
    app.act_file_tree(&json!({ "action": "diff", "path": "a.rs" })).unwrap();
    assert_eq!(state(&app), (true, 11), "collapsed by default");
    app.act_file_tree(&json!({ "action": "diff", "path": "a.rs", "collapse": false })).unwrap();
    assert_eq!(state(&app), (false, 30));
    app.act_file_tree(&json!({ "action": "diff", "path": "a.rs", "collapse": true })).unwrap();
    assert_eq!(state(&app), (true, 11));
    // what is typed in the right column is the file's text in its own tab, unsaved, and the diff follows
    app.filetrees.editors.get_mut(&tree).unwrap().type_into(&a, numbered(30, &[15, 16]));
    let ed = &app.filetrees.editors[&tree];
    assert!(ed.tabs.iter().any(|t| t.path == a && t.dirty()));
    let _ = fs::remove_dir_all(&dir);
}
