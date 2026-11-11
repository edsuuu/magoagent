//! The editor pane of a file tree node, as the reference app's: opening a file grows the node to the
//! right by a 600 wide pane (a 1 wide seam between), with a strip of tabs over the text. The text, its
//! colors, saving, pictures and PDFs are in filetree_editor.rs; the strip and the seam are drawn here.
//! Nothing in it depends on an IDE node.

use crate::app::filetree::*;
use crate::app::filetree_editor::{Body, BodyAct, body_ui, load};
use crate::app::filetree_menu::{Lead, Row, menu_row};
use crate::app::filetree_ui::{Palette, area, cr, fit, outline};
use crate::app::ui::menu_look::menu_frame;
use crate::prelude::*;
use std::path::Path;

/// The pane's width when it opens, the seam, the tree's and the pane's narrowest, the tab strip's
/// height and how narrow the node gets when the pane closes (all in the node's own points).
pub(crate) const EDITOR_W: f32 = 600.0;
pub(crate) const SEAM: f32 = 1.0;
const TREE_MIN: f32 = 280.0;
const EDITOR_MIN: f32 = 240.0;
const STRIP: f32 = 34.0;
const CLOSE_MIN: f32 = 300.0;

/// The tree's width in a node `node_w` wide: what was dragged, or all but the default pane, kept
/// between the narrowest the tree and the pane allow.
pub(crate) fn tree_width(spec: &FileTreeSpec, node_w: f32) -> f32 {
    let raw = spec.tree_width.unwrap_or(node_w - EDITOR_W - SEAM);
    raw.min((node_w - EDITOR_MIN - SEAM).max(TREE_MIN)).max(TREE_MIN)
}

/// The node's tree column and, with the editor shown, its pane (on screen, at zoom `z`).
pub(crate) fn parts(spec: &FileTreeSpec, frame: Rect, z: f32) -> (Rect, Option<Rect>) {
    if !spec.editor {
        return (frame, None);
    }
    let tree = Rect::from_min_max(frame.min, Pos2::new(frame.left() + tree_width(spec, frame.width() / z) * z, frame.bottom()));
    let pane = Rect::from_min_max(Pos2::new(tree.right() + SEAM * z, frame.top()), frame.max);
    (tree, Some(pane))
}

/// A file the pane has a tab for, and what it shows of it.
pub(crate) struct PaneTab {
    pub(crate) path: PathBuf,
    pub(crate) body: Body,
}

impl PaneTab {
    pub(crate) fn dirty(&self) -> bool {
        matches!(&self.body, Body::Text(t) if t.dirty)
    }

    fn save(&mut self) -> Result<(), String> {
        match &mut self.body {
            Body::Text(t) => t.save(&self.path),
            _ => Ok(()),
        }
    }
}

/// One node's editor: the tabs the strip shows, the one forward, and the preview among them.
pub(crate) struct Editor {
    ctx: Context,
    pub(crate) tabs: Vec<PaneTab>,
    pub(crate) active: Option<PathBuf>,
    /// The tab a single click made, in italics: the next click replaces it, an edit or a double-click keeps it.
    pub(crate) preview: Option<PathBuf>,
    last_check: f64,
    /// "Quebrar Linhas" of the editor's settings.
    pub(crate) wrap: bool,
}

impl Editor {
    pub(crate) fn new(ctx: Context) -> Self {
        Editor { ctx, tabs: vec![], active: None, preview: None, last_check: 0.0, wrap: false }
    }

    /// Opens `path` and brings it forward. `pinned` (a double-click, Enter, "Abrir") keeps the tab and
    /// takes the keyboard; a click only shows it, and the next click takes its place.
    pub(crate) fn open(&mut self, path: PathBuf, pinned: bool) {
        let new = !self.tabs.iter().any(|t| t.path == path);
        if new && !pinned {
            if let Some(old) = self.preview.clone() {
                let _ = self.close(&old);
            }
        }
        if new {
            let body = load(&path, &self.ctx);
            self.tabs.push(PaneTab { path: path.clone(), body });
            if !pinned {
                self.preview = Some(path.clone());
            }
        } else if pinned && self.preview.as_ref() == Some(&path) {
            self.preview = None;
        }
        if pinned {
            if let Some(Body::Text(t)) = self.tabs.iter_mut().find(|t| t.path == path).map(|t| &mut t.body) {
                t.focus = true;
            }
        }
        self.active = Some(path);
    }

    /// Saves and closes a tab; Err when it stays (its text could not be written).
    pub(crate) fn close(&mut self, path: &Path) -> Result<(), String> {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.path == path) {
            tab.save()?;
        }
        self.tabs.retain(|t| t.path != path);
        if self.preview.as_deref() == Some(path) {
            self.preview = None;
        }
        if self.active.as_deref() == Some(path) {
            self.active = self.tabs.last().map(|t| t.path.clone());
        }
        Ok(())
    }

    /// Saves the tab forward (Ctrl+S: over a file that changed under it too).
    pub(crate) fn save(&mut self) -> Result<(), String> {
        let active = self.active.clone();
        self.tabs.iter_mut().find(|t| Some(&t.path) == active.as_ref()).map_or(Ok(()), PaneTab::save)
    }

    /// Saves every tab with unsaved text (the app is closing, or the node is going).
    pub(crate) fn save_all(&mut self) {
        for tab in &mut self.tabs {
            let _ = tab.save();
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        self.active.as_ref().is_some_and(|a| self.tabs.iter().any(|t| &t.path == a && t.dirty()))
    }
}

impl Drop for Editor {
    /// The node was removed: what was typed is saved.
    fn drop(&mut self) {
        self.save_all();
    }
}

/// Something a press on the pane asks of the node.
pub(super) enum PaneAct {
    Select(PathBuf),
    Close(PathBuf),
    Save,
    Hide,
    /// The tree's new width, in the node's own points.
    Seam(f32),
    Reveal(PathBuf),
    OpenDefault(PathBuf),
    Retry(PathBuf),
    Pin(PathBuf),
    CloseOthers(PathBuf),
    CloseAll,
    CopyPath(PathBuf),
    RevealInTree(PathBuf),
    Wrap,
}

/// lucide's save, at 24-grid points.
fn glyph_save(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    let grid = [(15.2, 3.0), (16.6, 3.6), (20.4, 7.4), (21.0, 8.8), (21.0, 19.0), (19.0, 21.0), (5.0, 21.0), (3.0, 19.0), (3.0, 5.0), (5.0, 3.0)];
    outline(p, c, size, &grid, true, stroke);
    outline(p, c, size, &[(17.0, 21.0), (17.0, 14.0), (16.0, 13.0), (8.0, 13.0), (7.0, 14.0), (7.0, 21.0)], false, stroke);
    outline(p, c, size, &[(7.0, 3.0), (7.0, 7.0), (8.0, 8.0), (15.0, 8.0)], false, stroke);
}

/// lucide's square-arrow-out-up-right.
fn glyph_open(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    outline(p, c, size, &[(21.0, 13.0), (21.0, 19.0), (19.0, 21.0), (5.0, 21.0), (3.0, 19.0), (3.0, 5.0), (5.0, 3.0), (11.0, 3.0)], false, stroke);
    outline(p, c, size, &[(21.0, 3.0), (12.0, 12.0)], false, stroke);
    outline(p, c, size, &[(15.0, 3.0), (21.0, 3.0), (21.0, 9.0)], false, stroke);
}

/// lucide's settings-2: two lines, each with its knob.
fn glyph_settings(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    outline(p, c, size, &[(20.0, 7.0), (11.0, 7.0)], false, stroke);
    outline(p, c, size, &[(14.0, 17.0), (5.0, 17.0)], false, stroke);
    outline(p, c, size, &crate::app::filetree_menu::arc(17.0, 17.0, 3.0, 0.0, 360.0), true, stroke);
    outline(p, c, size, &crate::app::filetree_menu::arc(7.0, 7.0, 3.0, 0.0, 360.0), true, stroke);
}

fn glyph_x(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    outline(p, c, size, &[(18.0, 6.0), (6.0, 18.0)], false, stroke);
    outline(p, c, size, &[(6.0, 6.0), (18.0, 18.0)], false, stroke);
}

/// The pane of a node: the strip of tabs, its save and close buttons, then the file (or why there is none).
#[allow(clippy::too_many_arguments)]
pub(super) fn pane_ui(
    ctx: &Context,
    id: &str,
    frame: Rect,
    tree: Rect,
    pane: Rect,
    z: f32,
    canvas: Rect,
    dark: bool,
    ed: &mut Editor,
    acts: &mut Vec<PaneAct>,
) {
    let c = Palette::new(dark);
    let clip = pane.intersect(canvas);
    let stroke = |color: Color32| Stroke::new((1.0 * z).max(0.75), color);
    // an edit keeps the tab it was made in; the disk's news, once a second
    if ed.active.as_ref().is_some_and(|a| ed.preview.as_ref() == Some(a)) && ed.dirty() {
        ed.preview = None;
    }
    let now = ctx.input(|i| i.time);
    if now - ed.last_check > 1.0 {
        ed.last_check = now;
        for tab in &mut ed.tabs {
            if let Body::Text(t) = &mut tab.body {
                t.check_disk(&tab.path);
            }
        }
    }
    let (dirty, wrap) = (ed.dirty(), ed.wrap);
    let tabs_dirty: Vec<bool> = ed.tabs.iter().map(PaneTab::dirty).collect();
    area(ctx, ("ft_pane", id), pane, clip, |ui| {
        let p = ui.painter().with_clip_rect(clip);
        let strip = Rect::from_min_size(pane.min, Vec2::new(pane.width(), STRIP * z));
        p.rect_filled(strip, 0, c.hover);
        p.hline(pane.left()..=pane.right(), strip.bottom(), Stroke::new(1.0, c.border));
        // the buttons at the right: for text the settings and save, for a picture or PDF the system's
        // program; always close the pane
        let button = 19.5 * z;
        let mut right = pane.right() - 3.25 * z;
        let mut buttons = vec![];
        let shown = ed.active.as_ref().and_then(|a| ed.tabs.iter().find(|t| &t.path == a));
        let media = shown.is_some_and(|t| matches!(t.body, Body::Media(_) | Body::External(_)));
        let tools: &[(&str, char)] = if shown.is_none() {
            &[("Ocultar Editor", 'x')]
        } else if media {
            &[("Ocultar Editor", 'x'), ("Abrir no App Padrão", 'o')]
        } else {
            &[("Ocultar Editor", 'x'), ("Salvar", 's'), ("Configurações do Editor", 'g')]
        };
        for (tip, kind) in tools.iter().copied() {
            let rect = Rect::from_center_size(Pos2::new(right - button / 2.0, strip.center().y), Vec2::splat(button));
            right = rect.left();
            buttons.push((tip, kind, rect));
        }
        let tabs_end = right - 3.25 * z;
        for (tip, kind, rect) in buttons {
            let on = kind != 's' || dirty;
            let r = ui.interact(rect.intersect(clip), ui.scope_id().with(tip), if on { Sense::click() } else { Sense::hover() });
            if r.hovered() && on {
                p.rect_filled(rect, cr(button / 2.0), c.fg.gamma_multiply(0.1));
            }
            let color = if !on {
                c.muted.gamma_multiply(0.4)
            } else if r.hovered() {
                c.fg
            } else {
                c.muted
            };
            match kind {
                'x' => glyph_x(&p, rect.center(), 11.375 * z, stroke(color)),
                's' => glyph_save(&p, rect.center(), 11.375 * z, stroke(color)),
                'o' => glyph_open(&p, rect.center(), 11.375 * z, stroke(color)),
                _ => glyph_settings(&p, rect.center(), 11.375 * z, stroke(color)),
            }
            let r = r.on_hover_cursor(CursorIcon::PointingHand);
            let r = if kind == 'g' && egui::Popup::is_any_open(ui.ctx()) { r } else { r.tip(tip) };
            if kind == 'g' {
                egui::Popup::menu(&r).align(egui::RectAlign::BOTTOM_END).gap(6.0).frame(menu_frame(dark)).show(|ui| {
                    ui.set_width(160.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if menu_row(ui, Row { lead: Lead::Tick(ed.wrap), ..Row::new(None, "Quebrar Linhas") }).clicked() {
                        acts.push(PaneAct::Wrap);
                        ui.close();
                    }
                });
            } else if r.clicked() {
                let path = ed.active.clone().unwrap_or_default();
                acts.push(match kind {
                    'x' => PaneAct::Hide,
                    'o' => PaneAct::OpenDefault(path),
                    _ => PaneAct::Save,
                });
            }
        }
        // the tabs
        let strip_clip = Rect::from_min_max(strip.min, Pos2::new(tabs_end, strip.bottom())).intersect(clip);
        let sp = p.with_clip_rect(strip_clip);
        let mut x = pane.left();
        for (i, tab) in ed.tabs.iter().enumerate() {
            let name = tab.path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
            let font = FontId::proportional(12.0 * z);
            let active = ed.active.as_ref() == Some(&tab.path);
            let italic = ed.preview.as_ref() == Some(&tab.path);
            let color = if active { c.fg } else { c.muted };
            let mut job = egui::text::LayoutJob::simple_singleline(name.clone(), font.clone(), color);
            job.sections[0].format.italics = italic;
            let galley = ui.painter().layout_job(job);
            let width = (21.125 * z + galley.size().x + 10.0 * z).clamp(104.0 * z, 190.0 * z);
            let chip = Rect::from_min_size(Pos2::new(x, strip.top()), Vec2::new(width, strip.height()));
            x += width;
            if chip.left() > strip_clip.right() {
                break;
            }
            let r = ui.interact(chip.intersect(strip_clip), ui.scope_id().with(("tab", i)), Sense::click());
            let slot = Rect::from_center_size(Pos2::new(chip.left() + 3.25 * z + 6.5 * z, chip.center().y), Vec2::splat(13.0 * z));
            let over_x = ui.rect_contains_pointer(slot) && r.hovered();
            if active {
                sp.rect_filled(chip, 0, c.bg);
            } else if r.hovered() {
                sp.rect_filled(chip, 0, c.fg.gamma_multiply(0.07));
            }
            sp.vline(chip.right() - 0.5, chip.y_range(), Stroke::new(1.0, c.border));
            if r.hovered() {
                if over_x {
                    sp.rect_filled(slot, cr(2.0 * z), c.fg.gamma_multiply(0.15));
                }
                glyph_x(&sp, slot.center(), 9.75 * z, stroke(c.muted));
            } else if tabs_dirty[i] {
                sp.circle_filled(slot.center(), 3.5 * z, Color32::from_rgb(0xf0, 0x88, 0x3e));
            }
            let text = fit(&sp, &name, &font, chip.right() - 10.0 * z - (chip.left() + 21.125 * z));
            let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
            job.sections[0].format.italics = italic;
            let galley = sp.layout_job(job);
            sp.galley(Pos2::new(chip.left() + 21.125 * z, chip.center().y - galley.size().y / 2.0), galley, color);
            if r.clicked() {
                acts.push(if over_x { PaneAct::Close(tab.path.clone()) } else { PaneAct::Select(tab.path.clone()) });
            }
            if r.middle_clicked() {
                acts.push(PaneAct::Close(tab.path.clone()));
            }
            let unsaved = if tabs_dirty[i] { ", alterações não salvas" } else { "" };
            let r = r.on_hover_text(format!("{}{unsaved}", tab.path.display()));
            let (path, pinned) = (tab.path.clone(), ed.preview.as_ref() != Some(&tab.path));
            egui::Popup::context_menu(&r).frame(menu_frame(dark)).show(|ui| {
                ui.set_width(190.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                let item = |ui: &mut egui::Ui, label: &str| menu_row(ui, Row::new(None, label)).clicked();
                if !pinned && item(ui, "Manter Aberto") {
                    acts.push(PaneAct::Pin(path.clone()));
                    ui.close();
                }
                if item(ui, "Fechar Aba") {
                    acts.push(PaneAct::Close(path.clone()));
                    ui.close();
                }
                if item(ui, "Fechar Outras Abas") {
                    acts.push(PaneAct::CloseOthers(path.clone()));
                    ui.close();
                }
                if item(ui, "Fechar Todas as Abas") {
                    acts.push(PaneAct::CloseAll);
                    ui.close();
                }
                if item(ui, "Copiar Caminho") {
                    acts.push(PaneAct::CopyPath(path.clone()));
                    ui.close();
                }
                if item(ui, "Mostrar na Árvore") {
                    acts.push(PaneAct::RevealInTree(path.clone()));
                    ui.close();
                }
            });
        }
        // the file
        let body = Rect::from_min_max(Pos2::new(pane.left(), strip.bottom() + 1.0), pane.max);
        let active = ed.active.clone();
        match active.and_then(|a| ed.tabs.iter_mut().find(|t| t.path == a)) {
            None => {
                let galley = p.layout(
                    "Selecione um arquivo para editá-lo ou pré-visualizá-lo aqui.".into(),
                    FontId::proportional(12.0 * z),
                    c.muted,
                    (body.width() - 26.0 * z).max(10.0),
                );
                p.galley(body.center() - galley.size() / 2.0, galley, c.muted);
            }
            Some(tab) => {
                let (act, _) = ui.scope(|ui| body_ui(ui, body, &mut tab.body, &tab.path, z, dark, wrap, &c)).inner;
                match act {
                    BodyAct::Reveal => acts.push(PaneAct::Reveal(tab.path.clone())),
                    BodyAct::OpenDefault => acts.push(PaneAct::OpenDefault(tab.path.clone())),
                    BodyAct::Retry => acts.push(PaneAct::Retry(tab.path.clone())),
                    BodyAct::KeepMine => acts.push(PaneAct::Save),
                    BodyAct::None => {}
                }
            }
        }
        // the seam: dragged, it moves the split
        let grab = Rect::from_min_max(Pos2::new(tree.right() - 3.0 * z, pane.top()), Pos2::new(tree.right() + 6.0 * z, pane.bottom()));
        let r = ui.interact(grab.intersect(clip), ui.scope_id().with("seam"), Sense::drag()).on_hover_cursor(CursorIcon::ResizeHorizontal);
        if r.dragged() {
            if let Some(pos) = r.interact_pointer_pos() {
                acts.push(PaneAct::Seam(((pos.x - frame.left()) / z).round()));
            }
        }
    });
}

impl DrawingApp {
    /// Shows or hides the editor pane of tree `id`: the node grows by the pane to its right, or shrinks back.
    pub(crate) fn ft_show_editor(&mut self, id: &str, show: bool) {
        let Some(element) = self.scene.elements.iter_mut().find(|e| e.id == id) else { return };
        let (Some(spec), ElementKind::Rectangle { width, .. }) = (element.filetree.as_mut(), &mut element.kind) else { return };
        if show && !spec.editor {
            spec.tree_width = Some(*width);
            *width += EDITOR_W + SEAM;
            spec.editor = true;
        } else if !show && spec.editor {
            *width = tree_width(spec, *width).max(CLOSE_MIN);
            spec.editor = false;
        }
        // ponytail: no undo step for the node's width, as the IDE card's panel
    }

    /// Opens `path` in the pane of tree `id` (showing the pane first).
    pub(crate) fn ft_open(&mut self, id: &str, path: PathBuf, pinned: bool) {
        self.ft_show_editor(id, true);
        let ctx = self.filetrees.ctx.clone().unwrap_or_default();
        self.filetrees.editors.entry(id.to_string()).or_insert_with(|| Editor::new(ctx)).open(path, pinned);
    }

    pub(super) fn ft_pane_apply(&mut self, id: &str, act: PaneAct) {
        match act {
            PaneAct::Select(path) => {
                if let Some(ed) = self.filetrees.editors.get_mut(id) {
                    ed.active = Some(path);
                }
            }
            PaneAct::Close(path) => {
                if let Some(ed) = self.filetrees.editors.get_mut(id) {
                    if let Err(why) = ed.close(&path) {
                        self.toast(why);
                    }
                }
            }
            PaneAct::Save => {
                if let Some(Err(why)) = self.filetrees.editors.get_mut(id).map(Editor::save) {
                    self.toast(why);
                }
            }
            PaneAct::Hide => self.ft_show_editor(id, false),
            PaneAct::OpenDefault(path) => crate::app::portal::open_external(&path.to_string_lossy()),
            PaneAct::Retry(path) => {
                let ctx = self.filetrees.ctx.clone().unwrap_or_default();
                if let Some(tab) = self.filetrees.editors.get_mut(id).and_then(|ed| ed.tabs.iter_mut().find(|t| t.path == path)) {
                    tab.body = load(&path, &ctx);
                }
            }
            PaneAct::Pin(path) => {
                if let Some(ed) = self.filetrees.editors.get_mut(id).filter(|ed| ed.preview.as_ref() == Some(&path)) {
                    ed.preview = None;
                }
            }
            PaneAct::CloseOthers(path) => {
                let others: Vec<PathBuf> =
                    self.filetrees.editors.get(id).map(|ed| ed.tabs.iter().map(|t| t.path.clone()).filter(|p| *p != path).collect()).unwrap_or_default();
                for other in others {
                    self.ft_pane_apply(id, PaneAct::Close(other));
                }
            }
            PaneAct::CloseAll => {
                let all: Vec<PathBuf> = self.filetrees.editors.get(id).map(|ed| ed.tabs.iter().map(|t| t.path.clone()).collect()).unwrap_or_default();
                for path in all {
                    self.ft_pane_apply(id, PaneAct::Close(path));
                }
            }
            PaneAct::CopyPath(path) => {
                if let Some(ctx) = self.filetrees.ctx.clone() {
                    ctx.copy_text(path.display().to_string());
                }
                self.toast("Caminho copiado.");
            }
            PaneAct::RevealInTree(path) => self.ft_reveal(id, &path),
            PaneAct::Wrap => {
                if let Some(ed) = self.filetrees.editors.get_mut(id) {
                    ed.wrap = !ed.wrap;
                }
            }
            PaneAct::Seam(width) => {
                if let Some(spec) = self.ft_spec_mut(id) {
                    spec.tree_width = Some(width);
                }
            }
            PaneAct::Reveal(path) => {
                let dir = path.parent().map(Path::to_path_buf).unwrap_or(path);
                crate::app::portal::open_external(&dir.to_string_lossy());
            }
        }
    }
}
