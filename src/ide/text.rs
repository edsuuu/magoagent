//! Text helpers (char/line positions, words), go to declaration (language server first, then a
//! project scan) with Back/Forward, fuzzy file matching and the line edits (comment, duplicate).

use super::*;

pub(super) fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

pub(super) fn hash(s: &str, size: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    size.to_bits().hash(&mut h);
    h.finish()
}

pub(super) fn byte_at(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(b, _)| b)
}

pub(super) fn line_starts(s: &str) -> Vec<usize> {
    let mut v = vec![0];
    v.extend(s.chars().enumerate().filter(|(_, c)| *c == '\n').map(|(i, _)| i + 1));
    v
}

pub(super) fn line_of(s: &str, char_idx: usize) -> usize {
    s.chars().take(char_idx).filter(|c| *c == '\n').count()
}

/// Char index of (line, col), clamped to the line.
pub(super) fn char_at(s: &str, line: usize, col: usize) -> usize {
    let starts = line_starts(s);
    let total = s.chars().count();
    let Some(&start) = starts.get(line) else { return total };
    let end = starts.get(line + 1).map_or(total, |n| n - 1);
    start.saturating_add(col).min(end)
}

/// Identifier around char `c` (PHP `$vars` included), as a char range.
pub(super) fn word_at(s: &str, c: usize) -> (usize, usize) {
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
pub(super) fn family(id: &str) -> &str {
    match id {
        "blade" => "php",
        "javascriptreact" | "typescript" | "typescriptreact" => "javascript",
        "cpp" => "c",
        other => other,
    }
}

/// Native go-to-declaration: `<keyword> word` in the current file first, then the project.
pub(super) fn find_declaration(word: &str, current: &Path, files: &[PathBuf]) -> Option<(PathBuf, usize, usize)> {
    const DECL: &[&str] = &["function", "fn", "def", "func", "class", "interface", "trait", "enum", "struct", "type", "const", "macro_rules!", "record", "namespace", "let", "var"];
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
pub(super) fn framework_target(root: &Path, text: &str, c: usize) -> Option<(PathBuf, usize)> {
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
    let is_view_call = matches!(call, "view" | "View::make" | "@include" | "@extends" | "@component" | "@each" | "@includeIf" | "@includeWhen" | "make") || before.contains("->view(");
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

/// Lower = better. File-name hits beat path hits beat subsequence hits.
pub(super) fn fuzzy(rel: &str, q: &str) -> Option<usize> {
    if q.is_empty() {
        return Some(rel.len());
    }
    let lower = rel.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if name.starts_with(q) {
        return Some(lower.len());
    }
    if name.contains(q) {
        return Some(1_000 + lower.len());
    }
    if lower.contains(q) {
        return Some(2_000 + lower.len());
    }
    let mut it = lower.chars();
    q.chars().all(|c| it.any(|h| h == c)).then_some(3_000 + lower.len())
}

/// Ctrl+/ on the selected lines, VSCode style (at the indentation, with a space).
pub(super) fn toggle_comment(tab: &mut Tab) {
    let Some(prefix) = tab.lang.line.first() else { return };
    let (a, b) = tab.sel;
    let (l0, mut l1) = (line_of(&tab.text, a), line_of(&tab.text, b));
    if b > a && char_at(&tab.text, l1, 0) == b {
        l1 -= 1; // selection ends at column 0: leave that line alone
    }
    let mut lines: Vec<String> = tab.text.split('\n').map(String::from).collect();
    let range = l0..=l1.min(lines.len() - 1);
    let indent = lines[range.clone()].iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let all_commented = lines[range.clone()].iter().filter(|l| !l.trim().is_empty()).all(|l| l.trim_start().starts_with(prefix));
    for l in &mut lines[range] {
        if l.trim().is_empty() {
            continue;
        }
        if all_commented {
            let i = l.find(prefix).unwrap();
            let mut end = i + prefix.len();
            if l[end..].starts_with(' ') {
                end += 1;
            }
            l.replace_range(i..end, "");
        } else {
            l.insert_str(indent, &format!("{prefix} "));
        }
    }
    tab.text = lines.join("\n");
    if a == b {
        // PhpStorm: comment the line and move to the next one
        let c = char_at(&tab.text, l0 + 1, tab.cursor.1);
        tab.select = Some((c, c));
    } else {
        tab.select = Some((char_at(&tab.text, l0, 0), char_at(&tab.text, l1, usize::MAX)));
    }
    tab.dirty = true;
    tab.lsp_pending = Some(Instant::now());
}

/// Ctrl+D: duplicate the selection, or the current line.
pub(super) fn duplicate(tab: &mut Tab) {
    let (a, b) = tab.sel;
    if a != b {
        let (ba, bb) = (byte_at(&tab.text, a), byte_at(&tab.text, b));
        let piece = tab.text[ba..bb].to_string();
        tab.text.insert_str(bb, &piece);
        tab.select = Some((b, b + (b - a)));
    } else {
        let line = line_of(&tab.text, a);
        let (s, e) = (char_at(&tab.text, line, 0), char_at(&tab.text, line, usize::MAX));
        let (bs, be) = (byte_at(&tab.text, s), byte_at(&tab.text, e));
        let piece = format!("\n{}", &tab.text[bs..be]);
        tab.text.insert_str(be, &piece);
        let c = a + (e - s) + 1;
        tab.select = Some((c, c));
    }
    tab.dirty = true;
    tab.lsp_pending = Some(Instant::now());
}


impl Ide {
    pub(super) fn here(&self) -> Option<(PathBuf, usize, usize)> {
        self.tabs.get(self.active).map(|t| (t.path.clone(), t.cursor.0, t.cursor.1))
    }

    pub(super) fn jump_to(&mut self, path: PathBuf, line: usize, col: usize) {
        if let Some(here) = self.here() {
            self.back.push(here);
            self.forward.clear();
        }
        self.status.clear();
        self.open_file(path, Some((line, col)));
    }

    /// Ctrl+Click / Ctrl+B at char index `c` of the active tab:
    /// framework strings first (view/route/config/twig), then the language server, then a native search.
    pub(super) fn goto_declaration(&mut self, c: usize) {
        let Some(tab) = self.tabs.get(self.active) else { return };
        let line = line_of(&tab.text, c);
        let col = c - char_at(&tab.text, line, 0);
        if let Some(root) = &self.root
            && let Some((p, l)) = framework_target(root, &tab.text, c)
        {
            return self.jump_to(p, l, 0);
        }
        let (a, b) = word_at(&tab.text, c);
        let word = tab.text.chars().skip(a).take(b - a).collect::<String>().trim_start_matches('$').to_string();
        if word.is_empty() {
            return;
        }
        self.decl_word = Some(word);
        let (path, lang) = (tab.path.clone(), tab.lang);
        if let Some(insp) = &mut self.insp
            && insp.definition(&path, lang, line, col)
        {
            self.status = "Procurando a declaração…".into();
            return;
        }
        self.native_declaration();
    }

    /// Asks the language server about the word at the caret (usages, implementations…).
    pub(super) fn ask_at_cursor(&mut self, ask: lsp::Ask) {
        let Some(tab) = self.tabs.get(self.active) else { return };
        let (path, lang, (line, col)) = (tab.path.clone(), tab.lang, tab.cursor);
        let (a, b) = word_at(&tab.text, char_at(&tab.text, line, col));
        let word: String = tab.text.chars().skip(a).take(b - a).collect();
        if self.insp.as_mut().is_some_and(|insp| insp.ask(ask, &path, lang, line, col)) {
            self.usage_word = word;
            self.status = "Procurando…".into();
        } else {
            self.status = "Nenhum servidor de linguagem para este arquivo".into();
        }
    }

    /// The server's usages or implementations, listed in the Find tool window.
    pub(super) fn show_usages(&mut self, ask: lsp::Ask, places: Vec<(PathBuf, usize, usize)>) {
        let what = if ask == lsp::Ask::Usages { "Usos" } else { "Implementações" };
        if places.is_empty() {
            self.status = format!("{what} de '{}': nenhum", self.usage_word);
            return;
        }
        self.status.clear();
        let mut files: std::collections::HashMap<PathBuf, Vec<String>> = Default::default();
        let len = self.usage_word.chars().count();
        // ponytail: reads every file with a hit on the UI thread; a thread like native_declaration's if a wide symbol hitches
        let hits = places
            .into_iter()
            .map(|(path, line, col)| {
                let lines = files.entry(path.clone()).or_insert_with(|| {
                    // an open tab may be unsaved: the server saw that text, not the disk's
                    let text = self.tabs.iter().find(|t| t.path == path && t.is_text()).map(|t| t.text.clone());
                    text.unwrap_or_else(|| std::fs::read_to_string(&path).unwrap_or_default()).lines().map(String::from).collect()
                });
                find::Hit { text: lines.get(line).cloned().unwrap_or_default(), path, line, col, len }
            })
            .collect();
        self.find_results = Some((format!("{what} de {}", self.usage_word), hits));
        self.bottom = Some(ToolWindow::Find);
    }

    /// No language server (or it found nothing): scan the project for `function|class|fn|def… word`.
    pub(super) fn native_declaration(&mut self) {
        let (Some(word), Some(tab)) = (self.decl_word.clone(), self.tabs.get(self.active)) else { return };
        let fam = family(tab.lang.id);
        let files: Vec<PathBuf> = self.files.iter().filter(|p| fam == family(highlight::detect(p).id)).cloned().collect();
        let current = tab.path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(find_declaration(&word, &current, &files));
            ctx.request_repaint();
        });
        self.decl_rx = Some(rx);
        self.status = "Procurando a declaração…".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(text: &str, lang: &'static Lang) -> Tab {
        let mut t = Tab::new("x".into(), text.into(), false);
        t.lang = lang;
        t
    }

    #[test]
    fn comment_toggle_roundtrip() {
        let mut t = tab("fn a() {\n    x();\n    y();\n}", &highlight::RUST);
        t.sel = (10, 26);
        toggle_comment(&mut t);
        assert_eq!(t.text, "fn a() {\n    // x();\n    // y();\n}");
        t.sel = (10, 30);
        toggle_comment(&mut t);
        assert_eq!(t.text, "fn a() {\n    x();\n    y();\n}");
    }

    #[test]
    fn duplicate_line_and_positions() {
        let mut t = tab("ação\nb", &highlight::TEXT);
        t.sel = (2, 2);
        duplicate(&mut t);
        assert_eq!(t.text, "ação\nação\nb");
        assert_eq!(char_at(&t.text, 1, 2), 7);
        assert_eq!(line_of(&t.text, 7), 1);
    }

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

    #[test]
    fn fuzzy_ranks_names_first() {
        assert!(fuzzy("app/Models/User.php", "user") < fuzzy("app/user/Thing.php", "user"));
        assert!(fuzzy("app/Http/Controllers/UserController.php", "usrctl").is_some());
        assert!(fuzzy("a.php", "zz").is_none());
    }
}
