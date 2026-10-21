//! Database explorer and editor tabs, backed by the machine's safe SQLite shell.

use super::find::rel;
use super::*;
use sql::*;

mod sql;
mod view;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct Db {
    files: Option<Vec<PathBuf>>,
    file: Option<PathBuf>,
    tables: Vec<Table>,
    open: std::collections::HashSet<String>,
    error: Option<String>,
    seq: u64,
}

pub(super) use view::Sheet;

fn is_sqlite(path: &Path) -> bool {
    let mut head = [0u8; 16];
    std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && &head == b"SQLite format 3\0"
}

impl Ide {
    pub(super) fn db_panel(&mut self, ui: &mut Ui) {
        // ponytail: file discovery follows the project list; reopen the IDE for new files.
        if self.db.files.is_none() && self.files_rx.is_none() {
            let found: Vec<PathBuf> = self.files.iter().filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("sqlite" | "sqlite3" | "db")) && is_sqlite(p)).cloned().collect();
            if let Some(first) = found.first().cloned() { self.open_db(first); }
            self.db.files = Some(found);
        }
        let (mut console, mut refresh) = (false, false);
        ui.horizontal(|ui| {
            ui.set_height(36.0);
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Banco de dados").color(t::TEXT).strong());
            ui.add_enabled_ui(self.db.file.is_some(), |ui| {
                refresh = ui.small_button("↻").on_hover_text("Recarregar estrutura").clicked();
                console = ui.small_button("Console SQL").clicked();
            });
        });
        if self.db.files.as_ref().is_none_or(|f| f.is_empty()) {
            ui.label(egui::RichText::new("Nenhum banco SQLite neste projeto (.sqlite, .sqlite3 ou .db).").color(t::MUTED));
            return;
        }
        if let Some(error) = &self.db.error { ui.colored_label(t::ERROR, error); }
        let (mut pick, mut show) = (None, None);
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for file in self.db.files.iter().flatten() {
                let on = self.db.file.as_ref() == Some(file);
                let (resp, rect) = row(ui, false, 24.0);
                let chevron = if on { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
                icons::paint(ui.painter(), chevron, Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(14.0, 14.0)));
                table_icon(ui.painter(), pos2(rect.left() + 33.0, rect.center().y));
                let name = ui.painter().text(pos2(rect.left() + 46.0, rect.center().y), Align2::LEFT_CENTER, rel(&self.root, file), t::ui(t::UI_SIZE), t::TEXT);
                ui.painter().text(pos2(name.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, "SQLite", t::ui(12.0), t::MUTED);
                if resp.clicked() && !on { pick = Some(file.clone()); }
                if !on { continue; }
                ui.push_id(file, |ui| {
                    ui.indent("tables", |ui| {
                        egui::CollapsingHeader::new(format!("tabelas {}", self.db.tables.len())).default_open(true).show(ui, |ui| {
                            for table in &self.db.tables {
                                let open = self.db.open.contains(&table.name);
                                let (resp, rect) = row(ui, false, 24.0);
                                let x = rect.left() + 10.0;
                                icons::paint(ui.painter(), if open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT }, Rect::from_center_size(pos2(x, rect.center().y), vec2(14.0, 14.0)));
                                table_icon(ui.painter(), pos2(x + 17.0, rect.center().y));
                                ui.painter().text(pos2(x + 30.0, rect.center().y), Align2::LEFT_CENTER, &table.name, t::ui(t::UI_SIZE), t::TEXT);
                                let resp = resp.on_hover_text("Clique duplo abre os dados");
                                if resp.double_clicked() { show = Some(table.name.clone()); }
                                else if resp.clicked() {
                                    if open { self.db.open.remove(&table.name); } else { self.db.open.insert(table.name.clone()); }
                                }
                                if !self.db.open.contains(&table.name) { continue; }
                                ui.push_id(&table.name, |ui| ui.indent("structure", |ui| {
                                    for column in &table.columns {
                                        let (_, rect) = row(ui, false, 22.0);
                                        let x = rect.left() + 10.0;
                                        column_icon(ui.painter(), pos2(x, rect.center().y), column.pk > 0,
                                            table.foreign.iter().any(|(c, _)| c == &column.name), column.notnull);
                                        let name = ui.painter().text(pos2(x + 24.0, rect.center().y), Align2::LEFT_CENTER, &column.name, t::ui(t::UI_SIZE), t::TEXT);
                                        ui.painter().text(pos2(name.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, column.kind.to_lowercase(), t::ui(12.0), t::MUTED);
                                    }
                                    egui::CollapsingHeader::new("chaves").show(ui, |ui| {
                                        let mut keys: Vec<_> = table.columns.iter().filter(|c| c.pk > 0).collect();
                                        keys.sort_by_key(|c| c.pk);
                                        for c in keys { object_row(ui, &c.name, Some(t::WARNING)); }
                                    });
                                    egui::CollapsingHeader::new("chaves estrangeiras").show(ui, |ui| {
                                        for (_, name) in &table.foreign { object_row(ui, name, Some(t::LINK)); }
                                    });
                                    egui::CollapsingHeader::new("índices").show(ui, |ui| {
                                        for (name, cols) in &table.indexes { object_row(ui, &format!("{name} ({cols})"), None); }
                                    });
                                }));
                            }
                        });
                    });
                });
            }
        });
        if let Some(file) = pick.or_else(|| refresh.then(|| self.db.file.clone()).flatten()) { self.open_db(file); }
        if let Some(table) = show { self.open_sheet(Some(table)); }
        if console { self.open_sheet(None); }
    }

    fn open_db(&mut self, file: PathBuf) {
        // ponytail: schema reads once on the UI thread; move to a worker for huge schemas.
        let tables = schema(&file);
        self.db.error = tables.as_ref().err().cloned();
        self.db.tables = tables.unwrap_or_default();
        self.db.open.clear();
        self.db.file = Some(file);
    }

    pub(super) fn open_sheet(&mut self, table: Option<String>) {
        let Some(file) = self.db.file.clone() else { return };
        if let Some(i) = self.tabs.iter().position(|t| t.path == file && table.is_some() && t.sheet.as_ref().is_some_and(|s| s.table == table)) {
            self.active = i;
            return;
        }
        let meta = table.as_ref().and_then(|name| self.db.tables.iter().find(|t| &t.name == name)).cloned();
        self.db.seq += 1;
        let mut sheet = Sheet::new(self.id.with(("db_sheet", self.db.seq)), meta);
        if table.is_some() { sheet.reload(&file, &self.ctx); }
        let mut tab = Tab::new(file, String::new(), false);
        tab.sheet = Some(Box::new(sheet));
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    pub(super) fn sheet_view(&mut self, ui: &mut Ui) {
        let tab = &mut self.tabs[self.active];
        if let Some(sheet) = tab.sheet.as_mut() { sheet.ui(ui, &tab.path); }
    }
}

fn table_icon(p: &egui::Painter, at: egui::Pos2) {
    let r = Rect::from_center_size(at, vec2(13.0, 11.0));
    let stroke = Stroke::new(1.0, Color32::from_rgb(0x8C, 0x9B, 0xB0));
    p.rect_stroke(r, 1.5, stroke, egui::StrokeKind::Inside);
    p.line_segment([pos2(r.left(), r.top() + 3.5), pos2(r.right(), r.top() + 3.5)], stroke);
    p.line_segment([pos2(r.center().x, r.top() + 3.5), pos2(r.center().x, r.bottom())], stroke);
}

fn key_icon(p: &egui::Painter, at: egui::Pos2, color: Color32) {
    let s = Stroke::new(1.4, color);
    p.circle_stroke(at - vec2(3.0, 0.0), 2.6, s);
    p.line_segment([at, at + vec2(7.0, 0.0)], s);
    p.line_segment([at + vec2(4.0, 0.0), at + vec2(4.0, 3.0)], s);
    p.line_segment([at + vec2(7.0, 0.0), at + vec2(7.0, 3.0)], s);
}

fn column_icon(p: &egui::Painter, at: egui::Pos2, pk: bool, fk: bool, notnull: bool) {
    if pk { key_icon(p, at - vec2(0.0, if fk { 3.0 } else { 0.0 }), t::WARNING); }
    if fk { key_icon(p, at + vec2(0.0, if pk { 4.0 } else { 0.0 }), t::LINK); }
    if !pk && !fk { p.circle_stroke(at, 3.0, Stroke::new(1.0, t::SECONDARY)); }
    if notnull { p.circle_filled(at + vec2(12.0, 0.0), 1.8, t::TEXT); }
}

fn object_row(ui: &mut Ui, name: &str, key: Option<Color32>) {
    let (_, rect) = row(ui, false, 22.0);
    let at = pos2(rect.left() + 9.0, rect.center().y);
    if let Some(color) = key { key_icon(ui.painter(), at, color); }
    else { table_icon(ui.painter(), at); }
    ui.painter().text(at + vec2(20.0, 0.0), Align2::LEFT_CENTER, name, t::ui(12.0), t::TEXT);
}
