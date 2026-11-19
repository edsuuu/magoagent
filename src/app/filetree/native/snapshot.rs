use crate::app::filetree::changes::sorted;
use crate::app::filetree::problems::{ERROR, WARNING};
use crate::app::filetree::ui::{badge, name_color, status_color};
use crate::app::filetree::*;
use crate::editor::highlight::{self, St};
use crate::editor::{icons, theme as t};
use crate::prelude::*;
use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn hex(color: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
}

/// With the alpha last (`#RRGGBBAA`), for the colors drawn over the text.
fn hex_alpha(color: Color32) -> String {
    format!("{}{:02X}", hex(color), color.a())
}

/// A row's or a tab's icon: an embedded SVG by key (`filetree:icons`), or a lookalike badge (a colored box with letters).
#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct IconRef {
    key: Option<&'static str>,
    label: Option<&'static str>,
    color: Option<String>,
}

pub(super) fn icon_of(path: &Path, dir: bool) -> IconRef {
    let svg = |key| IconRef { key: Some(key), label: None, color: None };

    if dir {
        return svg(icons::FOLDER.0);
    }

    if let Some((key, _)) = icons::file_icon(path) {
        return svg(key);
    }

    match icons::badge(path) {
        Some((rgb, label)) => IconRef { key: None, label: Some(label), color: Some(format!("#{rgb:06X}")) },
        None => svg(icons::FT_UNKNOWN.0),
    }
}

#[derive(Serialize)]
pub(super) struct IconPair {
    dark: &'static str,
    light: String,
}

/// Every embedded icon in both palettes (the dark card's and the light one's), by key. Sent once.
pub(super) fn icon_table() -> BTreeMap<&'static str, IconPair> {
    icons::ALL.iter().map(|(name, svg)| (*name, IconPair { dark: svg, light: icons::to_light(svg) })).collect()
}

/// The highlight styles in the order the spans' kind numbers go: 0 is plain text, 12 to 16 the bracket levels.
const STYLES: [St; 17] = [
    St::Text,
    St::Keyword,
    St::Str,
    St::Number,
    St::Comment,
    St::Doc,
    St::Function,
    St::Method,
    St::Field,
    St::Constant,
    St::Meta,
    St::Tag,
    St::Bracket(0),
    St::Bracket(1),
    St::Bracket(2),
    St::Bracket(3),
    St::Bracket(4),
];

pub(super) fn kind_id(style: St) -> u32 {
    match style {
        St::Text => 0,
        St::Keyword => 1,
        St::Str => 2,
        St::Number => 3,
        St::Comment => 4,
        St::Doc => 5,
        St::Function => 6,
        St::Method => 7,
        St::Field => 8,
        St::Constant => 9,
        St::Meta => 10,
        St::Tag => 11,
        St::Bracket(depth) => 12 + u32::from(depth) % 5,
    }
}

#[derive(Serialize)]
struct Kind {
    color: String,
    italic: bool,
}

/// The code editor's colors (Islands Dark, as the egui editor paints it), sent once: `kinds[n]` styles the spans of kind `n`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Theme {
    kinds: Vec<Kind>,
    background: String,
    text: String,
    caret: String,
    caret_row: String,
    line_number: String,
    line_number_caret: String,
    selection: String,
    search_match: String,
    muted: String,
    link: String,
    string: String,
    comment: String,
    error: String,
    warning: String,
    added: String,
    modified: String,
    deleted: String,
    removed_line: String,
    added_line: String,
    removed_word: String,
    added_word: String,
    tab_selected: String,
    tab_border: String,
    right_margin: String,
}

pub(super) fn theme() -> Theme {
    use crate::app::filetree::compare as cmp;
    use crate::app::filetree::diff as dif;

    Theme {
        kinds: STYLES.iter().map(|style| Kind { color: hex(highlight::color(*style)), italic: matches!(style, St::Doc | St::Constant) }).collect(),
        background: hex(t::ISLAND),
        text: hex(t::EDITOR_TEXT),
        caret: hex(t::CARET),
        caret_row: hex(t::CARET_ROW),
        line_number: hex(t::LINE_NUMBER),
        line_number_caret: hex(t::LINE_NUMBER_CARET),
        selection: hex(t::EDITOR_SELECTION),
        search_match: hex_alpha(t::SEARCH_MATCH),
        muted: hex(t::MUTED),
        link: hex(t::LINK),
        string: hex(t::STRING),
        comment: hex(t::COMMENT),
        error: hex(t::ERROR),
        warning: hex(t::WARNING),
        added: hex(dif::ADDED),
        modified: hex(dif::MODIFIED),
        deleted: hex(dif::DELETED),
        removed_line: hex(cmp::REMOVED),
        added_line: hex(cmp::ADDED),
        removed_word: hex(cmp::REMOVED_WORD),
        added_word: hex(cmp::ADDED_WORD),
        tab_selected: hex(t::TAB_SELECTED),
        tab_border: hex(t::TAB_BORDER),
        right_margin: hex(t::RIGHT_MARGIN),
    }
}

#[derive(Serialize)]
pub(super) struct GitOut {
    repo: bool,
    branch: String,
    changes: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RowOut {
    path: String,
    name: String,
    dir: bool,
    depth: usize,
    open: bool,
    hint: String,
    icon: IconRef,
    /// The name's color (the git mark's, or the ignored gray, or the error red); None: the card's own.
    color: Option<String>,
    letter: Option<String>,
    letter_color: Option<String>,
    ignored: bool,
    problem: bool,
}

#[derive(Serialize)]
pub(super) struct ClipOut {
    count: usize,
    cut: bool,
}

#[derive(Serialize)]
pub(super) struct SortOut {
    key: &'static str,
    label: &'static str,
}

#[derive(Serialize)]
pub(super) struct ChangeOut {
    path: String,
    name: String,
    dir: String,
    letter: String,
    color: String,
    icon: IconRef,
}

#[derive(Serialize)]
pub(super) struct TabOut {
    path: String,
    name: String,
    icon: IconRef,
    dirty: bool,
    preview: bool,
    kind: &'static str,
}

#[derive(Serialize, Clone)]
pub(super) struct ProblemOut {
    line: usize,
    col: usize,
    severity: u8,
    message: String,
}

#[derive(Serialize, Clone)]
pub(super) struct ProblemFileOut {
    path: String,
    rel: String,
    problems: Vec<ProblemOut>,
}

/// The project's problems, read now and then (listing them asks every known file's).
#[derive(Default)]
pub(super) struct ProblemsOut {
    files: Vec<ProblemFileOut>,
    errors: usize,
    warnings: usize,
}

impl ProblemsOut {
    pub(super) fn read(ed: &crate::app::filetree::pane::Editor, root: &Path) -> Self {
        let mut files: Vec<(PathBuf, Vec<crate::editor::lsp::Diag>)> = ed
            .problems
            .files()
            .into_iter()
            .map(|file| {
                let diags = ed.problems.for_file(&file);

                (file, diags)
            })
            .filter(|(_, diags)| !diags.is_empty())
            .collect();

        files.sort_by(|a, b| a.0.cmp(&b.0));
        let count = |severity: u8| files.iter().flat_map(|(_, diags)| diags).filter(|d| d.severity == severity).count();
        let (errors, warnings) = (count(ERROR), count(WARNING));

        let files = files
            .into_iter()
            .take(200)
            .map(|(path, diags)| ProblemFileOut {
                rel: slashes(path.strip_prefix(root).unwrap_or(&path)),
                path: path.to_string_lossy().into_owned(),
                problems: diags.into_iter().take(100).map(|d| ProblemOut { line: d.line, col: d.col, severity: d.severity, message: d.message }).collect(),
            })
            .collect();

        ProblemsOut { files, errors, warnings }
    }
}

/// The tree's topic (`filetree:<id>`): everything but the open file's text, which has its own (`:doc`, `:live`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TreeOut {
    id: String,
    name: String,
    root: String,
    shown: String,
    dark: bool,
    can_back: bool,
    can_forward: bool,
    git: GitOut,
    filter: String,
    searching: bool,
    tree_rows: usize,
    hits_cut: bool,
    dropped: usize,
    empty_note: Option<String>,
    rows: Vec<RowOut>,
    selected: Option<String>,
    clipboard: Option<ClipOut>,
    hide_hidden: bool,
    sort: &'static str,
    sorts: Vec<SortOut>,
    reversed: bool,
    editor_open: bool,
    changes_open: bool,
    tree_width: Option<f32>,
    branches: Vec<String>,
    changes: Vec<ChangeOut>,
    tabs: Vec<TabOut>,
    active: Option<String>,
    wrap: bool,
    find_open: bool,
    problems_open: bool,
    problems: Vec<ProblemFileOut>,
    error_count: usize,
    warning_count: usize,
    /// The words of the "Voltar" button while a link opened a file in this tree (`link_back_ui`); None: no button.
    link_back: Option<&'static str>,
}

pub(super) fn tab_kind(body: &crate::app::filetree::editor::Body) -> &'static str {
    use crate::app::filetree::editor::Body;

    match body {
        Body::Text(_) => "text",
        Body::Media(_) => "media",
        Body::Refused(_) => "refused",
        Body::External(_) => "external",
        Body::Diff(_) => "diff",
        Body::Compare(_) => "compare",
        Body::Results(_) => "results",
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.to_string_lossy().into_owned(), |name| name.to_string_lossy().into_owned())
}

fn slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl DrawingApp {
    /// The tree `id` as the SwiftUI view reads it: the rows with their marks, the git state, the panels' lists and the editor's tabs.
    pub(super) fn native_tree_state(&self, id: &str, problems_open: bool, problems: &ProblemsOut) -> Option<TreeOut> {
        let spec = self.ft_spec(id)?;
        let view = self.filetrees.views.get(id)?;
        let dark = self.dark();
        let root = spec.root_path();

        let rows = view
            .rows
            .iter()
            .map(|row| {
                let mark = view.git.of(&row.path, row.dir);
                let ignored = view.git.ignored(&row.path);
                let problem = view.problems.contains(&row.path);
                let color = if problem { Some(crate::app::filetree::problems_ui::error_color(dark)) } else { name_color(mark, ignored, dark) };
                // the letter only on a file: a folder's state is its name's color
                let letter = mark.filter(|(_, rolled)| !rolled && !row.dir);

                RowOut {
                    path: row.path.to_string_lossy().into_owned(),
                    name: row.name.clone(),
                    dir: row.dir,
                    depth: row.depth,
                    open: row.open,
                    hint: row.hint.clone(),
                    icon: icon_of(&row.path, row.dir),
                    color: color.map(hex),
                    letter: letter.map(|(status, _)| badge(status).to_string()),
                    letter_color: letter.map(|(status, _)| hex(status_color(status, dark))),
                    ignored,
                    problem,
                }
            })
            .collect();

        let changes = if spec.changes {
            sorted(&root, &view.git.files)
                .into_iter()
                .map(|(path, status)| ChangeOut {
                    name: file_name(&path),
                    icon: icon_of(&path, false),
                    dir: path.strip_prefix(&root).ok().and_then(Path::parent).map(slashes).unwrap_or_default(),
                    letter: badge(status).to_string(),
                    color: hex(status_color(status, dark)),
                    path: path.to_string_lossy().into_owned(),
                })
                .collect()
        } else {
            vec![]
        };

        let editor = self.filetrees.editors.get(id).filter(|_| spec.editor);

        let tabs = editor
            .map(|ed| {
                ed.tabs
                    .iter()
                    .map(|tab| TabOut {
                        path: tab.path.to_string_lossy().into_owned(),
                        name: file_name(&tab.path),
                        icon: match tab.body {
                            crate::app::filetree::editor::Body::Compare(_) => IconRef { key: Some(icons::DIFF.0), label: None, color: None },
                            _ => icon_of(&tab.path, false),
                        },
                        dirty: tab.dirty(),
                        preview: ed.preview.as_ref() == Some(&tab.path),
                        kind: tab_kind(&tab.body),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Some(TreeOut {
            id: id.to_string(),
            name: spec.name(),
            root: root.to_string_lossy().into_owned(),
            shown: spec.shown().to_string_lossy().into_owned(),
            dark,
            can_back: spec.shown() != root,
            can_forward: !view.forward.is_empty(),
            git: GitOut { repo: view.git.repo, branch: view.git.branch.clone(), changes: view.git.changes },
            filter: view.filter.clone(),
            searching: !view.filter.trim().is_empty(),
            tree_rows: view.tree_rows,
            hits_cut: view.hits_cut,
            dropped: view.dropped,
            empty_note: view.empty_note(spec),
            rows,
            selected: view.selected.as_ref().map(|p| p.to_string_lossy().into_owned()),
            clipboard: self.filetrees.clipboard.as_ref().map(|(paths, cut)| ClipOut { count: paths.len(), cut: *cut }),
            hide_hidden: spec.hide_hidden,
            sort: spec.sort.key(),
            sorts: SortBy::ALL.iter().map(|by| SortOut { key: by.key(), label: by.label() }).collect(),
            reversed: spec.reversed,
            editor_open: spec.editor,
            changes_open: spec.changes,
            tree_width: spec.tree_width,
            branches: view.branches.clone(),
            changes,
            tabs,
            active: editor.and_then(|ed| ed.active.as_ref()).map(|p| p.to_string_lossy().into_owned()),
            wrap: editor.is_some_and(|ed| ed.wrap),
            find_open: editor.is_some_and(|ed| ed.find.is_some()),
            problems_open,
            problems: if problems_open { problems.files.clone() } else { vec![] },
            error_count: problems.errors,
            warning_count: problems.warnings,
            link_back: self
                .filetrees
                .link_back
                .as_ref()
                .filter(|back| back.tree == id && !self.chat_open)
                .map(|back| if back.chat_open { "Voltar ao chat" } else { "Voltar" }),
        })
    }
}
