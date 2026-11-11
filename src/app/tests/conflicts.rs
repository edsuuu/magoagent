//! The conflicts of a file tree's folder by the MCP's `file_tree`: listed, resolved by the agent, or
//! left to the person in the windows (ide/mergeview.rs), which take the keys as the tree's dialogs do.

use super::*;

#[test]
fn an_agent_lists_and_resolves_the_conflicts_of_a_file_tree() {
    let dir = crate::ide::mergetest::conflicted("tree");
    let mut app = DrawingApp::default();
    let mut act = |app: &mut DrawingApp, args: Value| app.act_file_tree(&args);
    act(&mut app, json!({ "action": "create", "path": dir.to_string_lossy() })).unwrap();
    let list = act(&mut app, json!({ "action": "conflicts" })).unwrap();
    assert!(list.contains("a.txt (os dois lados mudaram)") && list.contains("b.txt"), "{list}");
    let detail = act(&mut app, json!({ "action": "conflicts", "path": "a.txt" })).unwrap();
    assert!(detail.contains("2 conflitos") && detail.contains("    oito deles"), "{detail}");
    // merge: the window opens for the person, and the keys are its own
    assert!(!app.dialog_open());
    let said = act(&mut app, json!({ "action": "merge", "path": "a.txt" })).unwrap();
    assert!(said.contains("Janela de mesclagem") && app.filetrees.conflicts.is_open() && app.dialog_open(), "{said}");
    // resolve: a side for the whole file, or a pick per conflict; never a file outside the tree
    assert!(act(&mut app, json!({ "action": "resolve" })).unwrap_err().contains("path"));
    let outside = dir.parent().unwrap().join("fora.txt");
    assert!(act(&mut app, json!({ "action": "resolve", "path": outside.to_string_lossy(), "side": "mine" })).unwrap_err().contains("fora da árvore"));
    let said = act(&mut app, json!({ "action": "resolve", "path": "b.txt", "side": "theirs" })).unwrap();
    assert!(said.contains("versão deles") && fs::read_to_string(dir.join("b.txt")).unwrap() == "b deles\n", "{said}");
    let said = act(&mut app, json!({ "action": "resolve", "path": "a.txt", "picks": ["mine", "theirs"] })).unwrap();
    assert!(said.contains("conclui o merge"), "{said}");
    assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "zero\n1\ndois meu\n3\n4\n5\n6\n7\noito deles\n9\ndeles\n");
    assert_eq!(act(&mut app, json!({ "action": "conflicts" })).unwrap(), "Nenhum arquivo em conflito.");
    assert!(act(&mut app, json!({ "action": "merge" })).unwrap_err().contains("Nenhum arquivo em conflito"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_conflicted_file_in_the_tree_opens_the_merge_window() {
    let dir = crate::ide::mergetest::conflicted("tree-open");
    let mut app = DrawingApp::default();
    app.act_file_tree(&json!({ "action": "create", "path": dir.to_string_lossy() })).unwrap();
    let id = app.scene.elements.iter().find(|e| e.filetree.is_some()).unwrap().id.clone();
    app.ft_activate(&id, dir.join("a.txt"));
    assert!(app.filetrees.conflicts.is_open(), "a file in conflict goes to the merge window, not the editor");
    let _ = fs::remove_dir_all(&dir);
}
