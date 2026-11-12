use super::*;

pub(super) fn read_loop(key: &'static str, stdout: impl Read, tx: Sender<(&'static str, Value)>, ctx: egui::Context) {
    let mut r = BufReader::new(stdout);
    loop {
        let mut len = 0;
        loop {
            let mut line = String::new();
            if r.read_line(&mut line).unwrap_or(0) == 0 {
                let _ = tx.send((key, json!({"method": "$exit"})));
                return;
            }
            let line = line.trim();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length:") {
                len = v.trim().parse().unwrap_or(0);
            }
        }
        let mut buf = vec![0; len];
        if r.read_exact(&mut buf).is_err() {
            let _ = tx.send((key, json!({"method": "$exit"})));
            return;
        }
        if let Ok(v) = serde_json::from_slice(&buf) {
            let _ = tx.send((key, v));
            ctx.request_repaint();
        }
    }
}

/// Location | Location[] | LocationLink[] → (path, line, col) of each.
pub fn locations(r: &Value) -> Vec<(PathBuf, usize, usize)> {
    let one = |l: &Value| -> Option<(PathBuf, usize, usize)> {
        let target = l.get("targetUri").or(l.get("uri"))?.as_str()?;
        let start = &l.get("targetSelectionRange").or(l.get("range"))?["start"];
        Some((uri_path(target), start["line"].as_u64()? as usize, start["character"].as_u64()? as usize))
    };
    match r {
        Value::Array(all) => all.iter().filter_map(one).collect(),
        Value::Null => vec![],
        single => one(single).into_iter().collect(),
    }
}

/// A hover's contents (MarkupContent, MarkedString or a list of them) as Markdown: a MarkedString with a
/// language becomes a code block of it.
pub fn hover_text(r: &Value) -> Option<String> {
    let piece = |c: &Value| match (c.as_str(), c["language"].as_str()) {
        (Some(text), _) => Some(text.to_string()),
        (None, Some(lang)) => Some(format!("```{lang}\n{}\n```", c["value"].as_str().unwrap_or(""))),
        (None, None) => c["value"].as_str().map(str::to_string),
    };
    let text = match &r["contents"] {
        Value::Array(all) => all.iter().filter_map(piece).collect::<Vec<_>>().join("\n\n"),
        one => piece(one)?,
    };
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// The parameter hints of a textDocument/inlayHint answer (kind 2): (line, character, label).
pub fn inlay_hints(r: &Value) -> Vec<(usize, usize, String)> {
    r.as_array()
        .into_iter()
        .flatten()
        .filter(|h| h["kind"] == 2)
        .filter_map(|h| {
            let label = match &h["label"] {
                Value::String(s) => s.clone(),
                parts => parts.as_array()?.iter().filter_map(|p| p["value"].as_str()).collect(),
            };
            Some((h["position"]["line"].as_u64()? as usize, h["position"]["character"].as_u64()? as usize, label))
        })
        .collect()
}

/// The active signature's label and the byte range of its active parameter in it.
pub fn signature(r: &Value) -> Option<(String, Option<(usize, usize)>)> {
    let all = r["signatures"].as_array()?;
    let sig = all.get(r["activeSignature"].as_u64().unwrap_or(0) as usize).or(all.first())?;
    let label = sig["label"].as_str()?.to_string();
    let active = sig["activeParameter"].as_u64().or(r["activeParameter"].as_u64()).unwrap_or(0) as usize;
    let range = match &sig["parameters"][active]["label"] {
        // label offsets always count UTF-16 units, whatever the position encoding
        Value::Array(at) => at
            .first()
            .and_then(Value::as_u64)
            .zip(at.get(1).and_then(Value::as_u64))
            .map(|(a, b)| (utf16_byte(&label, a as usize), utf16_byte(&label, b as usize))),
        Value::String(name) => label.find(name.as_str()).map(|a| (a, a + name.len())),
        _ => None,
    }
    .filter(|&(a, b)| a <= b);
    Some((label, range))
}

/// Byte offset of the `n`th UTF-16 unit of `s` (clamped to its end).
pub(super) fn utf16_byte(s: &str, n: usize) -> usize {
    let mut units = 0;
    for (b, c) in s.char_indices() {
        if units >= n {
            return b;
        }
        units += c.len_utf16();
    }
    s.len()
}

pub(super) fn write(w: &mut impl Write, v: &Value) {
    let body = v.to_string();
    let _ = write!(w, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = w.flush();
}

/// Finds an executable on PATH (Windows: also .exe/.cmd/.bat, as npm installs .cmd shims).
pub fn which(name: &str) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
    std::env::split_paths(&std::env::var_os("PATH")?).flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}")))).find(|p| p.is_file())
}

pub fn uri(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = if s.starts_with('/') { s } else { format!("/{s}") };
    let mut out = String::from("file://");
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) { out.push(b as char) } else { out += &format!("%{b:02X}") }
    }
    out
}

pub fn uri_path(uri: &str) -> PathBuf {
    let s = uri.strip_prefix("file://").unwrap_or(uri).as_bytes();
    let mut bytes = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'%'
            && i + 2 < s.len()
            && let Ok(b) = u8::from_str_radix(std::str::from_utf8(&s[i + 1..i + 3]).unwrap_or(""), 16)
        {
            bytes.push(b);
            i += 3;
        } else {
            bytes.push(s[i]);
            i += 1;
        }
    }
    let s = String::from_utf8_lossy(&bytes).into_owned();
    // "/C:/x" -> "C:/x" on Windows
    let s = if cfg!(windows) && s.as_bytes().get(2) == Some(&b':') { s[1..].to_string() } else { s };
    PathBuf::from(s)
}

pub fn path_key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if cfg!(windows) { s.to_lowercase() } else { s }
}
