//! The Database tool window: the SQLite files in the project (Laravel's
//! database/database.sqlite too), their tables and columns, and an SQL console with the result
//! in a grid. It runs the `sqlite3` already on the machine, read-only: nothing is written to
//! the databases, and sqlite3's dot-commands that reach outside the file are off (`-safe`).

use super::find::rel;
use super::*;
use std::sync::mpsc::channel;

/// Rows drawn in the grid; the rest are counted, not shown.
const SHOWN: usize = 500;

pub(super) struct Grid {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Default)]
pub(super) struct Db {
    /// The project's SQLite files and the one open in the window.
    files: Vec<PathBuf>,
    file: Option<PathBuf>,
    /// table → (column, type, primary key)
    tables: Vec<(String, Vec<(String, String, bool)>)>,
    sql: String,
    result: Option<Result<Grid, String>>,
    running: Option<Receiver<Result<Grid, String>>>,
}

/// A SQLite file by its first bytes, not its name (".db" means many things).
fn is_sqlite(path: &Path) -> bool {
    let mut head = [0u8; 16];
    std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && &head == b"SQLite format 3\0"
}

/// Runs `sql` on `file` with sqlite3 in ASCII mode: fields split by 0x1F, rows by 0x1E, the
/// header first, NULL written as NULL.
pub(super) fn query(file: &Path, sql: &str) -> Result<Grid, String> {
    let out = command("sqlite3")
        // -ascii before -header: the other way round sqlite3 drops the header
        .args(["-readonly", "-safe", "-ascii", "-header", "-nullvalue", "NULL"])
        .arg(file)
        .arg(sql)
        .output()
        .map_err(|e| format!("sqlite3 não encontrado ({e}); instale o SQLite"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(parse(&String::from_utf8_lossy(&out.stdout)))
}

pub(super) fn parse(text: &str) -> Grid {
    let mut rows = text.split('\x1e').filter(|r| !r.is_empty()).map(|r| r.split('\x1f').map(String::from).collect::<Vec<_>>());
    let columns = rows.next().unwrap_or_default();
    Grid { columns, rows: rows.collect() }
}

/// Every table and its columns in one call.
fn schema(file: &Path) -> Result<Vec<(String, Vec<(String, String, bool)>)>, String> {
    const SQL: &str = "SELECT m.name, p.name, p.type, p.pk FROM sqlite_master m JOIN pragma_table_info(m.name) p \
                       WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite_%' ORDER BY m.name, p.cid";
    let mut tables: Vec<(String, Vec<(String, String, bool)>)> = Vec::new();
    for row in query(file, SQL)?.rows {
        let [table, column, kind, pk] = &row[..] else { continue };
        if tables.last().is_none_or(|(t, _)| t != table) {
            tables.push((table.clone(), Vec::new()));
        }
        if let Some((_, columns)) = tables.last_mut() {
            columns.push((column.clone(), kind.clone(), pk != "0"));
        }
    }
    Ok(tables)
}

impl Ide {
    pub(super) fn db_window(&mut self, ui: &mut Ui) {
        if self.db.files.is_empty() {
            // ponytail: found once per opening; a database created later needs the window reopened
            self.db.files = self.files.iter().filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("sqlite" | "sqlite3" | "db")) && is_sqlite(p)).cloned().collect();
            if let Some(first) = self.db.files.first().cloned() {
                self.open_db(first);
            }
        }
        if let Some(rx) = &self.db.running {
            if let Ok(result) = rx.try_recv() {
                self.db.result = Some(result);
                self.db.running = None;
            }
        }
        tool_header(ui, "Banco de dados");
        if self.db.files.is_empty() {
            ui.label(egui::RichText::new("Nenhum banco SQLite neste projeto (.sqlite, .sqlite3 ou .db).").color(t::MUTED));
            return;
        }
        let mut run = None;
        let mut pick = None;
        ui.horizontal_top(|ui| {
            egui::ScrollArea::vertical().id_salt("db_tree").max_width(260.0).auto_shrink([true, false]).show(ui, |ui| ui.vertical(|ui| {
                ui.set_min_width(240.0);
                for file in &self.db.files {
                    let on = self.db.file.as_ref() == Some(file);
                    let name = rel(&self.root, file);
                    if ui.selectable_label(on, egui::RichText::new(name).strong()).clicked() && !on {
                        pick = Some(file.clone());
                    }
                }
                ui.separator();
                for (table, columns) in &self.db.tables {
                    egui::CollapsingHeader::new(egui::RichText::new(table).color(t::TEXT)).id_salt(("db_table", table)).show(ui, |ui| {
                        if ui.small_button("Ver dados").clicked() {
                            run = Some(format!("SELECT * FROM \"{}\" LIMIT 200;", table.replace('"', "\"\"")));
                        }
                        for (column, kind, pk) in columns {
                            let mark = if *pk { "PK " } else { "" };
                            ui.label(egui::RichText::new(format!("{mark}{column}  {kind}")).font(t::mono(12.0)).color(t::MUTED));
                        }
                    });
                }
            }));
            ui.separator();
            ui.vertical(|ui| {
                // ⌘Enter is taken before the text box sees it, or the box swallows it
                let console_id = self.id.with("db_console");
                let enter = ui.memory(|m| m.has_focus(console_id)) && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
                let console = egui::TextEdit::multiline(&mut self.db.sql)
                    .id(console_id)
                    .font(t::mono(13.0))
                    .code_editor()
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text("SELECT … (⌘Enter executa; o banco abre só para leitura)");
                ui.add(console);
                ui.horizontal(|ui| {
                    if ui.button("▶ Executar").clicked() || enter {
                        run = Some(self.db.sql.clone());
                    }
                    let note = match (&self.db.running, &self.db.result) {
                        (Some(_), _) => "Executando…".to_string(),
                        (_, Some(Ok(g))) if g.rows.len() > SHOWN => format!("{} linhas (mostrando {SHOWN})", g.rows.len()),
                        (_, Some(Ok(g))) => format!("{} linha(s)", g.rows.len()),
                        _ => String::new(),
                    };
                    ui.label(egui::RichText::new(note).color(t::MUTED));
                });
                match &self.db.result {
                    Some(Err(error)) => {
                        ui.label(egui::RichText::new(error).color(t::ERROR));
                    }
                    Some(Ok(grid)) => grid_ui(ui, grid),
                    None => {}
                }
            });
        });
        if let Some(file) = pick {
            self.open_db(file);
        }
        if let (Some(sql), Some(file)) = (run, self.db.file.clone()) {
            self.db.sql = sql.clone();
            let (tx, rx) = channel();
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(query(&file, &sql));
                ctx.request_repaint();
            });
            self.db.running = Some(rx);
        }
    }

    /// Reads the schema and shows its error (no sqlite3, say) where a result would go.
    fn open_db(&mut self, file: PathBuf) {
        // ponytail: one sqlite3 run on the UI thread per opening; a database with thousands of
        // tables would hitch once — move it to a thread like the console if it shows
        let tables = schema(&file);
        self.db.result = tables.as_ref().err().map(|e| Err(e.clone()));
        self.db.tables = tables.unwrap_or_default();
        self.db.file = Some(file);
    }
}

/// The result as a table: a header row and at most SHOWN rows, long values cut.
fn grid_ui(ui: &mut Ui, grid: &Grid) {
    egui::ScrollArea::both().id_salt("db_grid").auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("db_grid_rows").striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
            for column in &grid.columns {
                ui.label(egui::RichText::new(column).font(t::mono(12.5)).color(t::TEXT).strong());
            }
            ui.end_row();
            for row in grid.rows.iter().take(SHOWN) {
                for value in row {
                    let short: String = value.chars().take(60).collect();
                    let color = if value == "NULL" { t::MUTED } else { t::TEXT };
                    ui.label(egui::RichText::new(short).font(t::mono(12.5)).color(color));
                }
                ui.end_row();
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_tables_rows_and_nothing_written() {
        let grid = parse("id\x1fnome\x1e1\x1fAna\x1e2\x1fNULL\x1e");
        assert_eq!(grid.columns, ["id", "nome"]);
        assert_eq!(grid.rows, [["1", "Ana"], ["2", "NULL"]]);
        if command("sqlite3").arg("-version").output().is_err() {
            return; // no sqlite3 on this machine: the parsing above is what's left to check
        }
        let dir = std::env::temp_dir().join(format!("mago-db-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("teste.sqlite");
        let create = "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT); INSERT INTO users(name) VALUES ('Ana'), (NULL);";
        assert!(command("sqlite3").arg(&file).arg(create).status().unwrap().success());
        assert!(is_sqlite(&file));
        let tables = schema(&file).unwrap();
        assert_eq!(tables[0].0, "users");
        assert_eq!(tables[0].1[0], ("id".to_string(), "INTEGER".to_string(), true));
        let rows = query(&file, "SELECT * FROM users ORDER BY id").unwrap();
        assert_eq!((rows.columns.len(), rows.rows[1][1].as_str()), (2, "NULL"));
        // read-only and safe: no writing, no shell
        assert!(query(&file, "DELETE FROM users").is_err());
        assert!(query(&file, ".shell echo oi").is_err());
        assert_eq!(query(&file, "SELECT count(*) AS n FROM users").unwrap().rows, [["2"]]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
