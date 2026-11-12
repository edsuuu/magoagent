//! The code in a file tree node's editor pane, drawn as the IDE's editor draws it (Islands Dark, its code font and so its
//! ligatures): line numbers, the caret's row, indent guides, the right margin, the bracket pair, the usages and the author
//! over each declaration (pressed, they list the usages and the history: filetree/vision.rs), the parameter names inside
//! calls, and Ctrl+click to a declaration, or on it to its usages. No IDE node behind it: only the IDE's pure parts
//! (highlight, hints, index, text, find) over a `Project` kept here for the tree's root. What needs a language server
//! (problems, documentation, signatures) stays in the IDE.

use crate::app::filetree::annotate::{Annot, gutter_menu, hover_line, paint_column};
use crate::app::filetree::editor::TextBuf;
use crate::app::filetree::problems::next_problem;
use crate::app::filetree::problems_ui::{STRIPE, chrome, spans as problem_spans, tip, waves};
use crate::app::filetree::ui::cr;
use crate::ide::editor::{RIGHT_MARGIN_COLUMN, caret_guide, decl_author, decl_span, indent_guides, indent_of, person_icon, restyle};
use crate::ide::find::{self, Hit};
use crate::ide::highlight::{self, Kind, Lang};
use crate::ide::hints::{self, Hint};
use crate::ide::index::{self, Index};
use crate::ide::lsp::Diag;
use crate::ide::text::{char_at, family, find_declaration, framework_target, hash, line_of, word_at};
use crate::ide::theme as t;
use crate::prelude::*;
use egui::text::{CCursor, LayoutJob};
use egui::text_selection::CCursorRange;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
mod text;
pub(crate) use text::*;

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
/// The most usages a declaration lists.
const USAGES_MAX: usize = 1000;

/// The code files under `root`: what the project indexes.
pub(crate) fn code_files(root: &Path) -> Vec<PathBuf> {
    files_under(root, INDEX_MAX, index::indexable)
}

/// The files under `root` that `keep` lets through, without the folders the IDE leaves out of a project (node_modules,
/// target, what the .gitignore names…), until there are `max` of them or the walk has seen `WALK_MAX` entries.
pub(crate) fn files_under(root: &Path, max: usize, keep: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let excluded = crate::ide::excluded::Excluded::load(root);
    let (mut out, mut stack, mut seen) = (vec![], vec![root.to_path_buf()], 0);
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            seen += 1;
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                // a folder with a .git of its own is another checkout (an agent's worktree, a nested repository), not this
                // project's code: copies of it would be indexed many times over
                if !excluded.contains(&path) && !path.join(".git").exists() {
                    stack.push(path);
                }
            } else if keep(&path) {
                out.push(path);
            }
            if out.len() >= max || seen >= WALK_MAX {
                return out;
            }
        }
    }
    out
}

/// The usages of `name` in `files`: its whole-word, same-case matches, without the declarations at `decls` (file, line,
/// column), the ones in `current` first.
// ponytail: by the name alone, as the count over the declaration: another symbol of the same name is listed too
pub(crate) fn usages_of(name: &str, files: &[PathBuf], decls: &[(PathBuf, usize, usize)], current: &Path) -> Vec<Hit> {
    let mut hits = find::search(files, name, true, true, &Default::default(), 0, USAGES_MAX);
    hits.retain(|h| !decls.iter().any(|d| d.0 == h.path && d.1 == h.line && d.2 == h.col));
    hits.sort_by_key(|h| h.path != current);
    hits
}

/// Where a Ctrl+click's search ended.
#[derive(Debug, PartialEq)]
pub(crate) enum Found {
    At(PathBuf, usize, usize),
    Nowhere(String),
    /// The click was on the declaration itself (its name, file, line and column): its usages are listed instead.
    Own(String, PathBuf, usize, usize),
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
    blame_rx: Option<Receiver<(PathBuf, Vec<Option<String>>, Annot)>>,
    /// The same blame as a date and an author per line, for the annotation column (filetree/annotate.rs).
    annot: HashMap<PathBuf, Annot>,
    /// The name a Ctrl+click asked for and where it was pressed (file, line, the name's columns), while its
    /// declaration is searched.
    #[allow(clippy::type_complexity)]
    decl: Option<(String, (PathBuf, usize, usize, usize), Receiver<Option<(PathBuf, usize, usize)>>)>,
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
        if let Some((path, lines, annot)) = self.blame_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.blame_rx = None;
            self.annot.insert(path.clone(), annot);
            self.blame.insert(path, lines);
            self.generation += 1;
        }
        let found = self.decl.as_ref()?.2.try_recv().ok()?;
        let (word, pressed, _) = self.decl.take()?;
        Some(match found {
            None => Found::Nowhere(word),
            Some((path, line, col)) if path == pressed.0 && line == pressed.1 && (pressed.2..pressed.3).contains(&col) => Found::Own(word, path, line, col),
            Some((path, line, col)) => Found::At(path, line, col),
        })
    }

    /// The files a usage of `name` may be in and the declarations left out of its usages: every declaration of that name
    /// the index knows, and the one at `own`. A local name (`let`, `var`) is only used in its own file.
    pub(crate) fn usage_scope(&self, name: &str, own: (&Path, usize, usize), local: bool) -> (Vec<PathBuf>, Vec<(PathBuf, usize, usize)>) {
        let files = match (local, self.files.is_empty()) {
            (true, _) => vec![own.0.to_path_buf()],
            // an agent may ask before the tree's editor ever opened: the root is listed now
            (false, true) => code_files(&self.root),
            (false, false) => self.files.clone(),
        };
        let known = self.index.symbols.iter().filter(|s| s.decl.name == name).map(|s| (s.path.clone(), s.decl.line, s.decl.col));
        (files, known.chain([(own.0.to_path_buf(), own.1, own.2)]).collect())
    }

    /// Who wrote each line of `path` and when, once git has said.
    pub(crate) fn annotation(&self, path: &Path) -> Option<&Annot> {
        self.annot.get(path)
    }

    /// The project's root.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Where `name` is declared, looked for now: in `current` first, then in the project's files of its language.
    pub(crate) fn declared(&self, name: &str, current: &Path) -> Option<(PathBuf, usize, usize)> {
        let fam = family(highlight::detect(current).id);
        let all = if self.files.is_empty() { code_files(&self.root) } else { self.files.clone() };
        let files: Vec<PathBuf> = all.into_iter().filter(|p| family(highlight::detect(p).id) == fam).collect();
        find_declaration(name, current, &files)
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
            let out = crate::ide::run_git(&dir, &["blame", "--porcelain", "--", name.as_str()]).unwrap_or_default();
            let _ = tx.send((path, crate::ide::parse_blame(&out), Annot::parse(&out)));
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
        (self.blame.remove(path), self.annot.remove(path));
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
        let line = line_of(text, a);
        let start = char_at(text, line, 0);
        self.decl = Some((word, (path.to_path_buf(), line, a - start, b - start), rx));
        None
    }
}

/// A declaration with a label over it: where its name is, its usages ("3 usos") and who wrote most of it.
pub(crate) struct Label {
    pub(crate) line: usize,
    pub(crate) col: usize,
    pub(crate) name: String,
    pub(crate) usages: String,
    pub(crate) author: String,
    /// The declaration's lines, as a half-open range: what its history is about.
    #[allow(dead_code)]
    pub(crate) span: (usize, usize),
}

/// What a press on the code asked.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Click {
    /// Ctrl+click on the name at this char: its declaration or, on the declaration, its usages. With where the name is.
    Name(usize, Pos2),
    /// The usages over the declaration `labels[i]`, and where they are written.
    Usages(usize, Pos2),
    /// The author over it: the history of its lines.
    Author(usize, Pos2),
    /// A line of the annotation column: the files its commit changed, listed where the press was.
    Commit(crate::app::filetree::vision::Commit, Pos2),
    /// Ctrl+Shift+F, with the selected text.
    Find(String),
    /// Ctrl+Alt+M or "Comentar para o Maestro" on a selection.
    Comment(crate::app::chat::code_comment::NoteReq),
    /// Ctrl+D, or "Mostrar Diff" in a marker's popup: the file against the last commit, in a tab.
    Diff,
    /// A problem to go to (F2, the traffic light, the error stripe): its line and column.
    Goto(usize, usize),
}

/// What is drawn of a text, worked out again only when the text, its size or the project's index changes.
pub(crate) struct Laid {
    key: (u64, u64),
    /// The colored text, with room before each named argument.
    job: LayoutJob,
    /// Each line's indent, and the blocks' indent guides (column, opening line, end line).
    cols: Vec<Option<usize>>,
    guides: Vec<(usize, usize, usize)>,
    /// The parameter names: the hint, its line and its label's width.
    pub(crate) hints: Vec<(Hint, usize, f32)>,
    /// The declarations with a label over them, in the order of their lines.
    pub(crate) labels: Vec<Label>,
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
        let guides = indent_guides(text);
        let (mut hints, mut labels): (Vec<(Hint, usize, f32)>, Vec<Label>) = (vec![], vec![]);
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
                    // "sem usos", as the reference IDE says it
                    let usages = code.index.vision(&d).unwrap_or_default().replace("nenhum uso", "sem usos");
                    let span = decl_span(&lines, d.line);
                    let author = blame.and_then(|b| decl_author(b, span)).unwrap_or_default();
                    if labels.last().is_none_or(|l| l.line != d.line) && !(usages.is_empty() && author.is_empty()) {
                        labels.push(Label { line: d.line, col: d.col, name: d.name, usages, author, span });
                    }
                }
            }
        }
        Laid { key, job, cols, guides, hints, labels }
    }
}
