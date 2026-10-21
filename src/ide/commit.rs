//! The Commit tool window and the side-by-side diff (HEAD vs working copy, or a change Claude Code
//! proposes, with Accept / Reject).

use super::git::{Change, DiffRow, RowKind, Status, word_marks};
use super::*;
use std::ops::Range;

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
        // rows are one pitch apart: the bands and » are placed on that pitch
        ui.spacing_mut().item_spacing.y = 0.0;
        let font = t::mono(self.font_size);
        let (natural, char_w) = ui.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
        let row_h = (natural * 1.2).round();
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        let lang = tab.lang;
        let path = tab.path.clone();
        let Some(diff) = tab.diff.as_mut() else { return };
        // toolbar: previous / next difference + titles
        let mut go: Option<isize> = None;
        let (mut decision, mut revert, mut undo) = (None, None, false);
        let plain = diff.claude.is_none();
        let mut comment = None;
        let has_agent = self.has_agent;
        let said = has_agent && !diff.comment.trim().is_empty();
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if icons::button(ui, icons::CHEVRON_UP, "Diferença anterior (Shift+F7)", false).clicked() {
                go = Some(-1);
            }
            if icons::button(ui, icons::CHEVRON_DOWN, "Próxima diferença (F7)", false).clicked() {
                go = Some(1);
            }
            let n = diff.blocks.len();
            ui.label(egui::RichText::new(format!("{n} diferença{}", if n == 1 { "" } else { "s" })).color(t::MUTED));
            undo = diff.undo.is_some() && ui.button("Desfazer").on_hover_text(format!("Devolve o arquivo como estava antes da última »  ({})", super::chrome::keys("Ctrl+Z"))).clicked();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                if diff.claude.is_some() {
                    let reject = if said { "Rejeitar e comentar" } else { "Rejeitar" };
                    if ui.add(egui::Button::new(reject).min_size(vec2(72.0, 26.0))).clicked() {
                        decision = Some(false);
                    }
                    if ui.add(egui::Button::new(egui::RichText::new("Aceitar").color(Color32::WHITE)).fill(t::ACCENT).min_size(vec2(72.0, 26.0))).clicked() {
                        decision = Some(true);
                    }
                } else if ui.add_enabled(said, egui::Button::new("Comentar ao agente").min_size(vec2(72.0, 26.0))).clicked() {
                    comment = Some(std::mem::take(&mut diff.comment));
                }
                let hint = if !has_agent {
                    "Ligue um agente a este IDE para comentar"
                } else if diff.claude.is_some() {
                    "O que mudar nesta proposta? (vai com Rejeitar)"
                } else {
                    "Comentário sobre esta alteração, para o agente"
                };
                ui.add_enabled(has_agent, egui::TextEdit::singleline(&mut diff.comment).hint_text(hint).desired_width(320.0));
                if diff.claude.is_some() {
                    ui.label(egui::RichText::new("Mudança proposta pelo Claude Code").color(t::MUTED));
                }
            });
        });
        // Ctrl+Z puts back the last » (a focused text box keeps its own undo)
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        if plain && diff.undo.is_some() && !typing && ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z))) {
            undo = true;
        }
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if let Some(text) = comment {
            // about a change already on disk: where it is and what it says, then the comment
            let (from, to, lines, removed) = diff.current_lines().unwrap_or((0, 0, Vec::new(), false));
            let mut code = lines.iter().take(20).copied().collect::<Vec<_>>().join("\n");
            if lines.len() > 20 {
                code += &format!("\n… (+{} linhas)", lines.len() - 20);
            }
            let what = if removed { format!("linhas {from}–{to} removidas") } else { format!("linhas {from}–{to}") };
            // a fence longer than any backtick run inside, so the code can't close it
            let fence = "`".repeat(code.split(|c| c != '`').map(str::len).max().unwrap_or(0).max(2) + 1);
            self.to_agent.push(format!("Sobre a alteração em {} ({what}):\n{fence}\n{code}\n{fence}\n{}", path.display(), text.trim()));
            self.status = format!("Comentário enviado sobre {file}");
        }
        if let Some(accept) = decision
            && let Some((reply, text)) = diff.claude.take()
        {
            // the CLI writes the file after FILE_SAVED; the disk watcher reloads open tabs
            let _ = reply.send(if accept { vec!["FILE_SAVED".into(), text] } else { vec!["DIFF_REJECTED".into()] });
            // rejected with a comment: it becomes the next message, "what to do differently"
            if !accept && said {
                self.to_agent.push(format!("Rejeitei a sua mudança em {file}: {}", diff.comment.trim()));
            }
            if accept && let Some(tab) = self.tabs.iter_mut().find(|t| t.diff.is_none() && t.path == path) {
                tab.dirty = false; // accepted over the buffer's unsaved edits: let the CLI's write in instead of clobbering it
            }
            self.close_tab(self.active);
            return;
        }
        // Shift+F7 first: plain F7 also matches it, since consume_shortcut ignores an extra Shift
        if ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::F7))) {
            go = Some(-1);
        } else if ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F7))) {
            go = Some(1);
        }
        // the first and last difference stop the walk, as PhpStorm's iterable does
        if let Some(d) = go
            && !diff.blocks.is_empty()
        {
            diff.current = (diff.current as isize + d).clamp(0, diff.blocks.len() as isize - 1) as usize;
            diff.scroll_to = Some(diff.blocks[diff.current]);
        }
        let full = ui.available_rect_before_wrap();
        // the two sides, and between them a gutter for the bands and the » buttons
        let gw = 40.0;
        let side_w = (full.width() - gw).max(0.0) / 2.0;
        let right_x = full.left() + side_w + gw;
        let gutter = 6.0 * char_w;
        ui.horizontal(|ui| {
            let p = ui.painter();
            p.text(pos2(full.left() + 12.0, full.top() + 10.0), Align2::LEFT_CENTER, "HEAD (somente leitura)", t::ui(t::UI_SIZE - 1.0), t::MUTED);
            p.text(pos2(right_x + 12.0, full.top() + 10.0), Align2::LEFT_CENTER, format!("Sua versão  {}", file_name(&path)), t::ui(t::UI_SIZE - 1.0), t::MUTED);
            ui.allocate_space(vec2(full.width(), 22.0));
        });
        let (ins, del, chg) = (Color32::from_rgb(0x29, 0x44, 0x36), Color32::from_rgb(0x48, 0x4A, 0x4A), Color32::from_rgb(0x38, 0x55, 0x70)); // Darcula DIFF_*
        // a modified line is dimmer than its changed words, which get the strong blue
        let chg_line = Color32::from_rgb(0x28, 0x37, 0x46);
        let mut sa = egui::ScrollArea::both().auto_shrink(false).id_salt(("diff", &path));
        if let Some(r) = diff.scroll_to.take() {
            sa = sa.vertical_scroll_offset((r as f32 * row_h - 120.0).max(0.0));
        }
        // ponytail: the word diff runs for each visible modified row, every frame; cache it per row if long lines get slow
        let out = sa.show_rows(ui, row_h, diff.rows.len(), |ui, range| {
            for i in range {
                let r = &diff.rows[i];
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().max(full.width()), row_h), Sense::hover());
                let left = Rect::from_min_size(rect.min, vec2(side_w, row_h));
                let right = Rect::from_min_max(pos2(right_x, rect.top()), rect.right_bottom());
                let p = ui.painter();
                let (lbg, rbg) = match r.kind {
                    RowKind::Same => (Color32::TRANSPARENT, Color32::TRANSPARENT),
                    RowKind::Insert => (Color32::TRANSPARENT, ins),
                    RowKind::Delete => (del, Color32::TRANSPARENT),
                    RowKind::Change => (if r.left.is_some() { chg_line } else { Color32::TRANSPARENT }, if r.right.is_some() { chg_line } else { Color32::TRANSPARENT }),
                };
                p.rect_filled(left, 0, lbg);
                p.rect_filled(right, 0, rbg);
                for x in [left.right(), right.left()] {
                    p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, t::POPUP_BORDER));
                }
                let (lm, rm) = match (r.kind, r.left, r.right) {
                    (RowKind::Change, Some(l), Some(n)) => word_marks(diff.old.get(l).map_or("", String::as_str), diff.new.get(n).map_or("", String::as_str)),
                    _ => Default::default(),
                };
                for (side, idx, lines, marks) in [(left, r.left, &diff.old, lm), (right, r.right, &diff.new, rm)] {
                    let Some(n) = idx else { continue };
                    p.text(pos2(side.left() + gutter - 8.0, side.center().y), Align2::RIGHT_CENTER, (n + 1).to_string(), font.clone(), t::LINE_NUMBER);
                    let line = lines.get(n).map(String::as_str).unwrap_or("");
                    let job = highlight::layout(lang, line, font.clone(), row_h);
                    let g = ui.fonts_mut(|f| f.layout_job(job));
                    let origin = pos2(side.left() + gutter + 4.0, side.top());
                    for span in marks {
                        // byte offsets become char cursors, and the galley gives the x of each
                        let x = |b: usize| g.pos_from_cursor(egui::text::CCursor::new(line[..b].chars().count())).left();
                        p.rect_filled(Rect::from_min_max(pos2(origin.x + x(span.start), side.top()), pos2(origin.x + x(span.end), side.bottom())), 0, chg);
                    }
                    p.with_clip_rect(side).galley(origin, g, t::EDITOR_TEXT);
                }
            }
        });
        // the bands and the » sit in the gutter over the rows, which scroll under them
        let off = out.state.clamped_offset();
        let row_top = out.inner_rect.top() - off.y;
        let (gx0, gx1) = (out.inner_rect.left() - off.x + side_w, out.inner_rect.left() - off.x + side_w + gw);
        let painter = ui.painter().with_clip_rect(out.inner_rect);
        for k in 0..diff.blocks.len() {
            let rows = diff.block_rows(k);
            let has = |left: bool| rows.clone().any(|i| if left { diff.rows[i].left.is_some() } else { diff.rows[i].right.is_some() });
            let (fill, edge) = match (has(true), has(false)) {
                (true, true) => (chg, Color32::from_rgb(0x43, 0x69, 0x8D)),
                (false, _) => (ins, Color32::from_rgb(0x44, 0x71, 0x52)),
                _ => (del, Color32::from_rgb(0x65, 0x6E, 0x76)),
            };
            let (l0, l1) = band_y(&diff.rows, rows.clone(), true, row_top, row_h);
            let (r0, r1) = band_y(&diff.rows, rows.clone(), false, row_top, row_h);
            if l0.min(r0) > out.inner_rect.bottom() || l1.max(r1) < out.inner_rect.top() {
                continue;
            }
            // a curved band: both edges are cubic beziers with horizontal ends, 30% and 70% across
            const STEPS: u32 = 12;
            let (mut mesh, mut upper, mut lower) = (egui::Mesh::default(), vec![], vec![]);
            for s in 0..=STEPS {
                let t = s as f32 / STEPS as f32;
                let x = bezier(gx0, gx0 + 0.3 * gw, gx0 + 0.7 * gw, gx1, t);
                let (a, b) = (pos2(x, bezier(l0, l0, r0, r0, t)), pos2(x, bezier(l1, l1, r1, r1, t)));
                mesh.colored_vertex(a, fill);
                mesh.colored_vertex(b, fill);
                upper.push(a);
                lower.push(b);
            }
            for s in 0..STEPS {
                let i = 2 * s;
                mesh.add_triangle(i, i + 2, i + 1);
                mesh.add_triangle(i + 2, i + 3, i + 1);
            }
            painter.add(egui::Shape::mesh(mesh));
            painter.add(egui::Shape::line(upper, Stroke::new(1.0, edge)));
            painter.add(egui::Shape::line(lower, Stroke::new(1.0, edge)));
            // PhpStorm's » on the left end of each band: put the block back; Ctrl+» appends it
            if plain {
                let at = Rect::from_center_size(pos2(gx0 + 14.0, l0 + row_h / 2.0), vec2(20.0, row_h));
                if out.inner_rect.contains(at.center()) {
                    let resp = ui.interact(at, ui.scope_id().with(("revert", k)), Sense::click());
                    // bare, as PhpStorm draws it: no box behind the arrow
                    painter.text(at.center(), Align2::CENTER_CENTER, "»", t::ui(16.0), if resp.hovered() { t::TEXT } else { t::LINK });
                    let resp = resp.on_hover_text("Desfazer esta alteração: o bloco volta como está no HEAD. Ctrl+clique: mantém o seu e acrescenta o do HEAD depois");
                    if resp.clicked() {
                        revert = Some((k, ui.input(|i| i.modifiers.command)));
                    }
                }
            }
        }
        if undo {
            self.revert_block(0, false, true);
        } else if let Some((k, append)) = revert {
            self.revert_block(k, append, false);
        }
    }
}

/// Screen y span of a block on one side: the rows that have a line there, or a thin tip at the
/// block's first row when that side is empty (a pure insert or delete).
fn band_y(rows: &[DiffRow], block: Range<usize>, left: bool, top: f32, row_h: f32) -> (f32, f32) {
    let has = |i: usize| if left { rows[i].left.is_some() } else { rows[i].right.is_some() };
    match (block.clone().find(|&i| has(i)), block.clone().rfind(|&i| has(i))) {
        (Some(a), Some(b)) => (top + a as f32 * row_h, top + (b + 1) as f32 * row_h),
        _ => (top + block.start as f32 * row_h, top + block.start as f32 * row_h + 2.0),
    }
}

/// One coordinate of a cubic bezier.
fn bezier(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
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
