//! The open file as the SwiftUI editor reads it. `filetree:<id>:doc` is what the active tab holds and changes only when the file is
//! opened, switched to or changed by Rust (the text goes along, once per `loadRev`); `filetree:<id>:live` is what the text's edits
//! change (the colors, the change markers, the problems), published by the text's hash. Offsets are UTF-16 units, as the
//! NSTextView counts them; Swift's edits come back to Rust as ranges (`ui.filetree.edit`), and Rust owns the buffer and the saving.

use super::snapshot::kind_id;
use crate::app::filetree::compare::{Compare, Item, word_spans};
use crate::app::filetree::editor::Body;
use crate::app::filetree::find::{FindFiles, count_note};
use crate::editor::highlight::{self, St};
use crate::editor::{RowKind, word_marks};
use crate::prelude::*;
use std::ops::Range;
use std::path::Path;

/// A text up to this many bytes gets the colors of the whole file; a bigger one only of the lines near the view.
const WHOLE_MAX: usize = 120_000;
/// The lines kept colored above and below the view of a big file.
const AROUND: usize = 300;

/// A change of one row of the compare tab, both sides, as `Compare` aligned them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompareRow {
    /// "same", "insert", "delete", "change" or "fold" (unchanged lines hidden).
    kind: &'static str,
    left_number: Option<usize>,
    right_number: Option<usize>,
    left: Option<String>,
    right: Option<String>,
    left_words: Vec<[usize; 2]>,
    right_words: Vec<[usize; 2]>,
    fold_key: Option<usize>,
    fold_count: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HitOut {
    path: String,
    rel: String,
    name: String,
    line: usize,
    col: usize,
    len: usize,
    text: String,
}

/// The active tab's body. `kind` says which of the optional parts are there.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct Doc {
    /// "none", "text", "media", "refused", "external", "diff", "compare" or "results".
    pub(super) kind: &'static str,
    pub(super) path: String,
    pub(super) load_rev: u64,
    pub(super) text: Option<String>,
    pub(super) lang: Option<String>,
    /// A refusal's or an external file's words.
    pub(super) message: Option<String>,
    pub(super) pdf: bool,
    pub(super) size_text: Option<String>,
    /// A commit's diff, as git wrote it.
    pub(super) diff: Option<String>,
    pub(super) compare: Option<CompareOut>,
    pub(super) results: Option<ResultsOut>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompareOut {
    file: String,
    head: String,
    unified: bool,
    collapse: bool,
    current: usize,
    /// The row each change starts at.
    blocks: Vec<usize>,
    removed: usize,
    added: usize,
    rows: Vec<CompareRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ResultsOut {
    title: String,
    count_note: String,
    selected: usize,
    hits: Vec<HitOut>,
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The words of `text` that differ from `other`'s line, as UTF-16 ranges (`word_spans` drops a line changed nearly whole).
fn words(text: &str, marks: Vec<Range<usize>>) -> Vec<[usize; 2]> {
    word_spans(text, marks).into_iter().map(|span| [utf16_len(&text[..span.start]), utf16_len(&text[..span.end])]).collect()
}

/// The columns as rows Swift draws side by side. Each column holds only its own lines now (`compare/mod.rs`), so they
/// are laid out again row for row: the unchanged stretches line up, and in a change the two sides face each other, the
/// longer side's extra lines across from empty rows. In one column each item is a whole row. Also where each change starts.
fn compare_pairs(compare: &Compare) -> (Vec<(Option<&Item>, Option<&Item>)>, Vec<usize>) {
    let [left, right] = &compare.items;

    if compare.unified {
        return (left.iter().map(|item| (Some(item), None)).collect(), compare.blocks.iter().map(|block| block.rows[0].start).collect());
    }

    let (mut pairs, mut starts, mut at) = (Vec::with_capacity(left.len().max(right.len())), vec![], [0, 0]);

    let lay = |pairs: &mut Vec<_>, at: &mut [usize; 2], ends: [usize; 2]| {
        while at[0] < ends[0] || at[1] < ends[1] {
            let (has_left, has_right) = (at[0] < ends[0], at[1] < ends[1]);

            pairs.push((has_left.then(|| &left[at[0]]), has_right.then(|| &right[at[1]])));
            at[0] += usize::from(has_left);
            at[1] += usize::from(has_right);
        }
    };

    for block in &compare.blocks {
        lay(&mut pairs, &mut at, [block.rows[0].start, block.rows[1].start]);
        starts.push(pairs.len());
        lay(&mut pairs, &mut at, [block.rows[0].end, block.rows[1].end]);
    }

    lay(&mut pairs, &mut at, [left.len(), right.len()]);

    (pairs, starts)
}

pub(super) fn compare_out(compare: &Compare) -> CompareOut {
    let old: Vec<&str> = compare.old.lines().collect();
    let new: Vec<&str> = compare.new.lines().collect();
    let line = |lines: &[&str], index: Option<usize>| index.and_then(|i| lines.get(i)).map(|text| text.to_string());
    let number = |index: Option<usize>| index.map(|i| i + 1);

    let kind = |row: RowKind| match row {
        RowKind::Same => "same",
        RowKind::Insert => "insert",
        RowKind::Delete => "delete",
        RowKind::Change => "change",
    };

    let (pairs, blocks) = compare_pairs(compare);
    let mut rows = Vec::with_capacity(pairs.len());

    for pair in pairs {
        let (left_row, right_row, partner) = match pair {
            (Some(Item::Fold { key, count, .. }), _) | (None, Some(Item::Fold { key, count, .. })) => {
                rows.push(CompareRow {
                    kind: "fold",
                    left_number: None,
                    right_number: None,
                    left: None,
                    right: None,
                    left_words: vec![],
                    right_words: vec![],
                    fold_key: Some(*key),
                    fold_count: Some(*count),
                });

                continue;
            }
            // one column: the row has both sides
            (Some(Item::Line { row, partner }), None) if compare.unified => (Some(*row), Some(*row), *partner),
            (left, right) => {
                let side = |item: Option<&Item>| match item {
                    Some(Item::Line { row, partner }) => Some((*row, *partner)),
                    _ => None,
                };

                let (left, right) = (side(left), side(right));

                (left.map(|(row, _)| row), right.map(|(row, _)| row), left.or(right).and_then(|(_, partner)| partner))
            }
        };

        let row_kind = left_row.or(right_row).map_or(RowKind::Same, |row| row.kind);
        let (left_at, right_at) = (left_row.and_then(|row| row.left), right_row.and_then(|row| row.right));
        let (left, right) = (line(&old, left_at), line(&new, right_at));
        let (mut left_words, mut right_words) = (vec![], vec![]);

        // a modified line against the line it pairs with: the changed words
        if row_kind == RowKind::Change {
            if let (Some(a), Some(b)) = (&left, &right) {
                let (o, n) = word_marks(a, b);

                (left_words, right_words) = (words(a, o), words(b, n));
            } else if let (Some(a), Some(p)) = (&left, partner.and_then(|p| new.get(p))) {
                left_words = words(a, word_marks(a, p).0);
            } else if let (Some(b), Some(p)) = (&right, partner.and_then(|p| old.get(p))) {
                right_words = words(b, word_marks(p, b).1);
            }
        }

        rows.push(CompareRow {
            kind: kind(row_kind),
            left_number: number(left_at),
            right_number: number(right_at),
            left,
            right,
            left_words,
            right_words,
            fold_key: None,
            fold_count: None,
        });
    }

    let (removed, added) = compare.totals();

    CompareOut {
        file: compare.file.to_string_lossy().into_owned(),
        head: compare.head.clone(),
        unified: compare.unified,
        collapse: compare.collapse,
        current: compare.cur,
        blocks,
        removed,
        added,
        rows,
    }
}

fn hits_out(root: &Path, hits: &[crate::editor::find::Hit]) -> Vec<HitOut> {
    hits.iter()
        .map(|hit| HitOut {
            path: hit.path.to_string_lossy().into_owned(),
            rel: hit.path.strip_prefix(root).unwrap_or(&hit.path).to_string_lossy().replace('\\', "/"),
            name: hit.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            line: hit.line,
            col: hit.col,
            len: hit.len,
            text: hit.text.trim_end().chars().take(300).collect(),
        })
        .collect()
}

/// A tab's body as `Doc` (the text of a text tab is added by the caller, which knows its revision).
pub(super) fn doc_of(body: &Body, path: &Path, root: &Path) -> Doc {
    let mut doc = Doc { path: path.to_string_lossy().into_owned(), ..Default::default() };

    match body {
        Body::Text(buf) => {
            doc.kind = "text";
            doc.lang = Some(buf.lang.id.to_string());
        }
        Body::Media(_) => {
            doc.kind = "media";
            doc.pdf = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
            doc.size_text = std::fs::metadata(path).ok().map(|m| crate::app::filetree::editor::bytes(m.len()));
        }
        Body::Refused(why) => (doc.kind, doc.message) = ("refused", Some(why.clone())),
        Body::External(why) => (doc.kind, doc.message) = ("external", Some(why.clone())),
        Body::Diff(text) => (doc.kind, doc.diff) = ("diff", Some(text.clone())),
        Body::Compare(compare) => (doc.kind, doc.compare) = ("compare", Some(compare_out(compare))),
        Body::Results(results) => {
            doc.kind = "results";

            doc.results = Some(ResultsOut {
                title: results.title(),
                count_note: count_note(&results.hits),
                selected: results.selected,
                hits: hits_out(root, &results.hits),
            });
        }
    }

    doc
}

/// What changes the doc besides its text's revision: a cheap key of the body, to publish it again only when it moved.
pub(super) fn body_key(body: &Body) -> String {
    match body {
        Body::Text(_) => String::new(),
        Body::Media(_) => "media".into(),
        Body::Refused(why) | Body::External(why) => why.clone(),
        Body::Diff(text) => text.len().to_string(),
        Body::Compare(c) => {
            format!("{}:{}:{}:{}:{}:{}", crate::editor::text::hash(&c.new, 0.0), c.old.len(), c.unified, c.collapse, c.cur, c.items[0].len())
        }
        Body::Results(r) => format!("{}:{}:{}", r.hits.len(), r.selected, r.title()),
    }
}

/// The byte offset of UTF-16 offset `at` in `text` (None past the end or inside a character).
pub(super) fn byte_of(text: &str, at: usize) -> Option<usize> {
    let mut units = 0;

    for (byte, character) in text.char_indices() {
        if units == at {
            return Some(byte);
        }

        if units > at {
            return None;
        }

        units += character.len_utf16();
    }

    (units == at).then_some(text.len())
}

/// The lines `from` to `to` (half open) as byte offsets.
fn line_bytes(text: &str, from: usize, to: usize) -> (usize, usize) {
    let (mut start, mut end, mut line) = (None, text.len(), 0);

    if from == 0 {
        start = Some(0);
    }

    for (byte, _) in text.match_indices('\n') {
        line += 1;

        if line == from {
            start = Some(byte + 1);
        }

        if line == to {
            end = byte;

            break;
        }
    }

    (start.unwrap_or(text.len()), end)
}

/// The spans (UTF-16 start, end, kind) of the tokens in the lines `from..to`, and the UTF-16 range those lines cover.
pub(super) fn spans(text: &str, tokens: &[(usize, usize, St)], lines: (usize, usize)) -> (Vec<u32>, [usize; 2]) {
    let (start, end) = if text.len() <= WHOLE_MAX { (0, text.len()) } else { line_bytes(text, lines.0.saturating_sub(AROUND), lines.1 + AROUND) };
    let (mut byte, mut units) = (start, utf16_len(&text[..start]));
    let from = units;
    let mut out = Vec::new();

    for &(a, b, style) in &tokens[tokens.partition_point(|token| token.1 <= start)..] {
        if a >= end {
            break;
        }

        if a < byte || b > text.len() || !text.is_char_boundary(a) || !text.is_char_boundary(b) {
            continue;
        }

        units += utf16_len(&text[byte..a]);
        let length = utf16_len(&text[a..b]);

        if kind_id(style) != 0 {
            out.extend([units as u32, (units + length) as u32, kind_id(style)]);
        }

        (byte, units) = (b, units + length);
    }

    (out, [from, from + utf16_len(&text[start..end])])
}

pub(super) fn tokens_of(text: &str, path: &Path) -> Vec<(usize, usize, St)> {
    highlight::tokens(highlight::detect(path), text)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HunkOut {
    start: usize,
    end: usize,
    kind: &'static str,
    /// The lines as the last commit has them (cut at 4000 characters).
    old: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiagOut {
    line: usize,
    col: usize,
    end_line: usize,
    end_col: usize,
    severity: u8,
    message: String,
}

#[derive(Serialize, Clone, Copy)]
pub(super) struct Jump {
    pub(super) line: usize,
    pub(super) col: usize,
    pub(super) seq: u64,
    pub(super) focus: bool,
}

/// What the text's edits change: sent when the key of `(text, view, rest)` moves.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Live {
    pub(super) path: String,
    pub(super) load_rev: u64,
    pub(super) applied_rev: u64,
    pub(super) dirty: bool,
    pub(super) external: bool,
    pub(super) has_head: bool,
    pub(super) hunks: Vec<HunkOut>,
    pub(super) diags: Vec<DiagOut>,
    pub(super) jump: Option<Jump>,
    /// Counts up each time the text is to take the keyboard (a file opened for good).
    pub(super) focus_seq: u64,
    /// UTF-16 range the spans cover (all of a small file).
    pub(super) span_range: [usize; 2],
    /// Triples: start, end, kind (see `filetree:theme`).
    pub(super) spans: Vec<u32>,
}

pub(super) fn hunks_out(buf: &crate::app::filetree::editor::TextBuf) -> Vec<HunkOut> {
    let (Some((_, hunks)), Some(head)) = (&buf.hunks, &buf.head) else { return vec![] };

    hunks
        .iter()
        .map(|hunk| {
            let old: String = head.split_inclusive('\n').skip(hunk.old.start).take(hunk.old.len()).collect();

            HunkOut {
                start: hunk.new.start,
                end: hunk.new.end,
                kind: match hunk.change() {
                    crate::app::filetree::diff::Change::Added => "added",
                    crate::app::filetree::diff::Change::Modified => "modified",
                    crate::app::filetree::diff::Change::Deleted => "deleted",
                },
                old: old.chars().take(4000).collect(),
            }
        })
        .collect()
}

pub(super) fn diags_out(diags: Vec<crate::editor::lsp::Diag>) -> Vec<DiagOut> {
    diags
        .into_iter()
        .take(500)
        .map(|d| DiagOut { line: d.line, col: d.col, end_line: d.end_line, end_col: d.end_col, severity: d.severity, message: d.message })
        .collect()
}

/// "Localizar nos Arquivos" as a topic (`filetree:<id>:find`; null while it is closed).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FindOut {
    query: String,
    case: bool,
    words: bool,
    regex: bool,
    multiline: bool,
    mask_on: bool,
    mask: String,
    in_dir: bool,
    folder: String,
    selected: usize,
    searching: bool,
    error: Option<String>,
    count_note: String,
    hits: Vec<HitOut>,
    /// Counts up each time the field is to take the keyboard.
    focus_seq: u64,
}

pub(super) fn find_out(find: &FindFiles, root: &Path, focus_seq: u64) -> FindOut {
    FindOut {
        query: find.query.clone(),
        case: find.case,
        words: find.words,
        regex: find.regex,
        multiline: find.multiline,
        mask_on: find.mask_on,
        mask: find.mask.clone(),
        in_dir: find.in_dir,
        folder: find.folder.strip_prefix(root).unwrap_or(&find.folder).to_string_lossy().replace('\\', "/"),
        selected: find.selected,
        searching: find.searching(),
        // a regular expression that does not compile (what the popup shows under the field)
        error: find.ask().needle().err().filter(|_| !find.query.is_empty()),
        count_note: if find.query.is_empty() { String::new() } else { count_note(&find.hits) },
        hits: hits_out(root, &find.hits),
        focus_seq,
    }
}
