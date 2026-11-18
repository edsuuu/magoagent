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
    Item {
        depth: usize,
        mark: Mark,
        text: &'a str,
    },
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

/// Where the caret is while a note is typed (characters): `head` moves, `anchor` stays.
#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub(crate) struct Caret {
    pub(crate) head: usize,
    pub(crate) anchor: usize,
}

/// What the editor draws over a hidden mark, as the resting note does (at the mark's character).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Ornament {
    Bullet(usize),
    Task(usize, bool),
    Quote(usize),
    /// a rule's row, from its first character
    Rule(usize),
}

/// The typed text as the reference's live preview shows it: the marks of headings, bold, italic, strike, code and links
/// hide unless the caret is on them; a list's, a task's or a quote's mark gives way to the resting note's bullet, box or
/// bar, a rule to its line. Only what is being typed shows its markdown. `bold` is the same layout with only the bold
/// text inked (egui has no bold face: it is painted again half a point to the right).
pub(crate) struct LiveText {
    pub(crate) job: LayoutJob,
    pub(crate) bold: LayoutJob,
    pub(crate) ornaments: Vec<Ornament>,
}

/// One character's look in the editor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Look {
    level: usize,
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: bool,
    block: bool,
    /// faded (a quote, a done task: the share of the ink)
    fade: Option<f32>,
    /// a mark shown while the caret is on it
    dim: bool,
    /// out of sight but keeping its width (where an ornament goes)
    clear: bool,
    /// out of sight and of the line (next to no width)
    hidden: bool,
}

/// The note's text for the editor, `size` and `wrap` in screen points, `caret` while the editor has the keys.
pub(crate) fn live_text(text: &str, size: f32, ink: Color32, wrap: f32, caret: Option<Caret>) -> LiveText {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut looks = vec![Look::default(); chars.len()];
    let mut ornaments = Vec::new();
    let on_line = |from: usize, to: usize| caret.is_some_and(|c| c.head == c.anchor && (from..=to).contains(&c.head));
    let mut code = false;
    let mut start = 0;
    while start <= chars.len() {
        let end = (start..chars.len()).find(|&i| chars[i].1 == '\n').unwrap_or(chars.len());
        let body: String = chars[start..end].iter().map(|(_, c)| c).collect();
        let trimmed = body.trim_start();
        let at = start + (body.chars().count() - trimmed.chars().count());
        let line = &mut looks[start..end];
        let here = on_line(start, end);
        let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        let level = heading_level(trimmed);
        if trimmed.starts_with("```") {
            code = !code;
            line.iter_mut().for_each(|look| look.dim = true);
        } else if code {
            line.iter_mut().for_each(|look| look.block = true);
        } else if trimmed.is_empty() {
        } else if compact.chars().count() >= 3 && compact.chars().all(|c| matches!(c, '-' | '*' | '_')) {
            if here {
                line.iter_mut().for_each(|look| look.dim = true);
            } else {
                line.iter_mut().for_each(|look| look.clear = true);
                ornaments.push(Ornament::Rule(start));
            }
        } else if level > 0 {
            line.iter_mut().for_each(|look| look.level = level);
            mark(&mut looks[at..at + level + 1], here);
            inline_looks(&chars, &mut looks, at + level + 1, end, Look { level, bold: true, ..Look::default() }, caret);
        } else if let Some(rest) = ["- ", "* ", "+ "].iter().find_map(|m| trimmed.strip_prefix(m)) {
            let task = ["[ ] ", "[x] ", "[X] "].iter().position(|t| rest.starts_with(t)).map(|kind| kind > 0);
            match task {
                Some(done) => {
                    mark(&mut looks[at..at + 2], here);
                    if here {
                        looks[at + 2..at + 5].iter_mut().for_each(|look| look.dim = true);
                    } else {
                        // the box takes one character's place, as close to the text as the resting note's
                        (looks[at + 2].hidden, looks[at + 3].clear, looks[at + 4].hidden) = (true, true, true);
                        ornaments.push(Ornament::Task(at + 3, done));
                    }
                    let base = if done { Look { strike: true, fade: Some(0.5), ..Look::default() } } else { Look::default() };
                    inline_looks(&chars, &mut looks, at + 6, end, base, caret);
                }
                None => {
                    if here {
                        looks[at].dim = true;
                    } else {
                        looks[at].clear = true;
                        ornaments.push(Ornament::Bullet(at));
                    }
                    inline_looks(&chars, &mut looks, at + 2, end, Look::default(), caret);
                }
            }
        } else if let Some(digits) = ordered(trimmed) {
            looks[at..at + digits + 1].iter_mut().for_each(|look| look.fade = Some(0.4));
            inline_looks(&chars, &mut looks, at + digits + 2, end, Look::default(), caret);
        } else if trimmed.starts_with('>') {
            if here {
                looks[at].dim = true;
            } else {
                looks[at].clear = true;
                ornaments.push(Ornament::Quote(at));
            }
            looks[at + 1..end].iter_mut().for_each(|look| look.fade = Some(0.7));
            inline_looks(&chars, &mut looks, at + 1, end, Look { fade: Some(0.7), ..Look::default() }, caret);
        } else {
            inline_looks(&chars, &mut looks, at, end, Look::default(), caret);
        }
        // the line's end keeps its line's height
        let line_level = looks[start..end].first().map_or(0, |look| look.level);
        if let Some(look) = looks.get_mut(end) {
            *look = Look { level: line_level, ..Look::default() };
        }
        start = end + 1;
    }
    let build = |bold_only: bool| {
        let mut job = LayoutJob::default();
        job.wrap.max_width = wrap;
        let mut from = 0;
        while from < chars.len() {
            let look = looks[from];
            let to = (from..chars.len()).find(|&i| looks[i] != look).unwrap_or(chars.len());
            let bytes = chars[from].0..chars.get(to).map_or(text.len(), |(byte, _)| *byte);
            job.append(&text[bytes], 0.0, format_of(look, size, ink, bold_only));
            from = to;
        }
        job
    };
    LiveText { job: build(false), bold: build(true), ornaments }
}

/// A block's mark (a heading's hashes, a list's dash): shown faint while the caret is on its line, out of the line otherwise.
fn mark(looks: &mut [Look], here: bool) {
    for look in looks {
        if here {
            look.dim = true;
        } else {
            look.hidden = true;
        }
    }
}

/// Bold, italic, strike, code and links over `from..to` of a line, from `base`; their marks hide unless the caret is
/// on them (inside the run they open and close).
fn inline_looks(chars: &[(usize, char)], looks: &mut [Look], from: usize, to: usize, base: Look, caret: Option<Caret>) {
    let in_range =
        |from: usize, to: usize| caret.is_some_and(|c| if c.head == c.anchor { (from..=to).contains(&c.head) } else { c.anchor > from && c.anchor < to });
    let reveal = |looks: &mut [Look], marks: &[std::ops::Range<usize>], shown: bool| {
        for range in marks {
            for look in &mut looks[range.clone()] {
                (look.dim, look.hidden) = (shown, !shown);
            }
        }
    };
    let find = |from: usize, wanted: &str| {
        let pattern: Vec<char> = wanted.chars().collect();
        (from..to).find(|&i| i + pattern.len() <= to && chars[i..i + pattern.len()].iter().map(|(_, c)| *c).eq(pattern.iter().copied()))
    };
    let starts = |i: usize, wanted: &str| find(i, wanted) == Some(i);
    let mut style = base;
    // where the open runs started: bold, strike, italic
    let mut open: [Option<usize>; 3] = [None; 3];
    let mut i = from;
    while i < to {
        if chars[i].1 == '`'
            && let Some(close) = find(i + 1, "`")
        {
            looks[i + 1..close].iter_mut().for_each(|look| *look = Look { code: true, ..style });
            looks[i] = style;
            looks[close] = style;
            reveal(looks, &[i..i + 1, close..close + 1], in_range(i, close + 1));
            i = close + 1;
        } else if chars[i].1 == '['
            && let Some(middle) = find(i + 1, "](")
            && let Some(close) = find(middle + 2, ")")
        {
            looks[i..=close].iter_mut().for_each(|look| *look = Look { link: true, ..style });
            reveal(looks, &[i..i + 1, middle..close + 1], in_range(i, close + 1));
            // the label stays a link either way
            looks[i + 1..middle].iter_mut().for_each(|look| (look.dim, look.hidden) = (false, false));
            i = close + 1;
        } else if let Some((kind, width)) = [("**", 0), ("~~", 1), ("*", 2)].iter().find(|(m, _)| starts(i, m)).map(|(m, kind)| (*kind, m.len())) {
            looks[i..i + width].iter_mut().for_each(|look| *look = style);
            match open[kind].take() {
                Some(opened) => reveal(looks, &[opened..opened + width, i..i + width], in_range(opened, i + width)),
                None => open[kind] = Some(i),
            }
            match kind {
                0 => style.bold = !style.bold,
                1 => style.strike = !style.strike,
                _ => style.italic = !style.italic,
            }
            i += width;
        } else {
            looks[i] = style;
            i += 1;
        }
    }
    // a run never closed goes to the line's end
    for (kind, opened) in open.iter().enumerate() {
        if let Some(opened) = *opened {
            let width = if kind == 2 { 1 } else { 2 };
            reveal(looks, std::slice::from_ref(&(opened..opened + width)), in_range(opened, to));
        }
    }
}

/// A character's format: its heading's size and line, faint, faded or out of sight.
fn format_of(look: Look, size: f32, ink: Color32, bold_only: bool) -> TextFormat {
    let line_size = size_of(look.level, size);
    let mut color = match look.fade {
        Some(share) => ink.gamma_multiply(share),
        None if look.dim => ink.gamma_multiply(0.4),
        None => ink,
    };
    if look.clear || look.hidden || (bold_only && !look.bold) {
        color = Color32::TRANSPARENT;
    }
    let font = if look.hidden {
        0.1
    } else if look.code {
        line_size * 0.95
    } else {
        line_size
    };
    let painted = !bold_only && !look.hidden && !look.clear;
    TextFormat {
        font_id: FontId::monospace(font),
        color,
        italics: look.italic,
        line_height: Some(line_size * NOTE_LINE),
        strikethrough: if painted && look.strike { Stroke::new(1.0, color) } else { Stroke::NONE },
        underline: if painted && look.link { Stroke::new(1.0, ink.gamma_multiply(0.4)) } else { Stroke::NONE },
        background: if painted && look.code {
            ink.gamma_multiply(0.1)
        } else if painted && look.block {
            ink.gamma_multiply(0.05)
        } else {
            Color32::TRANSPARENT
        },
        ..Default::default()
    }
}
