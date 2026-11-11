//! Tests of the conflict resolution (merge.rs): the three-way merge by chunk on plain texts, then
//! real conflicts in repositories in temporary folders (the helpers are gittest.rs's).

use super::git::git;
use super::gittest::{commit, repo};
use super::merge::*;
use std::path::{Path, PathBuf};

fn merge(base: &str, mine: &str, theirs: &str) -> Merge {
    Merge::new(PathBuf::from("a.txt"), base, mine, theirs)
}

#[test]
fn changes_of_one_side_go_in_and_only_the_shared_lines_conflict() {
    let base = "1\n2\n3\n4\n5\n6\n7\n8\n";
    // mine changes 2 and 7, theirs changes 4 and 7, each its own way; both add the same last line
    let m = merge(base, "1\nMEU\n3\n4\n5\n6\nsete meu\n8\nfim\n", "1\n2\n3\nDELES\n5\n6\nsete deles\n8\nfim\n");
    assert_eq!(m.conflicts().len(), 1);
    assert_eq!(m.pending(), 1);
    // before any answer the conflict shows the base; the rest is already merged
    assert_eq!(m.text(), "1\nMEU\n3\nDELES\n5\n6\n7\n8\nfim\n");
    let k = m.conflicts()[0];
    assert_eq!((m.side(&m.chunks[k], Side::Mine), m.side(&m.chunks[k], Side::Theirs)), (&["sete meu".to_string()][..], &["sete deles".to_string()][..]));
    // the same text on the three sides is one untouched chunk
    let same = merge(base, base, base);
    assert_eq!((same.chunks.len(), same.pending(), same.text().as_str()), (1, 0, base));
}

#[test]
fn a_conflict_takes_one_side_the_other_or_both_in_the_order_picked() {
    let (base, mine, theirs) = ("a\nb\nc\n", "a\nMEU\nc\n", "a\nDELES\nc\n");
    let mut m = merge(base, mine, theirs);
    let k = m.conflicts()[0];
    // » takes mine; theirs still waits: « puts it after, ✕ leaves it out
    m.apply(k, Side::Mine);
    assert_eq!((m.pending(), m.text().as_str()), (1, mine));
    m.ignore(k, Side::Theirs);
    assert_eq!((m.pending(), m.text().as_str()), (0, mine));
    m.reset(k);
    assert_eq!((m.pending(), m.text().as_str()), (1, base));
    m.apply(k, Side::Theirs);
    m.apply(k, Side::Mine);
    m.apply(k, Side::Mine); // a side goes in once
    assert_eq!((m.pending(), m.text().as_str()), (0, "a\nDELES\nMEU\nc\n"));
    // both ignored: the base's lines stay
    m.reset(k);
    m.ignore(k, Side::Mine);
    m.ignore(k, Side::Theirs);
    assert_eq!((m.pending(), m.text().as_str()), (0, base));
}

#[test]
fn accepting_a_side_gives_that_file_back_whole() {
    let base = "1\n2\n3\n4\n5\n6\n";
    let (mine, theirs) = ("1\nMEU\n3\n4\n5\nmeu fim", "0\n1\n2\n3\n4\n5\ndeles fim\n");
    let mut m = merge(base, mine, theirs);
    assert!(m.pending() > 0);
    m.accept(Side::Mine);
    assert_eq!((m.pending(), m.text().as_str()), (0, mine));
    m.accept(Side::Theirs);
    assert_eq!(m.text(), theirs);
    // a file both sides created has no base: one conflict with everything
    let mut both = merge("", "meu\n", "deles\n");
    assert_eq!(both.conflicts().len(), 1);
    both.pick(both.conflicts()[0], &[Side::Mine, Side::Theirs]);
    assert_eq!(both.text(), "meu\ndeles\n");
}

#[test]
fn the_arrows_walk_the_conflicts_and_stop_at_the_ends() {
    let mut m = merge("1\n2\n3\n4\n5\n", "A\n2\n3\n4\nB\n", "X\n2\n3\n4\nY\n");
    let all = m.conflicts();
    assert_eq!(all.len(), 2);
    m.go(1);
    assert_eq!(m.current, Some(all[0]));
    m.go(1);
    m.go(1);
    assert_eq!((m.current, m.scroll_to), (Some(all[1]), Some(all[1])));
    m.go(-1);
    m.go(-1);
    assert_eq!(m.current, Some(all[0]));
}

/// A repository where merging `outro` into `main` stopped with conflicts in a.txt (two conflicts,
/// and a change of each side that merges by itself) and in b.txt.
pub fn conflicted(name: &str) -> PathBuf {
    let dir = repo(name);
    let write = |a: &str, b: &str| {
        std::fs::write(dir.join("a.txt"), a).unwrap();
        std::fs::write(dir.join("b.txt"), b).unwrap();
    };
    write("1\n2\n3\n4\n5\n6\n7\n8\n9\n", "b\n");
    commit(&dir, "linhas");
    git(&dir, &["checkout", "-qb", "outro"]).unwrap();
    write("1\ndois deles\n3\n4\n5\n6\n7\noito deles\n9\ndeles\n", "b deles\n");
    commit(&dir, "outro");
    git(&dir, &["checkout", "-q", "main"]).unwrap();
    write("zero\n1\ndois meu\n3\n4\n5\n6\n7\noito meu\n9\n", "b meu\n");
    commit(&dir, "main");
    assert!(git(&dir, &["merge", "outro"]).is_err(), "the merge stops with conflicts");
    dir
}

fn staged(dir: &Path, file: &str) -> String {
    git(dir, &["show", &format!(":{file}")]).unwrap()
}

#[test]
fn a_real_merge_lists_its_conflicts_and_takes_mine_or_theirs() {
    let dir = conflicted("resolve-side");
    assert_eq!(unmerged(&dir).unwrap(), vec![("a.txt".to_string(), [true; 3]), ("b.txt".to_string(), [true; 3])]);
    let said = resolve_side(&dir, &dir.join("b.txt"), Side::Theirs).unwrap();
    assert!(said.contains("versão deles") && said.contains("1 arquivo"), "{said}");
    assert_eq!((std::fs::read_to_string(dir.join("b.txt")).unwrap().as_str(), staged(&dir, "b.txt").as_str()), ("b deles\n", "b deles\n"));
    assert!(resolve_side(&dir, &dir.join("b.txt"), Side::Mine).unwrap_err().contains("não está em conflito"));
    // mine for the other: nothing is left in conflict, the merge only waits for its commit
    let said = resolve_side(&dir, &dir.join("a.txt"), Side::Mine).unwrap();
    assert!(said.contains("minha versão") && said.contains("conclui o merge"), "{said}");
    assert_eq!(staged(&dir, "a.txt"), "zero\n1\ndois meu\n3\n4\n5\n6\n7\noito meu\n9\n");
    git(&dir, &["commit", "-qm", "merge"]).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_by_chunk_writes_and_stages_the_result() {
    let dir = conflicted("resolve-chunks");
    let a = dir.join("a.txt");
    let mut m = Merge::read(&dir, &a).unwrap();
    let all = m.conflicts();
    assert_eq!(all.len(), 2);
    // the first takes theirs, the second mine and then theirs; until both are answered, nothing is written
    m.apply(all[0], Side::Theirs);
    m.ignore(all[0], Side::Mine);
    m.apply(all[1], Side::Mine);
    assert!(m.write(&dir).unwrap_err().contains("1 conflito"));
    assert_eq!(unmerged(&dir).unwrap().len(), 2);
    m.apply(all[1], Side::Theirs);
    let said = m.write(&dir).unwrap();
    assert!(said.contains("a.txt mesclado") && said.contains("1 arquivo"), "{said}");
    // mine's new first line and theirs' new last one went in by themselves
    let merged = "zero\n1\ndois deles\n3\n4\n5\n6\n7\noito meu\noito deles\n9\ndeles\n";
    assert_eq!((std::fs::read_to_string(&a).unwrap().as_str(), staged(&dir, "a.txt").as_str()), (merged, merged));
    assert_eq!(unmerged(&dir).unwrap().len(), 1);
    // what has no chunks to merge says so
    assert!(Merge::read(&dir, &a).err().unwrap().contains("não está em conflito"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_agent_lists_and_resolves_conflicts() {
    let dir = conflicted("resolve-agent");
    let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
    let list = describe(&dir, None).unwrap();
    assert!(list.contains("a.txt (os dois lados mudaram)") && list.contains("b.txt"), "{list}");
    let detail = describe(&dir, Some(&a)).unwrap();
    assert!(
        detail.contains("2 conflitos") && detail.contains("Conflito 2") && detail.contains("    oito meu") && detail.contains("    oito deles"),
        "{detail}"
    );
    // a pick per conflict, in order; the wrong count or word changes nothing
    let err = resolve(&dir, &a, &serde_json::json!({ "picks": ["mine"] })).unwrap_err();
    assert!(err.contains("2 conflitos"), "{err}");
    assert!(resolve(&dir, &a, &serde_json::json!({ "picks": ["mine", "talvez"] })).is_err());
    assert!(resolve(&dir, &a, &serde_json::json!({ "side": "nenhum" })).is_err());
    assert!(resolve(&dir, &a, &serde_json::json!({})).unwrap_err().contains("side"));
    assert_eq!(unmerged(&dir).unwrap().len(), 2);
    resolve(&dir, &a, &serde_json::json!({ "picks": ["both_theirs_first", "base"] })).unwrap();
    assert_eq!(staged(&dir, "a.txt"), "zero\n1\ndois deles\ndois meu\n3\n4\n5\n6\n7\n8\n9\ndeles\n");
    // a side takes the whole file
    let said = resolve(&dir, &b, &serde_json::json!({ "side": "mine" })).unwrap();
    assert!(said.contains("conclui o merge"), "{said}");
    assert_eq!(describe(&dir, None).unwrap(), "Nenhum arquivo em conflito.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_one_side_deleted_is_kept_or_removed_whole() {
    let dir = repo("resolve-deleted");
    git(&dir, &["checkout", "-qb", "outro"]).unwrap();
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    commit(&dir, "sem a");
    git(&dir, &["checkout", "-q", "main"]).unwrap();
    std::fs::write(dir.join("a.txt"), "um\nMEU\ntres\n").unwrap();
    commit(&dir, "main");
    assert!(git(&dir, &["merge", "outro"]).is_err());
    assert_eq!(unmerged(&dir).unwrap(), vec![("a.txt".to_string(), [true, true, false])]);
    assert!(Merge::read(&dir, &dir.join("a.txt")).err().unwrap().contains("excluído de um dos lados"));
    let said = resolve_side(&dir, &dir.join("a.txt"), Side::Theirs).unwrap();
    assert!(said.contains("excluído") && !dir.join("a.txt").exists(), "{said}");
    assert!(unmerged(&dir).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_inside_the_repository_resolves_its_own_files() {
    // a file tree may show a folder of the repository, not its top: the paths are from that folder
    let dir = repo("resolve-inside");
    let sub = dir.join("pasta");
    std::fs::create_dir_all(&sub).unwrap();
    let write = |c: &str, d: &str| {
        std::fs::write(sub.join("c.txt"), c).unwrap();
        std::fs::write(sub.join("d.txt"), d).unwrap();
    };
    write("um\ndois\n", "d\n");
    commit(&dir, "pasta");
    git(&dir, &["checkout", "-qb", "outro"]).unwrap();
    write("um\ndois deles\n", "d deles\n");
    commit(&dir, "outro");
    git(&dir, &["checkout", "-q", "main"]).unwrap();
    write("um\ndois meu\n", "d meu\n");
    commit(&dir, "main");
    assert!(git(&dir, &["merge", "outro"]).is_err());
    assert_eq!(unmerged(&sub).unwrap().iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["c.txt", "d.txt"]);
    let mut m = Merge::read(&sub, &sub.join("c.txt")).unwrap();
    m.accept(Side::Theirs);
    m.write(&sub).unwrap();
    assert_eq!(staged(&dir, "pasta/c.txt"), "um\ndois deles\n");
    resolve_side(&sub, &sub.join("d.txt"), Side::Mine).unwrap();
    assert_eq!((staged(&dir, "pasta/d.txt").as_str(), unmerged(&dir).unwrap().len()), ("d meu\n", 0));
    let _ = std::fs::remove_dir_all(&dir);
}
