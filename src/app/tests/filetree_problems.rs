//! The problems of a file tree node's code: the local syntax check, what the tree and the editor do with them, F2 and the MCP.

use super::filetree::{temp, tree_frame, tree_on};
use super::*;
use crate::app::filetree_problems::{ERROR, WARNING, check, counts, merge, next_problem, propagate};
use crate::ide::highlight;
use crate::ide::lsp::Diag;
use std::path::Path;

fn found(name: &str, text: &str) -> Vec<(usize, usize, String)> {
    let path = Path::new(name);
    check(path, highlight::detect(path), text).into_iter().map(|d| (d.line, d.col, d.message)).collect()
}

fn at(line: usize, col: usize, severity: u8) -> Diag {
    Diag { line, col, end_line: line, end_col: col + 1, severity, message: format!("{line}:{col}"), source: "teste".into() }
}

#[test]
fn the_local_check_finds_open_brackets_and_quotes() {
    assert_eq!(found("a.rs", "fn main() {\n    let x = (1;\n}\n"), vec![(1, 12, "'(' sem o ')' correspondente".to_string())]);
    assert_eq!(found("a.rs", "fn main() {\n"), vec![(0, 10, "'{' sem o '}' correspondente".to_string())]);
    assert_eq!(found("a.js", "if (x) { y(); }}\n"), vec![(0, 15, "'}' sem o '{' correspondente".to_string())]);
    assert_eq!(found("a.js", "const s = 'aberta;\nlet y = 2;\n"), vec![(0, 10, "Aspas não fechadas".to_string())]);
    assert_eq!(found("a.py", "x = \"\"\"nunca fecha\nprint(1)\n"), vec![(0, 4, "Texto de aspas triplas não fechado".to_string())]);
    assert_eq!(found("a.c", "/* sem fim\nint x;\n"), vec![(0, 0, "Comentário não fechado".to_string())]);
    // a closer for an outer opener leaves the inner one reported
    assert_eq!(found("a.go", "func f() {\n  g(1, 2\n}\n"), vec![(1, 3, "'(' sem o ')' correspondente".to_string())]);
}

#[test]
fn the_local_check_leaves_valid_code_alone() {
    assert!(found("a.rs", "fn f<'a>(x: &'a str) -> char { let c = '('; let r = r#\"(\"#; /* ( /* ) */ ) */ c }\n").is_empty());
    assert!(found("a.js", "const r = /[(]\\)/g; const t = `a ${f(\"}\")} b`; // (\nlet u = 'http://x';\n").is_empty());
    assert!(found("a.py", "s = '''\n(\n'''\n# ( ' \nd = {\"a\": [1, 2]}\n").is_empty());
    assert!(found("a.php", "<p>don't (</p>\n<?php\n$a = <<<EOT\n( ' \"\nEOT;\nf(\"x\");\n?>\n<b>it's</b>\n").is_empty());
    assert!(found("a.css", "a { background: url(http://x/y.png); }\n").is_empty());
    assert!(found("a.cpp", "int n = 1'000; char c = ')';\n").is_empty());
    assert!(found("a.kt", "val s = \"${m[\"k\"]}\"\n").is_empty());
    // languages it does not know, and a lock file that is not JSON
    assert!(found("a.sh", "echo \"(\n").is_empty());
    assert!(found("Cargo.lock", "[[package]]\nname = \"a\"\n").is_empty());
    assert!(found("a.rb", "puts '(\n").is_empty());
}

#[test]
fn json_is_parsed_and_comments_fall_back_to_the_brackets() {
    assert_eq!(found("a.json", "{\"a\": 1,}\n"), vec![(0, 8, "Vírgula sobrando antes do fechamento".to_string())]);
    assert_eq!(found("a.json", "{\"a\": [1, 2}\n")[0].2, "Esperava vírgula ou o fechamento");
    assert!(found("a.json", "{\"a\": [1, 2]}\n").is_empty());
    // tsconfig-style comments are not an error
    assert!(found("tsconfig.json", "{\n  // c\n  \"a\": 1\n}\n").is_empty());
    assert_eq!(found("t.jsonc", "{ // c\n\"a\": [1\n}\n").len(), 1);
}

#[test]
fn the_check_agrees_with_this_very_source() {
    // no false alarm over a real corpus: this app's own Rust
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src];
    let mut files = 0;
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
                assert_eq!(found(&path.to_string_lossy(), &text), vec![], "{}", path.display());
                files += 1;
            }
        }
    }
    assert!(files > 50);
}

#[test]
fn problems_merge_count_and_navigate() {
    // the check's problem on a line the server flagged is dropped; infos are not kept
    let (lsp, local) = (vec![at(3, 0, ERROR), at(9, 0, 3)], [at(3, 5, ERROR), at(7, 1, ERROR)]);
    let all = merge(lsp, &local);
    assert_eq!(all.iter().map(|d| (d.line, d.col)).collect::<Vec<_>>(), vec![(3, 0), (7, 1)]);
    let diags = [at(2, 4, ERROR), at(5, 0, WARNING), at(9, 2, ERROR)];
    assert_eq!(counts(&diags), (2, 1));
    let go = |from, forward| next_problem(&diags, from, forward).map(|d| (d.line, d.col));
    assert_eq!(go((0, 0), true), Some((2, 4)));
    assert_eq!(go((2, 4), true), Some((5, 0)));
    assert_eq!(go((9, 2), true), Some((2, 4)), "F2 goes round to the first");
    assert_eq!(go((5, 0), false), Some((2, 4)));
    assert_eq!(go((2, 4), false), Some((9, 2)), "Shift+F2 goes round to the last");
    assert!(next_problem(&[], (0, 0), true).is_none());
}

#[test]
fn a_file_with_an_error_reddens_its_folders_up_to_the_root() {
    let root = Path::new("/r");
    let marked = propagate(&[root.join("a/b/c.rs"), root.join("a/d.rs"), PathBuf::from("/fora/x.rs")], root);
    let mut names: Vec<String> = marked.iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect();
    names.sort();
    assert_eq!(names, vec!["/r", "/r/a", "/r/a/b", "/r/a/b/c.rs", "/r/a/d.rs"]);
}

#[test]
fn the_tree_marks_what_the_editor_found_and_agents_list_it() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let dir = temp("problems");
    fs::create_dir_all(dir.join("src/deep")).unwrap();
    fs::write(dir.join("src/deep/ruim.rs"), "fn a() {\n    let x = (;\n}\n").unwrap();
    fs::write(dir.join("src/bom.rs"), "fn b() {}\n").unwrap();
    fs::write(dir.join("src/solto.json"), "{\"a\": 1,}").unwrap();
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    // nothing open: an agent asks of the file on disk, and of the whole project
    let one = app.act_file_tree(&json!({ "action": "problems", "path": "src/deep/ruim.rs" })).unwrap();
    let one = one.replace(std::path::MAIN_SEPARATOR, "/");
    assert_eq!(
        one,
        "1 arquivo(s) com problemas (1 erro(s), 0 aviso(s)):\nsrc/deep/ruim.rs: 1 erro, 0 avisos\n  erro 2:13 [sintaxe] '(' sem o ')' correspondente\n"
    );
    let all = app.act_file_tree(&json!({ "action": "problems" })).unwrap();
    assert!(all.starts_with("2 arquivo(s) com problemas (2 erro(s), 0 aviso(s)):"), "{all}");
    assert!(all.contains("solto.json") && all.contains("ruim.rs") && !all.contains("bom.rs"), "{all}");
    let none = app.act_file_tree(&json!({ "action": "problems", "path": "src/bom.rs" })).unwrap();
    assert_eq!(none.replace(std::path::MAIN_SEPARATOR, "/"), "Nenhum problema encontrado em src/bom.rs.");
    // opened in the editor, the tree's names go red: the file and the folders over it
    app.ft_activate(&tree, dir.join("src/deep/ruim.rs"));
    tree_frame(&mut app, &ctx, 100.0, vec![]);
    tree_frame(&mut app, &ctx, 100.1, vec![]);
    let marked = &app.filetrees.views[&tree].problems;
    for p in [dir.clone(), dir.join("src"), dir.join("src/deep"), dir.join("src/deep/ruim.rs")] {
        assert!(marked.contains(&p), "{}", p.display());
    }
    assert!(!marked.contains(&dir.join("src/bom.rs")));
    // the editor's problems: the open text's, by place
    let diags = app.filetrees.editors[&tree].problems.for_file(&dir.join("src/deep/ruim.rs"));
    assert_eq!(diags.len(), 1);
    assert_eq!((diags[0].line, diags[0].col), (1, 12));
    // a path outside the tree is refused
    assert!(app.act_file_tree(&json!({ "action": "problems", "path": "/fora/de/tudo" })).is_err());
}
