//! The diff viewer: HEAD (or another revision) against the working copy, a commit against its parent, or a change
//! Claude Code proposes (Accept / Reject). Side by side with the curved bands between the editors, or unified.

use super::commit::checkbox;
use super::git::{DiffEdit, DiffRow, RowKind, Status, word_marks};
use super::*;
use std::ops::Range;

impl Ide {
    pub(crate) fn diff_view(&mut self, ui: &mut Ui) {
        let (project, branch) = (self.folder.clone(), self.branch.clone());
        // rows are one pitch apart: the bands and » are placed on that pitch
        ui.spacing_mut().item_spacing.y = 0.0;
        let font = t::mono(self.font_size);
        let (natural, char_w) = ui.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
        let row_h = (natural * 1.2).round();
        // a plain diff's right side is the file's own buffer: an edit anywhere shows here, and an
        // edit here (», a line typed, ⌘Z) goes there, unsaved until ⌘S, like in PhpStorm
        let plain = self.tabs.get(self.active).and_then(|t| t.diff.as_ref()).is_some_and(|d| d.is_plain());
        let buffer = if plain { self.tabs.get(self.active).map(|t| t.path.clone()).and_then(|p| self.buffer_tab(&p)) } else { None };
        let text = buffer.map(|b| self.tabs[b].text.clone());
        let dirty = buffer.is_some_and(|b| self.tabs[b].dirty);
        // a tracked change can go into the commit in parts: a box per block on the right
        let tracked = self.tabs.get(self.active).is_some_and(|t| self.git.changes.iter().any(|c| c.path == t.path && c.status != Status::Untracked));
        // » and the boxes speak of HEAD: against another base they are off
        let (head, base) = (self.git.base.is_head(), format!("{} (somente leitura)", self.git.base.rev()));
        // the blocks of an ignoring diff are not the ones » and the boxes count: they wait for "Não ignorar"
        let ignoring = self.tabs.get(self.active).and_then(|t| t.diff.as_ref()).is_some_and(|d| d.ignore != super::diffbar::Ignore::None);
        let only = if ignoring { "Só vale com \"Não ignorar\"" } else { super::diffbase::ONLY_HEAD };
        let head = head && !ignoring;
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        let lang = tab.lang;
        let path = tab.path.clone();
        let Some(diff) = tab.diff.as_mut() else { return };
        if let Some(text) = &text {
            let seen = super::text::hash(text, 0.0);
            if diff.seen != seen {
                diff.recompute(text);
                diff.seen = seen;
            }
        }
        // toolbar: previous / next difference + titles
        let mut go: Option<isize> = None;
        let (mut decision, mut revert, mut undo, mut redo, mut save) = (None, None, false, false, false);
        let mut typed: Option<(usize, String)> = None;
        let mut toggle: Option<usize> = None;
        let unified = self.unified_diff;
        let mut set_ignore = None;
        let left_out = if plain && tracked && head { self.git.left_out(&path, diff.seen).cloned().unwrap_or_default() } else { Default::default() };
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
            let label = if left_out.is_empty() {
                if n == 0 { "Nenhuma diferença".to_string() } else { format!("{n} diferença{}", if n == 1 { "" } else { "s" }) }
            } else {
                format!("{n} diferenças, {} incluída{}", n - left_out.len(), if n - left_out.len() == 1 { "" } else { "s" })
            };
            ui.add_space(4.0);
            // PhpStorm's two dropdowns: the viewer, and what the differ ignores
            super::diffbar::choice(ui, &mut self.unified_diff, &[(false, "Lado a lado"), (true, "Unificado")]);
            let mut ignore = diff.ignore;
            if super::diffbar::choice(ui, &mut ignore, &super::diffbar::Ignore::ALL) {
                set_ignore = Some(ignore);
            }
            ui.label(egui::RichText::new(label).color(t::MUTED));
            if plain {
                undo |= ui.add_enabled(!diff.undo.is_empty(), egui::Button::new("Desfazer")).on_hover_text(super::chrome::keys("Ctrl+Z")).clicked();
                redo |= ui.add_enabled(!diff.redo.is_empty(), egui::Button::new("Refazer")).on_hover_text(super::chrome::keys("Ctrl+Shift+Z")).clicked();
                if dirty {
                    save = ui.button("Salvar").on_hover_text(super::chrome::keys("Ctrl+S")).clicked();
                }
            }
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
                // what is left of the row, up to 320: a narrow panel must not push it over the buttons on the left
                let width = (ui.available_width() - 16.0).clamp(80.0, 320.0);
                ui.add_enabled(has_agent, egui::TextEdit::singleline(&mut diff.comment).hint_text(hint).desired_width(width));
                if diff.claude.is_some() {
                    ui.label(egui::RichText::new("Mudança proposta pelo Claude Code").color(t::MUTED));
                }
            });
        });
        if let Some(ignore) = set_ignore {
            diff.set_ignore(ignore);
        }
        // Ctrl+Z / Ctrl+Shift+Z walk the edits made here (a focused text box keeps its own undo);
        // the Shift one first, since consume_shortcut ignores an extra Shift
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        if plain && !typing {
            let cmd_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
            if ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(cmd_shift, egui::Key::Z))) {
                redo = true;
            } else if ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z))) {
                undo = true;
            }
            save |= dirty && ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S)));
        }
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if let Some(text) = comment {
            let (from, to, _lines, removed) = diff.current_lines().unwrap_or((0, 0, Vec::new(), false));
            let source = if removed { &diff.old } else { &diff.new };
            let body = source.join("\n");
            if let Ok(reference) =
                super::coderef::build_reference(&project, &branch, &path, &body, from, to, &text, "diff", if removed { "head" } else { "local" })
            {
                self.to_agent.push(super::coderef::message(&reference));
            }
            self.status = format!("Comentário sobre {file} indo para o agente");
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
        let gw = if unified { 0.0 } else { 40.0 };
        let side_w = if unified { full.width() } else { (full.width() - gw).max(0.0) / 2.0 };
        let right_x = if unified { full.left() } else { full.left() + side_w + gw };
        // unified has two columns of numbers (before and after), side by side one
        let gutter = if unified { 11.0 * char_w } else { 6.0 * char_w };
        // Claude Code's openDiff: the file as it is against what the agent proposes
        let (left, right) = if let Some((l, r)) = &diff.sides {
            (l.as_str(), r.as_str())
        } else if diff.claude.is_some() {
            ("Atual (somente leitura)", "Proposta do Claude Code")
        } else {
            (base.as_str(), "Sua versão")
        };
        ui.horizontal(|ui| {
            let p = ui.painter();
            if unified {
                p.text(
                    pos2(full.left() + 12.0, full.top() + 10.0),
                    Align2::LEFT_CENTER,
                    format!("{left} ↔ {right}  {}", file_name(&path)),
                    t::ui(t::UI_SIZE - 1.0),
                    t::MUTED,
                );
            } else {
                p.text(pos2(full.left() + 12.0, full.top() + 10.0), Align2::LEFT_CENTER, left, t::ui(t::UI_SIZE - 1.0), t::MUTED);
                p.text(
                    pos2(right_x + 12.0, full.top() + 10.0),
                    Align2::LEFT_CENTER,
                    format!("{right}  {}", file_name(&path)),
                    t::ui(t::UI_SIZE - 1.0),
                    t::MUTED,
                );
            }
            ui.allocate_space(vec2(full.width(), 22.0));
        });
        let (ins, del, chg) = (Color32::from_rgb(0x29, 0x44, 0x36), Color32::from_rgb(0x48, 0x4A, 0x4A), Color32::from_rgb(0x38, 0x55, 0x70)); // Darcula DIFF_*
        // a modified line is dimmer than its changed words, which get the strong blue
        let chg_line = soft(chg);
        let mut sa = egui::ScrollArea::both().auto_shrink(false).id_salt(("diff", &path));
        if let Some(r) = diff.scroll_to.take() {
            sa = sa.vertical_scroll_offset((r as f32 * row_h - 120.0).max(0.0));
        }
        // ponytail: the word diff runs for each visible modified row, every frame; cache it per row if long lines get slow
        let split = unified.then(|| super::unified::unified_rows(&diff.rows));
        let display_rows: std::borrow::Cow<[DiffRow]> = match &split {
            Some(s) => s.iter().map(|r| r.0).collect::<Vec<_>>().into(),
            None => diff.rows.as_slice().into(),
        };
        let out = sa.show_rows(ui, row_h, display_rows.len(), |ui, range| {
            for i in range {
                let r = display_rows[i];
                let partner = split.as_ref().and_then(|s| s[i].1);
                let (rect, row) = ui.allocate_exact_size(vec2(ui.available_width().max(full.width()), row_h), Sense::click());
                let left = Rect::from_min_size(rect.min, vec2(side_w, row_h));
                let right = if unified { left } else { Rect::from_min_max(pos2(right_x, rect.top()), rect.right_bottom()) };
                let p = ui.painter().clone();
                let (lbg, rbg) = match r.kind {
                    RowKind::Same => (Color32::TRANSPARENT, Color32::TRANSPARENT),
                    RowKind::Insert => (Color32::TRANSPARENT, ins),
                    RowKind::Delete => (del, Color32::TRANSPARENT),
                    RowKind::Change => {
                        (if r.left.is_some() { chg_line } else { Color32::TRANSPARENT }, if r.right.is_some() { chg_line } else { Color32::TRANSPARENT })
                    }
                };
                p.rect_filled(left, 0, lbg);
                p.rect_filled(right, 0, rbg);
                for x in if unified { vec![] } else { vec![left.right(), right.left()] } {
                    p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, t::POPUP_BORDER));
                }
                let (lm, rm) = match (r.kind, r.left, r.right) {
                    (RowKind::Change, Some(l), Some(n)) => word_marks(diff.old.get(l).map_or("", String::as_str), diff.new.get(n).map_or("", String::as_str)),
                    // unified: the half of a modified row finds its words against the line it pairs with
                    (RowKind::Change, Some(l), None) if unified => {
                        (word_marks(diff.old.get(l).map_or("", String::as_str), partner.and_then(|n| diff.new.get(n)).map_or("", String::as_str)).0, Vec::new())
                    }
                    (RowKind::Change, None, Some(n)) if unified => {
                        (Vec::new(), word_marks(partner.and_then(|l| diff.old.get(l)).map_or("", String::as_str), diff.new.get(n).map_or("", String::as_str)).1)
                    }
                    _ => Default::default(),
                };
                // a double click on a line of yours types into it; Enter puts it in the buffer, Esc drops it
                if plain
                    && row.double_clicked()
                    && let Some(n) = r.right
                    && row.interact_pointer_pos().is_some_and(|p| p.x >= right_x)
                {
                    diff.editing = Some((n, diff.new.get(n).cloned().unwrap_or_default()));
                }
                if let (Some((n, line)), Some(at)) = (diff.editing.as_mut(), r.right)
                    && *n == at
                {
                    let edit = egui::TextEdit::singleline(line).font(font.clone()).frame(egui::Frame::NONE).margin(egui::Margin::ZERO);
                    let field = ui.put(Rect::from_min_max(pos2(right.left() + gutter + 4.0, right.top()), right.max), edit);
                    if !ui.memory(|m| m.has_focus(field.id)) && field.lost_focus() {
                        // Enter (or a click away) keeps it; Esc drops it
                        if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            typed = diff.editing.clone();
                        }
                        diff.editing = None;
                    } else {
                        field.request_focus();
                    }
                }
                let typing_row = diff.editing.as_ref().map(|e| e.0);
                let entries = if unified {
                    vec![(
                        r.left.is_some(),
                        left,
                        r.left.or(r.right),
                        if r.left.is_some() { &diff.old } else { &diff.new },
                        if r.left.is_some() { lm } else { rm },
                    )]
                } else {
                    vec![(true, left, r.left, &diff.old, lm), (false, right, r.right, &diff.new, rm)]
                };
                for (is_left, side, idx, lines, marks) in entries {
                    let Some(n) = idx else { continue };
                    if !is_left && typing_row == Some(n) {
                        p.text(pos2(side.left() + gutter - 8.0, side.center().y), Align2::RIGHT_CENTER, (n + 1).to_string(), font.clone(), t::LINE_NUMBER);
                        continue;
                    }
                    // the left side's numbers sit against the divider, its text starts at the edge
                    let (num_x, text_x, clip) = if unified {
                        // both columns, as the unified viewer has: the line's number before and after (blank on the side it is not on)
                        let y = side.center().y;
                        if let Some(l) = r.left {
                            p.text(pos2(side.left() + 5.0 * char_w + 4.0, y), Align2::RIGHT_CENTER, (l + 1).to_string(), font.clone(), t::LINE_NUMBER);
                        }
                        if let Some(n) = r.right {
                            p.text(pos2(side.left() + gutter - 8.0, y), Align2::RIGHT_CENTER, (n + 1).to_string(), font.clone(), t::LINE_NUMBER);
                        }
                        (f32::NAN, side.left() + gutter + 4.0, side)
                    } else if is_left {
                        (side.right() - 6.0, side.left() + 6.0, Rect::from_min_max(side.min, pos2(side.right() - gutter, side.bottom())))
                    } else {
                        (side.left() + gutter - 8.0, side.left() + gutter + 4.0, side)
                    };
                    if !unified {
                        p.text(pos2(num_x, side.center().y), Align2::RIGHT_CENTER, (n + 1).to_string(), font.clone(), t::LINE_NUMBER);
                    }
                    let line = lines.get(n).map(String::as_str).unwrap_or("");
                    let job = highlight::layout(lang, line, font.clone(), row_h);
                    let g = ui.fonts_mut(|f| f.layout_job(job));
                    let origin = pos2(text_x, side.top());
                    for span in marks {
                        // byte offsets become char cursors, and the galley gives the x of each
                        let x = |b: usize| g.pos_from_cursor(egui::text::CCursor::new(line[..b].chars().count())).left();
                        p.rect_filled(Rect::from_min_max(pos2(origin.x + x(span.start), side.top()), pos2(origin.x + x(span.end), side.bottom())), 0, chg);
                    }
                    p.with_clip_rect(clip).galley(origin, g, t::EDITOR_TEXT);
                }
            }
        });
        // the bands and the » sit in the gutter over the rows, which scroll under them
        let off = out.state.clamped_offset();
        let row_top = out.inner_rect.top() - off.y;
        let (gx0, gx1) = (out.inner_rect.left() - off.x + side_w, out.inner_rect.left() - off.x + side_w + gw);
        let painter = ui.painter().with_clip_rect(out.inner_rect);
        // (the bands and » belong to the two-sided viewer)
        for k in 0..if unified { 0 } else { diff.blocks.len() } {
            let rows = diff.block_rows(k);
            let has = |left: bool| rows.clone().any(|i| if left { diff.rows[i].left.is_some() } else { diff.rows[i].right.is_some() });
            let fill = match (has(true), has(false)) {
                (true, true) => chg,
                (false, _) => ins,
                _ => del,
            };
            let (l0, l1) = band_y(&diff.rows, rows.clone(), true, row_top, row_h);
            let (r0, r1) = band_y(&diff.rows, rows.clone(), false, row_top, row_h);
            if l0.min(r0) > out.inner_rect.bottom() || l1.max(r1) < out.inner_rect.top() {
                continue;
            }
            // a curved band: both edges are cubic beziers with horizontal ends, 30% and 70% across; no outline,
            // just the fill and a 1 px line down its middle, so a steep band never vanishes (PhpStorm's divider)
            const STEPS: u32 = 12;
            let (mut mesh, mut mid) = (egui::Mesh::default(), vec![]);
            for s in 0..=STEPS {
                let t = s as f32 / STEPS as f32;
                let x = bezier(gx0, gx0 + 0.3 * gw, gx0 + 0.7 * gw, gx1, t);
                let (a, b) = (pos2(x, bezier(l0, l0, r0, r0, t)), pos2(x, bezier(l1, l1, r1, r1, t)));
                mesh.colored_vertex(a, fill);
                mesh.colored_vertex(b, fill);
                mid.push(pos2(x, (a.y + b.y) / 2.0));
            }
            for s in 0..STEPS {
                let i = 2 * s;
                mesh.add_triangle(i, i + 2, i + 1);
                mesh.add_triangle(i + 2, i + 3, i + 1);
            }
            painter.add(egui::Shape::mesh(mesh));
            painter.add(egui::Shape::line(mid, Stroke::new(1.0, fill)));
            // PhpStorm's » on the left end of each band: put the block back; Ctrl+» appends it
            if plain {
                let at = Rect::from_center_size(pos2(gx0 + 14.0, l0 + row_h / 2.0), vec2(20.0, row_h));
                if out.inner_rect.contains(at.center()) {
                    let resp = ui.interact(at, ui.scope_id().with(("revert", k)), Sense::click());
                    // bare, as PhpStorm draws it: no box behind the arrow
                    let color = if !head {
                        t::MUTED
                    } else if resp.hovered() {
                        t::TEXT
                    } else {
                        t::LINK
                    };
                    painter.text(at.center(), Align2::CENTER_CENTER, "»", t::ui(16.0), color);
                    let resp = if head {
                        resp.on_hover_text("Desfazer esta alteração: o bloco volta como está no HEAD. Ctrl+clique: mantém o seu e acrescenta o do HEAD depois")
                    } else {
                        resp.on_hover_text(format!("Desfazer esta alteração: {}", only))
                    };
                    if head && resp.clicked() {
                        revert = Some((k, ui.input(|i| i.modifiers.command)));
                    }
                }
                // "Include into commit": the block's box at the right end of the band
                let cb = Rect::from_center_size(pos2(gx1 - 9.0, r0 + row_h / 2.0), vec2(13.0, 13.0));
                if tracked && out.inner_rect.contains(cb.center()) {
                    let resp = ui.interact(cb.expand(3.0), ui.scope_id().with(("include", k)), Sense::click());
                    checkbox(ui, cb, head && !left_out.contains(&k));
                    let resp = if head { resp.on_hover_text("Incluir no commit") } else { resp.on_hover_text(format!("Incluir no commit: {}", only)) };
                    if head && resp.clicked() {
                        toggle = Some(k);
                    }
                }
            }
        }
        if let Some(k) = toggle {
            let (seen, n) = (diff.seen, diff.blocks.len());
            let entry = self.git.excluded.entry(path.clone()).or_insert_with(|| (seen, n, Default::default()));
            if entry.0 != seen {
                *entry = (seen, n, Default::default());
            }
            entry.1 = n;
            if !entry.2.remove(&k) {
                entry.2.insert(k);
            }
        }
        let (Some(b), Some(text)) = (buffer, text) else { return };
        let Some(diff) = self.tabs[self.active].diff.as_mut() else { return };
        // what the buffer becomes: a block back to HEAD, a typed line, or a step through the history
        let edit = match (revert, typed) {
            (Some((k, append)), _) => Some(DiffEdit::Revert(k, append)),
            (_, Some((n, line))) => Some(DiffEdit::Line(n, line)),
            _ if undo => Some(DiffEdit::Undo),
            _ if redo => Some(DiffEdit::Redo),
            _ => None,
        };
        let next = edit.and_then(|e| diff.apply(&text, e));
        if let Some(next) = next {
            let tab = &mut self.tabs[b];
            tab.text = next;
            tab.dirty = true;
        }
        if save {
            self.save(b, false);
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

/// The tone of a line with changed words: 40% of the type's colour, 60% of the editor background.
fn soft(c: Color32) -> Color32 {
    let bg = t::ISLAND;
    let mix = |a: u8, b: u8| (a as f32 * 0.4 + b as f32 * 0.6).round() as u8;
    Color32::from_rgb(mix(c.r(), bg.r()), mix(c.g(), bg.g()), mix(c.b(), bg.b()))
}

/// One coordinate of a cubic bezier.
fn bezier(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_line_is_the_type_mixed_with_the_background() {
        assert_eq!(soft(Color32::from_rgb(0x29, 0x44, 0x36)), Color32::from_rgb(0x1F, 0x2B, 0x26));
        assert_eq!(soft(Color32::from_rgb(0x48, 0x4A, 0x4A)), Color32::from_rgb(0x2C, 0x2D, 0x2E));
        assert_eq!(soft(Color32::from_rgb(0x38, 0x55, 0x70)), Color32::from_rgb(0x25, 0x32, 0x3E));
    }
}
