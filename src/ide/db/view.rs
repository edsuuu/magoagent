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
    /// a new console takes the keyboard, ready to type into
    focus: bool,
    /// the console's result height (drag the line above it)
    split: f32,
    /// a row the grid should scroll to (a new one)
    reveal: Option<usize>,
}

impl Sheet {
    pub(super) fn new(id: Id, meta: Option<Table>) -> Self {
        Self { id, table: meta.as_ref().map(|t| t.name.clone()), meta, sql: String::new(), result: None, rows: vec![], count: 0, offset: 0,
            filter: String::new(), order: String::new(), applied: Default::default(), selected: Default::default(), cell: None, edit: None,
            running: None, message: None, history: vec![], show_history: false, preview: false, focus: true, split: 260.0, reveal: None }
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
                Ok(grid) => { self.message = Some((format!("{} em {ms} ms", rows_label(grid.rows.len())), false)); self.result = Some(grid); }
                Err(e) => { self.result = None; self.message = Some((e, true)); }
            },
        }
    }

    pub(super) fn ui(&mut self, ui: &mut Ui, file: &Path) {
        self.poll(file, ui.ctx());
        ui.push_id(self.id, |ui| {
            if self.meta.is_some() {
                self.table_toolbar(ui, file);
                let area = ui.available_rect_before_wrap();
                self.table_grid(ui, file);
                self.paging(ui, file, area);
            } else {
                self.console(ui, file);
            }
        });
    }

    /// PhpStorm's floating pill over the bottom of the grid: the rows shown and, past one page,
    /// the arrows. A failed query shows in a red strip under it, until closed.
    fn paging(&mut self, ui: &mut Ui, file: &Path, area: Rect) {
        let mut bottom = area.bottom() - 10.0;
        if let Some((message, true)) = &self.message {
            let strip = Rect::from_min_max(pos2(area.left(), area.bottom() - 30.0), area.max);
            ui.painter().rect_filled(strip, 0, Color32::from_rgb(0x5E, 0x34, 0x34));
            ui.painter().with_clip_rect(strip.shrink2(vec2(30.0, 0.0))).text(strip.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, message, t::ui(12.5), t::TEXT);
            let close = Rect::from_center_size(strip.right_center() - vec2(16.0, 0.0), vec2(20.0, 20.0));
            icons::paint(ui.painter(), icons::CLOSE_SMALL, close.shrink(2.0));
            if ui.interact(close, self.id.with("close_error"), Sense::click()).clicked() { self.message = None; }
            bottom = strip.top() - 10.0;
        }
        let pages = self.count > PAGE;
        let label = match self.count {
            0 => "0 linhas".to_string(),
            n if !pages => rows_label(n),
            n => format!("{}–{} de {n}", self.offset + 1, (self.offset + PAGE).min(n)),
        };
        let text = ui.painter().layout_no_wrap(label, t::ui(12.5), t::TEXT);
        let width = text.size().x + 24.0 + if pages { 4.0 * 26.0 + 4.0 } else { 0.0 };
        let pill = Rect::from_center_size(pos2(area.center().x, bottom - 15.0), vec2(width, 30.0));
        ui.painter().rect(pill, 8, t::POPUP, Stroke::new(1.0, t::POPUP_BORDER), egui::StrokeKind::Inside);
        let clean = self.running.is_none() && !self.pending();
        let last = self.count.saturating_sub(1) / PAGE * PAGE;
        let mut go = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(pill.shrink2(vec2(6.0, 2.0))).layout(egui::Layout::left_to_right(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let tip = if clean { "" } else { "Envie ou reverta as pendências antes de mudar de página" };
            if pages {
                if look::tool(ui, look::Glyph::First, "Primeira página", clean && self.offset > 0) { go = Some(0); }
                if look::tool(ui, look::Glyph::Previous, "Página anterior", clean && self.offset > 0) { go = Some(self.offset.saturating_sub(PAGE)); }
            }
            let (r, resp) = ui.allocate_exact_size(vec2(text.size().x + 12.0, 26.0), Sense::hover());
            ui.painter().galley(r.center() - text.size() / 2.0, text, t::TEXT);
            if !tip.is_empty() { resp.on_hover_text(tip); }
            if pages {
                if look::tool(ui, look::Glyph::Next, "Próxima página", clean && self.offset < last) { go = Some(self.offset + PAGE); }
                if look::tool(ui, look::Glyph::Last, "Última página", clean && self.offset < last) { go = Some(last); }
            }
        });
        if let Some(offset) = go { self.offset = offset; self.reload(file, ui.ctx()); }
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

    /// PhpStorm's data editor bar: icons only (reload | add, remove, revert, preview, submit),
    /// then WHERE and ORDER BY side by side.
    fn table_toolbar(&mut self, ui: &mut Ui, file: &Path) {
        let editable = !matches!(self.meta.as_ref().unwrap().identity(), Identity::ReadOnly);
        let idle = self.running.is_none();
        let mut send = idle && editable && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        let mut submit_now = false;
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            if look::tool(ui, look::Glyph::Refresh, "Recarregar", idle && !self.pending()) { self.reload(file, ui.ctx()); }
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
                for &i in &self.selected { self.rows[i].deleted = true; }
            }
            if look::tool(ui, look::Glyph::Revert, "Reverter as pendências", idle && self.pending()) { self.revert(); }
            if look::tool(ui, look::Glyph::Preview, "Ver o SQL pendente", idle) { self.preview = !self.preview; }
            submit_now = look::tool(ui, look::Glyph::Submit, &format!("Enviar ({})", chrome::keys("Ctrl+Enter")), idle && editable && self.pending());
            ui.separator();
            ui.label(egui::RichText::new("Tx: Auto").size(12.0).color(t::MUTED)).on_hover_text("Cada envio é uma transação, confirmada no fim");
            let can_null = self.cell.is_some_and(|(r, c)| !self.rows[r].deleted && !self.meta.as_ref().unwrap().columns[c].generated);
            if ui.add_enabled(idle && editable && can_null, egui::Button::new(egui::RichText::new("NULL").font(t::mono(11.5))).frame(false)).on_hover_text("Pôr NULL na célula").clicked() {
                let (r, c) = self.cell.unwrap();
                self.edit = None;
                self.rows[r].values[c] = Cell::Null;
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                let note = if !idle { "Executando…" } else if !editable { "somente leitura: sem rowid nem chave primária" } else { match &self.message { Some((m, false)) => m.as_str(), _ => "" } };
                ui.label(egui::RichText::new(note).size(12.0).color(t::MUTED));
            });
        });
        send |= submit_now;
        if send && self.pending() && self.finish_edit() {
            let (file, table, rows) = (file.to_path_buf(), self.meta.clone().unwrap(), self.rows.clone());
            self.start(ui.ctx(), move || Job::Submit(submit(&file, &table, &rows)));
        }
        look::rule(ui);
        // WHERE | ORDER BY, each half a box with no frame, its keyword as the hint
        let mut apply = false;
        ui.add_enabled_ui(idle && !self.pending(), |ui| {
            ui.horizontal(|ui| {
                ui.set_height(28.0);
                let half = (ui.available_width() - 24.0) / 2.0;
                for (k, (hint, value)) in [("WHERE", &mut self.filter), ("ORDER BY", &mut self.order)].into_iter().enumerate() {
                    if k == 1 { ui.separator(); }
                    ui.add_space(6.0);
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    look::funnel_or_sort(ui.painter(), r.center(), k == 0);
                    let edit = TextEdit::singleline(value).frame(Frame::NONE).font(t::mono(12.5)).hint_text(egui::RichText::new(hint).font(t::mono(12.5)).color(t::SECONDARY)).desired_width(half - 30.0);
                    let r = ui.add(edit);
                    apply |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                }
            });
        });
        look::rule(ui);
        if apply { self.applied = (self.filter.clone(), self.order.clone()); self.offset = 0; self.reload(file, ui.ctx()); }
        if self.preview {
            let text = dml(self.meta.as_ref().unwrap(), &self.rows).unwrap_or_else(|e| e);
            Frame::NONE.fill(t::INLINE).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical().id_salt("preview").max_height(160.0).show(ui, |ui| {
                    ui.label(egui::RichText::new(if text.is_empty() { "Sem alterações pendentes." } else { &text }).font(t::mono(12.0)).color(t::TEXT));
                });
            });
            look::rule(ui);
        }
    }

    fn table_grid(&mut self, ui: &mut Ui, file: &Path) {
        let can_sort = !self.pending() && self.running.is_none();
        let table = self.meta.as_ref().unwrap();
        let editable = self.running.is_none() && !matches!(table.identity(), Identity::ReadOnly);
        let (mut finish, mut cancel) = (false, false);
        let columns: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        let arrows: Vec<&str> = table.columns.iter().map(|c| {
            let name = ident(&c.name);
            if self.applied.1 == format!("{name} ASC") { "↑" } else if self.applied.1 == format!("{name} DESC") { "↓" } else { "" }
        }).collect();
        let rows: Vec<&[Cell]> = self.rows.iter().map(|r| r.values.as_slice()).collect();
        // PhpStorm's pending colors: changed cell, new row, row to delete
        let fill = |n: usize, c: usize| {
            let row = &self.rows[n];
            if row.deleted { Color32::from_rgb(0x48, 0x4A, 0x4A) }
            else if row.original.is_none() { Color32::from_rgb(0x29, 0x44, 0x36) }
            else if row.original.as_ref().is_some_and(|o| o[c] != row.values[c]) { Color32::from_rgb(0x38, 0x55, 0x70) }
            else { Color32::TRANSPARENT }
        };
        let id = self.id.with("cell");
        let editing = self.edit.as_ref().map(|e| (e.row, e.column));
        let mut edit_cell = |ui: &mut Ui, rect: Rect| {
            let Some(edit) = self.edit.as_mut() else { return };
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) { cancel = true; }
            let r = ui.put(rect, TextEdit::singleline(&mut edit.text).id(id).font(t::mono(12.5)).margin(Margin::symmetric(5, 3)).background_color(t::ISLAND));
            if edit.focus { r.request_focus(); edit.focus = false; }
            // Enter or a click elsewhere keeps the text, as in PhpStorm; only Escape drops it
            finish |= r.lost_focus();
        };
        let keys: Vec<bool> = table.columns.iter().map(|c| c.pk > 0).collect();
        let clicks = grid(ui, self.id.with("grid"), &columns, &arrows, &keys, &rows, self.offset, self.reveal.take(), &self.selected, self.cell, &fill, editing.map(|at| (at, &mut edit_cell as &mut dyn FnMut(&mut Ui, Rect))));
        let mut open = clicks.double.filter(|&(n, c)| editable && !self.rows[n].deleted && !table.columns[c].generated);
        let sort_by = clicks.header.filter(|_| can_sort).map(|c| ident(&table.columns[c].name));
        // the edit ends before another cell opens; a bad BLOB keeps its box
        if cancel { self.edit = None; } else if finish && !self.finish_edit() { open = None; }
        if let Some((n, multi)) = clicks.gutter {
            self.cell = None;
            if !multi { self.selected.clear(); }
            if !self.selected.insert(n) { self.selected.remove(&n); }
        }
        if let Some(at) = clicks.cell { self.cell = Some(at); self.selected.clear(); self.selected.insert(at.0); }
        if let Some((n, c)) = open {
            let value = &self.rows[n].values[c];
            self.edit = Some(Edit { row: n, column: c, text: if *value == Cell::Null { String::new() } else { value.text().into() }, focus: true });
        }
        if let Some(name) = sort_by {
            let (asc, desc) = (format!("{name} ASC"), format!("{name} DESC"));
            self.order = if self.applied.1 == asc { desc } else if self.applied.1 == desc { String::new() } else { asc };
            self.applied.1 = self.order.clone();
            self.offset = 0;
            self.reload(file, ui.ctx());
        }
    }

    /// The SQL console the way PhpStorm lays it out: a bar (run, history, transaction), the
    /// editor with line numbers, and the result in a panel under it that can be resized.
    fn console(&mut self, ui: &mut Ui, file: &Path) {
        let mut run = self.running.is_none() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            run |= look::tool(ui, look::Glyph::Run, &format!("Executar a seleção ou a instrução atual ({})", chrome::keys("Ctrl+Enter")), self.running.is_none());
            if look::tool(ui, look::Glyph::History, "Histórico das consultas", !self.history.is_empty()) { self.show_history = !self.show_history; }
            ui.separator();
            ui.label(egui::RichText::new("Tx: Auto").size(12.0).color(t::MUTED));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("main").size(12.0).color(t::MUTED)).on_hover_text("Schema");
            });
        });
        look::rule(ui);
        if self.show_history {
            Frame::NONE.fill(t::INLINE).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical().id_salt("history").max_height(140.0).show(ui, |ui| {
                    for sql in self.history.iter().rev() {
                        if ui.selectable_label(false, egui::RichText::new(sql).font(t::mono(12.0))).clicked() { self.sql = sql.clone(); self.show_history = false; }
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
            if let Some(grid) = &self.result { result_grid(ui, grid, self.id.with("grid")); }
        });
        let editor = Rect::from_min_max(area.min, pos2(area.right(), cut - 1.0));
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(editor));
        let ui = &mut ui;
        let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _width: f32| {
            let mut job = highlight::layout(&highlight::SQL, buf.as_str(), t::mono(13.0), 18.0, &[]);
            job.wrap.max_width = f32::INFINITY;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let gutter = 40.0;
        let mut output = None;
        egui::ScrollArea::both().id_salt("editor").auto_shrink(false).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add_space(gutter);
                let edit = TextEdit::multiline(&mut self.sql).id(self.id).font(t::mono(13.0)).code_editor().frame(Frame::NONE)
                    .desired_rows(12).desired_width(f32::INFINITY).layouter(&mut layouter)
                    .hint_text(format!("SQL · {} executa a seleção ou a instrução atual", chrome::keys("Ctrl+Enter")));
                output = Some(edit.show(ui));
            });
        });
        let Some(output) = output else { return };
        if std::mem::take(&mut self.focus) { output.response.request_focus(); }
        // line numbers in the gutter, one per line of SQL (the editor doesn't wrap)
        let top = output.galley_pos.y;
        let x = output.response.rect.left() - 10.0;
        let clip = ui.clip_rect();
        let caret_line = output.cursor_range.map(|r| self.sql[..byte_at(&self.sql, r.primary.index.0)].matches('\n').count());
        for (n, row) in output.galley.rows.iter().enumerate() {
            let y = top + row.rect().center().y;
            if y < clip.top() || y > clip.bottom() { continue; }
            let color = if caret_line == Some(n) { t::LINE_NUMBER_CARET } else { t::LINE_NUMBER };
            ui.painter().text(pos2(x, y), Align2::RIGHT_CENTER, (n + 1).to_string(), t::mono(12.5), color);
        }
        if run {
            let range = output.cursor_range.map(|r| { let [a, b] = r.sorted_cursors(); (a.index.0, b.index.0) }).unwrap_or((0, 0));
            let sql = instruction(&self.sql, range);
            if !sql.is_empty() {
                self.history.push(sql.clone());
                let file = file.to_path_buf();
                self.start(ui.ctx(), move || { let start = Instant::now(); let result = execute(&file, &format!("PRAGMA foreign_keys=ON;\n{sql}")); Job::Console(result, start.elapsed().as_millis()) });
            }
        }
    }
}

fn result_grid(ui: &mut Ui, result: &Grid, id: Id) {
    let columns: Vec<&str> = result.columns.iter().map(String::as_str).collect();
    let rows: Vec<&[Cell]> = result.rows.iter().map(Vec::as_slice).collect();
    let arrows = vec![""; columns.len()];
    grid(ui, id, &columns, &arrows, &[], &rows, 0, None, &Default::default(), None, &|_, _| Color32::TRANSPARENT, None);
}

/// What the person did in a grid this frame.
#[derive(Default)]
struct Clicks {
    header: Option<usize>,
    /// a row number (and whether Cmd/Ctrl was held, to add to the selection)
    gutter: Option<(usize, bool)>,
    cell: Option<(usize, usize)>,
    double: Option<(usize, usize)>,
}

/// PhpStorm's data grid, painted: a header that stays on top, a gutter of row numbers, thin lines
/// between cells, text on the left (numbers on the right), `<null>` muted, the chosen cell in the
/// selection blue. Only the rows in view are drawn. `edit` puts a text box over the cell being
/// edited.
#[allow(clippy::too_many_arguments)]
fn grid(
    ui: &mut Ui,
    id: Id,
    columns: &[&str],
    arrows: &[&str],
    keys: &[bool],
    rows: &[&[Cell]],
    first: usize,
    reveal: Option<usize>,
    selected: &std::collections::HashSet<usize>,
    cell: Option<(usize, usize)>,
    fill: &dyn Fn(usize, usize) -> Color32,
    mut edit: Option<((usize, usize), &mut dyn FnMut(&mut Ui, Rect))>,
) -> Clicks {
    const ROW: f32 = 24.0;
    const HEAD: f32 = 26.0;
    let font = t::mono(12.5);
    let cw = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
    let gutter = (first + rows.len()).max(1).to_string().len() as f32 * cw + 18.0;
    // each column fits its header and its widest value, within bounds
    let widths: Vec<f32> = (0..columns.len())
        .map(|c| {
            let longest = rows.iter().filter_map(|r| r.get(c)).map(|v| v.text().chars().count()).max().unwrap_or(0);
            // room for the mark before the name and the sort arrows after it
            (longest.max(columns[c].chars().count() + 6).min(40) as f32 * cw + 24.0).max(72.0)
        })
        .collect();
    let size = vec2(gutter + widths.iter().sum::<f32>(), HEAD + rows.len() as f32 * ROW);
    let line = Stroke::new(1.0, t::POPUP_BORDER);
    let mut clicks = Clicks::default();
    let mut area = egui::ScrollArea::both().id_salt(id).auto_shrink(false);
    if let Some(n) = reveal {
        area = area.vertical_scroll_offset((n as f32 * ROW - 200.0).max(0.0));
    }
    area.show_viewport(ui, |ui, view| {
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let origin = rect.min;
        let p = ui.painter().clone();
        let multi = ui.input(|i| i.modifiers.command);
        let first_row = ((view.min.y - HEAD) / ROW).floor().max(0.0) as usize;
        let last_row = ((((view.max.y - HEAD) / ROW).ceil().max(0.0)) as usize + 1).min(rows.len());
        for n in first_row..last_row {
            let y = origin.y + HEAD + n as f32 * ROW;
            let band = Rect::from_min_size(pos2(origin.x, y), vec2(size.x, ROW));
            if selected.contains(&n) {
                p.rect_filled(band, 0, t::SELECTION_INACTIVE);
            } else if n % 2 == 1 {
                p.rect_filled(band, 0, t::INLINE);
            }
            let number = Rect::from_min_size(band.min, vec2(gutter, ROW));
            p.text(number.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, (first + n + 1).to_string(), font.clone(), t::LINE_NUMBER);
            if ui.interact(number, id.with(("row", n)), Sense::click()).clicked() {
                clicks.gutter = Some((n, multi));
            }
            let mut x = origin.x + gutter;
            for (c, width) in widths.iter().enumerate() {
                let at = Rect::from_min_size(pos2(x, y), vec2(*width, ROW));
                let tint = fill(n, c);
                if tint != Color32::TRANSPARENT {
                    p.rect_filled(at, 0, tint);
                }
                let value = rows[n].get(c).unwrap_or(&Cell::Null);
                match edit.as_mut().filter(|(e, _)| *e == (n, c)) {
                    Some((_, put)) => put(ui, at.shrink(1.0)),
                    None => {
                        if cell == Some((n, c)) {
                            p.rect(at.shrink(0.5), 0, t::SELECTION, Stroke::new(1.0, t::ACCENT), egui::StrokeKind::Inside);
                        }
                        let text: String = value.text().chars().take(200).collect();
                        let numeric = matches!(value, Cell::Text(s) if s.parse::<f64>().is_ok());
                        let color = if *value == Cell::Null { t::SECONDARY } else { t::TEXT };
                        let (anchor, align) = if numeric { (at.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER) } else { (at.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER) };
                        p.with_clip_rect(at.shrink2(vec2(6.0, 0.0)).intersect(p.clip_rect())).text(anchor, align, text, font.clone(), color);
                        let r = ui.interact(at, id.with(("cell", n, c)), Sense::click());
                        if r.clicked() {
                            clicks.cell = Some((n, c));
                        }
                        if r.double_clicked() {
                            clicks.double = Some((n, c));
                        }
                        if value.text().chars().count() as f32 * cw > width - 16.0 {
                            r.on_hover_text(value.text());
                        }
                    }
                }
                x += width;
                p.vline(x, at.y_range(), line);
            }
            p.hline(band.x_range(), band.bottom(), line);
        }
        // the header stays on top while the rows scroll under it
        let top = origin.y + view.min.y;
        let head = Rect::from_min_size(pos2(origin.x, top), vec2(size.x.max(view.width()), HEAD));
        p.rect_filled(head, 0, t::FRAME);
        let mut x = origin.x + gutter;
        for (c, width) in widths.iter().enumerate() {
            let at = Rect::from_min_size(pos2(x, top), vec2(*width, HEAD));
            let r = ui.interact(at, id.with(("head", c)), Sense::click());
            if r.hovered() {
                p.rect_filled(at, 0, t::ROW_HOVER);
            }
            let inner = p.with_clip_rect(at.shrink2(vec2(4.0, 0.0)).intersect(p.clip_rect()));
            look::mark(&inner, at.left_center() + vec2(12.0, 0.0), look::Mark::Column { pk: keys.get(c).copied().unwrap_or(false) });
            inner.text(at.left_center() + vec2(24.0, 0.0), Align2::LEFT_CENTER, columns[c], t::ui(12.5), t::TEXT);
            // ⇕, the active half lit when the column orders the rows
            let (up, down) = (arrows[c] == "↑", arrows[c] == "↓");
            let a = at.right_center() - vec2(10.0, 0.0);
            for (dir, lit) in [(-1.0, up), (1.0, down)] {
                let color = if lit { t::LINK } else { t::SECONDARY };
                inner.add(egui::Shape::convex_polygon(vec![a + vec2(-3.0, 1.5 * dir), a + vec2(3.0, 1.5 * dir), a + vec2(0.0, 4.5 * dir)], color, Stroke::NONE));
            }
            if r.on_hover_text("Clique para ordenar (ASC, DESC, nada)").clicked() {
                clicks.header = Some(c);
            }
            x += width;
            p.vline(x, at.y_range(), line);
        }
        p.hline(head.x_range(), head.bottom(), Stroke::new(1.0, t::CONTROL_BORDER));
    });
    clicks
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
        // a click elsewhere takes the focus (held for a frame first, as egui records it) and keeps the text
        sheet.edit = Some(Edit { row: 0, column: 0, text: "fora".into(), focus: true });
        frame(&mut sheet, None);
        frame(&mut sheet, None);
        ctx.memory_mut(|m| m.surrender_focus(sheet.id.with("cell")));
        frame(&mut sheet, None);
        assert!(sheet.edit.is_none());
        assert_eq!(sheet.rows[0].values[0], Cell::Text("fora".into()));
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

/// "1 linha", "4 linhas".
fn rows_label(n: usize) -> String {
    format!("{n} {}", if n == 1 { "linha" } else { "linhas" })
}
