//! Embedded SQLite; no connection survives a request, and reads open the file read-only.
//! ATTACH is limited to zero (VACUUM INTO too), so no statement reaches another file; there is
//! no shell and no file functions, and load_extension() is never enabled.

use super::*;
use rusqlite::{Connection, OpenFlags, fallible_iterator::FallibleIterator, limits::Limit, types::ValueRef};

const CAP: usize = 8 << 20;
const DEADLINE: Duration = Duration::from_secs(60);
pub(super) const PAGE: usize = 500;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Cell {
    Null,
    Text(String),
    Blob(String),
}

impl Cell {
    pub(super) fn text(&self) -> &str {
        match self {
            Self::Null => "<null>",
            Self::Text(s) | Self::Blob(s) => s,
        }
    }
}

pub(super) struct Grid {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

#[derive(Clone, Default)]
pub(super) struct Column {
    pub name: String,
    pub kind: String,
    pub notnull: bool,
    pub pk: usize,
    pub generated: bool,
    pub auto_increment: bool,
    /// the DEFAULT, as written in the schema
    pub default: Option<String>,
    pub children: Vec<Column>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Dialect {
    #[default]
    Sqlite,
    Mysql,
    Postgres,
    Mongo,
    Redis,
}
impl Dialect {
    pub fn ident(self, name: &str) -> String {
        match self {
            Self::Mysql => mysql_sql::ident(name),
            Self::Sqlite | Self::Postgres | Self::Mongo | Self::Redis => ident(name),
        }
    }
    pub fn dml(self, table: &Table, rows: &[DataRow]) -> Result<String, String> {
        match self {
            Self::Sqlite => dml(table, rows),
            Self::Mysql => mysql_sql::dml(table, rows),
            Self::Postgres => postgres_sql::dml(table, rows),
            Self::Mongo | Self::Redis => Err("Use Enviar para aplicar as alterações.".into()),
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct Table {
    pub name: String,
    pub kind: String,
    pub schema: String,
    pub dialect: Dialect,
    pub materialized: bool,
    pub triggers: Vec<session::Object>,
    pub view: bool,
    pub columns: Vec<Column>,
    /// "column → table.column"
    pub foreign: Vec<String>,
    pub indexes: Vec<(String, String)>,
    pub without_rowid: bool,
}

#[derive(Clone, Debug)]
pub(super) enum Identity {
    Rowid(String),
    Primary(Vec<usize>),
    ReadOnly,
}

impl Table {
    pub(super) fn identity(&self) -> Identity {
        if self.view {
            return Identity::ReadOnly;
        }
        if self.dialect == Dialect::Sqlite && !self.without_rowid {
            for name in ["rowid", "_rowid_", "oid"] {
                if !self.columns.iter().any(|c| c.name.eq_ignore_ascii_case(name)) {
                    return Identity::Rowid(name.into());
                }
            }
        }
        let mut pk: Vec<_> = self.columns.iter().enumerate().filter(|(_, c)| c.pk > 0).collect();
        pk.sort_by_key(|(_, c)| c.pk);
        if !pk.is_empty() && pk.iter().all(|(_, c)| c.notnull || self.without_rowid || self.dialect != Dialect::Sqlite) {
            Identity::Primary(pk.into_iter().map(|(i, _)| i).collect())
        } else {
            Identity::ReadOnly
        }
    }
}

#[derive(Clone)]
pub(super) struct DataRow {
    pub original: Option<Vec<Cell>>,
    pub values: Vec<Cell>,
    pub rowid: Cell,
    pub deleted: bool,
}

impl DataRow {
    pub(super) fn pending(&self) -> bool {
        self.deleted || self.original.as_ref() != Some(&self.values)
    }
}

pub(super) struct Page {
    pub rows: Vec<DataRow>,
    pub count: usize,
    pub ttl: Option<i64>,
}

pub(super) fn ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn numeric(kind: &str) -> bool {
    let k = kind.to_ascii_uppercase();
    k.contains("INT") || (!k.contains("CHAR") && !k.contains("CLOB") && !k.contains("TEXT") && !k.contains("BLOB") && !k.is_empty())
}

// SQLite's decimal grammar, excluding NaN, infinity and expressions.
pub(super) fn number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - start;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - start;
    }
    if digits == 0 {
        return false;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == b.len() && s.parse::<f64>().is_ok_and(f64::is_finite)
}

fn literal(value: &Cell, kind: &str) -> String {
    match value {
        Cell::Null => "NULL".into(),
        Cell::Blob(hex) => format!("X'{hex}'"),
        Cell::Text(s) if numeric(kind) && number(s) => s.clone(),
        // SQL source cannot contain a NUL byte.
        Cell::Text(s) if s.contains('\0') => format!("CAST(X'{}' AS TEXT)", hex(s.as_bytes())),
        Cell::Text(s) => quoted(s),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

pub(super) fn dml(table: &Table, rows: &[DataRow]) -> Result<String, String> {
    let identity = table.identity();
    if matches!(identity, Identity::ReadOnly) {
        return Err("Tabela sem rowid ou chave primária segura; somente leitura.".into());
    }
    let mut statements = Vec::new();
    for row in rows.iter().filter(|r| r.pending()) {
        let target = ident(&table.name);
        if let Some(original) = &row.original {
            let predicate = match &identity {
                Identity::Rowid(name) => format!("{} = {}", ident(name), literal(&row.rowid, "INTEGER")),
                Identity::Primary(pk) => pk
                    .iter()
                    .map(|&i| format!("{} IS {}", ident(&table.columns[i].name), literal(&original[i], &table.columns[i].kind)))
                    .collect::<Vec<_>>()
                    .join(" AND "),
                Identity::ReadOnly => unreachable!(),
            };
            if row.deleted {
                statements.push(format!("DELETE FROM {target} WHERE {predicate};"));
            } else {
                let changes: Vec<_> = table
                    .columns
                    .iter()
                    .enumerate()
                    .filter(|(i, c)| !c.generated && row.values[*i] != original[*i])
                    .map(|(i, c)| format!("{} = {}", ident(&c.name), literal(&row.values[i], &c.kind)))
                    .collect();
                if !changes.is_empty() {
                    statements.push(format!("UPDATE {target} SET {} WHERE {predicate};", changes.join(", ")));
                }
            }
        } else if !row.deleted {
            let cols: Vec<_> = table.columns.iter().enumerate().filter(|(_, c)| !c.generated).collect();
            statements.push(format!(
                "INSERT INTO {target} ({}) VALUES ({});",
                cols.iter().map(|(_, c)| ident(&c.name)).collect::<Vec<_>>().join(", "),
                cols.iter().map(|(i, c)| literal(&row.values[*i], &c.kind)).collect::<Vec<_>>().join(", ")
            ));
        }
    }
    Ok(if statements.is_empty() { String::new() } else { format!("BEGIN IMMEDIATE;\n{}\nCOMMIT;", statements.join("\n")) })
}

pub(super) fn submit(file: &Path, table: &Table, rows: &[DataRow]) -> Result<(), String> {
    let sql = dml(table, rows)?;
    if !sql.is_empty() {
        run(file, &format!("PRAGMA foreign_keys=ON;\n{sql}"), true, DEADLINE)?;
    }
    Ok(())
}

pub(super) fn query(file: &Path, sql: &str) -> Result<Grid, String> {
    run(file, sql, false, DEADLINE)
}
pub(super) fn execute(file: &Path, sql: &str) -> Result<Grid, String> {
    run(file, sql, true, DEADLINE)
}

pub(super) fn run(file: &Path, sql: &str, write: bool, deadline: Duration) -> Result<Grid, String> {
    let flags = if write {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
    };
    let mut conn = Connection::open_with_flags(file, flags).map_err(|e| format!("Não foi possível abrir {}: {e}", file.display()))?;
    conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0).map_err(|e| e.to_string())?;
    let start = Instant::now();
    conn.progress_handler(1000, Some(move || start.elapsed() >= deadline)).map_err(|e| e.to_string())?;
    let mut batch = rusqlite::Batch::new(&conn, sql);
    let mut grid = Grid { columns: vec![], rows: vec![] };
    let mut size = 0usize;
    while let Some(mut stmt) = batch.next().map_err(|e| run_error(e, deadline))? {
        let columns: Vec<String> = stmt.column_names().map(|s| s.to_string()).collect();
        let mut rows = Vec::new();
        let mut result = stmt.raw_query();
        while let Some(row) = result.next().map_err(|e| run_error(e, deadline))? {
            let mut values = Vec::with_capacity(columns.len());
            for (i, name) in columns.iter().enumerate() {
                let value = match row.get_ref(i).map_err(|e| e.to_string())? {
                    ValueRef::Null => Cell::Null,
                    ValueRef::Integer(v) => Cell::Text(v.to_string()),
                    ValueRef::Real(v) => Cell::Text(serde_json::Number::from_f64(v).map_or_else(|| v.to_string(), |n| n.to_string())),
                    ValueRef::Text(v) => Cell::Text(String::from_utf8_lossy(v).into_owned()),
                    ValueRef::Blob(v) => Cell::Blob(hex(v)),
                };
                // Name plus value per cell, about what the CLI's JSON weighed; NULLs count too.
                size += name.len() + value.text().len();
                if size > CAP {
                    return Err("Resultado excedeu o limite de 8 MiB; refine a consulta.".into());
                }
                values.push(value);
            }
            rows.push(values);
        }
        // Like the CLI's JSON output, a statement without rows leaves the previous result.
        if !rows.is_empty() {
            grid = Grid { columns, rows };
        }
    }
    Ok(grid)
}

fn run_error(e: rusqlite::Error, deadline: Duration) -> String {
    if matches!(e.sqlite_error_code(), Some(rusqlite::ErrorCode::OperationInterrupted)) {
        format!("Consulta interrompida: passou de {} ms", deadline.as_millis())
    } else {
        e.to_string()
    }
}

/// Columns, foreign keys and index columns in one read-only request.
pub(super) fn schema(file: &Path) -> Result<Vec<Table>, String> {
    let sql = "SELECT 'T' tag, m.name t, '', '', '', '', p.wr, '' FROM sqlite_master m JOIN pragma_table_list p ON p.name=m.name AND p.schema='main' WHERE m.type='table' AND m.name NOT LIKE 'sqlite_%' \
        UNION ALL SELECT 'C', m.name, p.name, p.type, p.[notnull], p.pk, p.hidden, p.dflt_value FROM sqlite_master m JOIN pragma_table_xinfo(m.name) p WHERE m.type='table' AND p.hidden<>1 \
        UNION ALL SELECT 'F', m.name, p.[from], p.[table], p.[to], p.id, p.seq, '' FROM sqlite_master m JOIN pragma_foreign_key_list(m.name) p WHERE m.type='table' \
        UNION ALL SELECT 'I', m.name, p.name, x.name, p.[unique], x.seqno, '', '' FROM sqlite_master m JOIN pragma_index_list(m.name) p JOIN pragma_index_info(p.name) x WHERE m.type='table';";
    let grid = query(file, sql)?;
    let mut tables = Vec::new();
    for r in &grid.rows {
        if r[0].text() == "T" {
            tables.push(Table { name: r[1].text().into(), without_rowid: r[6].text() == "1", ..Default::default() });
        }
    }
    for r in &grid.rows {
        let Some(t) = tables.iter_mut().find(|t| t.name == r[1].text()) else { continue };
        match r[0].text() {
            "C" => t.columns.push(Column {
                name: r[2].text().into(),
                kind: r[3].text().into(),
                notnull: r[4].text() == "1",
                pk: r[5].text().parse().unwrap_or(0),
                generated: r[6].text() != "0",
                auto_increment: false,
                default: (r[7] != Cell::Null).then(|| r[7].text().to_string()),
                children: vec![],
            }),
            "F" => t.foreign.push(format!("{} → {}.{}", r[2].text(), r[3].text(), r[4].text())),
            "I" => {
                let name = r[2].text();
                let col = if r[3] == Cell::Null { "(expressão)" } else { r[3].text() };
                if let Some((_, cols)) = t.indexes.iter_mut().find(|(n, _)| n == name) {
                    cols.push_str(&format!(", {col}"));
                } else {
                    t.indexes.push((name.into(), col.into()));
                }
            }
            _ => {}
        }
    }
    tables.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tables)
}

fn from_where(table: &Table, filter: &str) -> String {
    format!("FROM {}{}", ident(&table.name), if filter.trim().is_empty() { String::new() } else { format!(" WHERE {filter}") })
}

// Encode table cells as hex so blobs and integer keys round-trip without JSON number loss.
fn encoded(name: &str) -> String {
    format!(
        "CASE WHEN {name} IS NULL THEN 'N' ELSE CASE typeof({name}) WHEN 'blob' THEN 'B' ELSE 'V' END || \
        hex(CASE typeof({name}) WHEN 'real' THEN quote({name}) ELSE CAST({name} AS BLOB) END) END"
    )
}

fn decoded(value: &Cell) -> Result<Cell, String> {
    let text = value.text();
    if text == "N" {
        return Ok(Cell::Null);
    }
    let data = text.get(1..).ok_or("Célula SQLite inválida")?;
    if text.starts_with('B') {
        return Ok(Cell::Blob(data.into()));
    }
    let bytes: Result<Vec<_>, _> = (0..data.len()).step_by(2).map(|i| u8::from_str_radix(&data[i..i + 2], 16)).collect();
    Ok(Cell::Text(String::from_utf8(bytes.map_err(|e| e.to_string())?).map_err(|_| "Texto não UTF-8; use o console para inspecionar como BLOB")?))
}

pub(super) fn load_page(file: &Path, table: &Table, filter: &str, order: &str, offset: usize) -> Result<Page, String> {
    let from = from_where(table, filter);
    // Count and page share one read snapshot, even when the page is empty.
    let mut cols: Vec<_> = table.columns.iter().enumerate().map(|(i, c)| format!("{} AS c{i}", encoded(&ident(&c.name)))).collect();
    let has_rowid = if let Identity::Rowid(name) = table.identity() {
        cols.push(format!("{} AS identity_value", encoded(&ident(&name))));
        true
    } else {
        false
    };
    let order = if order.trim().is_empty() { String::new() } else { format!(" ORDER BY {order}") };
    let sql = format!(
        "SELECT CAST(n AS TEXT), p.* FROM (SELECT count(*) AS n {from}) LEFT JOIN \
        (SELECT 1 AS present, {} {from}{order} LIMIT {PAGE} OFFSET {offset}) p ON 1",
        cols.join(", ")
    );
    let grid = query(file, &sql)?;
    let count = grid.rows.first().ok_or("Contagem ausente")?[0].text().parse().map_err(|_| "Contagem inválida")?;
    let rows = grid
        .rows
        .iter()
        .filter(|r| r[1] != Cell::Null)
        .map(|r| {
            let values: Result<Vec<_>, _> = r[2..2 + table.columns.len()].iter().map(decoded).collect();
            let values = values?;
            Ok(DataRow { rowid: if has_rowid { decoded(r.last().unwrap())? } else { Cell::Null }, original: Some(values.clone()), values, deleted: false })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Page { rows, count, ttl: None })
}
