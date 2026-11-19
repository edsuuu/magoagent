//! The grid as the SwiftUI terminal draws it (macOS): rows of styled text runs with their colors
//! already resolved, a revision per row (a hash of what the row shows) and the cursor. The view
//! only paints; selection and search marks are already in the run backgrounds, as in `paint`.

use super::*;
use std::hash::{Hash, Hasher};

/// A run of cells: first column, how many cells it covers, its text, text color, background
/// (-1: the theme's) and style bits (1 bold, 2 italic, 4 underline, 8 strikeout). Colors are 0xRRGGBB.
/// Serializes as an array. Only ASCII shares a run; any other character is a run of its own (a
/// wide one covers 2 cells) so the view puts it exactly in its cell.
pub(crate) type Run = (usize, usize, String, u32, i64, u8);

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Row {
    /// Same for the same look: the view repaints only the rows whose revision changed.
    pub(crate) rev: u32,
    pub(crate) runs: Vec<Run>,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct CursorOut {
    pub(crate) row: usize,
    pub(crate) col: usize,
    /// "block", "underline", "beam" or "hollow".
    pub(crate) shape: &'static str,
    /// The program asked for a blinking one.
    pub(crate) blink: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct GridSnapshot {
    pub(crate) cols: usize,
    pub(crate) rows: Vec<Row>,
    pub(crate) cursor: Option<CursorOut>,
    /// Lines scrolled up into the history, and how many lines it has.
    pub(crate) offset: usize,
    pub(crate) history: usize,
    /// On the alternate screen (less, Claude Code's fullscreen view): no history to show.
    pub(crate) alt_screen: bool,
}

fn rgb(color: Color32) -> u32 {
    u32::from_be_bytes([0, color.r(), color.g(), color.b()])
}

/// Underline in any of its forms.
const UNDERLINES: Flags = Flags::ALL_UNDERLINES;

impl TermHandle {
    /// The visible screen as rows of runs (`marks`: the search's, by visible row, as `paint` takes them).
    pub(crate) fn grid_snapshot(&self, look: &Look, marks: &[Vec<(usize, usize, bool)>]) -> GridSnapshot {
        let term = self.term.lock();
        let content = term.renderable_content();
        let (cols, lines) = (term.columns(), term.screen_lines());
        let mut rows: Vec<Vec<Run>> = vec![Vec::new(); lines];
        // the run being built, per the row it is on
        let mut open: Option<(usize, Run)> = None;

        let close = |open: &mut Option<(usize, Run)>, rows: &mut Vec<Vec<Run>>| {
            if let Some((line, mut run)) = open.take() {
                // a style-less tail of spaces shows nothing
                if run.4 < 0 && run.5 & 4 == 0 && run.5 & 8 == 0 {
                    let kept = run.2.trim_end_matches(' ').len();

                    run.1 -= run.2.len() - kept;
                    run.2.truncate(kept);
                }

                if !run.2.is_empty() {
                    rows[line].push(run);
                }
            }
        };

        for cell in content.display_iter {
            let Some(p) = point_to_viewport(content.display_offset, cell.point) else { continue };

            if p.line >= lines || cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }

            let (line, col) = (p.line, p.column.0);
            let width = if cell.flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
            let (mut fg, mut bg) = (resolve(cell.fg, content.colors, look), resolve(cell.bg, content.colors, look));

            if cell.flags.contains(Flags::INVERSE) {
                (fg, bg) = (bg, fg);
            }

            if content.selection.is_some_and(|range| range.contains(cell.point)) {
                bg = over(bg, look.selection);
            }

            if let Some(&(.., current)) = marks.get(line).and_then(|row| row.iter().find(|(a, b, _)| (*a..=*b).contains(&col))) {
                bg = over(bg, if current { MATCH_CURRENT } else { MATCH });
            }

            if cell.flags.contains(Flags::DIM) {
                fg = fg.gamma_multiply(0.66);
            }

            if cell.flags.contains(Flags::HIDDEN) {
                fg = bg;
            }

            let style = (cell.flags.contains(Flags::BOLD) as u8)
                | (cell.flags.contains(Flags::ITALIC) as u8) << 1
                | (cell.flags.intersects(UNDERLINES) as u8) << 2
                | (cell.flags.contains(Flags::STRIKEOUT) as u8) << 3;

            let bg_code = if bg == look.bg { -1 } else { rgb(bg) as i64 };
            let (fg_code, c) = (rgb(fg), cell.c);
            let plain_space = c == ' ' && bg_code < 0 && style & 12 == 0;

            // a space with nothing to show joins a run that has no background or line (whatever its color) and never starts one
            let joins = match &open {
                Some((l, run)) if *l == line && run.0 + run.1 == col && c.is_ascii() => {
                    (plain_space && run.4 < 0 && run.5 & 12 == 0) || (run.3 == fg_code && run.4 == bg_code && run.5 == style)
                }
                _ => false,
            };

            if joins {
                if let Some((_, run)) = &mut open {
                    run.2.push(c);
                    run.1 += 1;
                }

                continue;
            }

            close(&mut open, &mut rows);

            if !plain_space {
                open = Some((line, (col, width, c.to_string(), fg_code, bg_code, style)));

                if !c.is_ascii() {
                    close(&mut open, &mut rows);
                }
            }
        }

        close(&mut open, &mut rows);

        let rows = rows
            .into_iter()
            .map(|runs| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();

                runs.hash(&mut hasher);
                Row { rev: hasher.finish() as u32, runs }
            })
            .collect();

        let shape = |shape: CursorShape| match shape {
            CursorShape::Block => Some("block"),
            CursorShape::Underline => Some("underline"),
            CursorShape::Beam => Some("beam"),
            CursorShape::HollowBlock => Some("hollow"),
            CursorShape::Hidden => None,
        };

        let cursor = shape(content.cursor.shape).zip(point_to_viewport(content.display_offset, content.cursor.point)).map(|(shape, p)| CursorOut {
            row: p.line,
            col: p.column.0,
            shape,
            blink: term.cursor_style().blinking,
        });

        GridSnapshot {
            cols,
            rows,
            cursor,
            offset: content.display_offset,
            history: term.history_size(),
            alt_screen: content.mode.contains(TermMode::ALT_SCREEN),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shell that prints `script` (printf) and ends: its output stays in the grid.
    fn printed(script: &str, wait_for: &str) -> TerminalSession {
        let line = format!("printf '{script}'");
        let session = TerminalSession::spawn("", "", (30, 6), None, vec![], Some(("sh", &["-c", &line])), 100).unwrap();

        for _ in 0..100 {
            if session.handle.screen_text().contains(wait_for) {
                return session;
            }

            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        panic!("the shell printed nothing");
    }

    #[test]
    fn rows_carry_runs_with_resolved_colors_and_styles() {
        let look = Look::default();
        let session = printed("plain \\033[31mred\\033[0m \\033[1;4mbold\\033[0m \\033[44mblue\\033[0m\\n\\033[7minv\\033[0m \\344\\270\\255 x", "inv");
        let grid = session.handle.grid_snapshot(&look, &[]);

        assert_eq!((grid.cols, grid.rows.len()), (30, 6));
        let first = &grid.rows[0].runs;

        assert_eq!(first[0], (0, 5, "plain".into(), rgb(look.fg), -1, 0), "the space before the next run shows nothing");
        let red = first.iter().find(|r| r.2 == "red").unwrap();

        assert_eq!((red.0, red.3), (6, rgb(look.ansi[1])), "ANSI red, resolved");
        let bold = first.iter().find(|r| r.2 == "bold").unwrap();

        assert_eq!(bold.5, 1 | 4, "bold and underlined");
        let blue = first.iter().find(|r| r.2 == "blue").unwrap();

        assert_eq!(blue.4, rgb(look.ansi[4]) as i64);
        // inverse swaps the colors; a wide character is a run of its own over 2 cells
        let second = &grid.rows[1].runs;

        assert_eq!((second[0].2.as_str(), second[0].3, second[0].4), ("inv", rgb(look.bg), rgb(look.fg) as i64));
        let wide = second.iter().find(|r| r.2 == "中").unwrap();

        assert_eq!((wide.0, wide.1), (4, 2));
        assert_eq!(second.last().unwrap().2, "x", "its column follows the wide one");
        assert_eq!(second.last().unwrap().0, 7);
        assert!(grid.rows[3].runs.is_empty(), "an empty row has no runs");
    }

    #[test]
    fn a_row_changes_its_revision_only_when_what_it_shows_changes() {
        let look = Look::default();
        let session = printed("alpha\\nbeta", "beta");
        let h = &session.handle;
        let before = h.grid_snapshot(&look, &[]);

        assert_eq!(before.rows[0].rev, h.grid_snapshot(&look, &[]).rows[0].rev, "same screen, same revision");
        h.begin_selection(GridCell { row: 1, col: 0, right: false }, Pick::Chars);
        h.extend_selection(GridCell { row: 1, col: 2, right: true });
        let selected = h.grid_snapshot(&look, &[]);

        assert_eq!(selected.rows[0].rev, before.rows[0].rev, "an untouched row keeps its revision");
        assert_ne!(selected.rows[1].rev, before.rows[1].rev, "the selected one changes");
        let marked = h.grid_snapshot(&look, &[vec![], vec![], vec![(0, 1, true)]]);

        assert_ne!(marked.rows[2].rev, before.rows[2].rev);
    }

    #[test]
    fn the_cursor_and_the_scrollback_are_reported() {
        let look = Look::default();
        let session = printed("one\\ntwo\\nthree\\nfour\\nfive\\nsix\\nseven\\neight", "eight");
        let h = &session.handle;
        let grid = h.grid_snapshot(&look, &[]);

        assert!(grid.history > 0 && grid.offset == 0 && !grid.alt_screen);
        assert!(grid.cursor.is_some());
        session.scroll(2);
        let up = h.grid_snapshot(&look, &[]);

        assert_eq!(up.offset, 2);
        assert!(up.cursor.is_none() || up.cursor.as_ref().is_some_and(|c| c.row >= 2), "the cursor moves with the screen");
    }
}
