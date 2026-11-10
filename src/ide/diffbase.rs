//! The diff's base: another revision (a branch, a tag, a commit) instead of HEAD. The left side of
//! every diff shows the file there and the Commit panel lists what differs from it to the working
//! copy. Resolving and listing run on a thread, once per change of base (and per git refresh).

use super::git::{Change, Status, git};
use super::*;

/// The reason, in the hint, why what only makes sense against HEAD is switched off.
pub(super) const ONLY_HEAD: &str = "Só vale com a base no HEAD: esvazie o campo Comparar com, no painel de commit";

/// A revision asked for and its answer: the files that differ and the open diffs' left side.
type Answer = (String, Result<(Vec<Change>, Vec<(PathBuf, String)>), String>);

#[derive(Default)]
pub struct Base {
    /// the revision as typed and accepted; empty is HEAD
    pub name: String,
    /// the Commit panel's field
    pub input: String,
    /// what differs between `name` and the working copy
    pub changes: Vec<Change>,
    rx: Option<Receiver<Answer>>,
}

impl Base {
    pub fn rev(&self) -> &str {
        if self.name.is_empty() { "HEAD" } else { &self.name }
    }

    pub fn is_head(&self) -> bool {
        self.name.is_empty()
    }

    /// No base change waiting for its answer.
    pub fn rx_idle(&self) -> bool {
        self.rx.is_none()
    }
}

/// `path` as it is at `rev` (None when it does not exist there).
pub fn rev_text(root: &Path, rev: &str, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
    git(root, &["show", &format!("{rev}:{rel}")]).ok().map(|s| s.replace("\r\n", "\n"))
}

/// `rev` must name a commit; one that looks like an option never reaches git.
pub fn resolve(root: &Path, rev: &str) -> Result<(), String> {
    let bad = || format!("«{rev}» não é um ramo, uma etiqueta nem um commit deste repositório");
    if rev.starts_with('-') {
        return Err(bad());
    }
    git(root, &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")]).map(|_| ()).map_err(|_| bad())
}

/// What differs between `rev` and the working copy (`git diff --name-status`), plus the files git
/// does not track yet: a file that only one side has is in it, added or removed.
pub fn changes_since(root: &Path, rev: &str) -> Result<Vec<Change>, String> {
    resolve(root, rev)?;
    let out = git(root, &["diff", "--name-status", "--no-renames", "-z", rev, "--"])?;
    let mut v = vec![];
    let mut it = out.split('\0');
    while let (Some(st), Some(p)) = (it.next(), it.next()) {
        let status = match st.as_bytes().first() {
            Some(b'A') => Status::Added,
            Some(b'D') => Status::Deleted,
            Some(b'U') => Status::Conflict,
            _ => Status::Modified,
        };
        v.push(Change { path: root.join(p), status });
    }
    for p in git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?.split('\0').filter(|p| !p.is_empty()) {
        v.push(Change { path: root.join(p), status: Status::Untracked });
    }
    Ok(v)
}

impl Ide {
    /// The diffs compare with `rev` from now on (empty: HEAD). A revision that does not exist
    /// leaves the base as it was and says so on the status line (`base_poll`).
    pub(crate) fn set_diff_base(&mut self, rev: &str) {
        let Some(root) = self.root.clone() else { return };
        let rev = rev.trim().to_string();
        let paths: Vec<PathBuf> = self.tabs.iter().filter(|t| t.diff.as_ref().is_some_and(|d| d.claude.is_none())).map(|t| t.path.clone()).collect();
        let (tx, rx) = std::sync::mpsc::channel();
        let (ctx, asked) = (self.ctx.clone(), rev.clone());
        std::thread::spawn(move || {
            let at = if rev.is_empty() { "HEAD" } else { rev.as_str() };
            let list = if rev.is_empty() { Ok(vec![]) } else { changes_since(&root, &rev) };
            let res = list.map(|list| {
                let texts = paths.into_iter().map(|p| {
                    let text = rev_text(&root, at, &p).unwrap_or_default();
                    (p, text)
                });
                (list, texts.collect())
            });
            let _ = tx.send((rev, res));
            ctx.request_repaint();
        });
        self.git.base.input = asked;
        self.git.base.rx = Some(rx);
    }

    /// The answer of `set_diff_base`, once it comes.
    pub(crate) fn base_poll(&mut self) {
        let Some(Ok((rev, res))) = self.git.base.rx.as_ref().map(|rx| rx.try_recv()) else { return };
        self.git.base.rx = None;
        match res {
            Ok((changes, texts)) => {
                // ponytail: a diff opened while this ran keeps the old left side until the next change of base
                for tab in &mut self.tabs {
                    if let Some(d) = tab.diff.as_mut().filter(|d| d.claude.is_none())
                        && let Some((_, text)) = texts.iter().find(|(p, _)| *p == tab.path)
                    {
                        (d.base, d.seen) = (text.clone(), 0);
                    }
                }
                (self.git.base.name, self.git.base.input, self.git.base.changes) = (rev.clone(), rev, changes);
                self.status = format!("Diff contra {}", self.git.base.rev());
            }
            Err(e) => {
                self.git.base.input = self.git.base.name.clone();
                self.notify(format!("{e}; a base continua {}", self.git.base.rev()));
            }
        }
    }

    /// The MCP's `ide_git` diff: the Commit panel open, its diff against `base` (empty: HEAD)
    /// shown `mode` (side, unified; empty keeps it).
    pub(crate) fn agent_diff(&mut self, base: &str, mode: &str) -> Result<String, String> {
        let root = self.root.clone().ok_or("o IDE não está numa pasta")?;
        let unified = match mode {
            "" => self.unified_diff,
            "side" => false,
            "unified" => true,
            other => return Err(format!("modo desconhecido: {other} (use side ou unified)")),
        };
        let base = base.trim();
        if !base.is_empty() {
            resolve(&root, base)?;
        }
        self.unified_diff = unified;
        self.left = Some(Left::Commit);
        self.set_diff_base(base);
        self.git_refresh();
        Ok(format!(
            "Painel de commit aberto, diff contra {} ({}).",
            if base.is_empty() { "HEAD" } else { base },
            if unified { "unificado" } else { "lado a lado" }
        ))
    }

    /// The Commit panel's "Comparar com" field: Enter sets the base.
    pub(super) fn base_field(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Comparar com").color(t::SECONDARY));
            let w = (ui.available_width() - 10.0).max(60.0);
            let field = ui
                .add(TextEdit::singleline(&mut self.git.base.input).hint_text("HEAD").desired_width(w))
                .on_hover_text("Ramo, etiqueta ou commit (main, v0.0.8, a1b2c3d); vazio é o HEAD. Enter aplica");
            if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let rev = self.git.base.input.clone();
                self.set_diff_base(&rev);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository with a.txt and b.txt on the branch `antes`, then a changed, b removed and c
    /// added on the current branch, d untracked.
    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("magoagent-base-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q"]).unwrap();
        for (k, v) in [("user.name", "t"), ("user.email", "t@t"), ("commit.gpgsign", "false")] {
            git(&dir, &["config", k, v]).unwrap();
        }
        std::fs::write(dir.join("a.txt"), "um\n").unwrap();
        std::fs::write(dir.join("b.txt"), "b\n").unwrap();
        git(&dir, &["add", "."]).unwrap();
        git(&dir, &["commit", "-qm", "base"]).unwrap();
        git(&dir, &["branch", "antes"]).unwrap();
        std::fs::write(dir.join("a.txt"), "UM\n").unwrap();
        std::fs::remove_file(dir.join("b.txt")).unwrap();
        std::fs::write(dir.join("c.txt"), "c\n").unwrap();
        git(&dir, &["add", "-A"]).unwrap();
        git(&dir, &["commit", "-qm", "dois"]).unwrap();
        std::fs::write(dir.join("d.txt"), "d\n").unwrap();
        dir
    }

    fn wait(ide: &mut Ide) {
        let start = Instant::now();
        while ide.git.base.rx.is_some() && start.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(20));
            ide.base_poll();
        }
    }

    #[test]
    fn the_files_against_a_branch_include_changed_new_and_removed() {
        let dir = repo("list");
        let mut v: Vec<_> = changes_since(&dir, "antes").unwrap().into_iter().map(|c| (c.path, c.status)).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            v,
            vec![
                (dir.join("a.txt"), Status::Modified),
                (dir.join("b.txt"), Status::Deleted),
                (dir.join("c.txt"), Status::Added),
                (dir.join("d.txt"), Status::Untracked),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_as_it_is_at_a_revision() {
        let dir = repo("text");
        assert_eq!(rev_text(&dir, "antes", &dir.join("a.txt")).as_deref(), Some("um\n"));
        assert_eq!(rev_text(&dir, "HEAD", &dir.join("a.txt")).as_deref(), Some("UM\n"));
        assert_eq!(rev_text(&dir, "antes", &dir.join("c.txt")), None, "c.txt did not exist then");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_invalid_revision_is_an_error_and_keeps_the_base() {
        let dir = repo("invalid");
        assert!(resolve(&dir, "nao-existe").is_err());
        assert!(resolve(&dir, "--output=x").is_err(), "an option never reaches git");
        let mut ide = Ide::new(egui::Context::default(), "b", &dir.to_string_lossy());
        ide.open_diff(dir.join("a.txt"));
        assert_eq!(ide.tabs[ide.active].diff.as_ref().unwrap().base, "UM\n");
        // against the branch: the panel lists it, the open diff's left side is the file there
        ide.set_diff_base("antes");
        wait(&mut ide);
        assert_eq!((ide.git.base.name.as_str(), ide.git.base.changes.len()), ("antes", 4));
        assert_eq!(ide.tabs[ide.active].diff.as_ref().unwrap().base, "um\n");
        // a name that does not exist: said on the status line, the base stays
        ide.set_diff_base("nao-existe");
        wait(&mut ide);
        assert_eq!((ide.git.base.name.as_str(), ide.git.base.input.as_str()), ("antes", "antes"));
        assert!(ide.status.contains("nao-existe") && ide.status.contains("continua antes"), "{}", ide.status);
        // empty is HEAD again
        ide.set_diff_base("");
        wait(&mut ide);
        assert!(ide.git.base.is_head());
        assert_eq!(ide.tabs[ide.active].diff.as_ref().unwrap().base, "UM\n");
        drop(ide);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
