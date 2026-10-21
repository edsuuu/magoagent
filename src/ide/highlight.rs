//! Tiny table-driven highlighter. No grammars, no regex: one pass per buffer.
// ponytail: lexical only (no parse tree); add tree-sitter when semantic colors matter.

pub use super::langs::*;
use super::theme as t;
use eframe::egui::{Color32, FontId, text::LayoutJob, text::TextFormat};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum St {
    Text,
    Keyword,
    Str,
    Number,
    Comment,
    Doc,
    Function,
    Method,
    Field,
    Constant,
    Meta,
    Tag,
    /// a `()[]{}` bracket at this nesting depth (Rainbow Brackets)
    Bracket(u8),
}

pub fn color(s: St) -> Color32 {
    match s {
        St::Bracket(d) => t::BRACKETS[d as usize % t::BRACKETS.len()],
        St::Text => t::EDITOR_TEXT,
        St::Keyword => t::KEYWORD,
        St::Str => t::STRING,
        St::Number => t::NUMBER,
        St::Comment => t::COMMENT,
        St::Doc => t::DOC_COMMENT,
        St::Function => t::FUNCTION,
        St::Method => t::METHOD,
        St::Field | St::Constant => t::FIELD,
        St::Meta => t::METADATA,
        St::Tag => t::TAG,
    }
}

/// The highlighted text as one LayoutJob, every line `line_height` high.
pub fn layout(lang: &Lang, src: &str, font: FontId, line_height: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let fmt = |s: St| TextFormat {
        font_id: font.clone(),
        color: color(s),
        italics: matches!(s, St::Doc | St::Constant),
        line_height: Some(line_height),
        valign: eframe::egui::Align::Center,
        ..Default::default()
    };
    let push = |job: &mut LayoutJob, text: &str, s: St| job.append(text, 0.0, fmt(s));
    let mut at = 0;
    for (a, b, s) in tokens(lang, src) {
        if a > at {
            push(&mut job, &src[at..a], St::Text);
        }
        push(&mut job, &src[a..b], s);
        at = b;
    }
    push(&mut job, &src[at..], St::Text);
    job
}

type Tok = (usize, usize, St);

pub fn tokens(lang: &Lang, s: &str) -> Vec<Tok> {
    let mut out = vec![];
    match lang.kind {
        Kind::Code => code(lang, s, 0, s.len(), &mut out),
        Kind::Markup => markup(lang, s, &mut out),
        Kind::Plain => {}
    }
    out
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// Index of the matching `close` for an `open` at `i` (balanced), or `end`.
fn balanced(b: &[u8], mut i: usize, end: usize, open: u8, close: u8) -> usize {
    let mut depth = 0;
    while i < end {
        if b[i] == open {
            depth += 1;
        } else if b[i] == close {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        }
        i += 1;
    }
    end
}

fn string_end(b: &[u8], mut i: usize, end: usize, q: u8) -> usize {
    i += 1;
    while i < end {
        match b[i] {
            b'\\' => i += 2,
            c if c == q => return i + 1,
            b'\n' if q != b'`' && q != b'"' => return i, // unterminated char/'string'
            _ => i += 1,
        }
    }
    end
}

fn code(lang: &Lang, s: &str, start: usize, end: usize, out: &mut Vec<Tok>) {
    let b = s.as_bytes();
    let id = lang.id;
    let mut i = start;
    let mut prev_word = "";
    let mut css_depth = 0;
    let mut open: Vec<u8> = vec![]; // brackets not closed yet; a bracket's depth is its place here
    let mut line_start = true; // only whitespace seen since the last newline
    while i < end {
        let c = b[i];
        let rest = &s[i..end];
        if c == b'\n' {
            line_start = true;
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let was_line_start = std::mem::replace(&mut line_start, false);

        // attributes: Rust/PHP `#[...]`
        if (id == "rust" || id == "php") && rest.starts_with("#[") || (id == "rust" && rest.starts_with("#![")) {
            let e = balanced(b, i, end, b'[', b']');
            out.push((i, e, St::Meta));
            i = e;
            continue;
        }
        // C preprocessor: `#include` as keyword
        if matches!(id, "c" | "cpp" | "csharp") && c == b'#' && was_line_start {
            let mut e = i + 1;
            while e < end && is_ident(b[e]) {
                e += 1;
            }
            out.push((i, e, St::Keyword));
            i = e;
            continue;
        }
        if let Some((open, close)) = lang.block {
            if rest.starts_with(open) {
                let e = s[i + open.len()..end].find(close).map_or(end, |p| i + open.len() + p + close.len());
                let doc = rest.starts_with("/**") && !rest.starts_with("/**/");
                out.push((i, e, if doc { St::Doc } else { St::Comment }));
                i = e;
                continue;
            }
        }
        if let Some(lc) = lang.line.iter().find(|p| rest.starts_with(**p)) {
            let e = s[i..end].find('\n').map_or(end, |p| i + p);
            let doc = id == "rust" && (rest.starts_with("///") || rest.starts_with("//!"));
            out.push((i, e, if doc { St::Doc } else { St::Comment }));
            let _ = lc;
            i = e;
            continue;
        }
        if lang.quotes.as_bytes().contains(&c) {
            // Rust lifetime `'a` (no closing quote right after the identifier)
            if id == "rust" && c == b'\'' {
                let mut e = i + 1;
                while e < end && is_ident(b[e]) {
                    e += 1;
                }
                if e > i + 1 && (e >= end || b[e] != b'\'') {
                    out.push((i, e, St::Keyword));
                    i = e;
                    continue;
                }
            }
            let e = string_end(b, i, end, c);
            let key = matches!(id, "json" | "yaml") && s[e..end].trim_start().starts_with(':');
            out.push((i, e, if key { St::Field } else { St::Str }));
            i = e;
            continue;
        }
        if c.is_ascii_digit() || (c == b'#' && id == "css") {
            let mut e = i + 1;
            while e < end && (is_ident(b[e]) || b[e] == b'.' && e + 1 < end && b[e + 1].is_ascii_digit()) {
                e += 1;
            }
            if id == "css" {
                while e < end && (b[e] == b'%' || b[e].is_ascii_alphabetic()) {
                    e += 1;
                }
            }
            let st = if c == b'#' && css_depth == 0 { St::Tag } else { St::Number };
            out.push((i, e, st));
            i = e;
            continue;
        }
        if id == "css" {
            match c {
                b'{' => css_depth += 1,
                b'}' => css_depth = (css_depth - 1).max(0),
                _ => {}
            }
        }
        let sigil = matches!(c, b'$' | b'@') || (c == b'.' && id == "asm") || (c == b'%' && id == "asm");
        if is_ident(c) || (sigil && i + 1 < end && is_ident(b[i + 1])) {
            let mut e = i + 1;
            while e < end && (is_ident(b[e]) || (id == "css" && b[e] == b'-')) {
                e += 1;
            }
            let w = &s[i..e];
            let after = s[e..end].trim_start();
            let before = s[start..i].trim_end();
            let call = after.starts_with('(');
            let st = token_style(lang, w, prev_word, before, after, call, was_line_start, css_depth);
            if let Some(st) = st {
                out.push((i, e, st));
            }
            prev_word = w;
            i = e;
            continue;
        }
        // brackets outside strings and comments (those are consumed above). A closer takes its
        // opener's depth; a stray closer stays plain.
        // ponytail: regex literals and #[...] attributes are not bracket-counted; find/diff rows are laid out
        // one line at a time, so their depth restarts at 0
        match c {
            b'(' | b'[' | b'{' => {
                out.push((i, i + 1, St::Bracket(open.len() as u8)));
                open.push(c);
            }
            b')' | b']' | b'}' => {
                let want = match c {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                // a missing closer is closed by the next one of its kind, taking the blocks inside it along
                if let Some(p) = open.iter().rposition(|&o| o == want) {
                    open.truncate(p);
                    out.push((i, i + 1, St::Bracket(p as u8)));
                }
            }
            _ => {}
        }
        i += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn token_style(lang: &Lang, w: &str, prev: &str, before: &str, after: &str, call: bool, first: bool, css_depth: i32) -> Option<St> {
    let id = lang.id;
    let first_ch = w.as_bytes()[0];
    if first_ch == b'$' {
        return (id == "php" && w == "$this").then_some(St::Keyword);
    }
    if first_ch == b'@' {
        return Some(if matches!(id, "css" | "blade") { St::Keyword } else { St::Meta });
    }
    match id {
        "css" if css_depth > 0 && after.starts_with(':') => return Some(St::Field),
        "css" if css_depth == 0 => return Some(St::Tag),
        "asm" => {
            if after.starts_with(':') {
                return Some(St::Function);
            }
            if matches!(first_ch, b'.' | b'%') || lang.kw.contains(&w.to_ascii_lowercase().as_str()) {
                return Some(St::Meta);
            }
            if is_register(w) {
                return Some(St::Field);
            }
            return first.then_some(St::Keyword);
        }
        "yaml" | "toml" if first && (after.starts_with(':') || after.starts_with('=')) => return Some(St::Field),
        _ => {}
    }
    let kw = if id == "sql" { lang.kw.contains(&w.to_ascii_lowercase().as_str()) } else { lang.kw.contains(&w) };
    if kw {
        return Some(St::Keyword);
    }
    if matches!(prev, "fn" | "function" | "def" | "func") {
        return Some(St::Function);
    }
    let member = before.ends_with("->") || before.ends_with('.') && !before.ends_with("..");
    if member {
        return Some(if call { St::Method } else { St::Field });
    }
    if before.ends_with("::") && call {
        return Some(St::Method);
    }
    if id == "rust" && after.starts_with('!') && !after.starts_with("!=") {
        return Some(St::Function); // macro
    }
    let upper = w.len() > 1 && w.bytes().all(|b| b.is_ascii_uppercase() || b == b'_' || b.is_ascii_digit()) && w.bytes().any(|b| b.is_ascii_uppercase());
    upper.then_some(St::Constant)
}

fn is_register(w: &str) -> bool {
    const REGS: &[&str] = &[
        "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "rip", "eax", "ebx", "ecx", "edx", "esi",
        "edi", "ebp", "esp", "eip", "ax", "bx", "cx", "dx", "si", "di", "bp", "sp", "al", "ah", "bl", "bh",
        "cl", "ch", "dl", "dh", "cs", "ds", "es", "fs", "gs", "ss", "lr", "pc", "xzr", "wzr",
    ];
    let w = w.to_ascii_lowercase();
    let numbered = |p: &str| w.strip_prefix(p).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit() || b"dwb".contains(&b)));
    REGS.contains(&w.as_str()) || ["r", "x", "w", "xmm", "ymm", "zmm"].iter().any(|p| numbered(p))
}

/// HTML/XML/Blade: tags, attributes, comments, plus embedded <script>/<style> and Blade/PHP islands.
fn markup(lang: &Lang, s: &str, out: &mut Vec<Tok>) {
    let b = s.as_bytes();
    let blade = lang.id == "blade";
    let mut i = 0;
    while i < b.len() {
        let rest = &s[i..];
        let (open, close) = lang.block.unwrap();
        if rest.starts_with(open) || (blade && rest.starts_with("<!--")) {
            let close = if rest.starts_with("<!--") { "-->" } else { close };
            let e = rest.find(close).map_or(s.len(), |p| i + p + close.len());
            out.push((i, e, St::Comment));
            i = e;
            continue;
        }
        if rest.starts_with("<?php") || (blade && (rest.starts_with("{{") || rest.starts_with("{!!") || rest.starts_with("@php"))) {
            let (o, c) = if rest.starts_with("<?php") {
                ("<?php", "?>")
            } else if rest.starts_with("{!!") {
                ("{!!", "!!}")
            } else if rest.starts_with("@php") {
                ("@php", "@endphp")
            } else {
                ("{{", "}}")
            };
            let inner = i + o.len();
            let e = s[inner..].find(c).map_or(s.len(), |p| inner + p);
            out.push((i, inner, St::Keyword));
            code(&PHP, s, inner, e, out);
            if e < s.len() {
                out.push((e, e + c.len(), St::Keyword));
            }
            i = (e + c.len()).min(s.len());
            continue;
        }
        if blade && b[i] == b'@' && i + 1 < b.len() && b[i + 1].is_ascii_alphabetic() {
            let mut e = i + 1;
            while e < b.len() && is_ident(b[e]) {
                e += 1;
            }
            out.push((i, e, St::Keyword));
            if b.get(e) == Some(&b'(') {
                let pe = balanced(b, e, b.len(), b'(', b')');
                code(&PHP, s, e, pe, out);
                e = pe;
            }
            i = e;
            continue;
        }
        if b[i] == b'<' && i + 1 < b.len() && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'/' || b[i + 1] == b'!') {
            let mut e = i + 1 + (b[i + 1] == b'/') as usize;
            let name_start = e;
            while e < b.len() && (is_ident(b[e]) || b"-:.!".contains(&b[e])) {
                e += 1;
            }
            let name = s[name_start..e].to_ascii_lowercase();
            out.push((name_start, e, St::Tag));
            // attributes until '>'
            while e < b.len() && b[e] != b'>' {
                if b[e] == b'"' || b[e] == b'\'' {
                    let se = string_end(b, e, b.len(), b[e]);
                    out.push((e, se, St::Str));
                    e = se;
                } else if blade && b[e] == b'{' && s[e..].starts_with("{{") {
                    break; // let the outer loop handle {{ }} inside a tag
                } else {
                    e += 1;
                }
            }
            i = if e < b.len() && b[e] == b'>' { e + 1 } else { e };
            if b[name_start - 1] != b'/' && (name == "script" || name == "style") {
                let close = format!("</{name}");
                let ce = s[i..].to_ascii_lowercase().find(&close).map_or(s.len(), |p| i + p);
                code(if name == "script" { &JS } else { &CSS }, s, i, ce, out);
                i = ce;
            }
            continue;
        }
        i += s[i..].chars().next().map_or(1, char::len_utf8);
    }
    out.sort_by_key(|t| t.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styles(lang: &Lang, src: &str) -> Vec<(String, St)> {
        tokens(lang, src).into_iter().map(|(a, b, s)| (src[a..b].to_string(), s)).collect()
    }

    #[test]
    fn php() {
        let t = styles(&PHP, "<?php // hi\nfunction foo($a) { return $this->bar() . $this->baz . \"x\"; } #[Attr(['a'])]");
        assert!(t.contains(&("// hi".into(), St::Comment)));
        assert!(t.contains(&("foo".into(), St::Function)));
        assert!(t.contains(&("$this".into(), St::Keyword)));
        assert!(t.contains(&("bar".into(), St::Method)));
        assert!(t.contains(&("baz".into(), St::Field)));
        assert!(t.contains(&("\"x\"".into(), St::Str)));
        assert!(t.contains(&("#[Attr(['a'])]".into(), St::Meta)));
    }

    #[test]
    fn rust_lifetime_vs_char() {
        let t = styles(&RUST, "fn f<'a>(x: &'a str) -> char { 'z' } /// doc");
        assert!(t.contains(&("'a".into(), St::Keyword)));
        assert!(t.contains(&("'z'".into(), St::Str)));
        assert!(t.contains(&("/// doc".into(), St::Doc)));
    }

    #[test]
    fn blade_and_html() {
        let t = styles(&BLADE, "<div class=\"x\">{{ $user->name }}</div>@if($a)<script>let x = 1;</script>{{-- c --}}");
        assert!(t.contains(&("div".into(), St::Tag)));
        assert!(t.contains(&("\"x\"".into(), St::Str)));
        assert!(t.contains(&("name".into(), St::Field)));
        assert!(t.contains(&("@if".into(), St::Keyword)));
        assert!(t.contains(&("let".into(), St::Keyword)));
        assert!(t.contains(&("{{-- c --}}".into(), St::Comment)));
    }

    #[test]
    fn asm_sql_json() {
        let t = styles(&ASM, "_start:\n  mov rax, 60 ; exit\nsection .text");
        assert!(t.contains(&("_start".into(), St::Function)));
        assert!(t.contains(&("mov".into(), St::Keyword)));
        assert!(t.contains(&("rax".into(), St::Field)));
        assert!(t.contains(&("; exit".into(), St::Comment)));
        assert!(styles(&SQL, "select * FROM t").contains(&("FROM".into(), St::Keyword)));
        assert!(styles(&JSON, "{\"k\": \"v\"}").contains(&("\"k\"".into(), St::Field)));
    }

    #[test]
    fn rainbow_brackets() {
        // brackets inside a string or a comment don't count; a closer takes its opener's depth
        let brackets = |src: &str| -> Vec<(String, St)> { styles(&JS, src).into_iter().filter(|(_, s)| matches!(s, St::Bracket(_))).collect() };
        assert_eq!(
            brackets("f(\"(\", /* [ */ [1]) { x }"),
            vec![("(".into(), St::Bracket(0)), ("[".into(), St::Bracket(1)), ("]".into(), St::Bracket(1)), (")".into(), St::Bracket(0)), ("{".into(), St::Bracket(0)), ("}".into(), St::Bracket(0))]
        );
        // a missing ')' is closed by the '}' that comes after it
        assert_eq!(brackets("{ ( }"), vec![("{".into(), St::Bracket(0)), ("(".into(), St::Bracket(1)), ("}".into(), St::Bracket(0))]);
    }

    #[test]
    fn layout_covers_all_text() {
        // tokens must be sorted, non-overlapping and on char boundaries, or LayoutJob panics.
        let src = "<p>ção</p>{{ $é }} @foo(1) <style>a{b:1px}</style>";
        let mut at = 0;
        for (a, b, _) in tokens(&BLADE, src) {
            assert!(a >= at && b >= a && src.is_char_boundary(a) && src.is_char_boundary(b), "{a}..{b}");
            at = b;
        }
    }
}
