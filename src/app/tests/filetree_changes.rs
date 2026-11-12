//! The Alterações panel and the `changes` action: the files changed against the HEAD, and their diff on a press.

use super::filetree::{git, temp, tree_frame, tree_on};
use super::*;
use crate::app::filetree::editor::Body;

#[test]
fn the_changes_panel_lists_modified_new_and_deleted_files_and_opens_their_diff() {
    if crate::ide::command("git").arg("--version").output().is_err() {
        return;
    }
    let ctx = Context::default();
    let dir = temp("changes");
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "t"]);
    fs::create_dir_all(dir.join("pkg")).unwrap();
    fs::write(dir.join("kept.txt"), "um\n").unwrap();
    fs::write(dir.join("gone.txt"), "adeus\n").unwrap();
    fs::write(dir.join("pkg/b.txt"), "b\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "first"]);
    fs::write(dir.join("kept.txt"), "um\ndois\n").unwrap();
    fs::remove_file(dir.join("gone.txt")).unwrap();
    fs::write(dir.join("pkg/novo.txt"), "novo\n").unwrap();
    let mut app = DrawingApp::default();
    tree_frame(&mut app, &ctx, 0.0, vec![]);
    let id = tree_on(&mut app, &dir);
    let listed = app.act_file_tree(&json!({ "action": "changes" })).unwrap();
    assert_eq!(listed, "3 arquivos alterados:\nD gone.txt\nM kept.txt\nU pkg/novo.txt");
    // the panel takes the Banco de Dados' place, and draws with the rows under it
    app.ft_show_db(&id, true);
    app.act_file_tree(&json!({ "action": "changes", "show": true })).unwrap();
    let spec = app.ft_spec(&id).unwrap();
    assert!(spec.changes && !spec.db, "one panel at a time");
    for t in [1.0, 1.5, 2.0] {
        tree_frame(&mut app, &ctx, t, vec![]);
    }
    assert_eq!(app.filetrees.views[&id].git.files.len(), 3, "the panel's files come from the tree's git read");
    assert!(ctx.memory(|m| m.area_rect(egui::Id::unique(("ft_changes", id.as_str())))).is_some(), "the panel is over the rows");
    // a press on each: a modified file, a new one (against nothing) and a deleted one (against the HEAD) open a compare tab
    for name in ["kept.txt", "pkg/novo.txt", "gone.txt"] {
        app.ft_open_change(&id, &dir.join(name));
        let ed = &app.filetrees.editors[&id];
        let last = name.rsplit('/').next().unwrap();
        assert!(ed.tabs.iter().any(|t| matches!(&t.body, Body::Compare(_)) && t.path.to_string_lossy().contains(&format!("HEAD ↔ {last}"))), "{name}");
    }
    // one panel off, the other on again
    app.ft_show_db(&id, true);
    assert!(!app.ft_spec(&id).unwrap().changes);
    // outside a repository there is nothing to compare with
    let plain = temp("changes-plain");
    if !plain.ancestors().any(|a| a.join(".git").exists()) {
        let mut other = DrawingApp::default();
        tree_frame(&mut other, &ctx, 0.0, vec![]);
        tree_on(&mut other, &plain);
        assert!(other.act_file_tree(&json!({ "action": "changes" })).unwrap_err().contains("git"));
    }
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&plain);
}
