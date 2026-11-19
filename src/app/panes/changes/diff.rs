//! The diff of the file picked in Alterações: one number per line in a gutter (the file's, the HEAD's on a removed
//! line, as Claude Code's diff), the code in its language's colors
//! (`highlight`, read over the whole file so a comment or a string that spans lines keeps its color), a whole row of
//! green or red under an added or removed line (the row, never a word), long lines scrolling sideways. A press on a
//! line picks it for a review comment; Shift+click or a drag picks more (review.rs).

use super::super::card;
use crate::editor::highlight::{self, Lang, St};
use crate::prelude::*;
use egui::text::{LayoutJob, TextFormat};
use std::path::Path;

/// The most lines of a diff the panel shows.
const DIFF_LINES: usize = 400;
/// A row of the diff.
const LINE: f32 = 17.0;
/// The background of an added and of a removed line.
pub(crate) const ADDED_ROW: Color32 = Color32::from_rgba_unmultiplied_const(46, 160, 67, 40);
pub(crate) const REMOVED_ROW: Color32 = Color32::from_rgba_unmultiplied_const(248, 81, 73, 40);

/// A line of the diff: `+`, `-`, ` ` or `…` (stretches apart), its numbers in the HEAD and in the file now, its text
/// and its colors (byte ranges of `text` and their kind).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DiffLine {
    pub(crate) mark: char,
    pub(crate) old: Option<usize>,
    pub(crate) new: Option<usize>,
    pub(crate) text: String,
    pub(crate) spans: Vec<(usize, usize, St)>,
}

/// The diff of `old` to `new` as lines, three around each change, `…` between stretches apart; `DIFF_LINES` at most.
pub(crate) fn diff_lines(old: &str, new: &str) -> Vec<DiffLine> {
    let diff = similar::TextDiff::from_lines(old, new);
    let mut lines = Vec::new();

    for (index, group) in diff.grouped_ops(3).iter().enumerate() {
        if index > 0 {
            lines.push(DiffLine { mark: '…', old: None, new: None, text: String::new(), spans: Vec::new() });
        }

        for op in group {
            for change in diff.iter_changes(op) {
                let mark = match change.tag() {
                    similar::ChangeTag::Insert => '+',
                    similar::ChangeTag::Delete => '-',
                    similar::ChangeTag::Equal => ' ',
                };

                let text = change.value().trim_end_matches(['\n', '\r']).to_string();

                lines.push(DiffLine { mark, old: change.old_index().map(|n| n + 1), new: change.new_index().map(|n| n + 1), text, spans: Vec::new() });
            }
        }
    }

    lines.truncate(DIFF_LINES);
    lines
}

/// Where each line of `text` starts and ends (without its `\n`), by byte.
fn line_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;

    for (at, _) in text.match_indices('\n') {
        ranges.push((start, at));
        start = at + 1;
    }

    ranges.push((start, text.len()));
    ranges
}

/// The tokens of `tokens` (sorted, apart) that fall in `start..end`, from the line's own start.
fn line_spans(tokens: &[(usize, usize, St)], start: usize, end: usize) -> Vec<(usize, usize, St)> {
    let first = tokens.partition_point(|t| t.1 <= start);

    tokens[first..].iter().take_while(|t| t.0 < end).map(|&(a, b, kind)| (a.max(start) - start, b.min(end) - start, kind)).filter(|(a, b, _)| a < b).collect()
}

/// Each line's colors, from the tokens of the whole file it is in: the HEAD's for a removed line, the file's now for
/// the rest.
pub(crate) fn color_lines(lines: &mut [DiffLine], lang: &Lang, old: &str, new: &str) {
    let sides = [old, new].map(|text| (line_ranges(text), highlight::tokens(lang, text)));

    for line in lines.iter_mut() {
        let (side, number) = if line.mark == '-' { (&sides[0], line.old) } else { (&sides[1], line.new) };
        let Some(&(start, end)) = number.and_then(|n| side.0.get(n - 1)) else { continue };

        line.spans = line_spans(&side.1, start, end).into_iter().filter(|&(_, b, _)| b <= line.text.len()).collect();
    }
}

/// A kind of token's color: the editor's own (Islands Dark) on the dark panel, IntelliJ Light's on the light one.
pub(crate) fn token_color(kind: St, dark: bool, text: Color32) -> Color32 {
    if dark {
        return highlight::color(kind);
    }

    match kind {
        St::Text | St::Bracket(_) => text,
        St::Keyword | St::Tag => rgb_hex(0x0033b3),
        St::Str => rgb_hex(0x067d17),
        St::Number => rgb_hex(0x1750eb),
        St::Comment | St::Doc => rgb_hex(0x8c8c8c),
        St::Function | St::Method => rgb_hex(0x00627a),
        St::Field | St::Constant => rgb_hex(0x871094),
        St::Meta => rgb_hex(0x9e880d),
    }
}

/// A line's text in stretches of one kind each, tabs as four spaces (the SwiftUI panel paints the same stretches).
pub(crate) fn line_parts(line: &DiffLine) -> Vec<(String, St)> {
    let mut parts = Vec::new();

    let mut push = |range: std::ops::Range<usize>, kind: St| {
        if let Some(part) = line.text.get(range).filter(|p| !p.is_empty()) {
            parts.push((part.replace('\t', "    "), kind));
        }
    };

    let mut at = 0;

    for &(a, b, kind) in &line.spans {
        if a > at {
            push(at..a, St::Text);
        }

        push(a..b, kind);
        at = b;
    }

    push(at..line.text.len(), St::Text);
    parts
}

/// A line's text in its colors.
fn line_job(line: &DiffLine, font: &FontId, dark: bool, text: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();

    for (part, kind) in line_parts(line) {
        let format = TextFormat {
            font_id: font.clone(),
            color: token_color(kind, dark, text),
            italics: matches!(kind, St::Doc),
            valign: egui::Align::Center,
            ..Default::default()
        };

        job.append(&part, 0.0, format);
    }

    job
}

/// The rows `rows` (either order) as first and last.
pub(crate) fn ordered(rows: (usize, usize)) -> (usize, usize) {
    (rows.0.min(rows.1), rows.0.max(rows.1))
}

impl DrawingApp {
    /// The picked file's diff in a box: the gutter with the numbers, then the code, sideways scrolling when long. A
    /// press, a Shift+click or a drag over the lines picks them for a comment.
    pub(super) fn diff_view(&mut self, ui: &mut egui::Ui, path: &Path) {
        let (colors, dark) = (self.colors(), self.dark());
        let lines = std::mem::take(&mut self.panes.changes.diff);
        let mut picked = self.panes.changes.review.picked;
        let reveal = std::mem::take(&mut self.panes.changes.review.reveal);
        // a click, or a drag let go: the lines are picked
        let mut settled = false;

        card(ui, dark, |ui| {
            if lines.is_empty() {
                ui.label("Não há diferenças com o HEAD.");

                return;
            }

            let font = FontId::monospace(12.0);
            let char_width = ui.fonts_mut(|fonts| fonts.glyph_width(&font, '0'));
            let widest = lines.iter().map(|l| l.old.max(l.new).unwrap_or(0)).max().unwrap_or(0);
            let number_width = (widest.to_string().len().max(2) as f32) * char_width + 8.0;
            let gutter_width = number_width + char_width + 6.0;

            let text_width =
                lines.iter().map(|l| l.text.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum::<usize>()).max().unwrap_or(0) as f32 * char_width + 12.0;

            let height = lines.len() as f32 * LINE;
            let rows = picked.map(ordered);
            let ink = glass_ink(dark);

            // the whole row green or red, and the lines picked tinted over it (the change still shows)
            let paint_row = |p: &Painter, row: Rect, mark: char, index: usize| {
                match mark {
                    '+' => drop(p.rect_filled(row, 0, ADDED_ROW)),
                    '-' => drop(p.rect_filled(row, 0, REMOVED_ROW)),
                    _ => {}
                }

                if rows.is_some_and(|(a, b)| (a..=b).contains(&index)) {
                    p.rect_filled(row, 0, glass_accent(dark).gamma_multiply(0.3));
                }
            };

            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let mut responses = Vec::new();

            ui.horizontal_top(|ui| {
                let (gutter, response) = ui.allocate_exact_size(Vec2::new(gutter_width, height), Sense::click_and_drag());

                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, "Números das linhas"));
                responses.push((gutter.top(), response));
                let p = ui.painter();
                let small = FontId::monospace(11.0);

                for (index, line) in lines.iter().enumerate() {
                    let row = Rect::from_min_size(Pos2::new(gutter.left(), gutter.top() + index as f32 * LINE), Vec2::new(gutter_width, LINE));

                    if !ui.clip_rect().intersects(row) {
                        continue;
                    }

                    paint_row(p, row, line.mark, index);
                    let number = |n: Option<usize>| n.map_or(String::new(), |n| n.to_string());
                    let muted = ink.gamma_multiply(0.45);

                    // one column: the line in the file, or in the HEAD for one that left it
                    let shown = if line.mark == '-' { line.old } else { line.new };

                    p.text(Pos2::new(row.left() + number_width - 4.0, row.center().y), egui::Align2::RIGHT_CENTER, number(shown), small.clone(), muted);

                    let mark_color = match line.mark {
                        '+' => crate::app::filetree::diff::ADDED,
                        '-' => colors.danger,
                        _ => muted,
                    };

                    let mark = if line.mark == ' ' { String::new() } else { line.mark.to_string() };

                    p.text(Pos2::new(row.left() + number_width + 2.0, row.center().y), egui::Align2::LEFT_CENTER, mark, font.clone(), mark_color);
                }

                egui::ScrollArea::horizontal().id_salt(("changes_diff", path)).auto_shrink([false, true]).show(ui, |ui| {
                    let width = ui.available_width().max(text_width);
                    let (body, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

                    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, "Linhas do diff"));
                    responses.push((body.top(), response));
                    let p = ui.painter();

                    for (index, line) in lines.iter().enumerate() {
                        let row = Rect::from_min_size(Pos2::new(body.left(), body.top() + index as f32 * LINE), Vec2::new(width, LINE));

                        if !ui.clip_rect().intersects(row) {
                            continue;
                        }

                        paint_row(p, row, line.mark, index);

                        if line.mark == '…' {
                            p.text(Pos2::new(row.left() + 4.0, row.center().y), egui::Align2::LEFT_CENTER, "⋯", font.clone(), ink.gamma_multiply(0.45));

                            continue;
                        }

                        let galley = p.layout_job(line_job(line, &font, dark, ink.gamma_multiply(if line.mark == ' ' { 0.75 } else { 1.0 })));

                        p.galley(Pos2::new(row.left() + 4.0, row.center().y - galley.size().y / 2.0), galley, ink);
                    }
                });

                if let Some((first, last)) = rows.filter(|_| reveal) {
                    let span = Rect::from_x_y_ranges(gutter.x_range(), gutter.top() + first as f32 * LINE..=gutter.top() + (last + 1) as f32 * LINE);

                    // a line of room each side: the diff's sideways bar sits on the lowest line in sight
                    ui.scroll_to_rect(span.expand2(Vec2::new(0.0, LINE * 2.0)), None);
                }
            });

            let shift = ui.input(|i| i.modifiers.shift);
            let last = lines.len() - 1;

            for (top, response) in responses {
                let row_at = |y: f32| (((y - top) / LINE).floor().max(0.0) as usize).min(last);
                let response = response.on_hover_cursor(CursorIcon::PointingHand);

                if response.drag_started()
                    && let Some(origin) = ui.input(|i| i.pointer.press_origin())
                {
                    let row = row_at(origin.y);

                    picked = Some(match picked {
                        Some((anchor, _)) if shift => (anchor, row),
                        _ => (row, row),
                    });
                }

                if response.dragged()
                    && let (Some(at), Some((anchor, _))) = (response.interact_pointer_pos(), picked)
                {
                    picked = Some((anchor, row_at(at.y)));
                }

                settled |= response.drag_stopped();

                if response.clicked()
                    && let Some(at) = response.interact_pointer_pos()
                {
                    settled = true;
                    let row = row_at(at.y);

                    picked = match picked {
                        Some((anchor, _)) if shift => Some((anchor, row)),
                        // the one line picked, pressed again: let go
                        Some(rows) if rows == (row, row) => None,
                        _ => Some((row, row)),
                    };
                }
            }
        });

        if !lines.is_empty() {
            ui.label(egui::RichText::new("Clique numa linha para comentar; Shift+clique ou arraste para pegar mais.").size(11.0).color(colors.muted));
        }

        if super::live(ui) {
            let review = &mut self.panes.changes.review;

            // lines picked: the comment's bar takes the keys, and the lines stay in sight over it
            review.focus_draft |= settled && picked.is_some();
            review.reveal = settled && picked.is_some();
            review.picked = picked;
        }

        self.panes.changes.diff = lines;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(lines: &[DiffLine]) -> Vec<(char, &str)> {
        lines.iter().map(|l| (l.mark, l.text.as_str())).collect()
    }

    #[test]
    fn the_diff_keeps_three_lines_around_each_change_with_their_numbers() {
        let old: String = (1..=20).map(|n| format!("linha {n}\n")).collect();
        let new = old.replace("linha 2\n", "linha dois\n").replace("linha 18\n", "");
        let lines = diff_lines(&old, &new);
        let shown = marks(&lines);

        assert!(shown.contains(&('-', "linha 2")) && shown.contains(&('+', "linha dois")));
        assert!(shown.contains(&('-', "linha 18")));
        assert!(shown.contains(&('…', "")), "the stretches apart are split: {shown:?}");
        assert!(!shown.iter().any(|(_, text)| *text == "linha 10"), "far from the changes");
        assert!(diff_lines("igual\n", "igual\n").is_empty());
        let removed = lines.iter().find(|l| l.text == "linha 2").unwrap();

        assert_eq!((removed.old, removed.new), (Some(2), None));
        let added = lines.iter().find(|l| l.text == "linha dois").unwrap();

        assert_eq!((added.old, added.new), (None, Some(2)));
        let context = lines.iter().find(|l| l.text == "linha 17").unwrap();

        assert_eq!((context.old, context.new), (Some(17), Some(17)), "a context line has both");
    }

    #[test]
    fn each_line_takes_its_colors_from_the_whole_file() {
        let old = "fn velho() {}\n";
        let new = "/* um comentário\nque continua */\nfn novo() { let x = \"texto\"; }\n";
        let mut lines = diff_lines(old, new);

        color_lines(&mut lines, highlight::detect(Path::new("a.rs")), old, new);
        let kinds = |text: &str| lines.iter().find(|l| l.text == text).unwrap().spans.iter().map(|s| s.2).collect::<Vec<_>>();

        assert_eq!(kinds("que continua */"), vec![St::Comment], "the comment opened a line above still colors this one");
        assert!(kinds("fn novo() { let x = \"texto\"; }").contains(&St::Keyword));
        assert!(kinds("fn novo() { let x = \"texto\"; }").contains(&St::Str));
        assert!(kinds("fn velho() {}").contains(&St::Keyword), "a removed line is colored from the HEAD's text");

        for line in &lines {
            assert!(line.spans.iter().all(|&(a, b, _)| a < b && b <= line.text.len()), "{line:?}");
        }

        // light: the editor's light colors, the text the panel's
        let ink = Color32::BLACK;

        assert_eq!(token_color(St::Text, false, ink), ink);
        assert_ne!(token_color(St::Keyword, false, ink), token_color(St::Keyword, true, ink));
    }
}
