//! Parameter name hints (PhpStorm's inlay hints): `montar(10, 'x')` shows `largura: 10, titulo: 'x'`.
//! Paint only: `open` makes a gap before each hinted argument and the editor paints the label into it, so
//! the text, the caret and the clicks don't change. The names come from the language server when it offers
//! inlay hints, else from the project index (`Index::params_for`).
// ponytail: calls are found by scanning the text, not by a parser; `new Foo(...)` gets the names of the
// `__construct` the index files under `Foo`

use super::highlight::{self, St};
use super::*;
use eframe::egui::{FontId, text::ByteIndex, text::LayoutSection};
use std::collections::HashMap;

pub(super) const PAD: f32 = 4.0; // inside the label's box, on each side
pub(super) const AFTER: f32 = 2.0; // between the label and its argument: it sits close
pub(super) const LABEL_BG: Color32 = Color32::from_rgb(0x3B, 0x3D, 0x41);
pub(super) const LABEL_TEXT: Color32 = Color32::from_rgb(0x9D, 0xA0, 0xA8);
// PhpStorm's "show name for all arguments" (on for the user). Off, only the obscure arguments get a name.
// ponytail: a constant, not a setting; flip it to false for the "only obscure" mode
pub(super) const ALL_ARGS: bool = true;

/// PhpStorm's default exclusion list for PHP: no hints for these calls. A pattern is `name(p1, p2)` or a bare
/// name; `*` matches any run of characters, an empty name any call. `*.add` and `\define` match the bare name.
const PHP_EXCLUDED: [&str; 15] = [
    "*Exception",
    "(begin*, end*)",
    "(start*, end*)",
    "(first*, last*)",
    "(first*, second*)",
    "(from*, to*)",
    "(min*, max*)",
    "(key, value)",
    "(format, arg*)",
    "(message)",
    "(message, error)",
    "\\define(*,*,*)",
    "*.add(*)",
    "*.set(*,*)",
    "*.get(*)",
];

/// A name drawn before the argument at byte `byte` (char index `ch`): `label` is what it says, "valor:".
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Hint {
    pub byte: usize,
    pub ch: usize,
    pub label: String,
}

/// A call `name(args)`; each argument is (byte offset of its first character, its text).
pub(super) struct Call {
    pub name: String,
    pub args: Vec<(usize, String)>,
}

fn is_id(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// The calls in `text`: not declarations, not inside strings or comments, positional arguments only.
pub(super) fn calls(lang: &highlight::Lang, text: &str) -> Vec<Call> {
    let quoted: Vec<(usize, usize)> =
        highlight::tokens(lang, text).into_iter().filter(|t| matches!(t.2, St::Str | St::Comment | St::Doc)).map(|t| (t.0, t.1)).collect();
    let b = text.as_bytes();
    let mut out = vec![];
    for open in 0..b.len() {
        if b[open] != b'(' || quoted.iter().any(|&(x, y)| x <= open && open < y) {
            continue;
        }
        let start = b[..open].iter().rposition(|c| !is_id(*c)).map_or(0, |p| p + 1);
        // `$fn(...)` calls a variable, not a function of that name
        if start == open || b[start].is_ascii_digit() || (start > 0 && b[start - 1] == b'$') {
            continue;
        }
        let Some(close) = close_of(b, open) else { continue };
        if is_decl(text, start, close) {
            continue;
        }
        let args = parts(&text[open + 1..close])
            .into_iter()
            .filter_map(|(at, part)| {
                let t = part.trim();
                (!t.is_empty() && !is_named(t)).then(|| (open + 1 + at + (part.len() - part.trim_start().len()), t.to_string()))
            })
            .collect();
        out.push(Call { name: text[start..open].to_string(), args });
    }
    out
}

/// `function name(` (or `def`, `fn`, `func`, `sub`) and `name(...) {`: a declaration, not a call.
fn is_decl(text: &str, start: usize, close: usize) -> bool {
    let word = text[..start].trim_end().rsplit(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or("");
    matches!(word, "function" | "fn" | "def" | "func" | "sub") || text[close + 1..].trim_start().starts_with('{')
}

/// The byte of the `)` closing the `(` at `open`, strings skipped (a quote ends at its line's end too).
fn close_of(b: &[u8], open: usize) -> Option<usize> {
    let (mut depth, mut quote, mut i) = (0usize, None::<u8>, open);
    while i < b.len() {
        match (quote, b[i]) {
            (Some(_), b'\\') => i += 1,
            (Some(q), c) if c == q || c == b'\n' => quote = None,
            (Some(_), _) => {}
            (None, b'\'' | b'"' | b'`') => quote = Some(b[i]),
            (None, b'(' | b'[' | b'{') => depth += 1,
            (None, b')' | b']' | b'}') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The top-level comma-separated parts of `inner`, each with its byte offset in it.
fn parts(inner: &str) -> Vec<(usize, &str)> {
    let b = inner.as_bytes();
    let (mut out, mut depth, mut quote, mut start, mut i) = (vec![], 0usize, None::<u8>, 0usize, 0usize);
    while i < b.len() {
        match (quote, b[i]) {
            (Some(_), b'\\') => i += 1,
            (Some(q), c) if c == q || c == b'\n' => quote = None,
            (Some(_), _) => {}
            (None, b'\'' | b'"' | b'`') => quote = Some(b[i]),
            (None, b'(' | b'[' | b'{') => depth += 1,
            (None, b')' | b']' | b'}') => depth = depth.saturating_sub(1),
            (None, b',') if depth == 0 => {
                out.push((start, &inner[start..i]));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push((start, &inner[start..]));
    out
}

/// `name: value` (a PHP 8 named argument); `Foo::bar` is not one.
fn is_named(arg: &str) -> bool {
    let id = arg.bytes().take_while(|c| is_id(*c)).count();
    let rest = arg[id..].trim_start();
    id > 0 && rest.starts_with(':') && !rest.starts_with("::")
}

/// The parameter names of the declaration whose name starts at byte `at`, "" where a parameter has no
/// plain name (a destructured one). Go writes the type after the name, so it gives none.
pub(super) fn params_at(lang_id: &str, text: &str, at: usize) -> Option<Vec<String>> {
    if lang_id == "go" {
        return None;
    }
    let open = at + text[at..].find('(')?;
    let close = close_of(text.as_bytes(), open)?;
    let mut names: Vec<String> = parts(&text[open + 1..close])
        .into_iter()
        .map(|(_, p)| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| {
            if p.starts_with(['{', '[', '(']) {
                return String::new();
            }
            // the name is the last word before the type's `:` or the default's `=`
            let b = p.as_bytes();
            let cut = (0..b.len()).find(|&i| b[i] == b'=' || (b[i] == b':' && b.get(i + 1) != Some(&b':') && (i == 0 || b[i - 1] != b':'))).unwrap_or(b.len());
            let head = &p[..cut];
            let name = head.rsplit(|c: char| !(c.is_alphanumeric() || c == '_')).find(|w| !w.is_empty()).unwrap_or("");
            // PHP: `&$x` is by reference and `...$x` variadic, and the hint's name carries that mark
            let before = head.trim_end().trim_end_matches(|c: char| c.is_alphanumeric() || c == '_').trim_end_matches('$').trim_end();
            let mark = match lang_id {
                "php" if before.ends_with("...") => "...",
                "php" if before.ends_with('&') => "&",
                _ => "",
            };
            format!("{mark}{name}")
        })
        .collect();
    if matches!(names.first().map(String::as_str), Some("self" | "this" | "cls")) {
        names.remove(0);
    }
    Some(names)
}

/// PhpStorm's rules for when a name is worth its space: not when the argument already is a variable of
/// that name, not for a lone argument when the call's own name says it (`setName(name)`) or when the call
/// is a setter (`setName('x')`), and with `all` off only for the obscure arguments.
pub(super) fn wanted(call: &str, lone: bool, param: &str, arg: &str, all: bool) -> bool {
    let var = arg.strip_prefix('$').unwrap_or(arg);
    let bare = param.trim_start_matches(['&', '.']);
    let setter = call == "set" || call.strip_prefix("set").is_some_and(|r| r.starts_with(char::is_uppercase));
    !bare.is_empty() && var != bare && !(lone && (setter || call.to_lowercase().ends_with(&bare.to_lowercase()))) && (all || obscure(arg))
}

/// PhpStorm's "obscure" arguments: literals, constants, `Foo::BAR`, binary and ternary expressions. Text match,
/// so a string that holds " + " counts as binary too.
fn obscure(arg: &str) -> bool {
    const BINARY: [&str; 19] =
        [" + ", " - ", " * ", " / ", " % ", " . ", " && ", " || ", " ?? ", " == ", " != ", " === ", " !== ", " < ", " > ", " <= ", " >= ", " and ", " or "];
    let a = arg.trim().trim_start_matches(['-', '+']);
    let constant = a.len() > 1 && a.bytes().any(|b| b.is_ascii_uppercase()) && a.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
    a.starts_with(|c: char| c.is_ascii_digit() || c == '\'' || c == '"')
        || matches!(a.to_ascii_lowercase().as_str(), "null" | "true" | "false")
        || constant
        || (a.contains("::") && !a.contains(['(', '$']))
        || BINARY.iter().any(|op| arg.contains(op))
        || (arg.contains(" ? ") && arg.contains(" : "))
}

/// Whether PHP's default exclusion list silences the call `call` of a declaration with these parameters.
fn excluded(call: &str, params: &[String]) -> bool {
    PHP_EXCLUDED.iter().any(|pat| {
        let (name, args) = match pat.split_once('(') {
            Some((n, a)) => (n, Some(a.trim_end_matches(')').split(',').map(str::trim).collect::<Vec<_>>())),
            None => (*pat, None),
        };
        let name = name.rsplit(['.', '\\']).next().unwrap_or(name);
        (name.is_empty() || glob(name, call))
            && args.is_none_or(|a| a.len() == params.len() && a.iter().zip(params).all(|(p, n)| glob(p, n.trim_start_matches(['&', '.']))))
    })
}

/// Glob match where `*` is any run of characters.
fn glob(pat: &str, s: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == s;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if s.len() < first.len() + last.len() || !s.starts_with(first) || !s.ends_with(last) {
        return false;
    }
    let mut rest = &s[first.len()..s.len() - last.len()];
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(i) => rest = &rest[i + mid.len()..],
            None => return false,
        }
    }
    true
}

/// Byte offset of the start of each line.
pub(super) fn byte_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect()
}

/// Byte offset of (line, char column), clamped to the line.
pub(super) fn byte_of(text: &str, starts: &[usize], line: usize, col: usize) -> usize {
    let Some(&start) = starts.get(line) else { return text.len() };
    let end = starts.get(line + 1).map_or(text.len(), |n| n - 1);
    text[start..end].char_indices().nth(col).map_or(end, |(b, _)| start + b)
}

/// Hints from the project index: the parameter names of each call's function.
pub(super) fn from_index(calls: &[Call], index: &index::Index, path: &Path) -> Vec<Hint> {
    let mut out = vec![];
    for c in calls {
        let Some(params) = index.params_for(&c.name, path) else { continue };
        if highlight::detect(path).id == "php" && excluded(&c.name, params) {
            continue;
        }
        for ((byte, arg), param) in c.args.iter().zip(params) {
            if wanted(&c.name, c.args.len() == 1, param, arg, ALL_ARGS) {
                out.push(Hint { byte: *byte, ch: 0, label: format!("{param}:") });
            }
        }
    }
    out
}

/// Hints from the language server's answer, kept where they fall on a call's argument.
pub(super) fn from_lsp(text: &str, calls: &[Call], answer: &[(usize, usize, String)]) -> Vec<Hint> {
    let starts = byte_starts(text);
    let args: HashMap<usize, (&str, bool, &str)> =
        calls.iter().flat_map(|c| c.args.iter().map(move |(byte, arg)| (*byte, (c.name.as_str(), c.args.len() == 1, arg.as_str())))).collect();
    answer
        .iter()
        .filter_map(|(line, col, label)| {
            let byte = byte_of(text, &starts, *line, *col);
            let &(name, lone, arg) = args.get(&byte)?;
            let param = label.trim_end_matches(':').trim();
            wanted(name, lone, param, arg, ALL_ARGS).then(|| Hint { byte, ch: 0, label: format!("{param}:") })
        })
        .collect()
}

/// Sorts the hints, keeps one per argument and works out each char index (one pass over the text).
pub(super) fn finish(text: &str, mut hints: Vec<Hint>) -> Vec<Hint> {
    hints.sort_by_key(|h| h.byte);
    hints.dedup_by_key(|h| h.byte);
    let (mut at, mut ch) = (0, 0);
    for h in &mut hints {
        ch += text[at..h.byte].chars().count();
        at = h.byte;
        h.ch = ch;
    }
    hints
}

/// The width of a label in `font`.
pub(super) fn width(ui: &Ui, label: &str, font: &FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), font.clone(), LABEL_TEXT).size().x)
}

/// The gap a label of width `w` needs before its argument.
pub(super) fn gap(w: f32) -> f32 {
    w + 2.0 * PAD + AFTER
}

/// Opens a gap of `width` before the byte `at` of each mark (`(at, width)`, sorted by byte). The layout
/// keeps its text: only the glyphs from there on move.
pub(super) fn open(job: &mut LayoutJob, marks: &[(usize, f32)]) {
    for &(at, width) in marks {
        split_at(job, at);
        if let Some(s) = job.sections.iter_mut().find(|s| s.byte_range.start.0 == at) {
            s.leading_space += width;
        }
    }
}

/// Cuts the section holding byte `at` in two, so that a section starts there.
pub(super) fn split_at(job: &mut LayoutJob, at: usize) {
    let Some(i) = job.sections.iter().position(|s| s.byte_range.start.0 < at && at < s.byte_range.end.0) else { return };
    let tail = LayoutSection { leading_space: 0.0, byte_range: ByteIndex(at)..job.sections[i].byte_range.end, format: job.sections[i].format.clone() };
    job.sections[i].byte_range.end = ByteIndex(at);
    job.sections.insert(i + 1, tail);
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::text::CCursor;

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_name_is_shown_unless_the_argument_already_says_it() {
        assert!(wanted("montar", false, "titulo", "'x'", true));
        assert!(!wanted("montar", false, "largura", "$largura", true)); // a variable of that name
        assert!(!wanted("montar", false, "largura", "largura", true));
        assert!(wanted("montar", true, "largura", "10", true)); // a lone argument the call doesn't name
        assert!(!wanted("setName", true, "name", "'x'", true)); // a lone argument the call's name already says
        assert!(wanted("setName", false, "name", "'x'", true));
        assert!(!wanted("setNome", true, "value", "'x'", true)); // a setter's lone argument
        assert!(wanted("f", false, "&ref", "$x", true)); // by-reference mark, a different variable
        assert!(!wanted("f", false, "&x", "$x", true));
    }

    #[test]
    fn php_exclusions_obscure_arguments_and_constructors() {
        // the default exclusion list: by name, count and parameter names
        assert!(excluded("InvalidFooException", &strs(&["x"])));
        assert!(excluded("montar", &strs(&["key", "value"])));
        assert!(excluded("add", &strs(&["item"])));
        assert!(!excluded("montar", &strs(&["key"])));
        assert!(!excluded("montar", &strs(&["largura", "titulo"])));
        // with "all arguments" off, only the obscure ones (literals, constants, binary, ternary) get a name
        assert!(wanted("montar", false, "a", "10", false));
        assert!(wanted("montar", false, "a", "FOO", false));
        assert!(wanted("montar", false, "a", "Foo::BAR", false));
        assert!(wanted("montar", false, "a", "$x . 'y'", false));
        assert!(wanted("montar", false, "a", "$x ? 1 : 2", false));
        assert!(!wanted("montar", false, "a", "$x", false));
        assert!(!wanted("montar", false, "a", "$x->y()", false));
        // parameter marks: `&` by reference, `...` variadic
        assert_eq!(params_at("php", "function f(&$a, int ...$r, $b = 1) {}", 9), Some(strs(&["&a", "...r", "b"])));
    }

    #[test]
    fn constructor_hints_come_from_the_class() {
        let dir = std::env::temp_dir().join(format!("magoagent-ctor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("caixa.php");
        std::fs::write(&file, "<?php\nclass Caixa {\n    public function __construct(int $largura, string $titulo) {}\n}\n$c = new Caixa(10, 'x');\n").unwrap();
        let ix = index::Index::build(&dir, std::slice::from_ref(&file));
        let text = std::fs::read_to_string(&file).unwrap();
        let found = finish(&text, from_index(&calls(&highlight::PHP, &text), &ix, &file));
        assert_eq!(found.iter().map(|h| h.label.as_str()).collect::<Vec<_>>(), ["largura:", "titulo:"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn calls_skip_declarations_strings_and_named_arguments() {
        let src = "<?php\nfunction montar($a, $b) {}\nmontar(10, 'x, y', nome: 2);\n$s = 'soma(1)';";
        let found: Vec<_> = calls(&highlight::PHP, src).into_iter().map(|c| (c.name, c.args.into_iter().map(|a| a.1).collect::<Vec<_>>())).collect();
        assert_eq!(found, vec![("montar".to_string(), strs(&["10", "'x, y'"]))]);
    }

    #[test]
    fn parameter_names_come_from_the_declaration() {
        let php = "<?php\nfunction montar(int $largura, string $titulo = 'a, b', ...$resto): string {";
        assert_eq!(params_at("php", php, php.find("montar").unwrap()), Some(strs(&["largura", "titulo", "...resto"])));
        assert_eq!(params_at("typescript", "function f(a: number, b?: string, {c}: O) {}", 9), Some(strs(&["a", "b", ""])));
        assert_eq!(params_at("rust", "fn f(&self, mut x: i32) {}", 3), Some(strs(&["x"])));
        assert_eq!(params_at("go", "func f(a int)", 5), None);
    }

    #[test]
    fn hints_come_from_the_index_of_the_project() {
        let dir = std::env::temp_dir().join(format!("magoagent-hints-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("dicas.php");
        std::fs::write(&file, "<?php\nfunction montar(int $largura, string $titulo, bool $ativo = true) {}\n$x = montar($largura, 'x', true);\n").unwrap();
        let ix = index::Index::build(&dir, std::slice::from_ref(&file));
        let text = std::fs::read_to_string(&file).unwrap();
        let found = finish(&text, from_index(&calls(&highlight::PHP, &text), &ix, &file));
        assert_eq!(found.iter().map(|h| h.label.as_str()).collect::<Vec<_>>(), ["titulo:", "ativo:"]);
        assert_eq!(&text[found[0].byte..found[0].byte + 3], "'x'");
        assert_eq!(found[0].ch, text[..found[0].byte].chars().count());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_gap_moves_the_glyphs_not_the_text_or_the_clicks() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {}); // sets the fonts up
        out.textures_delta.clear();
        let font = egui::FontId::monospace(13.0);
        let layout = |marks: &[(usize, f32)]| {
            let mut job = LayoutJob::simple("funcao(10, 'x')".into(), font.clone(), Color32::WHITE, f32::INFINITY);
            open(&mut job, marks);
            ctx.fonts_mut(|f| f.layout_job(job))
        };
        let (plain, gap) = (layout(&[]), layout(&[(7, 40.0)]));
        assert_eq!(gap.text(), plain.text());
        let x = |g: &egui::Galley, i: usize| g.pos_from_cursor(CCursor::new(i)).min.x;
        // the `1` (char 7) moves by the gap, the `(` before it does not
        assert!((x(&gap, 7) - x(&plain, 7) - 40.0).abs() < 0.01);
        assert!((x(&gap, 6) - x(&plain, 6)).abs() < 0.01);
        // the caret stops at the same boundaries: each char's left edge clicks back to its own index
        let y = gap.pos_from_cursor(CCursor::new(0)).center().y;
        for i in 0..plain.text().chars().count() {
            assert_eq!(gap.cursor_from_pos(vec2(x(&gap, i) + 0.5, y)).index.0, i, "char {i}");
        }
        // a click in the gap lands next to the argument, never past it
        let mid = x(&gap, 7) - 20.0;
        assert!((6..=7).contains(&gap.cursor_from_pos(vec2(mid, y)).index.0));
    }
}
