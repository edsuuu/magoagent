//! The problems of a file tree node's code, as the reference IDE keeps them (HighlightInfo and the problem solver): what a
//! language server publishes (the IDE's LSP client, `ide/lsp.rs`, started per tree on its root) joined with a local syntax
//! check for the languages that have no server installed (brackets, quotes and comments left open, invalid JSON). The
//! editor draws them (filetree_problems_ui.rs), the tree marks the files and folders that hold errors, and the MCP lists
//! them (`file_tree` `problems`).

use crate::ide::highlight::{self, Kind, Lang};
use crate::ide::lsp::{Diag, Inspections};
use crate::ide::text::hash;
use crate::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// LSP severities: 1 error, 2 warning. Hints and infos are not kept (the reference's weak warnings).
pub(crate) const ERROR: u8 = 1;
pub(crate) const WARNING: u8 = 2;
/// The biggest text the local check reads, and the most problems it reports in one file.
const CHECK_MAX: usize = 1 << 20;
const CHECK_CAP: usize = 20;
/// The most files and the biggest file the project scan of the MCP reads.
const SCAN_FILES: usize = 4000;
const SCAN_MAX: u64 = 256 << 10;
/// The languages the local check knows well enough not to cry wolf (shell, ruby, lua, sql, asm and the markups have
/// quotes and brackets of their own: heredocs, `%w()`, long strings…).
const CHECKED: [&str; 17] = [
    "php",
    "css",
    "javascript",
    "javascriptreact",
    "typescript",
    "typescriptreact",
    "json",
    "rust",
    "go",
    "python",
    "c",
    "cpp",
    "csharp",
    "java",
    "kotlin",
    "swift",
    "dart",
];

/// Errors and warnings in `diags`.
pub(crate) fn counts(diags: &[Diag]) -> (usize, usize) {
    (diags.iter().filter(|d| d.severity == ERROR).count(), diags.iter().filter(|d| d.severity == WARNING).count())
}

/// What a language server said, plus the local check's problems on the lines it did not flag as errors, by place. Only
/// errors and warnings.
pub(crate) fn merge(lsp: Vec<Diag>, local: &[Diag]) -> Vec<Diag> {
    let flagged: HashSet<usize> = lsp.iter().filter(|d| d.severity == ERROR).map(|d| d.line).collect();
    let mut all: Vec<Diag> = lsp.into_iter().chain(local.iter().filter(|d| !flagged.contains(&d.line)).cloned()).filter(|d| d.severity <= WARNING).collect();
    all.sort_by_key(|d| (d.line, d.col));
    all
}

/// F2 / Shift+F2: the problem after (before) the caret at (line, col), going round to the first (last) when there is none
/// further, as the reference's GotoNextErrorHandler.
pub(crate) fn next_problem(diags: &[Diag], at: (usize, usize), forward: bool) -> Option<&Diag> {
    let place = |d: &&Diag| (d.line, d.col);
    if forward {
        diags.iter().filter(|d| place(d) > at).min_by_key(place).or_else(|| diags.iter().min_by_key(place))
    } else {
        diags.iter().filter(|d| place(d) < at).max_by_key(place).or_else(|| diags.iter().max_by_key(place))
    }
}

/// The files with errors and every folder over them up to the tree's root: what the tree draws in red.
pub(crate) fn propagate(files: &[PathBuf], root: &Path) -> HashSet<PathBuf> {
    let mut out = HashSet::new();
    for file in files.iter().filter(|f| f.starts_with(root)) {
        for p in file.ancestors().take_while(|p| p.starts_with(root)) {
            if !out.insert(p.to_path_buf()) {
                break;
            }
        }
    }
    out
}

/// The problems of every text tab of one editor, kept in step with its text: the language servers' (started on the tree's
/// root) and the local check's.
pub(crate) struct Problems {
    ctx: Context,
    root: PathBuf,
    lsp: Option<Inspections>,
    /// Per open text: the hash of the text the check ran on, and what it found.
    local: HashMap<PathBuf, (u64, Vec<Diag>)>,
    langs: HashMap<PathBuf, &'static Lang>,
}

impl Problems {
    pub(crate) fn new(ctx: Context) -> Self {
        Problems { ctx, root: PathBuf::new(), lsp: None, local: HashMap::new(), langs: HashMap::new() }
    }

    /// Frame by frame: the servers' news, then each text that changed checked and sent to its server; texts that closed are
    /// told to it.
    pub(crate) fn sync<'a>(&mut self, root: &Path, texts: impl Iterator<Item = (&'a Path, &'a str, &'static Lang)>) {
        if root != self.root {
            *self = Problems::new(self.ctx.clone());
            self.root = root.to_path_buf();
        }
        // ponytail: no server under test; and none for a root that is not a folder
        if self.lsp.is_none() && !cfg!(test) && root.is_dir() {
            self.lsp = Some(Inspections::new(self.ctx.clone(), root.to_path_buf()));
        }
        if let Some(lsp) = &mut self.lsp {
            lsp.poll();
        }
        let mut alive = HashSet::new();
        for (path, text, lang) in texts {
            alive.insert(path.to_path_buf());
            let h = hash(text, 0.0);
            if self.local.get(path).is_some_and(|(old, _)| *old == h) {
                continue;
            }
            let first = !self.local.contains_key(path);
            self.local.insert(path.to_path_buf(), (h, check(path, lang, text)));
            self.langs.insert(path.to_path_buf(), lang);
            if let Some(lsp) = self.lsp.as_mut().filter(|_| lang.kind == Kind::Code && !lang.servers.is_empty()) {
                if first { lsp.open(path, lang, text) } else { lsp.change(path, lang, text) }
            }
        }
        let gone: Vec<PathBuf> = self.local.keys().filter(|p| !alive.contains(*p)).cloned().collect();
        for path in gone {
            self.local.remove(&path);
            if let (Some(lsp), Some(lang)) = (self.lsp.as_mut(), self.langs.remove(&path)) {
                lsp.close(&path, lang);
            }
        }
    }

    /// `path` was written: the server hears of it (PHPStan runs then).
    pub(crate) fn saved(&mut self, path: &Path) {
        if let (Some(lsp), Some(lang)) = (self.lsp.as_mut(), self.langs.get(path)) {
            lsp.save(path, lang);
        }
    }

    /// The problems of `path`, by place.
    pub(crate) fn for_file(&self, path: &Path) -> Vec<Diag> {
        let lsp = self.lsp.as_ref().map(|l| l.for_file(path).into_iter().cloned().collect()).unwrap_or_default();
        merge(lsp, self.local.get(path).map_or(&[][..], |(_, d)| d))
    }

    /// The problems of `path` as it is now: `text` (its open tab's, or what is on disk) checked right away.
    pub(crate) fn for_text(&self, path: &Path, lang: &Lang, text: &str) -> Vec<Diag> {
        let lsp = self.lsp.as_ref().map(|l| l.for_file(path).into_iter().cloned().collect()).unwrap_or_default();
        merge(lsp, &check(path, lang, text))
    }

    /// Every file a problem is known of: the open texts and what the servers published.
    pub(crate) fn files(&self) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        let published = self.lsp.iter().flat_map(|l| l.all().map(|(p, _)| p.clone()));
        self.local.keys().cloned().chain(published).filter(|p| seen.insert(p.clone())).collect()
    }

    /// The files with an error, for the tree.
    pub(crate) fn error_files(&self) -> Vec<PathBuf> {
        self.files().into_iter().filter(|p| self.for_file(p).iter().any(|d| d.severity == ERROR)).collect()
    }
}

/// The local syntax check of `text`: only the languages in `CHECKED`.
pub(crate) fn check(path: &Path, lang: &Lang, text: &str) -> Vec<Diag> {
    if text.len() > CHECK_MAX || !CHECKED.contains(&lang.id) || text.trim().is_empty() {
        return vec![];
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if lang.id == "json" {
        // a .lock is not always JSON (Cargo.lock, yarn.lock)
        if ext == "lock" {
            return vec![];
        }
        let (found, comments) = Scan::run(lang, text);
        if comments || ext == "jsonc" {
            return found;
        }
        return json_error(text).into_iter().collect();
    }
    Scan::run(lang, text).0
}

fn diag(line: usize, col: usize, end_col: usize, message: String) -> Diag {
    Diag { line, col, end_line: line, end_col, severity: ERROR, message, source: "sintaxe".into() }
}

/// serde_json's complaint about `text`, in Portuguese, where it found it.
fn json_error(text: &str) -> Option<Diag> {
    let e = serde_json::from_str::<serde_json::Value>(text).err()?;
    let said = e.to_string();
    let message = [
        ("trailing comma", "Vírgula sobrando antes do fechamento"),
        ("EOF while parsing", "O JSON termina antes de fechar tudo"),
        ("expected `,` or", "Esperava vírgula ou o fechamento"),
        ("expected value", "Esperava um valor"),
        ("expected `:`", "Esperava dois-pontos depois da chave"),
        ("key must be a string", "A chave precisa ser uma string entre aspas"),
        ("trailing characters", "Texto sobrando depois do JSON"),
    ]
    .iter()
    .find(|(en, _)| said.contains(en))
    .map_or("JSON inválido", |(_, pt)| pt);
    let line = e.line().saturating_sub(1);
    let row = text.lines().nth(line).unwrap_or_default();
    // serde counts bytes in a line
    let col = row.char_indices().take_while(|(b, _)| *b < e.column().saturating_sub(1)).count();
    Some(diag(line, col, col + 1, message.into()))
}

/// One pass over the text, aware of comments, strings, regex literals and heredocs: counts the brackets and reports what is
/// left open or closed without opening.
struct Scan<'a> {
    c: Vec<char>,
    lang: &'a Lang,
    /// Char index where each line starts.
    starts: Vec<usize>,
    /// (start, end, message)
    found: Vec<(usize, usize, String)>,
    comments: bool,
    stack: Vec<(char, usize)>,
}

impl<'a> Scan<'a> {
    fn run(lang: &'a Lang, text: &str) -> (Vec<Diag>, bool) {
        let c: Vec<char> = text.chars().collect();
        let starts: Vec<usize> = std::iter::once(0).chain(c.iter().enumerate().filter(|(_, ch)| **ch == '\n').map(|(i, _)| i + 1)).collect();
        let mut s = Scan { c, lang, starts, found: vec![], comments: false, stack: vec![] };
        if lang.id == "php" {
            s.php();
        } else {
            s.code(0, false);
        }
        for (open, at) in std::mem::take(&mut s.stack) {
            s.found.push((at, at + 1, format!("'{open}' sem o '{}' correspondente", closer(open))));
        }
        s.found.sort();
        let out = s.found.iter().take(CHECK_CAP).map(|(a, b, m)| s.diag(*a, *b, m)).collect();
        (out, s.comments)
    }

    fn diag(&self, a: usize, b: usize, message: &str) -> Diag {
        let line = self.starts.partition_point(|s| *s <= a) - 1;
        let col = a - self.starts[line];
        let line_end = self.starts.get(line + 1).map_or(self.c.len(), |n| n - 1);
        diag(line, col, (b.min(line_end) - self.starts[line]).max(col + 1), message.to_string())
    }

    fn at(&self, i: usize, s: &str) -> bool {
        s.chars().enumerate().all(|(k, ch)| self.c.get(i + k) == Some(&ch))
    }

    /// PHP: the text outside `<?php … ?>` is HTML, left alone.
    fn php(&mut self) {
        let mut i = 0;
        while i < self.c.len() {
            match (i..self.c.len()).find(|&k| self.at(k, "<?php") || self.at(k, "<?=")) {
                Some(open) => i = self.code(open + if self.at(open, "<?=") { 3 } else { 5 }, true),
                None => break,
            }
        }
    }

    /// Scans code from `i`; back: where it ended (past a PHP `?>`, or the end of the text).
    fn code(&mut self, mut i: usize, php: bool) -> usize {
        let (id, n) = (self.lang.id, self.c.len());
        let js = matches!(id, "javascript" | "javascriptreact" | "typescript" | "typescriptreact");
        while i < n {
            let ch = self.c[i];
            if php && self.at(i, "?>") {
                return i + 2;
            }
            // comments (`http://` is not one, `#[` is a PHP attribute)
            if let Some(lc) =
                self.lang.line.iter().find(|p| self.at(i, p)).filter(|p| !(**p == "//" && i > 0 && self.c[i - 1] == ':') && !(**p == "#" && self.at(i, "#[")))
            {
                self.comments = true;
                let _ = lc;
                i = (i..n).find(|&k| self.c[k] == '\n').unwrap_or(n);
                continue;
            }
            if let Some((open, close)) = self.lang.block.filter(|(o, _)| self.at(i, o)) {
                self.comments = true;
                i = self.block_comment(i, open, close);
                continue;
            }
            if id == "php" && self.at(i, "<<<") {
                i = self.heredoc(i);
                continue;
            }
            if id == "rust"
                && (ch == 'r' || (ch == 'b' && self.at(i + 1, "r")))
                && (i == 0 || !is_ident(self.c[i - 1]))
                && let Some(end) = self.raw_string(i)
            {
                i = end;
                continue;
            }
            if self.lang.quotes.contains(ch) {
                // Rust's lifetime `'a`, and C++'s digit separator `1'000`
                let lifetime = id == "rust" && ch == '\'' && self.c.get(i + 1).is_some_and(|c| c.is_alphabetic() || *c == '_') && {
                    let end = (i + 1..n).find(|&k| !is_ident(self.c[k])).unwrap_or(n);
                    self.c.get(end) != Some(&'\'')
                };
                let separator = matches!(id, "c" | "cpp") && ch == '\'' && i > 0 && self.c[i - 1].is_ascii_alphanumeric();
                if lifetime || separator {
                    i += 1;
                } else {
                    i = self.string(i, ch);
                }
                continue;
            }
            if js
                && ch == '/'
                && self.regex_allowed(i)
                && let Some(end) = self.regex(i)
            {
                i = end;
                continue;
            }
            match ch {
                '(' | '[' | '{' => self.stack.push((ch, i)),
                ')' | ']' | '}' => self.close(ch, i),
                _ => {}
            }
            i += 1;
        }
        n
    }

    /// A closer: it closes the nearest opener of its kind, and the openers above that one stay open (a mistake); with none to
    /// close it is out of place.
    fn close(&mut self, ch: char, i: usize) {
        match self.stack.iter().rposition(|(open, _)| closer(*open) == ch) {
            Some(p) => {
                for (open, at) in self.stack.drain(p..).skip(1).collect::<Vec<_>>() {
                    self.found.push((at, at + 1, format!("'{open}' sem o '{}' correspondente", closer(open))));
                }
            }
            None => self.found.push((i, i + 1, format!("'{ch}' sem o '{}' correspondente", opener(ch)))),
        }
    }

    fn block_comment(&mut self, i: usize, open: &str, close: &str) -> usize {
        let n = self.c.len();
        // Rust's comments nest
        let (mut depth, mut k) = (1, i + open.len());
        while k < n {
            if self.at(k, close) {
                depth -= 1;
                k += close.len();
                if depth == 0 || self.lang.id != "rust" {
                    return k;
                }
            } else if self.lang.id == "rust" && self.at(k, open) {
                depth += 1;
                k += open.len();
            } else {
                k += 1;
            }
        }
        self.found.push((i, i + open.len(), "Comentário não fechado".into()));
        n
    }

    /// A string opened at `i` with `q`; back: where it ends. Interpolations (`${…}`) are scanned as code, so their quotes
    /// and brackets do not end it.
    fn string(&mut self, i: usize, q: char) -> usize {
        let (id, n) = (self.lang.id, self.c.len());
        let triple = matches!(id, "python" | "java" | "kotlin" | "swift" | "dart" | "csharp") && q != '`' && self.at(i, &q.to_string().repeat(3));
        let interp = matches!(id, "kotlin" | "dart") || (q == '`' && matches!(id, "javascript" | "javascriptreact" | "typescript" | "typescriptreact"));
        // a string runs over lines in these (and a literal in JSX text may hold an apostrophe)
        let multi = triple || q == '`' || matches!(id, "php" | "rust" | "csharp");
        let loose = matches!(id, "javascriptreact" | "typescriptreact");
        let mut k = i + if triple { 3 } else { 1 };
        while k < n {
            let ch = self.c[k];
            if ch == '\\' && !(id == "csharp" && i > 0 && self.c[i - 1] == '@') {
                k += 2;
            } else if triple && self.at(k, &q.to_string().repeat(3)) {
                return k + 3;
            } else if !triple && ch == q {
                return k + 1;
            } else if interp && ch == '$' && self.c.get(k + 1) == Some(&'{') {
                k = self.braces(k + 2);
            } else if ch == '\n' && !multi {
                if !loose {
                    self.found.push((i, k, "Aspas não fechadas".into()));
                }
                return k;
            } else {
                k += 1;
            }
        }
        if !loose {
            self.found.push((i, i + 1, if triple { "Texto de aspas triplas não fechado" } else { "Aspas não fechadas" }.into()));
        }
        n
    }

    /// Code inside a `${…}`, from after its `{`; back: after its `}`.
    fn braces(&mut self, mut k: usize) -> usize {
        let (n, mut depth) = (self.c.len(), 1);
        while k < n {
            match self.c[k] {
                q if self.lang.quotes.contains(q) => k = self.string(k, q),
                '{' => (depth, k) = (depth + 1, k + 1),
                '}' => {
                    depth -= 1;
                    k += 1;
                    if depth == 0 {
                        return k;
                    }
                }
                _ => k += 1,
            }
        }
        n
    }

    /// Rust's `r"…"`, `r#"…"#` and `br"…"`: back where it ends, None when `i` is no raw string.
    fn raw_string(&mut self, i: usize) -> Option<usize> {
        let mut k = i + if self.c[i] == 'b' { 2 } else { 1 };
        let hashes = (k..self.c.len()).take_while(|&j| self.c[j] == '#').count();
        k += hashes;
        if self.c.get(k) != Some(&'"') {
            return None;
        }
        let end = format!("\"{}", "#".repeat(hashes));
        match (k + 1..self.c.len()).find(|&j| self.at(j, &end)) {
            Some(j) => Some(j + end.len()),
            None => {
                self.found.push((i, i + 1, "Aspas não fechadas".into()));
                Some(self.c.len())
            }
        }
    }

    /// PHP's `<<<EOT … EOT;`: skips to the line that holds the label.
    fn heredoc(&mut self, i: usize) -> usize {
        let n = self.c.len();
        let mut k = i + 3;
        while self.c.get(k).is_some_and(|c| *c == ' ' || *c == '\'' || *c == '"') {
            k += 1;
        }
        let label: String = self.c[k..].iter().take_while(|c| is_ident(**c)).collect();
        if label.is_empty() {
            return i + 3;
        }
        let from = (k..n).find(|&j| self.c[j] == '\n').unwrap_or(n);
        let mut line = from + 1;
        while line < n {
            let begin = (line..n).find(|&j| !matches!(self.c[j], ' ' | '\t')).unwrap_or(n);
            if self.at(begin, &label) && !self.c.get(begin + label.len()).is_some_and(|c| is_ident(*c)) {
                return begin + label.len();
            }
            line = (line..n).find(|&j| self.c[j] == '\n').map_or(n, |j| j + 1);
        }
        n
    }

    /// A `/` starts a regex literal only after one of these, or at the start.
    fn regex_allowed(&self, i: usize) -> bool {
        (0..i).rev().map(|k| self.c[k]).find(|c| !c.is_whitespace()).is_none_or(|c| "(,=:[!&|?{;".contains(c))
    }

    /// A regex literal at `i`: back where it ends, None when the line has no closing `/`.
    fn regex(&self, i: usize) -> Option<usize> {
        let (mut k, mut class) = (i + 1, false);
        while k < self.c.len() && self.c[k] != '\n' {
            match self.c[k] {
                '\\' => k += 1,
                '[' => class = true,
                ']' => class = false,
                '/' if !class => return Some(k + 1),
                _ => {}
            }
            k += 1;
        }
        None
    }
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn closer(open: char) -> char {
    match open {
        '(' => ')',
        '[' => ']',
        _ => '}',
    }
}

fn opener(close: char) -> char {
    match close {
        ')' => '(',
        ']' => '[',
        _ => '{',
    }
}

/// The files of `root` the project scan reads: code of a checked language, not too big.
fn scannable(p: &Path) -> bool {
    CHECKED.contains(&highlight::detect(p).id) && std::fs::metadata(p).is_ok_and(|m| m.len() <= SCAN_MAX)
}

impl DrawingApp {
    /// `file_tree` `problems`: the problems of the file `path` (its open tab's text, else the disk's) or, with a folder or
    /// none, of the files under it: the open texts and what the servers published, then the local check over the rest.
    pub(crate) fn act_tree_problems(&mut self, id: &str, path: Option<PathBuf>) -> Result<String, String> {
        let root = self.ft_spec(id).map(FileTreeSpec::root_path).unwrap_or_default();
        let ed = self.filetrees.editors.get(id);
        let text_of = |p: &Path| match ed.and_then(|e| e.tabs.iter().find(|t| t.path == p)).map(|t| &t.body) {
            Some(crate::app::filetree::editor::Body::Text(t)) => Some(t.text.clone()),
            _ => std::fs::read_to_string(p).ok().map(|t| t.replace("\r\n", "\n")),
        };
        let empty = Problems::new(Context::default());
        let problems = ed.map_or(&empty, |e| &e.problems);
        let scope = path.clone().unwrap_or_else(|| root.clone());
        if scope.is_file() {
            let text = text_of(&scope).ok_or_else(|| format!("Não foi possível ler {}.", scope.display()))?;
            let diags = problems.for_text(&scope, highlight::detect(&scope), &text);
            return Ok(describe_problems(&root, &[(scope.clone(), diags)], &scope));
        }
        let mut files: Vec<PathBuf> = problems.files().into_iter().filter(|p| p.starts_with(&scope)).collect();
        let known: HashSet<PathBuf> = files.iter().cloned().collect();
        files.extend(crate::app::filetree::code::files_under(&scope, SCAN_FILES, scannable).into_iter().filter(|p| !known.contains(p)));
        files.sort();
        let listed: Vec<(PathBuf, Vec<Diag>)> = files
            .into_iter()
            .filter_map(|p| {
                let text = text_of(&p)?;
                Some((p.clone(), problems.for_text(&p, highlight::detect(&p), &text)))
            })
            .filter(|(_, d)| !d.is_empty())
            .collect();
        Ok(describe_problems(&root, &listed, &scope))
    }
}

/// The MCP's text for the problems found: a line a file with its counts, then a line a problem (the first line is 1).
pub(crate) fn describe_problems(root: &Path, files: &[(PathBuf, Vec<Diag>)], scope: &Path) -> String {
    let name = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    let with: Vec<&(PathBuf, Vec<Diag>)> = files.iter().filter(|(_, d)| !d.is_empty()).collect();
    if with.is_empty() {
        return format!("Nenhum problema encontrado em {}.", name(scope));
    }
    let (mut errors, mut warnings, mut out) = (0, 0, String::new());
    for (path, diags) in with.iter().map(|f| (&f.0, &f.1)).take(60) {
        let (e, w) = counts(diags);
        (errors, warnings) = (errors + e, warnings + w);
        let plural = |n: usize, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
        out += &format!("{}: {}, {}\n", name(path), plural(e, "erro", "erros"), plural(w, "aviso", "avisos"));
        for d in diags.iter().take(40) {
            let kind = if d.severity == ERROR { "erro" } else { "aviso" };
            out += &format!("  {kind} {}:{} [{}] {}\n", d.line + 1, d.col + 1, d.source, d.message.lines().next().unwrap_or_default());
        }
    }
    if with.len() > 60 {
        out += &format!("… e mais {} arquivo(s) com problemas.\n", with.len() - 60);
    }
    format!("{} arquivo(s) com problemas ({errors} erro(s), {warnings} aviso(s)):\n{out}", with.len())
}
