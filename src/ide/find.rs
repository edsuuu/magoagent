//! Find in Files (Ctrl+Shift+F) with preview, the Find tool window, and the text search behind
//! them and Search Everywhere's Text tab.

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub len: usize,
    pub text: String,
}

/// What a line is searched for: the text as typed (with the case told apart or not), or a regular expression.
pub(crate) enum Needle {
    Text(String, bool),
    Regex(regex::Regex),
}

impl Needle {
    /// `q` as text or, with `regex`, as an expression; Err says what is wrong with the expression.
    pub(crate) fn new(q: &str, case: bool, regex: bool) -> Result<Needle, String> {
        if !regex {
            return Ok(Needle::Text(q.to_string(), case));
        }
        regex::RegexBuilder::new(q).case_insensitive(!case).build().map(Needle::Regex).map_err(|e| e.to_string().lines().last().unwrap_or_default().to_string())
    }

    /// The next match in `line` from byte `from`: its start and end. An empty match (an empty text, an expression
    /// that matches nothing) ends the line.
    fn next(&self, line: &str, from: usize) -> Option<(usize, usize)> {
        let found = match self {
            Needle::Text(q, true) => line[from..].find(q.as_str()).map(|i| (from + i, from + i + q.len())),
            Needle::Text(q, false) => {
                let n = q.as_bytes();
                line.as_bytes()[from..].windows(n.len().max(1)).position(|w| w.eq_ignore_ascii_case(n)).map(|i| (from + i, from + i + n.len()))
            }
            Needle::Regex(re) => re.find_at(line, from).map(|m| (m.start(), m.end())),
        };
        found.filter(|(at, end)| end > at)
    }
}

/// The matches of `needle` in one file's `text`, appended to `out` up to `limit`.
fn scan(path: &Path, text: &str, needle: &Needle, words: bool, out: &mut Vec<Hit>, limit: usize) {
    let is_id = |c: char| c.is_alphanumeric() || c == '_';
    for (ln, line) in text.lines().enumerate() {
        let mut from = 0;
        while let Some((at, end)) = needle.next(line, from) {
            from = end;
            if words && (line[..at].ends_with(is_id) || line[end..].starts_with(is_id)) {
                continue;
            }
            let col = line[..at].chars().count();
            out.push(Hit { path: path.to_path_buf(), line: ln, col, len: line[at..end].chars().count(), text: line.chars().take(400).collect() });
            if out.len() >= limit {
                return;
            }
        }
    }
}

/// Case-(in)sensitive, optionally whole-word text search over files read from the disk. Stops
/// early when `epoch` moves on.
pub fn search(files: &[PathBuf], q: &str, case: bool, words: bool, cancel: &AtomicUsize, epoch: usize, limit: usize) -> Vec<Hit> {
    let (mut out, needle) = (vec![], Needle::Text(q.to_string(), case));
    for p in files {
        if q.is_empty() || cancel.load(Ordering::Relaxed) != epoch || out.len() >= limit {
            break;
        }
        if std::fs::metadata(p).map_or(true, |m| m.len() > textindex::MAX_FILE) {
            continue;
        }
        if let Some(text) = textindex::read_text(p) {
            scan(p, &text, &needle, words, &mut out, limit);
        }
    }
    out
}

/// The same search over the text index, with no disk reads for what it holds.
pub(crate) fn search_index(ix: &textindex::TextIndex, q: &str, case: bool, words: bool, cancel: &AtomicUsize, epoch: usize, limit: usize) -> Vec<Hit> {
    if q.is_empty() {
        return vec![];
    }
    search_in(ix, &Needle::Text(q.to_string(), case), words, |_| true, cancel, epoch, limit)
}

/// The matches of any `needle` in the files of the text index that `keep` lets through (a folder, a file mask).
pub(crate) fn search_in(
    ix: &textindex::TextIndex,
    needle: &Needle,
    words: bool,
    keep: impl Fn(&Path) -> bool,
    cancel: &AtomicUsize,
    epoch: usize,
    limit: usize,
) -> Vec<Hit> {
    let mut out = vec![];
    for e in ix.entries.iter().filter(|e| keep(&e.path)) {
        if cancel.load(Ordering::Relaxed) != epoch || out.len() >= limit {
            break;
        }
        match &e.text {
            Some(text) => scan(&e.path, text, needle, words, &mut out, limit),
            None => {
                if let Some(text) = textindex::read_text(&e.path) {
                    scan(&e.path, &text, needle, words, &mut out, limit);
                }
            }
        }
    }
    out
}

/// An agent's ide_search: case-insensitive, over the files Go to File lists, as
/// "path/relative/to/root:line: text" (1-based lines), at most `limit`.
pub(crate) fn project_search(root: &Path, q: &str, limit: usize) -> Vec<String> {
    let files = tree::list_files(root);
    let hits = search(&files, q, false, false, &AtomicUsize::new(0), 0, limit);
    hits.iter().map(|h| format!("{}:{}: {}", h.path.strip_prefix(root).unwrap_or(&h.path).display(), h.line + 1, h.text.trim())).collect()
}

/// Text search state shared by Find in Files and the Text tab of Search Everywhere.
#[derive(Default)]
pub struct Find {
    pub query: String,
    pub case: bool,
    pub words: bool,
    pub hits: Vec<Hit>,
    pub sel: usize,
    last: Option<(String, bool, bool, u64)>,
    epoch: Arc<AtomicUsize>,
    rx: Option<Receiver<Vec<Hit>>>,
    pub searching: bool,
}

impl Find {
    /// Restart the background search when the query/options changed; collect results.
    /// `version` moves when the index does, so a search made while it was still filling runs again.
    pub(super) fn tick(&mut self, ctx: &egui::Context, ix: &Arc<textindex::TextIndex>, version: u64) {
        let key = (self.query.clone(), self.case, self.words, version);
        if self.last.as_ref() != Some(&key) {
            self.last = Some(key);
            let epoch = self.epoch.fetch_add(1, Ordering::Relaxed) + 1;
            let (tx, rx) = std::sync::mpsc::channel();
            let (ix, q, case, words, cancel, ctx) = (ix.clone(), self.query.clone(), self.case, self.words, self.epoch.clone(), ctx.clone());
            std::thread::spawn(move || {
                let hits = search_index(&ix, &q, case, words, &cancel, epoch, 2000);
                if cancel.load(Ordering::Relaxed) == epoch {
                    let _ = tx.send(hits);
                    ctx.request_repaint();
                }
            });
            self.rx = Some(rx);
            self.searching = !self.query.is_empty();
        }
        if let Some(rx) = &self.rx
            && let Ok(h) = rx.try_recv()
        {
            self.hits = h;
            self.sel = 0;
            self.searching = false;
            self.rx = None;
        }
    }
}

pub(super) fn rel(root: &Option<PathBuf>, p: &Path) -> String {
    root.as_ref().and_then(|r| p.strip_prefix(r).ok()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// Line of text with the matched range on a highlight background.
pub(crate) fn match_text(ui: &Ui, pos: egui::Pos2, h: &Hit, max_w: f32) -> f32 {
    let font = t::mono(t::EDITOR_SIZE - 1.0);
    let trimmed = h.text.trim_start();
    let skip = h.text.chars().count() - trimmed.chars().count();
    let before: String = trimmed.chars().take(h.col.saturating_sub(skip)).collect();
    let hit: String = trimmed.chars().skip(h.col.saturating_sub(skip)).take(h.len).collect();
    let after: String = trimmed.chars().skip(h.col.saturating_sub(skip) + h.len).collect();
    let p = ui.painter().with_clip_rect(Rect::from_min_size(pos2(pos.x, pos.y - 12.0), vec2(max_w, 24.0)));
    let g1 = p.layout_no_wrap(before, font.clone(), t::EDITOR_TEXT);
    let g2 = p.layout_no_wrap(hit, font.clone(), Color32::WHITE);
    let g3 = p.layout_no_wrap(after, font, t::EDITOR_TEXT);
    let y = pos.y - g1.size().y / 2.0;
    let x2 = pos.x + g1.size().x;
    p.rect_filled(Rect::from_min_size(pos2(x2 - 1.0, y), vec2(g2.size().x + 2.0, g2.size().y)), 2, t::SEARCH_MATCH);
    let x3 = x2 + g2.size().x;
    let end = x3 + g3.size().x;
    p.galley(pos2(pos.x, y), g1, t::EDITOR_TEXT);
    p.galley(pos2(x2, y), g2, Color32::WHITE);
    p.galley(pos2(x3, y), g3, t::EDITOR_TEXT);
    end
}

pub(crate) fn popup_frame(ui: &Ui) -> Frame {
    Frame::popup(ui.style()).fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(12).inner_margin(0)
}

/// [Cc] [W] option toggles, as in PhpStorm's find popups.
pub(crate) fn toggle(ui: &mut Ui, label: &str, tip: &str, on: &mut bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    if *on {
        ui.painter().rect(r, 5, t::TAB_SELECTED, Stroke::new(1.0, t::TAB_BORDER), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 5, t::HOVER);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, t::ui(12.0), if *on { t::TEXT } else { t::MUTED });
    if resp.on_hover_text(tip).clicked() {
        *on = !*on;
    }
}

impl Ide {
    pub(crate) fn find_in_files(&mut self, ctx: &egui::Context) {
        let indexing = self.indexing();
        let Some(find) = &mut self.find_files else { return };
        find.tick(ctx, &self.text_index, self.text_gen);
        let (down, up, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if down {
            find.sel = (find.sel + 1).min(find.hits.len().saturating_sub(1));
        }
        if up {
            find.sel = find.sel.saturating_sub(1);
        }
        let mut chosen = if enter { find.hits.get(find.sel).cloned() } else { None };
        let mut close = esc || chosen.is_some();
        let mut to_window = false;
        let screen = self.rect;
        let w = 900.0f32.min(screen.width() - 40.0);
        let h = (screen.height() * 0.78).min(760.0);
        let root = self.root.clone();
        let lang_font = t::mono(self.font_size);
        egui::Area::new(self.id.with("find_in_files"))
            .order(Order::Foreground)
            .fixed_pos(pos2(screen.center().x - w / 2.0, screen.top() + (screen.height() - h) / 2.0))
            .show(ctx, |ui| {
                theme::style(ui.style_mut());
                popup_frame(ui).show(ui, |ui| {
                    ui.set_width(w);
                    ui.set_height(h);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.set_height(38.0);
                        ui.label(egui::RichText::new("Localizar nos arquivos").strong().color(t::TEXT));
                        let n = find.hits.len();
                        let count = if indexing {
                            "indexando…".to_string()
                        } else if find.searching {
                            "pesquisando…".to_string()
                        } else if n >= 2000 {
                            "2000+ resultados".into()
                        } else if find.query.is_empty() {
                            String::new()
                        } else {
                            format!("{n} resultados")
                        };
                        ui.label(egui::RichText::new(count).color(t::SECONDARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(10.0);
                            toggle(ui, "W", "Palavras inteiras", &mut find.words);
                            toggle(ui, "Cc", "Diferenciar maiúsculas", &mut find.case);
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let (ir, _) = ui.allocate_exact_size(vec2(18.0, 34.0), Sense::hover());
                        icons::at(ui.painter(), icons::SEARCH, ir.center());
                        let r = ui.add(
                            TextEdit::singleline(&mut find.query)
                                .hint_text("Texto a localizar")
                                .frame(Frame::NONE)
                                .desired_width(f32::INFINITY)
                                .font(t::mono(14.0)),
                        );
                        r.request_focus();
                    });
                    ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                    let list_h = (h - 38.0 - 34.0 - 40.0) * 0.5;
                    egui::ScrollArea::vertical().id_salt("fif_list").max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, false]).show(
                        ui,
                        |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for (i, hit) in find.hits.iter().enumerate() {
                                let (resp, rect) = row(ui, i == find.sel, 24.0);
                                let label = format!("{} {}", file_name(&hit.path), hit.line + 1);
                                let lg = ui.painter().layout_no_wrap(label.clone(), t::ui(t::UI_SIZE), t::SECONDARY);
                                match_text(ui, pos2(rect.left() + 14.0, rect.center().y), hit, rect.width() - lg.size().x - 40.0);
                                ui.painter().text(pos2(rect.right() - 14.0, rect.center().y), Align2::RIGHT_CENTER, label, t::ui(t::UI_SIZE), t::SECONDARY);
                                if i == find.sel && (up || down) {
                                    resp.scroll_to_me(None);
                                }
                                if resp.clicked() {
                                    find.sel = i;
                                }
                                if resp.double_clicked() {
                                    chosen = Some(hit.clone());
                                    close = true;
                                }
                            }
                        },
                    );
                    // preview of the selected match, PhpStorm-style
                    ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                    let prev_rect = Rect::from_min_size(pos2(ui.min_rect().left(), ui.min_rect().bottom() + 1.0), vec2(w, list_h));
                    ui.painter().rect_filled(prev_rect, 0, t::ISLAND);
                    if let Some(hit) = find.hits.get(find.sel) {
                        ui.painter().text(
                            pos2(prev_rect.left() + 14.0, prev_rect.top() + 12.0),
                            Align2::LEFT_CENTER,
                            rel(&root, &hit.path),
                            t::ui(t::UI_SIZE - 1.0),
                            t::SECONDARY,
                        );
                        if let Ok(src) = std::fs::read_to_string(&hit.path) {
                            let lang = highlight::detect(&hit.path);
                            let rows = ((list_h - 30.0) / 19.0) as usize;
                            let first = hit.line.saturating_sub(rows / 2);
                            let snippet: Vec<&str> = src.lines().skip(first).take(rows).collect();
                            let row_h = 19.0;
                            let job = highlight::layout(lang, &snippet.join("\n"), lang_font.clone(), row_h);
                            let g = ui.fonts_mut(|f| f.layout_job(job));
                            let top = prev_rect.top() + 26.0;
                            let y_hit = top + (hit.line - first) as f32 * row_h;
                            let p = ui.painter().with_clip_rect(prev_rect);
                            p.rect_filled(Rect::from_min_size(pos2(prev_rect.left(), y_hit), vec2(w, row_h)), 0, t::CARET_ROW);
                            for k in 0..snippet.len() {
                                p.text(
                                    pos2(prev_rect.left() + 46.0, top + k as f32 * row_h + row_h / 2.0),
                                    Align2::RIGHT_CENTER,
                                    (first + k + 1).to_string(),
                                    lang_font.clone(),
                                    t::LINE_NUMBER,
                                );
                            }
                            p.galley(pos2(prev_rect.left() + 60.0, top), g, t::EDITOR_TEXT);
                        }
                    }
                    ui.allocate_space(vec2(w, list_h));
                    ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, t::POPUP_BORDER));
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.set_height(40.0);
                        ui.label(egui::RichText::new("↑↓ seleciona · Enter abre · Esc fecha").color(t::SECONDARY).size(t::UI_SIZE - 1.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(12.0);
                            if ui
                                .add_enabled(!find.hits.is_empty(), egui::Button::new("Abrir na janela Localizar").stroke(Stroke::new(1.0, t::CONTROL_BORDER)))
                                .clicked()
                            {
                                to_window = true;
                                close = true;
                            }
                        });
                    });
                });
            });
        if to_window {
            self.find_results = self.find_files.as_ref().map(|f| (format!("Localizar: “{}”", f.query), f.hits.clone()));
            self.bottom = Some(ToolWindow::Find);
        }
        if close {
            self.find_files = None;
        }
        if let Some(h) = chosen {
            self.jump_to(h.path, h.line, h.col);
        }
    }

    /// Bottom "Find" tool window with results (a text search, or a symbol's usages) grouped by file.
    pub(crate) fn find_window(&mut self, ui: &mut Ui) {
        let Some((title, hits)) = &self.find_results else { return };
        tool_header(ui, &format!("{title}   {} resultados", hits.len()));
        let mut go = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut last: Option<&PathBuf> = None;
            for h in hits {
                if last != Some(&h.path) {
                    last = Some(&h.path);
                    let (_, rect) = row(ui, false, 24.0);
                    icons::file(ui.painter(), &h.path, pos2(rect.left() + 22.0, rect.center().y), 1.0);
                    let g = ui.painter().text(pos2(rect.left() + 36.0, rect.center().y), Align2::LEFT_CENTER, file_name(&h.path), t::ui(t::UI_SIZE), t::TEXT);
                    ui.painter().text(pos2(g.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, rel(&self.root, &h.path), t::ui(t::UI_SIZE), t::SECONDARY);
                }
                let (resp, rect) = row(ui, false, 22.0);
                ui.painter().text(
                    pos2(rect.left() + 44.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    format!("{}", h.line + 1),
                    t::mono(t::EDITOR_SIZE - 1.0),
                    t::LINE_NUMBER,
                );
                match_text(ui, pos2(rect.left() + 84.0, rect.center().y), h, rect.width() - 100.0);
                if resp.clicked() {
                    go = Some(h.clone());
                }
            }
        });
        if let Some(h) = go {
            self.jump_to(h.path, h.line, h.col);
        }
    }

    pub(crate) fn open_find(&mut self, query: Option<String>) {
        let mut f = Find::default();
        f.query = query.unwrap_or_else(|| self.selected_text());
        self.find_files = Some(f);
    }

    /// Selected text of the active editor (single line), for prefilling search fields.
    pub(crate) fn selected_text(&self) -> String {
        let Some(tab) = self.tabs.get(self.active) else { return String::new() };
        let (a, b) = tab.sel;
        let s: String = tab.text.chars().skip(a).take(b - a).collect();
        if s.contains('\n') { String::new() } else { s }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_search_options() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-find-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.php");
        std::fs::write(&f, "$user = User::find(1);\n// users ação User\n").unwrap();
        let files = vec![f.clone()];
        let c = AtomicUsize::new(0);
        assert_eq!(search(&files, "user", false, false, &c, 0, 100).len(), 4);
        assert_eq!(search(&files, "User", true, false, &c, 0, 100).len(), 2);
        let w = search(&files, "user", false, true, &c, 0, 100);
        assert_eq!(w.len(), 3);
        // char columns survive multi-byte text before the match
        let last = w.last().unwrap();
        assert_eq!((last.line, last.col, last.len), (1, 14, 4));
        // cancelled searches stop
        assert!(search(&files, "user", false, false, &c, 7, 100).is_empty());
        // a regular expression, any case or not, whole words too; a broken one says why, an empty match finds nothing
        let ix = textindex::TextIndex::build(&files, &Default::default());
        let by = |q: &str, case: bool, words: bool| search_in(&ix, &Needle::new(q, case, true).unwrap(), words, |_| true, &c, 0, 100);
        let found: Vec<_> = by(r"us\w+", false, false).iter().map(|h| (h.line, h.col, h.len)).collect();
        assert_eq!(found, [(0, 1, 4), (0, 8, 4), (1, 3, 5), (1, 14, 4)]);
        assert_eq!(by(r"U\w+", true, true).len(), 2);
        assert_eq!(by("x*", false, false).len(), 0);
        assert!(Needle::new("(", false, true).is_err());
        // only the files let through
        assert!(search_in(&ix, &Needle::new("user", false, false).unwrap(), false, |p| p.extension().is_some_and(|e| e == "rs"), &c, 0, 100).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
