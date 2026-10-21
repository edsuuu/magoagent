//! Table mutations and the SQL console share a tab, but never share write permissions.

use super::*;
use std::sync::mpsc::channel;

enum Job { Page(Result<Page, String>), Submit(Result<(), String>), Console(Result<Grid, String>, u128) }
struct Edit { row: usize, column: usize, text: String, focus: bool }

pub(in super::super) struct Sheet {
    id: Id,
    pub table: Option<String>,
    meta: Option<Table>,
    sql: String,
    result: Option<Grid>,
    rows: Vec<DataRow>,
    count: usize,
    offset: usize,
    filter: String,
    order: String,
    applied: (String, String),
    selected: std::collections::HashSet<usize>,
    cell: Option<(usize, usize)>,
    edit: Option<Edit>,
    running: Option<Receiver<Job>>,
    message: Option<(String, bool)>,
    history: Vec<String>,
    show_history: bool,
    preview: bool,
}

impl Sheet {
    pub(super) fn new(id: Id, meta: Option<Table>) -> Self {
        Self { id, table: meta.as_ref().map(|t| t.name.clone()), meta, sql: String::new(), result: None, rows: vec![], count: 0, offset: 0,
            filter: String::new(), order: String::new(), applied: Default::default(), selected: Default::default(), cell: None, edit: None,
            running: None, message: None, history: vec![], show_history: false, preview: false }
    }

    pub(in super::super) fn pending(&self) -> bool { self.edit.is_some() || self.rows.iter().any(DataRow::pending) }

    fn start(&mut self, ctx: &egui::Context, job: impl FnOnce() -> Job + Send + 'static) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        self.running = Some(rx);
        self.message = None;
        std::thread::spawn(move || { let _ = tx.send(job()); ctx.request_repaint(); });
    }

    pub(super) fn reload(&mut self, file: &Path, ctx: &egui::Context) {
        if self.pending() || self.running.is_some() { return; }
        let Some(table) = self.meta.clone() else { return };
        let (file, filter, order, offset) = (file.to_path_buf(), self.applied.0.clone(), self.applied.1.clone(), self.offset);
        self.rows.clear();
        self.selected.clear();
        self.cell = None;
        self.start(ctx, move || Job::Page(load_page(&file, &table, &filter, &order, offset)));
    }

    fn poll(&mut self, file: &Path, ctx: &egui::Context) {
        let job = self.running.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(job) => Some(job),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Job::Submit(Err("A tarefa do banco foi interrompida.".into()))),
            Err(_) => None,
        });
        let Some(job) = job else { return };
        self.running = None;
        match job {
            Job::Page(Ok(page)) => {
                self.count = page.count;
                self.rows = page.rows;
                if self.offset > 0 && self.rows.is_empty() {
                    self.offset = self.count.saturating_sub(1) / PAGE * PAGE;
                    self.reload(file, ctx);
                }
            }
            Job::Page(Err(e)) | Job::Submit(Err(e)) => self.message = Some((e, true)),
            Job::Submit(Ok(())) => { self.rows.clear(); self.edit = None; self.reload(file, ctx); }
            Job::Console(result, ms) => match result {
                Ok(grid) => { self.message = Some((format!("{} linhas em {ms} ms", grid.rows.len()), false)); self.result = Some(grid); }
                Err(e) => { self.result = None; self.message = Some((e, true)); }
            },
        }
    }

    pub(super) fn ui(&mut self, ui: &mut Ui, file: &Path) {
        self.poll(file, ui.ctx());
        ui.push_id(self.id, |ui| {
            Frame::NONE.inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
                if self.meta.is_some() { self.table_toolbar(ui, file); }
                else { self.console(ui, file); }
                if self.running.is_some() { ui.colored_label(t::MUTED, "Executando…"); }
                if let Some((message, error)) = &self.message { ui.colored_label(if *error { t::ERROR } else { t::MUTED }, message); }
            });
            if self.meta.is_some() { self.table_grid(ui, file); }
            else if let Some(grid) = &self.result { result_grid(ui, grid, self.id.with("result")); }
        });
    }

    fn finish_edit(&mut self) -> bool {
        if let Some(edit) = self.edit.take() {
            let blob = matches!(self.rows[edit.row].values[edit.column], Cell::Blob(_));
            if blob && (edit.text.len() % 2 != 0 || !edit.text.bytes().all(|b| b.is_ascii_hexdigit())) {
                self.edit = Some(edit);
                self.message = Some(("BLOB: informe pares de dígitos hexadecimais.".into(), true));
                return false;
            }
            self.rows[edit.row].values[edit.column] = if blob { Cell::Blob(edit.text.to_ascii_uppercase()) } else { Cell::Text(edit.text) };
        }
        true
    }

    fn revert(&mut self) {
        self.edit = None;
        self.rows.retain(|r| r.original.is_some());
        for r in &mut self.rows { r.values = r.original.clone().unwrap(); r.deleted = false; }
        self.selected.clear();
        self.cell = None;
        self.message = None;
    }

    fn table_toolbar(&mut self, ui: &mut Ui, file: &Path) {
        let editable = !matches!(self.meta.as_ref().unwrap().identity(), Identity::ReadOnly);
        let idle = self.running.is_none();
        let mut send = idle && editable && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        egui::ScrollArea::horizontal().id_salt("toolbar").show(ui, |ui| ui.horizontal(|ui| {
            let clean = idle && !self.pending();
            let last = self.count.saturating_sub(1) / PAGE * PAGE;
            for (label, offset, enabled) in [("|<", 0, self.offset > 0), ("<", self.offset.saturating_sub(PAGE), self.offset > 0),
                (">", self.offset + PAGE, self.offset < last), (">|", last, self.offset < last)] {
                if ui.add_enabled(clean && enabled, egui::Button::new(label)).clicked() { self.offset = offset; self.reload(file, ui.ctx()); }
            }
            ui.label(if self.count == 0 { "0–0 de 0".into() } else { format!("{}–{} de {}", self.offset + 1, (self.offset + PAGE).min(self.count), self.count) });
            if ui.add_enabled(clean, egui::Button::new("↻")).on_hover_text("Recarregar").clicked() { self.reload(file, ui.ctx()); }
            if ui.add_enabled(idle && editable && self.edit.is_none(), egui::Button::new("+ linha")).clicked() {
                let n = self.meta.as_ref().unwrap().columns.len();
                self.rows.push(DataRow { original: None, values: vec![Cell::Null; n], rowid: Cell::Null, deleted: false });
                self.selected.clear(); self.selected.insert(self.rows.len() - 1);
                self.cell = None;
            }
            if ui.add_enabled(idle && editable && !self.selected.is_empty(), egui::Button::new("− linhas")).clicked() {
                self.edit = None;
                for &i in &self.selected { self.rows[i].deleted = true; }
            }
            send |= ui.add_enabled(idle && editable && self.pending(), egui::Button::new("Enviar")).on_hover_text(format!("Enviar alterações ({})", chrome::keys("Ctrl+Enter"))).clicked();
            if ui.add_enabled(idle && self.pending(), egui::Button::new("Reverter")).on_hover_text("Descartar todas as pendências").clicked() { self.revert(); }
            if ui.add_enabled(idle, egui::Button::new("Ver SQL")).clicked() { self.preview = !self.preview; }
            let can_null = self.cell.is_some_and(|(r, c)| !self.rows[r].deleted && !self.meta.as_ref().unwrap().columns[c].generated);
            if ui.add_enabled(idle && editable && can_null, egui::Button::new("NULL")).clicked() {
                let (r, c) = self.cell.unwrap(); self.edit = None; self.rows[r].values[c] = Cell::Null;
            }
        }));
        if send && self.pending() {
            if !self.finish_edit() { return; }
            let (file, table, rows) = (file.to_path_buf(), self.meta.clone().unwrap(), self.rows.clone());
            self.start(ui.ctx(), move || Job::Submit(submit(&file, &table, &rows)));
        }
        if !editable { ui.colored_label(t::WARNING, "Tabela sem rowid ou chave primária segura; somente leitura."); }
        if self.pending() { ui.colored_label(t::MUTED, "Envie ou reverta as pendências antes de mudar página, filtro ou ordem."); }
        let mut apply = false;
        ui.add_enabled_ui(idle && !self.pending(), |ui| {
            for (label, value) in [("WHERE", &mut self.filter), ("ORDER BY", &mut self.order)] {
                ui.horizontal(|ui| {
                    ui.add_sized([76.0, 24.0], egui::Label::new(egui::RichText::new(label).font(t::mono(12.0)).color(t::MUTED)));
                    let r = ui.add(TextEdit::singleline(value).desired_width(f32::INFINITY).hint_text("Expressão sem a palavra-chave; Enter aplica"));
                    apply |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                });
            }
        });
        if apply { self.applied = (self.filter.clone(), self.order.clone()); self.offset = 0; self.reload(file, ui.ctx()); }
        if self.preview {
            let text = dml(self.meta.as_ref().unwrap(), &self.rows).unwrap_or_else(|e| e);
            egui::ScrollArea::vertical().id_salt("preview").max_height(160.0).show(ui, |ui| { ui.label(egui::RichText::new(if text.is_empty() { "Sem alterações pendentes." } else { &text }).font(t::mono(12.0))); });
        }
    }

    fn table_grid(&mut self, ui: &mut Ui, file: &Path) {
        let can_sort = !self.pending() && self.running.is_none();
        let table = self.meta.as_ref().unwrap();
        let editable = self.running.is_none() && !matches!(table.identity(), Identity::ReadOnly);
        let (mut sort, mut finish, mut cancel) = (None, false, false);
        egui::ScrollArea::both().id_salt(self.id.with("grid")).auto_shrink(false).show(ui, |ui| {
            egui::Grid::new(self.id.with("rows")).spacing([1.0, 1.0]).show(ui, |ui| {
                ui.label("#");
                for column in &table.columns {
                    let asc = format!("{} ASC", ident(&column.name));
                    let desc = format!("{} DESC", ident(&column.name));
                    let arrow = if self.applied.1 == asc { " ↑" } else if self.applied.1 == desc { " ↓" } else { "" };
                    if ui.add_enabled(can_sort, egui::Button::new(format!("{}{arrow}", column.name)).min_size(vec2(130.0, 24.0))).clicked() {
                        sort = Some(if self.applied.1 == asc { desc } else if self.applied.1 == desc { String::new() } else { asc });
                    }
                }
                ui.end_row();
                for (n, row) in self.rows.iter_mut().enumerate() {
                    let fill = if row.deleted { Color32::from_rgb(0x48, 0x4A, 0x4A) } else if row.original.is_none() { Color32::from_rgb(0x29, 0x44, 0x36) } else { Color32::TRANSPARENT };
                    Frame::NONE.fill(fill).show(ui, |ui| {
                        if ui.selectable_label(self.selected.contains(&n), (self.offset + n + 1).to_string()).on_hover_text(chrome::keys("Ctrl+clique seleciona várias linhas")).clicked() {
                            self.cell = None;
                            if !ui.input(|i| i.modifiers.command) { self.selected.clear(); }
                            if !self.selected.insert(n) { self.selected.remove(&n); }
                        }
                    });
                    for (c, value) in row.values.iter_mut().enumerate() {
                        let changed = row.original.as_ref().is_some_and(|o| o[c] != *value);
                        let fill = if changed && !row.deleted { Color32::from_rgb(0x38, 0x55, 0x70) } else { fill };
                        Frame::NONE.fill(fill).inner_margin(Margin::symmetric(4, 1)).show(ui, |ui| {
                            if let Some(edit) = self.edit.as_mut().filter(|e| e.row == n && e.column == c) {
                                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) { cancel = true; }
                                let r = ui.add(TextEdit::singleline(&mut edit.text).id(self.id.with("cell")).font(t::mono(12.5)).desired_width(130.0));
                                if edit.focus { r.request_focus(); edit.focus = false; }
                                finish |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            } else {
                                let short: String = value.text().chars().take(60).collect();
                                let text = egui::RichText::new(short).font(t::mono(12.5)).color(if *value == Cell::Null { t::MUTED } else { t::TEXT });
                                let r = ui.add_sized([130.0, 24.0], egui::Button::new(text).truncate().selected(self.cell == Some((n, c)))).on_hover_text(value.text());
                                if r.clicked() { self.cell = Some((n, c)); self.selected.clear(); self.selected.insert(n); }
                                if r.double_clicked() && editable && !row.deleted && !table.columns[c].generated {
                                    self.edit = Some(Edit { row: n, column: c, text: if *value == Cell::Null { String::new() } else { value.text().into() }, focus: true });
                                }
                            }
                        });
                    }
                    ui.end_row();
                }
            });
        });
        if cancel { self.edit = None; } else if finish { self.finish_edit(); }
        if let Some(order) = sort { self.order = order; self.applied.1 = self.order.clone(); self.offset = 0; self.reload(file, ui.ctx()); }
    }

    fn console(&mut self, ui: &mut Ui, file: &Path) {
        let mut run = self.running.is_none() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, width: f32| {
            let mut job = highlight::layout(&highlight::SQL, buf.as_str(), t::mono(13.0), 18.0, &[]);
            job.wrap.max_width = width;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let output = TextEdit::multiline(&mut self.sql).id(self.id).font(t::mono(13.0)).code_editor().desired_rows(6)
            .desired_width(f32::INFINITY).hint_text(format!("SQL · {} executa a seleção ou a instrução atual", chrome::keys("Ctrl+Enter")))
            .layouter(&mut layouter).show(ui);
        ui.horizontal(|ui| {
            run |= ui.add_enabled(self.running.is_none(), egui::Button::new("▶ Executar")).clicked();
            if ui.button("Histórico").clicked() { self.show_history = !self.show_history; }
        });
        if run {
            let range = output.cursor_range.map(|r| { let [a, b] = r.sorted_cursors(); (a.index.0, b.index.0) }).unwrap_or((0, 0));
            let sql = instruction(&self.sql, range);
            if !sql.is_empty() {
                self.history.push(sql.clone());
                let file = file.to_path_buf();
                self.start(ui.ctx(), move || { let start = Instant::now(); let result = execute(&file, &format!("PRAGMA foreign_keys=ON;\n{sql}")); Job::Console(result, start.elapsed().as_millis()) });
            }
        }
        if self.show_history {
            egui::ScrollArea::vertical().id_salt("history").max_height(140.0).show(ui, |ui| {
                for sql in self.history.iter().rev() {
                    if ui.selectable_label(false, sql).clicked() { self.sql = sql.clone(); self.show_history = false; }
                }
            });
        }
    }
}

fn result_grid(ui: &mut Ui, grid: &Grid, id: Id) {
    egui::ScrollArea::both().id_salt(id).auto_shrink(false).show_rows(ui, 24.0, grid.rows.len() + 1, |ui, range| {
        egui::Grid::new(id.with("rows")).striped(true).min_row_height(24.0).spacing([18.0, 0.0]).show(ui, |ui| {
            for index in range {
                if index == 0 {
                    ui.label("#");
                    for column in &grid.columns { ui.add_sized([130.0, 24.0], egui::Label::new(egui::RichText::new(column).font(t::mono(12.5)).strong()).truncate()); }
                } else {
                    ui.colored_label(t::LINE_NUMBER, index.to_string());
                    for value in &grid.rows[index - 1] {
                        let text: String = value.text().chars().take(60).collect();
                        ui.add_sized([130.0, 24.0], egui::Label::new(egui::RichText::new(text).font(t::mono(12.5)).color(if *value == Cell::Null { t::MUTED } else { t::TEXT })).truncate()).on_hover_text(value.text());
                    }
                }
                ui.end_row();
            }
        });
    });
}

/// Character selection; semicolons inside quoted values and comments don't end a statement.
pub(super) fn instruction(sql: &str, selection: (usize, usize)) -> String {
    let (a, b) = (byte_at(sql, selection.0), byte_at(sql, selection.1));
    if a != b { return sql[a..b].trim().into(); }
    let bytes = sql.as_bytes();
    let (mut start, mut i, mut quote) = (0, 0, 0);
    let mut previous = "";
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if c == quote {
                if bytes.get(i + 1) == Some(&quote) && quote != b']' { i += 1; } else { quote = 0; }
            }
        } else if matches!(c, b'\'' | b'"' | b'`' | b'[') { quote = if c == b'[' { b']' } else { c }; }
        else if bytes.get(i..i + 2) == Some(b"--") { while i < bytes.len() && bytes[i] != b'\n' { i += 1; } }
        else if bytes.get(i..i + 2) == Some(b"/*") {
            i += 2;
            while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" { i += 1; }
            i += 1;
        } else if c == b';' {
            if a <= i + 1 { return sql[start..=i].trim().into(); }
            previous = &sql[start..=i];
            start = i + 1;
        }
        i += 1;
    }
    let tail = sql[start..].trim();
    if tail.is_empty() { previous.trim().into() } else { tail.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_enter_escape_revert_and_pending_navigation() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        for (name, base) in [("ide", egui::FontFamily::Proportional), ("ide-mono", egui::FontFamily::Monospace)] {
            fonts.families.insert(egui::FontFamily::Name(name.into()), fonts.families[&base].clone());
        }
        ctx.set_fonts(fonts);
        let mut sheet = Sheet::new(Id::unique("cell-test"), Some(Table {
            name: "t".into(), columns: vec![Column { name: "value".into(), kind: "TEXT".into(), ..Default::default() }], ..Default::default()
        }));
        sheet.rows.push(DataRow { original: Some(vec![Cell::Null]), values: vec![Cell::Null], rowid: Cell::Text("1".into()), deleted: false });
        sheet.edit = Some(Edit { row: 0, column: 0, text: String::new(), focus: true });
        let frame = |sheet: &mut Sheet, key: Option<Key>| {
            let mut input = egui::RawInput::default();
            if let Some(key) = key { input.events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }); }
            ctx.run_ui(input, |ui| {
                t::style(ui.style_mut()); sheet.table_grid(ui, Path::new("unused.sqlite"));
            }).textures_delta.clear();
        };
        frame(&mut sheet, None);
        frame(&mut sheet, Some(Key::Enter));
        assert!(sheet.edit.is_none());
        assert_eq!(sheet.rows[0].values[0], Cell::Text(String::new()));
        assert!(sheet.pending());
        sheet.reload(Path::new("unused.sqlite"), &ctx);
        assert!(sheet.running.is_none() && sheet.rows.len() == 1, "pending values block navigation");
        sheet.edit = Some(Edit { row: 0, column: 0, text: "cancelar".into(), focus: true });
        frame(&mut sheet, None);
        frame(&mut sheet, Some(Key::Escape));
        assert!(sheet.edit.is_none());
        assert_eq!(sheet.rows[0].values[0], Cell::Text(String::new()));
        sheet.rows[0].deleted = true;
        sheet.rows.push(DataRow { original: None, values: vec![Cell::Null], rowid: Cell::Null, deleted: false });
        sheet.revert();
        assert!(!sheet.pending() && sheet.rows.len() == 1);
        assert_eq!(sheet.rows[0].values, [Cell::Null]);
        sheet.rows[0].values[0] = Cell::Blob("00FF".into());
        sheet.edit = Some(Edit { row: 0, column: 0, text: "0FF".into(), focus: false });
        assert!(!sheet.finish_edit() && sheet.edit.is_some());
        assert_eq!(sheet.rows[0].values[0], Cell::Blob("00FF".into()));
        sheet.edit.as_mut().unwrap().text = "0Fa1".into();
        assert!(sheet.finish_edit());
        assert_eq!(sheet.rows[0].values[0], Cell::Blob("0FA1".into()));
    }
}
