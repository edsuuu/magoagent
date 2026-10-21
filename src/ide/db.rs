//! The Database tool window: the SQLite files in the project (Laravel's
//! database/database.sqlite too), their tables and columns. A table's rows, or an SQL console,
//! open as editor tabs with the result in a grid. It runs the `sqlite3` already on the machine, read-only: nothing is written to
//! the databases, and sqlite3's dot-commands that reach outside the file are off (`-safe`), as
//! are ATTACH (so VACUUM INTO too), readfile(), writefile() and load_extension(). What `-safe`
//! leaves open is fsdir(), which lists and reads files the user can read anyway; blocking it
//! would take an authorizer, i.e. linking SQLite instead of running the shell.

use super::find::rel;
use super::*;
use std::io::Read;
use std::process::Stdio;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

/// Rows drawn in the grid; the rest are counted, not shown.
const SHOWN: usize = 500;
/// Bytes of result kept; past that sqlite3 is stopped and the count says "mais de".
const CAP: usize = 8 << 20;
/// A query still running after this is killed.
const DEADLINE: Duration = Duration::from_secs(60);

pub(super) struct Grid {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// The output passed CAP: `rows` is the start of the result, not all of it.
    pub more: bool,
}

#[derive(Default)]
pub(super) struct Db {
    /// The project's SQLite files (None until looked for) and the one open in the panel.
    files: Option<Vec<PathBuf>>,
    file: Option<PathBuf>,
    /// table → (column, type, primary key)
    tables: Vec<(String, Vec<(String, String, bool)>)>,
    /// tables showing their columns
    open: std::collections::HashSet<String>,
    /// why the tables couldn't be read (no sqlite3, say)
    error: Option<String>,
}

/// A table's rows, or an SQL console, open as an editor tab like PhpStorm's data editor.
pub(super) struct Sheet {
    /// None for a console
    pub table: Option<String>,
    sql: String,
    result: Option<Result<Grid, String>>,
    running: Option<Receiver<Result<Grid, String>>>,
}

impl Sheet {
    fn run(&mut self, file: &Path, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let (file, sql, ctx) = (file.to_path_buf(), self.sql.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(query(&file, &sql));
            ctx.request_repaint();
        });
        self.running = Some(rx);
    }
}

/// A SQLite file by its first bytes, not its name (".db" means many things).
fn is_sqlite(path: &Path) -> bool {
    let mut head = [0u8; 16];
    std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && &head == b"SQLite format 3\0"
}

/// Runs `sql` on `file` with sqlite3 in ASCII mode: fields split by 0x1F, rows by 0x1E, the
/// header first, NULL written as NULL. Reads at most CAP bytes and kills sqlite3 at the
/// DEADLINE, so `SELECT *` on a huge table or a recursive CTE that never ends can't eat the
/// memory or hang a thread for good.
pub(super) fn query(file: &Path, sql: &str) -> Result<Grid, String> {
    run(file, sql, DEADLINE)
}

fn run(file: &Path, sql: &str, deadline: Duration) -> Result<Grid, String> {
    // SQL starting with "-" (a "--" comment line, say) would be read as an option; sqlite3 before
    // 3.41 has no "--" to end the options, and a leading space is nothing to SQL
    let sql = if sql.starts_with('-') { format!(" {sql}") } else { sql.to_string() };
    let mut child = command("sqlite3")
        // -ascii before -header: the other way round sqlite3 drops the header
        .args(["-readonly", "-safe", "-ascii", "-header", "-nullvalue", "NULL"])
        .arg(file)
        .arg(sql)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("sqlite3 não encontrado ({e}); instale o SQLite"))?;
    let (mut stdout, mut stderr) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let child = Arc::new(Mutex::new(child));
    let watchdog = Arc::clone(&child);
    // ponytail: a fixed deadline instead of a Parar button; add the button if 60 s proves short
    std::thread::spawn(move || {
        std::thread::sleep(deadline);
        let _ = watchdog.lock().unwrap().kill();
    });
    let mut bytes = Vec::new();
    let _ = stdout.by_ref().take(CAP as u64 + 1).read_to_end(&mut bytes);
    let more = bytes.len() > CAP;
    drop(stdout); // a child still writing gets SIGPIPE…
    let _ = child.lock().unwrap().kill(); // …and on Windows, this
    let mut err = String::new();
    let _ = stderr.read_to_string(&mut err);
    let ok = child.lock().unwrap().wait().is_ok_and(|s| s.success());
    if !ok && !more {
        let err = err.trim();
        return Err(if err.is_empty() { format!("Consulta interrompida: passou de {} s", deadline.as_secs()) } else { err.to_string() });
    }
    if more {
        bytes.truncate(bytes.iter().rposition(|&b| b == 0x1e).map_or(0, |i| i + 1)); // the cut row goes
    }
    Ok(Grid { more, ..parse(&String::from_utf8_lossy(&bytes)) })
}

// ponytail: sqlite3 from 3.50 on escapes 0x1E/0x1F inside values (^^ and ^_), so the split is
// exact; an older sqlite3 writes them raw and such a row breaks in two. -quote and -json would
// need a real parser (unistr(), no header on empty results).
pub(super) fn parse(text: &str) -> Grid {
    let mut rows = text.split('\x1e').filter(|r| !r.is_empty()).map(|r| r.split('\x1f').map(String::from).collect::<Vec<_>>());
    let columns = rows.next().unwrap_or_default();
    Grid { columns, rows: rows.collect(), more: false }
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
    /// The Database tool window: the project's databases and their tables. A double click on a
    /// table opens its rows in the editor.
    pub(super) fn db_panel(&mut self, ui: &mut Ui) {
        // ponytail: found once the project's file list is in, which is listed once when the
        // folder opens; a database created later needs the IDE node opened again
        if self.db.files.is_none() && self.files_rx.is_none() {
            let found: Vec<PathBuf> = self.files.iter().filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("sqlite" | "sqlite3" | "db")) && is_sqlite(p)).cloned().collect();
            if let Some(first) = found.first().cloned() {
                self.open_db(first);
            }
            self.db.files = Some(found);
        }
        let mut console = false;
        ui.horizontal(|ui| {
            ui.set_height(36.0);
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Banco de dados").color(t::TEXT).strong());
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                console = self.db.file.is_some() && ui.small_button("Console SQL").on_hover_text("Abre um console para este banco, numa aba").clicked();
            });
        });
        let note = |ui: &mut Ui, text: &str, color: Color32| {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.add(egui::Label::new(egui::RichText::new(text).color(color)).wrap());
            });
        };
        if self.db.files.as_ref().is_none_or(|f| f.is_empty()) {
            note(ui, "Nenhum banco SQLite neste projeto (.sqlite, .sqlite3 ou .db).", t::MUTED);
            return;
        }
        if let Some(error) = &self.db.error {
            note(ui, error, t::ERROR);
        }
        let (mut pick, mut show) = (None, None);
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for file in self.db.files.iter().flatten() {
                let on = self.db.file.as_ref() == Some(file);
                let (resp, rect) = row(ui, false, 24.0);
                let chevron = if on { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
                icons::paint(ui.painter(), chevron, Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(14.0, 14.0)));
                icons::at(ui.painter(), icons::FT_SQL, pos2(rect.left() + 33.0, rect.center().y));
                ui.painter().text(pos2(rect.left() + 46.0, rect.center().y), Align2::LEFT_CENTER, rel(&self.root, file), t::ui(t::UI_SIZE), t::TEXT);
                if resp.clicked() && !on {
                    pick = Some(file.clone());
                }
                if !on {
                    continue;
                }
                for (table, columns) in &self.db.tables {
                    let open = self.db.open.contains(table);
                    let (resp, rect) = row(ui, false, 24.0);
                    let x = rect.left() + 34.0;
                    let chevron = if open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
                    icons::paint(ui.painter(), chevron, Rect::from_center_size(pos2(x, rect.center().y), vec2(14.0, 14.0)));
                    table_icon(ui.painter(), pos2(x + 17.0, rect.center().y));
                    ui.painter().text(pos2(x + 30.0, rect.center().y), Align2::LEFT_CENTER, table, t::ui(t::UI_SIZE), t::TEXT);
                    let resp = resp.on_hover_text("Clique duplo abre os dados");
                    // the first click of a double click opens the columns too, as in PhpStorm's tree
                    if resp.double_clicked() {
                        show = Some(table.clone());
                    } else if resp.clicked() {
                        show = None;
                        if open { self.db.open.remove(table); } else { self.db.open.insert(table.clone()); }
                    }
                    if !self.db.open.contains(table) {
                        continue;
                    }
                    for (column, kind, pk) in columns {
                        let (_, rect) = row(ui, false, 22.0);
                        let x = rect.left() + 64.0;
                        let p = ui.painter();
                        if *pk {
                            p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, "PK", t::ui(10.0), t::WARNING);
                        }
                        let g = p.text(pos2(x + 22.0, rect.center().y), Align2::LEFT_CENTER, column, t::ui(t::UI_SIZE), t::TEXT);
                        p.text(pos2(g.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, kind.to_lowercase(), t::ui(t::UI_SIZE - 1.0), t::SECONDARY);
                    }
                }
            }
        });
        if let Some(file) = pick {
            self.open_db(file);
        }
        if let Some(table) = show {
            self.open_sheet(Some(table));
        }
        if console {
            self.open_sheet(None);
        }
    }

    /// Reads the schema; its error (no sqlite3, say) shows in the panel.
    fn open_db(&mut self, file: PathBuf) {
        // ponytail: one sqlite3 run on the UI thread per opening; a database with thousands of
        // tables would hitch once — move it to a thread like the queries if it shows
        let tables = schema(&file);
        self.db.error = tables.as_ref().err().cloned();
        self.db.tables = tables.unwrap_or_default();
        self.db.open.clear();
        self.db.file = Some(file);
    }

    /// A tab with `table`'s first rows (the tab it already has, if open), or a new console.
    pub(super) fn open_sheet(&mut self, table: Option<String>) {
        let Some(file) = self.db.file.clone() else { return };
        let open = self.tabs.iter().position(|t| t.path == file && table.is_some() && t.sheet.as_ref().is_some_and(|s| s.table == table));
        if let Some(i) = open {
            self.active = i;
            return;
        }
        let sql = table.as_ref().map(|t| format!("SELECT * FROM \"{}\" LIMIT {SHOWN};", t.replace('"', "\"\""))).unwrap_or_default();
        let mut sheet = Sheet { table, sql, result: None, running: None };
        if sheet.table.is_some() {
            sheet.run(&file, &self.ctx);
        }
        let mut tab = Tab::new(file, String::new(), false);
        tab.sheet = Some(Box::new(sheet));
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    /// The active sheet tab: its query on top (editable, ⌘Enter runs it) and the rows below.
    pub(super) fn sheet_view(&mut self, ui: &mut Ui) {
        let (ctx, id) = (self.ctx.clone(), self.id);
        let tab = &mut self.tabs[self.active];
        let Some(sheet) = tab.sheet.as_mut() else { return };
        if let Some(rx) = &sheet.running
            && let Ok(result) = rx.try_recv()
        {
            sheet.result = Some(result);
            sheet.running = None;
        }
        let mut run = false;
        Frame::NONE.inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
            // ⌘Enter is taken before the text box sees it, or the box swallows it
            let console_id = id.with(("db_console", &tab.path, &sheet.table));
            run = ui.memory(|m| m.has_focus(console_id)) && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
            ui.add(egui::TextEdit::multiline(&mut sheet.sql)
                .id(console_id)
                .font(t::mono(13.0))
                .code_editor()
                .desired_rows(if sheet.table.is_some() { 1 } else { 4 })
                .desired_width(f32::INFINITY)
                .hint_text("SELECT … (⌘Enter executa; o banco abre só para leitura)"));
            ui.horizontal(|ui| {
                run |= ui.button("▶ Executar").clicked();
                let note = match (&sheet.running, &sheet.result) {
                    (Some(_), _) => "Executando…".to_string(),
                    (_, Some(Ok(g))) if g.rows.len() > SHOWN => format!("{}{} linhas (mostrando {SHOWN})", if g.more { "mais de " } else { "" }, g.rows.len()),
                    (_, Some(Ok(g))) => format!("{} linha(s)", g.rows.len()),
                    _ => String::new(),
                };
                ui.label(egui::RichText::new(note).color(t::MUTED));
            });
        });
        match &sheet.result {
            Some(Err(error)) => {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(error).color(t::ERROR));
                });
            }
            Some(Ok(grid)) => grid_ui(ui, grid, id.with(("db_grid", &tab.path, &sheet.table))),
            None => {}
        }
        if run {
            sheet.run(&tab.path, &ctx);
        }
    }
}

/// PhpStorm's table node: a small grid.
fn table_icon(p: &egui::Painter, at: egui::Pos2) {
    let r = Rect::from_center_size(at, vec2(13.0, 11.0));
    let stroke = Stroke::new(1.0, Color32::from_rgb(0x8C, 0x9B, 0xB0));
    p.rect_stroke(r, 1.5, stroke, egui::StrokeKind::Inside);
    p.line_segment([pos2(r.left(), r.top() + 3.5), pos2(r.right(), r.top() + 3.5)], stroke);
    p.line_segment([pos2(r.center().x, r.top() + 3.5), pos2(r.center().x, r.bottom())], stroke);
}

/// The result as a table: a header row and at most SHOWN rows, numbered, long values cut.
fn grid_ui(ui: &mut Ui, grid: &Grid, salt: Id) {
    egui::ScrollArea::both().id_salt(salt).auto_shrink(false).show(ui, |ui| Frame::NONE.inner_margin(Margin::symmetric(12, 4)).show(ui, |ui| {
        egui::Grid::new(salt.with("rows")).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
            ui.label("");
            for column in &grid.columns {
                ui.label(egui::RichText::new(column).font(t::mono(12.5)).color(t::TEXT).strong());
            }
            ui.end_row();
            for (n, row) in grid.rows.iter().take(SHOWN).enumerate() {
                ui.label(egui::RichText::new((n + 1).to_string()).font(t::mono(12.0)).color(t::LINE_NUMBER));
                for value in row {
                    let short: String = value.chars().take(60).collect();
                    let color = if value == "NULL" { t::MUTED } else { t::TEXT };
                    ui.label(egui::RichText::new(short).font(t::mono(12.5)).color(color));
                }
                ui.end_row();
            }
        });
    }));
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
        // a comment first: sqlite3 must not take "--…" for an option
        assert_eq!(query(&file, "-- só os nomes\nSELECT name FROM users WHERE name IS NOT NULL").unwrap().rows, [["Ana"]]);
        // read-only and safe: no writing, no shell, no other file, no copy of the database
        assert!(query(&file, "DELETE FROM users").is_err());
        assert!(query(&file, ".shell echo oi").is_err());
        assert!(query(&file, &format!("ATTACH '{}' AS x; CREATE TABLE x.t(a)", dir.join("x.db").display())).is_err());
        assert!(query(&file, &format!("VACUUM INTO '{}'", dir.join("copia.db").display())).is_err());
        assert!(query(&file, &format!("SELECT writefile('{}', 'x')", dir.join("w.txt").display())).is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "só o banco na pasta");
        assert_eq!(query(&file, "SELECT count(*) AS n FROM users").unwrap().rows, [["2"]]);
        // a result past CAP comes cut, with whole rows, and a query past the deadline is killed
        let big = query(&file, "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c LIMIT 2000000) SELECT x, 'abcdefghij' FROM c").unwrap();
        assert!(big.more && big.rows.len() > SHOWN && big.rows.iter().all(|r| r.len() == 2));
        let endless = run(&file, "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT count(*) FROM c", Duration::from_millis(300));
        assert!(endless.is_err_and(|e| e.contains("interrompida")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
