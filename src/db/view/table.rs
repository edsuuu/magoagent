use super::*;

impl Sheet {
    /// PhpStorm's data editor bar: icons only (reload | add, remove, revert, preview, submit),
    /// then WHERE and ORDER BY side by side.
    pub(super) fn table_toolbar(&mut self, ui: &mut Ui) {
        let editable = !self.session.readonly && !matches!(self.meta.as_ref().unwrap().identity(), Identity::ReadOnly);
        let idle = self.running.is_none();
        let mut send = idle && editable && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        let mut submit_now = false;
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            if look::tool(ui, look::Glyph::Refresh, "Recarregar", idle && !self.pending()) {
                self.reload(ui.ctx());
            }
            ui.separator();
            if look::tool(ui, look::Glyph::Add, "Adicionar linha", idle && editable && self.edit.is_none()) {
                let n = self.meta.as_ref().unwrap().columns.len();
                self.rows.push(DataRow { original: None, values: vec![Cell::Null; n], rowid: Cell::Null, deleted: false });
                self.selected.clear();
                self.selected.insert(self.rows.len() - 1);
                self.cell = None;
                self.reveal = Some(self.rows.len() - 1);
            }
            if look::tool(ui, look::Glyph::Remove, "Apagar as linhas selecionadas", idle && editable && !self.selected.is_empty()) {
                self.edit = None;
                for &i in &self.selected {
                    self.rows[i].deleted = true;
                }
            }
            if look::tool(ui, look::Glyph::Revert, "Reverter as pendências", idle && self.pending()) {
                self.revert();
            }
            if look::tool(ui, look::Glyph::Preview, "Ver o SQL pendente", idle && !matches!(self.session.dialect, Dialect::Mongo | Dialect::Redis)) {
                self.preview = !self.preview;
            }
            submit_now = look::tool(ui, look::Glyph::Submit, &format!("Enviar ({})", chrome::keys("Ctrl+Enter")), idle && editable && self.pending());
            ui.separator();
            self.transaction_bar(ui);
            if let Some(ttl) = self.ttl {
                ui.label(if ttl < 0 { "TTL: sem expiração".into() } else { format!("TTL: {ttl} s") });
            }
            let can_null = self.cell.is_some_and(|(r, c)| !self.rows[r].deleted && !self.meta.as_ref().unwrap().columns[c].generated);
            if ui
                .add_enabled(idle && editable && can_null, egui::Button::new(egui::RichText::new("NULL").font(t::mono(11.5))).frame(false))
                .on_hover_text("Pôr NULL na célula")
                .clicked()
            {
                let (r, c) = self.cell.unwrap();
                self.edit = None;
                self.rows[r].values[c] = Cell::Null;
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                let note = if !idle {
                    "Executando…"
                } else if !editable {
                    if self.session.readonly {
                        "somente leitura: conexão restrita"
                    } else if self.session.dialect == Dialect::Redis {
                        "somente leitura: tipo de chave"
                    } else if self.meta.as_ref().unwrap().view {
                        "somente leitura: view"
                    } else if self.session.dialect != Dialect::Sqlite {
                        "somente leitura: tabela sem PRIMARY KEY"
                    } else {
                        "somente leitura: sem rowid nem chave primária segura"
                    }
                } else {
                    match &self.message {
                        Some((m, false)) => m.as_str(),
                        _ => "",
                    }
                };
                ui.label(egui::RichText::new(note).size(12.0).color(t::MUTED));
            });
        });
        send |= submit_now;
        if send && self.pending() && self.finish_edit() {
            let (table, rows) = (self.meta.clone().unwrap(), self.rows.clone());
            self.start(ui.ctx(), Op::Submit(table, rows));
        }
        look::rule(ui);
        // WHERE | ORDER BY, each half a box with no frame, its keyword as the hint
        let mut apply = false;
        ui.add_enabled_ui(idle && !self.pending() && self.session.dialect != Dialect::Redis, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(28.0);
                let half = (ui.available_width() - 24.0) / 2.0;
                let hints = if self.session.dialect == Dialect::Mongo { ("Filtro JSON {}", "Ordenação JSON {\"_id\": 1}") } else { ("WHERE", "ORDER BY") };
                for (k, (hint, value)) in [(hints.0, &mut self.filter), (hints.1, &mut self.order)].into_iter().enumerate() {
                    if k == 1 {
                        ui.separator();
                    }
                    ui.add_space(6.0);
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    look::funnel_or_sort(ui.painter(), r.center(), k == 0);
                    let edit = TextEdit::singleline(value)
                        .frame(Frame::NONE)
                        .font(t::mono(12.5))
                        .hint_text(egui::RichText::new(hint).font(t::mono(12.5)).color(t::SECONDARY))
                        .desired_width(half - 30.0);
                    let r = ui.add(edit);
                    apply |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                }
            });
        });
        look::rule(ui);
        if apply {
            self.applied = (self.filter.clone(), self.order.clone());
            self.offset = 0;
            self.reload(ui.ctx());
        }
        if self.preview {
            let table = self.meta.as_ref().unwrap();
            let text = table.dialect.dml(table, &self.rows).unwrap_or_else(|e| e);
            Frame::NONE.fill(t::INLINE).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical().id_salt("preview").max_height(160.0).show(ui, |ui| {
                    ui.label(egui::RichText::new(if text.is_empty() { "Sem alterações pendentes." } else { &text }).font(t::mono(12.0)).color(t::TEXT));
                });
            });
            look::rule(ui);
        }
    }

    pub(super) fn table_grid(&mut self, ui: &mut Ui) {
        let can_sort = !self.pending() && self.running.is_none() && !matches!(self.session.dialect, Dialect::Mongo | Dialect::Redis);
        let table = self.meta.as_ref().unwrap();
        let editable = self.running.is_none() && !self.session.readonly && !matches!(table.identity(), Identity::ReadOnly);
        let (mut finish, mut cancel) = (false, false);
        let columns: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        let arrows: Vec<&str> = table
            .columns
            .iter()
            .map(|c| {
                let name = self.ident(&c.name);
                if self.applied.1 == format!("{name} ASC") {
                    "↑"
                } else if self.applied.1 == format!("{name} DESC") {
                    "↓"
                } else {
                    ""
                }
            })
            .collect();
        let rows: Vec<&[Cell]> = self.rows.iter().map(|r| r.values.as_slice()).collect();
        // PhpStorm's pending colors: changed cell, new row, row to delete
        let fill = |n: usize, c: usize| {
            let row = &self.rows[n];
            if row.deleted {
                Color32::from_rgb(0x48, 0x4A, 0x4A)
            } else if row.original.is_none() {
                Color32::from_rgb(0x29, 0x44, 0x36)
            } else if row.original.as_ref().is_some_and(|o| o[c] != row.values[c]) {
                Color32::from_rgb(0x38, 0x55, 0x70)
            } else {
                Color32::TRANSPARENT
            }
        };
        let id = self.id.with("cell");
        let editing = self.edit.as_ref().map(|e| (e.row, e.column));
        let mut edit_cell = |ui: &mut Ui, rect: Rect| {
            let Some(edit) = self.edit.as_mut() else { return };
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                cancel = true;
            }
            let r = ui.put(rect, TextEdit::singleline(&mut edit.text).id(id).font(t::mono(12.5)).margin(Margin::symmetric(5, 3)).background_color(t::ISLAND));
            if edit.focus {
                r.request_focus();
                edit.focus = false;
            }
            // Enter or a click elsewhere keeps the text, as in PhpStorm; only Escape drops it
            finish |= r.lost_focus();
        };
        let keys: Vec<bool> = table.columns.iter().map(|c| c.pk > 0).collect();
        // the last rows scroll up past the paging pill and the error strip (see paging)
        let pad = if matches!(self.message, Some((_, true))) { 80.0 } else { 50.0 };
        let clicks = grid(
            ui,
            self.id.with("grid"),
            &columns,
            &arrows,
            &keys,
            &rows,
            self.offset,
            self.reveal.take(),
            &self.selected,
            self.cell,
            &fill,
            editing.map(|at| (at, &mut edit_cell as &mut dyn FnMut(&mut Ui, Rect))),
            pad,
        );
        let mut open = clicks.double.filter(|&(n, c)| editable && !self.rows[n].deleted && !table.columns[c].generated);
        let sort_by = clicks.header.filter(|_| can_sort).map(|c| self.ident(&table.columns[c].name));
        // the edit ends before another cell opens; a bad BLOB keeps its box
        if cancel {
            self.edit = None;
        } else if finish && !self.finish_edit() {
            open = None;
        }
        if let Some((n, multi)) = clicks.gutter {
            self.cell = None;
            if !multi {
                self.selected.clear();
            }
            if !self.selected.insert(n) {
                self.selected.remove(&n);
            }
        }
        if let Some(at) = clicks.cell {
            self.cell = Some(at);
            self.selected.clear();
            self.selected.insert(at.0);
        }
        if let Some((n, c)) = open {
            let value = &self.rows[n].values[c];
            self.edit = Some(Edit { row: n, column: c, text: if *value == Cell::Null { String::new() } else { value.text().into() }, focus: true });
        }
        if let Some(name) = sort_by {
            let (asc, desc) = (format!("{name} ASC"), format!("{name} DESC"));
            self.order = if self.applied.1 == asc {
                desc
            } else if self.applied.1 == desc {
                String::new()
            } else {
                asc
            };
            self.applied.1 = self.order.clone();
            self.offset = 0;
            self.reload(ui.ctx());
        }
    }
}
