//! The branch chip: the local branches, the switch, and its refusal when changes would be lost.

use super::filetree::{git, temp, tree_frame, tree_on};
use super::*;

#[test]
fn the_branch_chip_switches_branches_and_refuses_what_would_lose_changes() {
    if crate::ide::command("git").arg("--version").output().is_err() {
        return;
    }
    let ctx = Context::default();
    let dir = temp("branch");
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "t"]);
    fs::write(dir.join("a.txt"), "one").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "first"]);
    git(&dir, &["branch", "-M", "main"]);
    git(&dir, &["checkout", "-q", "-b", "outro"]);
    fs::write(dir.join("a.txt"), "two").unwrap();
    git(&dir, &["commit", "-qam", "second"]);
    let mut app = DrawingApp::default();
    tree_frame(&mut app, &ctx, 0.0, vec![]);
    let id = tree_on(&mut app, &dir);
    let listed = app.act_file_tree(&json!({ "action": "branches" })).unwrap();
    assert_eq!(listed, "  main\n* outro", "the current one is marked");
    // a clean switch
    assert_eq!(app.act_file_tree(&json!({ "action": "checkout", "name": "main" })), Ok("Ramo trocado para main.".into()));
    assert_eq!(crate::ide::run_git(&dir, &["symbolic-ref", "--short", "HEAD"]).unwrap().trim(), "main");
    assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "one");
    // an edit that the other branch's file would overwrite: refused, and the tree stays where it is
    fs::write(dir.join("a.txt"), "mine").unwrap();
    let why = app.act_file_tree(&json!({ "action": "checkout", "name": "outro" })).unwrap_err();
    assert!(why.contains("mudanças locais") && why.contains("outro"), "{why}");
    assert_eq!(crate::ide::run_git(&dir, &["symbolic-ref", "--short", "HEAD"]).unwrap().trim(), "main");
    assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "mine");
    assert!(app.act_file_tree(&json!({ "action": "checkout", "name": "nao-existe" })).unwrap_err().contains("não existe"));
    assert!(app.ft_spec(&id).is_some());
    let _ = fs::remove_dir_all(&dir);
}
