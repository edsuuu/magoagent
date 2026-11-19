//! "Localizar nos Arquivos" of a file tree node (Ctrl+Shift+F), as the IDE's "Find in Files": a popup over everything, dragged
//! by its body and sized by its corner, with the text to find (the case told apart or not, whole words, a regular expression,
//! across lines), an optional file mask, the scope (the whole root or one folder), how many matches in how many files, the
//! matches one to a line and, under them, the whole file with the matches lit (`preview.rs`, the tree's own code editor).
//! The search is the IDE's (`editor::find`) over the root's text kept in memory (`editor::textindex`), read on a thread
//! without the excluded folders. Enter or a double-click opens the match in the pane; Ctrl+Enter keeps the list in a tab.

mod bar;
mod list;
mod preview;

use crate::app::filetree::code::{Project, files_under};
use crate::app::ui::{UiColors, dialog_check, segmented};
use crate::editor::find::{Hit, Needle, search_in};
use crate::editor::textindex::TextIndex;
use crate::prelude::*;
use bar::*;
pub(crate) use list::{Results, results_ui};
use preview::Preview;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};

/// The most matches listed ("500+ ocorrências"), and the most files of a root that are read.
pub(crate) const HITS_MAX: usize = 500;
// ponytail: the root's text stays in memory while the tree lives (the index's own budget caps it), and only the first
// files of a very big root are read; search the disk in pieces if a tree over a whole disk matters
const FILES_MAX: usize = 50_000;
/// The popup's smallest size, and the heights of its bands.
const SIZE_MIN: Vec2 = Vec2::new(620.0, 460.0);
const HEADER: f32 = 46.0;
const SEARCH: f32 = 46.0;
const SCOPES: f32 = 40.0;
const FOOTER: f32 = 50.0;

/// What is looked for, and where.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Ask {
    pub(crate) query: String,
    pub(crate) case: bool,
    pub(crate) words: bool,
    pub(crate) regex: bool,
    /// A match may cross lines ("várias linhas").
    pub(crate) multiline: bool,
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

    /// What the query looks for; Err says what is wrong with the regular expression.
    pub(crate) fn needle(&self) -> Result<Needle, String> {
        let made = if self.multiline { Needle::whole(&self.query, self.case, self.regex) } else { Needle::new(&self.query, self.case, self.regex) };

        made.map_err(|why| format!("Expressão regular inválida: {why}"))
    }

    /// The matches in the text index `ix`, up to `HITS_MAX`; Err says what is wrong with the regular expression.
    pub(crate) fn run(&self, ix: &TextIndex, cancel: &AtomicUsize, epoch: usize) -> Result<Vec<Hit>, String> {
        if self.query.is_empty() {
            return Ok(vec![]);
        }

        let needle = self.needle()?;

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

/// The popup's state: what is typed and ticked, the matches and the one selected.
pub(crate) struct FindFiles {
    pub(crate) query: String,
    pub(crate) case: bool,
    pub(crate) words: bool,
    pub(crate) regex: bool,
    pub(crate) multiline: bool,
    pub(crate) mask_on: bool,
    pub(crate) mask: String,
    /// The "Diretório" scope is on, and its folder (the one selected in the tree when the popup opened).
    pub(crate) in_dir: bool,
    pub(crate) folder: PathBuf,
    /// The folder as typed in its field, and the folder that text was last made from or read into.
    folder_text: String,
    folder_synced: PathBuf,
    pub(crate) hits: Vec<Hit>,
    pub(crate) selected: usize,
    /// The popup stays when a press lands outside it.
    pub(crate) pinned: bool,
    /// "Abrir resultados em nova aba": a match opens for good, not as the preview tab the next click replaces.
    pub(crate) new_tab: bool,
    /// What is wrong with the regular expression.
    error: String,
    /// What the matches are of: the search restarts when it or the root's text moves.
    last: Option<(Ask, u64)>,
    epoch: Arc<AtomicUsize>,
    rx: Option<Receiver<Result<Vec<Hit>, String>>>,
    /// The field takes the keyboard on the next frame.
    pub(crate) focus: bool,
    preview: Option<Preview>,
    /// The popup's size (dragged by its corner); zero until it is first drawn.
    size: Vec2,
    /// The search running is the last one's question again, on the root's text read anew.
    same_ask: bool,
}

/// What the popup asked of the pane.
pub(crate) enum FindAct {
    None,
    Close,
    /// A match was picked: its file, line and column, and whether it opens for good.
    Open(PathBuf, usize, usize, bool),
    /// "Abrir na janela de busca" (Ctrl+Enter): the question and what it found, kept in a tab.
    Results(Ask, Vec<Hit>),
}

impl FindFiles {
    pub(crate) fn new(query: String, folder: PathBuf) -> Self {
        FindFiles {
            query,
            case: false,
            words: false,
            regex: false,
            multiline: false,
            mask_on: false,
            mask: String::new(),
            in_dir: false,
            folder,
            folder_text: String::new(),
            folder_synced: PathBuf::new(),
            hits: vec![],
            selected: 0,
            pinned: false,
            new_tab: false,
            error: String::new(),
            last: None,
            epoch: Default::default(),
            rx: None,
            focus: true,
            preview: None,
            same_ask: false,
            size: Vec2::ZERO,
        }
    }

    pub(crate) fn ask(&self) -> Ask {
        Ask {
            query: self.query.clone(),
            case: self.case,
            words: self.words,
            regex: self.regex,
            multiline: self.multiline,
            mask: if self.mask_on { self.mask.trim().to_string() } else { String::new() },
            dir: self.in_dir.then(|| self.folder.clone()),
        }
    }

    /// The file shown under the list and the line and column its caret went to.
    #[cfg(test)]
    pub(crate) fn previewing(&self) -> Option<(PathBuf, (usize, usize))> {
        self.preview.as_ref().and_then(Preview::shown)
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
            self.same_ask = self.last.as_ref().is_some_and(|(ask, _)| *ask == key.0);
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
            // the same question searched again (the root's text was read again): the selection stays on its match
            let kept = self.hits.get(self.selected).map(|hit| (hit.path.clone(), hit.line, hit.col)).filter(|_| self.same_ask);

            self.rx = None;

            match found {
                Ok(hits) => (self.hits, self.error) = (hits, String::new()),
                Err(why) => (self.hits, self.error) = (vec![], why),
            }

            self.selected = kept.and_then(|kept| self.hits.iter().position(|hit| (&hit.path, hit.line, hit.col) == (&kept.0, kept.1, kept.2))).unwrap_or(0);
        }
    }

    /// The folder typed in its field, read against the root (a path from the root, or a whole one).
    fn read_folder(&mut self, root: &Path) {
        let typed = self.folder_text.trim().replace('\\', "/");
        let path = if Path::new(&typed).is_absolute() { PathBuf::from(&typed) } else { root.join(&typed) };

        if path.is_dir() {
            (self.folder, self.folder_synced) = (path.clone(), path);
        }
    }
}

/// The popup over `screen`, for the tree whose project is `code`, with the keys while it is open (↑↓, Enter, Ctrl+Enter, Esc).
/// A press outside it closes it, unless it is pinned.
pub(crate) fn find_ui(ctx: &Context, key: &str, code: &Project, screen: Rect, find: &mut FindFiles, texts: &mut Texts) -> FindAct {
    find.tick(texts, ctx);
    let root = code.root();
    // typing in the file under the list keeps the arrows and Enter for the text
    let typing = find.preview.as_ref().is_some_and(|preview| ctx.memory(|memory| memory.has_focus(preview.id)));

    let (down, up, enter, keep_list, esc) = ctx.input_mut(|input| {
        let (lists, shift) = (!typing, input.modifiers.shift);
        let enter_plain = lists && !(find.multiline && shift) && input.consume_key(Modifiers::NONE, Key::Enter);

        (
            lists && input.consume_key(Modifiers::NONE, Key::ArrowDown),
            lists && input.consume_key(Modifiers::NONE, Key::ArrowUp),
            enter_plain,
            input.consume_key(Modifiers::COMMAND, Key::Enter),
            input.consume_key(Modifiers::NONE, Key::Escape),
        )
    });

    if down {
        find.selected = (find.selected + 1).min(find.hits.len().saturating_sub(1));
    }

    if up {
        find.selected = find.selected.saturating_sub(1);
    }

    let mut open = enter.then_some(find.selected);
    let mut keep = keep_list && !find.hits.is_empty();

    if find.size == Vec2::ZERO {
        find.size = Vec2::new(900.0f32.min(screen.width() - 40.0), (screen.height() * 0.84).min(780.0));
    }

    find.size = find.size.max(SIZE_MIN.min(screen.size())).min(screen.size());
    let size = find.size;
    let rel = |path: &Path| path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/");

    if find.folder != find.folder_synced {
        (find.folder_text, find.folder_synced) = (rel(&find.folder), find.folder.clone());
    }

    let reading = texts.rx.is_some() && texts.generation == 0;
    let colors = UiColors::new(ctx.global_style().visuals.dark_mode);
    let c = &colors;
    let shadow = egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(110) };

    let frame = egui::Frame::new()
        .fill(c.island)
        .corner_radius(12)
        .inner_margin(0)
        .stroke(Stroke::new(1.0, Color32::from_black_alpha(if c.text.r() > 0x80 { 90 } else { 30 })))
        .shadow(shadow);

    let shown = egui::Window::new("Localizar nos Arquivos")
        .id(egui::Id::unique(("ft_find", key)))
        .order(egui::Order::Foreground)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .fixed_size(size)
        .default_pos(Pos2::new(screen.center().x - size.x / 2.0, screen.top() + (screen.height() - size.y) / 2.0))
        .constrain_to(screen)
        .frame(frame)
        .show(ctx, |ui| {
            let (whole, _) = ui.allocate_exact_size(size, Sense::hover());
            let band = |top: f32, height: f32| Rect::from_min_size(Pos2::new(whole.left(), top), Vec2::new(whole.width(), height));

            let row = |ui: &mut egui::Ui, rect: Rect, add: &mut dyn FnMut(&mut egui::Ui)| {
                let layout = egui::Layout::left_to_right(egui::Align::Center);

                ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(14.0, 0.0))).layout(layout), |ui| add(ui));
            };

            // a switch with its name in a box of `width` (the settings' switch, `dialog_check`)
            let switch = |ui: &mut egui::Ui, at: Pos2, width: f32, on: &mut bool, label: &str| {
                let rect = Rect::from_min_size(at, Vec2::new(width, 34.0));

                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    dialog_check(ui, on, label, c);
                });
            };

            let divider = |ui: &egui::Ui, y: f32| ui.painter().hline(whole.x_range(), y, Stroke::new(1.0, c.surface_low));
            // the title with the count, and at the right the file mask and the pin
            let header = band(whole.top(), HEADER);

            row(ui, header, &mut |ui| {
                ui.label(egui::RichText::new("Localizar nos Arquivos").size(15.0).color(c.text));
                ui.add_space(6.0);

                let count = if reading {
                    "lendo os arquivos…".to_string()
                } else if find.query.is_empty() || !find.error.is_empty() {
                    String::new()
                } else {
                    count_note(&find.hits)
                };

                ui.label(egui::RichText::new(count).size(13.0).color(c.muted));

                if reading || find.searching() {
                    spinner(ui);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, "Fixar: o popup não fecha ao perder o foco", find.pinned, c, paint_pin) {
                        find.pinned = !find.pinned;
                    }

                    ui.add_space(8.0);

                    ui.add_enabled_ui(find.mask_on, |ui| {
                        text_field(ui, &mut find.mask, egui::Id::unique(("ft_find_mask", key)), "*.rs", 13.0, 110.0, false, c)
                    });

                    ui.add_space(8.0);
                    let at = Pos2::new(whole.right() - 14.0 - 28.0 - 8.0 - 110.0 - 8.0 - 190.0, header.center().y - 17.0);

                    switch(ui, at, 190.0, &mut find.mask_on, "Máscara de arquivo");
                });
            });

            // the text to find, and how
            let search = band(header.bottom(), SEARCH);

            row(ui, search, &mut |ui| {
                let (icon, _) = ui.allocate_exact_size(Vec2::new(22.0, SEARCH), Sense::hover());

                paint_lens(ui.painter(), icon.center(), c.muted);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    word_toggle(ui, ".*", "Expressão regular", &mut find.regex, c);
                    word_toggle(ui, "W", "Palavras inteiras", &mut find.words, c);
                    word_toggle(ui, "Cc", "Diferenciar maiúsculas de minúsculas", &mut find.case, c);

                    if icon_button(ui, "Várias linhas (Shift+Enter quebra a linha)", find.multiline, c, paint_wrap) {
                        find.multiline = !find.multiline;
                    }

                    let clear = |painter: &Painter, rect: Rect, color: Color32| {
                        crate::app::filetree::pane::glyph_x(painter, rect.center(), 14.0, Stroke::new(1.3, color));
                    };

                    if !find.query.is_empty() && icon_button(ui, "Limpar", false, c, clear) {
                        find.query.clear();
                        find.focus = true;
                    }

                    let id = egui::Id::unique(("ft_find_query", key));
                    let edit = text_field(ui, &mut find.query, id, "Texto a localizar", 15.0, ui.available_width(), find.multiline, c);

                    if std::mem::take(&mut find.focus) || (!typing && ui.memory(|memory| memory.focused().is_none())) {
                        edit.request_focus();
                    }
                });
            });

            // where: the whole root, or a folder
            let scopes = band(search.bottom(), SCOPES);

            row(ui, scopes, &mut |ui| {
                let tabs = Rect::from_min_size(ui.cursor().min, Vec2::new(250.0, SCOPES));
                let mut chosen = usize::from(find.in_dir);

                ui.scope_builder(egui::UiBuilder::new().max_rect(tabs).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
                    ui.add_space((SCOPES - 28.0) / 2.0);
                    segmented(ui, &mut chosen, &["No Projeto", "Diretório"], c);
                });

                find.in_dir = chosen == 1;
                ui.add_space(250.0);

                if find.in_dir {
                    let before = find.folder_text.clone();
                    let id = egui::Id::unique(("ft_find_folder", key));

                    text_field(ui, &mut find.folder_text, id, "Pasta do projeto", 13.0, (whole.width() - 320.0).clamp(160.0, 460.0), false, c);

                    if find.folder_text != before {
                        find.read_folder(root);
                    }
                }
            });

            divider(ui, scopes.bottom());
            // the matches, and under them the file
            let footer = band(whole.bottom() - FOOTER, FOOTER);
            let space = footer.top() - scopes.bottom() - 1.0;
            let list = Rect::from_min_size(Pos2::new(whole.left(), scopes.bottom() + 1.0), Vec2::new(whole.width(), (space * 0.38).floor()));

            let note = if !find.error.is_empty() {
                Some((find.error.clone(), c.danger))
            } else if find.hits.is_empty() && !find.query.is_empty() && !find.searching() && !reading {
                Some(("Nada encontrado".to_string(), c.muted))
            } else {
                None
            };

            if let Some((text, color)) = note {
                ui.painter().text(list.center_top() + Vec2::new(0.0, 22.0), egui::Align2::CENTER_CENTER, text, FontId::proportional(13.0), color);
            }

            let picked = list::rows(ui, list, egui::Id::unique(("ft_find_list", key)), &find.hits, find.selected, up || down, c);

            if let Some(index) = picked.select {
                find.selected = index;
            }

            open = open.or(picked.open);
            divider(ui, list.bottom());
            let pane = Rect::from_min_max(Pos2::new(whole.left(), list.bottom() + 1.0), Pos2::new(whole.right(), footer.top()));

            ui.painter().rect_filled(pane, 0, c.island);
            let ask = find.ask();

            match find.hits.get(find.selected).cloned() {
                Some(hit) => preview::show(&mut find.preview, ui, pane, &hit, &ask, root, key, code, c),
                None => find.preview = None,
            }

            divider(ui, footer.top());

            // the footer: the new-tab switch at the left; at the right Ctrl+Enter and the button
            row(ui, footer, &mut |ui| {
                switch(ui, Pos2::new(footer.left() + 14.0, footer.center().y - 17.0), 270.0, &mut find.new_tab, "Abrir resultados em nova aba");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if action_button(ui, "Abrir na janela de busca").clicked() && !find.hits.is_empty() {
                        keep = true;
                    }

                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Ctrl+Enter").size(12.0).color(c.muted));
                });
            });

            // the corner: dragged, it sizes the popup
            let corner = Rect::from_min_size(whole.right_bottom() - Vec2::splat(18.0), Vec2::splat(18.0));
            let grip = ui.interact(corner, egui::Id::unique(("ft_find_grip", key)), Sense::drag()).on_hover_cursor(CursorIcon::ResizeNwSe);

            find.size += grip.drag_delta();

            for step in [5.0, 10.0, 15.0] {
                let (from, to) = (corner.right_bottom() - Vec2::new(step, 3.0), corner.right_bottom() - Vec2::new(3.0, step));

                ui.painter().line_segment([from, to], Stroke::new(1.0, c.muted.gamma_multiply(0.6)));
            }
        });

    let pressed_outside = |window: &egui::InnerResponse<Option<()>>| {
        ctx.input(|input| input.pointer.any_pressed() && input.pointer.interact_pos().is_some_and(|pos| !window.response.rect.contains(pos)))
    };

    let outside = !find.pinned && shown.as_ref().is_some_and(pressed_outside);

    if keep {
        return FindAct::Results(find.ask(), find.hits.clone());
    }

    match open.and_then(|index| find.hits.get(index)) {
        Some(hit) => FindAct::Open(hit.path.clone(), hit.line, hit.col, find.new_tab),
        None if esc || outside => FindAct::Close,
        None => FindAct::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_question_found_again_keeps_the_selection_and_a_new_one_starts_at_the_top() {
        let hit = |file: &str, line| Hit { path: PathBuf::from(file), line, col: 0, len: 1, text: String::new() };
        let ctx = Context::default();
        let mut texts = Texts::default();
        let mut find = FindFiles::new("x".into(), PathBuf::new());

        find.last = Some((find.ask(), texts.generation));
        find.hits = vec![hit("a", 0), hit("b", 1), hit("c", 2)];
        find.selected = 2;

        let deliver = |find: &mut FindFiles, texts: &mut Texts, hits: Vec<Hit>| {
            let (sender, receiver) = channel();

            sender.send(Ok(hits)).unwrap();
            find.rx = Some(receiver);
            find.tick(texts, &ctx);
        };

        // the root's text read again: the same match, now further down, stays selected
        find.same_ask = true;
        deliver(&mut find, &mut texts, vec![hit("new", 0), hit("a", 0), hit("b", 1), hit("c", 2)]);
        assert_eq!(find.selected, 3);
        // another question: the top
        find.same_ask = false;
        deliver(&mut find, &mut texts, vec![hit("a", 0), hit("c", 2)]);
        assert_eq!(find.selected, 0);
    }
}
