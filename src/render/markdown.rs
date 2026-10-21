//! Markdown on sticky notes: the line-by-line subset the reference app's note shows. Each source
//! line is one line on the card; headings, lists, task items, quotes, code blocks and rules get
//! their block look, and bold, italic, strike, code and links work inside a line.

use crate::prelude::*;
use egui::text::{LayoutJob, TextFormat};

/// Body text size and line height (CSS line-height 1.5), in points.
pub(crate) const NOTE_FONT: f32 = 14.0;
pub(crate) const NOTE_LINE: f32 = 1.5;

#[derive(Debug, PartialEq)]
pub(crate) enum Mark {
    Bullet,
    /// The number as typed, dot included ("2.").
    Number(String),
    Task(bool),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Line<'a> {
    Blank,
    Rule,
    Heading(usize, &'a str),
    /// `depth` is the indent in steps of two spaces.
    Item { depth: usize, mark: Mark, text: &'a str },
    Quote(&'a str),
    Code(&'a str),
    Para(&'a str),
}

/// The heading level of a line (1-6), or 0 when it is not a heading.
pub(crate) fn heading_level(line: &str) -> usize {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') { hashes } else { 0 }
}

/// Size of a heading level over the body text: h1 +8 points, h2 +5, h3 +3, h4 +1, h5 and h6 the body size.
pub(crate) fn size_of(level: usize, base: f32) -> f32 {
    base + match level {
        1 => 8.0,
        2 => 5.0,
        3 => 3.0,
        4 => 1.0,
        _ => 0.0,
    }
}

/// Every source line in order. Fence lines are not shown; the code between them is.
pub(crate) fn lines(text: &str) -> Vec<Line<'_>> {
    let mut code = false;
    let mut out = Vec::new();
    for raw in text.lines() {
        let t = raw.trim_start();
        if t.starts_with("```") {
            code = !code;
            continue;
        }
        let depth = (raw.len() - t.len()) / 2;
        let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        let level = heading_level(t);
        out.push(if code {
            Line::Code(raw)
        } else if t.is_empty() {
            Line::Blank
        } else if compact.len() >= 3 && compact.chars().all(|c| matches!(c, '-' | '*' | '_')) {
            Line::Rule
        } else if level > 0 {
            Line::Heading(level, t[level + 1..].trim())
        } else if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")) {
            match rest.strip_prefix("[ ] ").map(|r| (false, r)).or_else(|| rest.strip_prefix("[x] ").or_else(|| rest.strip_prefix("[X] ")).map(|r| (true, r))) {
                Some((done, text)) => Line::Item { depth, mark: Mark::Task(done), text },
                None => Line::Item { depth, mark: Mark::Bullet, text: rest },
            }
        } else if let Some(n) = ordered(t) {
            Line::Item { depth, mark: Mark::Number(format!("{}.", &t[..n])), text: &t[n + 2..] }
        } else if let Some(rest) = t.strip_prefix('>') {
            Line::Quote(rest.trim_start())
        } else {
            Line::Para(t)
        });
    }
    out
}

/// The digit count of an ordered item's number ("12. text" gives 2), when the line is one.
fn ordered(t: &str) -> Option<usize> {
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    matches!(t.get(digits..digits + 2), Some(". " | ") ")).then_some(digits)
}

/// A run of text sharing one style.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Span {
    pub(crate) text: String,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) strike: bool,
    pub(crate) code: bool,
    pub(crate) link: bool,
}

/// The inline styles of one line. `bold` starts the run in bold (headings).
pub(crate) fn inline(text: &str, bold: bool) -> Vec<Span> {
    let mut out = Vec::new();
    let mut style = Span { bold, ..Span::default() };
    let mut buf = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        let special = code(rest)
            .map(|(t, after)| (Span { text: t.into(), code: true, ..style.clone() }, after))
            .or_else(|| link(rest).map(|(t, after)| (Span { text: t.into(), link: true, ..style.clone() }, after)));
        if let Some((span, after)) = special {
            flush(&mut out, &mut buf, &style);
            out.push(span);
            rest = after;
        } else if let Some(after) = rest.strip_prefix("**") {
            flush(&mut out, &mut buf, &style);
            style.bold = !style.bold;
            rest = after;
        } else if let Some(after) = rest.strip_prefix("~~") {
            flush(&mut out, &mut buf, &style);
            style.strike = !style.strike;
            rest = after;
        } else if let Some(after) = rest.strip_prefix('*') {
            flush(&mut out, &mut buf, &style);
            style.italic = !style.italic;
            rest = after;
        } else {
            let c = rest.chars().next().unwrap_or_default();
            buf.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    flush(&mut out, &mut buf, &style);
    out
}

fn flush(out: &mut Vec<Span>, buf: &mut String, style: &Span) {
    if !buf.is_empty() {
        out.push(Span { text: std::mem::take(buf), ..style.clone() });
    }
}

/// `` `code` `` at the start of the text: its words and what follows.
fn code(rest: &str) -> Option<(&str, &str)> {
    let inner = rest.strip_prefix('`')?;
    let end = inner.find('`')?;
    Some((&inner[..end], &inner[end + 1..]))
}

/// `[label](url)` at the start of the text: the label and what follows.
fn link(rest: &str) -> Option<(&str, &str)> {
    let inner = rest.strip_prefix('[')?;
    let mid = inner.find("](")?;
    let close = inner[mid..].find(')')? + mid;
    Some((&inner[..mid], &inner[close + 1..]))
}

/// The spans as one layout, `size` and `wrap` in screen points. `bold_only` keeps just the bold
/// spans in `ink` and makes the rest transparent: egui has no bold face, so the caller paints
/// that layer twice, slightly apart.
pub(crate) fn job(spans: &[Span], size: f32, ink: Color32, wrap: f32, bold_only: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    for s in spans {
        let color = if bold_only && !s.bold { Color32::TRANSPARENT } else { ink };
        job.append(
            &s.text,
            0.0,
            TextFormat {
                font_id: FontId::monospace(if s.code { size * 0.95 } else { size }),
                color,
                italics: s.italic,
                line_height: Some(size * NOTE_LINE),
                strikethrough: if s.strike { Stroke::new(1.0, color) } else { Stroke::NONE },
                underline: if s.link { Stroke::new(1.0, ink.gamma_multiply(0.4)) } else { Stroke::NONE },
                background: if s.code { ink.gamma_multiply(0.1) } else { Color32::TRANSPARENT },
                ..Default::default()
            },
        );
    }
    job
}

/// The raw text while the editor types over it: each line at its heading's size, the markers as typed.
pub(crate) fn edit_job(text: &str, size: f32, ink: Color32, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    for line in text.split_inclusive('\n') {
        let s = size_of(heading_level(line.trim_start()), size);
        job.append(line, 0.0, TextFormat { font_id: FontId::monospace(s), color: ink, line_height: Some(s * NOTE_LINE), ..Default::default() });
    }
    job
}
