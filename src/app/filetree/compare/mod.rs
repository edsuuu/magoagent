//! The tab that compares a file with the last commit: two columns (the HEAD's text and the file's), row for row, so a line
//! only one side has faces an empty, hatched row on the other. Removed lines are a soft red, added ones a soft green and, in
//! a modified line, the changed words stronger. Each column has its numbers at its left edge; between the columns a narrow
//! strip marks each change, and its small arrow (on hover, or on the change the walk is on) puts the HEAD's lines back in
//! the file (`compare_ui` returns it; the pane reverts the change). Both columns scroll together. Long unchanged stretches
//! collapse to a "N linhas sem mudança" row (fold.rs). F7 and Shift+F7 (and Alt+↓ and Alt+↑) walk the changes. "Unificado"
//! puts both texts in one column, with the arrow at the left of the numbers.

use crate::editor::{DiffRow, RowKind, diff_rows, highlight, unified_rows};
use crate::prelude::*;
use std::collections::HashSet;
use std::ops::Range;
use std::path::Path;

mod edit;
mod fold;
mod ui;

pub(crate) use fold::{Plan, visible_plan};
pub(crate) use ui::compare_ui;

/// A removed and an added line over the editor's background, and the changed words inside them.
const REMOVED: Color32 = Color32::from_rgb(0x3A, 0x22, 0x25);
const ADDED: Color32 = Color32::from_rgb(0x1F, 0x33, 0x27);
const REMOVED_WORD: Color32 = Color32::from_rgb(0x6B, 0x2F, 0x34);
const ADDED_WORD: Color32 = Color32::from_rgb(0x2C, 0x5C, 0x3A);

/// One row of a column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Item {
    /// A line (side by side: only the column's own side of `row` is set, and neither on the empty row across from a line
    /// only the other side has), with the line it pairs with when modified.
    Line { row: DiffRow, partner: Option<usize> },
    /// `count` unchanged lines hidden (from line `starts` of the old and of the new text); `key` names the stretch (what `open_fold` takes).
    Fold { key: usize, count: usize, starts: [usize; 2] },
}

/// A change: its rows in each column (the second is empty in one column); an empty range is where a pure insertion or removal sits.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Block {
    pub(crate) rows: [Range<usize>; 2],
}

pub(crate) struct Compare {
    pub(crate) file: PathBuf,
    pub(crate) old: String,
    pub(crate) new: String,
    pub(crate) unified: bool,
    /// Whether the long unchanged stretches are hidden (the IDE's "Collapse Unchanged Fragments").
    pub(crate) collapse: bool,
    /// The change the walk is on (counted from 0).
    pub(crate) cur: usize,
    lang: &'static highlight::Lang,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    /// What each column lists: side by side the old and the new; in one column only the first.
    pub(crate) items: [Vec<Item>; 2],
    pub(crate) blocks: Vec<Block>,
    /// The stretches the person opened (by key).
    opened: HashSet<usize>,
    /// The first row on screen of each column (fractional).
    scroll: [f32; 2],
    /// The change to bring into view next frame.
    scroll_to: Option<usize>,
    /// How far each column is scrolled sideways, on its own (Shift+wheel, the touchpad or the bar under it).
    hscroll: [f32; 2],
    /// The HEAD's short hash, for the left column's header (empty out of a repository).
    pub(crate) head: String,
    /// The colored text of each column (the changed words stronger), by the text it was made for and the font size.
    jobs: [Option<(u64, egui::text::LayoutJob)>; 2],
    /// Where each column's caret was last frame, to follow it only when it moves.
    caret: [Option<usize>; 2],
}

impl Compare {
    pub(crate) fn new(file: &Path, old: String, new: String) -> Self {
        let mut compare = Compare {
            file: file.to_path_buf(),
            old,
            new,
            unified: false,
            collapse: true,
            cur: 0,
            lang: highlight::detect(file),
            old_lines: vec![],
            new_lines: vec![],
            items: [vec![], vec![]],
            blocks: vec![],
            opened: HashSet::new(),
            scroll: [0.0; 2],
            scroll_to: Some(0),
            hscroll: [0.0; 2],
            head: String::new(),
            jobs: [None, None],
            caret: [None; 2],
        };

        compare.rebuild();
        compare
    }

    pub(crate) fn set_new(&mut self, new: String) {
        self.new = new;
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.old_lines = self.old.lines().map(String::from).collect();
        self.new_lines = self.new.lines().map(String::from).collect();
        let aligned = diff_rows(&self.old, &self.new);
        let rows: Vec<(DiffRow, Option<usize>)> = if self.unified { unified_rows(&aligned) } else { aligned.into_iter().map(|row| (row, None)).collect() };
        // the rows of each change
        let mut spans: Vec<Range<usize>> = vec![];
        let mut index = 0;

        while index < rows.len() {
            let end = index + rows[index..].iter().take_while(|row| row.0.kind != RowKind::Same).count();

            if end > index {
                spans.push(index..end);
            }

            index = end.max(index + 1);
        }

        let keys: Vec<Option<usize>> = rows.iter().map(|(row, _)| if row.kind == RowKind::Same { row.left } else { None }).collect();

        self.items = [vec![], vec![]];
        self.jobs = [None, None];
        self.blocks.clear();
        let (mut next, mut started) = (0, [0, 0]);

        for step in visible_plan(&keys, &self.opened, self.collapse) {
            match step {
                Plan::Fold { key, first, count } => {
                    let starts = [rows[first].0.left.unwrap_or(0), rows[first].0.right.unwrap_or(0)];

                    self.items[0].push(Item::Fold { key, count, starts });

                    if !self.unified {
                        self.items[1].push(Item::Fold { key, count, starts });
                    }
                }
                Plan::Show(index) => {
                    let (row, partner) = rows[index];

                    if spans.get(next).is_some_and(|span| span.start == index) {
                        started = [self.items[0].len(), self.items[1].len()];
                    }

                    if self.unified {
                        self.items[0].push(Item::Line { row, partner });
                    } else {
                        self.items[0].push(Item::Line { row: DiffRow { right: None, ..row }, partner: row.left.and(row.right) });
                        self.items[1].push(Item::Line { row: DiffRow { left: None, ..row }, partner: row.right.and(row.left) });
                    }

                    if spans.get(next).is_some_and(|span| span.end == index + 1) {
                        self.blocks.push(Block { rows: [started[0]..self.items[0].len(), started[1]..self.items[1].len()] });
                        next += 1;
                    }
                }
            }
        }

        self.cur = self.cur.min(self.blocks.len().saturating_sub(1));
    }

    /// How many changes there are.
    pub(crate) fn count(&self) -> usize {
        self.blocks.len()
    }

    /// The walk's step to the next change (1) or the previous one (-1); the first and the last stop it.
    pub(crate) fn go(&mut self, by: isize) {
        if !self.blocks.is_empty() {
            self.cur = (self.cur as isize + by).clamp(0, self.blocks.len() as isize - 1) as usize;
            self.scroll_to = Some(self.cur);
        }
    }

    /// Side by side or one column.
    pub(crate) fn set_unified(&mut self, unified: bool) {
        if self.unified != unified {
            self.unified = unified;
            self.rebuild();
            self.scroll_to = Some(self.cur);
        }
    }

    /// Hide the long unchanged stretches or show everything.
    pub(crate) fn set_collapse(&mut self, collapse: bool) {
        if self.collapse != collapse {
            self.collapse = collapse;
            self.opened.clear();
            self.rebuild();
            self.scroll_to = Some(self.cur);
        }
    }

    /// Opens the hidden stretch `key` (the marker's click).
    pub(crate) fn open_fold(&mut self, key: usize) {
        self.opened.insert(key);
        self.rebuild();
    }

    /// The first row on screen of each column.
    #[cfg(test)]
    pub(crate) fn scroll(&self) -> [f32; 2] {
        self.scroll
    }

    /// After column `driver` scrolled to `top` (a row): clamp it, and the other one follows (side by side the rows match).
    pub(crate) fn scroll_driver(&mut self, driver: usize, top: f32, most: [f32; 2]) {
        self.scroll[driver] = top.clamp(0.0, most[driver]);

        if !self.unified {
            self.scroll[1 - driver] = self.scroll[driver].clamp(0.0, most[1 - driver]);
        }
    }

    /// How many lines the file lost and gained against the HEAD.
    pub(crate) fn totals(&self) -> (usize, usize) {
        let changed = self.items.iter().flatten().filter_map(|item| match item {
            Item::Line { row, .. } if row.kind != RowKind::Same => Some(row),
            _ => None,
        });

        changed.fold((0, 0), |(removed, added), row| (removed + row.left.is_some() as usize, added + row.right.is_some() as usize))
    }
}

impl Compare {
    /// The row of each line of a side-by-side column's text, and whether it shows (a hidden line sits on its marker's row).
    pub(crate) fn line_rows(&self, column: usize) -> Vec<(usize, bool)> {
        let mut rows = vec![];

        for (index, item) in self.items[column].iter().enumerate() {
            match item {
                Item::Line { row, .. } if row.left.or(row.right).is_some() => rows.push((index, true)),
                Item::Fold { count, .. } => rows.extend(std::iter::repeat_n((index, false), *count)),
                Item::Line { .. } => {}
            }
        }

        rows
    }

    /// The colored text of `column` (0 the HEAD's, 1 the file's), made again only when the text or the size moved. The
    /// changed words are not in it: `Look::line` paints them as spans as tall as the row (`word_spans`).
    pub(crate) fn colored(&mut self, column: usize, font: &egui::FontId, row_h: f32) -> (u64, egui::text::LayoutJob) {
        let text = if column == 0 { &self.old } else { &self.new };
        let key = crate::editor::text::hash(text, font.size);

        if let Some(made) = self.jobs[column].as_ref().filter(|(made, _)| *made == key) {
            return made.clone();
        }

        let job = highlight::layout(self.lang, text, font.clone(), row_h);

        self.jobs[column] = Some((key, job.clone()));
        (key, job)
    }
}

/// The changed words of a modified line as the spans painted under it: marks with only blanks between them are one span
/// (no holes at the spaces), blanks at a span's ends left out, and a line changed nearly whole has none (its row's color
/// already says it).
pub(crate) fn word_spans(text: &str, marks: Vec<Range<usize>>) -> Vec<Range<usize>> {
    let mut spans: Vec<Range<usize>> = vec![];

    for mark in marks {
        let (start, end) = (mark.start.min(text.len()), mark.end.min(text.len()));
        let Some(piece) = text.get(start..end) else { continue };
        let trimmed = piece.trim();

        if trimmed.is_empty() {
            continue;
        }

        let start = start + (piece.len() - piece.trim_start().len());
        let end = start + trimmed.len();

        match spans.last_mut() {
            Some(last) if text.get(last.end..start).is_some_and(|gap| gap.trim().is_empty()) => last.end = end,
            _ => spans.push(start..end),
        }
    }

    let content = text.chars().filter(|character| !character.is_whitespace()).count();
    let marked: usize = spans.iter().map(|span| text[span.clone()].chars().filter(|character| !character.is_whitespace()).count()).sum();

    if content == 0 || marked * 10 >= content * 7 {
        return vec![];
    }

    spans
}
