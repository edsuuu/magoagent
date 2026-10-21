//! The editor: tabs, find bar, the highlighted text with gutter marks, squiggles and Code Vision.

use super::*;

impl Ide {
    pub(super) fn center(&mut self, ui: &mut Ui) {
        if self.tabs.is_empty() {
            return self.empty_editor(ui);
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.tab_bar(ui);
        if self.tabs[self.active].diff.is_some() {
            return self.diff_view(ui);
        }
        if self.tabs[self.active].sheet.is_some() {
            return self.sheet_view(ui);
        }
        if self.find.is_some() {
            self.find_bar(ui);
        }
        self.inspection_widget(ui);
        self.editor(ui);
    }

    fn empty_editor(&mut self, ui: &mut Ui) {
        if self.root.is_none() {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.35);
                ui.label(egui::RichText::new(&self.status).size(16.0).color(t::TEXT));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Escolha outra pasta em Editar, na barra do IDE.").color(t::MUTED));
            });
            return;
        }
        let c = ui.max_rect().center();
        let hints = [("Pesquisar em tudo", "Shift duas vezes"), ("Ir para arquivo", "Ctrl+Shift+N  ·  Ctrl+P"), ("Localizar ação", "Ctrl+Shift+A  ·  Ctrl+Shift+P"), ("Projeto", "Alt+1"), ("Problemas", "Alt+6")];
        for (i, (what, keys)) in hints.iter().enumerate() {
            let y = c.y - 60.0 + i as f32 * 28.0;
            ui.painter().text(pos2(c.x - 8.0, y), Align2::RIGHT_CENTER, what, t::ui(14.0), t::MUTED);
            ui.painter().text(pos2(c.x + 8.0, y), Align2::LEFT_CENTER, super::chrome::keys(keys), t::ui(14.0), t::LINK);
        }
    }

    fn tab_bar(&mut self, ui: &mut Ui) {
        let mut close = None;
        egui::ScrollArea::horizontal().id_salt("tabs").max_height(38.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.set_height(38.0);
                for (i, tab) in self.tabs.iter().enumerate() {
                    let name = match (tab.diff.as_ref(), tab.sheet.as_ref()) {
                        (Some(d), _) => d.title.clone().unwrap_or_else(|| file_name(&tab.path) + " (diff)"),
                        (_, Some(sheet)) => sheet.table.clone().unwrap_or_else(|| format!("Console · {}", file_name(&tab.path))),
                        _ => file_name(&tab.path),
                    };
                    let galley = ui.painter().layout_no_wrap(name, t::ui(t::UI_SIZE), t::TEXT);
                    let w = galley.size().x + 56.0;
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                    let selected = i == self.active;
                    let p = ui.painter();
                    if selected {
                        p.rect(rect, 6, t::TAB_SELECTED, Stroke::new(1.0, t::TAB_BORDER), egui::StrokeKind::Inside);
                    } else if resp.hovered() {
                        p.rect_filled(rect, 6, t::HOVER);
                    }
                    let at = pos2(rect.left() + 16.0, rect.center().y);
                    // a table's tab is about the data, not the database file's git status
                    let base = if tab.sheet.is_some() {
                        icons::at(ui.painter(), icons::FT_SQL, at);
                        t::TEXT
                    } else {
                        icons::file(ui.painter(), &tab.path, at, 1.0);
                        self.file_status(&tab.path).map_or(t::TEXT, |s| s.color())
                    };
                    let color = if selected { base } else { base.gamma_multiply(0.75) }; // EditorTabs.unselectedAlpha
                    p.galley_with_override_text_color(pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0), galley, color);
                    let x = Rect::from_center_size(pos2(rect.right() - 14.0, rect.center().y), vec2(16.0, 16.0));
                    let over_x = ui.rect_contains_pointer(x);
                    if tab.dirty && !over_x {
                        p.circle_filled(x.center(), 3.5, t::MUTED);
                    } else if resp.hovered() || selected {
                        icons::paint(ui.painter(), if over_x { icons::CLOSE_SMALL_HOVER } else { icons::CLOSE_SMALL }, x);
                    }
                    if resp.clicked() {
                        if over_x { close = Some(i) } else { self.active = i }
                    }
                    if resp.middle_clicked() {
                        close = Some(i);
                    }
                    resp.on_hover_text(tab.path.display().to_string());
                }
            });
        });
        if let Some(i) = close {
            self.close_tab(i);
        }
    }

    fn find_bar(&mut self, ui: &mut Ui) {
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        let query = self.find.as_mut().unwrap();
        let mut close = false;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let (ir, _) = ui.allocate_exact_size(vec2(16.0, 24.0), Sense::hover());
            icons::at(ui.painter(), icons::SEARCH, ir.center());
            let r = ui.add(TextEdit::singleline(query).hint_text("Localizar").desired_width(280.0));
            if !r.has_focus() && ui.memory(|m| m.focused().is_none()) {
                r.request_focus();
            }
            let hay = tab.text.to_ascii_lowercase();
            let needle = query.to_ascii_lowercase();
            let hits: Vec<usize> = if needle.is_empty() { vec![] } else { hay.match_indices(&needle).map(|(b, _)| b).collect() };
            let cur_byte = byte_at(&tab.text, tab.sel.1);
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let back = ui.input(|i| i.modifiers.shift);
            let mut jump = |forward: bool| {
                let hit = if forward {
                    hits.iter().find(|&&b| b >= cur_byte).or(hits.first())
                } else {
                    let start = byte_at(&tab.text, tab.sel.0);
                    hits.iter().rev().find(|&&b| b < start).or(hits.last())
                };
                if let Some(&b) = hit {
                    let a = tab.text[..b].chars().count();
                    tab.select = Some((a, a + query.chars().count()));
                }
            };
            ui.label(egui::RichText::new(if needle.is_empty() { String::new() } else if hits.is_empty() { "0 resultados".into() } else { format!("{} resultados", hits.len()) }).color(t::MUTED));
            if ui.small_button("↑").clicked() || (enter && back) {
                jump(false);
            }
            if ui.small_button("↓").clicked() || (enter && !back) {
                jump(true);
            }
            if enter {
                r.request_focus();
            }
            if ui.small_button("×").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                close = true;
            }
        });
        if close {
            self.find = None;
        }
    }

    /// PhpStorm's top-right "✖ 2 ⚠ 5" widget.
    fn inspection_widget(&mut self, ui: &mut Ui) {
        let (Some(insp), Some(tab)) = (&self.insp, self.tabs.get(self.active)) else { return };
        let diags = insp.for_file(&tab.path);
        let errors = diags.iter().filter(|d| d.severity == 1).count();
        let warnings = diags.iter().filter(|d| d.severity == 2).count();
        let text = match (errors, warnings) {
            (0, 0) => "✔".to_string(),
            (e, 0) => format!("✖ {e}"),
            (0, w) => format!("⚠ {w}"),
            (e, w) => format!("✖ {e}  ⚠ {w}"),
        };
        let color = if errors > 0 { t::ERROR } else if warnings > 0 { t::WARNING } else { Color32::from_rgb(0x4E, 0x9D, 0x6C) };
        let rect = ui.max_rect();
        let r = Rect::from_min_size(pos2(rect.right() - 90.0, rect.top() + 44.0), vec2(80.0, 22.0));
        let resp = ui.interact(r, self.id.with("insp_widget"), Sense::click());
        ui.painter().text(r.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, text, t::ui(12.0), color);
        if resp.on_hover_text("Problemas (Alt+6)").clicked() {
            self.bottom = Some(ToolWindow::Problems);
        }
    }

    fn editor(&mut self, ui: &mut Ui) {
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
                let a = decls.iter()
                    .filter_map(|d| blame.and_then(|b| decl_author(b, decl_span(&lines, d.line))).map(|n| (d.line, n))).collect();
                tab.authors = Some(((text_hash, self.blame_gen), a));
            }
        }
        let vision: Vec<(usize, String, String)> = if code_vision { tab.vision.as_ref().map(|v| v.1.clone()).unwrap_or_default() } else { vec![] };
        let authors: Vec<(usize, String)> = if code_vision { tab.authors.as_ref().map(|a| a.1.clone()).unwrap_or_default() } else { vec![] };
        // a row opened above each labelled declaration (see open_gaps)
        let mut gap_lines: Vec<usize> = vision.iter().map(|v| v.0).chain(authors.iter().map(|a| a.0)).collect();
        gap_lines.sort_unstable();
        gap_lines.dedup();
        let gaps: Vec<(usize, f32)> = gap_lines.into_iter().map(|l| (l, row_h)).collect();
        let lines = tab.text.matches('\n').count() + 1;
        let gutter_w = lines.to_string().len().max(3) as f32 * char_w + 26.0;
        // indent guides: one column per indent step; the caret's block gets the lighter one
        let cols: Vec<Option<usize>> = tab.text.split('\n').map(indent_of).collect();
        let unit = indent_unit(&cols);
        let top = cols.iter().flatten().copied().max().unwrap_or(0);
        let active = (0..=top / unit).rev().map(|k| k * unit).find(|&col| guide_on(&cols, tab.cursor.0.min(lines - 1), col));
        let id = self.id.with(("editor", &tab.path));

        if let Some((a, b)) = tab.select.take() {
            let mut st = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
            st.store(ui.ctx(), id);
            ui.memory_mut(|m| m.request_focus(id));
            tab.scroll_to = Some(line_of(&tab.text, a));
        }

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
                    let mut galley = ui.fonts_mut(|f| f.layout_job(job));
                    open_gaps(std::sync::Arc::make_mut(&mut galley), &gaps);
                    galley
                };
                // Esc is egui's "drop focus" (and then the IDE's "back to the canvas") unless a hint is open
                let keys = egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: signature.is_some() };
                let out = TextEdit::multiline(text).id(id).font(font.clone()).code_editor().frame(Frame::NONE)
                    .event_filter(keys).desired_width(f32::INFINITY).desired_rows(lines).margin(Margin::symmetric(6, 0))
                    .layouter(&mut layouter).show(ui);
                let resp = out.response.response.clone();
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
                ui.painter().set(caret_bg, Shape::rect_filled(
                    Rect::from_min_max(pos2(clip.left(), caret_y), pos2(clip.right(), caret_y + row_h)), 0, t::CARET_ROW));
                let first = rows.partition_point(|r| origin.y + r.rect().bottom() < clip.top());
                let last = rows.partition_point(|r| origin.y + r.rect().top() < clip.bottom()).min(lines);
                // VCS change markers (ADDED/MODIFIED/DELETED_LINES_COLOR), right of the line numbers
                for &(a, b, kind) in &marks {
                    let x = gutter.right() - 7.0;
                    let y0 = row_top(a);
                    match kind {
                        git::MarkKind::Deleted => {
                            let c = Color32::from_rgb(0x86, 0x8A, 0x91);
                            ui.painter().add(Shape::convex_polygon(vec![pos2(x - 1.0, y0 - 4.0), pos2(x + 5.0, y0), pos2(x - 1.0, y0 + 4.0)], c, Stroke::NONE));
                        }
                        _ => {
                            let c = if kind == git::MarkKind::Added { Color32::from_rgb(0x54, 0x91, 0x59) } else { Color32::from_rgb(0x37, 0x5F, 0xAD) };
                            ui.painter().rect_filled(Rect::from_min_max(pos2(x, y0 + 1.0), pos2(x + 3.0, row_top(b.max(a + 1) - 1) + row_h - 1.0)), 1, c);
                        }
                    }
                }
                // guides go under the text (the placeholder above), one segment per line: the row opened above a
                // declaration has no guide, so it stays clear for the labels
                let mut guides = vec![];
                for k in 0..=top / unit {
                    let col = k * unit;
                    let x = origin.x + 6.0 + col as f32 * char_w;
                    let color = if active == Some(col) { t::INDENT_GUIDE_SELECTED } else { t::INDENT_GUIDE };
                    for l in first..last {
                        if guide_on(&cols, l, col) {
                            guides.push(Shape::line_segment([pos2(x, row_top(l)), pos2(x, row_top(l) + row_h)], Stroke::new(1.0, color)));
                        }
                    }
                }
                ui.painter().set(guides_bg, Shape::Vec(guides));
                for l in first..last {
                    let color = if l == tab.cursor.0 { t::LINE_NUMBER_CARET } else { t::LINE_NUMBER };
                    ui.painter().text(pos2(gutter.right() - 16.0, row_top(l) + row_h / 2.0), Align2::RIGHT_CENTER, (l + 1).to_string(), font.clone(), color);
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
                        person_icon(ui.painter(), pos2(x + 5.0, y), t::AUTHOR);
                        ui.painter().text(pos2(x + 14.0, y), Align2::LEFT_CENTER, name, t::ui(t::UI_SIZE - 1.0), t::AUTHOR);
                    }
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
                // resting on a name: its documentation from the language server
                let resting = ui.input(|i| !i.modifiers.command && !i.pointer.any_down() && i.pointer.time_since_last_movement() > 0.5);
                if let Some(p) = resp.hover_pos() {
                    ui.ctx().request_repaint_after(Duration::from_millis(500));
                    let c = out.galley.cursor_from_pos(p - origin).index.0;
                    let (a, b) = word_at(&tab.text, c);
                    if resting && b > a {
                        let line = line_of(&tab.text, a);
                        let at = (tab.path.clone(), line, a - char_at(&tab.text, line, 0));
                        match &hover {
                            Some((answered, text)) if *answered == at => {
                                resp.clone().on_hover_ui_at_pointer(|ui| {
                                    ui.set_max_width(560.0);
                                    ui.label(egui::RichText::new(text).font(t::mono(12.0)).color(t::TEXT));
                                });
                            }
                            _ => hover_ask = Some(at),
                        }
                    }
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

/// Opens a gap of `h` points above each line in `gaps` (line, h): that row and every row after it move
/// down. Rows are moved, not made taller, because egui centres the text of a custom line height.
fn open_gaps(galley: &mut egui::Galley, gaps: &[(usize, f32)]) {
    let mut shift = 0.0;
    for (i, row) in galley.rows.iter_mut().enumerate() {
        if let Some(&(_, h)) = gaps.iter().find(|g| g.0 == i) {
            shift += h;
        }
        row.pos.y += shift;
    }
    galley.rect.max.y += shift;
    galley.mesh_bounds.max.y += shift;
}

/// A person glyph (head and shoulders) centred on `c`, drawn with strokes.
fn person_icon(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    let stroke = Stroke::new(1.2, color);
    p.circle_stroke(c - egui::vec2(0.0, 2.6), 2.4, stroke);
    let shoulders = (0..=8)
        .map(|i| {
            let a = std::f32::consts::PI * i as f32 / 8.0;
            pos2(c.x - 4.2 * a.cos(), c.y + 4.2 - 3.4 * a.sin())
        })
        .collect();
    p.add(Shape::line(shoulders, stroke));
}

/// The lines of the declaration on line `d`: down to the last non-blank line still indented past it,
/// its braces included, as a half-open range.
// ponytail: by indentation, not by the parser; a declaration split over several lines at its own indent ends early
fn decl_span(lines: &[&str], d: usize) -> (usize, usize) {
    let base = indent_of(lines[d]).unwrap_or(0);
    let mut end = d + 1;
    for (l, text) in lines.iter().enumerate().skip(d + 1) {
        match indent_of(text) {
            None => {}
            Some(c) if c > base || (c == base && text.trim_start().starts_with(['{', '}'])) => end = l + 1,
            Some(_) => break,
        }
    }
    (d, end)
}

/// Who wrote most of the lines in `span`: "nome", or "nome +N" when N other people wrote the rest.
fn decl_author(blame: &[Option<String>], (from, to): (usize, usize)) -> Option<String> {
    let mut count = std::collections::BTreeMap::new();
    for name in blame.get(from..to.min(blame.len())).unwrap_or_default().iter().flatten() {
        *count.entry(name.as_str()).or_insert(0usize) += 1;
    }
    let (top, _) = count.iter().max_by_key(|(_, n)| **n)?;
    let others = count.len() - 1;
    Some(if others == 0 { top.to_string() } else { format!("{top} +{others}") })
}

/// Columns of leading whitespace (a tab is 4), None for a blank line.
fn indent_of(line: &str) -> Option<usize> {
    if line.trim().is_empty() {
        return None;
    }
    Some(line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum())
}

/// The file's indent step: its most common rise between two non-blank lines, 4 when there is none.
// ponytail: one step for the whole file; a file that mixes steps gets its most common one
fn indent_unit(cols: &[Option<usize>]) -> usize {
    let mut rises = [0usize; 9];
    let mut prev = 0;
    for &c in cols.iter().flatten() {
        if c > prev && c - prev <= 8 {
            rises[c - prev] += 1;
        }
        prev = c;
    }
    (1..9).max_by_key(|&d| rises[d]).filter(|&d| rises[d] > 0).unwrap_or(4)
}

/// Whether the guide at column `col` is drawn on line `l`: on the lines inside its block and on the
/// opening and closing lines; a blank line carries it only when the block goes on on both sides.
fn guide_on(cols: &[Option<usize>], l: usize, col: usize) -> bool {
    let prev = cols[..l].iter().rev().find_map(|c| *c).unwrap_or(0);
    let next = cols[l + 1..].iter().find_map(|c| *c).unwrap_or(0);
    match cols[l] {
        Some(c) => col < c || (col == c && (prev > col || next > col)),
        None => col < prev.min(next),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_open_a_row_above_their_declaration() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {}); // sets the fonts up
        out.textures_delta.clear();
        let font = egui::FontId::monospace(13.0);
        let row_h = ctx.fonts_mut(|f| f.row_height(&font)).round();
        let job = egui::text::LayoutJob::simple("a\nb\nc\nd".into(), font, Color32::WHITE, f32::INFINITY);
        let mut galley = ctx.fonts_mut(|f| f.layout_job(job));
        let before: Vec<f32> = galley.rows.iter().map(|r| r.pos.y).collect();
        let height = galley.rows[0].rect().height();
        // two declarations: lines 1 and 3 get a row above them
        open_gaps(std::sync::Arc::make_mut(&mut galley), &[(1, row_h), (3, row_h)]);
        assert_eq!(galley.rows[0].pos.y, before[0]);
        assert_eq!(galley.rows[1].pos.y, before[1] + row_h);
        assert_eq!(galley.rows[2].pos.y, before[2] + row_h);
        assert_eq!(galley.rows[3].pos.y, before[3] + 2.0 * row_h);
        assert_eq!(galley.rows[0].rect().height(), height); // the row above keeps its height
    }

    #[test]
    fn author_of_a_declaration() {
        let lines = ["class A {", "  f() {", "    x", "  }", "", "}", "y"];
        assert_eq!(decl_span(&lines, 0), (0, 6)); // through its closing brace, not `y`
        assert_eq!(decl_span(&lines, 1), (1, 4)); // a method ends at its own brace
        let blame = [Some("ana".to_string()), Some("edsuuu".into()), Some("edsuuu".into()), None];
        assert_eq!(decl_author(&blame, (0, 4)).as_deref(), Some("edsuuu +1"));
        assert_eq!(decl_author(&blame, (3, 4)), None); // only uncommitted lines
    }

    #[test]
    fn guides_span_the_block_and_stop_before_a_blank_tail() {
        let cols: Vec<_> = "fn f() {\n    if a {\n        x();\n    }\n\n}".split('\n').map(indent_of).collect();
        let on = |l, col| guide_on(&cols, l, col);
        assert_eq!(indent_unit(&cols), 4);
        assert!(on(1, 4) && on(2, 4) && on(3, 4)); // the `if` block: opening, body, closing
        assert!(!on(0, 4) && !on(5, 4));
        assert!(!on(4, 0) && !on(4, 4)); // the blank line before `}` ends the guides
    }
}
