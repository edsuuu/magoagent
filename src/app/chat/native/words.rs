//! The agent's words for SwiftUI, cut as the egui chat cuts them (chat/segments.rs, markdown.rs): code blocks, pictures
//! and files' cards (media.rs), and the words in paragraphs (a bubble each) and tables, line by line (heading, quote,
//! list item), each line in runs of **bold**, *italics* and `code` with the files it names (filetree/link.rs) found.
//! SwiftUI only paints them.

use crate::app::chat::markdown::{blocks, emphasis, table_cells};
use crate::app::chat::segments::{Seg, data_uri, file_size, is_local, is_picture, resolve, segments};
use crate::app::filetree::link::{FileLink, file_link_at, opens_in_tree, path_like, trimmed};
use crate::prelude::*;
use std::path::Path;

/// A piece of the agent's words: text (markdown), code, image (a picture on disk), web (one on the web) or file (a card).
#[derive(Serialize, PartialEq, Debug, Default)]
pub(super) struct Part {
    pub(super) kind: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) text: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) info: String,
    /// A file card's kind and weight ("RS · 12 KB").
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) detail: String,
    /// How a file card opens: tree, note or default; "" when it isn't there.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub(super) opens: &'static str,
    /// The words: paragraphs and tables.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) paras: Vec<Para>,
}

/// A paragraph (its lines, in a bubble) or a table (its rows of cells, the first the header).
#[derive(Serialize, PartialEq, Debug)]
pub(super) struct Para {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) lines: Vec<Line>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) rows: Vec<Vec<Vec<Run>>>,
}

/// One line: text, heading (`level` 1 to 3), quote or item (its `mark`, "•" or "2.", and `level` its indentation).
#[derive(Serialize, PartialEq, Debug)]
pub(super) struct Line {
    pub(super) kind: &'static str,
    #[serde(skip_serializing_if = "is_zero")]
    pub(super) level: usize,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) mark: String,
    pub(super) runs: Vec<Run>,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[derive(Serialize, PartialEq, Debug, Default)]
pub(super) struct Run {
    pub(super) text: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(super) bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(super) italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(super) code: bool,
    /// A file it names, there: a click opens it in the Árvore (MCP `file_link`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) line: Option<usize>,
}

/// The runs of one line, as markdown.rs's `md_runs` lays them out: `code` (a file when it names one), **bold** (all of
/// it with `bold`), *italics*, and the paths among the words that name files.
pub(super) fn runs(text: &str, bold: bool, find: &dyn Fn(&str) -> Option<FileLink>) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();

    let mut add = |text: &str, bold: bool, italic: bool, code: bool, link: Option<FileLink>| {
        if text.is_empty() {
            return;
        }

        let (path, line) = link.map_or((None, None), |link| (Some(link.path.display().to_string()), link.line));

        // the same look as the run before, no file: one run
        if let Some(last) = out.last_mut().filter(|last| (last.bold, last.italic, last.code) == (bold, italic, code) && last.path.is_none() && path.is_none()) {
            last.text.push_str(text);

            return;
        }

        out.push(Run { text: text.into(), bold, italic, code, path, line });
    };

    // inside **…**; `code` spans don't reset it
    let mut on = false;

    for (index, piece) in text.split('`').enumerate() {
        if index % 2 == 1 {
            add(piece, false, false, true, find(piece.trim()));

            continue;
        }

        for (count, run) in piece.split("**").enumerate() {
            if count > 0 {
                on = !on;
            }

            for (words, italic) in emphasis(run) {
                let thick = bold || on;
                let mut rest = words;

                while !rest.is_empty() {
                    let gap = rest.len() - rest.trim_start().len();
                    let word_end = rest[gap..].find(char::is_whitespace).map_or(rest.len(), |end| gap + end);
                    let (start, token) = trimmed(&rest[gap..word_end]);

                    match (token.contains(['/', '\\', '.']) && path_like(token)).then(|| find(token)).flatten() {
                        Some(link) => {
                            let (before, after) = (gap + start, gap + start + token.len());

                            add(&rest[..before], thick, italic, false, None);
                            add(token, thick, italic, false, Some(link));
                            add(&rest[after..word_end], thick, italic, false, None);
                        }
                        None => add(&rest[..word_end], thick, italic, false, None),
                    }

                    rest = &rest[word_end..];
                }
            }
        }
    }

    out
}

/// One run of words in paragraphs and tables (markdown.rs `prose_blocks` and `md_lines`).
pub(super) fn paras(words: &str, find: &dyn Fn(&str) -> Option<FileLink>) -> Vec<Para> {
    let lines: Vec<&str> = words.trim_matches('\n').lines().map(str::trim_end).collect();

    blocks(&lines)
        .into_iter()
        .map(|(is_table, range)| {
            let block = &lines[range];

            if is_table {
                let rows: Vec<&str> = block.iter().map(|line| line.trim_start()).collect();
                let rows = table_cells(&rows).into_iter().enumerate().map(|(index, row)| row.into_iter().map(|cell| runs(cell, index == 0, find)).collect());

                return Para { lines: Vec::new(), rows: rows.collect() };
            }

            Para { lines: block.iter().map(|line| md_line(line, find)).collect(), rows: Vec::new() }
        })
        .collect()
}

/// A line of a paragraph as markdown.rs's `md_lines` reads it.
fn md_line(line: &str, find: &dyn Fn(&str) -> Option<FileLink>) -> Line {
    let words = line.trim_start();
    let indent = line.len() - words.len();
    let hashes = words.chars().take_while(|&letter| letter == '#').count();

    if (1..=6).contains(&hashes) && words[hashes..].starts_with(' ') {
        return Line { kind: "heading", level: hashes.min(3), mark: String::new(), runs: runs(&words[hashes + 1..], true, find) };
    }

    if let Some(quote) = words.strip_prefix('>') {
        return Line { kind: "quote", level: 0, mark: String::new(), runs: runs(quote.trim_start(), false, find) };
    }

    let number = words.split_once(". ").filter(|(number, _)| !number.is_empty() && number.len() < 4 && number.bytes().all(|byte| byte.is_ascii_digit()));

    match (words.strip_prefix("- ").or(words.strip_prefix("* ")), number) {
        (Some(rest), _) => Line { kind: "item", level: indent, mark: "•".into(), runs: runs(rest, false, find) },
        (None, Some((number, rest))) => Line { kind: "item", level: indent, mark: format!("{number}."), runs: runs(rest, false, find) },
        _ => Line { kind: "text", level: 0, mark: String::new(), runs: runs(words, false, find) },
    }
}

/// The agent's words cut as the egui chat cuts them (chat/segments.rs), the pictures and files looked up from `base`.
pub(super) fn parts(text: &str, base: Option<&Path>, now: f64) -> Vec<Part> {
    let find = |token: &str| file_link_at(token, base, now);

    segments(text)
        .into_iter()
        .map(|seg| match seg {
            Seg::Text(words) => Part { kind: "text", paras: paras(&words, &find), ..Default::default() },
            Seg::Code { info, code } => Part { kind: "code", text: code, info, ..Default::default() },
            Seg::Image { alt, src } => picture(&src, &alt, base),
            Seg::File { label, path, linked } => {
                let full = resolve(&path, base);

                // a path in backticks that isn't a file is just a word in the code colour (media.rs `agent_words_in`)
                if linked || full.exists() {
                    file_card(&full, &label)
                } else {
                    Part { kind: "text", paras: paras(&format!("`{path}`"), &find), ..Default::default() }
                }
            }
        })
        .collect()
}

/// A picture (media.rs `chat_picture`): one on disk shows; one on the web is a card that opens it in the browser; a file
/// that isn't a picture, its card.
pub(super) fn picture(src: &str, alt: &str, base: Option<&Path>) -> Part {
    // ponytail: a picture inside the message (a data address) is a card; its bytes would go in every snapshot
    if data_uri(src).is_some() {
        return Part {
            kind: "file",
            info: if alt.is_empty() { "Imagem".into() } else { alt.into() },
            detail: "Imagem na mensagem".into(),
            ..Default::default()
        };
    }

    if !is_local(src) {
        let host = src.split("://").nth(1).and_then(|rest| rest.split('/').next()).unwrap_or(src);

        return Part {
            kind: "web",
            text: src.into(),
            info: format!("Imagem de {host}"),
            detail: "Clique para abrir no navegador".into(),
            ..Default::default()
        };
    }

    let path = resolve(src, base);

    if path.is_file() && is_picture(&path.to_string_lossy()) {
        Part { kind: "image", text: path.display().to_string(), info: alt.into(), ..Default::default() }
    } else {
        file_card(&path, "")
    }
}

/// A file's card (media.rs `chat_file_card`): its name, kind and weight, and how a click opens it.
pub(super) fn file_card(path: &Path, label: &str) -> Part {
    let meta = fs::metadata(path).ok();

    let name =
        if label.is_empty() { path.file_name().map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy()).into_owned() } else { label.into() };

    let detail = match &meta {
        Some(meta) if meta.is_dir() => "Pasta".to_string(),
        Some(meta) => {
            let extension = path.extension().map(|extension| extension.to_string_lossy().to_uppercase());

            [extension, Some(file_size(meta.len()))].into_iter().flatten().collect::<Vec<_>>().join(" · ")
        }
        None => "Arquivo não encontrado".to_string(),
    };

    let opens = match &meta {
        None => "",
        Some(meta) if meta.is_file() && crate::app::notes::file::opens_as_note(path) => "note",
        Some(_) if opens_in_tree(path) => "tree",
        Some(_) => "default",
    };

    Part { kind: "file", text: path.display().to_string(), info: name, detail, opens, paras: Vec::new() }
}
