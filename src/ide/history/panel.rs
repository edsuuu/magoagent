use super::*;

impl Ide {
    /// Shows the panel and loads the log.
    pub(crate) fn open_history(&mut self) {
        self.left = Some(Left::History);
        self.history_load();
    }

    pub(crate) fn toggle_history(&mut self) {
        if self.left == Some(Left::History) {
            self.left = None;
        } else {
            self.open_history();
        }
    }

    /// Reads the log on a thread; the answer is `history_poll`'s.
    pub(crate) fn history_load(&mut self) {
        let Some(root) = self.root.clone().filter(|_| self.git.repo) else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let (ctx, limit) = (self.ctx.clone(), self.git.history.limit);
        std::thread::spawn(move || {
            let _ = tx.send(Loaded::Log(read_all(&root, limit)));
            ctx.request_repaint();
        });
        self.git.history.rx = Some(rx);
    }

    pub(crate) fn history_poll(&mut self) {
        let h = &mut self.git.history;
        if let Some(Ok(Loaded::Log(res))) = h.rx.as_ref().map(|rx| rx.try_recv()) {
            h.rx = None;
            match res {
                Ok(commits) => {
                    (h.rows, h.lanes) = graph(&commits);
                    (h.commits, h.error) = (commits, String::new());
                }
                Err(e) => h.error = e,
            }
        }
        if let Some(Ok(Loaded::Files(hash, res))) = h.files_rx.as_ref().map(|rx| rx.try_recv()) {
            h.files_rx = None;
            if h.selected.as_deref() == Some(hash.as_str()) {
                match res {
                    Ok(files) => h.files = files,
                    Err(e) => h.error = e,
                }
            }
        }
    }

    /// Picks a commit: its files load on a thread.
    pub(crate) fn history_select(&mut self, hash: &str) {
        let Some(root) = self.root.clone() else { return };
        let Some(commit) = self.git.history.commits.iter().find(|c| c.hash == hash) else { return };
        let (hash, parents) = (commit.hash.clone(), commit.parents.clone());
        self.git.history.selected = Some(hash.clone());
        self.git.history.files.clear();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let commit = Commit { hash: hash.clone(), parents, author: String::new(), date: String::new(), refs: vec![], subject: String::new() };
            let _ = tx.send(Loaded::Files(hash, commit_files(&root, &commit)));
            ctx.request_repaint();
        });
        self.git.history.files_rx = Some(rx);
    }

    /// A commit's change of one file in a diff tab: its first parent on the left, the commit on the
    /// right, both read-only (the Commit window's diff, with no buffer behind it).
    pub(crate) fn open_commit_diff(&mut self, commit: &Commit, rel: &str) -> Result<(), String> {
        let root = self.root.clone().ok_or("o IDE não está numa pasta")?;
        let path = root.join(rel);
        let title = format!("{}: {}", short(&commit.hash), file_name(&path));
        if let Some(i) = self.tabs.iter().position(|t| t.path == path && t.diff.as_ref().is_some_and(|d| d.title.as_deref() == Some(title.as_str()))) {
            self.active = i;
            return Ok(());
        }
        let old = commit.parents.first().and_then(|p| rev_text(&root, p, &path)).unwrap_or_default();
        let new = rev_text(&root, &commit.hash, &path).unwrap_or_default();
        if old.contains('\0') || new.contains('\0') {
            return Err("Arquivo binário: o git não tem texto para comparar.".into());
        }
        let mut diff = Diff::new(&old, &new);
        diff.title = Some(title);
        let left = commit.parents.first().map_or("vazio".to_string(), |p| short(p).to_string());
        diff.sides = Some((format!("{left} (somente leitura)"), format!("{} (somente leitura)", short(&commit.hash))));
        let mut tab = Tab::new(path, String::new(), false);
        tab.diff = Some(Box::new(diff));
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        Ok(())
    }

    /// The MCP's `show`: the panel on the commit `rev`, and its files; with `path`, that file's diff opens.
    pub(crate) fn agent_show(&mut self, rev: &str, path: &str) -> Result<String, String> {
        let root = self.root.clone().ok_or("o IDE não está numa pasta")?;
        let commit = find_commit(&root, rev)?;
        let files = commit_files(&root, &commit)?;
        let rel = path.trim().replace('\\', "/");
        if !rel.is_empty() {
            if !files.iter().any(|(_, f)| *f == rel) {
                return Err(format!("{rel} não mudou em {} (o caminho é relativo à pasta do IDE)", short(&commit.hash)));
            }
            self.open_commit_diff(&commit, &rel)?;
        }
        self.open_history();
        self.git.history.selected = Some(commit.hash.clone());
        self.git.history.files = files.clone();
        let list = files.iter().map(|(s, f)| format!("{} {f}", status_letter(*s))).collect::<Vec<_>>().join("\n");
        Ok(format!("{} {} {}: {}\n{list}", short(&commit.hash), commit.date, commit.author, commit.subject))
    }

    pub(crate) fn history_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.set_height(36.0);
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Histórico").color(t::TEXT).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if icons::button(ui, icons::REFRESH, "Atualizar o histórico", false).clicked() {
                    self.git_refresh();
                    self.history_load();
                }
            });
        });
        if !self.git.repo {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Esta pasta não é um repositório Git").color(t::MUTED));
            });
            return;
        }
        self.merge_banner(ui);
        self.stash_section(ui);
        ui.separator();
        let selected = self.git.history.selected.clone();
        let detail_h = if selected.is_some() { 190.0 } else { 0.0 };
        let list_h = (ui.available_height() - detail_h).max(80.0);
        let mut pick = None;
        egui::ScrollArea::vertical().id_salt("history_list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let h = &self.git.history;
            if !h.error.is_empty() {
                ui.colored_label(t::ERROR, super::branches::explain(&h.error));
            } else if h.commits.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new(if h.rx.is_some() { "Lendo o histórico…" } else { "Nenhum commit ainda." }).color(t::MUTED));
                });
            }
            let remotes = &self.git.snap.remotes;
            for (c, g) in h.commits.iter().zip(&h.rows) {
                let (resp, rect) = row(ui, selected.as_deref() == Some(c.hash.as_str()), ROW_H);
                let resp = named(resp, format!("{}, {}, {}, {}", c.subject, c.author, c.date, short(&c.hash)));
                paint_commit(ui, rect, c, g, h.lanes, remotes);
                if resp.clicked() {
                    pick = Some(c.hash.clone());
                }
            }
            if h.commits.len() >= h.limit {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    if ui.button("Carregar mais commits").clicked() {
                        pick = Some(String::new());
                    }
                });
            }
        });
        match pick {
            Some(hash) if hash.is_empty() => {
                self.git.history.limit += PAGE;
                self.history_load();
            }
            Some(hash) => self.history_select(&hash),
            None => {}
        }
        if let Some(hash) = selected {
            self.commit_detail(ui, &hash);
        }
    }

    /// The selected commit: who, when, and its files (a click opens the diff).
    pub(super) fn commit_detail(&mut self, ui: &mut Ui, hash: &str) {
        let Some(i) = self.git.history.commits.iter().position(|c| c.hash == hash) else { return };
        ui.separator();
        let c = &self.git.history.commits[i];
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.add(egui::Label::new(egui::RichText::new(&c.subject).color(t::TEXT).strong()).truncate());
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new(format!("{} · {} · {}", c.author, c.date, short(&c.hash))).color(t::SECONDARY).small());
            if ui.small_button("Copiar hash").clicked() {
                ui.ctx().copy_text(c.hash.clone());
            }
        });
        let mut open = None;
        egui::ScrollArea::vertical().id_salt("commit_files").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if self.git.history.files.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new(if self.git.history.files_rx.is_some() {
                            "Lendo os arquivos…"
                        } else {
                            "Este commit não alterou nenhum arquivo."
                        })
                        .color(t::MUTED),
                    );
                });
            }
            for (status, rel) in &self.git.history.files {
                let (resp, rect) = row(ui, false, 22.0);
                let resp = named(resp, format!("{} {rel}", status_letter(*status)));
                let p = ui.painter().with_clip_rect(rect);
                p.text(pos2(rect.left() + 14.0, rect.center().y), Align2::LEFT_CENTER, status_letter(*status), t::ui(t::UI_SIZE), status.color());
                p.text(pos2(rect.left() + 32.0, rect.center().y), Align2::LEFT_CENTER, rel, t::ui(t::UI_SIZE), t::TEXT);
                if resp.on_hover_text("Mostra as alterações deste commit neste arquivo").clicked() {
                    open = Some(rel.clone());
                }
            }
        });
        if let Some(rel) = open {
            let commit = &self.git.history.commits[i];
            let commit = Commit {
                hash: commit.hash.clone(),
                parents: commit.parents.clone(),
                author: String::new(),
                date: String::new(),
                refs: vec![],
                subject: String::new(),
            };
            if let Err(why) = self.open_commit_diff(&commit, &rel) {
                self.notify(why);
            }
        }
    }

    /// While a merge waits: its conflicting files (a click opens one in the editor) and the way out.
    pub(super) fn merge_banner(&mut self, ui: &mut Ui) {
        if !self.git.snap.merging {
            return;
        }
        let conflicts: Vec<PathBuf> = self.git.changes.iter().filter(|c: &&Change| c.status == Status::Conflict).map(|c| c.path.clone()).collect();
        let root = self.root.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Merge em andamento").color(t::ERROR).strong());
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let text = if conflicts.is_empty() {
                "Sem conflitos: falta concluir o merge.".to_string()
            } else {
                format!("{} em conflito: resolva no editor.", conflicts.len())
            };
            ui.label(egui::RichText::new(text).color(t::MUTED));
        });
        let mut open = None;
        for path in &conflicts {
            let (resp, rect) = row(ui, false, 22.0);
            let rel = path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/");
            let resp = named(resp, format!("Conflito em {rel}"));
            let p = ui.painter().with_clip_rect(rect);
            p.text(pos2(rect.left() + 14.0, rect.center().y), Align2::LEFT_CENTER, "!", t::ui(t::UI_SIZE), Status::Conflict.color());
            p.text(pos2(rect.left() + 32.0, rect.center().y), Align2::LEFT_CENTER, &rel, t::ui(t::UI_SIZE), t::TEXT);
            if resp.clicked() {
                open = Some(path.clone());
            }
        }
        if let Some(path) = open {
            self.open_file(path, None);
        }
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            if ui.button("Concluir merge").on_hover_text("Prepara os arquivos resolvidos e faz o commit do merge").clicked() {
                self.git_op_or_say(GitOp::MergeFinish);
            }
            if ui.button("Interromper merge").clicked() {
                self.git.ask = Some(Ask::MergeAbort);
            }
        });
        ui.separator();
    }
}
