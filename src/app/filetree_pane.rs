//! The editor pane of a file tree node, as the reference app's: opening a file grows the node to the
//! right by a 600 wide pane (a 1 wide seam between), with a strip of tabs over the text. Saving, pictures
//! and PDFs are in filetree_editor.rs, the code as the IDE draws it in filetree_code.rs, the tabs in
//! filetree_tabs.rs, the lists of a declaration's usages and history in filetree_vision.rs and "Localizar
//! nos Arquivos" in filetree_find.rs; the strip's buttons and the seam are drawn here. Nothing in it depends
//! on an IDE node.

use crate::app::filetree::*;
use crate::app::filetree_code::{Click, Found, Project};
use crate::app::filetree_editor::{Body, BodyAct, body_ui, load};
use crate::app::filetree_find::{FindAct, FindFiles, Texts, find_ui};
use crate::app::filetree_menu::{Lead, Row, menu_row};
use crate::app::filetree_tabs::{STRIP, tabs_ui};
use crate::app::filetree_ui::{Palette, area, cr, outline};
use crate::app::filetree_vision::{Commit, ListPopup, Picked, commit_diff, popup_ui};
use crate::app::ui::menu_look::menu_frame;
use crate::prelude::*;
use std::path::Path;

/// The pane's width when it opens, the seam, the tree's and the pane's narrowest and how narrow the
/// node gets when the pane closes (all in the node's own points).
pub(crate) const EDITOR_W: f32 = 600.0;
pub(crate) const SEAM: f32 = 1.0;
const TREE_MIN: f32 = 280.0;
const EDITOR_MIN: f32 = 240.0;
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

    /// Writes its unsaved text. Back: whether a file was written (a text nobody changed is left as it is on disk).
    fn save(&mut self) -> Result<bool, String> {
        match &mut self.body {
            Body::Text(t) if t.dirty => t.save(&self.path).map(|()| true),
            _ => Ok(false),
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
    /// What the code knows of the tree's root: parameter names, usages, declarations (filetree_code.rs).
    pub(crate) code: Project,
    /// The list a declaration's label opened: its usages or its history (filetree_vision.rs).
    pub(crate) popup: Option<ListPopup>,
    /// Where the last Ctrl+click's name is: its usages hang there once the search says it is the declaration.
    pressed: Pos2,
    /// "Localizar nos Arquivos" while it is open, and the root's text it searches (filetree_find.rs).
    pub(crate) find: Option<FindFiles>,
    pub(crate) texts: Texts,
}

impl Editor {
    pub(crate) fn new(ctx: Context) -> Self {
        Editor {
            ctx,
            tabs: vec![],
            active: None,
            preview: None,
            last_check: 0.0,
            wrap: false,
            code: Project::default(),
            popup: None,
            pressed: Pos2::ZERO,
            find: None,
            texts: Texts::default(),
        }
    }

    /// Shows what commit `hash` changed in `file` (`text`, as git wrote it), in a tab of its own, read only.
    pub(crate) fn open_diff(&mut self, file: &Path, hash: &str, text: String) {
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        // the commit before the name: the tab keeps the file's icon
        let path = file.with_file_name(format!("{} {name}", &hash[..hash.len().min(7)]));
        if !self.tabs.iter().any(|t| t.path == path) {
            self.tabs.push(PaneTab { path: path.clone(), body: Body::Diff(text) });
        }
        self.active = Some(path);
    }

    /// The usages of the name declared at `line` and `col` of `path` (an open text): the popup is hung at `anchor`.
    fn list_usages(&mut self, name: &str, path: &Path, line: usize, col: usize, anchor: Pos2, ctx: &Context) {
        let Some(Body::Text(t)) = self.tabs.iter().find(|t| t.path == path).map(|t| &t.body) else { return };
        let decl = t.text.lines().nth(line).unwrap_or_default();
        self.popup = Some(ListPopup::usages(&self.code, name, (path, line, col), decl, t.lang.id, anchor, ctx));
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

    /// Opens `path` for good with the caret at `line` and `col` (where a Ctrl+click's name is declared).
    pub(crate) fn goto(&mut self, path: PathBuf, line: usize, col: usize) {
        self.open(path.clone(), true);
        if let Some(Body::Text(t)) = self.tabs.iter_mut().find(|t| t.path == path).map(|t| &mut t.body) {
            t.jump = Some((line, col));
        }
    }

    /// Writes a tab's unsaved text; the project reads the file's declarations again.
    fn save_tab(&mut self, path: &Path) -> Result<(), String> {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.path == path)
            && tab.save()?
        {
            self.code.saved(path);
        }
        Ok(())
    }

    /// Saves and closes a tab; Err when it stays (its text could not be written).
    pub(crate) fn close(&mut self, path: &Path) -> Result<(), String> {
        self.save_tab(path)?;
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
        self.active.clone().map_or(Ok(()), |active| self.save_tab(&active))
    }

    /// Saves every tab with unsaved text (the app is closing, or the node is going).
    pub(crate) fn save_all(&mut self) {
        for path in self.tabs.iter().map(|t| t.path.clone()).collect::<Vec<_>>() {
            let _ = self.save_tab(&path);
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
    /// A Ctrl+click's declaration, a usage or a match picked: the file, the line and the column.
    Goto(PathBuf, usize, usize),
    /// The name a Ctrl+click found no declaration of.
    Missing(String),
    /// What a commit of a declaration's history changed in the file.
    Diff(PathBuf, Commit),
    /// Ctrl+Shift+F in the code, with the text selected.
    Find(String),
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

pub(super) fn glyph_x(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
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
    // the project's news: its index, who wrote the code forward, where a Ctrl+click's name is declared
    match ed.code.poll() {
        Some(Found::At(path, line, col)) => acts.push(PaneAct::Goto(path, line, col)),
        Some(Found::Nowhere(word)) => acts.push(PaneAct::Missing(word)),
        // the click was on the declaration itself: its usages, as the reference IDE
        Some(Found::Own(name, path, line, col)) => ed.list_usages(&name, &path, line, col, ed.pressed, ctx),
        None => {}
    }
    let is_code = |t: &PaneTab| matches!(&t.body, Body::Text(b) if b.lang.kind == crate::ide::highlight::Kind::Code);
    if let Some(path) = ed.active.clone().filter(|a| ed.tabs.iter().any(|t| &t.path == a && is_code(t))) {
        ed.code.blame(&path, ctx);
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
        let tools: &[(&str, char)] = if shown.is_none_or(|t| matches!(t.body, Body::Diff(_))) {
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
        // the tabs, up to the buttons
        let tabs = Rect::from_min_max(strip.min, Pos2::new(tabs_end, strip.bottom()));
        tabs_ui(ui, ed, tabs, tabs.intersect(clip), z, dark, &c, &tabs_dirty, acts);
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
                let (act, _) = ui.scope(|ui| body_ui(ui, body, &mut tab.body, &tab.path, z, wrap, &c, &ed.code)).inner;
                match act {
                    BodyAct::Code(click) => {
                        if let Body::Text(t) = &tab.body {
                            let label = |i: usize| t.laid.as_ref().and_then(|l| l.labels.get(i));
                            match click {
                                Click::Name(at, anchor) => {
                                    ed.pressed = anchor;
                                    // a framework's string answers now, a name once the project is searched (`poll`)
                                    if let Some(Found::At(path, line, col)) = ed.code.declaration(&tab.path, t.lang, &t.text, at, ctx) {
                                        acts.push(PaneAct::Goto(path, line, col));
                                    }
                                }
                                Click::Usages(i, anchor) => {
                                    if let Some(l) = label(i) {
                                        let decl = t.text.lines().nth(l.line).unwrap_or_default();
                                        ed.popup = Some(ListPopup::usages(&ed.code, &l.name, (&tab.path, l.line, l.col), decl, t.lang.id, anchor, ctx));
                                    }
                                }
                                Click::Author(i, anchor) => {
                                    if let Some(l) = label(i) {
                                        ed.popup = Some(ListPopup::history(&tab.path, &l.name, l.span, anchor, ctx));
                                    }
                                }
                                Click::Find(query) => acts.push(PaneAct::Find(query)),
                            }
                        }
                    }
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
    // over everything: the list a declaration's label opened, and "Localizar nos Arquivos"
    match popup_ui(ctx, id, &mut ed.popup) {
        Some(Picked::Usage(path, line, col)) => acts.push(PaneAct::Goto(path, line, col)),
        Some(Picked::Commit(file, commit)) => acts.push(PaneAct::Diff(file, commit)),
        None => {}
    }
    if let Some(find) = &mut ed.find {
        match find_ui(ctx, id, ed.code.root(), canvas, find, &mut ed.texts) {
            FindAct::Open(path, line, col) => {
                ed.find = None;
                acts.push(PaneAct::Goto(path, line, col));
            }
            FindAct::Close => ed.find = None,
            FindAct::None => {}
        }
    }
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

    /// "Localizar nos Arquivos" (Ctrl+Shift+F) over the root of tree `id`, with `query` typed in; its "Diretório" scope
    /// is the folder selected in the tree (or the selected file's), else the one shown. The pane shows, for the matches.
    pub(crate) fn ft_find_open(&mut self, id: &str, query: String) {
        let Some(spec) = self.ft_spec(id).cloned() else { return };
        self.ft_show_editor(id, true);
        let ctx = self.filetrees.ctx.clone().unwrap_or_default();
        let selected = self.filetrees.views.get(id).and_then(|v| v.selected.clone());
        let folder = match selected {
            Some(path) if path.is_dir() => path,
            Some(path) => path.parent().map_or_else(|| spec.shown(), Path::to_path_buf),
            None => spec.shown(),
        };
        let ed = self.ft_editor(id);
        ed.texts.refresh(&spec.root_path(), &ctx);
        (ed.popup, ed.find) = (None, Some(FindFiles::new(query, folder)));
    }

    /// The editor of tree `id` (made now if it never opened), its code read against the tree's root: its project.
    pub(crate) fn ft_editor(&mut self, id: &str) -> &mut Editor {
        let ctx = self.filetrees.ctx.clone().unwrap_or_default();
        let root = self.ft_spec(id).map(FileTreeSpec::root_path).unwrap_or_default();
        let ed = self.filetrees.editors.entry(id.to_string()).or_insert_with(|| Editor::new(ctx.clone()));
        ed.code.open(root, &ctx);
        ed
    }

    /// Opens `path` in the pane of tree `id` (showing the pane first).
    pub(crate) fn ft_open(&mut self, id: &str, path: PathBuf, pinned: bool) {
        self.ft_show_editor(id, true);
        self.ft_editor(id).open(path, pinned);
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
            PaneAct::Goto(path, line, col) => {
                self.ft_show_editor(id, true);
                self.ft_editor(id).goto(path, line, col);
            }
            PaneAct::Missing(word) => self.toast(format!("Declaração de '{word}' não encontrada")),
            PaneAct::Diff(file, commit) => match commit_diff(&file, &commit.hash) {
                Ok(text) => {
                    if let Some(ed) = self.filetrees.editors.get_mut(id) {
                        ed.open_diff(&file, &commit.hash, text);
                    }
                }
                Err(why) => self.toast(why),
            },
            PaneAct::Find(query) => self.ft_find_open(id, query),
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
