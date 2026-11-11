//! Tests of the git operations (branches.rs), the stash (stash.rs) and the log (history.rs) on
//! repositories in temporary folders, with a local test identity: never the project's own.

use super::branches::*;
use super::git::{Status, git};
use super::*;

/// A repository in a temporary folder with a local test identity, on `main`, with a.txt committed.
pub fn repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("magoagent-gitops-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]).unwrap();
    git(&dir, &["symbolic-ref", "HEAD", "refs/heads/main"]).unwrap();
    for (k, v) in [("user.name", "Teste"), ("user.email", "t@t"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")] {
        git(&dir, &["config", k, v]).unwrap();
    }
    std::fs::write(dir.join("a.txt"), "um\ndois\ntres\n").unwrap();
    commit(&dir, "base");
    dir
}

pub fn commit(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]).unwrap();
    git(dir, &["commit", "-qm", message]).unwrap();
}

/// The IDE on `dir`, its first git refresh in.
pub fn ide(dir: &Path) -> Ide {
    let mut ide = Ide::new(egui::Context::default(), "g", &dir.to_string_lossy());
    settle(&mut ide);
    ide
}

/// Waits for the running git command, its refresh and the history's load.
pub fn settle(ide: &mut Ide) {
    let start = Instant::now();
    loop {
        ide.git_poll();
        if (ide.git.busy.is_none() && ide.git.rx.is_none() && !ide.git.history.loading()) || start.elapsed() > Duration::from_secs(30) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Runs `op` and waits; Ok(status line) or Err(why).
pub fn run(ide: &mut Ide, op: GitOp) -> Result<String, String> {
    ide.git_op(op)?;
    settle(ide);
    Ok(ide.status.clone())
}

#[test]
fn refs_parse_with_upstreams_and_counts() {
    let out = "*\trefs/heads/main\torigin/main\tahead 2, behind 1\n \trefs/heads/x\t\t\n \trefs/remotes/origin/HEAD\t\t\n \trefs/remotes/origin/main\t\t\n \trefs/heads/y\torigin/y\tgone\n";
    let v = parse_refs(out);
    assert_eq!(v.len(), 4);
    assert_eq!((v[0].name.as_str(), v[0].current, v[0].ahead, v[0].behind, v[0].upstream.as_str()), ("main", true, 2, 1, "origin/main"));
    assert!(v[2].remote && v[2].name == "origin/main");
    assert_eq!((v[3].ahead, v[3].behind), (0, 0));
}

#[test]
fn branch_names_are_checked() {
    for ok in ["feature/x", "v1.2", "a_b"] {
        assert!(name_problem(ok).is_none(), "{ok}");
    }
    for bad in ["", "-x", "a b", "a..b", "x.lock", "a/", "/a", "a~1", "a:b", "@", "a//b", "a/.b"] {
        assert!(name_problem(bad).is_some(), "{bad}");
    }
}

#[test]
fn git_failures_come_out_in_portuguese() {
    assert!(explain("fatal: The current branch x has no upstream branch.").contains("Publique"));
    assert!(explain("error: The branch 'x' is not fully merged.").contains("exclusão forçada"));
    assert!(explain("CONFLICT (content): Merge conflict in a.txt\nAutomatic merge failed; fix conflicts").contains("1 arquivo"));
    assert_eq!(explain("alguma coisa\nfatal: outra"), "fatal: outra");
}

#[test]
fn branches_are_created_switched_and_deleted_with_their_refusals() {
    let dir = repo("branches");
    let mut ide = ide(&dir);
    assert_eq!(ide.branch, "main");
    run(&mut ide, GitOp::CreateBranch { name: "feature/x".into(), from: String::new(), checkout: true }).unwrap();
    assert_eq!(ide.branch, "feature/x");
    assert!(ide.git.snap.has_local("main") && ide.git.snap.has_local("feature/x"));
    // the same name again, and one git would refuse, never run
    assert!(ide.git_op(GitOp::CreateBranch { name: "main".into(), from: String::new(), checkout: false }).unwrap_err().contains("Já existe"));
    assert!(ide.git_op(GitOp::CreateBranch { name: "a b".into(), from: String::new(), checkout: false }).is_err());
    // created from another branch, without switching to it
    run(&mut ide, GitOp::CreateBranch { name: "lado".into(), from: "main".into(), checkout: false }).unwrap();
    assert_eq!(ide.branch, "feature/x");
    run(&mut ide, GitOp::Checkout("main".into())).unwrap();
    assert_eq!(ide.branch, "main");
    assert!(ide.git_op(GitOp::Checkout("main".into())).unwrap_err().contains("já está"));
    assert!(ide.git_op(GitOp::Checkout("nao-existe".into())).is_err());
    assert!(ide.git_op(GitOp::Checkout("--orphan".into())).is_err(), "an option never reaches git");
    // the current branch stays; one with its own commit is refused until forced
    assert!(ide.git_op(GitOp::DeleteBranch { name: "main".into(), force: false }).is_err());
    run(&mut ide, GitOp::Checkout("lado".into())).unwrap();
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit(&dir, "so no lado");
    run(&mut ide, GitOp::Checkout("main".into())).unwrap();
    let refused = run(&mut ide, GitOp::DeleteBranch { name: "lado".into(), force: false }).unwrap();
    assert!(refused.contains("falhou") && refused.contains("exclusão forçada"), "{refused}");
    assert!(ide.git.snap.has_local("lado"));
    run(&mut ide, GitOp::DeleteBranch { name: "lado".into(), force: true }).unwrap();
    run(&mut ide, GitOp::DeleteBranch { name: "feature/x".into(), force: false }).unwrap();
    assert!(!ide.git.snap.has_local("lado") && !ide.git.snap.has_local("feature/x"));
    // busy: refused, nothing queued
    ide.git.busy = Some("Push…".into());
    assert!(ide.git_op(GitOp::Fetch).unwrap_err().contains("ocupado"));
    drop(ide);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_without_conflict_goes_through() {
    let dir = repo("merge-ok");
    let mut ide = ide(&dir);
    run(&mut ide, GitOp::CreateBranch { name: "outro".into(), from: String::new(), checkout: true }).unwrap();
    std::fs::write(dir.join("novo.txt"), "novo\n").unwrap();
    commit(&dir, "novo");
    run(&mut ide, GitOp::Checkout("main".into())).unwrap();
    assert!(ide.git_op(GitOp::Merge("main".into())).is_err(), "not into itself");
    let said = run(&mut ide, GitOp::Merge("outro".into())).unwrap();
    assert!(said.contains("concluído"), "{said}");
    assert!(dir.join("novo.txt").exists() && !ide.git.snap.merging);
    drop(ide);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_with_conflict_lists_the_files_and_finishes_once_resolved() {
    let dir = repo("merge-conflict");
    let mut ide = ide(&dir);
    run(&mut ide, GitOp::CreateBranch { name: "outro".into(), from: String::new(), checkout: true }).unwrap();
    std::fs::write(dir.join("a.txt"), "um\nOUTRO\ntres\n").unwrap();
    commit(&dir, "outro");
    run(&mut ide, GitOp::Checkout("main".into())).unwrap();
    std::fs::write(dir.join("a.txt"), "um\nMAIN\ntres\n").unwrap();
    commit(&dir, "main");
    let said = run(&mut ide, GitOp::Merge("outro".into())).unwrap();
    assert!(said.contains("falhou") && said.contains("conflitos"), "{said}");
    assert!(ide.git.snap.merging);
    let conflicts: Vec<_> = ide.git.changes.iter().filter(|c| c.status == Status::Conflict).map(|c| c.path.clone()).collect();
    assert_eq!(conflicts, vec![dir.join("a.txt")]);
    // a second merge, or finishing with the markers still in the file, is refused
    assert!(ide.git_op(GitOp::Merge("outro".into())).unwrap_err().contains("andamento"));
    assert!(ide.git_op(GitOp::MergeFinish).unwrap_err().contains("a.txt"));
    // resolved in the editor (its text on disk), the merge finishes with its commit
    std::fs::write(dir.join("a.txt"), "um\nOS DOIS\ntres\n").unwrap();
    run(&mut ide, GitOp::MergeFinish).unwrap();
    assert!(!ide.git.snap.merging);
    assert_eq!(git(&dir, &["rev-list", "--parents", "-n", "1", "HEAD"]).unwrap().split_whitespace().count(), 3, "a merge commit has two parents");
    // and a merge that is dropped puts the files back
    run(&mut ide, GitOp::CreateBranch { name: "terceiro".into(), from: "HEAD~1".into(), checkout: true }).unwrap();
    std::fs::write(dir.join("a.txt"), "um\nTERCEIRO\ntres\n").unwrap();
    commit(&dir, "terceiro");
    run(&mut ide, GitOp::Merge("main".into())).unwrap();
    assert!(ide.git.snap.merging);
    run(&mut ide, GitOp::MergeAbort).unwrap();
    assert!(!ide.git.snap.merging);
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "um\nTERCEIRO\ntres\n");
    drop(ide);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn publish_fetch_and_the_remote_branches_work_against_a_bare_remote() {
    let dir = repo("remote");
    let bare = std::env::temp_dir().join(format!("magoagent-gitops-bare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare"]).unwrap();
    git(&bare, &["symbolic-ref", "HEAD", "refs/heads/main"]).unwrap();
    let mut ide = ide(&dir);
    // no remote yet: refused before git runs
    assert!(ide.git_op(GitOp::Publish).unwrap_err().contains("remoto"));
    assert!(ide.git_op(GitOp::Fetch).unwrap_err().contains("remoto"));
    git(&dir, &["remote", "add", "origin", &bare.to_string_lossy()]).unwrap();
    ide.git_refresh();
    settle(&mut ide);
    assert!(ide.git_op(GitOp::Push).is_ok(), "the plain push is git's to refuse");
    settle(&mut ide);
    assert!(ide.status.contains("falhou") && ide.status.contains("Publique"), "{}", ide.status);
    run(&mut ide, GitOp::Publish).unwrap();
    let b = ide.git.snap.current().unwrap().clone();
    assert_eq!((b.upstream.as_str(), b.ahead), ("origin/main", 0));
    assert!(ide.git_op(GitOp::Publish).unwrap_err().contains("já está publicado"));
    assert!(git(&bare, &["rev-parse", "--verify", "refs/heads/main"]).is_ok());
    // someone else pushes a branch; a fetch brings it as a remote branch
    let other = std::env::temp_dir().join(format!("magoagent-gitops-other-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&other);
    git(&std::env::temp_dir(), &["clone", "-q", &bare.to_string_lossy(), &other.to_string_lossy()]).unwrap();
    for (k, v) in [("user.name", "Outro"), ("user.email", "o@o"), ("commit.gpgsign", "false")] {
        git(&other, &["config", k, v]).unwrap();
    }
    git(&other, &["checkout", "-q", "-b", "remoto"]).unwrap();
    std::fs::write(other.join("r.txt"), "r\n").unwrap();
    commit(&other, "do outro");
    git(&other, &["push", "-q", "origin", "remoto"]).unwrap();
    assert!(!ide.git.snap.branches.iter().any(|b| b.name == "origin/remoto"));
    run(&mut ide, GitOp::Fetch).unwrap();
    assert!(ide.git.snap.branches.iter().any(|b| b.name == "origin/remoto" && b.remote));
    // switching to it makes a local branch that tracks it; and it can be pulled later
    run(&mut ide, GitOp::Checkout("origin/remoto".into())).unwrap();
    assert_eq!(ide.branch, "remoto");
    assert_eq!(ide.git.snap.current().unwrap().upstream, "origin/remoto");
    assert!(dir.join("r.txt").exists());
    run(&mut ide, GitOp::Checkout("main".into())).unwrap();
    // pull with something new upstream, and push with something new here
    std::fs::write(other.join("r2.txt"), "r2\n").unwrap();
    git(&other, &["checkout", "-q", "main"]).unwrap();
    git(&other, &["merge", "-q", "remoto"]).unwrap();
    git(&other, &["push", "-q", "origin", "main"]).unwrap();
    run(&mut ide, GitOp::Pull).unwrap();
    assert!(dir.join("r.txt").exists());
    std::fs::write(dir.join("local.txt"), "l\n").unwrap();
    commit(&dir, "local");
    ide.git_refresh();
    settle(&mut ide);
    assert_eq!(ide.git.snap.current().unwrap().ahead, 1);
    assert!(ide.branch_label().contains("↑1"), "{}", ide.branch_label());
    run(&mut ide, GitOp::Push).unwrap();
    assert_eq!(git(&bare, &["rev-parse", "main"]).unwrap(), git(&dir, &["rev-parse", "main"]).unwrap());
    drop(ide);
    for d in [&dir, &bare, &other] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn the_other_agent_actions_ask_for_confirmation_and_describe() {
    let dir = repo("agent");
    let mut ide = ide(&dir);
    let call = |ide: &mut Ide, v: serde_json::Value| ide.agent_git(&v);
    assert!(call(&mut ide, serde_json::json!({ "action": "branch_delete", "name": "x" })).unwrap_err().contains("confirm"));
    assert!(call(&mut ide, serde_json::json!({ "action": "stash_drop" })).unwrap_err().contains("confirm"));
    assert!(call(&mut ide, serde_json::json!({ "action": "merge_abort" })).unwrap_err().contains("confirm"));
    assert!(call(&mut ide, serde_json::json!({ "action": "checkout" })).unwrap_err().contains("diga"));
    assert!(call(&mut ide, serde_json::json!({ "action": "xyz" })).is_err());
    call(&mut ide, serde_json::json!({ "action": "branch_create", "name": "novo" })).unwrap();
    settle(&mut ide);
    let said = call(&mut ide, serde_json::json!({ "action": "branches" })).unwrap();
    assert!(said.contains("novo (atual") && said.contains("main") && said.contains("Merge em andamento: não"), "{said}");
    call(&mut ide, serde_json::json!({ "action": "checkout", "name": "main" })).unwrap();
    settle(&mut ide);
    call(&mut ide, serde_json::json!({ "action": "branch_delete", "name": "novo", "confirm": true })).unwrap();
    settle(&mut ide);
    assert!(!ide.git.snap.has_local("novo"));
    drop(ide);
    let _ = std::fs::remove_dir_all(&dir);
}
