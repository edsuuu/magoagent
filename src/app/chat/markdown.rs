//! The agent's words in the chat (and the IDE's docs): the light Markdown it writes, each
//! paragraph a bubble, tables a grid, code blocks a card.

use crate::app::chat::list::code_card;
use crate::app::chat::segments::{Seg, segments};
use crate::app::chat::{bubble_fill, tint};
use crate::prelude::*;
use egui::Galley;
use egui::text::LayoutJob;
use std::sync::Arc;

/// The size of a message's words, the interface's 14 (asked smaller than 16; the user's bubble too).
pub(crate) const PROSE: f32 = 14.0;

/// `prose` with `media` drawing the pictures and files (chat/media.rs), never wider than `width`.
pub(crate) fn prose_with(ui: &mut egui::Ui, text: &str, c: &UiColors, dark: bool, width: f32, bubbles: bool, media: &mut dyn FnMut(&mut egui::Ui, &Seg)) {
    let room = width.min(ui.available_width());
    for seg in segments(text) {
        match &seg {
            // the first line of a fence names the language, and maybe the file
            Seg::Code { info, code } => within(ui, room, |ui| code_card(ui, info, code, c, dark)),
            Seg::Text(words) => prose_blocks(ui, words, c, dark, width, room, bubbles),
            Seg::Image { .. } | Seg::File { .. } => within(ui, room, |ui| media(ui, &seg)),
        }
    }
}

/// One block laid out in a child `room` wide, the column told it took `room` at most: egui grows a column to whatever
/// it holds, and every line after a wide block wrapped at that width, past the edge (the island's log, with no bubbles:
/// "os textos que a IA informa estão ficando cortados"); now what comes after still wraps in `room`.
fn within(ui: &mut egui::Ui, room: f32, lay: impl FnOnce(&mut egui::Ui)) {
    let mut rect = ui.available_rect_before_wrap();
    rect.set_width(room);
    // as `Ui::scope`, with its own id
    let salt = ui.next_auto_id();
    ui.skip_ahead_auto_ids(1);
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(salt).max_rect(rect).layout(*ui.layout()));
    lay(&mut child);
    let used = child.min_rect();
    ui.advance_cursor_after_rect(Rect::from_min_size(used.min, Vec2::new(used.width().min(room), used.height())));
}

/// The blocks of one run of words: each paragraph a bubble of its own (sized by the conversation's `width`), each table
/// a card in the same column, all in `room`.
fn prose_blocks(ui: &mut egui::Ui, part: &str, c: &UiColors, dark: bool, width: f32, room: f32, bubbles: bool) {
    let lines: Vec<&str> = part.trim_matches('\n').lines().map(str::trim_end).collect();
    for (is_table, range) in blocks(&lines) {
        let block = &lines[range];
        within(ui, room, |ui| {
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
        });
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
            md_line(ui, &t[hashes + 1..], [20.0, 17.0, 15.0][hashes.min(3) - 1], c.text, true, c);
            continue;
        }
        if let Some(quote) = t.strip_prefix('>') {
            egui::Frame::new().inner_margin(egui::Margin { left: 12, ..Default::default() }).show(ui, |ui| {
                let r = md_line(ui, quote.trim_start(), PROSE, c.muted, false, c);
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
                md_line(ui, t, PROSE, c.text, false, c);
            }
        }
    }
}

/// A list item; nested ones step in by the source's indentation.
fn list_item(ui: &mut egui::Ui, mark: &str, rest: &str, indent: usize, c: &UiColors) {
    ui.horizontal_top(|ui| {
        ui.add_space(4.0 + indent as f32 * 7.0);
        ui.label(egui::RichText::new(mark).size(PROSE).color(c.muted));
        md_line(ui, rest, PROSE, c.text, false, c);
    });
}

/// One line of Markdown, wrapped and selectable, with its `code`, **bold** (`bold`: all of it) and *italic* runs. The UI
/// font has no bold weight, so bold runs are drawn a second time one pixel to the right: the same layout with everything
/// else transparent.
fn md_line(ui: &mut egui::Ui, text: &str, size: f32, color: egui::Color32, bold: bool, c: &UiColors) -> egui::Rect {
    let width = ui.available_width();
    let (mut job, mut ghost) = (egui::text::LayoutJob::default(), egui::text::LayoutJob::default());
    let any = md_runs(&mut job, &mut ghost, text, size, color, bold, c);
    (job.wrap.max_width, ghost.wrap.max_width) = (width, width);
    let rect = ui.add(egui::Label::new(job).wrap()).rect;
    if any {
        let ghost = ui.fonts_mut(|f| f.layout_job(ghost));
        // a whole pixel: text snaps to the pixel grid, a fraction could round back onto the original
        ui.painter().galley(rect.min + egui::vec2(1.0 / ui.pixels_per_point(), 0.0), ghost, egui::Color32::TRANSPARENT);
    }
    rect
}

/// The runs of one line of Markdown added to `job` (and to `ghost`, its bold runs alone): `code` in monospace on its
/// ground, **bold** (all of it when `bold`) and *italics*. True when any run is bold.
fn md_runs(job: &mut LayoutJob, ghost: &mut LayoutJob, text: &str, size: f32, color: Color32, bold: bool, c: &UiColors) -> bool {
    let mut any = false;
    let mut add = |run: &str, font: FontId, color, background, thick: bool, italics: bool| {
        let format = |color, background| egui::TextFormat { font_id: font.clone(), color, background, italics, ..Default::default() };
        job.append(run, 0.0, format(color, background));
        ghost.append(run, 0.0, format(if thick { color } else { Color32::TRANSPARENT }, Color32::TRANSPARENT));
        any |= thick && !run.is_empty();
    };
    // inside **…**; `code` spans don't reset it
    let mut on = false;
    for (k, piece) in text.split('`').enumerate() {
        if k % 2 == 1 {
            // 1.5 smaller; small on a card's picture far out, half
            add(piece, FontId::monospace((size - 1.5).max(size / 2.0)), c.text, c.surface_low, false, false);
            continue;
        }
        for (b, run) in piece.split("**").enumerate() {
            if b > 0 {
                on = !on;
            }
            for (words, italics) in emphasis(run) {
                add(words, FontId::proportional(size), color, Color32::TRANSPARENT, bold || on, italics);
            }
        }
    }
    any
}

/// A run between ** marks cut at its *italics* (or _italics_): (words, italic). A mark opens before a letter and closes
/// after one, `_` only outside a word (snake_case stays as written); a mark with no pair is just a character.
fn emphasis(run: &str) -> Vec<(&str, bool)> {
    let chars: Vec<(usize, char)> = run.char_indices().collect();
    let at = |index: usize| chars.get(index).map(|&(_, letter)| letter);
    let (mut out, mut start, mut index) = (Vec::new(), 0, 0);
    while index < chars.len() {
        let (place, mark) = chars[index];
        let word = |letter: Option<char>| letter.is_some_and(char::is_alphanumeric);
        let opens =
            matches!(mark, '*' | '_') && at(index + 1).is_some_and(|next| !next.is_whitespace()) && (mark == '*' || !word(index.checked_sub(1).and_then(at)));
        let close = (index + 2..chars.len())
            .filter(|_| opens)
            .find(|&end| chars[end].1 == mark && !chars[end - 1].1.is_whitespace() && (mark == '*' || !word(at(end + 1))));
        let Some(end) = close else {
            index += 1;
            continue;
        };
        if place > start {
            out.push((&run[start..place], false));
        }
        out.push((&run[place + 1..chars[end].0], true));
        start = chars[end].0 + 1;
        index = end + 1;
    }
    if start < run.len() {
        out.push((&run[start..], false));
    }
    out
}

/// Markdown laid out to be painted, not in a ui (a chat card's picture on the canvas): its lines with their **bold**,
/// *italics* and `code`, a list's marks as bullets, headings in bold and code blocks in monospace, wrapped at `width` and
/// cut at `rows` with "…". With it, its bold runs alone (`md_line`), to paint one pixel to the right; None with none.
pub(crate) fn md_layout(text: &str, size: f32, ink: Color32, width: f32, rows: usize, c: &UiColors) -> (LayoutJob, Option<LayoutJob>) {
    let (mut job, mut ghost, mut any, mut fence) = (LayoutJob::default(), LayoutJob::default(), false, false);
    for line in text.lines().map(str::trim) {
        if line.starts_with("```") {
            fence = !fence;
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if !job.text.is_empty() {
            job.append("\n", 0.0, egui::TextFormat::simple(FontId::proportional(size), ink));
            ghost.append("\n", 0.0, egui::TextFormat::simple(FontId::proportional(size), Color32::TRANSPARENT));
        }
        if fence {
            any |= md_runs(&mut job, &mut ghost, &format!("`{line}`"), size, ink, false, c);
            continue;
        }
        let hashes = line.chars().take_while(|&letter| letter == '#').count();
        let (mark, words, bold) = match line.strip_prefix("- ").or(line.strip_prefix("* ")) {
            Some(item) => ("• ", item, false),
            None if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') => ("", line[hashes..].trim_start(), true),
            None => ("", line.strip_prefix('>').map_or(line, str::trim_start), false),
        };
        any |= md_runs(&mut job, &mut ghost, &format!("{mark}{words}"), size, ink, bold, c);
    }
    for layout in [&mut job, &mut ghost] {
        layout.wrap.max_width = width;
        layout.wrap.max_rows = rows;
        layout.wrap.overflow_character = Some('…');
    }
    (job, any.then_some(ghost))
}

/// `md_layout` laid out by `painter`, for `paint_md`.
pub(crate) fn md_galleys(painter: &Painter, text: &str, size: f32, ink: Color32, width: f32, rows: usize, c: &UiColors) -> (Arc<Galley>, Option<Arc<Galley>>) {
    let (job, ghost) = md_layout(text, size, ink, width, rows, c);
    (painter.layout_job(job), ghost.map(|ghost| painter.layout_job(ghost)))
}

/// `md_galleys` painted at `at`: the bold runs a second time, a pixel to the right.
pub(crate) fn paint_md(painter: &Painter, at: Pos2, (galley, ghost): (Arc<Galley>, Option<Arc<Galley>>), ink: Color32) {
    painter.galley(at, galley, ink);
    if let Some(ghost) = ghost {
        painter.galley(at + Vec2::new(1.0 / painter.pixels_per_point(), 0.0), ghost, Color32::TRANSPARENT);
    }
}

/// The words `md_layout` shows, without their marks: a line of Markdown where only plain text goes (a popup).
pub(crate) fn md_plain(text: &str) -> String {
    md_layout(text, PROSE, Color32::WHITE, f32::INFINITY, usize::MAX, &UiColors::new(true)).0.text
}

/// A message's words for a small preview: each line's spaces folded, the blank lines out, `words` words at most.
pub(crate) fn tight(text: &str, words: usize) -> String {
    let mut left = words;
    let mut lines = Vec::new();
    for line in text.lines() {
        let line: Vec<&str> = line.split_whitespace().take(left).collect();
        left -= line.len();
        if !line.is_empty() {
            lines.push(line.join(" "));
        }
        if left == 0 {
            break;
        }
    }
    lines.join("\n")
}

/// A table cell's words, and the room around them in its cell.
const CELL: f32 = 13.0;
const CELL_MARGIN: egui::Margin = egui::Margin::symmetric(10, 6);
/// The narrowest a column of a table too wide for the conversation is squeezed to (its words wrap); past that, the
/// table scrolls sideways.
const COLUMN_LEAST: f32 = 90.0;

/// A Markdown table as a grid: the header muted and bold, the rows striped, all in one thin frame of radius 8. As wide
/// as the conversation's column may be (asked: "pode pegar a largura inteira do chat para não ficar espremido"): its
/// columns as wide as their words when it fits, a narrow table at its own width; wider, the wide columns share what is
/// left and their cells wrap, nothing cut; wider still, it scrolls sideways.
fn table(ui: &mut egui::Ui, rows: &[&str], c: &UiColors, dark: bool) {
    let cells = table_cells(rows);
    let ink = |n: usize| if n == 0 { tint(dark, 140) } else { c.text };
    // each column as wide as its widest cell on one line, measured as `md_line` lays it out (and its bold a pixel more)
    let mut natural: Vec<f32> = Vec::new();
    for (n, row) in cells.iter().enumerate() {
        for (column, cell) in row.iter().enumerate() {
            let (mut job, mut ghost) = (LayoutJob::default(), LayoutJob::default());
            md_runs(&mut job, &mut ghost, cell, CELL, ink(n), n == 0, c);
            let width = ui.fonts_mut(|f| f.layout_job(job)).size().x.ceil() + 1.0 + CELL_MARGIN.sum().x;
            match natural.get_mut(column) {
                Some(widest) => *widest = widest.max(width),
                None => natural.push(width),
            }
        }
    }
    // the frame's line on each side
    let widths = column_widths(&natural, ui.available_width() - 2.0, COLUMN_LEAST);
    let wider = widths.iter().sum::<f32>() + 2.0 > ui.available_width();
    let grid = |ui: &mut egui::Ui| {
        egui::Frame::new().stroke(Stroke::new(1.0, tint(dark, 26))).corner_radius(8).show(ui, |ui| {
            egui::Grid::new(ui.next_auto_id()).striped(true).spacing([0.0, 0.0]).show(ui, |ui| {
                for (n, row) in cells.iter().enumerate() {
                    for (column, cell) in row.iter().enumerate() {
                        egui::Frame::new().inner_margin(CELL_MARGIN).show(ui, |ui| {
                            // the column's width, measured above: the cell wraps in it, never wider (the grid's own
                            // widths come from the frame before)
                            ui.set_width(widths.get(column).copied().unwrap_or(COLUMN_LEAST) - CELL_MARGIN.sum().x);
                            md_line(ui, cell, CELL, ink(n), n == 0, c);
                        });
                    }
                    ui.end_row();
                }
            });
        });
    };
    if wider {
        egui::ScrollArea::horizontal().id_salt(ui.next_auto_id()).auto_shrink([false, true]).show(ui, grid);
    } else {
        grid(ui);
    }
}

/// The columns' widths in `room`: as wide as their words (`natural`) when the table fits; else the narrow ones keep
/// theirs and the wide ones share what is left alike (their cells wrap), none under `least` (past the room, then).
fn column_widths(natural: &[f32], room: f32, least: f32) -> Vec<f32> {
    if natural.iter().sum::<f32>() <= room {
        return natural.to_vec();
    }
    let mut widths = natural.to_vec();
    let (mut open, mut left): (Vec<usize>, f32) = ((0..natural.len()).collect(), room);
    while !open.is_empty() {
        let share = left / open.len() as f32;
        let (fits, wide): (Vec<usize>, Vec<usize>) = open.iter().partition(|&&column| natural[column] <= share);
        if fits.is_empty() {
            for column in wide {
                widths[column] = share.max(least.min(natural[column]));
            }
            break;
        }
        left -= fits.iter().map(|&column| natural[column]).sum::<f32>();
        open = wide;
    }
    widths
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
    use super::prose_with;
    use crate::prelude::*;

    /// The right edge of what shows of each text `lay` paints in a Ui `width` wide (inside its clip: a table scrolled
    /// sideways shows only its part in view), with its letters; after a few frames (the tables measure, the grids settle).
    fn right_edges(width: f32, lay: impl Fn(&mut egui::Ui)) -> Vec<(f32, String)> {
        use crate::prelude::*;
        let ctx = egui::Context::default();
        let mut edges = Vec::new();
        for _ in 0..3 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 2000.0))), ..Default::default() };
            let output = ctx.run_ui(raw, |ui| {
                ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 2000.0))), |ui| lay(ui));
            });
            edges = output
                .shapes
                .into_iter()
                .filter_map(|clipped| match clipped.shape {
                    Shape::Text(text) if !text.galley.is_empty() => {
                        let right = text.visual_bounding_rect().right().min(clipped.clip_rect.right());
                        Some((right, text.galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect()))
                    }
                    _ => None,
                })
                .collect();
        }
        edges
    }

    #[test]
    fn the_agents_words_wrap_in_their_width_whatever_came_before() {
        use crate::prelude::*;
        let paragraph = "Para pegar o código novo, reiniciei o worker: o antigo, do `chat-dev`, saiu. O novo está rodando na aba fila \
                         chat-dev do Terminal. A IA listou só a `A3VTNFCUVVYH`. Vou conferir a regra do Fluxo 1 no prompt e se a `7CJ4` \
                         estava nos dados desse atendimento.";
        // egui's own way: something wider than the column grows it, and a line laid out after wraps at the new width
        let grown = right_edges(300.0, |ui| {
            ui.allocate_exact_size(Vec2::new(700.0, 10.0), Sense::hover());
            ui.label(paragraph);
        });
        assert!(grown.iter().any(|(right, _)| *right > 300.0), "the hypothesis: {grown:?}");
        // the island's log (no bubbles): a code line with no spaces, a table far wider than the log, a long command, then
        // the words; each block inside the width it was given, every line after them too
        let colors = UiColors::new(true);
        let message = format!(
            "Rodei:\n\n```sh\n{}\nphp rodar.php -- --desde=35728,2 --ate=35800 --fila=perguntas --modo=completo --saida=/tmp/x.json\n```\n\n\
             | Contrato | Parcelas: total / pagas / a pagar | Próxima | Última | Dia do vencimento |\n|---|---|---|---|---|\n\
             | Contrato 5 (A3VTNFCUVVYH) | 48 / 12 / 36 | 10/12/2026 — R$ 1.234,56 | 10/11/2029 | 10 |\n\n- {paragraph}\n\n{paragraph}",
            "x".repeat(120)
        );
        for (width, room) in [(300.0, 292.0), (420.0, 412.0)] {
            let column = std::cell::Cell::new(0.0_f32);
            let edges = right_edges(width, |ui| {
                ui.set_width(width);
                // as `island_log`: the words in 8 less than the log
                prose_with(ui, &message, &colors, true, room, false, &mut |_, _| {});
                prose_with(ui, paragraph, &colors, true, room, false, &mut |_, _| {});
                column.set(ui.min_rect().width());
            });
            assert!(edges.iter().any(|(_, text)| text.contains("atendimento")), "{edges:?}");
            let out: Vec<_> = edges.iter().filter(|(right, _)| *right > room + 0.5).collect();
            assert!(out.is_empty(), "past {room}: {out:?}");
            // and the log's column as wide as it was: what comes after the words (the person's bubble) in its place
            assert!(column.get() <= width + 0.5, "the column grew to {}", column.get());
        }
    }

    #[test]
    fn a_table_keeps_its_columns_as_wide_as_their_words_until_the_room_runs_out() {
        // it fits: each column its own width, the table not stretched to the room
        assert_eq!(super::column_widths(&[200.0, 150.0, 100.0], 1000.0, 90.0), [200.0, 150.0, 100.0]);
        // it doesn't: the narrow ones keep theirs, the wide ones share what is left (and wrap)
        assert_eq!(super::column_widths(&[200.0, 700.0, 500.0], 800.0, 90.0), [200.0, 300.0, 300.0]);
        // nor at the least: never under it, the table then scrolls sideways
        assert_eq!(super::column_widths(&[400.0, 400.0, 60.0], 150.0, 90.0), [90.0, 90.0, 60.0]);
    }

    #[test]
    fn markdown_tables_split_into_cells() {
        let rows = ["| arquivo | linhas |", "|---------|:------:|", "| `maestro.rs` | 93 |", "| **ação.rs** | +5 |"];
        assert_eq!(super::table_cells(&rows), [vec!["arquivo", "linhas"], vec!["`maestro.rs`", "93"], vec!["**ação.rs**", "+5"]]);
        assert!(super::rule("|:--|---:|") && !super::rule("| a |"));
    }

    #[test]
    fn italics_pair_their_marks_and_snake_case_or_a_lone_star_stay() {
        assert_eq!(super::emphasis("isso é *muito* bom"), [("isso é ", false), ("muito", true), (" bom", false)]);
        assert_eq!(super::emphasis("_tudo_"), [("tudo", true)]);
        assert_eq!(super::emphasis("chat_node_overlay e 5 * 3 * 2"), [("chat_node_overlay e 5 * 3 * 2", false)]);
        assert_eq!(super::emphasis("um *sem par"), [("um *sem par", false)]);
    }

    #[test]
    fn a_cards_picture_shows_the_markdown_without_its_marks() {
        use crate::prelude::*;
        let answer = "## Resumo\n\n- **O que aconteceu:** o clique na caixa *Buscar*\n- **O que fiz:** rodei `--fechar`\n\n```sh\nls\n```";
        let colors = crate::app::ui::UiColors::new(true);
        let (job, ghost) = super::md_layout(answer, 10.5, egui::Color32::WHITE, 200.0, 4, &colors);
        assert_eq!(job.text, "Resumo\n• O que aconteceu: o clique na caixa Buscar\n• O que fiz: rodei --fechar\nls");
        assert_eq!((job.wrap.max_rows, job.wrap.overflow_character), (4, Some('…')), "cut by the picture's rows");
        assert!(job.sections.iter().any(|s| s.format.italics) && job.sections.iter().any(|s| s.format.font_id.family == egui::FontFamily::Monospace));
        assert!(ghost.is_some(), "the bold runs painted twice");
        assert_eq!(super::md_plain("- **Feito:** `ok`"), "• Feito: ok");
        assert_eq!(super::tight("oi,\n\n tudo   bem?", 60), "oi,\ntudo bem?");
        assert_eq!(super::tight("um dois\ntrês quatro", 3), "um dois\ntrês");
    }

    #[test]
    fn chat_text_splits_into_paragraphs_and_tables() {
        let lines = ["Oi", "tudo bem", "", "| a | b |", "|---|---|", "| 1 | 2 |", "fim"];
        assert_eq!(super::blocks(&lines), vec![(false, 0..2), (true, 3..6), (false, 6..7)]);
    }
}
