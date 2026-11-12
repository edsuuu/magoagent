use super::*;

impl Ide {
    pub(super) fn editor(&mut self, ui: &mut Ui) {
        if ui.rect_contains_pointer(ui.max_rect()) {
            let zoom = ui.input(|i| if i.modifiers.command { i.zoom_delta() } else { 1.0 });
            self.font_size = (self.font_size * zoom).clamp(8.0, 40.0);
        }
        let font = t::mono(self.font_size);
        let (natural, char_w) = ui.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
        let row_h = (natural * 1.2).round(); // IntelliJ default line spacing
        let diags: Vec<lsp::Diag> = match (&self.insp, self.tabs.get(self.active)) {
            (Some(i), Some(tab)) => i.for_file(&tab.path).into_iter().cloned().collect(),
            _ => vec![],
        };
        let (root, repo) = (self.root.clone(), self.git.repo);
        let (index_gen, code_vision) = (self.index_gen, self.code_vision);
        if code_vision {
            self.blame_request();
        }
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        let marks = git::marks(root.as_deref().filter(|_| repo), tab);
        // Code Vision: "3 usages · 1 implementation" above declarations (PhpStorm "Top" position). Code Author
        // (who wrote most of a declaration, from git blame) goes on the same line, after it.
        let text_hash = hash(&tab.text, 0.0);
        let code = code_vision && tab.lang.kind == highlight::Kind::Code;
        let code_text = tab.lang.kind == highlight::Kind::Code;
        let vision_stale = code && tab.vision.as_ref().is_none_or(|v| v.0 != (text_hash, index_gen));
        let author_stale = code && tab.authors.as_ref().is_none_or(|a| a.0 != (text_hash, self.blame_gen));
        if vision_stale || author_stale {
            let decls = index::declarations(tab.lang.id, &tab.text);
            if vision_stale {
                let v = decls.iter().filter_map(|d| self.index.vision(d).map(|t| (d.line, t, d.name.clone()))).collect();
                tab.vision = Some(((text_hash, index_gen), v));
            }
            if author_stale {
                let lines: Vec<&str> = tab.text.split('\n').collect();
                let blame = self.blame.get(&tab.path);
                let a = decls.iter().filter_map(|d| blame.and_then(|b| decl_author(b, decl_span(&lines, d.line))).map(|n| (d.line, n))).collect();
                tab.authors = Some(((text_hash, self.blame_gen), a));
            }
        }
        // parameter names inside calls: the server's when it offers them, else the project index's (hints.rs)
        let answer = self.insp.as_ref().and_then(|i| i.inlays.as_ref()).filter(|a| a.path == tab.path && a.hash == text_hash);
        let inlay_key = (text_hash, self.index_gen, answer.map_or(0, |a| a.seq));
        if code_text && tab.inlay.as_ref().is_none_or(|h| h.0 != inlay_key) {
            let lsp = self.insp.as_ref().is_some_and(|i| i.inlay_server(tab.lang));
            let found = match answer {
                _ if !lsp => hints::from_index(&hints::calls(tab.lang, &tab.text), &self.index, &tab.path),
                Some(a) => hints::from_lsp(&tab.text, &hints::calls(tab.lang, &tab.text), &a.hints),
                None => vec![],
            };
            tab.inlay = Some((inlay_key, hints::finish(&tab.text, found)));
        }
        let inlays: Vec<hints::Hint> = tab.inlay.as_ref().map(|h| h.1.clone()).unwrap_or_default();
        let label_font = t::ui(font.size - 1.0); // the UI face, a little smaller than the code
        let label_w: Vec<f32> = inlays.iter().map(|h| hints::width(ui, &h.label, &label_font)).collect();
        let hint_gaps: Vec<(usize, f32)> = inlays.iter().zip(&label_w).map(|(h, w)| (h.byte, hints::gap(*w))).collect();
        let vision: Vec<(usize, String, String)> = if code_vision { tab.vision.as_ref().map(|v| v.1.clone()).unwrap_or_default() } else { vec![] };
        let authors: Vec<(usize, String)> = if code_vision { tab.authors.as_ref().map(|a| a.1.clone()).unwrap_or_default() } else { vec![] };
        // a row opened above each labelled declaration (see open_gaps)
        let mut gap_lines: Vec<usize> = vision.iter().map(|v| v.0).chain(authors.iter().map(|a| a.0)).collect();
        gap_lines.sort_unstable();
        gap_lines.dedup();
        let gaps: Vec<(usize, f32)> = gap_lines.into_iter().map(|l| (l, row_h)).collect();
        let lines = tab.text.matches('\n').count() + 1;
        let gutter_w = lines.to_string().len().max(3) as f32 * char_w + 26.0;
        // indent guides: one per block, at its opening line's column; the caret's block gets the lighter one
        let cols: Vec<Option<usize>> = tab.text.split('\n').map(indent_of).collect();
        let blocks = indent_guides(&tab.text);
        let active = caret_guide(&blocks, tab.cursor.0);
        let id = self.id.with(("editor", &tab.path));

        if let Some((a, b)) = tab.select.take() {
            let mut st = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
            st.store(ui.ctx(), id);
            ui.memory_mut(|m| m.request_focus(id));
            tab.scroll_to = Some(line_of(&tab.text, a));
        }

        // the bracket next to the caret (the one before it first) and its pair; the layouter and the paint below use it
        let bracket = TextEdit::load_state(ui.ctx(), id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.0).and_then(|c| {
            let at = |ci: usize| tab.text.char_indices().nth(ci).map(|(b, _)| b);
            [c.checked_sub(1), Some(c)].into_iter().flatten().find_map(|ci| at(ci).and_then(|b| highlight::pair(tab.lang, &tab.text, b)))
        });
        let mut ctrl_click = None;
        let mut vision_click = None;
        // the language server's answers this frame, and what to ask it next
        let hover = self.insp.as_ref().and_then(|i| i.hover.clone());
        let signature = self.insp.as_ref().and_then(|i| i.signature.clone());
        let mut hover_ask = None;
        let mut signature_ask = None;
        egui::ScrollArea::both().id_salt(id).auto_shrink(false).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let (gutter, _) = ui.allocate_exact_size(vec2(gutter_w, (lines + gaps.len()) as f32 * row_h), Sense::hover());
                let caret_bg = ui.painter().add(Shape::Noop);
                let guides_bg = ui.painter().add(Shape::Noop);
                let Tab { text, cache, lang, .. } = tab;
                let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
                    let s = buf.as_str();
                    let h = hash(s, font.size);
                    if cache.as_ref().is_none_or(|c| c.0 != h) {
                        *cache = Some((h, highlight::layout(lang, s, font.clone(), row_h)));
                    }
                    let mut job = cache.as_ref().unwrap().1.clone();
                    job.wrap.max_width = f32::INFINITY;
                    hints::open(&mut job, &hint_gaps);
                    if let Some((own, _, pair)) = &bracket {
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
                    open_gaps(std::sync::Arc::make_mut(&mut galley), &gaps);
                    galley
                };
                // Esc is egui's "drop focus" (and then the IDE's "back to the canvas") unless a hint is open
                let keys = egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: signature.is_some() };
                // a right click, or a click on its menu, keeps the selection (the TextEdit would put the caret under the pointer)
                let before = egui::TextEdit::load_state(ui.ctx(), id).and_then(|s| s.cursor.char_range()).filter(|r| !r.is_empty());
                let right = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
                    || (egui::Popup::is_any_open(ui.ctx()) && ui.input(|i| i.pointer.any_pressed()));
                let mut out = TextEdit::multiline(text)
                    .id(id)
                    .font(font.clone())
                    .code_editor()
                    .frame(Frame::NONE)
                    .event_filter(keys)
                    .desired_width(f32::INFINITY)
                    .desired_rows(lines)
                    .margin(Margin::symmetric(6, 0))
                    .layouter(&mut layouter)
                    .show(ui);
                let resp = out.response.response.clone();
                if let Some(kept) = before.filter(|_| right) {
                    out.state.cursor.set_char_range(Some(kept));
                    out.state.clone().store(ui.ctx(), id);
                    out.cursor_range = Some(kept);
                }
                let comment_path = tab.path.clone();
                let (sel_start, sel_end) = tab.sel;
                let caret_line = tab.cursor.0;
                let mut ask_maestro = false;
                t::context_menu(&resp).show(|ui| {
                    if ui.button("Comentar com agente…").clicked() {
                        let (from, to) = if sel_start != sel_end {
                            (line_of(&tab.text, sel_start) + 1, line_of(&tab.text, sel_end.saturating_sub(1)) + 1)
                        } else {
                            (caret_line + 1, caret_line + 1)
                        };
                        self.code_comment = Some((comment_path.clone(), from, to, String::new()));
                        ui.close();
                    }
                    // the selection's popover, the app's (app/chat/code_comment.rs)
                    let has_sel = sel_start != sel_end;
                    if ui.add_enabled(has_sel, egui::Button::new("Comentar para o Maestro").shortcut_text("Ctrl+Alt+M")).clicked() {
                        ask_maestro = true;
                        ui.close();
                    }
                });
                if resp.has_focus() && ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, Key::M))) {
                    ask_maestro = true;
                }
                if ask_maestro {
                    let end = out.galley_pos + out.galley.pos_from_cursor(egui::text::CCursor::new(sel_end.max(sel_start))).max.to_vec2();
                    self.note_req = crate::app::chat::code_comment::note_req(&tab.path, tab.lang.id, &tab.text, (sel_start, sel_end), end);
                }
                if resp.changed() {
                    tab.dirty = true;
                    tab.lsp_pending = Some(Instant::now());
                    // a call being typed: '(' or ',' asks for its parameters, ')' puts them away
                    if let Some(r) = out.cursor_range {
                        let c = r.primary.index.0;
                        match tab.text.chars().nth(c.wrapping_sub(1)) {
                            Some('(' | ',') => signature_ask = Some(true),
                            Some(')') => signature_ask = Some(false),
                            _ => {}
                        }
                    }
                }
                if out.cursor_range.is_some() && ui.input(|i| i.key_pressed(Key::Escape)) {
                    signature_ask = Some(false);
                }
                if let Some(r) = out.cursor_range {
                    let range = r.as_sorted_char_range();
                    tab.sel = (range.start.0, range.end.0);
                    let c = r.primary.index.0;
                    let line = line_of(&tab.text, c);
                    let col = c - char_at(&tab.text, line, 0);
                    tab.cursor = (line, col);
                }
                let origin = out.galley_pos;
                // A dot per space, a bar per tab.
                if SHOW_WHITESPACES {
                    let clip = ui.clip_rect();
                    for row in &out.galley.rows {
                        let y = origin.y + row.rect().center().y;
                        if y < clip.top() - row_h || y > clip.bottom() + row_h {
                            continue;
                        }
                        for g in &row.row.glyphs {
                            let (x, w) = (origin.x + row.pos.x + g.pos.x, g.advance_width);
                            match g.chr {
                                ' ' => {
                                    ui.painter().circle_filled(pos2(x + w / 2.0, y), 1.0, t::WHITESPACES);
                                }
                                '\t' => {
                                    ui.painter().line_segment([pos2(x + 2.0, y), pos2(x + w - 2.0, y)], Stroke::new(1.0, t::WHITESPACES));
                                }
                                _ => {}
                            }
                        }
                    }
                }
                // no bold face here: the matched pair's glyphs are drawn a second time, 0.6 px to the right
                if let Some((own, st, Some(p))) = &bracket {
                    for r in [own, p] {
                        let at = out.galley.pos_from_cursor(CCursor::new(tab.text[..r.start].chars().count()));
                        ui.painter().text(
                            origin + at.min.to_vec2() + vec2(0.6, 0.0),
                            Align2::LEFT_TOP,
                            &tab.text[r.clone()],
                            font.clone(),
                            highlight::color(*st),
                        );
                    }
                }
                let clip = ui.clip_rect();
                // rows are logical lines (no wrapping); their heights vary where Code Vision sits
                let rows = &out.galley.rows;
                let row_top = |l: usize| -> f32 {
                    match rows.get(l) {
                        Some(r) => origin.y + r.rect().top(),
                        None => origin.y + rows.last().map_or(0.0, |r| r.rect().bottom()) + l.saturating_sub(rows.len()) as f32 * row_h,
                    }
                };
                let caret_y = row_top(tab.cursor.0);
                ui.painter()
                    .set(caret_bg, Shape::rect_filled(Rect::from_min_max(pos2(clip.left(), caret_y), pos2(clip.right(), caret_y + row_h)), 0, t::CARET_ROW));
                let first = rows.partition_point(|r| origin.y + r.rect().bottom() < clip.top());
                let last = rows.partition_point(|r| origin.y + r.rect().top() < clip.bottom()).min(lines);
                // the gutter stays put when the text scrolls sideways: over the text, on its own background
                let gutter_r = gutter.right().max(clip.left() + gutter_w);
                if gutter.left() < clip.left() {
                    ui.painter().rect_filled(Rect::from_min_max(clip.min, pos2(gutter_r, clip.bottom())), 0, t::ISLAND);
                    ui.painter().rect_filled(Rect::from_min_max(pos2(clip.left(), caret_y), pos2(gutter_r, caret_y + row_h)), 0, t::CARET_ROW);
                }
                // VCS change markers (ADDED/MODIFIED/DELETED_LINES_COLOR), right of the line numbers
                for &(a, b, kind) in &marks {
                    let x = gutter_r - 7.0;
                    let y0 = row_top(a);
                    match kind {
                        git::MarkKind::Deleted => {
                            let c = Color32::from_rgb(0x86, 0x8A, 0x91);
                            ui.painter().add(Shape::convex_polygon(vec![pos2(x - 1.0, y0 - 4.0), pos2(x + 5.0, y0), pos2(x - 1.0, y0 + 4.0)], c, Stroke::NONE));
                        }
                        _ => {
                            let c = if kind == git::MarkKind::Added { Color32::from_rgb(0x54, 0x91, 0x59) } else { Color32::from_rgb(0x37, 0x5F, 0xAD) };
                            ui.painter().rect_filled(Rect::from_min_max(pos2(x, y0 + 1.0), pos2(x + 4.0, row_top(b.max(a + 1) - 1) + row_h - 1.0)), 1, c);
                        }
                    }
                }
                // guides go under the text (the placeholder above): one line a block, from under its opening line to
                // its last line, through the rows opened above the declarations inside it (their labels start further in)
                let mut guides = vec![];
                for &(col, start, end) in &blocks {
                    let (a, b) = ((start + 1).max(first), end.min(last));
                    if a < b {
                        // at the column's own left edge (the galley origin already has the margin), under the
                        // opening line's first letter
                        let x = origin.x + col as f32 * char_w;
                        let color = if active == Some((col, start, end)) { t::INDENT_GUIDE_SELECTED } else { t::INDENT_GUIDE };
                        guides.push(Shape::line_segment([pos2(x, row_top(a - 1) + row_h), pos2(x, row_top(b - 1) + row_h)], Stroke::new(1.0, color)));
                    }
                }
                // The galley origin already includes the text edit's margin.
                let wrap = origin.x + RIGHT_MARGIN_COLUMN as f32 * char_w;
                guides.push(Shape::line_segment([pos2(wrap, clip.top()), pos2(wrap, clip.bottom())], Stroke::new(1.0, t::RIGHT_MARGIN)));
                ui.painter().set(guides_bg, Shape::Vec(guides));
                for l in first..last {
                    let color = if l == tab.cursor.0 { t::LINE_NUMBER_CARET } else { t::LINE_NUMBER };
                    ui.painter().text(pos2(gutter_r - 16.0, row_top(l) + row_h / 2.0), Align2::RIGHT_CENTER, (l + 1).to_string(), font.clone(), color);
                }
                // the space opened above each declaration: Code Vision, then the Code Author after it, both
                // starting at the declaration's column
                let pointer = ui.input(|i| i.pointer.hover_pos());
                for &(line, _) in &gaps {
                    let y = row_top(line) - row_h / 2.0;
                    if y < clip.top() - row_h || y > clip.bottom() + row_h {
                        continue;
                    }
                    let mut x = origin.x + cols.get(line).copied().flatten().unwrap_or(0) as f32 * char_w;
                    if let Some((_, label, name)) = vision.iter().find(|v| v.0 == line) {
                        let g = ui.painter().text(pos2(x, y), Align2::LEFT_CENTER, label, t::ui(t::UI_SIZE - 1.0), t::SECONDARY);
                        if pointer.is_some_and(|p| g.expand(2.0).contains(p)) {
                            ui.painter().line_segment([pos2(g.left(), g.bottom()), pos2(g.right(), g.bottom())], Stroke::new(1.0, t::SECONDARY));
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            if ui.input(|i| i.pointer.primary_clicked()) {
                                vision_click = Some(name.clone());
                            }
                        }
                        x = g.right() + 12.0;
                    }
                    if let Some((_, name)) = authors.iter().find(|a| a.0 == line) {
                        person_icon(ui.painter(), pos2(x + 5.0, y), t::AUTHOR, 1.0);
                        ui.painter().text(pos2(x + 14.0, y), Align2::LEFT_CENTER, name, t::ui(t::UI_SIZE - 1.0), t::AUTHOR);
                    }
                }
                // parameter names: a box in the gap before each argument (hints::open); the text stays where it was
                for (h, &w) in inlays.iter().zip(&label_w) {
                    let r = out.galley.pos_from_cursor(CCursor::new(h.ch));
                    let (y0, y1) = (origin.y + r.min.y, origin.y + r.max.y);
                    if y1 < clip.top() || y0 > clip.bottom() {
                        continue;
                    }
                    let right = origin.x + r.min.x - hints::AFTER;
                    let b = Rect::from_min_max(pos2(right - w - 2.0 * hints::PAD, y0 + 2.0), pos2(right, y1 - 2.0));
                    ui.painter().rect_filled(b, 4, hints::LABEL_BG);
                    ui.painter().text(b.center(), Align2::CENTER_CENTER, &h.label, label_font.clone(), hints::LABEL_TEXT);
                }
                // inspection squiggles + hover message
                if !diags.is_empty() {
                    let starts = line_starts(&tab.text);
                    let mut msgs = vec![];
                    for d in diags.iter().filter(|d| d.severity <= 2 && d.line < starts.len()) {
                        let line_end = starts.get(d.line + 1).map_or(tab.text.chars().count(), |s| s - 1);
                        let mut a = (starts[d.line] + d.col).min(line_end);
                        if d.col == 0 {
                            let indent = tab.text[byte_at(&tab.text, starts[d.line])..].chars().take_while(|c| *c == ' ' || *c == '\t').count();
                            a = (a + indent).min(line_end);
                        }
                        let mut b = if d.end_line == d.line { (starts[d.line] + d.end_col).min(line_end) } else { line_end };
                        if b <= a {
                            b = (a + 1).min(line_end.max(a + 1));
                        }
                        let ra = out.galley.pos_from_cursor(CCursor::new(a));
                        let rb = out.galley.pos_from_cursor(CCursor::new(b));
                        let (x0, x1) = (origin.x + ra.min.x, origin.x + rb.min.x.max(ra.min.x + char_w));
                        let y = origin.y + ra.min.y + row_h - 1.0;
                        let color = if d.severity == 1 { t::ERROR } else { t::WARNING };
                        let mut pts = vec![];
                        let mut x = x0;
                        let mut up = true;
                        while x <= x1 {
                            pts.push(pos2(x, if up { y - 1.5 } else { y + 0.5 }));
                            x += 2.0;
                            up = !up;
                        }
                        ui.painter().add(Shape::line(pts, Stroke::new(1.0, color)));
                        if let Some(p) = pointer
                            && Rect::from_min_max(pos2(x0, y - row_h), pos2(x1, y + 2.0)).contains(p)
                        {
                            msgs.push(format!("{}  ({})", d.message, d.source));
                        }
                    }
                    if !msgs.is_empty() {
                        resp.clone().on_hover_text_at_pointer(msgs.join("\n"));
                    }
                }
                // the name under the pointer: its documentation from the language server, in a popup that stays open
                // while the pointer is on the name or on the popup (docs.rs)
                let resting = ui.input(|i| !i.modifiers.command && !i.pointer.any_down() && i.pointer.time_since_last_movement() > 0.5);
                let pointer = ui.input(|i| i.pointer.hover_pos());
                if resp.hover_pos().is_some() {
                    ui.ctx().request_repaint_after(Duration::from_millis(500));
                }
                let word = resp.hover_pos().and_then(|p| {
                    let c = out.galley.cursor_from_pos(p - origin).index.0;
                    let (a, b) = word_at(&tab.text, c);
                    (b > a).then(|| {
                        let line = line_of(&tab.text, a);
                        let at = (tab.path.clone(), line, a - char_at(&tab.text, line, 0));
                        let (ra, rb) = (out.galley.pos_from_cursor(CCursor::new(a)), out.galley.pos_from_cursor(CCursor::new(b)));
                        (at, Rect::from_min_max(origin + ra.min.to_vec2(), pos2(origin.x + rb.min.x, origin.y + ra.max.y)))
                    })
                });
                let on_doc = self.doc.as_ref().and_then(|d| d.rect).zip(pointer).is_some_and(|(r, p)| r.expand(12.0).contains(p));
                match word {
                    Some((at, anchor)) if resting || self.doc.as_ref().is_some_and(|d| d.at == at) => {
                        if !hover.as_ref().is_some_and(|(answered, _)| *answered == at) {
                            hover_ask = Some(at.clone());
                        }
                        let rect = self.doc.take().filter(|d| d.at == at).and_then(|d| d.rect);
                        self.doc = Some(docs::Popup { at, anchor, rect });
                    }
                    _ if on_doc => {}
                    _ => self.doc = None,
                }
                if let Some(d) = self.doc.as_mut()
                    && let Some((answered, md)) = hover.as_ref()
                    && *answered == d.at
                {
                    d.rect = Some(docs::show(ui.ctx(), id.with("doc"), d.anchor, tab.lang, md));
                }
                // the parameters of the call being typed, over the caret, the current one bold
                // (a caret means the editor has the keyboard; this response's own focus flag doesn't)
                if let (Some((label, active)), Some(r)) = (&signature, out.cursor_range) {
                    {
                        let caret = origin + out.galley.pos_from_cursor(CCursor::new(r.primary.index.0)).min.to_vec2();
                        let mut job = egui::text::LayoutJob::default();
                        let (a, b) = active.unwrap_or((label.len(), label.len()));
                        for (part, color) in [(&label[..a], t::MUTED), (&label[a..b], t::TEXT), (&label[b..], t::MUTED)] {
                            job.append(part, 0.0, egui::TextFormat::simple(t::mono(12.5), color));
                        }
                        egui::Area::new(id.with("signature")).order(Order::Tooltip).fixed_pos(caret - vec2(0.0, row_h + 12.0)).show(ui.ctx(), |ui| {
                            Frame::popup(ui.style()).fill(t::POPUP).show(ui, |ui| ui.label(job));
                        });
                    }
                }
                // Ctrl+hover: underline the symbol like a link; Ctrl+click: go to declaration
                if ui.input(|i| i.modifiers.command)
                    && let Some(p) = resp.hover_pos()
                {
                    let c = out.galley.cursor_from_pos(p - origin).index.0;
                    let (a, b) = word_at(&tab.text, c);
                    if b > a {
                        let ra = out.galley.pos_from_cursor(CCursor::new(a));
                        let rb = out.galley.pos_from_cursor(CCursor::new(b));
                        let y = origin.y + ra.min.y + row_h - 3.0;
                        ui.painter().line_segment([pos2(origin.x + ra.min.x, y), pos2(origin.x + rb.min.x, y)], Stroke::new(1.0, t::LINK));
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        if resp.clicked() {
                            ctrl_click = Some(c);
                        }
                    }
                }
                if let Some(line) = tab.scroll_to.take() {
                    ui.scroll_to_rect(Rect::from_min_size(pos2(origin.x, row_top(line)), vec2(1.0, row_h)), Some(Align::Center));
                }
            });
        });
        if let Some(c) = ctrl_click {
            self.goto_declaration(c);
        }
        if let (Some((path, line, col)), Some(insp), Some(tab)) = (hover_ask, &mut self.insp, self.tabs.get(self.active)) {
            insp.ask(lsp::Ask::Hover, &path, tab.lang, line, col);
        }
        let here = self.tabs.get(self.active).map(|t| (t.path.clone(), t.cursor.0));
        if signature_ask == Some(true) {
            if let (Some(insp), Some(tab)) = (&mut self.insp, self.tabs.get_mut(self.active)) {
                // the server must see the '(' first: the edit goes now, not after the typing pause
                tab.lsp_pending = None;
                insp.change(&tab.path, tab.lang, &tab.text);
                insp.ask(lsp::Ask::Signature, &tab.path, tab.lang, tab.cursor.0, tab.cursor.1);
                self.sig_line = here.clone();
            }
        }
        // closed with ')' or Esc, or the caret left the call's line
        if signature_ask == Some(false) || (self.sig_line.is_some() && self.sig_line != here) {
            self.sig_line = None;
            if let Some(insp) = &mut self.insp {
                insp.forget(lsp::Ask::Signature);
                insp.signature = None;
            }
        }
        if let Some(name) = vision_click {
            // PhpStorm opens the usages list; ours: Find in Files on the whole word
            self.open_find(Some(name));
            if let Some(f) = &mut self.find_files {
                f.words = true;
                f.case = true;
            }
        }
    }
}
