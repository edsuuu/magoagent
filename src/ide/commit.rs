//! The Commit tool window: the changed files with their boxes, the message, Commit / Commit and push.

use super::git::{Change, Status};
use super::*;

impl Ide {
    pub(crate) fn commit_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.set_height(36.0);
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Commit").color(t::TEXT).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if icons::button(ui, icons::REFRESH, "Atualizar", false).clicked() {
                    self.git_refresh();
                }
                if let Some(sel) = self.git.selected.clone()
                    && icons::button(ui, icons::DIFF, "Mostrar diff", false).clicked()
                {
                    self.open_diff(sel);
                }
            });
        });
        if !self.git.repo {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Esta pasta não é um repositório Git").color(t::MUTED));
            });
            if ui.button("Criar repositório Git").clicked() {
                self.git_run("git init", vec![vec!["init".into()]]);
            }
            return;
        }
        self.base_field(ui);
        // against another base the list is what differs from it: nothing to tick for a commit
        let head = self.git.base.is_head();
        let bottom_h = 190.0;
        let list_h = (ui.available_height() - bottom_h).max(80.0);
        let root = self.root.clone().unwrap_or_default();
        let mut open = None;
        let mut toggles = vec![];
        egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let list = if head { &self.git.changes } else { &self.git.base.changes };
            for untracked in [false, true] {
                let group: Vec<&Change> = list.iter().filter(|c| (c.status == Status::Untracked) == untracked).collect();
                if group.is_empty() {
                    continue;
                }
                let all = group.iter().all(|c| self.git.included(c));
                let (resp, rect) = row(ui, false, 26.0);
                let mut check = all;
                let cb = Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), vec2(14.0, 14.0));
                if head {
                    checkbox(ui, cb, check);
                }
                let title = if untracked { "Arquivos não versionados" } else { "Alterações" };
                let g = ui.painter().text(pos2(rect.left() + 36.0, rect.center().y), Align2::LEFT_CENTER, title, t::ui(t::UI_SIZE), t::TEXT);
                ui.painter().text(
                    pos2(g.right() + 8.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    if group.len() == 1 { "1 arquivo".to_string() } else { format!("{} arquivos", group.len()) },
                    t::ui(t::UI_SIZE),
                    t::SECONDARY,
                );
                if head && resp.clicked() {
                    check = !check;
                    toggles.extend(group.iter().map(|c| (c.path.clone(), c.status, check)));
                }
                for c in group {
                    let selected = self.git.selected.as_ref() == Some(&c.path);
                    let (resp, rect) = row(ui, selected, 24.0);
                    let resp = if head { resp } else { resp.on_hover_text(format!("Incluir no commit: {}", super::diffbase::ONLY_HEAD)) };
                    let cb = Rect::from_center_size(pos2(rect.left() + 40.0, rect.center().y), vec2(14.0, 14.0));
                    let inc = head && self.git.included(c);
                    let part = self.git.excluded.get(&c.path).filter(|e| inc && !e.2.is_empty()).map(|e| (e.1, e.1 - e.2.len().min(e.1)));
                    if part.is_some() {
                        checkbox_mixed(ui, cb)
                    } else if head {
                        checkbox(ui, cb, inc)
                    }
                    icons::file(ui.painter(), &c.path, pos2(rect.left() + 60.0, rect.center().y), 1.0);
                    let name = file_name(&c.path);
                    let g = ui.painter().text(pos2(rect.left() + 72.0, rect.center().y), Align2::LEFT_CENTER, name, t::ui(t::UI_SIZE), c.status.color());
                    let dir = c.path.parent().and_then(|d| d.strip_prefix(&root).ok()).map(|d| d.display().to_string()).unwrap_or_default();
                    let d = ui.painter().text(pos2(g.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, dir, t::ui(t::UI_SIZE), t::SECONDARY);
                    if let Some((n, k)) = part {
                        ui.painter().text(
                            pos2(d.right() + 8.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!("{k} de {n} blocos"),
                            t::ui(t::UI_SIZE - 1.0),
                            t::MUTED,
                        );
                    }
                    if resp.clicked() {
                        if head && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| cb.expand(4.0).contains(p)) {
                            toggles.push((c.path.clone(), c.status, !inc));
                        } else {
                            self.git.selected = Some(c.path.clone());
                        }
                    }
                    if resp.double_clicked() {
                        open = Some(c.path.clone());
                    }
                }
            }
            if list.is_empty() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new("Nenhuma alteração").color(t::MUTED));
                });
            }
        });
        for (p, st, on) in toggles {
            if st == Status::Untracked {
                if on {
                    self.git.checked_untracked.insert(p)
                } else {
                    self.git.checked_untracked.remove(&p)
                };
            } else if on {
                self.git.unchecked.remove(&p);
            } else {
                self.git.unchecked.insert(p);
            }
        }
        if let Some(p) = open {
            self.open_diff(p);
        }

        // message + actions, pinned to the bottom like PhpStorm
        ui.separator();
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.add(TextEdit::multiline(&mut self.git.message).hint_text("Mensagem do commit").desired_rows(5).desired_width(f32::INFINITY));
            ui.add_space(6.0);
            ui.checkbox(&mut self.git.amend, "Corrigir o último commit (amend)");
            ui.add_space(6.0);
            let files: Vec<String> = self
                .git
                .changes
                .iter()
                .filter(|c| self.git.included(c))
                .map(|c| c.path.strip_prefix(&root).unwrap_or(&c.path).to_string_lossy().into_owned())
                .collect();
            let can = head && !files.is_empty() && (!self.git.message.trim().is_empty() || self.git.amend) && self.git.busy.is_none();
            let why = |r: egui::Response| if head { r } else { r.on_disabled_hover_text(super::diffbase::ONLY_HEAD) };
            ui.horizontal(|ui| {
                let commit =
                    ui.add_enabled(can, egui::Button::new(egui::RichText::new("Commit").color(Color32::WHITE)).fill(t::ACCENT).min_size(vec2(80.0, 28.0)));
                let push = ui.add_enabled(can, egui::Button::new("Commit e push…").min_size(vec2(0.0, 28.0)).stroke(Stroke::new(1.0, t::CONTROL_BORDER)));
                let (commit, push) = (why(commit), why(push));
                if (commit.clicked() || push.clicked())
                    && self.git.changes.iter().any(|c| self.git.included(c) && self.git.excluded.get(&c.path).is_some_and(|e| !e.2.is_empty()))
                {
                    self.commit_in_parts(push.clicked());
                } else if commit.clicked() || push.clicked() {
                    let mut add = vec!["add".to_string(), "-A".into(), "--".into()];
                    add.extend(files.iter().cloned());
                    let mut commit_cmd = vec!["commit".to_string()];
                    if self.git.amend {
                        commit_cmd.push("--amend".into());
                    }
                    if self.git.message.trim().is_empty() {
                        commit_cmd.push("--no-edit".into());
                    } else {
                        commit_cmd.extend(["-m".into(), self.git.message.clone()]);
                    }
                    commit_cmd.push("--".into());
                    commit_cmd.extend(files.iter().cloned());
                    let mut cmds = vec![add, commit_cmd];
                    if push.clicked() {
                        cmds.push(vec!["push".into()]);
                    }
                    let label = if push.clicked() { "Commit e push" } else { "Commit" };
                    self.git_run(label, cmds);
                }
                if let Some(b) = &self.git.busy {
                    ui.label(egui::RichText::new(b).color(t::MUTED));
                }
            });
        });
    }

    /// Commit with some blocks left out (the diff's boxes): each partial file goes in as HEAD plus
    /// its included blocks, read from disk; blocks picked on text other than what is on disk now
    /// (an unsaved edit, a change since) stop it, so nothing unexpected goes in.
    fn commit_in_parts(&mut self, push: bool) {
        let Some(root) = self.root.clone() else { return };
        let rel = |p: &Path| p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/");
        let (mut whole, mut partial, mut done) = (vec![], vec![], vec![]);
        for c in self.git.changes.iter().filter(|c| self.git.included(c)) {
            let Some((seen, n, out)) = self.git.excluded.get(&c.path).filter(|e| !e.2.is_empty()) else {
                whole.push(rel(&c.path));
                continue;
            };
            let disk = std::fs::read_to_string(&c.path).unwrap_or_default();
            let crlf = disk.contains("\r\n");
            let disk = disk.replace("\r\n", "\n");
            if super::text::hash(&disk, 0.0) != *seen {
                return self
                    .notify(format!("{} mudou (ou tem edição não salva) desde que os blocos foram marcados: salve e marque de novo", file_name(&c.path)));
            }
            done.push(c.path.clone());
            if out.len() >= *n {
                continue; // every block left out: the file stays out
            }
            let head = super::git::head_text(&root, &c.path).unwrap_or_default();
            let text = super::git::partial_text(&head, &disk, out);
            partial.push((rel(&c.path), if crlf { text.replace('\n', "\r\n") } else { text }));
        }
        let mut commit = vec!["commit".to_string()];
        if self.git.amend {
            commit.push("--amend".into());
        }
        if self.git.message.trim().is_empty() {
            commit.push("--no-edit".into());
        } else {
            commit.extend(["-m".into(), self.git.message.clone()]);
        }
        for p in done {
            self.git.excluded.remove(&p);
        }
        let after = if push { vec![vec!["push".to_string()]] } else { vec![] };
        self.git_commit_parts(if push { "Commit e push" } else { "Commit" }, whole, partial, commit, after);
    }
}

/// Checkbox painted in the New UI style (the egui one is too small and grey).
/// A box for a file going in with only some of its blocks: filled, with a dash.
pub(super) fn checkbox_mixed(ui: &Ui, r: Rect) {
    let p = ui.painter();
    p.rect_filled(r, 3, t::ACCENT);
    p.line_segment([pos2(r.left() + 3.5, r.center().y), pos2(r.right() - 3.5, r.center().y)], Stroke::new(1.6, Color32::WHITE));
}

pub(super) fn checkbox(ui: &Ui, r: Rect, on: bool) {
    let p = ui.painter();
    if on {
        p.rect_filled(r, 3, t::ACCENT);
        p.line(
            vec![pos2(r.left() + 3.5, r.center().y), pos2(r.left() + 6.0, r.bottom() - 4.0), pos2(r.right() - 3.0, r.top() + 4.0)],
            Stroke::new(1.6, Color32::WHITE),
        );
    } else {
        p.rect_stroke(r, 3, Stroke::new(1.0, Color32::from_rgb(0x6F, 0x73, 0x7A)), egui::StrokeKind::Inside);
    }
}
