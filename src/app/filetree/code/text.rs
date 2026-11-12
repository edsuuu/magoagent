use super::*;

/// Opens a row of `h` above each of `lines` (in order): that line's rows and every row after them move down. Rows are
/// moved, not made taller, as the IDE's `open_gaps`; by line and not by row, because the pane's text may wrap.
pub(super) fn open_rows(galley: &mut egui::Galley, lines: impl Iterator<Item = usize>, h: f32) {
    let mut lines = lines.peekable();
    let (mut shift, mut line, mut starts) = (0.0, 0, true);
    for row in &mut galley.rows {
        if starts && lines.next_if_eq(&line).is_some() {
            shift += h;
        }
        row.pos.y += shift;
        starts = row.ends_with_newline;
        line += usize::from(starts);
    }
    galley.rect.max.y += shift;
    galley.mesh_bounds.max.y += shift;
}

/// The text in `body`, edited. Back: whether it changed, and what a press on the code asked.
#[allow(clippy::too_many_arguments)]
pub(crate) fn text_ui(
    ui: &mut egui::Ui,
    body: Rect,
    buf: &mut TextBuf,
    path: &Path,
    id: egui::Id,
    z: f32,
    wrap: bool,
    code: &Project,
    diags: &[Diag],
) -> (bool, Option<Click>) {
    let font = t::mono(t::EDITOR_SIZE * z);
    let label_font = t::ui(font.size - z); // the UI face, a little smaller than the code
    let vision_font = t::ui((t::UI_SIZE - 1.0) * z);
    let (natural, char_w) = ui.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
    let row_h = (natural * 1.2).round(); // the IDE's line spacing
    let lines = buf.text.matches('\n').count() + 1;
    // the annotation column (who wrote each line) sits left of the line numbers
    let annotation = buf.annotate.then(|| code.annotation(path)).flatten();
    let ann_w = annotation.map_or(0.0, |a| a.width(ui.painter(), &vision_font, z));
    let gutter = Rect::from_min_size(body.min, Vec2::new(ann_w + lines.to_string().len().max(3) as f32 * char_w + 26.0 * z, body.height()));
    // the error stripe at the right, beside the scroll bar
    let stripe = Rect::from_min_max(Pos2::new(body.right() - STRIPE * z - 1.0, body.top()), Pos2::new(body.right() - 1.0, body.bottom() - 1.0));
    let area = Rect::from_min_max(Pos2::new(gutter.right(), body.top()), Pos2::new(stripe.left(), body.bottom()));
    // the editor's own background, inside the card's border; the line numbers stay on it when the text goes sideways
    let p = ui.painter().with_clip_rect(body.intersect(ui.clip_rect()));
    p.rect_filled(body.with_max_x(body.right() - 1.0).with_max_y(body.bottom() - 1.0), egui::CornerRadius { se: cr(7.0 * z), ..Default::default() }, t::ISLAND);
    // where a Ctrl+click landed: the caret goes there, and that line to the middle of the pane
    let landed = buf.jump.take().map(|(line, col)| {
        let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(char_at(&buf.text, line, col)))));
        state.store(ui.ctx(), id);
        buf.focus = true;
        line
    });
    let want_focus = std::mem::take(&mut buf.focus);
    // the bracket next to the caret (the one before it first) and its pair, found again when the caret or the text moves
    let caret = egui::TextEdit::load_state(ui.ctx(), id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.0);
    let before = hash(&buf.text, font.size);
    crate::app::filetree::diff::refresh(buf, before);
    if buf.bracket.as_ref().is_none_or(|b| b.0 != (before, caret)) {
        let at = |ci: usize| buf.text.char_indices().nth(ci).map(|(b, _)| b);
        let pair = caret
            .filter(|_| buf.text.len() <= SMART_MAX)
            .and_then(|c| [c.checked_sub(1), Some(c)].into_iter().flatten().find_map(|ci| at(ci).and_then(|b| highlight::pair(buf.lang, &buf.text, b))));
        buf.bracket = Some(((before, caret), pair));
    }
    let bracket = buf.bracket.as_ref().and_then(|b| b.1.clone());
    // the caret's place, for the problems' arrows and F2
    let at = caret
        .map(|c| {
            let line = line_of(&buf.text, c);
            (line, c - char_at(&buf.text, line, 0))
        })
        .unwrap_or_default();
    let (mut changed, mut click) = (false, None);
    // where the text's lines are, for the gutter's change markers (drawn once the scroll area is done)
    let mut geom: Option<(f32, Vec<(f32, f32)>)> = None;
    // the line of the annotation column under the pointer
    let mut annotated = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        ui.set_clip_rect(area.intersect(ui.clip_rect()));
        ui.visuals_mut().selection.bg_fill = t::EDITOR_SELECTION;
        ui.visuals_mut().selection.stroke = Stroke::new(1.0, t::TEXT);
        ui.visuals_mut().text_cursor.stroke = Stroke::new((2.0 * z).max(1.0), t::CARET);
        egui::ScrollArea::both().id_salt(id.with("scroll")).auto_shrink(false).show(ui, |ui| {
            // under the text: the caret's row and the guides, filled in once the text is laid out
            let caret_bg = ui.painter().add(Shape::Noop);
            let guides_bg = ui.painter().add(Shape::Noop);
            let hover_bg = ui.painter().add(Shape::Noop);
            let TextBuf { text, laid, lang, .. } = buf;
            let lang: &'static Lang = lang;
            let mut layouter = |ui: &egui::Ui, b: &dyn egui::TextBuffer, width: f32| {
                let s = b.as_str();
                let key = (hash(s, font.size), code.generation);
                if laid.as_ref().is_none_or(|l| l.key != key) {
                    *laid = Some(Laid::new(key, ui, lang, s, path, code, &font, &label_font, row_h, z));
                }
                let l = laid.as_ref().unwrap();
                let mut job = l.job.clone();
                job.wrap.max_width = if wrap { width } else { f32::INFINITY };
                // the pair is of the text before this frame's typing: its bytes may not be this text's
                if let Some((own, _, pair)) = bracket.as_ref().filter(|_| key.0 == before) {
                    match pair {
                        Some(p) => {
                            for r in [own, p] {
                                restyle(&mut job, r.clone(), |f| f.background = t::BRACKET_PAIR);
                            }
                        }
                        None => restyle(&mut job, own.clone(), |f| f.color = t::UNMATCHED_BRACE),
                    }
                }
                let mut galley = ui.fonts_mut(|f| f.layout_job(job));
                if !l.labels.is_empty() {
                    open_rows(std::sync::Arc::make_mut(&mut galley), l.labels.iter().map(|l| l.line), row_h);
                }
                galley
            };
            // a right click keeps the selection (the TextEdit would put the caret under the pointer), as in JetBrains IDEs
            let before = egui::TextEdit::load_state(ui.ctx(), id).and_then(|s| s.cursor.char_range()).filter(|r| !r.is_empty());
            // and so does a click on its menu, which would reach the text under the menu
            let right = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
                || (egui::Popup::is_any_open(ui.ctx()) && ui.input(|i| i.pointer.any_pressed()));
            let mut out = egui::TextEdit::multiline(text)
                .id(id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::symmetric((6.0 * z).round() as i8, 0))
                .font(font.clone())
                .layouter(&mut layouter)
                .lock_focus(true)
                .desired_width(if wrap { area.width() } else { f32::INFINITY })
                .desired_rows(1)
                .show(ui);
            let resp = out.response.response.clone();
            if let Some(kept) = before.filter(|_| right) {
                out.state.cursor.set_char_range(Some(kept));
                out.state.clone().store(ui.ctx(), id);
            }
            if want_focus {
                resp.request_focus();
            }
            changed = resp.changed();
            let Some(l) = laid.as_ref() else { return };
            let (origin, clip) = (out.galley_pos, ui.clip_rect());
            // each line's rows: the top of its first and the bottom of its last (one row a line, unless the text wraps)
            let mut spans: Vec<(f32, f32)> = Vec::with_capacity(lines);
            let mut starts = true;
            for row in &out.galley.rows {
                let r = row.rect();
                match spans.last_mut() {
                    Some(s) if !starts => s.1 = r.bottom(),
                    _ => spans.push((r.top(), r.bottom())),
                }
                starts = row.ends_with_newline;
            }
            geom = Some((origin.y, spans.clone()));
            let first = spans.partition_point(|s| origin.y + s.1 < clip.top());
            let last = spans.partition_point(|s| origin.y + s.0 < clip.bottom()).min(l.cols.len());
            // the caret's row, across the text and the line numbers
            let caret = out.state.cursor.char_range().map(|r| out.galley.pos_from_cursor(r.primary));
            let caret_line = caret.map(|r| spans.partition_point(|s| s.1 <= r.center().y).min(spans.len().saturating_sub(1)));
            if let Some(r) = caret {
                let (y0, y1) = (origin.y + r.min.y, origin.y + r.max.y);
                ui.painter().set(caret_bg, Shape::rect_filled(Rect::from_min_max(Pos2::new(clip.left(), y0), Pos2::new(clip.right(), y1)), 0, t::CARET_ROW));
                p.rect_filled(Rect::from_min_max(Pos2::new(gutter.left(), y0), Pos2::new(gutter.right(), y1)), 0, t::CARET_ROW);
            }
            // indent guides: one line a block, at its opening line's column, from under that line to the block's last
            // one (through the rows opened above the declarations inside it, whose labels start further in); the
            // caret's block the lighter one
            let active = caret_line.and_then(|line| caret_guide(&l.guides, line));
            let mut guides = vec![];
            for &(col, start, end) in &l.guides {
                let (a, b) = ((start + 1).max(first), end.min(last));
                if a < b {
                    let x = origin.x + col as f32 * char_w;
                    let color = if active == Some((col, start, end)) { t::INDENT_GUIDE_SELECTED } else { t::INDENT_GUIDE };
                    guides
                        .push(Shape::line_segment([Pos2::new(x, origin.y + spans[a - 1].1), Pos2::new(x, origin.y + spans[b - 1].1)], Stroke::new(1.0, color)));
                }
            }
            if !wrap {
                let x = origin.x + RIGHT_MARGIN_COLUMN as f32 * char_w;
                guides.push(Shape::line_segment([Pos2::new(x, clip.top()), Pos2::new(x, clip.bottom())], Stroke::new(1.0, t::RIGHT_MARGIN)));
            }
            ui.painter().set(guides_bg, Shape::Vec(guides));
            for line in first..last {
                let color = if caret_line == Some(line) { t::LINE_NUMBER_CARET } else { t::LINE_NUMBER };
                p.text(
                    Pos2::new(gutter.right() - 16.0 * z, origin.y + spans[line].0 + row_h / 2.0),
                    egui::Align2::RIGHT_CENTER,
                    (line + 1).to_string(),
                    font.clone(),
                    color,
                );
            }
            // the annotation column; the line the pointer is on in it is washed in green, across the gutter and the code
            if let Some(a) = annotation {
                let column = Rect::from_min_size(gutter.min, Vec2::new(ann_w, gutter.height()));
                let hovered = ui.input(|i| i.pointer.hover_pos()).filter(|q| p.clip_rect().contains(*q));
                if let Some(line) = paint_column(&p, column, origin.y, &spans, first..last, a, &vision_font, hovered, z) {
                    annotated = Some(line);
                    let (y0, y1) = (origin.y + spans[line].0, origin.y + spans[line].1);
                    ui.painter()
                        .set(hover_bg, Shape::rect_filled(Rect::from_min_max(Pos2::new(clip.left(), y0), Pos2::new(clip.right(), y1)), 0, hover_line()));
                    p.rect_filled(Rect::from_min_max(Pos2::new(column.right(), y0), Pos2::new(gutter.right(), y1)), 0, hover_line());
                }
            }
            // the row opened above each declaration: its usages, then who wrote it, from the declaration's column;
            // under the pointer each is a link (to the list of usages, to the history of the declaration's lines)
            let pointer = resp.hover_pos();
            let link = |ui: &egui::Ui, text: Rect, whole: Rect, color: Color32| {
                let over = pointer.is_some_and(|p| whole.expand(2.0 * z).contains(p));
                if over {
                    ui.painter().hline(text.x_range(), text.bottom(), Stroke::new(1.0, color));
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                over && resp.clicked()
            };
            for (i, label) in l.labels.iter().enumerate().filter(|(_, label)| (first..=last).contains(&label.line)) {
                let Some(span) = spans.get(label.line) else { continue };
                let y = origin.y + span.0 - row_h / 2.0;
                let mut x = origin.x + l.cols.get(label.line).copied().flatten().unwrap_or(0) as f32 * char_w;
                if !label.usages.is_empty() {
                    let r = ui.painter().text(Pos2::new(x, y), egui::Align2::LEFT_CENTER, &label.usages, vision_font.clone(), t::SECONDARY);
                    if link(ui, r, r, t::SECONDARY) {
                        click = Some(Click::Usages(i, r.left_bottom()));
                    }
                    x = r.right() + 12.0 * z;
                }
                if !label.author.is_empty() {
                    person_icon(ui.painter(), Pos2::new(x + 5.0 * z, y), t::AUTHOR, z);
                    let r = ui.painter().text(Pos2::new(x + 14.0 * z, y), egui::Align2::LEFT_CENTER, &label.author, vision_font.clone(), t::AUTHOR);
                    if link(ui, r, r.with_min_x(x), t::AUTHOR) {
                        click = Some(Click::Author(i, Pos2::new(x, r.bottom())));
                    }
                }
            }
            // parameter names: a box in the gap before each argument (hints::open); the text stays where it was
            for (h, _, w) in l.hints.iter().filter(|hint| (first..last).contains(&hint.1)) {
                let r = out.galley.pos_from_cursor(CCursor::new(h.ch));
                let right = origin.x + r.min.x - hints::AFTER * z;
                let b = Rect::from_min_max(
                    Pos2::new(right - w - 2.0 * hints::PAD * z, origin.y + r.min.y + 2.0 * z),
                    Pos2::new(right, origin.y + r.max.y - 2.0 * z),
                );
                ui.painter().rect_filled(b, cr(4.0 * z), hints::LABEL_BG);
                ui.painter().text(b.center(), egui::Align2::CENTER_CENTER, &h.label, label_font.clone(), hints::LABEL_TEXT);
            }
            // Ctrl+D: "Mostrar Diff"
            if resp.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::D)) {
                click = Some(Click::Diff);
            }
            // the problems' wavy lines (the message in a tooltip), and F2 / Shift+F2 to the next / previous one
            let under = waves(&ui.painter().clone(), &out.galley, origin, diags, &problem_spans(text, diags, first..last), z, resp.hover_pos());
            if let Some(d) = under {
                resp.clone().on_hover_ui_at_pointer(|ui| tip(ui, d));
            }
            if ui.memory(|m| m.has_focus(id)) {
                let back = ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::F2));
                let forward = !back && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F2));
                if let Some(d) = (back || forward).then(|| next_problem(diags, at, forward)).flatten() {
                    click = Some(Click::Goto(d.line, d.col));
                }
            }
            // "Comentar para o Maestro": the right-click menu or Ctrl+Alt+M, on the selected block
            // the block as it was when the menu opened: the click on the menu would reach the text under it
            let memo = id.with("menu_pick");
            let mut picked = out.state.cursor.char_range().map(|r| r.as_sorted_char_range()).unwrap_or_default();
            if egui::Popup::is_any_open(ui.ctx()) {
                picked = ui.data(|d| d.get_temp(memo)).unwrap_or(picked);
            } else {
                ui.data_mut(|d| d.insert_temp(memo, picked.clone()));
            }
            let mut ask = false;
            egui::Popup::context_menu(&resp).frame(crate::app::ui::menu_look::menu_frame(false).fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER))).show(
                |ui| {
                    t::style(ui.style_mut());
                    if ui.add_enabled(picked.start != picked.end, egui::Button::new("Comentar para o Maestro").shortcut_text("Ctrl+Alt+M")).clicked() {
                        ask = true;
                        ui.close();
                    }
                },
            );
            ask |= resp.has_focus() && ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, Key::M)));
            if ask {
                let end = origin + out.galley.pos_from_cursor(CCursor::new(picked.end.0)).max.to_vec2();
                if let Some(req) = crate::app::chat::code_comment::note_req(path, lang.id, text, (picked.start.0, picked.end.0), end) {
                    click = Some(Click::Comment(req));
                }
            }
            // Ctrl+Shift+F: "Localizar nos Arquivos", on the text selected in one line
            if resp.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::F)) {
                let picked = out.state.cursor.char_range().map(|r| r.as_sorted_char_range()).unwrap_or_default();
                let picked: String = text.chars().skip(picked.start.0).take(picked.end.0 - picked.start.0).collect();
                click = Some(Click::Find(if picked.contains('\n') { String::new() } else { picked }));
            }
            // Ctrl+hover: the name underlined like a link; Ctrl+click: its declaration (on the declaration, its usages)
            if ui.input(|i| i.modifiers.command)
                && let Some(at) = resp.hover_pos()
            {
                let c = out.galley.cursor_from_pos(at - origin).index.0;
                let (a, b) = word_at(text, c);
                if b > a {
                    let (ra, rb) = (out.galley.pos_from_cursor(CCursor::new(a)), out.galley.pos_from_cursor(CCursor::new(b)));
                    let y = origin.y + ra.max.y - 3.0 * z;
                    ui.painter().line_segment([Pos2::new(origin.x + ra.min.x, y), Pos2::new(origin.x + rb.min.x, y)], Stroke::new(1.0, t::LINK));
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    if resp.clicked() {
                        click = Some(Click::Name(c, Pos2::new(origin.x + ra.min.x, origin.y + ra.max.y)));
                    }
                }
            }
            if let Some(span) = landed.and_then(|line| spans.get(line)) {
                ui.scroll_to_rect(Rect::from_min_size(Pos2::new(origin.x, origin.y + span.0), Vec2::new(1.0, row_h)), Some(egui::Align::Center));
            }
        });
    });
    let gutter_r = gutter_menu(ui, gutter, id, &mut buf.annotate);
    // over a line of the annotation column, its commit beside the pointer; a press lists the files it changed
    if let Some((commit, hint)) = annotated.and_then(|line| annotation?.commit_at(line)) {
        let r = gutter_r.on_hover_cursor(CursorIcon::PointingHand).on_hover_text_at_pointer(hint);
        if r.clicked() {
            click = Some(Click::Commit(commit.clone(), r.interact_pointer_pos().unwrap_or(gutter.left_top())));
        }
    }
    if let Some((origin_y, spans)) = geom {
        let out = crate::app::filetree::diff::marks_ui(ui, &p, gutter, origin_y, &spans, buf, id, z);
        changed |= out.changed;
        if out.diff {
            click = Some(Click::Diff);
        }
    }
    if let Some((line, col)) = chrome(ui, area, stripe, lines, diags, at, z, id.with("problems")) {
        click = Some(Click::Goto(line, col));
    }
    (changed, click)
}
