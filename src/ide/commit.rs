//! The Commit tool window and the side-by-side diff (HEAD vs working copy, or a change Claude Code
//! proposes, with Accept / Reject).

use super::git::{Change, RowKind, Status};
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
                    && icons::button(ui, icons::DIFF, "Mostrar diff (Ctrl+D)", false).clicked()
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
        let bottom_h = 190.0;
        let list_h = (ui.available_height() - bottom_h).max(80.0);
        let root = self.root.clone().unwrap_or_default();
        let mut open = None;
        let mut toggles = vec![];
        egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for untracked in [false, true] {
                let group: Vec<&Change> = self.git.changes.iter().filter(|c| (c.status == Status::Untracked) == untracked).collect();
                if group.is_empty() {
                    continue;
                }
                let all = group.iter().all(|c| self.git.included(c));
                let (resp, rect) = row(ui, false, 26.0);
                let mut check = all;
                let cb = Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), vec2(14.0, 14.0));
                checkbox(ui, cb, check);
                let title = if untracked { "Arquivos não versionados" } else { "Alterações" };
                let g = ui.painter().text(pos2(rect.left() + 36.0, rect.center().y), Align2::LEFT_CENTER, title, t::ui(t::UI_SIZE), t::TEXT);
                ui.painter().text(pos2(g.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, if group.len() == 1 { "1 arquivo".to_string() } else { format!("{} arquivos", group.len()) }, t::ui(t::UI_SIZE), t::SECONDARY);
                if resp.clicked() {
                    check = !check;
                    toggles.extend(group.iter().map(|c| (c.path.clone(), c.status, check)));
                }
                for c in group {
                    let selected = self.git.selected.as_ref() == Some(&c.path);
                    let (resp, rect) = row(ui, selected, 24.0);
                    let cb = Rect::from_center_size(pos2(rect.left() + 40.0, rect.center().y), vec2(14.0, 14.0));
                    let inc = self.git.included(c);
                    checkbox(ui, cb, inc);
                    icons::file(ui.painter(), &c.path, pos2(rect.left() + 60.0, rect.center().y), 1.0);
                    let name = file_name(&c.path);
                    let g = ui.painter().text(pos2(rect.left() + 72.0, rect.center().y), Align2::LEFT_CENTER, name, t::ui(t::UI_SIZE), c.status.color());
                    let dir = c.path.parent().and_then(|d| d.strip_prefix(&root).ok()).map(|d| d.display().to_string()).unwrap_or_default();
                    ui.painter().text(pos2(g.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, dir, t::ui(t::UI_SIZE), t::SECONDARY);
                    if resp.clicked() {
                        if ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| cb.expand(4.0).contains(p)) {
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
            if self.git.changes.is_empty() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new("Nenhuma alteração").color(t::MUTED));
                });
            }
        });
        for (p, st, on) in toggles {
            if st == Status::Untracked {
                if on { self.git.checked_untracked.insert(p) } else { self.git.checked_untracked.remove(&p) };
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
            let files: Vec<String> = self.git.changes.iter().filter(|c| self.git.included(c)).map(|c| c.path.strip_prefix(&root).unwrap_or(&c.path).to_string_lossy().into_owned()).collect();
            let can = !files.is_empty() && (!self.git.message.trim().is_empty() || self.git.amend) && self.git.busy.is_none();
            ui.horizontal(|ui| {
                let commit = ui.add_enabled(can, egui::Button::new(egui::RichText::new("Commit").color(Color32::WHITE)).fill(t::ACCENT).min_size(vec2(80.0, 28.0)));
                let push = ui.add_enabled(can, egui::Button::new("Commit e push…").min_size(vec2(0.0, 28.0)).stroke(Stroke::new(1.0, t::CONTROL_BORDER)));
                if commit.clicked() || push.clicked() {
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

    // ------------------------------------------------------------ side-by-side diff

    pub(crate) fn diff_view(&mut self, ui: &mut Ui) {
        let font = t::mono(self.font_size);
        let (natural, char_w) = ui.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
        let row_h = (natural * 1.2).round();
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        let lang = tab.lang;
        let path = tab.path.clone();
        let Some(diff) = tab.diff.as_mut() else { return };
        // toolbar: previous / next difference + titles
        let mut go: Option<isize> = None;
        let mut decision = None;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if icons::button(ui, icons::CHEVRON_UP, "Diferença anterior (Shift+F7)", false).clicked() {
                go = Some(-1);
            }
            if icons::button(ui, icons::CHEVRON_DOWN, "Próxima diferença (F7)", false).clicked() {
                go = Some(1);
            }
            ui.label(egui::RichText::new(format!("{} diferenças", diff.blocks.len())).color(t::MUTED));
            if diff.claude.is_some() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    if ui.add(egui::Button::new("Rejeitar").min_size(vec2(72.0, 26.0))).clicked() {
                        decision = Some(false);
                    }
                    if ui.add(egui::Button::new(egui::RichText::new("Aceitar").color(Color32::WHITE)).fill(t::ACCENT).min_size(vec2(72.0, 26.0))).clicked() {
                        decision = Some(true);
                    }
                    ui.label(egui::RichText::new("Mudança proposta pelo Claude Code").color(t::MUTED));
                });
            }
        });
        if let Some(accept) = decision
            && let Some((reply, text)) = diff.claude.take()
        {
            // the CLI writes the file after FILE_SAVED; the disk watcher reloads open tabs
            let _ = reply.send(if accept { vec!["FILE_SAVED".into(), text] } else { vec!["DIFF_REJECTED".into()] });
            if accept && let Some(tab) = self.tabs.iter_mut().find(|t| t.diff.is_none() && t.path == path) {
                tab.dirty = false; // accepted over the buffer's unsaved edits: let the CLI's write in instead of clobbering it
            }
            self.close_tab(self.active);
            return;
        }
        if ui.input(|i| i.key_pressed(egui::Key::F7)) {
            go = Some(if ui.input(|i| i.modifiers.shift) { -1 } else { 1 });
        }
        if let Some(d) = go
            && !diff.blocks.is_empty()
        {
            diff.current = (diff.current as isize + d).rem_euclid(diff.blocks.len() as isize) as usize;
            diff.scroll_to = Some(diff.blocks[diff.current]);
        }
        let full = ui.available_rect_before_wrap();
        let half = full.width() / 2.0;
        let gutter = 6.0 * char_w;
        ui.horizontal(|ui| {
            let p = ui.painter();
            p.text(pos2(full.left() + 12.0, full.top() + 10.0), Align2::LEFT_CENTER, "HEAD (somente leitura)", t::ui(t::UI_SIZE - 1.0), t::MUTED);
            p.text(pos2(full.left() + half + 12.0, full.top() + 10.0), Align2::LEFT_CENTER, format!("Sua versão  {}", file_name(&path)), t::ui(t::UI_SIZE - 1.0), t::MUTED);
            ui.allocate_space(vec2(full.width(), 22.0));
        });
        let (ins, del, chg) = (Color32::from_rgb(0x29, 0x44, 0x36), Color32::from_rgb(0x48, 0x4A, 0x4A), Color32::from_rgb(0x38, 0x55, 0x70)); // Darcula DIFF_*
        let mut sa = egui::ScrollArea::both().auto_shrink(false).id_salt(("diff", &path));
        if let Some(r) = diff.scroll_to.take() {
            sa = sa.vertical_scroll_offset((r as f32 * row_h - 120.0).max(0.0));
        }
        sa.show_rows(ui, row_h, diff.rows.len(), |ui, range| {
            for i in range {
                let r = &diff.rows[i];
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().max(full.width()), row_h), Sense::hover());
                let left = Rect::from_min_size(rect.min, vec2(half - 1.0, row_h));
                let right = Rect::from_min_size(pos2(rect.left() + half + 1.0, rect.top()), vec2(half - 1.0, row_h));
                let p = ui.painter();
                let (lbg, rbg) = match r.kind {
                    RowKind::Same => (Color32::TRANSPARENT, Color32::TRANSPARENT),
                    RowKind::Insert => (Color32::TRANSPARENT, ins),
                    RowKind::Delete => (del, Color32::TRANSPARENT),
                    RowKind::Change => (if r.left.is_some() { chg } else { Color32::TRANSPARENT }, if r.right.is_some() { chg } else { Color32::TRANSPARENT }),
                };
                p.rect_filled(left, 0, lbg);
                p.rect_filled(right, 0, rbg);
                p.line_segment([pos2(rect.left() + half, rect.top()), pos2(rect.left() + half, rect.bottom())], Stroke::new(1.0, t::POPUP_BORDER));
                for (side, idx, lines) in [(left, r.left, &diff.old), (right, r.right, &diff.new)] {
                    let Some(n) = idx else { continue };
                    p.text(pos2(side.left() + gutter - 8.0, side.center().y), Align2::RIGHT_CENTER, (n + 1).to_string(), font.clone(), t::LINE_NUMBER);
                    let line = lines.get(n).map(String::as_str).unwrap_or("");
                    let job = highlight::layout(lang, line, font.clone(), row_h, &[]);
                    let g = ui.fonts_mut(|f| f.layout_job(job));
                    p.with_clip_rect(side).galley(pos2(side.left() + gutter + 4.0, side.top()), g, t::EDITOR_TEXT);
                }
            }
        });
    }
}

/// Checkbox painted in the New UI style (the egui one is too small and grey).
fn checkbox(ui: &Ui, r: Rect, on: bool) {
    let p = ui.painter();
    if on {
        p.rect_filled(r, 3, t::ACCENT);
        p.line(vec![pos2(r.left() + 3.5, r.center().y), pos2(r.left() + 6.0, r.bottom() - 4.0), pos2(r.right() - 3.0, r.top() + 4.0)], Stroke::new(1.6, Color32::WHITE));
    } else {
        p.rect_stroke(r, 3, Stroke::new(1.0, Color32::from_rgb(0x6F, 0x73, 0x7A)), egui::StrokeKind::Inside);
    }
}
