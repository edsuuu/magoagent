//! An agent's own environment: a git worktree of the repository it was recruited in, on a branch of
//! its own, so two agents changing the same code don't step on each other. Nothing is merged or
//! removed by the app: the branch and the folder stay until someone joins or takes them away.

use crate::prelude::*;
use std::path::Path;

/// Where a recruit's worktree goes and the branch it gets: `<repo>/.magoagent/worktrees/<slug>` and
/// `mago/<slug>`, the slug being the agent's name made safe plus the start of its id.
pub(crate) fn plan(repo: &Path, name: &str, id: &str) -> (PathBuf, String) {
    let safe: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let short: String = id.chars().filter(char::is_ascii_alphanumeric).take(6).collect();
    let slug = [safe.trim_matches('-'), short.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    let slug = if slug.is_empty() { "agente".to_string() } else { slug };
    (repo.join(".magoagent").join("worktrees").join(&slug), format!("mago/{slug}"))
}

/// Makes the worktree for agent `name` out of the repository `folder` is in, from its HEAD: the
/// folder and the branch. `.magoagent/` goes to the repository's own exclude file, so nothing
/// tracked changes.
// ponytail: git runs on the caller's thread (the interface's, on a recruit): a huge repository holds
// the frame while it checks out; hand it to a thread when that shows
pub(crate) fn make(folder: &Path, name: &str, id: &str) -> Result<(PathBuf, String), String> {
    let git = crate::ide::run_git;
    let root = git(folder, &["rev-parse", "--show-toplevel"])
        .map_err(|_| format!("{} não está num repositório git: o ambiente próprio é um worktree, precisa de um.", folder.display()))?;
    let root = PathBuf::from(root.trim());
    let (path, branch) = plan(&root, name, id);
    // left out of git here only: `info/exclude` is the repository's, not a tracked file
    if let Ok(common) = git(&root, &["rev-parse", "--git-common-dir"]) {
        let exclude = root.join(common.trim()).join("info").join("exclude");
        let known = fs::read_to_string(&exclude).unwrap_or_default();
        if !known.lines().any(|l| l.trim() == ".magoagent/") {
            let _ = fs::create_dir_all(exclude.parent().unwrap_or(&root));
            let _ = fs::write(&exclude, format!("{}{}.magoagent/\n", known, if known.is_empty() || known.ends_with('\n') { "" } else { "\n" }));
        }
    }
    git(&root, &["worktree", "add", "-b", &branch, &path.to_string_lossy()]).map_err(|e| format!("O git não criou o ambiente: {e}"))?;
    Ok((path, branch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recruits_environment_has_a_safe_folder_and_a_branch_of_its_own() {
        let repo = Path::new("/repo");
        let (path, branch) = plan(repo, "Revisão de Código", "ab12cd34-ef56");
        assert_eq!(branch, "mago/revis-o-de-c-digo-ab12cd");
        assert_eq!(path, repo.join(".magoagent").join("worktrees").join("revis-o-de-c-digo-ab12cd"));
        assert_eq!(plan(repo, "  ", "xyz").1, "mago/xyz");
        assert_eq!(plan(repo, "../..", "").1, "mago/agente");
        // two agents of the same name get different environments
        assert_ne!(plan(repo, "Front", "aaaaaa").0, plan(repo, "Front", "bbbbbb").0);
    }

    #[test]
    fn the_worktree_is_made_from_the_repository_and_kept_out_of_git() {
        let dir = std::env::temp_dir().join(format!("mago-env-{}", new_id()));
        fs::create_dir_all(dir.join("src")).unwrap();
        let git = |args: &[&str]| crate::ide::run_git(&dir, args);
        // no git on this machine: nothing to check here
        if git(&["init"]).is_err() {
            return;
        }
        fs::write(dir.join("src").join("a.txt"), "a").unwrap();
        git(&["add", "-A"]).unwrap();
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-m", "base"]).unwrap();
        // from a folder inside the repository: the worktree is of the repository itself
        let (path, branch) = make(&dir.join("src"), "Front", "abcdef99").unwrap();
        assert!(path.join("src").join("a.txt").is_file(), "{}", path.display());
        assert_eq!(branch, "mago/front-abcdef");
        assert_eq!(crate::ide::run_git(&path, &["branch", "--show-current"]).unwrap().trim(), "mago/front-abcdef");
        assert_eq!(git(&["status", "--porcelain"]).unwrap().trim(), "", "the worktrees' folder is not something to commit");
        // the same agent again: git refuses, and says why
        assert!(make(&dir, "Front", "abcdef99").unwrap_err().starts_with("O git não criou o ambiente"));
        // outside a repository
        let plain = std::env::temp_dir().join(format!("mago-env-plain-{}", new_id()));
        fs::create_dir_all(&plain).unwrap();
        assert!(make(&plain, "Front", "abcdef99").unwrap_err().contains("não está num repositório git"));
        let _ = git(&["worktree", "remove", "--force", &path.to_string_lossy()]);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&plain);
    }
}
