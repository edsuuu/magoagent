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
