//! The two windows that resolve conflicts (the logic is in merge.rs): "Conflitos", the files a merge
//! or a stash left in conflict with Aceitar Meus / Aceitar Deles / Mesclar…, and the merge window,
//! three panes (mine, the result, theirs) where each conflict takes one side, the other or both.
//! They take a repository folder, not an IDE: of this module's neighbours they only use what draws
//! (the theme, the icons, the highlight).

use super::diffview::soft;
use super::git::Status;
use super::merge::{Chunk, Merge, Side, count, rel, resolve_side, unmerged};
use super::*;

// Darcula DIFF_INSERTED, DIFF_DELETED, DIFF_MODIFIED (as the diff viewer) and DIFF_CONFLICT
const INSERTED: Color32 = Color32::from_rgb(0x29, 0x44, 0x36);
const DELETED: Color32 = Color32::from_rgb(0x48, 0x4A, 0x4A);
const MODIFIED: Color32 = Color32::from_rgb(0x38, 0x55, 0x70);
const CONFLICT: Color32 = Color32::from_rgb(0x45, 0x30, 0x2B);

/// The gutter between two panes, where a conflict's buttons sit.
const GUTTER: f32 = 48.0;

/// What a click in one of the two windows asks for.
enum Act {
    Mine(PathBuf),
    Theirs(PathBuf),
    Merge(PathBuf),
    Close,
    Apply(usize, Side),
    Ignore(usize, Side),
    Reset(usize),
    Accept(Side),
    Go(isize),
    Done,
    Cancel,
}

/// Where a chunk starts in the merge window: its first row, the line each pane is at there, and
/// how many conflicts there are up to it (a conflict's number).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct At {
    row: usize,
    mine: usize,
    result: usize,
    theirs: usize,
    n: usize,
}

/// Every chunk's place and the rows in all: a chunk is as tall as its tallest pane, so the three
/// stay side by side.
fn layout(m: &Merge) -> (Vec<At>, usize) {
    let mut at = At::default();
    let places = m
        .chunks
        .iter()
        .map(|c| {
            at.n += c.conflict as usize;
            let here = at;
            let result = m.result_len(c);
            at.row += c.mine.len().max(result).max(c.theirs.len());
            (at.mine, at.result, at.theirs) = (at.mine + c.mine.len(), at.result + result, at.theirs + c.theirs.len());
            here
        })
        .collect();
    (places, at.row)
}

/// The three panes' fill for a chunk: a waiting conflict is red on all of them; otherwise each
/// pane that differs from the base says how (added, removed, changed), softly.
fn tones(m: &Merge, c: &Chunk) -> [Color32; 3] {
    if c.pending() {
        return [CONFLICT; 3];
    }
    let base = &m.base[c.base.clone()];
    let tone = |same: bool, empty: bool| match (same, base.is_empty(), empty) {
        (true, ..) => Color32::TRANSPARENT,
        (_, true, _) => soft(INSERTED),
        (_, _, true) => soft(DELETED),
        _ => soft(MODIFIED),
    };
    let result = m.result(c);
    let side = |lines: &[String]| tone(lines == base, lines.is_empty());
    [side(m.side(c, Side::Mine)), tone(result.iter().copied().eq(base.iter().map(String::as_str)), result.is_empty()), side(m.side(c, Side::Theirs))]
}

fn window_frame() -> Frame {
    Frame::NONE.fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(12).inner_margin(Margin::same(14))
}

/// The two windows of one repository folder and what they keep between frames. Whoever has the
/// folder (the file tree) keeps one of these, opens it with `show` or `merge` and draws it with `ui`.
#[derive(Default)]
pub struct Conflicts {
    root: PathBuf,
    /// the files in conflict, read when the list opens and after each answer
    files: Vec<PathBuf>,
    /// "Conflitos" is on screen (while there are files in conflict)
    open: bool,
    selected: Option<PathBuf>,
    /// the file in the merge window ("Mesclar…")
    merge: Option<Merge>,
}

impl Conflicts {
    /// "Conflitos" for the repository folder `root`, on `file` when given. Err: nothing is in conflict there.
    pub fn show(&mut self, root: &Path, file: Option<PathBuf>) -> Result<(), String> {
        (self.root, self.merge) = (root.to_path_buf(), None);
        self.read()?;
        if self.files.is_empty() {
            return Err("Nenhum arquivo em conflito.".into());
        }
        (self.open, self.selected) = (true, file);
        Ok(())
    }

    /// The merge window on `file` (absolute, under the repository folder `root`), at its first conflict.
    pub fn merge(&mut self, root: &Path, file: &Path) -> Result<(), String> {
        let mut merge = Merge::read(root, file)?;
        merge.go(1);
        (self.root, self.merge) = (root.to_path_buf(), Some(merge));
        Ok(())
    }

    /// One of the two windows is on screen: the keys are theirs.
    pub fn is_open(&self) -> bool {
        self.open || self.merge.is_some()
    }

    // ponytail: the list is read when it opens and after each answer, not while it sits open; a file
    // resolved outside meanwhile is refused with its reason. Read it on a timer if that is missed.
    fn read(&mut self) -> Result<(), String> {
        self.files = unmerged(&self.root)?.into_iter().map(|(f, _)| self.root.join(f)).collect();
        self.open &= !self.files.is_empty();
        Ok(())
    }

    /// Draws the window that is open. Back: what to tell the person (what was resolved, or why not).
    pub fn ui(&mut self, ctx: &egui::Context) -> Option<String> {
        let act = if self.merge.is_some() { self.window(ctx) } else { self.list(ctx) }?;
        self.act(act)
    }

    fn act(&mut self, act: Act) -> Option<String> {
        let said = match act {
            Act::Mine(path) => resolve_side(&self.root, &path, Side::Mine),
            Act::Theirs(path) => resolve_side(&self.root, &path, Side::Theirs),
            Act::Merge(path) => return self.merge(&self.root.clone(), &path).err(),
            Act::Close => {
                self.open = false;
                return None;
            }
            Act::Done => self.merge.as_ref()?.write(&self.root),
            Act::Cancel => {
                self.merge = None;
                return None;
            }
            step => {
                let m = self.merge.as_mut()?;
                match step {
                    Act::Apply(k, side) => m.apply(k, side),
                    Act::Ignore(k, side) => m.ignore(k, side),
                    Act::Reset(k) => m.reset(k),
                    Act::Accept(side) => m.accept(side),
                    Act::Go(step) => m.go(step),
                    _ => {}
                }
                return None;
            }
        };
        // a file is out: the merge window gives way to the list of what is left (none: it closes)
        if said.is_ok() {
            (self.merge, self.open) = (None, true);
        }
        self.read().ok();
        Some(said.unwrap_or_else(|why| why))
    }

    /// "Conflitos": the files in conflict and, for the selected one, the three ways out.
    fn list(&mut self, ctx: &egui::Context) -> Option<Act> {
        if !self.open || self.files.is_empty() {
            return None;
        }
        let (files, root) = (&self.files, &self.root);
        let selected = self.selected.clone().filter(|p| files.contains(p)).unwrap_or_else(|| files[0].clone());
        let (mut act, mut pick) = (None, None);
        let modal = egui::Modal::new(Id::unique("merge_conflicts")).frame(window_frame()).show(ctx, |ui| {
            t::style(ui.style_mut());
            ui.set_width(620.0);
            ui.label(egui::RichText::new("Conflitos").color(t::TEXT).strong().size(t::UI_SIZE + 2.0));
            ui.add_space(4.0);
            let what = if files.len() == 1 { "O arquivo a seguir tem conflitos:" } else { "Os arquivos a seguir têm conflitos:" };
            ui.label(egui::RichText::new(what).color(t::MUTED));
            ui.add_space(6.0);
            ui.horizontal_top(|ui| {
                Frame::NONE.fill(t::ISLAND).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(6).inner_margin(Margin::symmetric(0, 4)).show(ui, |ui| {
                    ui.set_width(ui.available_width() - 170.0);
                    let list = egui::ScrollArea::vertical().id_salt("conflict_files").max_height(200.0).auto_shrink([false, false]);
                    // the scroll area takes the layout around it, which here is a row
                    list.show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for path in files {
                                let (resp, rect) = row(ui, *path == selected, 24.0);
                                let dir =
                                    path.parent().and_then(|d| d.strip_prefix(root).ok()).map(|d| d.to_string_lossy().replace('\\', "/")).unwrap_or_default();
                                let resp = named(resp, format!("Conflito em {}", rel(root, path)));
                                icons::file(ui.painter(), path, pos2(rect.left() + 20.0, rect.center().y), 1.0);
                                let p = ui.painter().with_clip_rect(rect);
                                let name = p.text(
                                    pos2(rect.left() + 34.0, rect.center().y),
                                    Align2::LEFT_CENTER,
                                    file_name(path),
                                    t::ui(t::UI_SIZE),
                                    Status::Conflict.color(),
                                );
                                p.text(pos2(name.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, dir, t::ui(t::UI_SIZE), t::SECONDARY);
                                if resp.clicked() {
                                    pick = Some(path.clone());
                                }
                                if resp.double_clicked() {
                                    act = Some(Act::Merge(path.clone()));
                                }
                            }
                        })
                    });
                });
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let button =
                        |ui: &mut Ui, label: &str, tip: &str| ui.add(egui::Button::new(label).min_size(vec2(160.0, 28.0))).on_hover_text(tip).clicked();
                    if button(ui, "Aceitar Meus", "Fica a versão do ramo em que você está (HEAD); a que está chegando é descartada") {
                        act = Some(Act::Mine(selected.clone()));
                    }
                    if button(ui, "Aceitar Deles", "Fica a versão que está chegando (o ramo mesclado ou o stash); a do seu ramo é descartada") {
                        act = Some(Act::Theirs(selected.clone()));
                    }
                    if button(ui, "Mesclar…", "Abre as três versões lado a lado para escolher trecho por trecho") {
                        act = Some(Act::Merge(selected.clone()));
                    }
                });
            });
            ui.add_space(10.0);
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new("Fechar").min_size(vec2(84.0, 28.0))).clicked() {
                    act = Some(Act::Close);
                }
            });
        });
        if pick.is_some() {
            self.selected = pick;
        }
        if modal.should_close() {
            act.get_or_insert(Act::Close);
        }
        act
    }

    /// The merge window: mine, the result and theirs side by side, row by row; a conflict's buttons
    /// are in the gutters at its first row. Only Aplicar and Cancelar close it (a click outside
    /// would throw the answers away).
    fn window(&mut self, ctx: &egui::Context) -> Option<Act> {
        let size = (ctx.content_rect().size() - vec2(120.0, 120.0)).max(vec2(640.0, 360.0));
        let font = t::mono(t::EDITOR_SIZE);
        let m = self.merge.as_mut()?;
        let lang = highlight::detect(&m.path);
        let name = rel(&self.root, &m.path);
        let (total, pending) = (m.conflicts().len(), m.pending());
        let (places, rows) = layout(m);
        let scroll_to = m.scroll_to.take();
        let m = &*m;
        let mut act = None;
        egui::Modal::new(Id::unique("merge_window")).frame(window_frame()).show(ctx, |ui| {
            t::style(ui.style_mut());
            ui.set_width(size.x);
            ui.set_height(size.y);
            ui.label(egui::RichText::new(format!("Mesclar revisões de {name}")).color(t::TEXT).strong().size(t::UI_SIZE + 2.0));
            ui.horizontal(|ui| {
                if icons::button(ui, icons::CHEVRON_UP, "Conflito anterior (Shift+F7)", false).clicked() {
                    act = Some(Act::Go(-1));
                }
                if icons::button(ui, icons::CHEVRON_DOWN, "Próximo conflito (F7)", false).clicked() {
                    act = Some(Act::Go(1));
                }
                let (label, color) = match (total, pending) {
                    (0, _) => ("Nenhum conflito: os dois lados se mesclam sozinhos".to_string(), t::MUTED),
                    (_, 0) => ("Todos os conflitos resolvidos".to_string(), t::MUTED),
                    _ => (format!("{} por resolver, de {total}", count(pending, "conflito", "conflitos")), t::WARNING),
                };
                ui.label(egui::RichText::new(label).color(color));
                ui.label(egui::RichText::new("· as alterações sem conflito já estão no resultado").color(t::SECONDARY));
            });
            // Shift+F7 first: plain F7 also matches it, since consume_shortcut ignores an extra Shift
            if ui.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::SHIFT, Key::F7))) {
                act = Some(Act::Go(-1));
            } else if ui.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::NONE, Key::F7))) {
                act = Some(Act::Go(1));
            }
            let (natural, char_w) = ui.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
            let row_h = (natural * 1.2).round();
            let numbers = 5.0 * char_w + 12.0;
            // the three panes and the two gutters between them
            let full = ui.available_rect_before_wrap();
            let pane_w = ((full.width() - 2.0 * GUTTER) / 3.0).max(0.0);
            let left = |n: usize, x: f32| x + n as f32 * (pane_w + GUTTER);
            // ponytail: long lines go sideways with Shift+wheel (or the trackpad), the three panes together and
            // with no bar to drag; a bar under each pane if it is missed
            let widest = m.base.iter().chain(&m.mine).chain(&m.theirs).map(|l| l.chars().count()).max().unwrap_or(0) as f32 * char_w;
            let shift_id = Id::unique(("merge_shift", &m.path));
            let shift = ui.data(|d| d.get_temp::<f32>(shift_id)).unwrap_or(0.0) - ui.input(|i| i.smooth_scroll_delta.x);
            let shift = shift.clamp(0.0, (widest - (pane_w - numbers - 12.0)).max(0.0));
            ui.data_mut(|d| d.insert_temp(shift_id, shift));
            ui.horizontal(|ui| {
                for (n, title) in ["Minha versão", "Resultado", "Versão deles"].into_iter().enumerate() {
                    let at = pos2(left(n, full.left()) + 8.0, full.top() + 11.0);
                    ui.painter().text(at, Align2::LEFT_CENTER, title, t::ui(t::UI_SIZE - 1.0), t::MUTED);
                }
                ui.allocate_space(vec2(full.width(), 22.0));
            });
            let mut area = egui::ScrollArea::vertical().id_salt(("merge", &m.path)).max_height((ui.available_height() - 44.0).max(60.0)).auto_shrink(false);
            if let Some(k) = scroll_to {
                area = area.vertical_scroll_offset((places[k].row as f32 * row_h - 3.0 * row_h).max(0.0));
            }
            Frame::NONE.fill(t::ISLAND).corner_radius(6).show(ui, |ui| {
                area.show_rows(ui, row_h, rows, |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // the chunk of the row before: its fills and its result, found once per chunk
                    let mut chunk: Option<(usize, [Color32; 3], Vec<&str>)> = None;
                    for r in range {
                        let k = places.partition_point(|at| at.row <= r) - 1;
                        let (c, at) = (&m.chunks[k], places[k]);
                        if chunk.as_ref().is_none_or(|seen| seen.0 != k) {
                            chunk = Some((k, tones(m, c), m.result(c)));
                        }
                        let Some((_, fill, result)) = &chunk else { continue };
                        let i = r - at.row;
                        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::hover());
                        let p = ui.painter().clone();
                        let lines = [
                            (m.side(c, Side::Mine).get(i).map(String::as_str), at.mine),
                            (result.get(i).copied(), at.result),
                            (m.side(c, Side::Theirs).get(i).map(String::as_str), at.theirs),
                        ];
                        for (n, (line, first)) in lines.into_iter().enumerate() {
                            let pane = Rect::from_min_size(pos2(left(n, rect.left()), rect.top()), vec2(pane_w, row_h));
                            // a side's gutter carries its fill up to the result
                            let band = match n {
                                0 => pane.with_max_x(pane.right() + GUTTER),
                                2 => pane.with_min_x(pane.left() - GUTTER),
                                _ => pane,
                            };
                            p.rect_filled(band, 0, fill[n]);
                            for x in [pane.left(), pane.right()] {
                                p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, t::POPUP_BORDER));
                            }
                            let Some(line) = line else { continue };
                            p.text(
                                pos2(pane.left() + numbers - 8.0, pane.center().y),
                                Align2::RIGHT_CENTER,
                                (first + i + 1).to_string(),
                                font.clone(),
                                t::LINE_NUMBER,
                            );
                            let galley = ui.fonts_mut(|f| f.layout_job(highlight::layout(lang, line, font.clone(), row_h)));
                            p.with_clip_rect(pane.with_min_x(pane.left() + numbers)).galley(
                                pos2(pane.left() + numbers + 4.0 - shift, pane.top()),
                                galley,
                                t::EDITOR_TEXT,
                            );
                        }
                        if m.current == Some(k) {
                            p.rect_filled(Rect::from_min_size(pos2(left(1, rect.left()), rect.top()), vec2(2.0, row_h)), 0, t::ACCENT);
                        }
                        if !c.conflict || i > 0 {
                            continue;
                        }
                        // the conflict's buttons: ✕ and » for mine, « and ✕ for theirs; an answered conflict can be put back
                        let button = |x: f32, arrow: &str, icon: Option<icons::Svg>, label: String| {
                            let at = Rect::from_center_size(pos2(x, rect.center().y), vec2(20.0, row_h.min(20.0)));
                            let resp = named(ui.interact(at, ui.scope_id().with((k, x.to_bits())), Sense::click()), &label);
                            if resp.hovered() {
                                p.rect_filled(at, 4, t::HOVER);
                            }
                            match icon {
                                Some(icon) => icons::paint(&p, icon, Rect::from_center_size(at.center(), vec2(16.0, 16.0))),
                                None => {
                                    p.text(at.center(), Align2::CENTER_CENTER, arrow, t::ui(16.0), if resp.hovered() { t::TEXT } else { t::LINK });
                                }
                            }
                            resp.on_hover_text(label).clicked()
                        };
                        let (g1, g2) = (left(0, rect.left()) + pane_w, left(1, rect.left()) + pane_w);
                        if !c.done[0] {
                            if button(g1 + 13.0, "", Some(icons::CLOSE_SMALL), format!("Ignorar o lado esquerdo no conflito {}", at.n)) {
                                act = Some(Act::Ignore(k, Side::Mine));
                            }
                            if button(g1 + GUTTER - 13.0, "»", None, format!("Aceitar o lado esquerdo no conflito {}", at.n)) {
                                act = Some(Act::Apply(k, Side::Mine));
                            }
                        }
                        if !c.done[1] {
                            if button(g2 + 13.0, "«", None, format!("Aceitar o lado direito no conflito {}", at.n)) {
                                act = Some(Act::Apply(k, Side::Theirs));
                            }
                            if button(g2 + GUTTER - 13.0, "", Some(icons::CLOSE_SMALL), format!("Ignorar o lado direito no conflito {}", at.n)) {
                                act = Some(Act::Ignore(k, Side::Theirs));
                            }
                        }
                        if c.done != [false; 2] {
                            let x = if c.done[0] { g1 } else { g2 } + GUTTER / 2.0;
                            if button(x, "", Some(icons::REFRESH), format!("Desfazer a resolução do conflito {}", at.n)) {
                                act = Some(Act::Reset(k));
                            }
                        }
                    }
                });
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new("Aceitar Esquerda").min_size(vec2(0.0, 28.0)))
                    .on_hover_text("O resultado fica igual à minha versão, inteira")
                    .clicked()
                {
                    act = Some(Act::Accept(Side::Mine));
                }
                if ui
                    .add(egui::Button::new("Aceitar Direita").min_size(vec2(0.0, 28.0)))
                    .on_hover_text("O resultado fica igual à versão deles, inteira")
                    .clicked()
                {
                    act = Some(Act::Accept(Side::Theirs));
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let apply = egui::Button::new(egui::RichText::new("Aplicar").color(Color32::WHITE)).fill(t::ACCENT).min_size(vec2(84.0, 28.0));
                    let apply = ui
                        .add_enabled(pending == 0, apply)
                        .on_hover_text("Grava o resultado no arquivo e marca o conflito como resolvido (git add)")
                        .on_disabled_hover_text("Resolva todos os conflitos antes de aplicar");
                    if apply.clicked() {
                        act = Some(Act::Done);
                    }
                    if ui.add(egui::Button::new("Cancelar").min_size(vec2(84.0, 28.0))).clicked() {
                        act = Some(Act::Cancel);
                    }
                });
            });
        });
        act
    }
}

#[cfg(test)]
mod tests {
    use super::super::mergetest::conflicted;
    use super::*;

    #[test]
    fn a_chunk_is_as_tall_as_its_tallest_pane_and_the_lines_keep_counting() {
        // one conflict: mine has two lines there, theirs one, the base (the result, until answered) one
        let mut m = Merge::new(PathBuf::from("a.txt"), "a\nb\nc\n", "a\nM1\nM2\nc\n", "a\nT\nc\n");
        let (places, rows) = layout(&m);
        assert_eq!(places.len(), 3);
        assert_eq!(places[1], At { row: 1, mine: 1, result: 1, theirs: 1, n: 1 });
        assert_eq!(places[2], At { row: 3, mine: 3, result: 2, theirs: 2, n: 1 });
        assert_eq!(rows, 5);
        // both sides in the result: it becomes the tallest pane
        let k = m.conflicts()[0];
        assert_eq!(tones(&m, &m.chunks[k]), [CONFLICT; 3]);
        m.pick(k, &[Side::Mine, Side::Theirs]);
        let (places, rows) = layout(&m);
        assert_eq!((places[2].row, places[2].result, rows), (4, 4, 6));
        assert_eq!(tones(&m, &m.chunks[k]), [soft(MODIFIED); 3]);
        assert_eq!(tones(&m, &m.chunks[0]), [Color32::TRANSPARENT; 3]);
    }

    #[test]
    fn the_windows_take_a_folder_and_answer_file_by_file() {
        let dir = conflicted("windows");
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        let mut w = Conflicts::default();
        assert!(!w.is_open());
        w.show(&dir, Some(b.clone())).unwrap();
        assert_eq!((w.is_open(), &w.files, &w.selected), (true, &vec![a.clone(), b.clone()], &Some(b.clone())));
        // Mesclar…: the three panes on a.txt, at its first conflict; theirs for everything, then Aplicar
        assert_eq!(w.act(Act::Merge(a.clone())), None);
        let first = w.merge.as_ref().map(|m| (m.conflicts()[0], m.current, m.pending()));
        assert!(matches!(first, Some((k, Some(at), 2)) if k == at), "{first:?}");
        assert_eq!(w.act(Act::Accept(Side::Theirs)), None);
        let said = w.act(Act::Done).unwrap();
        assert!(said.contains("a.txt mesclado") && said.contains("1 arquivo"), "{said}");
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "1\ndois deles\n3\n4\n5\n6\n7\noito deles\n9\ndeles\n");
        // back at the list, with what is left; Cancelar in the merge window throws its answers away
        assert!(w.merge.is_none() && w.open && w.files == [b.clone()]);
        w.act(Act::Merge(b.clone()));
        w.act(Act::Accept(Side::Theirs));
        w.act(Act::Cancel);
        assert!(w.merge.is_none() && w.open && std::fs::read_to_string(&b).unwrap().contains("<<<<<<<"));
        // Aceitar Meus on the last one: nothing left, the window closes by itself
        let said = w.act(Act::Mine(b.clone())).unwrap();
        assert!(said.contains("minha versão") && said.contains("conclui o merge"), "{said}");
        assert!(!w.is_open() && std::fs::read_to_string(&b).unwrap() == "b meu\n");
        assert_eq!(w.show(&dir, None).unwrap_err(), "Nenhum arquivo em conflito.");
        // a file that is not in conflict has no merge window, and says why
        assert!(w.merge(&dir, &a).unwrap_err().contains("não está em conflito") && !w.is_open());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_answer_given_somewhere_else_is_not_written_over() {
        let dir = conflicted("windows-outside");
        let b = dir.join("b.txt");
        let mut w = Conflicts::default();
        w.merge(&dir, &b).unwrap();
        w.act(Act::Accept(Side::Theirs));
        // a terminal, or an agent, answers the file while its merge window is open
        resolve_side(&dir, &b, Side::Mine).unwrap();
        let said = w.act(Act::Done).unwrap();
        assert!(said.contains("resolvido por fora") && std::fs::read_to_string(&b).unwrap() == "b meu\n", "{said}");
        // the window stays, to be closed by hand
        assert!(w.merge.is_some());
        w.act(Act::Cancel);
        assert!(!w.is_open());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn esc_closes_the_list_and_leaves_the_merge_window_open() {
        let dir = conflicted("windows-esc");
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        for (name, family) in [("ide", egui::FontFamily::Proportional), ("ide-mono", egui::FontFamily::Monospace)] {
            fonts.families.insert(egui::FontFamily::Name(name.into()), fonts.families[&family].clone());
        }
        ctx.set_fonts(fonts);
        // a frame with the window drawn, as the app does it; back: what it said
        let frame = |w: &mut Conflicts, events: Vec<egui::Event>| {
            let screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000.0, 600.0)));
            let mut said = None;
            ctx.run_ui(egui::RawInput { screen_rect, events, ..Default::default() }, |ui| said = w.ui(ui.ctx())).textures_delta.clear();
            said
        };
        let esc = || vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }];
        let mut w = Conflicts::default();
        w.show(&dir, None).unwrap();
        frame(&mut w, vec![]);
        frame(&mut w, vec![]);
        assert!(w.is_open());
        frame(&mut w, esc());
        assert!(!w.is_open(), "Esc closes the list");
        // the merge window keeps the answers: only Aplicar and Cancelar close it
        w.merge(&dir, &dir.join("a.txt")).unwrap();
        frame(&mut w, vec![]);
        frame(&mut w, vec![]);
        frame(&mut w, esc());
        assert!(w.merge.is_some());
        // F7 and Shift+F7 walk the conflicts
        let at = |w: &Conflicts| w.merge.as_ref().and_then(|m| m.current);
        let first = at(&w);
        let f7 = |modifiers| vec![egui::Event::Key { key: Key::F7, physical_key: None, pressed: true, repeat: false, modifiers }];
        frame(&mut w, f7(Modifiers::NONE));
        assert_ne!(at(&w), first);
        frame(&mut w, f7(Modifiers::SHIFT));
        assert_eq!(at(&w), first);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
