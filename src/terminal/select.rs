//! Selecting in a terminal's grid, copying it, and finding text in it (the grid's own regex
//! search): what the card's Copiar, Selecionar Tudo and Buscar work on.

use super::*;
use alacritty_terminal::index::{Boundary, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::search::{Match, RegexSearch};

/// How many matches a search keeps: more are no use to look through.
pub(crate) const MAX_MATCHES: usize = 5000;

/// What a press starts to select: characters, a word (double click) or a line (triple click).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pick {
    Chars,
    Word,
    Line,
}

/// A cell of the visible screen: row from the top, column, and which half of it the pointer is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GridCell {
    pub(crate) row: i32,
    pub(crate) col: usize,
    pub(crate) right: bool,
}

/// What a search found: its matches in reading order, and whether the query was no valid regex.
#[derive(Clone, Debug, Default)]
pub(crate) struct Found {
    pub(crate) matches: Vec<Match>,
    pub(crate) invalid: bool,
}

impl TermHandle {
    /// When the shell last printed (ms since the app started): what a search ran on goes stale with it.
    pub(crate) fn output_stamp(&self) -> u64 {
        self.last_output.load(Ordering::Relaxed)
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.term.lock().selection.as_ref().is_some_and(|s| !s.is_empty())
    }

    pub(crate) fn selected_text(&self) -> String {
        self.term.lock().selection_to_string().unwrap_or_default().trim_end().to_string()
    }

    /// "Selecionar Tudo": the whole screen and its scrollback.
    pub(crate) fn select_all(&self) {
        let mut term = self.term.lock();
        let (from, to) = (Point::new(term.topmost_line(), Column(0)), Point::new(term.bottommost_line(), term.last_column()));
        let mut selection = Selection::new(SelectionType::Simple, from, Side::Left);
        selection.update(to, Side::Right);
        term.selection = Some(selection);
    }

    pub(crate) fn clear_selection(&self) {
        self.term.lock().selection = None;
    }

    fn point_of(term: &Term<Listener>, cell: GridCell) -> Point {
        let row = cell.row.clamp(0, term.screen_lines() as i32 - 1);
        Point::new(Line(row - term.grid().display_offset() as i32), Column(cell.col.min(term.columns() - 1)))
    }

    /// A press on `cell` starts a selection there.
    pub(crate) fn begin_selection(&self, cell: GridCell, pick: Pick) {
        let mut term = self.term.lock();
        let ty = match pick {
            Pick::Chars => SelectionType::Simple,
            Pick::Word => SelectionType::Semantic,
            Pick::Line => SelectionType::Lines,
        };
        let side = if cell.right { Side::Right } else { Side::Left };
        term.selection = Some(Selection::new(ty, Self::point_of(&term, cell), side));
    }

    /// The pointer moved on, still pressed: the selection reaches `cell`; above or below the screen
    /// it scrolls the history a line toward there.
    pub(crate) fn extend_selection(&self, cell: GridCell) {
        let mut term = self.term.lock();
        let rows = term.screen_lines() as i32;
        if cell.row < 0 {
            term.scroll_display(Scroll::Delta(1));
        } else if cell.row >= rows {
            term.scroll_display(Scroll::Delta(-1));
        }
        let (point, side) = (Self::point_of(&term, cell), if cell.right { Side::Right } else { Side::Left });
        if let Some(selection) = term.selection.as_mut() {
            selection.update(point, side);
        }
    }

    /// Every match of `query` (as the text it is, or as a regex), the scrollback too. Lower case
    /// ignores the case, as the grid's search does.
    pub(crate) fn find_all(&self, query: &str, regex: bool) -> Found {
        let mut found = Found::default();
        if query.is_empty() {
            return found;
        }
        let pattern = if regex { query.to_string() } else { escape(query) };
        let Ok(mut search) = RegexSearch::new(&pattern) else {
            found.invalid = true;
            return found;
        };
        let term = self.term.lock();
        let end = Point::new(term.bottommost_line(), term.last_column());
        let mut start = Point::new(term.topmost_line(), Column(0));
        while let Some(m) = term.regex_search_right(&mut search, start, end) {
            let next = m.end().add(&*term, Boundary::None, 1);
            let last = *m.end() == end || next <= start;
            found.matches.push(m);
            if last || found.matches.len() >= MAX_MATCHES {
                break;
            }
            start = next;
        }
        found
    }

    /// Scrolls the history so `m` shows, near the middle when it was off the screen.
    pub(crate) fn reveal(&self, m: &Match) {
        let mut term = self.term.lock();
        let (top, rows) = (-(term.grid().display_offset() as i32), term.screen_lines() as i32);
        if m.start().line.0 < top || m.start().line.0 >= top + rows {
            let to = Point::new(Line(m.start().line.0 - rows / 2), m.start().column);
            let to = to.grid_clamp(&*term, Boundary::Grid);
            term.scroll_to_point(to);
        }
    }

    /// The screen's matches by visible row: (first column, last column, the current one).
    pub(crate) fn marks(&self, matches: &[Match], current: Option<usize>) -> Vec<Vec<(usize, usize, bool)>> {
        let term = self.term.lock();
        let (offset, rows, last) = (term.grid().display_offset() as i32, term.screen_lines(), term.columns() - 1);
        let mut by_row = vec![Vec::new(); rows];
        for (k, m) in matches.iter().enumerate() {
            let (from, to) = (m.start().line.0 + offset, m.end().line.0 + offset);
            for line in from.max(0)..=to.min(rows as i32 - 1) {
                let a = if line == from { m.start().column.0 } else { 0 };
                let b = if line == to { m.end().column.0 } else { last };
                by_row[line as usize].push((a, b, current == Some(k)));
            }
        }
        by_row
    }
}

/// `text` as a regex that matches just itself.
fn escape(text: &str) -> String {
    text.chars().fold(String::new(), |mut out, c| {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
        out
    })
}

/// The cell under `pos` (screen) for a grid whose top-left is `origin` and whose cells are `cell`.
pub(crate) fn cell_at(pos: Pos2, origin: Pos2, cell: Vec2) -> GridCell {
    let (dx, dy) = ((pos.x - origin.x) / cell.x, (pos.y - origin.y) / cell.y);
    GridCell { row: dy.floor() as i32, col: dx.max(0.0).floor() as usize, right: dx.rem_euclid(1.0) >= 0.5 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shell that prints `text` and ends: its output stays in the grid.
    fn printed(text: &str) -> TerminalSession {
        let line = format!("echo {text}");
        let args: Vec<&str> = if cfg!(windows) { vec!["/c", &line] } else { vec!["-c", &line] };
        let program = if cfg!(windows) { "cmd" } else { "sh" };
        let session = TerminalSession::spawn("", "", (40, 6), None, vec![], Some((program, &args)), 100).unwrap();
        for _ in 0..100 {
            if session.handle.screen_text().contains(text) {
                return session;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("the shell printed nothing");
    }

    #[test]
    fn finds_text_with_smart_case_and_as_a_regex() {
        let session = printed("zebra stripes ZEBRA");
        let h = &session.handle;
        assert_eq!(h.find_all("zebra", false).matches.len(), 2, "lower case ignores the case");
        assert_eq!(h.find_all("ZEBRA", false).matches.len(), 1, "a capital does not");
        assert_eq!(h.find_all("z.bra", false).matches.len(), 0, "plain text is no regex");
        assert_eq!(h.find_all("z.bra", true).matches.len(), 2);
        assert!(h.find_all("(", true).invalid);
        assert!(h.find_all("", false).matches.is_empty());
        let found = h.find_all("stripes", false);
        assert_eq!(found.matches.len(), 1);
        let marks = h.marks(&found.matches, Some(0));
        assert!(marks.iter().flatten().any(|&(a, b, current)| b - a == 6 && current), "{marks:?}");
    }

    #[test]
    fn selects_all_and_a_cell_range_and_clears() {
        let session = printed("alpha beta");
        let h = &session.handle;
        assert!(!h.has_selection());
        h.select_all();
        assert!(h.has_selection() && h.selected_text().contains("alpha beta"));
        h.clear_selection();
        assert!(!h.has_selection());
        let row = (0..6).find(|r| h.screen_text().lines().nth(*r).is_some_and(|l| l.contains("alpha"))).unwrap() as i32;
        let col = h.screen_text().lines().nth(row as usize).unwrap().find("alpha").unwrap();
        h.begin_selection(GridCell { row, col, right: false }, Pick::Chars);
        h.extend_selection(GridCell { row, col: col + 4, right: true });
        assert_eq!(h.selected_text(), "alpha");
        h.begin_selection(GridCell { row, col: col + 7, right: false }, Pick::Word);
        assert_eq!(h.selected_text(), "beta");
    }

    #[test]
    fn a_pointer_maps_to_its_cell_and_half() {
        let cell = Vec2::new(10.0, 20.0);
        let c = cell_at(Pos2::new(37.0, 65.0), Pos2::new(10.0, 5.0), cell);
        assert_eq!(c, GridCell { row: 3, col: 2, right: true });
        assert_eq!(cell_at(Pos2::new(0.0, 0.0), Pos2::new(10.0, 5.0), cell).row, -1);
    }
}
