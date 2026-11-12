//! Table mutations and the SQL console share a tab, but never share write permissions.

use super::grid::*;
#[cfg(test)]
pub(super) use super::instruction::instruction;
pub(super) use super::instruction::instruction_mode;
use super::*;
use session::{Job, Op, Session};
mod console;
mod table;

struct Edit {
    row: usize,
    column: usize,
    text: String,
    focus: bool,
}

pub(crate) struct Sheet {
    id: Id,
    pub(super) session: Session,
    pub table: Option<String>,
    meta: Option<Table>,
    sql: String,
    result: Option<Grid>,
    rows: Vec<DataRow>,
    count: usize,
    ttl: Option<i64>,
    offset: usize,
    filter: String,
    order: String,
    applied: (String, String),
    selected: std::collections::HashSet<usize>,
    cell: Option<(usize, usize)>,
    edit: Option<Edit>,
    running: Option<Receiver<Job>>,
    revision: u64,
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
    pub(super) fn new(id: Id, meta: Option<Table>, session: Session) -> Self {
        let mongo = session.dialect == Dialect::Mongo;
        Self {
            id,
            session,
            table: meta.as_ref().map(|t| if t.dialect != Dialect::Sqlite { format!("{}.{}", t.schema, t.name) } else { t.name.clone() }),
            meta,
            sql: String::new(),
            result: None,
            rows: vec![],
            count: 0,
            ttl: None,
            offset: 0,
            filter: if mongo { "{}".into() } else { String::new() },
            order: if mongo { r#"{"_id": 1}"#.into() } else { String::new() },
            applied: Default::default(),
            selected: Default::default(),
            cell: None,
            edit: None,
            running: None,
            revision: 0,
            message: None,
            history: vec![],
            show_history: false,
            preview: false,
            focus: true,
            split: 260.0,
            reveal: None,
        }
    }

    pub(crate) fn pending(&self) -> bool {
        self.edit.is_some() || self.rows.iter().any(DataRow::pending)
    }

    pub(crate) fn busy(&self) -> bool {
        self.pending() || self.running.is_some()
    }
    pub(crate) fn source(&self) -> &str {
        &self.session.id
    }
    pub(crate) fn matches(&self, id: &str, schema: &str, table: Option<&str>) -> bool {
        table.is_some() && self.session.id == id && self.meta.as_ref().is_some_and(|t| t.schema == schema && Some(t.name.as_str()) == table)
    }
    pub(super) fn ddl(&mut self, object: session::Object, ctx: &egui::Context) {
        self.start(ctx, Op::Ddl(object));
    }

    fn start(&mut self, ctx: &egui::Context, op: Op) {
        self.running = Some(self.session.request(op, ctx));
        self.message = None;
    }

    pub(super) fn reload(&mut self, ctx: &egui::Context) {
        if self.pending() || self.running.is_some() {
            return;
        }
        let Some(table) = self.meta.clone() else { return };
        let (filter, order, offset) = (self.applied.0.clone(), self.applied.1.clone(), self.offset);
        self.revision = self.session.revision();
        self.rows.clear();
        self.selected.clear();
        self.cell = None;
        self.start(ctx, Op::Page(table, filter, order, offset));
    }

    pub(crate) fn poll(&mut self, ctx: &egui::Context) {
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
                self.ttl = page.ttl;
                self.rows = page.rows;
                if self.offset > 0 && self.rows.is_empty() {
                    self.offset = self.count.saturating_sub(1) / PAGE * PAGE;
                    self.reload(ctx);
                }
            }
            Job::Page(Err(e)) | Job::Submit(Err(e)) => self.message = Some((e, true)),
            Job::Submit(Ok(())) => {
                self.rows.clear();
                self.edit = None;
                self.message = Some(("Operação concluída.".into(), false));
                self.reload(ctx);
            }
            Job::Console(result, ms) => match result {
                Ok(grid) => {
                    self.message = Some((format!("{} em {ms} ms", rows_label(grid.rows.len())), false));
                    self.result = Some(grid);
                }
                Err(e) => {
                    self.result = None;
                    self.message = Some((e, true));
                }
            },
            Job::Ddl(result) => match result {
                Ok(ddl) => self.sql = ddl,
                Err(e) => self.message = Some((e, true)),
            },
            _ => {}
        }
    }

    pub(crate) fn ui(&mut self, ui: &mut Ui) {
        self.poll(ui.ctx());
        if self.meta.is_some() && self.revision != self.session.revision() && !self.busy() {
            self.reload(ui.ctx());
        }
        ui.push_id(self.id, |ui| {
            if self.meta.is_some() {
                self.table_toolbar(ui);
                let area = ui.available_rect_before_wrap();
                self.table_grid(ui);
                self.paging(ui, area);
            } else {
                self.console(ui);
            }
        });
    }

    fn ident(&self, name: &str) -> String {
        self.session.dialect.ident(name)
    }

    fn transaction_bar(&mut self, ui: &mut Ui) {
        if matches!(self.session.dialect, Dialect::Sqlite | Dialect::Mongo | Dialect::Redis) {
            ui.label("Tx: Auto");
            return;
        }
        let auto = self.session.auto();
        let idle = !self.busy();
        let mut next = auto;
        ui.add_enabled_ui(idle, |ui| {
            egui::ComboBox::from_id_salt("tx").selected_text(if auto { "Tx: Auto" } else { "Tx: Manual" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut next, true, "Auto (confirma as pendências)");
                ui.selectable_value(&mut next, false, "Manual");
            });
        });
        if next != auto {
            self.start(ui.ctx(), Op::Auto(next));
        }
        if !auto {
            if ui.add_enabled(idle, egui::Button::new("Commit")).clicked() {
                self.start(ui.ctx(), Op::Commit);
            }
            if ui.add_enabled(idle, egui::Button::new("Rollback")).clicked() {
                self.start(ui.ctx(), Op::Rollback);
            }
        }
    }

    /// PhpStorm's floating pill over the bottom of the grid: the rows shown and, past one page,
    /// the arrows. A failed query shows in a red strip under it, until closed.
    fn paging(&mut self, ui: &mut Ui, area: Rect) {
        let mut bottom = area.bottom() - 10.0;
        if let Some((message, true)) = &self.message {
            let strip = Rect::from_min_max(pos2(area.left(), area.bottom() - 30.0), area.max);
            ui.painter().rect_filled(strip, 0, Color32::from_rgb(0x5E, 0x34, 0x34));
            ui.painter().with_clip_rect(strip.shrink2(vec2(30.0, 0.0))).text(
                strip.left_center() + vec2(12.0, 0.0),
                Align2::LEFT_CENTER,
                message,
                t::ui(12.5),
                t::TEXT,
            );
            let close = Rect::from_center_size(strip.right_center() - vec2(16.0, 0.0), vec2(20.0, 20.0));
            icons::paint(ui.painter(), icons::CLOSE_SMALL, close.shrink(2.0));
            if ui.interact(close, self.id.with("close_error"), Sense::click()).clicked() {
                self.message = None;
            }
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
                if look::tool(ui, look::Glyph::First, "Primeira página", clean && self.offset > 0) {
                    go = Some(0);
                }
                if look::tool(ui, look::Glyph::Previous, "Página anterior", clean && self.offset > 0) {
                    go = Some(self.offset.saturating_sub(PAGE));
                }
            }
            let (r, resp) = ui.allocate_exact_size(vec2(text.size().x + 12.0, 26.0), Sense::hover());
            ui.painter().galley(r.center() - text.size() / 2.0, text, t::TEXT);
            if !tip.is_empty() {
                resp.on_hover_text(tip);
            }
            if pages {
                if look::tool(ui, look::Glyph::Next, "Próxima página", clean && self.offset < last) {
                    go = Some(self.offset + PAGE);
                }
                if look::tool(ui, look::Glyph::Last, "Última página", clean && self.offset < last) {
                    go = Some(last);
                }
            }
        });
        if let Some(offset) = go {
            self.offset = offset;
            self.reload(ui.ctx());
        }
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
        for r in &mut self.rows {
            r.values = r.original.clone().unwrap();
            r.deleted = false;
        }
        self.selected.clear();
        self.cell = None;
        self.message = None;
    }
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
        let mut sheet = Sheet::new(
            Id::unique("cell-test"),
            Some(Table { name: "t".into(), columns: vec![Column { name: "value".into(), kind: "TEXT".into(), ..Default::default() }], ..Default::default() }),
            Session::new(conexoes::Conexao::sqlite(Path::new("unused.sqlite")), String::new()),
        );
        sheet.rows.push(DataRow { original: Some(vec![Cell::Null]), values: vec![Cell::Null], rowid: Cell::Text("1".into()), deleted: false });
        sheet.edit = Some(Edit { row: 0, column: 0, text: String::new(), focus: true });
        let frame = |sheet: &mut Sheet, key: Option<Key>| {
            let mut input = egui::RawInput::default();
            if let Some(key) = key {
                input.events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
            }
            ctx.run_ui(input, |ui| {
                t::style(ui.style_mut());
                sheet.table_grid(ui);
            })
            .textures_delta
            .clear();
        };
        frame(&mut sheet, None);
        frame(&mut sheet, Some(Key::Enter));
        assert!(sheet.edit.is_none());
        assert_eq!(sheet.rows[0].values[0], Cell::Text(String::new()));
        assert!(sheet.pending());
        sheet.reload(&ctx);
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
