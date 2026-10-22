//! redis-cli style tokens and RESP result grids.
use super::*;
use ::redis::Value;

pub(super) fn parse(line: &str) -> Result<Vec<Vec<u8>>, String> {
    let chars: Vec<char> = line.chars().collect();
    let (mut i, mut args) = (0, vec![]);
    while i < chars.len() {
        if chars[i].is_whitespace() { i += 1; continue; }
        let (mut word, mut quote) = (Vec::new(), None);
        loop {
            if i == chars.len() { if quote.is_some() { return Err("Aspas não fechadas.".into()); } break; }
            let c = chars[i];
            if quote.is_none() && c.is_whitespace() { break; }
            if matches!(c, '\'' | '"') && (quote.is_none() || quote == Some(c)) {
                if quote.is_none() { quote = Some(c); } else {
                    quote = None;
                    if i + 1 < chars.len() && !chars[i + 1].is_whitespace() { return Err("Espaço esperado após as aspas.".into()); }
                }
                i += 1; continue;
            }
            if c == '\\' && quote == Some('"') {
                i += 1;
                let c = *chars.get(i).ok_or("Escape incompleto.")?;
                if c == 'x' {
                    let h: String = chars.get(i + 1..i + 3).ok_or("Escape hexadecimal incompleto.")?.iter().collect();
                    word.push(u8::from_str_radix(&h, 16).map_err(|_| "Escape hexadecimal inválido.")?); i += 3; continue;
                }
                match c { 'n' => word.push(b'\n'), 'r' => word.push(b'\r'), 't' => word.push(b'\t'), 'b' => word.push(8), 'a' => word.push(7), _ => word.extend(c.to_string().as_bytes()) }
            } else if c == '\\' && quote == Some('\'') && chars.get(i + 1) == Some(&'\'') {
                word.push(b'\''); i += 1;
            } else { word.extend(c.to_string().as_bytes()); }
            i += 1;
        }
        args.push(word);
    }
    Ok(args)
}
pub(super) fn cell(value: Value) -> Cell {
    match value {
        Value::Nil => Cell::Null,
        Value::BulkString(bytes) => match String::from_utf8(bytes) { Ok(s) => Cell::Text(s), Err(e) => Cell::Blob(e.as_bytes().iter().map(|b| format!("{b:02X}")).collect()) },
        Value::SimpleString(s) => Cell::Text(s), Value::Okay => Cell::Text("OK".into()),
        Value::Int(n) => Cell::Text(n.to_string()), Value::Double(n) => Cell::Text(n.to_string()), Value::Boolean(b) => Cell::Text(b.to_string()),
        Value::Array(a) | Value::Set(a) => Cell::Text(serde_json::Value::Array(a.into_iter().map(|v| serde_json::Value::String(cell(v).text().into())).collect()).to_string()),
        Value::Map(m) => Cell::Text(serde_json::Value::Object(m.into_iter().map(|(k, v)| (cell(k).text().into(), serde_json::Value::String(cell(v).text().into()))).collect()).to_string()),
        Value::Attribute { data, .. } => cell(*data),
        Value::VerbatimString { text, .. } => Cell::Text(text),
        v => Cell::Text(format!("{v:?}")),
    }
}
pub(super) fn grid(value: Value) -> Grid {
    match value {
        Value::Attribute { data, .. } => grid(*data),
        Value::Map(m) => Grid { columns: vec!["campo".into(), "valor".into()], rows: m.into_iter().map(|(k, v)| vec![cell(k), cell(v)]).collect() },
        Value::Array(a) | Value::Set(a) => Grid { columns: vec!["valor".into()], rows: a.into_iter().map(|v| vec![cell(v)]).collect() },
        v => Grid { columns: vec!["valor".into()], rows: vec![vec![cell(v)]] },
    }
}
pub(super) fn bytes(c: &Cell) -> Result<Vec<u8>, String> {
    match c {
        Cell::Blob(h) => (0..h.len()).step_by(2).map(|i| h.get(i..i + 2).ok_or("BLOB inválido".into()).and_then(|s| u8::from_str_radix(s, 16).map_err(|e| e.to_string()))).collect(),
        Cell::Null => Ok(vec![]), _ => Ok(c.text().as_bytes().to_vec()),
    }
}
