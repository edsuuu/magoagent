//! The code of a file tree node's editor pane, as the reference IDE's: the usages of a declaration (Ctrl+click on it, the
//! label over it, an agent), the history of its lines in git with what a commit changed, and "Localizar nos Arquivos".

use super::filetree::{git, temp, tree_frame, tree_on};
use super::*;
use crate::app::filetree::editor::Body;
use crate::app::filetree::find::{Ask, HITS_MAX, count_note, mask_regex};
use crate::app::filetree::vision::{Rows, age, commit_diff, decl_kind, line_history, usages_title};
use crate::ide::find::Hit;
use std::path::Path;

/// Frames until `done`: the project is listed, read and searched on threads.
fn until(app: &mut DrawingApp, ctx: &Context, what: &str, done: impl Fn(&mut DrawingApp) -> bool) {
    for i in 0..400 {
        tree_frame(app, ctx, 100.0 + i as f64 * 0.02, vec![]);
        if done(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("{what}");
}

fn key(key: Key, modifiers: Modifiers) -> egui::Event {
    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

#[test]
fn the_usages_of_a_declaration_are_listed_without_it_by_ctrl_click_and_for_agents() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let dir = temp("usages");
    let (lib, index) = (dir.join("lib.php"), dir.join("index.php"));
    fs::write(&lib, "<?php\n\nfunction montar(int $largura) {}\n").unwrap();
    fs::write(&index, "<?php\nmontar(10);\n$x = montar(2) + desmontar(1);\n").unwrap();
    // an excluded folder is not the project's
    fs::create_dir_all(dir.join("node_modules")).unwrap();
    fs::write(dir.join("node_modules/dep.php"), "<?php\nmontar(3);\n").unwrap();
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    // an agent asks from a file that uses the name: whole words, the declaration left out
    let said = app.act_file_tree(&json!({ "action": "usages", "path": "index.php", "name": "montar" })).unwrap();
    assert_eq!(said, "Função montar (em lib), declarado em lib.php:3: 2 usos\nindex.php:2: montar(10);\nindex.php:3: $x = montar(2) + desmontar(1);\n");
    assert_eq!(app.act_file_tree(&json!({ "action": "usages", "path": "index.php", "name": "nada" })), Err("Declaração de 'nada' não encontrada.".into()));
    assert!(app.act_file_tree(&json!({ "action": "usages", "name": "montar" })).is_err(), "a file is asked for");
    // Ctrl+click on the declaration itself: its usages hang in a popup, searched on a thread
    frame(&mut app, &ctx, vec![]);
    app.ft_activate(&tree, lib.clone());
    let ed = app.filetrees.editors.get_mut(&tree).unwrap();
    let Body::Text(t) = &ed.tabs[0].body else { panic!("not a text") };
    let (lang, text) = (t.lang, t.text.clone());
    assert_eq!(ed.code.declaration(&lib, lang, &text, 17, &ctx), None, "a name is searched off the UI thread");
    let usages = |app: &DrawingApp| match app.filetrees.editors[&tree].popup.as_ref().map(|p| (p.title.as_str(), &p.rows)) {
        Some(("Função montar (em lib)", Rows::Usages(hits))) => hits.iter().map(|h| (h.path.clone(), h.line, h.col)).collect(),
        _ => vec![],
    };
    until(&mut app, &ctx, "no usages listed", |app| usages(app) == [(index.clone(), 1, 0), (index.clone(), 2, 5)]);
    // Enter opens the selected usage in a tab, and the popup goes
    tree_frame(&mut app, &ctx, 200.0, vec![key(Key::Enter, Modifiers::NONE)]);
    let ed = &app.filetrees.editors[&tree];
    assert!(ed.popup.is_none() && ed.active.as_deref() == Some(index.as_path()), "the usage did not open");
    // the label over the declaration says how many, and which lines its history is about
    app.ft_pane_apply(&tree, crate::app::filetree::pane::PaneAct::Select(lib.clone()));
    let labels = |app: &DrawingApp| match app.filetrees.editors[&tree].tabs.iter().find(|t| t.path == lib).map(|t| &t.body) {
        Some(Body::Text(t)) => {
            t.laid.as_ref().map(|l| l.labels.iter().map(|l| (l.name.clone(), l.usages.clone(), l.span)).collect::<Vec<_>>()).unwrap_or_default()
        }
        _ => vec![],
    };
    until(&mut app, &ctx, "no label over the declaration", |app| labels(app) == [("montar".to_string(), "2 usos".to_string(), (2, 3))]);
    // what the popup's title calls a declaration
    let rust = crate::ide::highlight::detect(Path::new("a.rs")).id;
    assert_eq!(decl_kind(rust, "pub(crate) const KINDS: &[&str] = &[];", "KINDS"), ("Constante", false));
    assert_eq!(decl_kind(rust, "    let total = 1;", "total"), ("Variável", true));
    assert_eq!(decl_kind(lang.id, "function montar(int $largura) {}", "montar"), ("Função", false));
    assert_eq!(usages_title("Constante", "KINDS", Path::new("src/events.rs")), "Constante KINDS (em events)");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn find_in_files_searches_the_roots_text_by_case_words_regex_mask_and_folder() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let dir = temp("find");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::create_dir_all(dir.join("node_modules")).unwrap();
    let a = dir.join("src/a.rs");
    fs::write(&a, "fn montar() {}\nfn desmontar() { montar(); }\n").unwrap();
    fs::write(dir.join("notas.txt"), "Montar a mesa\n").unwrap();
    fs::write(dir.join("node_modules/dep.rs"), "montar\n").unwrap();
    let found = |ask: Ask| {
        let mut hits: Vec<_> =
            ask.run_now(&dir).unwrap().iter().map(|h| (h.path.file_name().unwrap().to_string_lossy().into_owned(), h.line, h.col, h.len)).collect();
        hits.sort();
        hits
    };
    let ask = |query: &str| Ask { query: query.into(), ..Default::default() };
    let hit = |name: &str, line: usize, col: usize| (name.to_string(), line, col, 6);
    // any case, inside words too; the excluded folder is not read
    assert_eq!(found(ask("montar")), [hit("a.rs", 0, 3), hit("a.rs", 1, 6), hit("a.rs", 1, 17), hit("notas.txt", 0, 0)]);
    assert_eq!(found(Ask { case: true, ..ask("montar") }), [hit("a.rs", 0, 3), hit("a.rs", 1, 6), hit("a.rs", 1, 17)]);
    assert_eq!(found(Ask { words: true, ..ask("montar") }), [hit("a.rs", 0, 3), hit("a.rs", 1, 17), hit("notas.txt", 0, 0)]);
    assert_eq!(found(Ask { regex: true, ..ask(r"des\w+") }), [("a.rs".to_string(), 1, 3, 9)]);
    assert!(Ask { regex: true, ..ask("(") }.run_now(&dir).unwrap_err().starts_with("Expressão regular inválida"));
    assert_eq!(found(Ask { mask: "*.txt".into(), ..ask("montar") }), [hit("notas.txt", 0, 0)]);
    assert_eq!(found(Ask { dir: Some(dir.join("src")), ..ask("montar") }).len(), 3);
    assert!(found(ask("")).is_empty());
    let mask = mask_regex("*.rs, *.toml").unwrap();
    assert!(mask.is_match("a.rs") && mask.is_match("Cargo.TOML") && !mask.is_match("a.rsx") && mask_regex(" ").is_none());
    // how many, in how many files; a cut list says so
    let fake = |path: &str| Hit { path: path.into(), line: 0, col: 0, len: 1, text: String::new() };
    assert_eq!(count_note(&[fake("a")]), "1 ocorrência em 1 arquivo");
    assert_eq!(count_note(&[fake("a"), fake("a"), fake("b")]), "3 ocorrências em 2 arquivos");
    let many: Vec<Hit> = (0..HITS_MAX).map(|i| fake(if i < 9 { "a" } else { "b" })).collect();
    assert_eq!(count_note(&many), "500+ ocorrências em 2+ arquivos");

    // Ctrl+Shift+F with the tree focused: the window opens with the pane, and what is typed is searched on a thread
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    frame(&mut app, &ctx, vec![]);
    app.card_focus = Some(tree.clone());
    tree_frame(&mut app, &ctx, 1.0, vec![key(Key::F, Modifiers::COMMAND | Modifiers::SHIFT)]);
    assert!(app.filetrees.editors[&tree].find.is_some() && app.ft_spec(&tree).unwrap().editor, "the window did not open");
    tree_frame(&mut app, &ctx, 1.1, vec![]);
    tree_frame(&mut app, &ctx, 1.2, vec![egui::Event::Text("montar".into())]);
    let hits = |app: &DrawingApp| app.filetrees.editors[&tree].find.as_ref().map_or(0, |f| f.hits.len());
    until(&mut app, &ctx, "what was typed was not searched", |app| hits(app) == 4);
    tree_frame(&mut app, &ctx, 200.0, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.filetrees.editors[&tree].find.is_none(), "Esc closes the window");
    // an agent searches: the matches come back, and the window shows the person the same search
    let said = app.act_file_tree(&json!({ "action": "find_in_files", "query": "montar", "words": true, "mask": "*.rs" })).unwrap();
    assert_eq!(said, "2 ocorrências em 1 arquivo\nsrc/a.rs:1: fn montar() {}\nsrc/a.rs:2: fn desmontar() { montar(); }\n");
    let find = app.filetrees.editors[&tree].find.as_ref().unwrap();
    assert!(find.query == "montar" && find.words && find.mask_on && find.mask == "*.rs" && !find.in_dir);
    assert!(app.act_file_tree(&json!({ "action": "find_in_files", "query": "(", "regex": true })).is_err());
    assert!(app.act_file_tree(&json!({ "action": "find_in_files" })).is_err());
    // Enter opens the selected match in a tab
    app.act_file_tree(&json!({ "action": "find_in_files", "query": "desmontar" })).unwrap();
    until(&mut app, &ctx, "the window found nothing", |app| hits(app) == 1);
    tree_frame(&mut app, &ctx, 300.0, vec![key(Key::Enter, Modifiers::NONE)]);
    let ed = &app.filetrees.editors[&tree];
    assert!(ed.find.is_none() && ed.active.as_deref() == Some(a.as_path()), "the match did not open");
    // an agent opens a file at a line
    app.act_file_tree(&json!({ "action": "open", "path": "notas.txt", "line": 1 })).unwrap();
    assert_eq!(app.filetrees.editors[&tree].active.as_deref(), Some(dir.join("notas.txt").as_path()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_history_of_a_declarations_lines_and_what_a_commit_changed_in_the_file() {
    assert_eq!(age(100, 130), "agora");
    assert_eq!(age(10, 0), "agora");
    assert_eq!(age(0, 300), "há 5 min");
    assert_eq!(age(0, 7200), "há 2 h");
    assert_eq!(age(0, 90_000), "ontem");
    assert_eq!(age(0, 4 * 86_400), "há 4 dias");
    assert_eq!(age(0, 2_592_000), "há 1 mês");
    assert_eq!(age(0, 3 * 2_592_000), "há 3 meses");
    assert_eq!(age(0, 31_536_000), "há 1 ano");
    assert_eq!(age(0, 2 * 31_536_000 + 5), "há 2 anos");
    if crate::ide::command("git").arg("--version").output().is_err() {
        return;
    }
    let dir = temp("history");
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "t"]);
    let a = dir.join("a.rs");
    let write = |um: &str, dois: &str, tail: &str| fs::write(&a, format!("fn um() {{\n    {um};\n}}\n\nfn dois() {{\n    {dois};\n}}\n{tail}")).unwrap();
    write("1", "2", "");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "primeiro"]);
    write("1", "22", "");
    git(&dir, &["commit", "-qam", "muda dois"]);
    write("11", "22", "");
    git(&dir, &["commit", "-qam", "muda um"]);
    let subjects = |span: (usize, usize)| line_history(&a, span).unwrap().iter().map(|c| c.subject.clone()).collect::<Vec<_>>();
    // only the commits that touched those lines, the newest first
    assert_eq!(subjects((4, 7)), ["muda dois", "primeiro"]);
    assert_eq!(subjects((0, 3)), ["muda um", "primeiro"]);
    let all = line_history(&a, (0, 7)).unwrap();
    assert_eq!(all.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(), ["muda um", "muda dois", "primeiro"]);
    assert!(all[0].author == "t" && all[0].time > 0 && all[0].hash.len() >= 40);
    // lines written since, past the end of the last commit's file: the range is cut to it
    write("11", "22", "\nfn tres() {}\n");
    assert_eq!(subjects((4, 9)), ["muda dois", "primeiro"]);
    assert!(line_history(&a, (8, 9)).is_ok());
    // what a commit changed in the file; nothing but a hash goes to git
    let diff = commit_diff(&a, &all[1].hash).unwrap();
    assert!(diff.contains("muda dois") && diff.contains("-    2;") && diff.contains("+    22;") && !diff.contains("11;"), "{diff}");
    assert!(commit_diff(&a, "--output=x").is_err());

    // an agent asks for the history of a declaration, or of some lines, and for what one commit changed
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    let said = app.act_file_tree(&json!({ "action": "history", "path": "a.rs", "name": "dois" })).unwrap();
    let short = |i: usize| all[i].hash[..7].to_string();
    assert_eq!(said, format!("Histórico de a.rs, linhas 5 a 7: 2 commits\n{}  t, agora: muda dois\n{}  t, agora: primeiro\n", short(1), short(2)));
    let said = app.act_file_tree(&json!({ "action": "history", "path": "a.rs", "from": 1, "to": 3 })).unwrap();
    assert!(said.starts_with("Histórico de a.rs, linhas 1 a 3: 2 commits\n") && said.contains("muda um"), "{said}");
    assert!(app.act_file_tree(&json!({ "action": "history", "path": "a.rs" })).is_err(), "a declaration or lines are asked for");
    assert!(app.act_file_tree(&json!({ "action": "history", "path": "a.rs", "name": "nada" })).is_err());
    let said = app.act_file_tree(&json!({ "action": "history", "path": "a.rs", "commit": all[1].hash })).unwrap();
    assert_eq!(said, diff);
    // the diff is a tab of its own in the pane, read only
    let ed = &app.filetrees.editors[&tree];
    let tab = ed.tabs.iter().find(|t| ed.active.as_ref() == Some(&t.path)).unwrap();
    assert_eq!(tab.path.file_name().unwrap().to_string_lossy(), format!("{} a.rs", short(1)));
    assert!(matches!(&tab.body, Body::Diff(text) if *text == diff) && !tab.dirty() && app.ft_spec(&tree).unwrap().editor);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn another_checkout_inside_the_project_is_not_its_code() {
    let dir = temp("checkouts");
    fs::write(
        dir.join("a.rs"),
        "fn a() {}
",
    )
    .unwrap();
    // an agent's worktree (a .git file) and a nested repository (a .git folder): copies, not the project
    for (sub, git) in [("wt", false), ("repo", true)] {
        fs::create_dir_all(dir.join(sub)).unwrap();
        fs::write(
            dir.join(sub).join("b.rs"),
            "fn b() {}
",
        )
        .unwrap();
        if git {
            fs::create_dir_all(dir.join(sub).join(".git")).unwrap();
        } else {
            fs::write(
                dir.join(sub).join(".git"),
                "gitdir: x
",
            )
            .unwrap();
        }
    }
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/c.rs"),
        "fn c() {}
",
    )
    .unwrap();
    let mut files = crate::app::filetree::code::code_files(&dir);
    files.sort();
    assert_eq!(files, [dir.join("a.rs"), dir.join("src").join("c.rs")]);
}
