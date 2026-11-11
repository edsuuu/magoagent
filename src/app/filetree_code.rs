//! The code in a file tree node's editor pane, drawn as the IDE's editor draws it (Islands Dark, its code font and so its
//! ligatures): line numbers, the caret's row, indent guides, the right margin, the bracket pair, the usages and the author
//! over each declaration, the parameter names inside calls, and Ctrl+click to a declaration. No IDE node behind it: only
//! the IDE's pure parts (highlight, hints, index, text) over a `Project` kept here for the tree's root. What needs a
//! language server (problems, documentation, signatures) stays in the IDE.

use crate::app::filetree_editor::TextBuf;
use crate::app::filetree_ui::cr;
use crate::ide::editor::{RIGHT_MARGIN_COLUMN, decl_author, decl_span, guide_on, indent_of, indent_unit, person_icon, restyle};
use crate::ide::highlight::{self, Kind, Lang};
use crate::ide::hints::{self, Hint};
use crate::ide::index::{self, Index};
use crate::ide::text::{char_at, family, find_declaration, framework_target, hash, word_at};
use crate::ide::theme as t;
use crate::prelude::*;
use egui::text::{CCursor, LayoutJob};
use egui::text_selection::CCursorRange;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};

/// The biggest text that gets the parameter names, the labels over its declarations and the bracket pair (the IDE opens
/// no bigger file); past it the text is only colored.
// ponytail: those read the whole text each time it changes; a bigger file needs them cut to the lines in view
const SMART_MAX: usize = 256 << 10;
/// The most code files of a tree's root that are indexed, and the most entries its walk looks at (a tree may sit on a
/// whole home folder).
// ponytail: caps, not a watcher: a root over many projects indexes the first files found, and a file made outside is
// only known once saved here; index the repository of the open file, and list again on focus, if that bites
const INDEX_MAX: usize = 20_000;
const WALK_MAX: usize = 300_000;

/// The code files under `root`, without the folders the IDE leaves out of a project (node_modules, target, what the
/// .gitignore names…), until there are `INDEX_MAX` of them or the walk has seen `WALK_MAX` entries.
fn code_files(root: &Path) -> Vec<PathBuf> {
    let excluded = crate::ide::excluded::Excluded::load(root);
    let (mut out, mut stack, mut seen) = (vec![], vec![root.to_path_buf()], 0);
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            seen += 1;
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                if !excluded.contains(&path) {
                    stack.push(path);
                }
            } else if index::indexable(&path) {
                out.push(path);
            }
            if out.len() >= INDEX_MAX || seen >= WALK_MAX {
                return out;
            }
        }
    }
    out
}

/// Where a Ctrl+click's search ended.
#[derive(Debug, PartialEq)]
pub(crate) enum Found {
    At(PathBuf, usize, usize),
    Nowhere(String),
}

/// What the code of one tree's editor knows of its project (the tree's root), as the IDE's: the files and their
/// declarations, listed and read on a thread, for the parameter names, the usage counts and Ctrl+click; and who wrote
/// each line.
#[derive(Default)]
pub(crate) struct Project {
    root: PathBuf,
    files: Vec<PathBuf>,
    index: Index,
    /// Moves with the index and the blames: what is drawn from them is worked out again.
    generation: u64,
    index_rx: Option<Receiver<(Index, Vec<PathBuf>)>>,
    /// `git blame` per file (None: an uncommitted line; empty outside git).
    blame: HashMap<PathBuf, Vec<Option<String>>>,
    blame_rx: Option<Receiver<(PathBuf, Vec<Option<String>>)>>,
    /// The name a Ctrl+click asked for, while its declaration is searched.
    decl: Option<(String, Receiver<Option<(PathBuf, usize, usize)>>)>,
}

impl Project {
    /// Indexes `root` (a thread lists and reads it), unless it already is the project.
    pub(crate) fn open(&mut self, root: PathBuf, ctx: &Context) {
        if root == self.root || !root.is_dir() {
            return;
        }
        let (tx, rx) = channel();
        *self = Project { root: root.clone(), generation: self.generation + 1, index_rx: Some(rx), ..Default::default() };
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let files = code_files(&root);
            let _ = tx.send((Index::build(&files), files));
            ctx.request_repaint();
        });
    }

    /// Whether the project's files are still being read.
    pub(crate) fn indexing(&self) -> bool {
        self.index_rx.is_some()
    }

    /// The threads' news. Back: where a Ctrl+click's declaration is, once its search ends.
    pub(crate) fn poll(&mut self) -> Option<Found> {
        if let Some((index, files)) = self.index_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            (self.index, self.files, self.index_rx) = (index, files, None);
            self.generation += 1;
        }
        if let Some((path, lines)) = self.blame_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.blame_rx = None;
            self.blame.insert(path, lines);
            self.generation += 1;
        }
        let found = self.decl.as_ref()?.1.try_recv().ok()?;
        let (word, _) = self.decl.take()?;
        Some(found.map_or(Found::Nowhere(word), |(path, line, col)| Found::At(path, line, col)))
    }

    /// Asks git who wrote each line of `path`, once; one run at a time.
    pub(crate) fn blame(&mut self, path: &Path, ctx: &Context) {
        if self.blame_rx.is_some() || self.blame.contains_key(path) {
            return;
        }
        let (Some(dir), Some(name)) = (path.parent().map(Path::to_path_buf), path.file_name().map(|n| n.to_string_lossy().into_owned())) else { return };
        let (tx, rx) = channel();
        let (path, ctx) = (path.to_path_buf(), ctx.clone());
        std::thread::spawn(move || {
            let lines = crate::ide::run_git(&dir, &["blame", "--porcelain", "--", name.as_str()]).map(|out| crate::ide::parse_blame(&out)).unwrap_or_default();
            let _ = tx.send((path, lines));
            ctx.request_repaint();
        });
        self.blame_rx = Some(rx);
    }

    /// `path` was written: its declarations are read again, and its blame was of the old text.
    pub(crate) fn saved(&mut self, path: &Path) {
        if index::indexable(path) {
            self.index.update(path);
            if !self.files.iter().any(|f| f == path) {
                self.files.push(path.to_path_buf());
            }
        }
        self.blame.remove(path);
        self.generation += 1;
    }

    /// Ctrl+click at char `c` of `text`: where the name under it is declared. A framework's string (a view, a route)
    /// answers now; a name is searched in its file, then in the project's files of the same language family, on a
    /// thread (`poll` brings the answer).
    pub(crate) fn declaration(&mut self, path: &Path, lang: &Lang, text: &str, c: usize, ctx: &Context) -> Option<Found> {
        if let Some((file, line)) = framework_target(&self.root, text, c) {
            return Some(Found::At(file, line, 0));
        }
        let (a, b) = word_at(text, c);
        let word = text.chars().skip(a).take(b - a).collect::<String>().trim_start_matches('$').to_string();
        if word.is_empty() {
            return None;
        }
        let fam = family(lang.id);
        let files: Vec<PathBuf> = self.files.iter().filter(|p| family(highlight::detect(p).id) == fam).cloned().collect();
        let (tx, rx) = channel();
        let (current, name, ctx) = (path.to_path_buf(), word.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(find_declaration(&name, &current, &files));
            ctx.request_repaint();
        });
        self.decl = Some((word, rx));
        None
    }
}

/// What is drawn of a text, worked out again only when the text, its size or the project's index changes.
pub(crate) struct Laid {
    key: (u64, u64),
    /// The colored text, with room before each named argument.
    job: LayoutJob,
    /// Each line's indent, the file's indent step and its deepest indent.
    cols: Vec<Option<usize>>,
    unit: usize,
    deepest: usize,
    /// The parameter names: the hint, its line and its label's width.
    pub(crate) hints: Vec<(Hint, usize, f32)>,
    /// The declarations with a label over them, by line: the usages ("3 usos") and who wrote most of it.
    pub(crate) labels: Vec<(usize, String, String)>,
}

impl Laid {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        key: (u64, u64),
        ui: &egui::Ui,
        lang: &Lang,
        text: &str,
        path: &Path,
        code: &Project,
        font: &FontId,
        label: &FontId,
        row_h: f32,
        z: f32,
    ) -> Self {
        let mut job = highlight::layout(lang, text, font.clone(), row_h);
        let cols: Vec<Option<usize>> = text.split('\n').map(indent_of).collect();
        let (unit, deepest) = (indent_unit(&cols), cols.iter().flatten().copied().max().unwrap_or(0));
        let (mut hints, mut labels) = (vec![], vec![]);
        if lang.kind == Kind::Code && text.len() <= SMART_MAX {
            let (mut at, mut line) = (0, 0);
            for h in hints::finish(text, hints::from_index(&hints::calls(lang, text), &code.index, path)) {
                line += text[at..h.byte].matches('\n').count();
                at = h.byte;
                let width = hints::width(ui, &h.label, label);
                hints.push((h, line, width));
            }
            let gaps: Vec<(usize, f32)> = hints.iter().map(|(h, _, w)| (h.byte, w + (2.0 * hints::PAD + hints::AFTER) * z)).collect();
            hints::open(&mut job, &gaps);
            // an index still being read counts no usage: the labels wait for it
            if !code.indexing() {
                let lines: Vec<&str> = text.split('\n').collect();
                let blame = code.blame.get(path);
                for d in index::declarations(lang.id, text) {
                    let usages = code.index.vision(&d).unwrap_or_default();
                    let author = blame.and_then(|b| decl_author(b, decl_span(&lines, d.line))).unwrap_or_default();
                    if labels.last().is_none_or(|l: &(usize, String, String)| l.0 != d.line) && !(usages.is_empty() && author.is_empty()) {
                        labels.push((d.line, usages, author));
                    }
                }
            }
        }
        Laid { key, job, cols, unit, deepest, hints, labels }
    }
}

/// Opens a row of `h` above each of `lines` (in order): that line's rows and every row after them move down. Rows are
/// moved, not made taller, as the IDE's `open_gaps`; by line and not by row, because the pane's text may wrap.
fn open_rows(galley: &mut egui::Galley, lines: impl Iterator<Item = usize>, h: f32) {
    let mut lines = lines.peekable();
    let (mut shift, mut line, mut starts) = (0.0, 0, true);
    for row in &mut galley.rows {
        if starts && lines.next_if_eq(&line).is_some() {
            shift += h;
        }
        row.pos.y += shift;
        starts = row.ends_with_newline;
        line += usize::from(starts);
    }
    galley.rect.max.y += shift;
    galley.mesh_bounds.max.y += shift;
}

/// The text in `body`, edited. Back: whether it changed, and the char a Ctrl+click asked the declaration of.
#[allow(clippy::too_many_arguments)]
pub(super) fn text_ui(
    ui: &mut egui::Ui,
    body: Rect,
    buf: &mut TextBuf,
    path: &Path,
    id: egui::Id,
    z: f32,
    wrap: bool,
    code: &Project,
) -> (bool, Option<usize>) {
    let font = t::mono(t::EDITOR_SIZE * z);
    let label_font = t::ui(font.size - z); // the UI face, a little smaller than the code
    let vision_font = t::ui((t::UI_SIZE - 1.0) * z);
    let (natural, char_w) = ui.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
    let row_h = (natural * 1.2).round(); // the IDE's line spacing
    let lines = buf.text.matches('\n').count() + 1;
    let gutter = Rect::from_min_size(body.min, Vec2::new(lines.to_string().len().max(3) as f32 * char_w + 26.0 * z, body.height()));
    let area = Rect::from_min_max(Pos2::new(gutter.right(), body.top()), body.max);
    // the editor's own background, inside the card's border; the line numbers stay on it when the text goes sideways
    let p = ui.painter().with_clip_rect(body.intersect(ui.clip_rect()));
    p.rect_filled(body.with_max_x(body.right() - 1.0).with_max_y(body.bottom() - 1.0), egui::CornerRadius { se: cr(7.0 * z), ..Default::default() }, t::ISLAND);
    // where a Ctrl+click landed: the caret goes there, and that line to the middle of the pane
    let landed = buf.jump.take().map(|(line, col)| {
        let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(char_at(&buf.text, line, col)))));
        state.store(ui.ctx(), id);
        buf.focus = true;
        line
    });
    let want_focus = std::mem::take(&mut buf.focus);
    // the bracket next to the caret (the one before it first) and its pair, found again when the caret or the text moves
    let caret = egui::TextEdit::load_state(ui.ctx(), id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.0);
    let before = hash(&buf.text, font.size);
    if buf.bracket.as_ref().is_none_or(|b| b.0 != (before, caret)) {
        let at = |ci: usize| buf.text.char_indices().nth(ci).map(|(b, _)| b);
        let pair = caret
            .filter(|_| buf.text.len() <= SMART_MAX)
            .and_then(|c| [c.checked_sub(1), Some(c)].into_iter().flatten().find_map(|ci| at(ci).and_then(|b| highlight::pair(buf.lang, &buf.text, b))));
        buf.bracket = Some(((before, caret), pair));
    }
    let bracket = buf.bracket.as_ref().and_then(|b| b.1.clone());
    let (mut changed, mut ctrl_click) = (false, None);
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        ui.set_clip_rect(area.intersect(ui.clip_rect()));
        ui.visuals_mut().selection.bg_fill = t::EDITOR_SELECTION;
        ui.visuals_mut().selection.stroke = Stroke::new(1.0, t::TEXT);
        ui.visuals_mut().text_cursor.stroke = Stroke::new((2.0 * z).max(1.0), t::CARET);
        egui::ScrollArea::both().id_salt(id.with("scroll")).auto_shrink(false).show(ui, |ui| {
            // under the text: the caret's row and the guides, filled in once the text is laid out
            let caret_bg = ui.painter().add(Shape::Noop);
            let guides_bg = ui.painter().add(Shape::Noop);
            let TextBuf { text, laid, lang, .. } = buf;
            let lang: &'static Lang = lang;
            let mut layouter = |ui: &egui::Ui, b: &dyn egui::TextBuffer, width: f32| {
                let s = b.as_str();
                let key = (hash(s, font.size), code.generation);
                if laid.as_ref().is_none_or(|l| l.key != key) {
                    *laid = Some(Laid::new(key, ui, lang, s, path, code, &font, &label_font, row_h, z));
                }
                let l = laid.as_ref().unwrap();
                let mut job = l.job.clone();
                job.wrap.max_width = if wrap { width } else { f32::INFINITY };
                // the pair is of the text before this frame's typing: its bytes may not be this text's
                if let Some((own, _, pair)) = bracket.as_ref().filter(|_| key.0 == before) {
                    match pair {
                        Some(p) => {
                            for r in [own, p] {
                                restyle(&mut job, r.clone(), |f| f.background = t::BRACKET_PAIR);
                            }
                        }
                        None => restyle(&mut job, own.clone(), |f| f.color = t::UNMATCHED_BRACE),
                    }
                }
                let mut galley = ui.fonts_mut(|f| f.layout_job(job));
                if !l.labels.is_empty() {
                    open_rows(std::sync::Arc::make_mut(&mut galley), l.labels.iter().map(|l| l.0), row_h);
                }
                galley
            };
            let out = egui::TextEdit::multiline(text)
                .id(id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::symmetric((6.0 * z).round() as i8, 0))
                .font(font.clone())
                .layouter(&mut layouter)
                .lock_focus(true)
                .desired_width(if wrap { area.width() } else { f32::INFINITY })
                .desired_rows(1)
                .show(ui);
            let resp = out.response.response.clone();
            if want_focus {
                resp.request_focus();
            }
            changed = resp.changed();
            let Some(l) = laid.as_ref() else { return };
            let (origin, clip) = (out.galley_pos, ui.clip_rect());
            // each line's rows: the top of its first and the bottom of its last (one row a line, unless the text wraps)
            let mut spans: Vec<(f32, f32)> = Vec::with_capacity(lines);
            let mut starts = true;
            for row in &out.galley.rows {
                let r = row.rect();
                match spans.last_mut() {
                    Some(s) if !starts => s.1 = r.bottom(),
                    _ => spans.push((r.top(), r.bottom())),
                }
                starts = row.ends_with_newline;
            }
            let first = spans.partition_point(|s| origin.y + s.1 < clip.top());
            let last = spans.partition_point(|s| origin.y + s.0 < clip.bottom()).min(l.cols.len());
            // the caret's row, across the text and the line numbers
            let caret = out.state.cursor.char_range().map(|r| out.galley.pos_from_cursor(r.primary));
            let caret_line = caret.map(|r| spans.partition_point(|s| s.1 <= r.center().y).min(spans.len().saturating_sub(1)));
            if let Some(r) = caret {
                let (y0, y1) = (origin.y + r.min.y, origin.y + r.max.y);
                ui.painter().set(caret_bg, Shape::rect_filled(Rect::from_min_max(Pos2::new(clip.left(), y0), Pos2::new(clip.right(), y1)), 0, t::CARET_ROW));
                p.rect_filled(Rect::from_min_max(Pos2::new(gutter.left(), y0), Pos2::new(gutter.right(), y1)), 0, t::CARET_ROW);
            }
            // indent guides: one column per indent step, the caret's block the lighter one; a row opened above a
            // declaration has none, so it stays clear for its label
            let active =
                caret_line.and_then(|line| (0..=l.deepest / l.unit).rev().map(|k| k * l.unit).find(|&col| guide_on(&l.cols, line.min(l.cols.len() - 1), col)));
            let mut guides = vec![];
            for col in (0..=l.deepest / l.unit).map(|k| k * l.unit) {
                let x = origin.x + col as f32 * char_w;
                let color = if active == Some(col) { t::INDENT_GUIDE_SELECTED } else { t::INDENT_GUIDE };
                for line in (first..last).filter(|&line| guide_on(&l.cols, line, col)) {
                    guides.push(Shape::line_segment([Pos2::new(x, origin.y + spans[line].0), Pos2::new(x, origin.y + spans[line].1)], Stroke::new(1.0, color)));
                }
            }
            if !wrap {
                let x = origin.x + RIGHT_MARGIN_COLUMN as f32 * char_w;
                guides.push(Shape::line_segment([Pos2::new(x, clip.top()), Pos2::new(x, clip.bottom())], Stroke::new(1.0, t::RIGHT_MARGIN)));
            }
            ui.painter().set(guides_bg, Shape::Vec(guides));
            for line in first..last {
                let color = if caret_line == Some(line) { t::LINE_NUMBER_CARET } else { t::LINE_NUMBER };
                p.text(
                    Pos2::new(gutter.right() - 16.0 * z, origin.y + spans[line].0 + row_h / 2.0),
                    egui::Align2::RIGHT_CENTER,
                    (line + 1).to_string(),
                    font.clone(),
                    color,
                );
            }
            // the row opened above each declaration: its usages, then who wrote it, from the declaration's column
            for (line, usages, author) in l.labels.iter().filter(|label| (first..=last).contains(&label.0)) {
                let Some(span) = spans.get(*line) else { continue };
                let y = origin.y + span.0 - row_h / 2.0;
                let mut x = origin.x + l.cols.get(*line).copied().flatten().unwrap_or(0) as f32 * char_w;
                if !usages.is_empty() {
                    x = ui.painter().text(Pos2::new(x, y), egui::Align2::LEFT_CENTER, usages, vision_font.clone(), t::SECONDARY).right() + 12.0 * z;
                }
                if !author.is_empty() {
                    person_icon(ui.painter(), Pos2::new(x + 5.0 * z, y), t::AUTHOR, z);
                    ui.painter().text(Pos2::new(x + 14.0 * z, y), egui::Align2::LEFT_CENTER, author, vision_font.clone(), t::AUTHOR);
                }
            }
            // parameter names: a box in the gap before each argument (hints::open); the text stays where it was
            for (h, _, w) in l.hints.iter().filter(|hint| (first..last).contains(&hint.1)) {
                let r = out.galley.pos_from_cursor(CCursor::new(h.ch));
                let right = origin.x + r.min.x - hints::AFTER * z;
                let b = Rect::from_min_max(
                    Pos2::new(right - w - 2.0 * hints::PAD * z, origin.y + r.min.y + 2.0 * z),
                    Pos2::new(right, origin.y + r.max.y - 2.0 * z),
                );
                ui.painter().rect_filled(b, cr(4.0 * z), hints::LABEL_BG);
                ui.painter().text(b.center(), egui::Align2::CENTER_CENTER, &h.label, label_font.clone(), hints::LABEL_TEXT);
            }
            // Ctrl+hover: the name underlined like a link; Ctrl+click: its declaration
            if ui.input(|i| i.modifiers.command)
                && let Some(at) = resp.hover_pos()
            {
                let c = out.galley.cursor_from_pos(at - origin).index.0;
                let (a, b) = word_at(text, c);
                if b > a {
                    let (ra, rb) = (out.galley.pos_from_cursor(CCursor::new(a)), out.galley.pos_from_cursor(CCursor::new(b)));
                    let y = origin.y + ra.max.y - 3.0 * z;
                    ui.painter().line_segment([Pos2::new(origin.x + ra.min.x, y), Pos2::new(origin.x + rb.min.x, y)], Stroke::new(1.0, t::LINK));
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    if resp.clicked() {
                        ctrl_click = Some(c);
                    }
                }
            }
            if let Some(span) = landed.and_then(|line| spans.get(line)) {
                ui.scroll_to_rect(Rect::from_min_size(Pos2::new(origin.x, origin.y + span.0), Vec2::new(1.0, row_h)), Some(egui::Align::Center));
            }
        });
    });
    (changed, ctrl_click)
}
