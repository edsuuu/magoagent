use super::*;

impl Ide {
    /// Starts `git blame` for the active file unless it has an answer; one run at a time. The answer
    /// goes to `blame` (empty outside git or for an uncommitted file).
    pub(crate) fn blame_request(&mut self) {
        let Some(tab) = self.tabs.get(self.active) else { return };
        if !self.git.repo
            || self.blame_rx.is_some()
            || self.blame.contains_key(&tab.path)
            || tab.lang.kind != highlight::Kind::Code
            || tab.diff.is_some()
            || tab.sheet.is_some()
        {
            return;
        }
        let (Some(root), path) = (self.root.clone(), tab.path.clone()) else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let rel = path.strip_prefix(&root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            let lines = git(&root, &["blame", "--porcelain", "--", rel.as_str()]).map(|out| parse_blame(&out)).unwrap_or_default();
            let _ = tx.send((path, lines));
            ctx.request_repaint();
        });
        self.blame_rx = Some(rx);
    }

    pub(crate) fn git_refresh(&mut self) {
        let Some(root) = self.root.clone() else { return };
        // another base: its list follows the working copy too (unless a new base is on its way)
        if !self.git.base.is_head() && self.git.base.rx_idle() {
            let rev = self.git.base.name.clone();
            self.set_diff_base(&rev);
        }
        if self.git.busy.is_some() {
            return; // one channel for both: a refresh now would drop the running command's Done (Done refreshes anyway)
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let st = git(&root, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
            let msg = match st {
                Ok(out) => Msg::Status {
                    repo: true,
                    changes: parse_status(&root, &out),
                    branches: git(&root, &["branch", "--format=%(refname:short)"]).unwrap_or_default().lines().map(String::from).collect(),
                    branch: git(&root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default().trim().to_string(),
                    snap: Box::new(Snapshot::read(&root)),
                },
                Err(_) => Msg::Status { repo: false, changes: vec![], branches: vec![], branch: String::new(), snap: Default::default() },
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
        self.git.rx = Some(rx);
    }

    /// Runs git commands in order on a background thread, stopping at the first failure.
    pub(crate) fn git_run(&mut self, label: &str, cmds: Vec<Vec<String>>) {
        let Some(root) = self.root.clone() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let (ctx, label) = (self.ctx.clone(), label.to_string());
        self.git.busy = Some(format!("{label}…"));
        std::thread::spawn(move || {
            let mut res = Ok(String::new());
            for c in &cmds {
                let args: Vec<&str> = c.iter().map(String::as_str).collect();
                res = git(&root, &args);
                if res.is_err() {
                    break;
                }
            }
            let _ = tx.send(Msg::Done(label, res));
            ctx.request_repaint();
        });
        self.git.rx = Some(rx);
    }

    /// An agent's commit (the MCP's ide_git): the tabs saved first; `files` (relative to the
    /// project) go in alone, else only the changes of files git already tracks (`add -u`). Back:
    /// what goes in and the untracked files left out, for the agent to name them if they belong.
    pub(crate) fn agent_commit(&mut self, message: &str, files: &[String]) -> Result<String, String> {
        let root = self.root.clone().ok_or("o IDE não está numa pasta")?;
        if let Some(busy) = &self.git.busy {
            return Err(format!("o git do IDE está ocupado ({busy}): espere terminar"));
        }
        self.save_all();
        if let Some(tab) = self.tabs.iter().find(|t| t.is_text() && t.dirty) {
            return Err(format!("{} não pôde ser salvo (mudou no disco ou a gravação falhou): resolva no IDE antes do commit", file_name(&tab.path)));
        }
        let rel = |p: &Path| p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/");
        let (mut tracked, mut untracked) = (vec![], vec![]);
        for c in parse_status(&root, &git(&root, &["status", "--porcelain=v1", "-z", "--untracked-files=all"])?) {
            (if c.status == Status::Untracked { &mut untracked } else { &mut tracked }).push(rel(&c.path));
        }
        let s = |v: &[&str]| v.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        let (cmds, going) = if files.is_empty() {
            (vec![s(&["add", "-u"]), s(&["commit", "-m", message])], tracked)
        } else {
            // exactly the files named, the way the Commit panel commits its ticked ones
            let named: Vec<String> = files.iter().map(|f| f.trim().replace('\\', "/").trim_start_matches("./").to_string()).collect();
            if let Some(f) = named.iter().find(|f| !tracked.contains(f) && !untracked.contains(f)) {
                return Err(format!("{f} não tem mudanças no projeto (o caminho é relativo à pasta do IDE)"));
            }
            untracked.retain(|f| !named.contains(f));
            let (mut add, mut commit) = (s(&["add", "-A", "--"]), s(&["commit", "-m", message, "--"]));
            add.extend(named.iter().cloned());
            commit.extend(named.iter().cloned());
            (vec![add, commit], named)
        };
        let list = |v: &[String]| if v.is_empty() { "nenhum".to_string() } else { v.join(", ") };
        if going.is_empty() {
            return Err(format!("nada versionado mudou; não versionados (nomeie em files para incluir): {}", list(&untracked)));
        }
        self.git_run("Commit", cmds);
        Ok(format!(
            "Commit começou no IDE com: {}. Não versionados que ficaram de fora (nomeie em files para incluir): {}. O resultado aparece no IDE.",
            list(&going),
            list(&untracked)
        ))
    }

    /// `git_run` for a commit with some blocks left out (`commit_parts`), then `after` (a push).
    pub(crate) fn git_commit_parts(&mut self, label: &str, whole: Vec<String>, partial: Vec<(String, String)>, commit: Vec<String>, after: Vec<Vec<String>>) {
        let Some(root) = self.root.clone() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let (ctx, label) = (self.ctx.clone(), label.to_string());
        self.git.busy = Some(format!("{label}…"));
        std::thread::spawn(move || {
            let mut res = commit_parts(&root, &whole, &partial, &commit);
            for c in &after {
                if res.is_err() {
                    break;
                }
                let args: Vec<&str> = c.iter().map(String::as_str).collect();
                res = git(&root, &args);
            }
            let _ = tx.send(Msg::Done(label, res));
            ctx.request_repaint();
        });
        self.git.rx = Some(rx);
    }

    pub(crate) fn git_poll(&mut self) {
        self.base_poll();
        self.history_poll();
        let Some(rx) = &self.git.rx else { return };
        let Ok(msg) = rx.try_recv() else { return };
        self.git.rx = None;
        match msg {
            Msg::Status { repo, changes, branches, branch, snap } => {
                self.git.repo = repo;
                self.git.status.clear();
                if let Some(root) = &self.root {
                    for c in &changes {
                        self.git.status.insert(path_key(&c.path), c.status);
                        // PhpStorm also tints directories that contain changes
                        let mut p = c.path.parent();
                        while let Some(d) = p.filter(|d| d.starts_with(root) && *d != root.as_path()) {
                            self.git.status.entry(path_key(d)).or_insert(Status::Modified);
                            p = d.parent();
                        }
                    }
                }
                self.git.changes = changes;
                self.git.branches = branches;
                self.git.snap = *snap;
                if !branch.is_empty() {
                    self.branch = branch;
                }
                if self.left == Some(Left::History) {
                    self.history_load();
                }
            }
            Msg::Done(label, res) => {
                self.git.busy = None;
                match res {
                    Ok(_) => {
                        self.notify(format!("{label}: concluído"));
                        if label.starts_with("Commit") {
                            self.git.message.clear();
                            self.git.amend = false;
                        }
                        let rev = self.git.base.rev().to_string();
                        for tab in &mut self.tabs {
                            tab.head = None; // HEAD moved: reload the diff base
                            // an open diff compares with the new HEAD too: the blocks just committed leave it
                            if let (Some(root), Some(d)) = (&self.root, tab.diff.as_mut().filter(|d| d.is_plain())) {
                                d.base = rev_text(root, &rev, &tab.path).unwrap_or_default();
                                d.seen = 0;
                            }
                        }
                    }
                    Err(e) => self.notify(format!("{label} falhou: {}", explain(&e))),
                }
                self.git_refresh();
            }
        }
    }

    pub(crate) fn file_status(&self, p: &Path) -> Option<Status> {
        self.git.status.get(&path_key(p)).copied()
    }

    /// The file's own tab, whose text the diff's right side edits; opened again when it was closed.
    pub(crate) fn buffer_tab(&mut self, path: &Path) -> Option<usize> {
        if let Some(i) = self.tabs.iter().position(|t| t.is_text() && t.path == path) {
            return Some(i);
        }
        let (at, n) = (self.active, self.tabs.len());
        self.open_file(path.to_path_buf(), None);
        self.active = at;
        (self.tabs.len() > n).then_some(n)
    }

    pub(crate) fn open_diff(&mut self, path: PathBuf) {
        let Some(root) = self.root.clone() else { return };
        if let Some(i) = self.tabs.iter().position(|t| t.path == path && t.diff.as_ref().is_some_and(|d| d.sides.is_none())) {
            self.active = i;
            return;
        }
        let Some(buffer) = self.buffer_tab(&path) else { return };
        let (old, new) = (rev_text(&root, self.git.base.rev(), &path).unwrap_or_default(), self.tabs[buffer].text.clone());
        let mut tab = Tab::new(path, String::new(), false);
        tab.diff = Some(Box::new(Diff::new(&old, &new)));
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }
}
