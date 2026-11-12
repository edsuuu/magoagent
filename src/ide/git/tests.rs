use super::*;

#[test]
fn a_comment_points_at_the_current_change() {
    let diff = Diff::new("a\nb\nc\nd\n", "a\nB\nB2\nc\nd\nE\n");
    // first change: line 2 became two lines (2–3)
    assert_eq!(diff.current_lines(), Some((2, 3, vec!["B", "B2"], false)));
    let next = Diff { current: 1, ..Diff::new("a\nb\nc\nd\n", "a\nB\nB2\nc\nd\nE\n") };
    assert_eq!(next.current_lines(), Some((6, 6, vec!["E"], false)));
    // only removed: the old lines, by their old numbers
    assert_eq!(Diff::new("a\nb\nc\nd\n", "a\nd\n").current_lines(), Some((2, 3, vec!["b", "c"], true)));
    assert_eq!(Diff::new("x\n", "x\n").current_lines(), None);
}

#[test]
fn porcelain_parsing() {
    let root = Path::new("/r");
    let out = " M app/User.php\0?? new.txt\0R  b.php\0a.php\0D  gone.php\0UU c.php\0";
    let v: Vec<_> = parse_status(root, out).into_iter().map(|c| (c.path, c.status)).collect();
    assert_eq!(
        v,
        vec![
            (root.join("app/User.php"), Status::Modified),
            (root.join("new.txt"), Status::Untracked),
            (root.join("b.php"), Status::Renamed),
            (root.join("gone.php"), Status::Deleted),
            (root.join("c.php"), Status::Conflict),
        ]
    );
}

#[test]
fn the_arrow_puts_one_block_back() {
    let (old, new) = ("a\nb\nc\nd\n", "a\nB\nB2\nc\nd\nE\n");
    assert_eq!(revert(old, new, 0, false), "a\nb\nc\nd\nE\n");
    assert_eq!(revert(old, new, 1, false), "a\nB\nB2\nc\nd\n");
    assert_eq!(revert(old, new, 2, false), new, "no such block: nothing changes");
    // Ctrl+»: the new lines stay and the old ones come after them
    assert_eq!(revert(old, new, 0, true), "a\nB\nB2\nb\nc\nd\nE\n");
    assert_eq!(revert("a\nb", "a\nx", 0, true), "a\nx\nb");
    // a removed block comes back where it was; one without the last newline too
    assert_eq!(revert("a\nb\nc\n", "a\nc\n", 0, false), "a\nb\nc\n");
    assert_eq!(revert("a\nb", "a\nx", 0, false), "a\nb");
    assert_eq!(revert("a\rb\n", "a\rx\n", 0, false), "a\rb\n", "a lone \\r is a line end to the differ");
    assert_eq!(Diff::new(old, &revert(old, new, 0, false)).blocks.len(), 1);
}

#[test]
fn a_partial_commit_takes_only_the_included_blocks() {
    let dir = std::env::temp_dir().join(format!("magoagent-partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (file, other) = (dir.join("a.php"), dir.join("b.php"));
    let head = "1\n2\n3\n4\n5\n6\n7\n";
    std::fs::write(&file, head).unwrap();
    std::fs::write(&other, "x\n").unwrap();
    let id = ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
    git(&dir, &["init", "-q"]).unwrap();
    git(&dir, &["add", "."]).unwrap();
    git(&dir, &[&id[..], &["commit", "-qm", "base"]].concat()).unwrap();
    // two blocks in a.php; the second one stays out
    let disk = "1\nDOIS\n3\n4\n5\nSEIS\n7\n";
    std::fs::write(&file, disk).unwrap();
    std::fs::write(&other, "y\n").unwrap();
    let text = partial_text(head, disk, &BTreeSet::from([1]));
    assert_eq!(text, "1\nDOIS\n3\n4\n5\n6\n7\n");
    let commit: Vec<String> = [&id[..], &["commit", "-qm", "parcial"]].concat().into_iter().map(String::from).collect();
    commit_parts(&dir, &["b.php".into()], &[("a.php".into(), text.clone())], &commit).unwrap();
    assert_eq!(git(&dir, &["show", "HEAD:a.php"]).unwrap(), text);
    assert_eq!(git(&dir, &["show", "HEAD:b.php"]).unwrap(), "y\n");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), disk, "the disk keeps both blocks");
    // what is left is the excluded block, unstaged
    assert_eq!(git(&dir, &["diff", "--cached", "--name-only"]).unwrap(), "");
    assert_eq!(git(&dir, &["diff", "--name-only"]).unwrap(), "a.php\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_open_diff_compares_with_head_after_a_commit() {
    let dir = std::env::temp_dir().join(format!("magoagent-diffhead-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.txt");
    std::fs::write(&file, "1\n2\n").unwrap();
    let id = ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
    git(&dir, &["init", "-q"]).unwrap();
    git(&dir, &["add", "."]).unwrap();
    git(&dir, &[&id[..], &["commit", "-qm", "base"]].concat()).unwrap();
    std::fs::write(&file, "1\nDOIS\n").unwrap();
    let mut ide = Ide::new(egui::Context::default(), "h", &dir.to_string_lossy());
    ide.open_diff(file.clone());
    git(&dir, &[&id[..], &["commit", "-qam", "dois"]].concat()).unwrap();
    // the IDE's own commit finished
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Msg::Done("Commit".into(), Ok(String::new()))).unwrap();
    ide.git.rx = Some(rx);
    ide.git_poll();
    assert_eq!(ide.tabs[ide.active].diff.as_ref().unwrap().base, "1\nDOIS\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_agent_commit_saves_first_and_takes_untracked_files_only_when_named() {
    let dir = std::env::temp_dir().join(format!("magoagent-agentcommit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
    std::fs::write(&a, "a\n").unwrap();
    std::fs::write(&b, "b\n").unwrap();
    git(&dir, &["init", "-q"]).unwrap();
    for (k, v) in [("user.name", "t"), ("user.email", "t@t"), ("commit.gpgsign", "false")] {
        git(&dir, &["config", k, v]).unwrap();
    }
    git(&dir, &["add", "."]).unwrap();
    git(&dir, &["commit", "-qm", "base"]).unwrap();
    std::fs::write(dir.join("segredo.env"), "TOKEN=x\n").unwrap();
    let mut ide = Ide::new(egui::Context::default(), "c", &dir.to_string_lossy());
    let wait = |ide: &mut Ide| {
        let start = Instant::now();
        while ide.git.busy.is_some() && start.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(20));
            ide.git_poll();
        }
        assert!(ide.status.contains("concluído"), "{}", ide.status);
    };
    let names = |dir: &Path| git(dir, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
    // b.txt only in the editor; a.txt typed in while someone rewrote it on disk
    ide.open_file(b.clone(), None);
    (ide.tabs[0].text, ide.tabs[0].dirty) = ("b2\n".into(), true);
    ide.open_file(a.clone(), None);
    (ide.tabs[1].text, ide.tabs[1].dirty) = ("a2\n".into(), true);
    std::fs::write(&a, "deles\n").unwrap();
    std::fs::File::options().write(true).open(&a).unwrap().set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
    ide.git.busy = Some("Push…".into());
    assert!(ide.agent_commit("dois", &[]).unwrap_err().contains("ocupado"));
    ide.git.busy = None;
    let err = ide.agent_commit("dois", &[]).unwrap_err();
    assert!(err.contains("a.txt"), "{err}");
    assert_eq!(git(&dir, &["rev-list", "--count", "HEAD"]).unwrap().trim(), "1", "nothing committed");
    // the person keeps theirs: the tracked changes go in, saved; the untracked one is named as left out
    ide.save(1, true);
    let said = ide.agent_commit("dois", &[]).unwrap();
    assert!(said.contains("a.txt, b.txt") && said.contains("segredo.env"), "{said}");
    wait(&mut ide);
    assert_eq!((names(&dir), git(&dir, &["show", "HEAD:b.txt"]).unwrap()), ("a.txt\nb.txt\n".into(), "b2\n".into()));
    // named, it goes in alone
    assert!(ide.agent_commit("x", &["nada.txt".into()]).is_err());
    std::fs::write(&b, "b3\n").unwrap();
    ide.agent_commit("segredo", &["./segredo.env".into()]).unwrap();
    wait(&mut ide);
    assert_eq!(names(&dir), "segredo.env\n");
    drop(ide);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edits_in_the_diff_undo_and_redo() {
    let base = "a\nb\nc\n";
    let mut d = Diff::new(base, "a\nB\nc\nX\n");
    let mine = "a\nB\nc\nX\n".to_string();
    // » on the first block puts b back; a typed line, then two undos and a redo
    let one = d.apply(&mine, DiffEdit::Revert(0, false)).unwrap();
    assert_eq!(one, "a\nb\nc\nX\n");
    let two = d.apply(&one, DiffEdit::Line(3, "Y".into())).unwrap();
    assert_eq!(two, "a\nb\nc\nY\n");
    assert_eq!(d.apply(&two, DiffEdit::Line(3, "Y".into())), None, "the same line changes nothing");
    let back = d.apply(&two, DiffEdit::Undo).unwrap();
    assert_eq!(back, one);
    let back = d.apply(&back, DiffEdit::Undo).unwrap();
    assert_eq!(back, mine);
    assert_eq!(d.apply(&back, DiffEdit::Undo), None);
    assert_eq!(d.apply(&back, DiffEdit::Redo).unwrap(), one);
}

#[test]
fn an_edit_recomputes_the_blocks() {
    let mut d = Diff::new("a\nb\nc\n", "a\nB\nc\n");
    assert_eq!(d.blocks.len(), 1);
    d.recompute("a\nb\nc\n");
    assert!(d.blocks.is_empty());
    d.recompute("a\nb\nc\nX\n");
    assert_eq!((d.blocks.len(), d.base.as_str()), (1, "a\nb\nc\n"));
}

#[test]
fn changed_words_of_a_line() {
    assert_eq!(word_marks("foo bar baz", "foo qux baz"), (vec![4..7], vec![4..7]));
    assert_eq!(word_marks("same", "same"), (vec![], vec![]));
    // an added word marks only the new side
    assert!(word_marks("a", "a b").0.is_empty());
}

#[test]
fn both_sides_walk_the_same_rows() {
    // the viewer scrolls one list of rows: every line of each side shows up once, in order
    let (old, new) = ("a\nb\nc\nd\n", "a\nX\nY\nd\ne\n");
    let rows = diff_rows(old, new);
    let left: Vec<usize> = rows.iter().filter_map(|r| r.left).collect();
    let right: Vec<usize> = rows.iter().filter_map(|r| r.right).collect();
    assert_eq!((left, right), (vec![0, 1, 2, 3], vec![0, 1, 2, 3, 4]));
    let d = Diff::new(old, new);
    assert_eq!(d.blocks, vec![1, 4]);
    assert_eq!((d.block_rows(0), d.block_rows(1)), (1..3, 4..5));
}

#[test]
fn marks_and_rows() {
    let old = "a\nb\nc\nd\n";
    let new = "a\nB\nc\nx\nd\n";
    assert_eq!(line_marks(old, new), vec![(1, 2, MarkKind::Modified), (3, 4, MarkKind::Added)]);
    assert_eq!(line_marks("a\nb\n", "a\n"), vec![(1, 1, MarkKind::Deleted)]);
    let rows = diff_rows(old, new);
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[1], DiffRow { left: Some(1), right: Some(1), kind: RowKind::Change });
    assert_eq!(rows[3], DiffRow { left: None, right: Some(3), kind: RowKind::Insert });
    assert_eq!(Diff::new(old, new).blocks, vec![1, 3]);
}

#[test]
fn blame_porcelain_names_each_line() {
    // the author comes once per commit; uncommitted lines (all-zero sha) have none
    let a = "1".repeat(40);
    let z = "0".repeat(40);
    let out = format!(
        "{a} 1 1 2\nauthor edsuuu\nauthor-mail <x@x>\nsummary first\nfilename f.js\n\tfn f() {{\n{a} 2 2\n\tx();\n{z} 3 3 1\nauthor Not Committed Yet\n\ty();\n"
    );
    assert_eq!(parse_blame(&out), vec![Some("edsuuu".to_string()), Some("edsuuu".to_string()), None]);
}
