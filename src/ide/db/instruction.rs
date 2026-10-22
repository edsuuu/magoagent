//! Statement boundaries shared by the console selection and PostgreSQL transport.
use super::*;

// Keep byte ranges: the editor selects characters, while the wire sends UTF-8.
pub(super) fn ranges(sql: &str, dialect: Dialect, no_backslash: bool) -> Vec<std::ops::Range<usize>> {
    let bytes = sql.as_bytes();
    let (mut start, mut i, mut quote, mut escapes) = (0, 0, 0, false);
    let mut result = vec![];
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if escapes && c == b'\\' { i += 2; continue; }
            if c == quote {
                if bytes.get(i + 1) == Some(&quote) && quote != b']' { i += 1; } else { quote = 0; }
            }
        } else if dialect == Dialect::Postgres && c == b'$' && (i == 0 || !word(bytes[i - 1])) {
            let end = bytes[i + 1..].iter().position(|&b| b == b'$').map(|n| i + 1 + n);
            if let Some(end) = end.filter(|&end| {
                let tag = &bytes[i + 1..end];
                tag.is_empty() || (!tag[0].is_ascii_digit() && tag.iter().all(|&b| word(b) && b != b'$'))
            }) {
                let delimiter = &sql[i..=end];
                i = sql[end + 1..].find(delimiter).map(|n| end + 1 + n + delimiter.len()).unwrap_or(bytes.len());
                continue;
            }
        } else if matches!(c, b'\'' | b'"') || (dialect != Dialect::Postgres && matches!(c, b'`' | b'[')) {
            quote = if c == b'[' { b']' } else { c };
            escapes = (dialect == Dialect::Mysql && !no_backslash && matches!(c, b'\'' | b'"')) ||
                (dialect == Dialect::Postgres && c == b'\'' && i > 0 && matches!(bytes[i - 1], b'e' | b'E') && (i < 2 || !word(bytes[i - 2])));
        } else if (dialect == Dialect::Mysql && c == b'#') || (bytes.get(i..i + 2) == Some(b"--") && (dialect != Dialect::Mysql || bytes.get(i + 2).is_none_or(|c| c.is_ascii_whitespace()))) {
            while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
        } else if bytes.get(i..i + 2) == Some(b"/*") {
            let mut depth = 1;
            i += 2;
            while i < bytes.len() && depth > 0 {
                if bytes.get(i..i + 2) == Some(b"/*") && dialect == Dialect::Postgres { depth += 1; i += 2; }
                else if bytes.get(i..i + 2) == Some(b"*/") { depth -= 1; i += 2; }
                else { i += 1; }
            }
            continue;
        } else if c == b';' { result.push(start..i + 1); start = i + 1; }
        i += 1;
    }
    if !sql[start..].trim().is_empty() { result.push(start..sql.len()); }
    result
}
fn word(b: u8) -> bool { b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$') || b >= 128 }

// Transaction commands may start after comments, including nested block comments.
pub(super) fn words(sql: &str) -> Vec<String> {
    let mut s = sql.trim_start();
    loop {
        if s.starts_with("--") { s = s.split_once('\n').map(|(_, tail)| tail).unwrap_or("").trim_start(); }
        else if s.starts_with("/*") {
            let (mut i, mut depth) = (2, 1);
            while i < s.len() && depth > 0 {
                if s.as_bytes().get(i..i + 2) == Some(b"/*") { depth += 1; i += 2; }
                else if s.as_bytes().get(i..i + 2) == Some(b"*/") { depth -= 1; i += 2; }
                else { i += 1; }
            }
            s = s[i..].trim_start();
        } else { break; }
    }
    s.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').filter(|s| !s.is_empty()).take(8).map(str::to_ascii_uppercase).collect()
}

#[cfg(test)]
pub(super) fn instruction(sql: &str, selection: (usize, usize)) -> String { instruction_mode(sql, selection, Dialect::Sqlite, false) }
pub(super) fn instruction_mode(sql: &str, selection: (usize, usize), dialect: Dialect, no_backslash: bool) -> String {
    let (a, b) = (byte_at(sql, selection.0), byte_at(sql, selection.1));
    if a != b { return sql[a..b].trim().into(); }
    if dialect == Dialect::Redis {
        let start = sql[..a].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let end = sql[a..].find('\n').map(|i| a + i).unwrap_or(sql.len());
        return sql[start..end].trim().into();
    }
    let ranges = if dialect == Dialect::Mongo { mongo_console::ranges(sql) } else { ranges(sql, dialect, no_backslash) };
    ranges.iter().find(|r| if dialect == Dialect::Mongo { a < r.end } else { a <= r.end }).or(ranges.last()).map(|r| sql[r.clone()].trim().into()).unwrap_or_default()
}
