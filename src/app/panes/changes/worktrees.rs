//! The worktrees of the repositories Alterações shows (`git worktree list`): the main checkout and the agents' own
//! (`.magoagent/worktrees/`, maestro/environments.rs). Picking one shows its changes alone, read by git on the panel's
//! thread whether or not a file tree on the canvas shows that folder; "Todos" is the trees' folders, as before.

use super::{folder_name, key};
use crate::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Worktree {
    pub(crate) path: PathBuf,
    /// Its branch, or the start of the commit it is on (detached).
    pub(crate) branch: String,
    /// The repository's own checkout (the first one git lists).
    pub(crate) main: bool,
}

impl Worktree {
    /// "pasta · ramo".
    pub(crate) fn label(&self) -> String {
        let name = folder_name(&self.path);

        if self.branch.is_empty() { name } else { format!("{name} · {}", self.branch) }
    }
}

/// `git worktree list --porcelain`: a record per worktree, apart by a blank line; a bare repository and a worktree whose
/// folder is gone (prunable) are left out. Git writes the paths with `/`, on Windows too: the system's own here.
pub(crate) fn parse_worktrees(out: &str) -> Vec<Worktree> {
    let mut list = Vec::new();
    let mut records = 0;
    let (mut path, mut branch, mut head, mut skip) = (None::<String>, String::new(), String::new(), false);

    let mut close = |path: &mut Option<String>, branch: &mut String, head: &mut String, skip: &mut bool| {
        if let Some(found) = path.take() {
            if !*skip {
                let branch = if branch.is_empty() { head.clone() } else { std::mem::take(branch) };

                list.push(Worktree { path: PathBuf::from(found.replace('/', std::path::MAIN_SEPARATOR_STR)), branch, main: records == 0 });
            }

            records += 1;
        }

        branch.clear();
        head.clear();
        *skip = false;
    };

    for line in out.lines() {
        if let Some(found) = line.strip_prefix("worktree ") {
            close(&mut path, &mut branch, &mut head, &mut skip);
            path = Some(found.to_string());
        } else if let Some(name) = line.strip_prefix("branch ") {
            branch = name.trim_start_matches("refs/heads/").to_string();
        } else if let Some(commit) = line.strip_prefix("HEAD ") {
            head = commit.chars().take(7).collect();
        } else if line == "bare" || line.starts_with("prunable") {
            skip = true;
        }
    }

    close(&mut path, &mut branch, &mut head, &mut skip);
    list
}

/// Every worktree of the repositories of `folders`, once each, in the order found.
pub(crate) fn read_worktrees(folders: &[PathBuf]) -> Vec<Worktree> {
    let mut list: Vec<Worktree> = Vec::new();

    for folder in folders {
        let Ok(out) = crate::editor::run_git(folder, &["worktree", "list", "--porcelain"]) else { continue };

        for worktree in parse_worktrees(&out) {
            if !list.iter().any(|known| key(&known.path) == key(&worktree.path)) {
                list.push(worktree);
            }
        }
    }

    list
}

/// The worktree `wanted` names, any case: its folder's name, its branch (with or without `mago/`), its label or its path.
pub(crate) fn find_worktree<'a>(list: &'a [Worktree], wanted: &str) -> Option<&'a Worktree> {
    let wanted = wanted.trim().to_lowercase();
    let wanted_path = key(std::path::Path::new(&wanted));

    let names = |w: &Worktree| {
        let branch = w.branch.to_lowercase();

        [folder_name(&w.path).to_lowercase(), branch.trim_start_matches("mago/").to_string(), branch, w.label().to_lowercase()]
    };

    list.iter().find(|w| key(&w.path).trim_end_matches('/') == wanted_path.trim_end_matches('/')).or_else(|| list.iter().find(|w| names(w).contains(&wanted)))
}

impl DrawingApp {
    /// The folders whose repositories' worktrees the picker lists: the trees' and the picked worktree.
    pub(crate) fn worktree_folders(&self) -> Vec<PathBuf> {
        let mut folders: Vec<PathBuf> = self.change_groups().into_iter().map(|g| g.root).collect();

        folders.extend(self.panes.changes.worktree.clone());
        folders
    }

    /// The picker over the list: "Todos" (the trees' folders) or a worktree, as "pasta · ramo".
    pub(super) fn worktree_switcher(&mut self, ui: &mut egui::Ui) {
        let list = self.panes.changes.worktrees.clone();
        let current = self.panes.changes.worktree.clone();

        if list.is_empty() && current.is_none() {
            return;
        }

        let shown = current
            .as_ref()
            .map_or("Todos".to_string(), |path| list.iter().find(|w| key(&w.path) == key(path)).map_or_else(|| folder_name(path), Worktree::label));

        let mut picked = None;
        let combo = egui::ComboBox::from_id_salt("changes_worktree").width(ui.available_width()).selected_text(format!("Worktree: {shown}")).truncate();

        combo.show_ui(ui, |ui| {
            if ui.selectable_label(current.is_none(), "Todos").on_hover_text("As alterações de todas as Árvores do canvas").clicked() {
                picked = Some(None);
            }

            for worktree in &list {
                let on = current.as_ref().is_some_and(|path| key(path) == key(&worktree.path));
                let text = if worktree.main { format!("{} (principal)", worktree.label()) } else { worktree.label() };

                if ui.selectable_label(on, text).on_hover_text(worktree.path.to_string_lossy()).clicked() {
                    picked = Some(Some(worktree.path.clone()));
                }
            }
        });

        if let Some(choice) = picked.filter(|_| super::live(ui)) {
            self.set_worktree(choice);
        }
    }

    /// Shows `path`'s changes (none: the trees'): read again at once, with nothing picked.
    pub(crate) fn set_worktree(&mut self, path: Option<PathBuf>) {
        let changes = &mut self.panes.changes;

        if changes.worktree.as_deref().map(key) == path.as_deref().map(key) {
            return;
        }

        (changes.worktree, changes.worktree_git, changes.read_at) = (path, None, None);
        (changes.selected, changes.diff, changes.review.picked) = (None, Vec::new(), None);
    }

    /// The MCP's pick: the worktree `wanted` names ("todos": the trees' folders), read now so the answer can say which.
    pub(crate) fn choose_worktree(&mut self, wanted: &str) -> Result<String, String> {
        if matches!(wanted.trim().to_lowercase().as_str(), "todos" | "todas" | "all" | "tudo") {
            self.set_worktree(None);

            return Ok("Alterações de todas as Árvores.".into());
        }

        let list = read_worktrees(&self.worktree_folders());

        let Some(found) = find_worktree(&list, wanted).cloned() else {
            let names: Vec<String> = list.iter().map(Worktree::label).collect();

            return Err(if names.is_empty() {
                "Nenhum worktree: as Árvores do canvas não estão num repositório git.".into()
            } else {
                format!("Nenhum worktree {wanted}. Há: {}.", names.join("; "))
            });
        };

        self.panes.changes.worktrees = list;
        self.set_worktree(Some(found.path.clone()));
        Ok(format!("Worktree {} ({}).", found.label(), found.path.display()))
    }

    /// The MCP's list: each worktree with its folder, the picked one marked.
    pub(crate) fn describe_worktrees(&mut self) -> String {
        let list = read_worktrees(&self.worktree_folders());

        self.panes.changes.worktrees = list.clone();

        if list.is_empty() {
            return "Nenhum worktree: as Árvores do canvas não estão num repositório git.".into();
        }

        let current = self.panes.changes.worktree.as_deref().map(key);
        let mut out = format!("Worktrees ({}):", list.len());

        out += &format!("\n{} Todos (as Árvores do canvas)", if current.is_none() { "*" } else { " " });

        for worktree in &list {
            let on = current.as_deref() == Some(key(&worktree.path).as_str());
            let main = if worktree.main { " (principal)" } else { "" };

            out += &format!("\n{} {}{main} — {}", if on { "*" } else { " " }, worktree.label(), worktree.path.display());
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worktree_list_has_the_main_checkout_first_and_leaves_bare_and_gone_ones_out() {
        let out = "worktree /repo\nHEAD 1234567890abcdef\nbranch refs/heads/main\n\n\
                   worktree /repo/.magoagent/worktrees/revisor-ab12cd\nHEAD abcdef1234567890\nbranch refs/heads/mago/revisor-ab12cd\n\n\
                   worktree /tmp/solto\nHEAD fedcba9876543210\ndetached\n\n\
                   worktree /tmp/sumiu\nHEAD 0000000000000000\nbranch refs/heads/velho\nprunable gitdir file points to non-existent location\n";

        let list = parse_worktrees(out);
        let sep = std::path::MAIN_SEPARATOR_STR;
        let path = |p: &str| PathBuf::from(p.replace('/', sep));

        assert_eq!(list.len(), 3, "{list:?}");
        assert_eq!(list[0], Worktree { path: path("/repo"), branch: "main".into(), main: true });
        assert_eq!(list[1].branch, "mago/revisor-ab12cd");
        assert!(!list[1].main);
        assert_eq!((list[2].branch.as_str(), list[2].label()), ("fedcba9", "solto · fedcba9".into()), "detached: the commit");
        // a bare repository's own record is not a checkout
        let bare = parse_worktrees("worktree /srv/repo.git\nbare\n\nworktree /srv/trabalho\nHEAD 1111111\nbranch refs/heads/main\n");

        assert_eq!(bare.len(), 1);
        assert!(!bare[0].main, "the first record was the bare one");
        // picked by name, branch (with or without mago/), path
        assert_eq!(find_worktree(&list, "Revisor-AB12CD").map(|w| w.branch.as_str()), Some("mago/revisor-ab12cd"));
        assert_eq!(find_worktree(&list, "mago/revisor-ab12cd").map(|w| w.main), Some(false));
        assert_eq!(find_worktree(&list, "main").map(|w| w.main), Some(true));
        assert_eq!(find_worktree(&list, &path("/tmp/solto").to_string_lossy()).map(|w| w.branch.as_str()), Some("fedcba9"));
        assert!(find_worktree(&list, "outro").is_none());
    }
}
