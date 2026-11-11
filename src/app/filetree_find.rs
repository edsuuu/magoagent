//! "Localizar nos Arquivos" of a file tree node (Ctrl+Shift+F), as the reference IDE's: a floating window with the text to
//! find (the case told apart or not, whole words, a regular expression), an optional file mask, the scope (the whole root
//! or one folder of it), how many matches in how many files, the matches and, under them, the file around the selected
//! one. The search is the IDE's (`ide::find`) over the root's text kept in memory (`ide::textindex`), read on a thread
//! without the excluded folders. Enter or a double-click opens the match in a tab.

use crate::app::filetree_code::files_under;
use crate::ide::editor::restyle;
use crate::ide::find::{Hit, Needle, match_text, popup_frame, search_in, toggle};
use crate::ide::textindex::{self, TextIndex};
use crate::ide::theme as t;
use crate::ide::{highlight, icons};
use crate::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};

/// The most matches listed ("500+ ocorrências"), and the most files of a root that are read.
pub(crate) const HITS_MAX: usize = 500;
// ponytail: the root's text stays in memory while the tree lives (the index's own budget caps it), and only the first
// files of a very big root are read; search the disk in pieces if a tree over a whole disk matters
const FILES_MAX: usize = 50_000;
/// A row of the list, and of the preview.
const ROW: f32 = 24.0;
const LINE: f32 = 19.0;

/// What is looked for, and where.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Ask {
    pub(crate) query: String,
    pub(crate) case: bool,
    pub(crate) words: bool,
    pub(crate) regex: bool,
    /// The file mask ("*.rs, *.toml"); empty: every file.
    pub(crate) mask: String,
    /// The folder searched; None: the whole root.
    pub(crate) dir: Option<PathBuf>,
}

/// The file names a mask lets through: its patterns (apart by commas, semicolons or spaces) with `*` and `?`, in any case.
pub(crate) fn mask_regex(mask: &str) -> Option<regex::Regex> {
    let patterns: Vec<String> =
        mask.split([',', ';', ' ']).filter(|p| !p.is_empty()).map(|p| regex::escape(p).replace(r"\*", ".*").replace(r"\?", ".")).collect();
    if patterns.is_empty() {
        return None;
    }
    regex::Regex::new(&format!("(?i)^(?:{})$", patterns.join("|"))).ok()
}

impl Ask {
    /// Whether a file is in the scope and passes the mask.
    fn keep(&self) -> impl Fn(&Path) -> bool + use<> {
        let (mask, dir) = (mask_regex(&self.mask), self.dir.clone());
        move |p: &Path| {
            dir.as_ref().is_none_or(|d| p.starts_with(d)) && mask.as_ref().is_none_or(|m| p.file_name().is_some_and(|n| m.is_match(&n.to_string_lossy())))
        }
    }

    /// The matches in the text index `ix`, up to `HITS_MAX`; Err says what is wrong with the regular expression.
    pub(crate) fn run(&self, ix: &TextIndex, cancel: &AtomicUsize, epoch: usize) -> Result<Vec<Hit>, String> {
        if self.query.is_empty() {
            return Ok(vec![]);
        }
        let needle = Needle::new(&self.query, self.case, self.regex).map_err(|why| format!("Expressão regular inválida: {why}"))?;
        Ok(search_in(ix, &needle, self.words, self.keep(), cancel, epoch, HITS_MAX))
    }

    /// The matches under `root`, read from the disk now (an agent's search), without the excluded folders.
    pub(crate) fn run_now(&self, root: &Path) -> Result<Vec<Hit>, String> {
        let keep = self.keep();
        let files = files_under(root, FILES_MAX, |p| keep(p));
        self.run(&TextIndex::build(&files, &TextIndex::default()), &AtomicUsize::new(0), 0)
    }
}

/// "12 ocorrências em 3 arquivos", or "500+ ocorrências em 28+ arquivos" when the list was cut.
pub(crate) fn count_note(hits: &[Hit]) -> String {
    // the matches come file by file
    let mut files: Vec<&PathBuf> = hits.iter().map(|h| &h.path).collect();
    files.dedup();
    let (n, f, more) = (hits.len(), files.len(), if hits.len() >= HITS_MAX { "+" } else { "" });
    format!("{n}{more} {} em {f}{more} {}", if n == 1 { "ocorrência" } else { "ocorrências" }, if f == 1 { "arquivo" } else { "arquivos" })
}

/// The text of a tree's root in memory: read on a thread when the window first opens, and what changed each time it does.
#[derive(Default)]
pub(crate) struct Texts {
    index: Arc<TextIndex>,
    generation: u64,
    rx: Option<Receiver<TextIndex>>,
}

impl Texts {
    pub(crate) fn refresh(&mut self, root: &Path, ctx: &Context) {
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let (root, old, ctx) = (root.to_path_buf(), self.index.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(TextIndex::build(&files_under(&root, FILES_MAX, |_| true), &old));
            ctx.request_repaint();
        });
        self.rx = Some(rx);
    }

    fn poll(&mut self) {
        if let Some(index) = self.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            (self.index, self.rx) = (Arc::new(index), None);
            self.generation += 1;
        }
    }
}

/// The lines of a file around the selected match, colored, with the match lit.
struct Preview {
    of: (PathBuf, usize, usize),
    first: usize,
    lines: usize,
    galley: Arc<egui::Galley>,
}

fn preview(ui: &egui::Ui, hit: &Hit, rows: usize) -> Option<Preview> {
    let src = textindex::read_text(&hit.path)?;
    let first = hit.line.saturating_sub(rows / 2);
    let lines: Vec<&str> = src.lines().skip(first).take(rows).collect();
    let line = lines.get(hit.line - first)?;
    let mut job = highlight::layout(highlight::detect(&hit.path), &lines.join("\n"), t::mono(t::EDITOR_SIZE), LINE);
    let at = line.char_indices().nth(hit.col).map_or(line.len(), |(b, _)| b);
    let len: usize = line[at..].chars().take(hit.len).map(char::len_utf8).sum();
    let start = lines[..hit.line - first].iter().map(|l| l.len() + 1).sum::<usize>() + at;
    restyle(&mut job, start..start + len, |f| (f.background, f.color) = (t::SEARCH_MATCH, Color32::WHITE));
    Some(Preview { of: (hit.path.clone(), hit.line, hit.col), first, lines: lines.len(), galley: ui.fonts_mut(|f| f.layout_job(job)) })
}

/// The window's state: what is typed and ticked, the matches and the one selected.
pub(crate) struct FindFiles {
    pub(crate) query: String,
    pub(crate) case: bool,
    pub(crate) words: bool,
    pub(crate) regex: bool,
    pub(crate) mask_on: bool,
    pub(crate) mask: String,
    /// The "Diretório" scope is on, and its folder (the one selected in the tree when the window opened).
    pub(crate) in_dir: bool,
    pub(crate) folder: PathBuf,
    pub(crate) hits: Vec<Hit>,
    sel: usize,
    /// What is wrong with the regular expression.
    error: String,
    /// What the matches are of: the search restarts when it or the root's text moves.
    last: Option<(Ask, u64)>,
    epoch: Arc<AtomicUsize>,
    rx: Option<Receiver<Result<Vec<Hit>, String>>>,
    /// The field takes the keyboard on the next frame.
    focus: bool,
    preview: Option<Preview>,
}

/// What the window asked of the pane.
pub(crate) enum FindAct {
    None,
    Close,
    /// A match was picked: its file, line and column.
    Open(PathBuf, usize, usize),
}

impl FindFiles {
    pub(crate) fn new(query: String, folder: PathBuf) -> Self {
        FindFiles {
            query,
            case: false,
            words: false,
            regex: false,
            mask_on: false,
            mask: String::new(),
            in_dir: false,
            folder,
            hits: vec![],
            sel: 0,
            error: String::new(),
            last: None,
            epoch: Default::default(),
            rx: None,
            focus: true,
            preview: None,
        }
    }

    pub(crate) fn ask(&self) -> Ask {
        Ask {
            query: self.query.clone(),
            case: self.case,
            words: self.words,
            regex: self.regex,
            mask: if self.mask_on { self.mask.trim().to_string() } else { String::new() },
            dir: self.in_dir.then(|| self.folder.clone()),
        }
    }

    /// Whether the matches shown are not yet those of what is typed.
    pub(crate) fn searching(&self) -> bool {
        self.rx.is_some()
    }

    /// Searches again, on a thread, when what is asked or the root's text changed; takes in what was found.
    pub(crate) fn tick(&mut self, texts: &mut Texts, ctx: &Context) {
        texts.poll();
        let key = (self.ask(), texts.generation);
        if self.last.as_ref() != Some(&key) {
            let epoch = self.epoch.fetch_add(1, Ordering::Relaxed) + 1;
            let (tx, rx) = channel();
            let (ask, index, cancel, ctx) = (key.0.clone(), texts.index.clone(), self.epoch.clone(), ctx.clone());
            std::thread::spawn(move || {
                let found = ask.run(&index, &cancel, epoch);
                if cancel.load(Ordering::Relaxed) == epoch {
                    let _ = tx.send(found);
                    ctx.request_repaint();
                }
            });
            (self.rx, self.last) = (Some(rx), Some(key));
        }
        if let Some(found) = self.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            (self.rx, self.sel, self.preview) = (None, 0, None);
            match found {
                Ok(hits) => (self.hits, self.error) = (hits, String::new()),
                Err(why) => (self.hits, self.error) = (vec![], why),
            }
        }
    }
}

/// A scope's tab: lit when it is the one searched.
fn scope(ui: &mut egui::Ui, label: &str, on: bool) -> bool {
    let width = ui.painter().layout_no_wrap(label.to_string(), t::ui(t::UI_SIZE), t::TEXT).size().x + 18.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click());
    if on {
        ui.painter().rect(rect, 5, t::TAB_SELECTED, Stroke::new(1.0, t::TAB_BORDER), StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 5, t::HOVER);
    }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, t::ui(t::UI_SIZE), if on { t::TEXT } else { t::MUTED });
    resp.clicked()
}

/// The window over `screen`, for the tree whose root is `root`, with the keys while it is open (↑↓, Enter, Esc). A
/// press outside it closes it.
pub(crate) fn find_ui(ctx: &Context, key: &str, root: &Path, screen: Rect, find: &mut FindFiles, texts: &mut Texts) -> FindAct {
    find.tick(texts, ctx);
    let key_of = |i: &mut egui::InputState, key: Key| i.consume_key(Modifiers::NONE, key);
    let (down, up, enter, esc) = ctx.input_mut(|i| (key_of(i, Key::ArrowDown), key_of(i, Key::ArrowUp), key_of(i, Key::Enter), key_of(i, Key::Escape)));
    if down {
        find.sel = (find.sel + 1).min(find.hits.len().saturating_sub(1));
    }
    if up {
        find.sel = find.sel.saturating_sub(1);
    }
    let mut open = enter.then_some(find.sel);
    let (w, h) = (900.0f32.min(screen.width() - 40.0), (screen.height() * 0.78).min(760.0));
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/");
    let reading = texts.rx.is_some() && texts.generation == 0;
    let shown = egui::Area::new(egui::Id::unique(("ft_find", key)))
        .order(egui::Order::Foreground)
        .default_pos(Pos2::new(screen.center().x - w / 2.0, screen.top() + (screen.height() - h) / 2.0))
        .movable(true)
        .show(ctx, |ui| {
            t::style(ui.style_mut());
            popup_frame(ui).show(ui, |ui| {
                ui.set_width(w);
                ui.set_height(h);
                // the title, the count and the file mask
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.set_height(38.0);
                    ui.label(egui::RichText::new("Localizar nos Arquivos").strong().color(t::TEXT));
                    let count = if reading {
                        "lendo os arquivos…".to_string()
                    } else if find.searching() {
                        "pesquisando…".to_string()
                    } else if find.query.is_empty() || !find.error.is_empty() {
                        String::new()
                    } else {
                        count_note(&find.hits)
                    };
                    ui.label(egui::RichText::new(count).color(t::SECONDARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        ui.add_enabled(find.mask_on, egui::TextEdit::singleline(&mut find.mask).hint_text("*.rs").desired_width(110.0));
                        ui.checkbox(&mut find.mask_on, "Máscara de arquivo:");
                    });
                });
                // the text to find and how
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.set_height(34.0);
                    let (icon, _) = ui.allocate_exact_size(Vec2::new(18.0, 34.0), Sense::hover());
                    icons::at(ui.painter(), icons::SEARCH, icon.center());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        toggle(ui, ".*", "Expressão regular", &mut find.regex);
                        toggle(ui, "W", "Palavras", &mut find.words);
                        toggle(ui, "Cc", "Diferenciar maiúsculas de minúsculas", &mut find.case);
                        let field = egui::TextEdit::singleline(&mut find.query).hint_text("Texto a localizar").frame(egui::Frame::NONE).font(t::mono(14.0));
                        let r = ui.add(field.desired_width(f32::INFINITY));
                        if std::mem::take(&mut find.focus) || ui.memory(|m| m.focused().is_none()) {
                            r.request_focus();
                        }
                    });
                });
                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                // where: the whole root, or the folder that was selected in the tree
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.set_height(32.0);
                    if scope(ui, "No Projeto", !find.in_dir) {
                        find.in_dir = false;
                    }
                    if scope(ui, "Diretório", find.in_dir) {
                        find.in_dir = true;
                    }
                    if find.in_dir {
                        let folder = rel(&find.folder);
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(if folder.is_empty() { root.display().to_string() } else { folder }).color(t::SECONDARY));
                    }
                });
                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                // the matches: the line with the text lit, its file and line at the right
                let list_h = ((h - 38.0 - 34.0 - 32.0 - 34.0) * 0.5).floor();
                let mut pressed = None;
                egui::ScrollArea::vertical().id_salt("ft_find_list").max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, false]).show(
                    ui,
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let note = if !find.error.is_empty() {
                            find.error.clone()
                        } else if find.hits.is_empty() && !find.query.is_empty() && !find.searching() && !reading {
                            "Nada encontrado".to_string()
                        } else {
                            String::new()
                        };
                        if !note.is_empty() {
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
                            let color = if find.error.is_empty() { t::SECONDARY } else { t::ERROR };
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, note, t::ui(t::UI_SIZE), color);
                        }
                        for (i, hit) in find.hits.iter().enumerate() {
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW), Sense::click());
                            let fill = if i == find.sel {
                                t::SELECTION
                            } else if resp.hovered() {
                                t::ROW_HOVER
                            } else {
                                Color32::TRANSPARENT
                            };
                            ui.painter().rect_filled(rect.shrink2(Vec2::new(6.0, 0.0)), 5, fill);
                            let label = format!("{} {}", hit.path.file_name().unwrap_or_default().to_string_lossy(), hit.line + 1);
                            let at = ui.painter().text(
                                rect.right_center() - Vec2::new(14.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                label,
                                t::ui(t::UI_SIZE),
                                t::SECONDARY,
                            );
                            match_text(ui, Pos2::new(rect.left() + 14.0, rect.center().y), hit, at.left() - 12.0 - rect.left() - 14.0);
                            if i == find.sel && (up || down) {
                                resp.scroll_to_me(None);
                            }
                            if resp.clicked() {
                                pressed = Some(i);
                            }
                            if resp.double_clicked() {
                                open = Some(i);
                            }
                        }
                    },
                );
                if let Some(i) = pressed {
                    find.sel = i;
                }
                // the file around the selected match
                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                let (pane, _) = ui.allocate_exact_size(Vec2::new(w, list_h), Sense::hover());
                let pane = pane.with_min_y(pane.top() + 1.0);
                ui.painter().rect_filled(pane, 0, t::ISLAND);
                if let Some(hit) = find.hits.get(find.sel) {
                    let p = ui.painter().with_clip_rect(pane);
                    p.text(Pos2::new(pane.left() + 14.0, pane.top() + 13.0), egui::Align2::LEFT_CENTER, rel(&hit.path), t::ui(t::UI_SIZE - 1.0), t::SECONDARY);
                    if find.preview.as_ref().is_none_or(|v| v.of != (hit.path.clone(), hit.line, hit.col)) {
                        find.preview = preview(ui, hit, ((list_h - 30.0) / LINE) as usize);
                    }
                    if let Some(view) = &find.preview {
                        let top = pane.top() + 26.0;
                        p.rect_filled(
                            Rect::from_min_size(Pos2::new(pane.left(), top + (hit.line - view.first) as f32 * LINE), Vec2::new(w, LINE)),
                            0,
                            t::CARET_ROW,
                        );
                        for k in 0..view.lines {
                            let at = Pos2::new(pane.left() + 46.0, top + k as f32 * LINE + LINE / 2.0);
                            p.text(at, egui::Align2::RIGHT_CENTER, (view.first + k + 1).to_string(), t::mono(t::EDITOR_SIZE), t::LINE_NUMBER);
                        }
                        p.galley(Pos2::new(pane.left() + 60.0, top), view.galley.clone(), t::EDITOR_TEXT);
                    }
                }
                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.set_height(32.0);
                    ui.label(egui::RichText::new("↑↓ seleciona · Enter ou clique duplo abre · Esc fecha").color(t::SECONDARY).size(t::UI_SIZE - 1.0));
                });
            });
        });
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !shown.response.rect.contains(p)));
    match open.and_then(|i| find.hits.get(i)) {
        Some(hit) => FindAct::Open(hit.path.clone(), hit.line, hit.col),
        None if esc || outside => FindAct::Close,
        None => FindAct::None,
    }
}
