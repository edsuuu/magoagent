//! The agent's words in the chat (and the IDE's docs): the light Markdown it writes, each
//! paragraph a bubble, tables a grid, code blocks a card.

use crate::app::chat::{bubble_fill, tint};
use crate::app::chat_list::code_card;
use crate::prelude::*;

/// The agent's words, in the light Markdown it writes: ``` blocks, headings, lists, quotes,
/// tables, **bold** and `code`. Each block of code is a card; the words are in `prose_blocks`.
/// `bubbles`: each run of words in a bubble of its own (the chat); off, plain text (the IDE's docs).
pub(crate) fn prose(ui: &mut egui::Ui, text: &str, c: &UiColors, dark: bool, width: f32, bubbles: bool) {
    for (k, part) in text.split("```").enumerate() {
        if k % 2 == 1 {
            // the first line of a fence names the language, and maybe the file
            let (info, code) = part.split_once('\n').unwrap_or(("", part));
            code_card(ui, info.trim(), code.trim_end(), c, dark);
            continue;
        }
        prose_blocks(ui, part, c, dark, width, bubbles);
    }
}

/// The blocks of one run of words: each paragraph a bubble of its own, each table a card in the
/// same column.
fn prose_blocks(ui: &mut egui::Ui, part: &str, c: &UiColors, dark: bool, width: f32, bubbles: bool) {
    let lines: Vec<&str> = part.trim_matches('\n').lines().map(str::trim_end).collect();
    for (is_table, range) in blocks(&lines) {
        let block = &lines[range];
        if is_table {
            let rows: Vec<&str> = block.iter().map(|l| l.trim_start()).collect();
            table(ui, &rows, c, dark);
        } else if !bubbles {
            md_lines(ui, block, c, dark);
        } else {
            egui::Frame::new().fill(bubble_fill(dark, false)).corner_radius(16).inner_margin(egui::Margin::symmetric(12, 7)).show(ui, |ui| {
                // the bubble hugs its words: up to 720 wide, and 48 pt short of the column's edge
                ui.set_max_width((width - 48.0).min(720.0) - 24.0);
                md_lines(ui, block, c, dark);
            });
        }
    }
}

/// Splits the lines of a run of words into blocks: (is a table, its lines). Blank lines end a
/// paragraph; a table starts at a `|` line whose next line is its |---| row.
fn blocks(lines: &[&str]) -> Vec<(bool, std::ops::Range<usize>)> {
    let table_at = |i: usize| lines[i].trim_start().starts_with('|') && lines.get(i + 1).is_some_and(|l| rule(l.trim_start()));
    let mut out = vec![];
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim().is_empty() {
            i += 1;
            continue;
        }
        let (start, table) = (i, table_at(i));
        if table {
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                i += 1;
            }
        } else {
            while i < lines.len() && !lines[i].trim().is_empty() && !table_at(i) {
                i += 1;
            }
        }
        out.push((table, start..i));
    }
    out
}

/// The lines of one paragraph block: headings, quotes, lists and plain lines, as Markdown.
fn md_lines(ui: &mut egui::Ui, lines: &[&str], c: &UiColors, dark: bool) {
    for &line in lines {
        let t = line.trim_start();
        let hashes = t.chars().take_while(|&ch| ch == '#').count();
        if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
            ui.add_space(4.0);
            md_line(ui, &t[hashes + 1..], [20.0, 17.0, 15.5][hashes.min(3) - 1], c.text, true, c);
            continue;
        }
        if let Some(quote) = t.strip_prefix('>') {
            egui::Frame::new().inner_margin(egui::Margin { left: 12, ..Default::default() }).show(ui, |ui| {
                let r = md_line(ui, quote.trim_start(), 14.0, c.muted, false, c);
                // the quote's bar, 3 pt and translucent so it reads on the grey bubble
                ui.painter().vline(r.left() - 6.0, r.y_range(), Stroke::new(3.0, tint(dark, 64)));
            });
            continue;
        }
        let number = t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.len() < 4 && n.bytes().all(|b| b.is_ascii_digit()));
        match (t.strip_prefix("- ").or(t.strip_prefix("* ")), number) {
            (Some(rest), _) => list_item(ui, "•", rest, line.len() - t.len(), c),
            (None, Some((n, rest))) => list_item(ui, &format!("{n}."), rest, line.len() - t.len(), c),
            _ => {
                md_line(ui, t, 14.0, c.text, false, c);
            }
        }
    }
}

/// A list item; nested ones step in by the source's indentation.
fn list_item(ui: &mut egui::Ui, mark: &str, rest: &str, indent: usize, c: &UiColors) {
    ui.horizontal_top(|ui| {
        ui.add_space(4.0 + indent as f32 * 7.0);
        ui.label(egui::RichText::new(mark).size(14.0).color(c.muted));
        md_line(ui, rest, 14.0, c.text, false, c);
    });
}

/// One line of Markdown, wrapped and selectable, with its `code` and **bold** runs (`bold`: all
/// of it). The UI font has no bold weight, so bold runs are drawn a second time one pixel to
/// the right: the same layout with everything else transparent.
fn md_line(ui: &mut egui::Ui, text: &str, size: f32, color: egui::Color32, bold: bool, c: &UiColors) -> egui::Rect {
    let width = ui.available_width();
    let (mut job, mut ghost, mut any) = (egui::text::LayoutJob::default(), egui::text::LayoutJob::default(), false);
    let mut add = |run: &str, font: egui::FontId, color, background, thick: bool| {
        let format = |color, background| egui::TextFormat { font_id: font.clone(), color, background, ..Default::default() };
        job.append(run, 0.0, format(color, background));
        ghost.append(run, 0.0, format(if thick { color } else { egui::Color32::TRANSPARENT }, egui::Color32::TRANSPARENT));
        any |= thick && !run.is_empty();
    };
    // inside **…**; `code` spans don't reset it
    let mut on = false;
    for (k, piece) in text.split('`').enumerate() {
        if k % 2 == 1 {
            add(piece, egui::FontId::monospace(size - 1.5), c.text, c.surface_low, false);
            continue;
        }
        for (b, run) in piece.split("**").enumerate() {
            if b > 0 {
                on = !on;
            }
            add(run, egui::FontId::proportional(size), color, egui::Color32::TRANSPARENT, bold || on);
        }
    }
    (job.wrap.max_width, ghost.wrap.max_width) = (width, width);
    let rect = ui.add(egui::Label::new(job).wrap()).rect;
    if any {
        let ghost = ui.fonts_mut(|f| f.layout_job(ghost));
        // a whole pixel: text snaps to the pixel grid, a fraction could round back onto the original
        ui.painter().galley(rect.min + egui::vec2(1.0 / ui.pixels_per_point(), 0.0), ghost, egui::Color32::TRANSPARENT);
    }
    rect
}

/// A Markdown table as a grid: the header muted and bold, the rows striped, the columns as wide as
/// their words, all in one thin frame of radius 8.
fn table(ui: &mut egui::Ui, rows: &[&str], c: &UiColors, dark: bool) {
    let cells = table_cells(rows);
    egui::Frame::new().stroke(Stroke::new(1.0, tint(dark, 26))).corner_radius(8).show(ui, |ui| {
        egui::Grid::new(ui.next_auto_id()).striped(true).spacing([0.0, 0.0]).show(ui, |ui| {
            for (n, row) in cells.iter().enumerate() {
                for cell in row {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                        // ponytail: cells don't wrap, so the grid sizes columns by their words (its
                        // first-frame widths would stick and break every word); a cell wider than
                        // the line runs past the frame. Upgrade: measure the columns and wrap to them.
                        ui.set_max_width(f32::INFINITY);
                        md_line(ui, cell, 13.0, if n == 0 { tint(dark, 140) } else { c.text }, n == 0, c);
                    });
                }
                ui.end_row();
            }
        });
    });
}

/// The |---| row of a table.
fn rule(line: &str) -> bool {
    line.starts_with('|') && line.chars().all(|ch| "|-: ".contains(ch))
}

/// The cells of a table's rows, without the |---| row; their ** and ` marks stay for `md_line`.
fn table_cells<'a>(rows: &[&'a str]) -> Vec<Vec<&'a str>> {
    rows.iter().copied().filter(|r| !rule(r)).map(|r| r.trim().trim_matches('|').split('|').map(str::trim).collect()).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn markdown_tables_split_into_cells() {
        let rows = ["| arquivo | linhas |", "|---------|:------:|", "| `maestro.rs` | 93 |", "| **ação.rs** | +5 |"];
        assert_eq!(super::table_cells(&rows), [vec!["arquivo", "linhas"], vec!["`maestro.rs`", "93"], vec!["**ação.rs**", "+5"]]);
        assert!(super::rule("|:--|---:|") && !super::rule("| a |"));
    }

    #[test]
    fn chat_text_splits_into_paragraphs_and_tables() {
        let lines = ["Oi", "tudo bem", "", "| a | b |", "|---|---|", "| 1 | 2 |", "fim"];
        assert_eq!(super::blocks(&lines), vec![(false, 0..2), (true, 3..6), (false, 6..7)]);
    }
}
