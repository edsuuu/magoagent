use super::*;

impl Sheet {
    /// The SQL console the way PhpStorm lays it out: a bar (run, history, transaction), the
    /// editor with line numbers, and the result in a panel under it that can be resized.
    pub(super) fn console(&mut self, ui: &mut Ui) {
        let mut run = self.running.is_none() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            run |=
                look::tool(ui, look::Glyph::Run, &format!("Executar a seleção ou a instrução atual ({})", chrome::keys("Ctrl+Enter")), self.running.is_none());
            if look::tool(ui, look::Glyph::History, "Histórico das consultas", !self.history.is_empty()) {
                self.show_history = !self.show_history;
            }
            ui.separator();
            self.transaction_bar(ui);
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(match self.session.dialect {
                        Dialect::Sqlite => "main",
                        Dialect::Mysql => "MySQL",
                        Dialect::Postgres => "PostgreSQL",
                        Dialect::Mongo => "MongoDB",
                        Dialect::Redis => "Redis",
                    })
                    .size(12.0)
                    .color(t::MUTED),
                )
                .on_hover_text("Schema");
            });
        });
        look::rule(ui);
        if self.show_history {
            Frame::NONE.fill(t::INLINE).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical().id_salt("history").max_height(140.0).show(ui, |ui| {
                    for sql in self.history.iter().rev() {
                        if ui.selectable_label(false, egui::RichText::new(sql).font(t::mono(12.0))).clicked() {
                            self.sql = sql.clone();
                            self.show_history = false;
                        }
                    }
                });
            });
        }
        // the editor on top, the result under it; the line between them drags
        let area = ui.available_rect_before_wrap();
        ui.allocate_rect(area, Sense::hover());
        self.split = self.split.clamp(80.0, (area.height() - 80.0).max(80.0));
        let cut = area.bottom() - self.split;
        let handle = Rect::from_min_max(pos2(area.left(), cut - 3.0), pos2(area.right(), cut + 3.0));
        let drag = ui.interact(handle, self.id.with("split"), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeVertical);
        self.split -= drag.drag_delta().y;
        ui.painter().hline(area.x_range(), cut, Stroke::new(1.0, if drag.hovered() || drag.dragged() { t::ACCENT } else { t::POPUP_BORDER }));
        let result = Rect::from_min_max(pos2(area.left(), cut + 1.0), area.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(result), |ui| {
            let line = match (&self.running, &self.message, &self.result) {
                (Some(_), _, _) => ("Executando…".to_string(), t::MUTED),
                (_, Some((m, error)), _) => (m.clone(), if *error { t::ERROR } else { t::MUTED }),
                (_, _, Some(g)) => (rows_label(g.rows.len()), t::MUTED),
                _ => (String::new(), t::MUTED),
            };
            ui.horizontal(|ui| {
                ui.set_height(26.0);
                ui.add_space(10.0);
                ui.label(egui::RichText::new(line.0).size(12.0).color(line.1));
            });
            if let Some(grid) = &self.result {
                result_grid(ui, grid, self.id.with("grid"));
            }
        });
        let editor = Rect::from_min_max(area.min, pos2(area.right(), cut - 1.0));
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(editor));
        let ui = &mut ui;
        let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _width: f32| {
            let mut job = highlight::layout(&highlight::SQL, buf.as_str(), t::mono(13.0), 18.0);
            job.wrap.max_width = f32::INFINITY;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let gutter = 40.0;
        let mut output = None;
        let sideways = egui::ScrollArea::both()
            .id_salt("editor")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.add_space(gutter);
                    let edit = TextEdit::multiline(&mut self.sql)
                        .id(self.id)
                        .font(t::mono(13.0))
                        .code_editor()
                        .frame(Frame::NONE)
                        .desired_rows(12)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter)
                        .hint_text(format!(
                            "{} · {} executa a seleção ou a instrução atual",
                            match self.session.dialect {
                                Dialect::Mongo => "MongoDB",
                                Dialect::Redis => "Redis",
                                _ => "SQL",
                            },
                            chrome::keys("Ctrl+Enter")
                        ));
                    output = Some(edit.show(ui));
                });
            })
            .state
            .clamped_offset()
            .x;
        let Some(output) = output else { return };
        if std::mem::take(&mut self.focus) {
            output.response.request_focus();
        }
        // line numbers in the gutter, one per line of SQL (the editor doesn't wrap); it stays put when
        // the text scrolls sideways, over the text
        let top = output.galley_pos.y;
        let x = output.response.rect.left() + sideways - 10.0;
        if sideways > 0.0 {
            ui.painter().rect_filled(Rect::from_min_max(editor.min, pos2(x + 10.0, editor.bottom())), 0, t::ISLAND);
        }
        let clip = ui.clip_rect();
        let caret_line = output.cursor_range.map(|r| self.sql[..byte_at(&self.sql, r.primary.index.0)].matches('\n').count());
        for (n, row) in output.galley.rows.iter().enumerate() {
            let y = top + row.rect().center().y;
            if y < clip.top() || y > clip.bottom() {
                continue;
            }
            let color = if caret_line == Some(n) { t::LINE_NUMBER_CARET } else { t::LINE_NUMBER };
            ui.painter().text(pos2(x, y), Align2::RIGHT_CENTER, (n + 1).to_string(), t::mono(12.5), color);
        }
        if run {
            let range = output
                .cursor_range
                .map(|r| {
                    let [a, b] = r.sorted_cursors();
                    (a.index.0, b.index.0)
                })
                .unwrap_or((0, 0));
            let sql = instruction_mode(&self.sql, range, self.session.dialect, self.session.no_backslash());
            if !sql.is_empty() {
                self.history.push(sql.clone());
                self.start(ui.ctx(), Op::Console(sql));
            }
        }
    }
}
