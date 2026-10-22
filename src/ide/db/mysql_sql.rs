//! MySQL names, literals and mutations; independent of the wire connection.
use super::*;

pub(super) fn ident(s: &str) -> String { format!("`{}`", s.replace('`', "``")) }
pub(super) fn target(t: &Table) -> String { format!("{}.{}", ident(&t.schema), ident(&t.name)) }

// Hex conversion keeps backslashes and control bytes independent of NO_BACKSLASH_ESCAPES.
pub(super) fn literal(v: &Cell) -> String {
    match v {
        Cell::Null => "NULL".into(),
        Cell::Blob(s) => format!("X'{s}'"),
        Cell::Text(s) if s.chars().any(|c| c == '\\' || c.is_control()) =>
            format!("CONVERT(X'{}' USING utf8mb4)", s.as_bytes().iter().map(|b| format!("{b:02X}")).collect::<String>()),
        Cell::Text(s) => format!("'{}'", s.replace('\'', "''")),
    }
}

fn value(v: &Cell, column: &Column) -> String {
    let kind = column.kind.split(['(', ' ']).next().unwrap_or("").to_ascii_uppercase();
    let numeric = matches!(kind.as_str(), "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" | "BIGINT" | "DECIMAL" | "NUMERIC" | "FLOAT" | "DOUBLE" | "REAL" | "YEAR");
    if let Cell::Text(s) = v {
        // Keep integer PK comparisons exact above 2^53; strings can coerce to DOUBLE.
        if numeric && sql::number(s) { return s.clone(); }
    }
    literal(v)
}

pub(super) fn from_where(t: &Table, filter: &str) -> String {
    format!("FROM {}{}", target(t), if filter.trim().is_empty() { String::new() } else { format!(" WHERE {filter}") })
}

pub(super) fn page_sql(t: &Table, filter: &str, order: &str, offset: usize) -> String {
    let columns = t.columns.iter().enumerate().map(|(i, c)| format!("{} AS `c{i}`", ident(&c.name))).collect::<Vec<_>>().join(", ");
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
            result.push(format!("INSERT INTO {target} ({}) VALUES ({});", cols.iter().map(|(_, c)| ident(&c.name)).collect::<Vec<_>>().join(", "),
                cols.iter().map(|(i, c)| value(&row.values[*i], c)).collect::<Vec<_>>().join(", ")));
        }
    }
    Ok(result)
}

pub(super) fn dml(t: &Table, rows: &[DataRow]) -> Result<String, String> {
    let statements = statements(t, rows)?;
    Ok(if statements.is_empty() { String::new() } else { format!("START TRANSACTION;\n{}\nCOMMIT;", statements.join("\n")) })
}
