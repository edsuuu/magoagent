//! The changes of a file against the last commit in a file tree node's editor, as the reference IDE's line status tracker
//! draws them: a bar in the gutter (green for added lines, blue for modified ones, a gray triangle where lines were
//! deleted), the popup a press on it opens (the old lines, Anterior and Próxima, Reverter, Copiar and Mostrar Diff) and
//! the rollback of one range. The side-by-side tab is in filetree_compare.rs. The ranges come from the same line
//! diff as the viewer's, so a marker and the tab's n-th change are the same one.

use crate::app::filetree_compare::Compare;
use crate::app::filetree_editor::{Body, TextBuf, load};
use crate::app::filetree_pane::{Editor, PaneTab};
use crate::ide::theme as t;
use crate::ide::word_marks;
use crate::prelude::*;
use similar::{DiffTag, TextDiff};
use std::ops::Range;
use std::path::Path;

pub(crate) const ADDED: Color32 = Color32::from_rgb(0x54, 0x91, 0x59);
pub(crate) const MODIFIED: Color32 = Color32::from_rgb(0x37, 0x5F, 0xAD);
const DELETED: Color32 = Color32::from_rgb(0x86, 0x8A, 0x91);
/// The popup's old lines: the viewer's gray for lines that went, the blue of a changed word.
const OLD_LINE: Color32 = Color32::from_rgb(0x2B, 0x2D, 0x2E);
const OLD_WORD: Color32 = Color32::from_rgb(0x38, 0x55, 0x70);

/// One range of change: the lines it was (`old`, in the last commit's text) and the lines it is (`new`), half-open.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hunk {
    pub(crate) old: Range<usize>,
    pub(crate) new: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Change {
    Added,
    Modified,
    Deleted,
}

impl Hunk {
    pub(crate) fn change(&self) -> Change {
        match (self.old.is_empty(), self.new.is_empty()) {
            (true, _) => Change::Added,
            (_, true) => Change::Deleted,
            _ => Change::Modified,
        }
    }

    /// "adicionado", "modificado" or "apagado".
    pub(crate) fn word(&self) -> &'static str {
        match self.change() {
            Change::Added => "adicionado",
            Change::Modified => "modificado",
            Change::Deleted => "apagado",
        }
    }
}

/// The ranges where `new` differs from `old`, in order.
pub(crate) fn hunks(old: &str, new: &str) -> Vec<Hunk> {
    let ops = TextDiff::from_lines(old, new).ops().to_vec();
    ops.iter()
        .filter_map(|op| {
            let (tag, o, n) = op.as_tag_tuple();
            (tag != DiffTag::Equal).then_some(Hunk { old: o, new: n })
        })
        .collect()
}

/// `text` with the lines of `hunk` as they are in `head` (the rollback of that one range).
pub(crate) fn rollback(text: &str, head: &str, hunk: &Hunk) -> String {
    let diff = TextDiff::from_lines(head, text);
    let (o, n) = (diff.old_slices(), diff.new_slices());
    let mut out: String = n[..hunk.new.start.min(n.len())].concat();
    out.push_str(&o[hunk.old.start.min(o.len())..hunk.old.end.min(o.len())].concat());
    let tail = n[hunk.new.end.min(n.len())..].concat();
    // the old range ended the file without a line end, and lines follow it
    if !tail.is_empty() && !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out + &tail
}

/// `path` as the last commit has it, with LF line ends (None outside git, or for a file it does not have).
pub(crate) fn head_text(path: &Path) -> Option<String> {
    let (dir, name) = (path.parent()?, path.file_name()?.to_string_lossy());
    let text = crate::ide::run_git(dir, &["show", &format!("HEAD:./{name}")]).ok()?;
    Some(text.replace("\r\n", "\n").trim_start_matches('\u{feff}').to_string())
}

/// The text's ranges, worked out again only when the text (`key`) moved.
pub(crate) fn refresh(buf: &mut TextBuf, key: u64) {
    let Some(head) = &buf.head else { return };
    if buf.hunks.as_ref().is_none_or(|h| h.0 != key) {
        buf.hunks = Some((key, hunks(head, &buf.text)));
    }
}

/// What the markers and their popup asked of the editor.
#[derive(Default)]
pub(crate) struct MarkOut {
    /// A rollback changed the text.
    pub(crate) changed: bool,
    /// "Mostrar Diff".
    pub(crate) diff: bool,
}

#[derive(PartialEq)]
enum PopAct {
    Prev,
    Next,
    Revert,
    Copy,
    Diff,
    Close,
}

/// The bars at the gutter's right edge, each pressable, and the popup of the one pressed. `spans` are the lines' rows
/// (top, bottom) from `origin_y`; `gutter` is where the line numbers are, `p` paints clipped to the editor.
#[allow(clippy::too_many_arguments)]
pub(crate) fn marks_ui(ui: &mut egui::Ui, p: &Painter, gutter: Rect, origin_y: f32, spans: &[(f32, f32)], buf: &mut TextBuf, id: egui::Id, z: f32) -> MarkOut {
    let mut out = MarkOut::default();
    let Some((_, hunks)) = buf.hunks.clone() else { return out };
    let clip = p.clip_rect();
    let top = |l: usize| origin_y + spans.get(l).or(spans.last()).map_or(0.0, |s| if l < spans.len() { s.0 } else { s.1 });
    let bottom = |l: usize| origin_y + spans.get(l).or(spans.last()).map_or(0.0, |s| s.1);
    let (x0, x1) = (gutter.right() - 11.0 * z, gutter.right() - 7.0 * z);
    let mut rects = Vec::with_capacity(hunks.len());
    let mut pressed = None;
    for (i, h) in hunks.iter().enumerate() {
        let (y0, y1) = match h.change() {
            Change::Deleted => (top(h.new.start) - 4.0 * z, top(h.new.start) + 4.0 * z),
            _ => (top(h.new.start) + z, bottom(h.new.end - 1) - z),
        };
        let hit = Rect::from_min_max(Pos2::new(x0 - 2.0 * z, y0), Pos2::new(x1 + 2.0 * z, y1));
        rects.push(hit);
        if y1 < clip.top() || y0 > clip.bottom() {
            continue;
        }
        match h.change() {
            Change::Deleted => {
                let y = top(h.new.start);
                p.add(Shape::convex_polygon(vec![Pos2::new(x0 - z, y - 4.0 * z), Pos2::new(x1 + z, y), Pos2::new(x0 - z, y + 4.0 * z)], DELETED, Stroke::NONE));
            }
            c => {
                let color = if c == Change::Added { ADDED } else { MODIFIED };
                p.rect_filled(Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1)), 1, color);
            }
        }
        let r = ui.interact(hit.intersect(clip), id.with(("hunk", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        if r.clicked() {
            pressed = Some(i);
        }
    }
    let Some(i) = buf.pop.filter(|i| *i < hunks.len()) else {
        buf.pop = pressed;
        return out;
    };
    let act = popup_ui(ui.ctx(), id, rects[i].right_bottom().max(Pos2::new(0.0, clip.top())), gutter, buf, &hunks, i, z);
    buf.pop = if pressed == Some(i) { None } else { pressed.or(Some(i)) };
    // the line the caret and the view go to
    let go = |buf: &mut TextBuf, to: usize| {
        buf.pop = Some(to);
        buf.jump = Some((hunks[to].new.start.min(buf.text.matches('\n').count()), 0));
    };
    match act {
        Some(PopAct::Prev) if i > 0 => go(buf, i - 1),
        Some(PopAct::Next) if i + 1 < hunks.len() => go(buf, i + 1),
        Some(PopAct::Revert) => {
            if let Some(head) = &buf.head {
                buf.text = rollback(&buf.text, head, &hunks[i]);
                (buf.pop, out.changed) = (None, true);
            }
        }
        Some(PopAct::Copy) => {
            let old = buf.head.as_deref().map(|h| old_lines(h, &hunks[i])).unwrap_or_default();
            ui.ctx().copy_text(old);
        }
        Some(PopAct::Diff) => (buf.pop, out.diff) = (None, true),
        Some(PopAct::Close) => buf.pop = None,
        _ => {}
    }
    out
}

/// The old lines of `hunk` (as the last commit has them).
fn old_lines(head: &str, hunk: &Hunk) -> String {
    head.split_inclusive('\n').skip(hunk.old.start).take(hunk.old.len()).collect()
}

/// The popup of a marker: its toolbar and, when lines went or changed, the old ones with their changed words. Esc or a
/// press outside it (the gutter apart: the markers are pressed there) closes it.
#[allow(clippy::too_many_arguments)]
fn popup_ui(ctx: &Context, id: egui::Id, at: Pos2, gutter: Rect, buf: &TextBuf, hunks: &[Hunk], i: usize, z: f32) -> Option<PopAct> {
    let (h, head) = (&hunks[i], buf.head.as_deref().unwrap_or_default());
    let mut act = None;
    let area = egui::Area::new(id.with("hunk_popup")).order(egui::Order::Foreground).fixed_pos(at + Vec2::new(6.0 * z, 0.0)).constrain(true).show(ctx, |ui| {
        egui::Frame::NONE.fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(6).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.horizontal(|ui| {
                let button = |ui: &mut egui::Ui, label: &str, on: bool, tip: &str| {
                    let text = egui::RichText::new(label).font(t::ui(t::UI_SIZE * z)).color(if on { t::TEXT } else { t::SECONDARY });
                    ui.add_enabled(on, egui::Button::new(text).frame(false)).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip).clicked()
                };
                if button(ui, "Anterior", i > 0, "Mudança anterior") {
                    act = Some(PopAct::Prev);
                }
                if button(ui, "Próxima", i + 1 < hunks.len(), "Próxima mudança") {
                    act = Some(PopAct::Next);
                }
                if button(ui, "Reverter", true, "Desfaz esta mudança: o trecho volta como está no HEAD") {
                    act = Some(PopAct::Revert);
                }
                if button(ui, "Copiar", !h.old.is_empty(), "Copia o trecho antigo") {
                    act = Some(PopAct::Copy);
                }
                if button(ui, "Mostrar Diff", true, "Ctrl+D") {
                    act = Some(PopAct::Diff);
                }
                ui.label(egui::RichText::new(format!("{} de {}", i + 1, hunks.len())).font(t::ui((t::UI_SIZE - 1.0) * z)).color(t::MUTED));
            });
            if h.old.is_empty() {
                return;
            }
            ui.separator();
            let old = old_lines(head, h);
            let new: String = buf.text.split_inclusive('\n').skip(h.new.start).take(h.new.len()).collect();
            let font = t::mono(t::EDITOR_SIZE * z);
            // a modified range shows its changed words, as the reference's popup does
            let words = if h.new.is_empty() { vec![] } else { word_marks(&old, &new).0 };
            let mut job = egui::text::LayoutJob::default();
            let mut at = 0;
            let mut push = |job: &mut egui::text::LayoutJob, to: usize, word: bool| {
                if to > at {
                    let bg = if word { OLD_WORD } else { OLD_LINE };
                    job.append(&old[at..to], 0.0, egui::TextFormat { font_id: font.clone(), color: t::EDITOR_TEXT, background: bg, ..Default::default() });
                    at = to;
                }
            };
            for w in words {
                push(&mut job, w.start, false);
                push(&mut job, w.end, true);
            }
            push(&mut job, old.len(), false);
            egui::ScrollArea::both().max_height(220.0 * z).max_width(560.0 * z).auto_shrink(true).show(ui, |ui| {
                ui.add(egui::Label::new(job.clone()).extend());
            });
        });
    });
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !gutter.contains(p)));
    if outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
        act = Some(PopAct::Close);
    }
    act
}

impl Editor {
    /// Opens the tab that compares `file` with the last commit, side by side, over what is in the editor now (the open tab's
    /// text, saved or not; else the disk's). Err with why when there is nothing to compare.
    pub(crate) fn open_compare(&mut self, file: &Path) -> Result<usize, String> {
        let old = head_text(file).ok_or("Este arquivo não está no último commit do git, então não há com o que comparar.")?;
        let open = self.tabs.iter().find_map(|t| match &t.body {
            Body::Text(b) if t.path == file => Some(b.text.clone()),
            _ => None,
        });
        let new = match open {
            Some(text) => text,
            None => match load(file, &self.ctx) {
                Body::Text(b) => b.text,
                _ => return Err("Este arquivo não é um texto, então não há diff para mostrar.".into()),
            },
        };
        let n = hunks(&old, &new).len();
        if n == 0 {
            return Err("Não há diferenças com o HEAD.".into());
        }
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        let path = file.with_file_name(format!("HEAD ↔ {name}"));
        match self.tabs.iter_mut().find(|t| t.path == path) {
            Some(tab) => tab.body = Body::Compare(Box::new(Compare::new(file, old, new))),
            None => self.tabs.push(PaneTab { path: path.clone(), body: Body::Compare(Box::new(Compare::new(file, old, new))) }),
        }
        self.active = Some(path);
        Ok(n)
    }

    /// A compare tab follows the text of its file while that is open in another tab (what is typed, a rollback).
    pub(crate) fn sync_compares(&mut self) {
        let news: Vec<(usize, String)> = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let Body::Compare(c) = &t.body else { return None };
                self.tabs.iter().find_map(|o| match &o.body {
                    Body::Text(b) if o.path == c.file && b.text != c.new => Some((i, b.text.clone())),
                    _ => None,
                })
            })
            .collect();
        for (i, text) in news {
            if let Body::Compare(c) = &mut self.tabs[i].body {
                c.set_new(text);
            }
        }
    }

    /// The ranges changed in `file` against the last commit: the open tab's text, else the disk's.
    pub(crate) fn file_hunks(&self, file: &Path) -> Result<(String, String, Vec<Hunk>), String> {
        let old = head_text(file).ok_or("Este arquivo não está no último commit do git.")?;
        let new = match self.tabs.iter().find(|t| t.path == file).map(|t| &t.body) {
            Some(Body::Text(b)) => b.text.clone(),
            _ => match load(file, &self.ctx) {
                Body::Text(b) => b.text,
                _ => return Err("Este arquivo não é um texto.".into()),
            },
        };
        let list = hunks(&old, &new);
        Ok((old, new, list))
    }
}

impl DrawingApp {
    /// "Mostrar Diff" (Ctrl+D) of `file` in the pane of tree `id`.
    pub(crate) fn ft_show_diff(&mut self, id: &str, file: &Path) -> Result<usize, String> {
        self.ft_show_editor(id, true);
        self.ft_editor(id).open_compare(file)
    }

    /// The `file_tree` actions `diff` (open the tab, list the ranges), `hunks` (list them) and `revert_hunk` (rollback of the
    /// range `index`, counted from 1 as `hunks` lists them; the file is written).
    pub(crate) fn act_tree_diff(&mut self, id: &str, action: &str, args: &Value, file: PathBuf) -> Result<String, String> {
        if !file.is_file() {
            return Err("Diga o arquivo (path).".into());
        }
        let name = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let (old, new, list) = self.ft_editor(id).file_hunks(&file)?;
        let describe = |list: &[Hunk]| -> String {
            let lines: String = list
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let at = match h.change() {
                        Change::Deleted => format!("depois da linha {}", h.new.start),
                        _ => format!("linhas {} a {}", h.new.start + 1, h.new.end),
                    };
                    let was = if h.old.is_empty() { String::new() } else { format!(", antes {} a {}", h.old.start + 1, h.old.end) };
                    format!("{}. {} ({at}{was})\n", i + 1, h.word())
                })
                .collect();
            format!("{name}: {} mudança(s) contra o HEAD\n{lines}", list.len())
        };
        match action {
            "hunks" => Ok(describe(&list)),
            "diff" => {
                self.ft_show_diff(id, &file)?;
                Ok(describe(&list))
            }
            _ => {
                let n = args["index"].as_u64().filter(|n| *n > 0).ok_or("Diga qual mudança reverter (index, a partir de 1, como hunks lista).")? as usize;
                let hunk = list.get(n - 1).ok_or_else(|| format!("{name} tem só {} mudança(s).", list.len()))?;
                if self.filetrees.editors.get(id).is_some_and(|ed| ed.tabs.iter().any(|t| t.path == file && t.dirty())) {
                    return Err("O arquivo tem mudanças não salvas no editor: salve antes (save).".into());
                }
                let text = rollback(&new, &old, hunk);
                // the file's own line ends and BOM stay: only the lines of the range change
                let raw = fs::read_to_string(&file).map_err(|e| format!("Não foi possível ler {name}: {e}"))?;
                let text = if raw.contains("\r\n") { text.replace('\n', "\r\n") } else { text };
                let text = if raw.starts_with('\u{feff}') { format!("\u{feff}{text}") } else { text };
                fs::write(&file, text).map_err(|e| format!("Não foi possível salvar {name}: {e}"))?;
                let ctx = self.filetrees.ctx.clone().unwrap_or_default();
                if let Some(tab) = self.filetrees.editors.get_mut(id).and_then(|ed| ed.tabs.iter_mut().find(|t| t.path == file)) {
                    tab.body = load(&file, &ctx);
                }
                Ok(format!("Mudança {n} de {name} revertida para o HEAD."))
            }
        }
    }
}
