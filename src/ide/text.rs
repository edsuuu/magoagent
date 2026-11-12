//! Text helpers (char/line positions, words), go to declaration (language server first, then a
//! project scan) with Back/Forward, fuzzy file matching and the line edits (comment, duplicate).

use super::*;

pub(super) fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

pub(crate) fn hash(s: &str, size: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    size.to_bits().hash(&mut h);
    h.finish()
}

pub(crate) fn byte_at(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(b, _)| b)
}

pub(super) fn line_starts(s: &str) -> Vec<usize> {
    let mut v = vec![0];
    v.extend(s.chars().enumerate().filter(|(_, c)| *c == '\n').map(|(i, _)| i + 1));
    v
}

pub(crate) fn line_of(s: &str, char_idx: usize) -> usize {
    s.chars().take(char_idx).filter(|c| *c == '\n').count()
}

/// Char index of (line, col), clamped to the line.
pub(crate) fn char_at(s: &str, line: usize, col: usize) -> usize {
    let starts = line_starts(s);
    let total = s.chars().count();
    let Some(&start) = starts.get(line) else { return total };
    let end = starts.get(line + 1).map_or(total, |n| n - 1);
    start.saturating_add(col).min(end)
}

/// Identifier around char `c` (PHP `$vars` included), as a char range.
pub(crate) fn word_at(s: &str, c: usize) -> (usize, usize) {
    let is_id = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    let line = line_of(s, c);
    let start = char_at(s, line, 0);
    let chars: Vec<char> = s.chars().skip(start).take_while(|ch| *ch != '\n').collect();
    let local = (c - start).min(chars.len());
    let mut a = local;
    while a > 0 && is_id(chars[a - 1]) {
        a -= 1;
    }
    let mut b = local;
    while b < chars.len() && is_id(chars[b]) {
        b += 1;
    }
    (start + a, start + b)
}

/// Languages that can reference each other's declarations.
pub(crate) fn family(id: &str) -> &str {
    match id {
        "blade" => "php",
        "javascriptreact" | "typescript" | "typescriptreact" => "javascript",
        "cpp" => "c",
        other => other,
    }
}

/// Native go-to-declaration: `<keyword> word` in the current file first, then the project.
pub(crate) fn find_declaration(word: &str, current: &Path, files: &[PathBuf]) -> Option<(PathBuf, usize, usize)> {
    const DECL: &[&str] = &[
        "function",
        "fn",
        "def",
        "func",
        "class",
        "interface",
        "trait",
        "enum",
        "struct",
        "type",
        "const",
        "macro_rules!",
        "record",
        "namespace",
        "let",
        "var",
    ];
    let is_id = |c: char| c.is_alphanumeric() || c == '_';
    for p in std::iter::once(current).chain(files.iter().map(|p| p.as_path()).filter(|p| *p != current)) {
        let Ok(text) = std::fs::read_to_string(p) else { continue };
        if !text.contains(word) {
            continue;
        }
        for (ln, line) in text.lines().enumerate() {
            let mut from = 0;
            while let Some(i) = line[from..].find(word) {
                let at = from + i;
                let end = at + word.len();
                from = end;
                if line[..at].ends_with(is_id) || line[end..].starts_with(is_id) {
                    continue;
                }
                let prev = line[..at].trim_end().rsplit(|c: char| c.is_whitespace()).next().unwrap_or("");
                // `let`/`var` only count in the current file (locals)
                if DECL.contains(&prev) && (p == current || !matches!(prev, "let" | "var")) {
                    return Some((p.to_path_buf(), ln, line[..at].chars().count()));
                }
            }
        }
    }
    None
}

/// Laravel / Symfony string navigation: view('a.b'), @include('a.b'), config('app.x'),
/// route('name'), and Twig template paths. Returns (file, line).
pub(crate) fn framework_target(root: &Path, text: &str, c: usize) -> Option<(PathBuf, usize)> {
    let line = line_of(text, c);
    let start = char_at(text, line, 0);
    let chars: Vec<char> = text.chars().skip(start).take_while(|ch| *ch != '\n').collect();
    let local = c - start;
    let q = |ch: char| ch == '\'' || ch == '"';
    let open = chars[..local.min(chars.len())].iter().rposition(|ch| q(*ch))?;
    let close = local + chars.get(local..)?.iter().position(|ch| q(*ch))?;
    let value: String = chars[open + 1..close].iter().collect();
    let before: String = chars[..open].iter().collect();
    let call = before.trim_end().trim_end_matches('(').rsplit(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '@' || ch == ':')).next().unwrap_or("");
    let views = root.join("resources/views");
    let is_view_call = matches!(call, "view" | "View::make" | "@include" | "@extends" | "@component" | "@each" | "@includeIf" | "@includeWhen" | "make")
        || before.contains("->view(");
    if is_view_call || (views.is_dir() && !value.contains('/') && value.contains('.')) {
        let rel = value.replace("::", "/").replace('.', "/");
        for cand in [views.join(format!("{rel}.blade.php")), views.join(format!("{rel}.php")), views.join(&rel).join("index.blade.php")] {
            if cand.is_file() {
                return Some((cand, 0));
            }
        }
    }
    if call == "config" || call == "Config::get" {
        let file = root.join("config").join(format!("{}.php", value.split('.').next()?));
        if file.is_file() {
            let key = value.split('.').nth(1).unwrap_or("");
            let src = std::fs::read_to_string(&file).ok()?;
            let l = src.lines().position(|l| l.contains(&format!("'{key}'"))).unwrap_or(0);
            return Some((file, l));
        }
    }
    if call == "route" || call == "to_route" {
        for e in std::fs::read_dir(root.join("routes")).ok()?.flatten() {
            let src = std::fs::read_to_string(e.path()).unwrap_or_default();
            if let Some(l) = src.lines().position(|l| l.contains(&format!("name('{value}')")) || l.contains(&format!("name(\"{value}\")"))) {
                return Some((e.path(), l));
            }
        }
    }
    if value.ends_with(".twig") {
        let file = root.join("templates").join(&value);
        if file.is_file() {
            return Some((file, 0));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_and_framework_strings() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("resources/views/users")).unwrap();
        std::fs::create_dir_all(dir.join("app")).unwrap();
        std::fs::write(dir.join("resources/views/users/show.blade.php"), "x").unwrap();
        std::fs::write(dir.join("app/Role.php"), "<?php\n\nenum RoleEnum: string\n{\n}").unwrap();
        let cur = dir.join("app/C.php");
        let src = "<?php\nreturn view('users.show');\n$r = RoleEnum::Admin;";
        std::fs::write(&cur, src).unwrap();
        let c = src.find("users.show").unwrap() + 2;
        assert_eq!(framework_target(&dir, src, c), Some((dir.join("resources/views/users/show.blade.php"), 0)));
        let c = src.find("RoleEnum").unwrap() + 1;
        let (a, b) = word_at(src, c);
        assert_eq!(&src[a..b], "RoleEnum");
        assert_eq!(find_declaration("RoleEnum", &cur, &[dir.join("app/Role.php")]), Some((dir.join("app/Role.php"), 2, 5)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
