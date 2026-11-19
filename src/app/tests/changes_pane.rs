//! Alterações (panes/changes/): the edge pulled wider and back, the list in ten rows with its search by name or code,
//! an agent's worktree picked, the diff in its colors, and lines picked for a review that goes to an agent, by the
//! panel and by the MCP's `changes_review`.

use super::filetree::{git, temp, tree_on};
use super::panes::{SCREEN, find, opened, pane_click, pane_frame, pane_input, texts};
use super::*;
use crate::app::filetree::view::TreeGit;
use crate::app::panes::{CANVAS_MIN, PANE_MIN, PANE_WIDTH, Pane, clamp_pane_width};
use crate::editor::highlight::St;
use egui::accesskit::Role;
use std::path::Path;

type Shown = Vec<(Role, String, Rect)>;

fn has_git() -> bool {
    crate::editor::command("git").arg("--version").output().is_ok()
}

/// A repository whose first commit has `files` (name, text).
fn repo(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = temp(name);

    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "t"]);

    for (file, text) in files {
        fs::write(dir.join(file), text).unwrap();
    }

    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "first"]);
    dir
}

/// The Alterações panel open on a tree of `dir`, its git read.
fn panel_on(dir: &Path) -> (Context, DrawingApp, String) {
    let (ctx, mut app, _) = opened(Pane::Changes);
    let id = tree_on(&mut app, dir);

    app.filetrees.views.get_mut(&id).unwrap().git = TreeGit::read(dir);
    (ctx, app, id)
}

/// Frames until `done` (the panel reads git on a thread of its own), 5 s at most.
fn until(app: &mut DrawingApp, ctx: &Context, done: impl Fn(&DrawingApp) -> bool) -> Shown {
    for _ in 0..500 {
        let shown = pane_frame(app, ctx, vec![]);

        if done(app) {
            return pane_frame(app, ctx, vec![]);
        }

        drop(shown);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    panic!("the panel's read never came");
}

/// The files the list shows, by their rows' names, top to bottom.
fn files(shown: &Shown) -> Vec<String> {
    let mut rows: Vec<&(Role, String, Rect)> =
        shown.iter().filter(|(role, text, _)| *role == Role::Button && text.contains('.') && !text.contains(' ')).collect();

    rows.sort_by(|a, b| a.2.top().total_cmp(&b.2.top()));
    rows.into_iter().map(|(_, text, _)| text.clone()).collect()
}

fn chat(id: &str, name: &str, cwd: &Path) -> crate::app::chat::direct::Chat {
    crate::app::chat::direct::Chat {
        id: id.into(),
        agent: "claude".into(),
        name: name.into(),
        cwd: cwd.to_string_lossy().into_owned(),
        session: None,
        mode: 0,
        model: String::new(),
        effort: String::new(),
        recruit: false,
        named: false,
        draft: false,
    }
}

fn press(at: Pos2, pressed: bool, modifiers: Modifiers) -> egui::Event {
    egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers }
}

#[test]
fn the_edge_pulls_the_panel_wider_keeps_it_and_a_double_click_gives_back_the_default() {
    assert_eq!(clamp_pane_width(100.0, 1200.0), PANE_MIN);
    assert_eq!(clamp_pane_width(5000.0, 1200.0), 1200.0 - CANVAS_MIN, "the canvas keeps its room");
    assert_eq!(clamp_pane_width(450.4, 0.0), 450.0, "the window not known yet: only the least");
    assert_eq!(clamp_pane_width(900.0, 500.0), PANE_MIN, "a window too small still has the narrowest panel");
    let (ctx, mut app, _) = opened(Pane::Changes);

    assert_eq!(app.pane_width(), PANE_WIDTH);
    let edge = Pos2::new(SCREEN.x - PANE_WIDTH, 400.0);
    let to = Pos2::new(SCREEN.x - 520.0, 400.0);

    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edge), button(edge, true)]);
    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edge - Vec2::new(10.0, 0.0))]);
    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
    assert_eq!((app.panes.pulled, app.pane_width()), (Some(520.0), 520.0), "the canvas gives the panel its width as it is pulled");
    assert_eq!(app.prefs.pane_width, None, "the prefs (and their file) are not written at every frame of the pull");
    pane_frame(&mut app, &ctx, vec![button(to, false)]);
    pane_frame(&mut app, &ctx, vec![]);
    assert_eq!((app.prefs.pane_width, app.panes.pulled), (Some(520.0), None), "let go, it is kept");
    let shown = pane_frame(&mut app, &ctx, vec![egui::Event::PointerGone]);
    let said = find(&shown, Role::Label, "Nenhuma Árvore").expect("the panel says it has no tree");

    assert!(said.left() < SCREEN.x - 500.0, "the panel's insides take the width: {said:?}");
    // pulled past the canvas' room, it stops there
    let edge = Pos2::new(SCREEN.x - 520.0, 400.0);

    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edge), button(edge, true)]);
    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(30.0, 400.0))]);
    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(20.0, 400.0))]);
    pane_frame(&mut app, &ctx, vec![button(Pos2::new(20.0, 400.0), false)]);
    pane_frame(&mut app, &ctx, vec![egui::Event::PointerGone]);
    assert_eq!(app.prefs.pane_width, Some(app.panes.room - CANVAS_MIN));
    // a double-click on the edge: back to the default
    let edge = Pos2::new(SCREEN.x - app.pane_width(), 400.0);

    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edge), button(edge, true)]);
    pane_frame(&mut app, &ctx, vec![button(edge, false)]);
    pane_frame(&mut app, &ctx, vec![button(edge, true)]);
    pane_frame(&mut app, &ctx, vec![button(edge, false)]);
    assert_eq!((app.prefs.pane_width, app.pane_width()), (None, PANE_WIDTH));
    // the MCP's width
    assert_eq!(app.act("panel", &json!({ "width": 480 })).unwrap(), "Largura do painel: 480 px.");
    assert_eq!((app.prefs.pane_width, app.panes.open), (Some(480.0), Some(Pane::Changes)), "only the width changes");
    assert_eq!(app.act("panel", &json!({ "name": "changes", "open": true, "width": 0 })).unwrap(), "Painel Alterações aberto. Largura do painel: 340 px.");
    assert_eq!(app.prefs.pane_width, None);
}

#[test]
fn the_list_shows_six_rows_and_the_search_finds_a_name_or_a_stretch_of_code() {
    if !has_git() {
        return;
    }

    let names: Vec<String> = (0..12).map(|n| format!("arquivo{n:02}.txt")).collect();
    let committed: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "linha um\n")).collect();
    let dir = repo("changes-list", &committed);

    for (index, name) in names.iter().enumerate() {
        let text = if index == 7 { "linha um\nlet segredo = 42;\n".to_string() } else { format!("linha um\nmudou {index}\n") };

        fs::write(dir.join(name), text).unwrap();
    }

    let (ctx, mut app, _) = panel_on(&dir);
    let shown = until(&mut app, &ctx, |app| app.changed_lines(&dir.join("arquivo07.txt")).is_some());

    assert_eq!(files(&shown), names, "every file is in the list, in order");
    // six rows in view: the rest scrolls inside the list, the review starts right under it
    let row = |name: &str| find(&shown, Role::Button, name).unwrap();
    let below = find(&shown, Role::Label, "Revisão").expect("the review under the list");

    assert!(below.top() < row("arquivo07.txt").top(), "the 8th row is past the list's bottom: {below:?}");
    assert!(below.top() > row("arquivo05.txt").bottom(), "the 6th is in view");
    assert!(below.top() - row("arquivo00.txt").top() < 7.0 * 28.0 + 20.0, "{below:?}");
    // by the file's name
    app.panes.changes.search = "ARQUIVO03".into();
    let shown = pane_frame(&mut app, &ctx, vec![]);

    assert_eq!(files(&shown), ["arquivo03.txt"]);
    assert!(texts(&shown).iter().any(|t| t == "1 de 12 arquivos"), "{:?}", texts(&shown));
    // by a stretch of its changes
    app.panes.changes.search = "segredo = 42".into();
    let shown = pane_frame(&mut app, &ctx, vec![]);

    assert_eq!(files(&shown), ["arquivo07.txt"]);
    app.panes.changes.search = "mudou".into();
    assert_eq!(files(&pane_frame(&mut app, &ctx, vec![])).len(), 11);
    app.panes.changes.search = "nada assim".into();
    let shown = pane_frame(&mut app, &ctx, vec![]);

    assert!(files(&shown).is_empty());
    assert!(texts(&shown).iter().any(|t| t.starts_with("Nada com “nada assim”")), "{:?}", texts(&shown));
    // the files under their folder's header, which counts them and folds on a click; a search shows them folded or not
    app.panes.changes.search.clear();
    let shown = pane_frame(&mut app, &ctx, vec![]);
    let folder = find(&shown, Role::DisclosureTriangle, "(raiz) (12)").expect("the root's header, with its count");

    pane_click(&mut app, &ctx, folder.center());
    assert!(files(&pane_frame(&mut app, &ctx, vec![])).is_empty(), "folded");
    app.panes.changes.search = "arquivo03".into();
    assert_eq!(files(&pane_frame(&mut app, &ctx, vec![])), ["arquivo03.txt"]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_agents_worktree_is_picked_and_shows_its_own_changes() {
    if !has_git() {
        return;
    }

    let dir = repo("changes-worktrees", &[("a.txt", "um\n")]);
    let (worktree, branch) = crate::app::maestro::environments::make(&dir, "Revisor", "ab12cd").unwrap();

    fs::write(worktree.join("a.txt"), "um\ndois\n").unwrap();
    fs::write(worktree.join("novo.txt"), "novo\n").unwrap();
    let (ctx, mut app, _) = panel_on(&dir);

    assert_eq!(app.changed_file_count(), 0, "the main checkout has none");
    let shown = until(&mut app, &ctx, |app| app.panes.changes.worktrees.len() == 2);
    let listed = app.act("changes_review", &json!({ "action": "worktrees" })).unwrap();

    assert!(listed.contains("* Todos") && listed.contains(&format!("revisor-ab12cd · {branch}")), "{listed}");
    // the picker on top: Todos, then each worktree
    let picker = shown.iter().find(|(_, text, _)| text.starts_with("Worktree: Todos")).map(|(.., rect)| *rect).expect("the worktree picker");
    let shown = pane_click(&mut app, &ctx, picker.center());
    let shown = if texts(&shown).iter().any(|t| t.starts_with("revisor-ab12cd")) { shown } else { pane_frame(&mut app, &ctx, vec![]) };
    let item = shown.iter().find(|(_, text, _)| text.starts_with("revisor-ab12cd")).map(|(.., rect)| *rect).expect("the worktree in the list");

    pane_click(&mut app, &ctx, item.center());
    assert_eq!(app.panes.changes.worktree.as_deref(), Some(worktree.as_path()));
    let shown = until(&mut app, &ctx, |app| app.panes.changes.worktree_git.is_some());

    assert_eq!(files(&shown), ["a.txt", "novo.txt"], "{:?}", texts(&shown));
    assert!(texts(&shown).iter().any(|t| t.starts_with(&format!("Worktree: revisor-ab12cd · {branch}"))), "{:?}", texts(&shown));
    let row = find(&shown, Role::Button, "a.txt").unwrap();

    pane_click(&mut app, &ctx, row.center());
    assert!(app.panes.changes.diff.iter().any(|l| l.mark == '+' && l.text == "dois"));
    // no tree shows the worktree: Abrir na Árvore makes one
    let shown = pane_frame(&mut app, &ctx, vec![]);

    pane_click(&mut app, &ctx, find(&shown, Role::Button, "Abrir na Árvore").unwrap().center());
    let tree = app.scene.elements.iter().find(|e| e.filetree.as_ref().is_some_and(|s| s.root_path() == worktree)).expect("a tree of the worktree");

    assert_eq!(app.terminal_elevated.as_deref(), Some(tree.id.as_str()));
    // the MCP: back to all the trees, then the worktree by its branch
    assert_eq!(app.act("panel", &json!({ "worktree": "todos" })).unwrap(), "Painel Alterações aberto. Alterações de todas as Árvores.");
    assert_eq!(app.panes.changes.worktree, None);
    let picked = app.act("panel", &json!({ "worktree": branch })).unwrap();

    assert!(picked.contains("Worktree revisor-ab12cd"), "{picked}");
    assert_eq!(app.panes.changes.worktree.as_deref(), Some(worktree.as_path()));
    assert!(app.act("panel", &json!({ "worktree": "outro" })).unwrap_err().contains("Nenhum worktree outro"));
    let _ = crate::editor::run_git(&dir, &["worktree", "remove", "--force", &worktree.to_string_lossy()]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn lines_picked_in_the_colored_diff_become_a_review_that_goes_to_the_agent() {
    if !has_git() {
        return;
    }

    let dir = repo("changes-review", &[("calc.rs", "fn soma(a: i32, b: i32) -> i32 {\n    a + b\n}\n")]);

    fs::write(dir.join("calc.rs"), "fn soma(a: i32, b: i32) -> i32 {\n    a - b\n}\n\nfn dobro(x: i32) -> i32 {\n    x * 3\n}\n").unwrap();
    let (ctx, mut app, _) = panel_on(&dir);

    app.chats = vec![chat("fora", "Outro", Path::new("/longe")), chat("rev", "Revisor", &dir)];
    let shown = until(&mut app, &ctx, |app| app.change_line_counts(&dir.join("calc.rs")).is_some());

    pane_click(&mut app, &ctx, find(&shown, Role::Button, "calc.rs").unwrap().center());
    let diff = app.panes.changes.diff.clone();
    let line = |mark: char, text: &str| diff.iter().position(|l| l.mark == mark && l.text == text).unwrap();

    assert!(diff[line('+', "fn dobro(x: i32) -> i32 {")].spans.iter().any(|s| s.2 == St::Keyword), "the code in its colors");
    let shown = pane_frame(&mut app, &ctx, vec![]);
    let body = find(&shown, Role::Group, "Linhas do diff").expect("the diff's lines");
    let at = |row: usize| Pos2::new(body.left() + 60.0, body.top() + row as f32 * 17.0 + 8.5);
    // a press picks a line, Shift+click the ones down to another
    let (removed, added) = (line('-', "    a + b"), line('+', "    a - b"));

    pane_click(&mut app, &ctx, at(removed));
    assert_eq!(app.panes.changes.review.picked, Some((removed, removed)));
    pane_input(&mut app, &ctx, vec![egui::Event::PointerMoved(at(added)), press(at(added), true, Modifiers::SHIFT)], Modifiers::SHIFT);
    pane_input(&mut app, &ctx, vec![press(at(added), false, Modifiers::SHIFT)], Modifiers::SHIFT);
    assert_eq!(app.panes.changes.review.picked, Some((removed, added)));
    let shown = pane_frame(&mut app, &ctx, vec![egui::Event::PointerGone]);

    assert!(texts(&shown).iter().any(|t| t == "Comentar calc.rs:2"), "{:?}", texts(&shown));
    // written (the button takes clicks from the frame after)
    app.panes.changes.review.draft = "a soma virou subtração".into();
    let shown = pane_frame(&mut app, &ctx, vec![]);

    pane_click(&mut app, &ctx, find(&shown, Role::Button, "Comentar").unwrap().center());
    let review = &app.panes.changes.review;

    assert_eq!((review.comments.len(), review.picked), (1, None));
    assert_eq!((review.comments[0].place(), review.comments[0].excerpt.as_str()), ("calc.rs:2".to_string(), "-    a + b\n+    a - b"));
    // a line picked: the comment's field has the keys, and Esc lets the line go, not the panel
    pane_click(&mut app, &ctx, at(added));
    pane_frame(&mut app, &ctx, vec![egui::Event::PointerGone]);
    assert!(ctx.memory(|m| m.focused()).is_some(), "the comment's field takes the keys");
    let escape = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };

    pane_frame(&mut app, &ctx, vec![escape]);
    assert_eq!((app.panes.changes.review.picked, app.panes.open), (None, Some(Pane::Changes)));
    // a drag picks a stretch
    let shown = pane_frame(&mut app, &ctx, vec![]);
    let body = find(&shown, Role::Group, "Linhas do diff").unwrap();
    let at = |row: usize| Pos2::new(body.left() + 60.0, body.top() + row as f32 * 17.0 + 8.5);
    let (first, last) = (line('+', "fn dobro(x: i32) -> i32 {"), line('+', "    x * 3"));

    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at(first)), button(at(first), true)]);
    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at(first) + Vec2::new(0.0, 6.0))]);
    pane_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at(last))]);
    pane_frame(&mut app, &ctx, vec![button(at(last), false)]);
    assert_eq!(app.panes.changes.review.picked, Some((first, last)));
    app.panes.changes.review.draft = "o dobro é x * 2".into();
    pane_frame(&mut app, &ctx, vec![egui::Event::PointerGone]);
    let shown = pane_frame(&mut app, &ctx, vec![]);

    pane_click(&mut app, &ctx, find(&shown, Role::Button, "Comentar").unwrap().center());
    assert_eq!(app.panes.changes.review.comments[1].place(), "calc.rs:5-6");
    // the × takes one away
    let shown = pane_frame(&mut app, &ctx, vec![]);
    let crosses: Vec<Rect> = shown.iter().filter(|(role, text, _)| *role == Role::Button && text == "×").map(|(.., rect)| *rect).collect();

    assert_eq!(crosses.len(), 2);
    pane_click(&mut app, &ctx, crosses[1].center());
    assert_eq!(app.panes.changes.review.comments.len(), 1);
    // the agent working in the folder comes first and gets it, with the general note
    assert_eq!(app.panes.changes.review.agent, "rev");
    app.panes.changes.review.note = "rode os testes depois".into();
    let shown = pane_frame(&mut app, &ctx, vec![]);

    pane_click(&mut app, &ctx, find(&shown, Role::Button, "Enviar revisão").unwrap().center());
    let (to, message) = app.chat_outbox.last().cloned().expect("the review went");

    assert_eq!(to, "rev");
    assert!(message.contains("\n\n1. calc.rs:2\n```diff\n-    a + b\n+    a - b\n```\na soma virou subtração"), "{message}");
    assert!(message.ends_with("Observação geral: rode os testes depois"), "{message}");
    assert!(app.panes.changes.review.comments.is_empty() && app.panes.changes.review.note.is_empty(), "sent, the review starts over");
    // the whole diff as a PR
    let shown = pane_frame(&mut app, &ctx, vec![]);

    pane_click(&mut app, &ctx, find(&shown, Role::Button, "Pedir análise de PR").unwrap().center());
    let (to, message) = app.chat_outbox.last().cloned().unwrap();

    assert_eq!(to, "rev");
    assert!(message.starts_with("Faça uma análise de PR") && message.contains("M calc.rs (+5 −1)"), "{message}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_review_tool_comments_lines_and_sends_them_like_a_pr_review() {
    if !has_git() {
        return;
    }

    let dir = repo("changes-review-mcp", &[("calc.rs", "fn soma(a: i32, b: i32) -> i32 {\n    a + b\n}\n")]);

    fs::write(dir.join("calc.rs"), "fn soma(a: i32, b: i32) -> i32 {\n    a - b\n}\n").unwrap();
    let (_ctx, mut app, _) = panel_on(&dir);

    app.chats = vec![chat("rev", "Revisor", &dir)];
    let act = |app: &mut DrawingApp, args: Value| app.act("changes_review", &args);

    assert_eq!(
        act(&mut app, json!({ "action": "comment", "path": "calc.rs", "from": 2, "note": "errado" })).unwrap(),
        "Comentário 1 em calc.rs:2 juntado à revisão."
    );

    assert_eq!(app.panes.changes.review.comments[0].excerpt, "-    a + b\n+    a - b");
    // a line that did not change: the file's own
    act(&mut app, json!({ "action": "comment", "path": dir.join("calc.rs").to_string_lossy(), "from": 1, "note": "nome ruim" })).unwrap();
    assert_eq!(app.panes.changes.review.comments[1].excerpt, " fn soma(a: i32, b: i32) -> i32 {");
    assert_eq!(act(&mut app, json!({ "action": "list" })).unwrap(), "2 comentários:\n1. calc.rs:2 — errado\n2. calc.rs:1 — nome ruim");
    assert!(act(&mut app, json!({ "action": "comment", "path": "calc.rs", "from": 2 })).unwrap_err().contains("note"));
    assert!(act(&mut app, json!({ "action": "comment", "path": "nada.rs", "from": 1, "note": "x" })).unwrap_err().contains("Não achei nada.rs"));
    assert!(act(&mut app, json!({ "action": "comment", "path": "calc.rs", "from": 99, "note": "x" })).unwrap_err().contains("tem só 3 linhas"));
    assert!(act(&mut app, json!({ "action": "remove", "index": 5 })).unwrap_err().contains("de 1 a 2"));
    act(&mut app, json!({ "action": "remove", "index": 2 })).unwrap();
    // the comments given go alone, and the ones piled up stay
    let said = act(&mut app, json!({ "action": "send", "agent": "Revisor", "comments": [{ "path": "calc.rs", "from": 3, "note": "chave" }] })).unwrap();

    assert_eq!(said, "Revisão com 1 comentário enviada. Enviado a Revisor.");
    assert!(app.chat_outbox.last().unwrap().1.contains("1. calc.rs:3\n"), "{:?}", app.chat_outbox);
    assert_eq!(app.panes.changes.review.comments.len(), 1);
    // the ones piled up, to the agent in the folder
    act(&mut app, json!({ "action": "send", "note": "e rode os testes" })).unwrap();
    let message = &app.chat_outbox.last().unwrap().1;

    assert!(message.contains("1. calc.rs:2\n") && message.ends_with("Observação geral: e rode os testes"), "{message}");
    assert!(app.panes.changes.review.comments.is_empty());
    assert!(act(&mut app, json!({ "action": "send" })).unwrap_err().starts_with("Nada a enviar"));
    assert!(act(&mut app, json!({ "action": "send", "agent": "Ninguém", "note": "x" })).unwrap_err().contains("Nenhum agente chamado Ninguém"));
    assert_eq!(act(&mut app, json!({ "action": "pr" })).unwrap(), "Análise de PR pedida. Enviado a Revisor.");
    assert!(app.chat_outbox.last().unwrap().1.contains("M calc.rs"));
    assert_eq!(act(&mut app, json!({ "action": "clear" })).unwrap(), "Revisão limpa.");
    assert!(act(&mut app, json!({ "action": "nada" })).is_err());
    let _ = fs::remove_dir_all(&dir);
}
