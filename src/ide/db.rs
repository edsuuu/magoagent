//! Database explorer and editor tabs, backed by the machine's safe SQLite shell.

use super::find::rel;
use super::*;
use sql::*;

mod look;
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
        // PhpStorm's tool window: the title, a row of icon buttons, then the tree
        let (mut console, mut refresh) = (false, false);
        tool_header(ui, "Banco de dados");
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            let on = self.db.file.is_some();
            refresh = look::tool(ui, look::Glyph::Refresh, "Recarregar a estrutura", on);
            console = look::tool(ui, look::Glyph::Run, "Novo console SQL", on);
        });
        look::rule(ui);
        if self.db.files.as_ref().is_none_or(|f| f.is_empty()) {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.add(egui::Label::new(egui::RichText::new("Nenhum banco SQLite neste projeto (.sqlite, .sqlite3 ou .db).").color(t::MUTED)).wrap());
            });
            return;
        }
        if let Some(error) = &self.db.error { ui.colored_label(t::ERROR, error); }
        let (mut pick, mut show) = (None, None);
        let mut toggle = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let open = |key: &str| self.db.open.contains(key);
            for file in self.db.files.iter().flatten() {
                let on = self.db.file.as_ref() == Some(file);
                let count = on.then_some(self.db.tables.len());
                if look::tree_row(ui, 0, Some(on), look::Mark::Source, &rel(&self.root, file), "SQLite", count).clicked() && !on {
                    pick = Some(file.clone());
                }
                if !on { continue; }
                if look::tree_row(ui, 1, Some(open("#main")), look::Mark::Schema, "main", "", None).clicked() { toggle = Some("#main".to_string()); }
                if !open("#main") { continue; }
                let n = self.db.tables.len().to_string();
                if look::tree_row(ui, 2, Some(open("#tables")), look::Mark::Folder, "tabelas", &n, None).clicked() { toggle = Some("#tables".to_string()); }
                if !open("#tables") { continue; }
                for table in &self.db.tables {
                    let resp = look::tree_row(ui, 3, Some(open(&table.name)), look::Mark::Table, &table.name, "", None).on_hover_text("Clique duplo abre os dados");
                    if resp.double_clicked() { show = Some(table.name.clone()); }
                    else if resp.clicked() { toggle = Some(table.name.clone()); }
                    if !open(&table.name) { continue; }
                    // the table's structure in folders with counts, as PhpStorm groups it
                    let keys: Vec<&Column> = { let mut k: Vec<_> = table.columns.iter().filter(|c| c.pk > 0).collect(); k.sort_by_key(|c| c.pk); k };
                    let groups = [("colunas", table.columns.len()), ("chaves", usize::from(!keys.is_empty())), ("chaves estrangeiras", table.foreign.len()), ("índices", table.indexes.len())];
                    for (group, count) in groups.into_iter().filter(|(_, n)| *n > 0) {
                        let key = format!("{}#{group}", table.name);
                        if look::tree_row(ui, 4, Some(open(&key)), look::Mark::Folder, group, &count.to_string(), None).clicked() { toggle = Some(key.clone()); }
                        if !open(&key) { continue; }
                        match group {
                            "colunas" => for c in &table.columns {
                                let mut note = c.kind.to_lowercase();
                                // INTEGER PRIMARY KEY is the rowid: SQLite numbers it, like auto increment
                                if c.pk > 0 && keys.len() == 1 && c.kind.eq_ignore_ascii_case("integer") && !table.without_rowid { note += " (auto increment)"; }
                                if let Some(d) = &c.default { note += &format!(" = {d}"); }
                                if c.notnull && c.pk == 0 { note += " not null"; }
                                look::tree_row(ui, 5, None, look::Mark::Column { pk: c.pk > 0 }, &c.name, &note, None);
                            },
                            "chaves" => {
                                let cols: Vec<&str> = keys.iter().map(|c| c.name.as_str()).collect();
                                look::tree_row(ui, 5, None, look::Mark::Key(look::PRIMARY_KEY), "primary key", &format!("({})", cols.join(", ")), None);
                            }
                            "chaves estrangeiras" => for (_, fk) in &table.foreign {
                                look::tree_row(ui, 5, None, look::Mark::Key(look::FOREIGN_KEY), fk, "", None);
                            },
                            _ => for (name, cols) in &table.indexes {
                                look::tree_row(ui, 5, None, look::Mark::Index, name, &format!("({cols})"), None);
                            },
                        }
                    }
                }
            }
        });
        if let Some(key) = toggle
            && !self.db.open.remove(&key)
        {
            // a table opens with its columns showing
            if !key.contains('#') { self.db.open.insert(format!("{key}#colunas")); }
            self.db.open.insert(key);
        }
        if let Some(file) = pick.or_else(|| refresh.then(|| self.db.file.clone()).flatten()) { self.open_db(file); }
        if let Some(table) = show { self.open_sheet(Some(table)); }
        if console { self.open_sheet(None); }
    }

    fn open_db(&mut self, file: PathBuf) {
        // ponytail: schema reads once on the UI thread; move to a worker for huge schemas.
        let tables = schema(&file);
        self.db.error = tables.as_ref().err().cloned();
        self.db.tables = tables.unwrap_or_default();
        // a reload keeps what was open; another file starts with its tables showing
        if self.db.file.as_ref() != Some(&file) {
            self.db.open = ["#main", "#tables"].map(String::from).into();
        }
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
