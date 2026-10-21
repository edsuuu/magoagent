//! SQLite shell, introspection and pending mutations. No connection survives a request.
//! Every run passes `-safe` (no shell, no other file, no ATTACH, writefile() or load_extension());
//! reads add `-readonly`. What `-safe` leaves open is fsdir(), which lists what the person can
//! read anyway; blocking it would take an authorizer, i.e. linking SQLite instead of the shell.

use super::*;
use std::io::{Read, Write};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

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
        match self { Self::Null => "<null>", Self::Text(s) | Self::Blob(s) => s }
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
    /// the DEFAULT, as written in the schema
    pub default: Option<String>,
}

#[derive(Clone, Default)]
pub(super) struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    /// "column → table.column"
    pub foreign: Vec<String>,
    pub indexes: Vec<(String, String)>,
    pub without_rowid: bool,
}

#[derive(Clone, Debug)]
pub(super) enum Identity { Rowid(String), Primary(Vec<usize>), ReadOnly }

impl Table {
    pub(super) fn identity(&self) -> Identity {
        if !self.without_rowid {
            for name in ["rowid", "_rowid_", "oid"] {
                if !self.columns.iter().any(|c| c.name.eq_ignore_ascii_case(name)) {
                    return Identity::Rowid(name.into());
                }
            }
        }
        let mut pk: Vec<_> = self.columns.iter().enumerate().filter(|(_, c)| c.pk > 0).collect();
        pk.sort_by_key(|(_, c)| c.pk);
        if !pk.is_empty() && pk.iter().all(|(_, c)| c.notnull || self.without_rowid) {
            Identity::Primary(pk.into_iter().map(|(i, _)| i).collect())
        } else { Identity::ReadOnly }
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
}

pub(super) fn ident(s: &str) -> String { format!("\"{}\"", s.replace('"', "\"\"")) }
fn quoted(s: &str) -> String { format!("'{}'", s.replace('\'', "''")) }

fn numeric(kind: &str) -> bool {
    let k = kind.to_ascii_uppercase();
    k.contains("INT") || (!k.contains("CHAR") && !k.contains("CLOB") && !k.contains("TEXT") && !k.contains("BLOB") && !k.is_empty())
}

// SQLite's decimal grammar, excluding NaN, infinity and expressions.
fn number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() { i += 1; }
    let mut digits = i - start;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() { i += 1; }
        digits += i - start;
    }
    if digits == 0 { return false; }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) { i += 1; }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() { i += 1; }
        if i == start { return false; }
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

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02X}")).collect() }

pub(super) fn dml(table: &Table, rows: &[DataRow]) -> Result<String, String> {
    let identity = table.identity();
    if matches!(identity, Identity::ReadOnly) { return Err("Tabela sem rowid ou chave primária segura; somente leitura.".into()); }
    let mut statements = Vec::new();
    for row in rows.iter().filter(|r| r.pending()) {
        let target = ident(&table.name);
        if let Some(original) = &row.original {
            let predicate = match &identity {
                Identity::Rowid(name) => format!("{} = {}", ident(name), literal(&row.rowid, "INTEGER")),
                Identity::Primary(pk) => pk.iter().map(|&i| format!("{} IS {}", ident(&table.columns[i].name), literal(&original[i], &table.columns[i].kind))).collect::<Vec<_>>().join(" AND "),
                Identity::ReadOnly => unreachable!(),
            };
            if row.deleted {
                statements.push(format!("DELETE FROM {target} WHERE {predicate};"));
            } else {
                let changes: Vec<_> = table.columns.iter().enumerate().filter(|(i, c)| !c.generated && row.values[*i] != original[*i])
                    .map(|(i, c)| format!("{} = {}", ident(&c.name), literal(&row.values[i], &c.kind))).collect();
                if !changes.is_empty() { statements.push(format!("UPDATE {target} SET {} WHERE {predicate};", changes.join(", "))); }
            }
        } else if !row.deleted {
            let cols: Vec<_> = table.columns.iter().enumerate().filter(|(_, c)| !c.generated).collect();
            statements.push(format!("INSERT INTO {target} ({}) VALUES ({});", cols.iter().map(|(_, c)| ident(&c.name)).collect::<Vec<_>>().join(", "),
                cols.iter().map(|(i, c)| literal(&row.values[*i], &c.kind)).collect::<Vec<_>>().join(", ")));
        }
    }
    Ok(if statements.is_empty() { String::new() } else { format!("BEGIN IMMEDIATE;\n{}\nCOMMIT;", statements.join("\n")) })
}

pub(super) fn submit(file: &Path, table: &Table, rows: &[DataRow]) -> Result<(), String> {
    let sql = dml(table, rows)?;
    if !sql.is_empty() { run(file, &format!("PRAGMA foreign_keys=ON;\n{sql}"), true, DEADLINE)?; }
    Ok(())
}

pub(super) fn query(file: &Path, sql: &str) -> Result<Grid, String> { run(file, sql, false, DEADLINE) }
pub(super) fn execute(file: &Path, sql: &str) -> Result<Grid, String> { run(file, sql, true, DEADLINE) }

pub(super) fn run(file: &Path, sql: &str, write: bool, deadline: Duration) -> Result<Grid, String> {
    let mut cmd = command("sqlite3");
    if !write { cmd.arg("-readonly"); }
    // JSON preserves NULL, empty text and control characters without a sentinel collision.
    // -bail stops stdin at the first error; command-line SQL can continue to COMMIT.
    let mut child = cmd.args(["-safe", "-bail", "-json"]).arg(file)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
        .map_err(|e| format!("sqlite3 não encontrado ({e}); instale o SQLite"))?;
    let mut stdin = child.stdin.take().unwrap();
    let sql = format!("{sql}\n");
    let input = std::thread::spawn(move || stdin.write_all(sql.as_bytes()));
    let (stdout, stderr) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let child = Arc::new(Mutex::new(child));
    let reader = |pipe: Box<dyn Read + Send>, child: Arc<Mutex<std::process::Child>>| std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = pipe.take(CAP as u64 + 1).read_to_end(&mut bytes);
        if bytes.len() > CAP { let _ = child.lock().unwrap().kill(); }
        (bytes, result)
    });
    let out = reader(Box::new(stdout), child.clone());
    let err = reader(Box::new(stderr), child.clone());
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.lock().unwrap().try_wait().map_err(|e| e.to_string())? { break status; }
        if start.elapsed() >= deadline {
            let _ = child.lock().unwrap().kill();
            let _ = child.lock().unwrap().wait();
            let _ = out.join();
            let _ = err.join();
            let _ = input.join();
            return Err(format!("Consulta interrompida: passou de {} ms", deadline.as_millis()));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let (bytes, read) = out.join().map_err(|_| "Falha ao ler o resultado")?;
    let (errors, _) = err.join().map_err(|_| "Falha ao ler o erro")?;
    let written = input.join().map_err(|_| "Falha ao enviar SQL")?;
    read.map_err(|e| e.to_string())?;
    if bytes.len() > CAP || errors.len() > CAP { return Err("Resultado excedeu o limite de 8 MiB; refine a consulta.".into()); }
    if !status.success() { return Err(String::from_utf8_lossy(&errors).trim().to_string()); }
    written.map_err(|e| e.to_string())?;
    parse(&bytes)
}

// Preserve SELECT column order, including repeated names (serde_json::Map sorts its keys).
struct JsonRow(Vec<(String, serde_json::Value)>);
impl<'de> serde::Deserialize<'de> for JsonRow {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = JsonRow;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("uma linha SQL") }
            fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<JsonRow, M::Error> {
                let mut row = Vec::new();
                while let Some(pair) = map.next_entry()? { row.push(pair); }
                Ok(JsonRow(row))
            }
        }
        d.deserialize_map(Visitor)
    }
}

fn parse(bytes: &[u8]) -> Result<Grid, String> {
    let mut grid = Grid { columns: vec![], rows: vec![] };
    for result in serde_json::Deserializer::from_slice(bytes).into_iter::<Vec<JsonRow>>() {
        let rows = result.map_err(|e| format!("Resultado SQLite inválido: {e}"))?;
        grid.columns = rows.first().map(|r| r.0.iter().map(|(k, _)| k.clone()).collect()).unwrap_or_default();
        grid.rows = rows.into_iter().map(|r| r.0.into_iter().map(|(_, v)| match v {
            serde_json::Value::Null => Cell::Null,
            serde_json::Value::String(s) => Cell::Text(s),
            v => Cell::Text(v.to_string()),
        }).collect()).collect();
    }
    Ok(grid)
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
        if r[0].text() == "T" { tables.push(Table { name: r[1].text().into(), without_rowid: r[6].text() == "1", ..Default::default() }); }
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
                default: (r[7] != Cell::Null).then(|| r[7].text().to_string()),
            }),
            "F" => t.foreign.push(format!("{} → {}.{}", r[2].text(), r[3].text(), r[4].text())),
            "I" => {
                let name = r[2].text();
                let col = if r[3] == Cell::Null { "(expressão)" } else { r[3].text() };
                if let Some((_, cols)) = t.indexes.iter_mut().find(|(n, _)| n == name) { cols.push_str(&format!(", {col}")); }
                else { t.indexes.push((name.into(), col.into())); }
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
    format!("CASE WHEN {name} IS NULL THEN 'N' ELSE CASE typeof({name}) WHEN 'blob' THEN 'B' ELSE 'V' END || \
        hex(CASE typeof({name}) WHEN 'real' THEN quote({name}) ELSE CAST({name} AS BLOB) END) END")
}

fn decoded(value: &Cell) -> Result<Cell, String> {
    let text = value.text();
    if text == "N" { return Ok(Cell::Null); }
    let data = text.get(1..).ok_or("Célula SQLite inválida")?;
    if text.starts_with('B') { return Ok(Cell::Blob(data.into())); }
    let bytes: Result<Vec<_>, _> = (0..data.len()).step_by(2).map(|i| u8::from_str_radix(&data[i..i + 2], 16)).collect();
    Ok(Cell::Text(String::from_utf8(bytes.map_err(|e| e.to_string())?).map_err(|_| "Texto não UTF-8; use o console para inspecionar como BLOB")?))
}

pub(super) fn load_page(file: &Path, table: &Table, filter: &str, order: &str, offset: usize) -> Result<Page, String> {
    let from = from_where(table, filter);
    // Count and page share one read snapshot, even when the page is empty.
    let mut cols: Vec<_> = table.columns.iter().enumerate().map(|(i, c)| format!("{} AS c{i}", encoded(&ident(&c.name)))).collect();
    let has_rowid = if let Identity::Rowid(name) = table.identity() { cols.push(format!("{} AS identity_value", encoded(&ident(&name)))); true } else { false };
    let order = if order.trim().is_empty() { String::new() } else { format!(" ORDER BY {order}") };
    let sql = format!("SELECT CAST(n AS TEXT), p.* FROM (SELECT count(*) AS n {from}) LEFT JOIN \
        (SELECT 1 AS present, {} {from}{order} LIMIT {PAGE} OFFSET {offset}) p ON 1", cols.join(", "));
    let grid = query(file, &sql)?;
    let count = grid.rows.first().ok_or("Contagem ausente")?[0].text().parse().map_err(|_| "Contagem inválida")?;
    let rows = grid.rows.iter().filter(|r| r[1] != Cell::Null).map(|r| {
        let values: Result<Vec<_>, _> = r[2..2 + table.columns.len()].iter().map(decoded).collect();
        let values = values?;
        Ok(DataRow { rowid: if has_rowid { decoded(r.last().unwrap())? } else { Cell::Null }, original: Some(values.clone()), values, deleted: false })
    }).collect::<Result<Vec<_>, String>>()?;
    Ok(Page { rows, count })
}
