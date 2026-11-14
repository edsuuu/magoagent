//! The tab that compares a file with the last commit, as the reference IDE's diff viewer: two columns (the HEAD's text and the
//! file's), each with only its own lines, the changed ones colored (green added, gray removed, blue modified) and, in a
//! modified line, its changed words stronger. Between the columns a strip joins each change's lines on the left to its lines
//! on the right (band.rs); its » puts the HEAD's lines back in the file (`compare_ui` returns it; the pane reverts the
//! change). Both columns scroll together, each at its own pace so the matching lines stay level (`map_row`). Long unchanged
//! stretches collapse to a "N linhas sem mudança" row (fold.rs). F7 and Shift+F7 (and Alt+↓ and Alt+↑) walk the changes;
//! the numbers of both files sit in the middle, a checkbox marks each change. "Unificado" puts both texts in one column, with the » at the left of the numbers.

use crate::editor::{DiffRow, RowKind, diff_rows, highlight, unified_rows};
use crate::prelude::*;
use std::collections::HashSet;
use std::ops::Range;
use std::path::Path;

mod band;
mod edit;
mod fold;
mod ui;

pub(crate) use band::map_row;
pub(crate) use fold::{Plan, visible_plan};
pub(crate) use ui::compare_ui;

/// Darcula's DIFF_* colors, as the IDE's viewer paints them.
const INSERTED: Color32 = Color32::from_rgb(0x29, 0x44, 0x36);
const DELETED: Color32 = Color32::from_rgb(0x48, 0x4A, 0x4A);
const CHANGED: Color32 = Color32::from_rgb(0x38, 0x55, 0x70);

/// The soft tint of a modified line: the word color mixed into the editor's background.
fn soft(color: Color32) -> Color32 {
    let (bg, mix) = (crate::editor::theme::ISLAND, |a: u8, b: u8| (a as f32 * 0.4 + b as f32 * 0.6).round() as u8);
    Color32::from_rgb(mix(color.r(), bg.r()), mix(color.g(), bg.g()), mix(color.b(), bg.b()))
}

/// One row of a column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Item {
    /// A line (side by side: only the column's own side of `row` is set), with the line it pairs with when modified.
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
    /// The changes whose checkbox the person cleared (by index); the box is a mark for now, it changes nothing else.
    pub(crate) unchecked: HashSet<usize>,
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
            unchecked: HashSet::new(),
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
                        if row.left.is_some() {
                            self.items[0].push(Item::Line { row: DiffRow { right: None, ..row }, partner: row.right });
                        }
                        if row.right.is_some() {
                            self.items[1].push(Item::Line { row: DiffRow { left: None, ..row }, partner: row.left });
                        }
                    }
                    if spans.get(next).is_some_and(|span| span.end == index + 1) {
                        self.blocks.push(Block { rows: [started[0]..self.items[0].len(), started[1]..self.items[1].len()] });
                        next += 1;
                    }
                }
            }
        }
        self.cur = self.cur.min(self.blocks.len().saturating_sub(1));
        let count = self.blocks.len();
        self.unchecked.retain(|index| *index < count);
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

    /// The (old row, new row) pairs where each change starts and ends.
    pub(crate) fn knots(&self) -> Vec<(f32, f32)> {
        self.blocks
            .iter()
            .flat_map(|block| [(block.rows[0].start as f32, block.rows[1].start as f32), (block.rows[0].end as f32, block.rows[1].end as f32)])
            .collect()
    }

    /// After column `driver` scrolled to `top` (a row): clamp it, and the other one follows so what is at a third of the
    /// view stays level.
    pub(crate) fn scroll_driver(&mut self, driver: usize, top: f32, most: [f32; 2], focal: f32) {
        self.scroll[driver] = top.clamp(0.0, most[driver]);
        if !self.unified {
            let other = 1 - driver;
            self.scroll[other] = (map_row(&self.knots(), driver, self.scroll[driver] + focal) - focal).clamp(0.0, most[other]);
        }
    }
}

impl Compare {
    /// The stretches a side-by-side column hides, as (first line, how many) in that column's text.
    pub(crate) fn hidden(&self, column: usize) -> Vec<(usize, usize)> {
        self.items[column]
            .iter()
            .filter_map(|item| match item {
                Item::Fold { count, starts, .. } => Some((starts[column], *count)),
                Item::Line { .. } => None,
            })
            .collect()
    }

    /// The colored text of `column` (0 the HEAD's, 1 the file's) with the changed words of its modified lines stronger, made
    /// again only when the text, the diff or the size moved.
    pub(crate) fn colored(&mut self, column: usize, font: &egui::FontId, row_h: f32) -> (u64, egui::text::LayoutJob) {
        let text = if column == 0 { &self.old } else { &self.new };
        let key = crate::editor::text::hash(text, font.size);
        if let Some(made) = self.jobs[column].as_ref().filter(|(made, _)| *made == key) {
            return made.clone();
        }
        let starts: Vec<usize> = text
            .split('\n')
            .scan(0, |at, line| {
                let start = *at;
                *at += line.len() + 1;
                Some(start)
            })
            .collect();
        let mut job = highlight::layout(self.lang, text, font.clone(), row_h);
        for item in &self.items[column] {
            let Item::Line { row, partner: Some(other) } = item else { continue };
            if row.kind != RowKind::Change {
                continue;
            }
            let (line, marks) = match (column, row.left, row.right) {
                (0, Some(line), _) => (line, word_marks_of(&self.old_lines, &self.new_lines, line, *other).0),
                (1, _, Some(line)) => (line, word_marks_of(&self.old_lines, &self.new_lines, *other, line).1),
                _ => continue,
            };
            let Some(&start) = starts.get(line) else { continue };
            for mark in marks {
                crate::editor::editor::restyle(&mut job, start + mark.start..start + mark.end, |format| format.background = CHANGED);
            }
        }
        self.jobs[column] = Some((key, job.clone()));
        (key, job)
    }
}

/// The changed words of old line `old` against new line `new`, as byte ranges of each.
fn word_marks_of(old_lines: &[String], new_lines: &[String], old: usize, new: usize) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    crate::editor::word_marks(old_lines.get(old).map_or("", String::as_str), new_lines.get(new).map_or("", String::as_str))
}
