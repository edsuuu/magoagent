use super::*;

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
    let rows = diff_rows("a\nb\nc\nd\n", "a\nB\nc\nx\nd\n");
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[1], DiffRow { left: Some(1), right: Some(1), kind: RowKind::Change });
    assert_eq!(rows[3], DiffRow { left: None, right: Some(3), kind: RowKind::Insert });
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
