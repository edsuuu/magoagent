//! The lists the labels over a declaration open in a file tree node's editor pane, as the reference IDE's: the usages of
//! a name (what Ctrl+click on a declaration lists too) and the history of a declaration's lines (`git log -L`), in a small
//! popup hung where the press was. Picking a usage opens its file at the line; picking a commit opens what it changed in
//! the file, in a tab.

use crate::app::filetree::code::{Project, usages_of};
use crate::ide::Status;
use crate::ide::find::{Hit, match_text, popup_frame};
use crate::ide::theme as t;
use crate::ide::{icons, index, run_git};
use crate::prelude::*;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};

/// The popup's width, a row's height and how many rows it shows before it scrolls.
const WIDTH: f32 = 640.0;
const ROW: f32 = 24.0;
const ROWS: f32 = 12.0;

/// A commit in the history of some lines.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Commit {
    pub(crate) hash: String,
    pub(crate) author: String,
    /// When it was made, in seconds since 1970.
    pub(crate) time: i64,
    pub(crate) subject: String,
}

fn in_folder(path: &Path) -> Result<(&Path, String), String> {
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => Ok((dir, name.to_string_lossy().into_owned())),
        _ => Err(format!("{} não é um arquivo.", path.display())),
    }
}

/// The commits that changed lines `from..to` (counted from 0, half-open) of `path`, the newest first: `git log -L`.
// ponytail: the lines are the working copy's, read against the last commit's file, so uncommitted lines above them
// shift the range (past the file's end it is cut to it); map the lines through the blame if that bites
pub(crate) fn line_history(path: &Path, (from, to): (usize, usize)) -> Result<Vec<Commit>, String> {
    let (dir, name) = in_folder(path)?;
    let log = |from: usize, to: usize| {
        let range = format!("-L{},{}:{name}", from + 1, to.max(from + 1));
        run_git(dir, &["log", "--no-patch", "--format=%H%x1f%an%x1f%at%x1f%s", range.as_str()])
    };
    let out = log(from, to).or_else(|why| {
        // "file x has only 40 lines": the last commit's file is shorter than the working copy
        let lines = why.split("has only ").nth(1).and_then(|rest| rest.split(' ').next()?.parse::<usize>().ok()).filter(|n| *n > 0).ok_or(why)?;
        log(from.min(lines - 1), to.min(lines))
    })?;
    let commit = |line: &str| {
        let mut parts = line.split('\x1f');
        Some(Commit { hash: parts.next()?.into(), author: parts.next()?.into(), time: parts.next()?.parse().ok()?, subject: parts.next()?.into() })
    };
    Ok(out.lines().filter_map(commit).collect())
}

/// An agent names the commit: nothing but a hash goes to git.
fn check_hash(hash: &str) -> Result<(), String> {
    if (4..=64).contains(&hash.len()) && hash.chars().all(|c| c.is_ascii_hexdigit()) { Ok(()) } else { Err(format!("{hash} não é o hash de um commit.")) }
}

/// What commit `hash` changed in `path`, as `git show` writes it, under a line with the commit, who made it and when.
pub(crate) fn commit_diff(path: &Path, hash: &str) -> Result<String, String> {
    check_hash(hash)?;
    let (dir, name) = in_folder(path)?;
    run_git(dir, &["show", "--format=%h  %an, %ad%n%s%n", "--date=format:%d/%m/%Y %H:%M", hash, "--", name.as_str()])
}

/// The files commit `hash` changed in the repository of `path` (a file in it): git's mark and the whole path, with the
/// path from the repository's root.
pub(crate) fn commit_files(path: &Path, hash: &str) -> Result<Vec<(Status, PathBuf, String)>, String> {
    check_hash(hash)?;
    let (dir, _) = in_folder(path)?;
    let root = PathBuf::from(run_git(dir, &["rev-parse", "--show-toplevel"])?.trim());
    let out = run_git(dir, &["show", "--name-status", "--format=", hash])?;
    Ok(out.lines().filter_map(|l| parse_name_status(l).map(|(s, rel)| (s, root.join(&rel), rel))).collect())
}

/// A line of `git show --name-status`: "M\tsrc/a.rs", "R100\told\tnew" (the new name counts).
pub(crate) fn parse_name_status(line: &str) -> Option<(Status, String)> {
    let mut parts = line.split('\t');
    let status = match parts.next()?.chars().next()? {
        'A' | 'C' => Status::Added,
        'D' => Status::Deleted,
        'R' => Status::Renamed,
        _ => Status::Modified,
    };
    Some((status, parts.last()?.to_string()))
}

/// How long before `now` the moment `time` was (both in seconds): "agora", "há 5 min", "há 3 h", "ontem", "há 4 dias",
/// "há 2 meses", "há 1 ano".
pub(crate) fn age(time: i64, now: i64) -> String {
    let s = (now - time).max(0);
    let some = |n: i64, one: &str, many: &str| format!("há {n} {}", if n == 1 { one } else { many });
    match s {
        0..60 => "agora".into(),
        60..3600 => format!("há {} min", s / 60),
        3600..86_400 => format!("há {} h", s / 3600),
        86_400..172_800 => "ontem".into(),
        172_800..2_592_000 => format!("há {} dias", s / 86_400),
        2_592_000..31_536_000 => some(s / 2_592_000, "mês", "meses"),
        _ => some(s / 31_536_000, "ano", "anos"),
    }
}

pub(crate) fn now() -> i64 {
    SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// What the declaration of `name` on `line` is, in the reference IDE's words, and whether the name is a local one (used
/// in its own file only).
pub(crate) fn decl_kind(lang: &str, line: &str, name: &str) -> (&'static str, bool) {
    use index::Kind::*;
    if let Some(d) = index::declarations(lang, line).into_iter().find(|d| d.name == name) {
        let kind = match d.kind {
            Class => "Classe",
            Interface => "Interface",
            Trait => "Trait",
            Enum => "Enum",
            Struct => "Struct",
            Function => "Função",
            Method => "Método",
            Constant => "Constante",
            Type => "Tipo",
            Module => "Módulo",
        };
        return (kind, false);
    }
    match line.split(name).next().and_then(|before| before.split_whitespace().last()).unwrap_or_default() {
        "const" | "static" => ("Constante", false),
        "let" | "var" | "mut" => ("Variável", true),
        "macro_rules!" => ("Macro", false),
        _ => ("Símbolo", false),
    }
}

/// "Constante KINDS (em events)": the popup's title for the usages of `name`, declared in `path`.
pub(crate) fn usages_title(kind: &str, name: &str, path: &Path) -> String {
    format!("{kind} {name} (em {})", path.file_stem().unwrap_or_default().to_string_lossy())
}

pub(crate) fn usages_count(n: usize) -> String {
    match n {
        0 => "sem usos".into(),
        1 => "1 uso".into(),
        n => format!("{n} usos"),
    }
}

pub(crate) fn files_count(n: usize) -> String {
    match n {
        1 => "1 arquivo".into(),
        n => format!("{n} arquivos"),
    }
}

pub(crate) fn commits_count(n: usize) -> String {
    match n {
        0 => "sem commits".into(),
        1 => "1 commit".into(),
        n => format!("{n} commits"),
    }
}

/// What a popup lists: the usages of a name, or the commits of some lines of a file.
pub(crate) enum Rows {
    Usages(Vec<Hit>),
    // ponytail: no press opens the history list since the author turns on the annotation column; a "Histórico" entry would bring it back
    #[allow(dead_code)]
    Commits(PathBuf, Vec<Commit>),
    /// The files a commit changed (a click on the annotation column): git's mark, the path and the path from the root.
    Files(Commit, Vec<(Status, PathBuf, String)>),
}

impl Rows {
    fn len(&self) -> usize {
        match self {
            Rows::Usages(hits) => hits.len(),
            Rows::Commits(_, commits) => commits.len(),
            Rows::Files(_, files) => files.len(),
        }
    }
}

/// The row picked in a popup: a usage (file, line, column) or a commit of a file.
pub(crate) enum Picked {
    Usage(PathBuf, usize, usize),
    Commit(PathBuf, Commit),
}

/// The popup of a declaration's usages or history: its rows come from a thread.
pub(crate) struct ListPopup {
    pub(crate) title: String,
    anchor: Pos2,
    pub(crate) rows: Rows,
    sel: usize,
    /// Why there is nothing to list (git's own words).
    note: String,
    rx: Option<Receiver<Result<Rows, String>>>,
    /// The width of the rows' first column, measured once the rows are known.
    lead: Option<f32>,
}

impl ListPopup {
    fn new(title: String, anchor: Pos2, rows: Rows, ctx: &Context, work: impl FnOnce() -> Result<Rows, String> + Send + 'static) -> Self {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
        ListPopup { title, anchor, rows, sel: 0, note: String::new(), rx: Some(rx), lead: None }
    }

    /// The usages of `name`, declared at `own` (file, line, column) by the line `decl` of a file in language `lang`.
    pub(crate) fn usages(code: &Project, name: &str, own: (&Path, usize, usize), decl: &str, lang: &str, anchor: Pos2, ctx: &Context) -> Self {
        let (kind, local) = decl_kind(lang, decl, name);
        let (files, decls) = code.usage_scope(name, own, local);
        let (word, current) = (name.to_string(), own.0.to_path_buf());
        Self::new(usages_title(kind, name, own.0), anchor, Rows::Usages(vec![]), ctx, move || Ok(Rows::Usages(usages_of(&word, &files, &decls, &current))))
    }

    /// The commits that changed the lines `span` of `path`: the declaration of `name`.
    #[allow(dead_code)]
    pub(crate) fn history(path: &Path, name: &str, span: (usize, usize), anchor: Pos2, ctx: &Context) -> Self {
        let file = path.to_path_buf();
        Self::new(format!("Histórico de {name}"), anchor, Rows::Commits(file.clone(), vec![]), ctx, move || {
            line_history(&file, span).map(|commits| Rows::Commits(file, commits))
        })
    }
}

impl ListPopup {
    /// The files `commit` changed, in the repository of `path`: a press on that commit's line in the annotation column.
    pub(crate) fn files(path: &Path, commit: Commit, anchor: Pos2, ctx: &Context) -> Self {
        let title = format!("{} {}, {}", &commit.hash[..commit.hash.len().min(8)], commit.author, age(commit.time, now()));
        let file = path.to_path_buf();
        let rows = Rows::Files(commit.clone(), vec![]);
        Self::new(title, anchor, rows, ctx, move || commit_files(&file, &commit.hash).map(|files| Rows::Files(commit, files)))
    }
}

/// The popup in `slot`, hung under where the press was, with the keys while it is open (↑↓, Enter, Esc). Back: the row
/// picked. Esc, a pick or a press outside it closes it.
pub(crate) fn popup_ui(ctx: &Context, key: &str, slot: &mut Option<ListPopup>) -> Option<Picked> {
    let popup = slot.as_mut()?;
    if let Some(result) = popup.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
        popup.rx = None;
        match result {
            Ok(rows) => popup.rows = rows,
            Err(why) => popup.note = why,
        }
    }
    let n = popup.rows.len();
    let key_of = |i: &mut egui::InputState, key: Key| i.consume_key(Modifiers::NONE, key);
    let (down, up, enter, esc) = ctx.input_mut(|i| (key_of(i, Key::ArrowDown), key_of(i, Key::ArrowUp), key_of(i, Key::Enter), key_of(i, Key::Escape)));
    if down {
        popup.sel = (popup.sel + 1).min(n.saturating_sub(1));
    }
    if up {
        popup.sel = popup.sel.saturating_sub(1);
    }
    let mut pick = enter.then_some(popup.sel).filter(|i| *i < n);
    let shown =
        egui::Area::new(egui::Id::unique(("ft_vision", key))).order(egui::Order::Foreground).fixed_pos(popup.anchor + Vec2::new(0.0, 4.0)).show(ctx, |ui| {
            t::style(ui.style_mut());
            popup_frame(ui).show(ui, |ui| {
                ui.set_width(WIDTH);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                // the title, and how many rows; it holds the keyboard, so the keys are the popup's and not the code's
                let (head, holder) = ui.allocate_exact_size(Vec2::new(WIDTH, 32.0), Sense::click());
                holder.request_focus();
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(holder.id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true })
                });
                let count = match &popup.rows {
                    _ if popup.rx.is_some() => "procurando…".to_string(),
                    Rows::Usages(hits) => usages_count(hits.len()),
                    Rows::Commits(_, commits) => commits_count(commits.len()),
                    Rows::Files(_, files) => files_count(files.len()),
                };
                let count = ui.painter().text(head.right_center() - Vec2::new(12.0, 0.0), egui::Align2::RIGHT_CENTER, count, t::ui(t::UI_SIZE), t::SECONDARY);
                let title = ui.painter().with_clip_rect(head.with_max_x(count.left() - 12.0));
                // ponytail: the IDE's one weight of its font; the title is struck twice, a hair apart, as the card's name
                for dx in [0.0, 0.4] {
                    title.text(head.left_center() + Vec2::new(12.0 + dx, 0.0), egui::Align2::LEFT_CENTER, &popup.title, t::ui(t::UI_SIZE), t::TEXT);
                }
                ui.painter().hline(head.x_range(), head.bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                // the first column is as wide as its widest text
                let width = |ui: &egui::Ui, text: &str| ui.painter().layout_no_wrap(text.to_string(), t::ui(t::UI_SIZE), t::TEXT).size().x;
                let lead = *popup.lead.get_or_insert_with(|| match &popup.rows {
                    Rows::Usages(hits) => hits.iter().map(|h| width(ui, &name_of(&h.path))).fold(0.0, f32::max).min(220.0),
                    Rows::Commits(_, commits) => commits.iter().map(|c| width(ui, &c.author)).fold(0.0, f32::max).min(160.0),
                    Rows::Files(_, files) => files.iter().map(|f| width(ui, &name_of(&f.1))).fold(0.0, f32::max).min(220.0),
                });
                if popup.rx.is_some() {
                    popup.lead = None;
                }
                let now = now();
                egui::ScrollArea::vertical().max_height(ROW * ROWS).show(ui, |ui| {
                    ui.set_width(WIDTH);
                    for i in 0..n {
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(WIDTH, ROW), Sense::click());
                        let fill = if i == popup.sel {
                            t::SELECTION
                        } else if resp.hovered() {
                            t::ROW_HOVER
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(rect.shrink2(Vec2::new(6.0, 0.0)), 5, fill);
                        let p = ui.painter().with_clip_rect(rect.shrink2(Vec2::new(12.0, 0.0)));
                        let y = rect.center().y;
                        match &popup.rows {
                            Rows::Usages(hits) => {
                                // the file's icon and name, the line, then the line of code with the name lit
                                let hit = &hits[i];
                                icons::file(&p, &hit.path, Pos2::new(rect.left() + 22.0, y), 1.0);
                                p.text(Pos2::new(rect.left() + 36.0, y), egui::Align2::LEFT_CENTER, name_of(&hit.path), t::ui(t::UI_SIZE), t::TEXT);
                                let line = Pos2::new(rect.left() + 36.0 + lead + 44.0, y);
                                p.text(line, egui::Align2::RIGHT_CENTER, (hit.line + 1).to_string(), t::ui(t::UI_SIZE), t::SECONDARY);
                                match_text(ui, Pos2::new(line.x + 14.0, y), hit, rect.right() - 12.0 - line.x - 14.0);
                            }
                            Rows::Commits(_, commits) => {
                                // who, when, and the commit's first line
                                let commit = &commits[i];
                                p.text(Pos2::new(rect.left() + 16.0, y), egui::Align2::LEFT_CENTER, &commit.author, t::ui(t::UI_SIZE), t::TEXT);
                                let when = Pos2::new(rect.left() + 16.0 + lead + 14.0, y);
                                p.text(when, egui::Align2::LEFT_CENTER, age(commit.time, now), t::ui(t::UI_SIZE), t::SECONDARY);
                                p.text(when + Vec2::new(86.0, 0.0), egui::Align2::LEFT_CENTER, &commit.subject, t::ui(t::UI_SIZE), t::EDITOR_TEXT);
                            }
                            Rows::Files(_, files) => {
                                // git's mark in its color, the file's icon and name, then its folder
                                let (status, path, rel) = &files[i];
                                let mark = crate::app::filetree::ui::badge(*status).to_string();
                                p.text(Pos2::new(rect.left() + 16.0, y), egui::Align2::LEFT_CENTER, mark, t::ui(t::UI_SIZE), status.color());
                                icons::file(&p, path, Pos2::new(rect.left() + 38.0, y), 1.0);
                                p.text(Pos2::new(rect.left() + 52.0, y), egui::Align2::LEFT_CENTER, name_of(path), t::ui(t::UI_SIZE), status.color());
                                let folder = Path::new(rel).parent().map(|d| d.display().to_string()).unwrap_or_default();
                                p.text(Pos2::new(rect.left() + 52.0 + lead + 14.0, y), egui::Align2::LEFT_CENTER, folder, t::ui(t::UI_SIZE), t::SECONDARY);
                            }
                        }
                        if i == popup.sel && (up || down) {
                            resp.scroll_to_me(None);
                        }
                        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            pick = Some(i);
                        }
                    }
                });
                // under the rows: the selected usage's whole path, or the selected commit and its whole first line
                let foot = match &popup.rows {
                    _ if !popup.note.is_empty() => popup.note.clone(),
                    Rows::Usages(hits) => hits.get(popup.sel).map(|h| h.path.display().to_string()).unwrap_or_default(),
                    Rows::Commits(_, commits) => {
                        commits.get(popup.sel).map(|c| format!("{}  {}", &c.hash[..c.hash.len().min(8)], c.subject)).unwrap_or_default()
                    }
                    Rows::Files(commit, _) => commit.subject.clone(),
                };
                if !foot.is_empty() {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(WIDTH, 26.0), Sense::hover());
                    ui.painter().hline(rect.x_range(), rect.top(), Stroke::new(1.0, t::POPUP_BORDER));
                    let p = ui.painter().with_clip_rect(rect.shrink2(Vec2::new(12.0, 0.0)));
                    p.text(rect.left_center() + Vec2::new(12.0, 0.0), egui::Align2::LEFT_CENTER, foot, t::ui(t::UI_SIZE - 1.0), t::SECONDARY);
                }
            });
        });
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !shown.response.rect.contains(p)));
    let picked = pick.map(|i| match &popup.rows {
        Rows::Usages(hits) => Picked::Usage(hits[i].path.clone(), hits[i].line, hits[i].col),
        Rows::Commits(file, commits) => Picked::Commit(file.clone(), commits[i].clone()),
        Rows::Files(commit, files) => Picked::Commit(files[i].1.clone(), commit.clone()),
    });
    if esc || outside || picked.is_some() {
        *slot = None;
    }
    picked
}

fn name_of(path: &Path) -> String {
    path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())
}
