//! Floors: a workspace under another, alone on its canvas or with its own git worktree (a checkout
//! of the parent's repository on a branch of its own). Git runs in threads that report back here.

use crate::prelude::*;
use crate::ide::git;
use std::path::Path;

/// What a thread working for the workspaces reports.
pub(crate) enum SpaceJob {
    Note(String),
    /// The floor whose worktree is ready, or why there is none.
    Floor(Result<Space, String>),
}

/// A branch name made from a floor's name: lowercase, words joined by dashes.
pub(crate) fn branch_from(name: &str) -> String {
    let words: Vec<String> = name.split(|c: char| !c.is_alphanumeric() && c != '/' && c != '_').filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
    words.join("-")
}

/// `git worktree add` with the refusals said the way the person can act on them: the branch is
/// taken when it exists, else made from HEAD. The folder goes under `floors`, named after it.
pub(crate) fn add_worktree(repo: &Path, branch: &str, floors: &Path) -> Result<Worktree, String> {
    let not_repo = || format!("{} não é um repositório Git. Rode git init lá, ou aponte o espaço de trabalho para um repositório.", repo.display());
    if !repo.is_dir() {
        return Err(not_repo());
    }
    git(repo, &["version"]).map_err(|_| "O Git não pode ser usado nesta máquina, então um andar não pode ter seu próprio checkout. Instale o Git e tente de novo.".to_string())?;
    let top = git(repo, &["rev-parse", "--show-toplevel"]).map_err(|_| not_repo())?;
    git(repo, &["rev-parse", "--verify", "--quiet", "HEAD"]).map_err(|_| format!("{} ainda não tem commits. Faça um primeiro commit e então um andar poderá ramificar dele.", repo.display()))?;
    git(repo, &["check-ref-format", "--branch", branch]).map_err(|_| format!("\"{branch}\" não é um nome de branch válido no Git. Evite espaços e os caracteres ~ ^ : ? *"))?;
    let base = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default().trim().to_string();
    // the checkouts git knows, one block each: a branch open in one can't open in another
    let listed = git(repo, &["worktree", "list", "--porcelain"]).unwrap_or_default().replace("\r\n", "\n");
    let checkouts: Vec<&str> = listed.split("\n\n").collect();
    let line = |block: &str, key: &str| block.lines().find_map(|l| l.strip_prefix(key)).unwrap_or_default().to_string();
    if let Some(holder) = checkouts.iter().find(|b| b.lines().any(|l| l == format!("branch refs/heads/{branch}"))) {
        let at = line(holder, "worktree ");
        return Err(if holder.lines().any(|l| l.starts_with("prunable")) {
            format!("A branch {branch} ainda está presa a um checkout cuja pasta não existe mais ({at}). Rode git worktree prune em {} e tente de novo.", repo.display())
        } else {
            format!("A branch {branch} já está aberta em {at}. Escolha outra branch para o andar.")
        });
    }
    let leaf = branch.replace(['/', '\\'], "-");
    // a folder deleted by hand stays registered in git until a prune: its name is skipped too
    let registered = |p: &Path| checkouts.iter().any(|b| Path::new(&line(b, "worktree ")) == p);
    let path = (1..).map(|n| floors.join(if n == 1 { leaf.clone() } else { format!("{leaf}-{n}") })).find(|p| !p.exists() && !registered(p)).unwrap_or_default();
    fs::create_dir_all(floors).map_err(|e| format!("Não foi possível usar {} para este andar: {e}", floors.display()))?;
    let at = path.to_string_lossy().to_string();
    let exists = git(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok();
    let args = if exists { vec!["worktree", "add", &at, branch] } else { vec!["worktree", "add", "-b", branch, &at] };
    git(repo, &args).map_err(|e| format!("O Git não conseguiu criar o checkout: {e}"))?;
    Ok(Worktree { repo: top.trim().into(), path: at, branch: branch.into(), base })
}

/// Removes a floor's worktree once its sessions let go of the folder (on Windows a shell still in
/// it holds it a moment), never with `--force`: changes made meanwhile keep it. Only this one:
/// git takes it off its list even when the folder is already gone, and other checkouts it lists
/// (one on a drive not plugged in) stay.
pub(crate) fn remove_worktree(worktree: &Worktree) -> Result<(), String> {
    let mut result = Ok(());
    for _ in 0..10 {
        result = git(Path::new(&worktree.repo), &["worktree", "remove", &worktree.path]).map(|_| ());
        if result.is_ok() || !Path::new(&worktree.path).exists() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    result
}

impl DrawingApp {
    /// Where floors' worktrees go: beside the main drawing, outside any repository.
    fn floors_dir(&self) -> Option<PathBuf> {
        Some(self.spaces_home.as_ref()?.with_file_name("andares"))
    }

    /// Novo Andar under `parent`: shown at once without isolation; with it, once git made its
    /// worktree (`space_jobs` takes it from there). An error comes back for the dialog.
    pub(crate) fn create_floor(&mut self, parent: &str, name: &str, isolate: bool, branch: &str) -> Result<(), String> {
        let Some(base) = self.spaces.spaces.iter().find(|s| s.id == parent && s.parent.is_none()).cloned() else { return Err("Esse espaço de trabalho não existe mais.".into()) };
        let floor = Space { id: new_id(), name: name.trim().into(), color: base.color, root: base.root.clone(), view: None, parent: Some(parent.into()), worktree: None };
        if !isolate {
            self.add_floor(floor);
            return Ok(());
        }
        if base.root.trim().is_empty() {
            return Err("Este espaço de trabalho não tem diretório de trabalho, então não há o que extrair. Defina um em Editar Espaço de Trabalho primeiro.".into());
        }
        let floors = self.floors_dir().ok_or("Sem pasta de dados para guardar o checkout do andar.")?;
        let branch = match branch.trim() {
            "" => branch_from(name),
            typed => typed.to_string(),
        };
        let repo = match base.root.trim().strip_prefix('~') {
            Some(rest) => std::env::home_dir().unwrap_or_default().join(rest.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(base.root.trim()),
        };
        let tx = self.space_events.0.clone();
        std::thread::spawn(move || {
            let made = add_worktree(&repo, &branch, &floors).map(|worktree| Space { root: worktree.path.clone(), worktree: Some(worktree), ..floor });
            let _ = tx.send(SpaceJob::Floor(made));
        });
        Ok(())
    }

    /// A new floor goes after its parent's last floor, and shows.
    fn add_floor(&mut self, floor: Space) {
        let parent = floor.parent.clone();
        let after = self.spaces.spaces.iter().rposition(|s| s.id == *parent.as_ref().unwrap_or(&String::new()) || s.parent == parent);
        let id = floor.id.clone();
        self.spaces.spaces.insert(after.map_or(self.spaces.spaces.len(), |i| i + 1), floor);
        self.switch_space(&id);
    }

    /// What the git threads said: a floor ready (or why not), a worktree that stayed.
    pub(crate) fn space_jobs(&mut self) {
        let jobs: Vec<SpaceJob> = self.space_events.1.try_iter().collect();
        for job in jobs {
            match job {
                SpaceJob::Note(text) => self.toast(text),
                SpaceJob::Floor(Ok(floor)) => {
                    if matches!(self.space_dialog, Some(spaces::SpaceDialog::Floor { .. })) {
                        self.space_dialog = None;
                    }
                    self.add_floor(floor);
                }
                SpaceJob::Floor(Err(why)) => match &mut self.space_dialog {
                    Some(spaces::SpaceDialog::Floor { busy, error, .. }) => (*busy, *error) = (false, Some(why)),
                    _ => self.toast(format!("Não foi possível criar o andar. {why}")),
                },
            }
        }
    }

    /// Before a floor with a worktree goes: changes not committed there keep it (the reference
    /// deletes them with it; here they are someone's work). None = it may go.
    pub(crate) fn floor_kept(&self, id: &str) -> Option<String> {
        let worktree = self.spaces.spaces.iter().find(|s| s.id == id)?.worktree.as_ref()?;
        let path = Path::new(&worktree.path);
        match git(path, &["status", "--porcelain"]) {
            Ok(changes) if changes.trim().is_empty() => None,
            Ok(_) => Some(format!("{} tem alterações sem commit: faça commit ou descarte antes de apagar o andar.", worktree.path)),
            Err(_) if !path.exists() => None,
            Err(e) => Some(format!("Não foi possível conferir {}: {e}", worktree.path)),
        }
    }

    /// The sessions of a drawing that goes, ended now: its folder can be removed after.
    pub(crate) fn end_sessions(&mut self, scene: &Scene) {
        let ids: std::collections::HashSet<&str> = scene.elements.iter().map(|e| e.id.as_str()).collect();
        self.terminals.retain(|id, _| !ids.contains(id.as_str()));
        self.ides.retain(|id, _| !ids.contains(id.as_str()));
    }

    /// The worktree of a floor that went, removed in a thread; a folder that stays is told.
    pub(crate) fn drop_worktree(&self, worktree: Worktree) {
        let tx = self.space_events.0.clone();
        std::thread::spawn(move || {
            if let Err(why) = remove_worktree(&worktree) {
                let _ = tx.send(SpaceJob::Note(format!("O andar foi excluído, mas a pasta {} ficou: {why}", worktree.path)));
            }
        });
    }
}
