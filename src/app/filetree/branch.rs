//! The branch chip of a tree's header: a menu of the local branches (the one checked out ticked) and the switch to another.

use crate::app::filetree::FileTreeSpec;
use crate::app::filetree::menu::{Lead, Row, menu_row};
use crate::app::filetree::ui::Act;
use crate::ide::run_git;
use crate::prelude::*;
use std::path::Path;

/// The local branches of the repository `root` is in, by name.
pub(crate) fn local_branches(root: &Path) -> Result<Vec<String>, String> {
    let out = run_git(root, &["for-each-ref", "--format=%(refname:short)", "--sort=refname", "refs/heads"])?;
    Ok(out.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
}

/// `git checkout branch`, with the reason in words when git refuses: changes that the switch would overwrite.
pub(crate) fn checkout(root: &Path, branch: &str) -> Result<String, String> {
    let branches = local_branches(root)?;
    if !branches.iter().any(|b| b == branch) {
        return Err(format!("O ramo \"{branch}\" não existe neste repositório."));
    }
    let current = run_git(root, &["symbolic-ref", "--short", "-q", "HEAD"]).map(|b| b.trim().to_string()).unwrap_or_default();
    if current == branch {
        return Ok(format!("Já está no ramo {branch}."));
    }
    run_git(root, &["checkout", branch, "--"]).map_err(|why| {
        if why.contains("overwritten") || why.contains("local changes") {
            format!("Não foi possível trocar para {branch}: há mudanças locais que seriam sobrescritas. Faça commit, guarde (stash) ou descarte antes.")
        } else {
            format!("Não foi possível trocar para {branch}: {}", why.lines().next().unwrap_or_default())
        }
    })?;
    Ok(format!("Ramo trocado para {branch}."))
}

/// The chip's menu: one row per local branch, the current one ticked.
pub(crate) fn branch_menu(ui: &mut egui::Ui, branches: &[String], current: &str, acts: &mut Vec<Act>) {
    ui.set_width(200.0);
    ui.spacing_mut().item_spacing.y = 0.0;
    if branches.is_empty() {
        menu_row(ui, Row { on: false, ..Row::new(None, "Nenhum ramo local") });
    }
    for branch in branches {
        if menu_row(ui, Row { lead: Lead::Tick(branch == current), ..Row::new(None, branch) }).clicked() {
            acts.push(Act::Branch(branch.clone()));
            ui.close();
        }
    }
}

impl DrawingApp {
    /// Switches tree `id`'s repository to `branch`; what git said (or the refusal) is the answer.
    pub(crate) fn ft_checkout(&mut self, id: &str, branch: &str) -> Result<String, String> {
        let root = self.ft_spec(id).map(FileTreeSpec::root_path).ok_or("árvore não encontrada")?;
        let done = checkout(&root, branch)?;
        self.ft_refresh(id);
        Ok(done)
    }

    /// The `file_tree` actions `branches` (the local ones, the current marked with `*`) and `checkout` (`name`).
    pub(crate) fn act_tree_branch(&mut self, id: &str, action: &str, name: &str) -> Result<String, String> {
        if action == "checkout" {
            return self.ft_checkout(id, name.trim());
        }
        let root = self.ft_spec(id).map(FileTreeSpec::root_path).ok_or("árvore não encontrada")?;
        let current = run_git(&root, &["symbolic-ref", "--short", "-q", "HEAD"]).map(|b| b.trim().to_string()).unwrap_or_default();
        let branches = local_branches(&root)?;
        Ok(branches.iter().map(|b| format!("{} {b}", if *b == current { "*" } else { " " })).collect::<Vec<_>>().join("\n"))
    }
}
