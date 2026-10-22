//! PostgreSQL names, text literals and primary-key mutations.
use super::*;

pub(super) fn ident(s: &str) -> String { sql::ident(s) }
pub(super) fn target(t: &Table) -> String { format!("{}.{}", ident(&t.schema), ident(&t.name)) }

pub(super) fn literal(v: &Cell) -> String {
    match v {
        Cell::Null => "NULL".into(),
        Cell::Blob(s) => format!("'\\x{s}'::bytea"),
        Cell::Text(s) if s.chars().any(|c| c == '\\' || c.is_control()) => {
            let mut out = String::from("E'");
            for c in s.chars() {
                match c {
                    '\\' => out.push_str("\\\\"), '\'' => out.push_str("''"),
                    c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
                    c => out.push(c),
                }
            }
            out.push('\''); out
        }
        Cell::Text(s) => format!("'{}'", s.replace('\'', "''")),
    }
}

fn value(v: &Cell, column: &Column) -> String {
    let kind = column.kind.split(['(', ' ']).next().unwrap_or("").to_ascii_uppercase();
    let numeric = matches!(kind.as_str(), "SMALLINT" | "INTEGER" | "BIGINT" | "NUMERIC" | "DECIMAL" | "REAL" | "DOUBLE" | "SMALLSERIAL" | "SERIAL" | "BIGSERIAL");
    if let Cell::Text(s) = v {
        // Keep numeric keys exact, including bigint above 2^53.
        if numeric && sql::number(s) { return s.clone(); }
    }
    literal(v)
}

pub(super) fn from_where(t: &Table, filter: &str) -> String {
    format!("FROM {}{}", target(t), if filter.trim().is_empty() { String::new() } else { format!(" WHERE {filter}") })
}

pub(super) fn page_sql(t: &Table, filter: &str, order: &str, offset: usize) -> String {
    let columns = t.columns.iter().enumerate().map(|(i, c)| format!("{} AS \"c{i}\"", ident(&c.name))).collect::<Vec<_>>().join(", ");
    let order = if order.trim().is_empty() { String::new() } else { format!(" ORDER BY {order}") };
    format!("SELECT {columns} {}{order} LIMIT {PAGE} OFFSET {offset}", from_where(t, filter))
}

pub(super) fn statements(t: &Table, rows: &[DataRow]) -> Result<Vec<String>, String> {
    let Identity::Primary(pk) = t.identity() else { return Err("Tabela sem PRIMARY KEY; somente leitura.".into()) };
    let mut result = vec![];
    for row in rows.iter().filter(|r| r.pending()) {
        let target = target(t);
        if let Some(original) = &row.original {
            let predicate = pk.iter().map(|&i| format!("{} = {}", ident(&t.columns[i].name), value(&original[i], &t.columns[i]))).collect::<Vec<_>>().join(" AND ");
            if row.deleted { result.push(format!("DELETE FROM {target} WHERE {predicate};")); }
            else {
                let changes = t.columns.iter().enumerate().filter(|(i, c)| !c.generated && row.values[*i] != original[*i])
                    .map(|(i, c)| format!("{} = {}", ident(&c.name), value(&row.values[i], c))).collect::<Vec<_>>();
                if !changes.is_empty() { result.push(format!("UPDATE {target} SET {} WHERE {predicate};", changes.join(", "))); }
            }
        } else if !row.deleted {
            let cols = t.columns.iter().enumerate().filter(|(i, c)| !c.generated && !(c.auto_increment && row.values[*i] == Cell::Null)).collect::<Vec<_>>();
            if cols.is_empty() { result.push(format!("INSERT INTO {target} DEFAULT VALUES;")); }
            else { result.push(format!("INSERT INTO {target} ({}) VALUES ({});", cols.iter().map(|(_, c)| ident(&c.name)).collect::<Vec<_>>().join(", "),
                cols.iter().map(|(i, c)| value(&row.values[*i], c)).collect::<Vec<_>>().join(", "))); }
        }
    }
    Ok(result)
}

pub(super) fn dml(t: &Table, rows: &[DataRow]) -> Result<String, String> {
    let statements = statements(t, rows)?;
    Ok(if statements.is_empty() { String::new() } else { format!("BEGIN;\n{}\nCOMMIT;", statements.join("\n")) })
}
