//! File trees on the bridge: one roped to a terminal or a chat shows in that agent's `list`, and is
//! the tree its `file_tree` means when it names none.

use super::*;

/// The only file tree roped to `from`; none when it has no tree, or more than one.
pub(super) fn linked(nodes: &[Node], from: &str) -> Option<String> {
    let links = &nodes.iter().find(|n| n.id == from)?.links;
    let mut trees = nodes.iter().filter(|n| n.kind == NodeKind::Tree && links.contains(&n.id));
    let first = trees.next()?;
    trees.next().is_none().then(|| first.id.clone())
}

/// The tree an agent means: by name among the ones it reaches, or the only one when it names none.
pub(super) fn named(nodes: &[Node], from: &str, name: &str) -> Option<Node> {
    let mut trees = reachable(nodes, from).into_iter().filter(|n| n.kind == NodeKind::Tree);
    if name.trim().is_empty() {
        let only = trees.next()?;
        return trees.next().is_none().then_some(only);
    }
    trees.find(|n| n.name.eq_ignore_ascii_case(name.trim()))
}

/// `file_tree` asked with no `tree`: the one roped to who asks goes in its place, before the app
/// falls back to the selected one or the only one (app/filetree.rs `tree_named`).
pub(super) fn aim(action: &str, args: &mut serde_json::Map<String, Value>, nodes: &[Node], from: &str) {
    let unnamed = args.get("tree").and_then(Value::as_str).is_none_or(|t| t.trim().is_empty());
    if action == "file_tree"
        && unnamed
        && let Some(id) = linked(nodes, from)
    {
        args.insert("tree".into(), id.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_tree_with_no_tree_means_the_one_roped_to_who_asks() {
        let node = |id: &str, kind, links: &[&str]| Node {
            id: id.into(),
            kind,
            name: id.into(),
            text: String::new(),
            links: links.iter().map(|l| l.to_string()).collect(),
            maestro: true,
        };
        let nodes = vec![
            node("Dev", NodeKind::Terminal, &["a"]),
            node("Papo", NodeKind::Chat, &["b"]),
            node("Dois", NodeKind::Terminal, &["a", "b"]),
            node("Solto", NodeKind::Terminal, &[]),
            node("a", NodeKind::Tree, &["Dev", "Dois"]),
            node("b", NodeKind::Tree, &["Papo", "Dois"]),
        ];
        let aimed = |from: &str, action: &str, args: Value| {
            let mut args = args.as_object().cloned().unwrap();
            aim(action, &mut args, &nodes, from);
            args.get("tree").and_then(Value::as_str).map(String::from)
        };
        // a terminal's and a chat's: each its own tree, of the two on the canvas
        assert_eq!(aimed("Dev", "file_tree", json!({ "action": "list" })).as_deref(), Some("a"));
        assert_eq!(aimed("Papo", "file_tree", json!({ "action": "list", "tree": " " })).as_deref(), Some("b"));
        // a tree said stays; two roped, none, or a caller outside the canvas: the app's own choice
        assert_eq!(aimed("Dev", "file_tree", json!({ "tree": "b" })).as_deref(), Some("b"));
        assert_eq!(aimed("Dois", "file_tree", json!({})), None);
        assert_eq!(aimed("Solto", "file_tree", json!({})), None);
        assert_eq!(aimed("", "file_tree", json!({})), None);
        // only `file_tree` has a tree to mean
        assert_eq!(aimed("Dev", "tasks", json!({})), None);
    }

    #[test]
    fn a_roped_tree_shows_in_list_and_opens_files_for_the_agent() {
        let node = |id: &str, kind, text: &str, links: &[&str]| Node {
            id: id.into(),
            kind,
            name: id.into(),
            text: text.into(),
            links: links.iter().map(|l| l.to_string()).collect(),
            maestro: false,
        };
        let shared = Mutex::new(Shared::default());
        shared.lock().unwrap().nodes =
            vec![node("Dev", NodeKind::Terminal, "", &["Src"]), node("Src", NodeKind::Tree, "/proj", &["Dev"]), node("Solto", NodeKind::Terminal, "", &[])];
        let (sender, commands) = mpsc::channel();
        let run = |from: &str, request: Value| {
            let mut request = request;
            request["from"] = from.into();
            handle(&request, &shared, &sender)
        };
        let list = run("Dev", json!({ "cmd": "list" })).unwrap();
        assert!(list.contains("Árvores de arquivos") && list.contains("\"Src\" (/proj)"), "{list}");
        assert!(!run("Solto", json!({ "cmd": "list" })).unwrap().contains("Src"), "an unroped agent does not see it");
        // ide_open with no IDE: the tree's editor pane shows the file, at its line, through the app's own `file_tree`
        let opened = std::thread::scope(|s| {
            let ask = s.spawn(|| run("Dev", json!({ "cmd": "ide_open", "path": "src/main.rs", "line": 7 })));
            let UiCommand::Act { action, args, reply } = commands.recv_timeout(Duration::from_secs(5)).unwrap() else { panic!("not an act") };
            let _ = reply.send(Ok("aberto".into()));
            assert_eq!(action, "file_tree");
            assert_eq!((args["action"].as_str(), args["tree"].as_str(), args["line"].as_u64()), (Some("open"), Some("Src"), Some(7)));
            assert!(args["path"].as_str().unwrap().replace('\\', "/").ends_with("/proj/src/main.rs"));
            ask.join().unwrap()
        });
        assert_eq!(opened, Ok("aberto".into()));
        // a path above the tree is refused; an agent with no tree gets the IDE's own error
        assert!(run("Dev", json!({ "cmd": "ide_open", "path": "../x" })).is_err());
        assert!(run("Solto", json!({ "cmd": "ide_open", "path": "a.rs" })).unwrap_err().contains("IDE"));
    }
}
