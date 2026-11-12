//! The terminal card's options: the right-click menu in the reference app's order (Copiar, Colar,
//! Selecionar Tudo | Renomear, Editar…, Monitorar Atividade | Desfocar, Recarregar, Descarregar,
//! Padrões para Novos Elementos | Copiar Elemento, Duplicar, Bloquear | Apagar) and what each row
//! does. What the app already did another way (rename, edit, reload, duplicate, delete…) is called
//! from here; the rest (selecting, copying, pasting, the blur, the defaults) lives in this file.

use crate::app::terminal::find::TermFind;
use crate::app::terminal::procs::TermProcs;
use crate::app::ui::context::{Entry, entry, item, sub, tick};
use crate::prelude::*;

/// A terminal's smallest and largest size, in world units (the reference app's range).
const SIZE_RANGE: ([f32; 2], [f32; 2]) = ([200.0, 120.0], [1600.0, 1200.0]);

/// What the terminal cards keep between frames, outside the scene.
#[derive(Default)]
pub(crate) struct TermUi {
    /// The terminal whose grid a press is selecting in.
    pub(crate) selecting: Option<String>,
    /// The search bar over a terminal.
    pub(crate) find: Option<TermFind>,
    /// The popover listing a terminal's processes.
    pub(crate) procs: Option<TermProcs>,
    /// Text for the system clipboard, queued by an action that has no `Context` (the MCP's).
    pub(crate) copy_out: Option<String>,
}

/// `size` as a new terminal would take it: on the grid, inside the range.
pub(crate) fn adopted_size(size: Vec2) -> [f32; 2] {
    let snap = |v: f32, lo: f32, hi: f32| ((v / GRID).round() * GRID).clamp(lo, hi);
    [snap(size.x, SIZE_RANGE.0[0], SIZE_RANGE.1[0]), snap(size.y, SIZE_RANGE.0[1], SIZE_RANGE.1[1])]
}

impl DrawingApp {
    /// The colors and the font size (pt) a terminal is painted with: its own over the defaults.
    pub(crate) fn terminal_look(&self, spec: &TerminalSpec) -> (Look, f32) {
        let (theme, font, _) = self.prefs.terminal.resolve(&spec.look);
        (Look::of(theme, self.dark()), font)
    }

    /// What a new terminal measures: the size picked with "Usar Este Tamanho", else 80 by 24 cells.
    pub(crate) fn new_terminal_size(&self) -> Vec2 {
        self.prefs.terminal.size.map_or_else(|| terminal_size(80, 24), |[w, h]| Vec2::new(w, h))
    }

    pub(crate) fn terminal_index(&self, id: &str) -> Option<usize> {
        self.scene.elements.iter().position(|e| e.id == id && e.terminal.is_some())
    }

    fn live_handle(&self, id: &str) -> Option<&TermHandle> {
        match self.terminals.get(id) {
            Some(Ok(session)) => Some(&session.handle),
            _ => None,
        }
    }

    /// The rows of a terminal's menu, in the reference app's order. "Padrões para Novos Elementos"
    /// rows are off while the terminal already is what the default says.
    pub(crate) fn terminal_entries(&self, i: usize) -> Vec<Entry> {
        let e = &self.scene.elements[i];
        let spec = e.terminal.clone().unwrap_or_default();
        let has_selection = self.live_handle(&e.id).is_some_and(TermHandle::has_selection);
        let (theme, font, _) = self.prefs.terminal.resolve(&spec.look);
        let same_size = adopted_size(local_bounds(e).size()) == adopted_size(self.new_terminal_size());
        let same_font = font == self.prefs.terminal.font_size;
        let same_theme = theme == self.prefs.terminal.theme;
        let mut rows = vec![
            entry("copy_sel", "Copiar", None, false, has_selection),
            item("paste_text", "Colar"),
            item("select_all_text", "Selecionar Tudo"),
            Entry::Sep,
            item("rename", "Renomear"),
            item("edit", "Editar…"),
            tick("monitor", "Monitorar Atividade", !spec.quiet),
            Entry::Sep,
            if spec.look.blurred { item("unblur", "Remover Desfoque") } else { item("blur", "Desfocar") },
            item("reload", "Recarregar"),
            entry("unload", "Descarregar", None, false, !spec.unloaded),
            sub(
                "Padrões para Novos Elementos",
                vec![
                    entry("use_size", "Usar Este Tamanho", None, false, !same_size),
                    entry("use_font", "Usar Esta Fonte", None, false, !same_font),
                    entry("use_theme", "Usar Este Tema", None, false, !same_theme),
                ],
            ),
            Entry::Sep,
        ];
        // the nodes it is joined to, one row each to cut it (ui/links.rs)
        let joined = self.links().remove(&e.id).unwrap_or_default();
        if !joined.is_empty() {
            let nodes = self.agent_nodes();
            let cuts = joined
                .iter()
                .map(|other| item(&format!("disconnect:{other}"), &nodes.iter().find(|n| &n.id == other).map(|n| n.name.clone()).unwrap_or_default()))
                .collect();
            rows.extend([sub("Desconectar", cuts), Entry::Sep]);
        }
        rows.extend([
            item("copy_el", "Copiar Elemento"),
            entry("dup", "Duplicar", None, false, !e.locked),
            item("lock", if e.locked { "Desbloquear" } else { "Bloquear" }),
            Entry::Sep,
            item("delete", "Apagar"),
        ]);
        rows
    }

    /// Runs the rows only a terminal has; false for the ones the menu already handled another way.
    pub(crate) fn terminal_menu_action(&mut self, ctx: &Context, index: Option<usize>, id: &str) -> bool {
        let Some(i) = index.filter(|i| self.scene.elements[*i].terminal.is_some()) else { return false };
        let tid = self.scene.elements[i].id.clone();
        match id {
            "copy_sel" => {
                if let Some(text) = self.live_handle(&tid).map(TermHandle::selected_text).filter(|t| !t.is_empty()) {
                    ctx.copy_text(text);
                }
            }
            "paste_text" => {
                // the keys go to it from now on; the system's clipboard answers with a Paste event
                self.terminal_focus = Some(tid);
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            "select_all_text" => {
                self.terminal_focus = Some(tid.clone());
                if let Some(h) = self.live_handle(&tid) {
                    h.select_all();
                }
            }
            "blur" | "unblur" => self.set_blurred(i, id == "blur"),
            "use_size" => self.prefs.terminal.size = Some(adopted_size(local_bounds(&self.scene.elements[i]).size())),
            "use_font" => self.use_font_of(i),
            "use_theme" => self.use_theme_of(i),
            _ => return false,
        }
        true
    }

    /// Desfocar (`on`) or Remover Desfoque: its screen is hidden behind a veil, as one undo step.
    pub(crate) fn set_blurred(&mut self, i: usize, on: bool) {
        if self.scene.elements[i].terminal.as_ref().is_none_or(|t| t.look.blurred == on) {
            return;
        }
        self.snapshot();
        if let Some(t) = self.scene.elements[i].terminal.as_mut() {
            t.look.blurred = on;
        }
        if on && self.terminal_focus.as_ref() == Some(&self.scene.elements[i].id) {
            self.terminal_focus = None;
        }
    }

    /// "Usar Esta Fonte": the terminal's font size becomes the default's.
    pub(crate) fn use_font_of(&mut self, i: usize) {
        let spec = self.scene.elements[i].terminal.clone().unwrap_or_default();
        self.prefs.terminal.font_size = self.prefs.terminal.resolve(&spec.look).1;
    }

    /// "Usar Este Tema": the terminal's theme becomes the default's.
    pub(crate) fn use_theme_of(&mut self, i: usize) {
        let spec = self.scene.elements[i].terminal.clone().unwrap_or_default();
        self.prefs.terminal.theme = self.prefs.terminal.resolve(&spec.look).0.to_string();
    }

    /// Selecting and copying in a terminal's grid: the card's own presses and drags (`origin` is
    /// the grid's top-left on screen, `cell` one cell's size). A press selects characters, a double
    /// click a word, a triple click a line; a click elsewhere clears it.
    pub(crate) fn terminal_pointer(&mut self, ctx: &Context, id: &str, origin: Pos2, cell: Vec2, r: &PointerGesture) {
        let Some(h) = self.live_handle(id).cloned() else { return };
        let at = |pos: Pos2| cell_at(pos, origin, cell);
        let on_grid = |pos: Pos2| pos.y >= origin.y - 4.0;
        if r.drag_started && r.pos.is_some_and(on_grid) {
            h.begin_selection(at(r.pos.unwrap_or(origin)), Pick::Chars);
            self.term_ui.selecting = Some(id.to_string());
        }
        if self.term_ui.selecting.as_deref() == Some(id) {
            if r.dragging
                && let Some(pos) = r.pos
            {
                h.extend_selection(at(pos));
                ctx.request_repaint();
            }
            if r.drag_stopped {
                self.term_ui.selecting = None;
                if self.prefs.terminal.copy_on_select && h.has_selection() {
                    ctx.copy_text(h.selected_text());
                }
            }
        }
        if r.clicked && r.pos.is_some_and(on_grid) {
            h.clear_selection();
            let pick = if r.triple {
                Some(Pick::Line)
            } else if r.double {
                Some(Pick::Word)
            } else {
                None
            };
            if let (Some(pick), Some(pos)) = (pick, r.pos) {
                h.begin_selection(at(pos), pick);
                if self.prefs.terminal.copy_on_select {
                    ctx.copy_text(h.selected_text());
                }
            }
        }
    }

    /// Sends the system clipboard's text (or an image, as the path of a png in the temp folder)
    /// that arrived as a paste event, and Ctrl+Shift+C: the selection to the clipboard. Returns the
    /// bytes to write to the shell.
    pub(crate) fn pasted_bytes(&self, id: &str, event: &egui::Event) -> Option<Vec<u8>> {
        let bracketed = self.live_handle(id)?.mode().contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE);
        match event {
            egui::Event::Paste(text) => Some(paste_bytes(text, bracketed)),
            egui::Event::PasteImage(img) => {
                let path = crate::app::canvas::pointer::save_pasted(img, &std::env::temp_dir())?;
                Some(paste_bytes(&format!("{} ", shell_quote(&path.to_string_lossy())), bracketed))
            }
            _ => None,
        }
    }

    /// Empties what an action without a `Context` queued for the clipboard.
    pub(crate) fn flush_copy_out(&mut self, ctx: &Context) {
        if let Some(text) = self.term_ui.copy_out.take() {
            ctx.copy_text(text);
        }
    }
}

/// What a terminal card's pointer did this frame (see `terminal_pointer`).
#[derive(Clone, Copy, Default)]
pub(crate) struct PointerGesture {
    pub(crate) pos: Option<Pos2>,
    pub(crate) clicked: bool,
    pub(crate) double: bool,
    pub(crate) triple: bool,
    pub(crate) drag_started: bool,
    pub(crate) dragging: bool,
    pub(crate) drag_stopped: bool,
}

/// A path as a shell takes it: in quotes when it has spaces or the like.
pub(crate) fn shell_quote(path: &str) -> String {
    if !path.chars().any(|c| c.is_whitespace() || "'\"$&();<>|`\\!*?[]{}#~".contains(c)) {
        return path.to_string();
    }
    if cfg!(windows) { format!("\"{}\"", path.replace('"', "")) } else { format!("'{}'", path.replace('\'', "'\\''")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The labels of a menu in order, a separator as "-" and a submenu as "label >".
    fn outline(rows: &[Entry]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                Entry::Sep => "-".to_string(),
                Entry::Item { label, check, .. } => format!("{}{label}", if check.is_some() { "[] " } else { "" }),
                Entry::Sub { label, .. } => format!("{label} >"),
            })
            .collect()
    }

    fn app_with_terminal() -> (DrawingApp, usize) {
        let mut app = DrawingApp::default();
        let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 640.0, height: 400.0 });
        e.terminal = Some(TerminalSpec { name: "Um".into(), ..Default::default() });
        app.scene.elements.push(e);
        (app, 0)
    }

    #[test]
    fn the_menu_lists_the_reference_apps_items_in_its_order() {
        let (app, i) = app_with_terminal();
        let rows = app.terminal_entries(i);
        assert_eq!(
            outline(&rows),
            [
                "Copiar",
                "Colar",
                "Selecionar Tudo",
                "-",
                "Renomear",
                "Editar…",
                "[] Monitorar Atividade",
                "-",
                "Desfocar",
                "Recarregar",
                "Descarregar",
                "Padrões para Novos Elementos >",
                "-",
                "Copiar Elemento",
                "Duplicar",
                "Bloquear",
                "-",
                "Apagar",
            ]
        );
        let Entry::Sub { items, .. } = &rows[11] else { panic!("the defaults are a submenu") };
        assert_eq!(outline(items), ["Usar Este Tamanho", "Usar Esta Fonte", "Usar Este Tema"]);
    }

    #[test]
    fn what_the_terminal_is_changes_its_rows() {
        let (mut app, i) = app_with_terminal();
        let enabled = |rows: &[Entry], want: &str| rows.iter().any(|r| matches!(r, Entry::Item { id, enabled: true, .. } if id == want));
        // no selection: nothing to copy; a fresh one has the defaults' font and theme, not their size
        let rows = app.terminal_entries(i);
        assert!(!enabled(&rows, "copy_sel") && enabled(&rows, "paste_text"));
        let Entry::Sub { items, .. } = &rows[11] else { panic!() };
        assert!(enabled(items, "use_size") && !enabled(items, "use_font") && !enabled(items, "use_theme"));
        // its own font and theme can be made the defaults; a blurred one offers to clear it
        let t = app.scene.elements[i].terminal.as_mut().unwrap();
        t.look = TermLook { theme: Some("nord".into()), font_size: Some(18.0), shell: String::new(), blurred: true };
        t.unloaded = true;
        t.quiet = true;
        app.scene.elements[i].locked = true;
        let rows = app.terminal_entries(i);
        let outline = outline(&rows);
        assert!(outline.contains(&"Remover Desfoque".to_string()) && outline.contains(&"Desbloquear".to_string()));
        assert!(!enabled(&rows, "unload") && !enabled(&rows, "dup"));
        let Entry::Sub { items, .. } = &rows[11] else { panic!() };
        assert!(enabled(items, "use_font") && enabled(items, "use_theme"));
        // and adopting them changes the defaults, after which those rows go off
        let ctx = Context::default();
        assert!(app.terminal_menu_action(&ctx, Some(i), "use_font") && app.terminal_menu_action(&ctx, Some(i), "use_theme"));
        assert!(app.terminal_menu_action(&ctx, Some(i), "use_size"));
        assert_eq!((app.prefs.terminal.font_size, app.prefs.terminal.theme.as_str()), (18.0, "nord"));
        assert_eq!(app.prefs.terminal.size, Some([640.0, 400.0]));
        assert_eq!(app.new_terminal_size(), Vec2::new(640.0, 400.0));
        let Entry::Sub { items, .. } = &app.terminal_entries(i)[11] else { panic!() };
        assert!(!enabled(items, "use_size") && !enabled(items, "use_font") && !enabled(items, "use_theme"));
        // rows the app already knew are left to it
        assert!(!app.terminal_menu_action(&ctx, Some(i), "rename"));
    }

    #[test]
    fn blurring_is_one_undo_step_and_takes_the_keys_away() {
        let (mut app, i) = app_with_terminal();
        app.terminal_focus = Some(app.scene.elements[i].id.clone());
        app.set_blurred(i, true);
        assert!(app.scene.elements[i].terminal.as_ref().unwrap().look.blurred && app.terminal_focus.is_none());
        app.undo();
        assert!(!app.scene.elements[i].terminal.as_ref().unwrap().look.blurred);
    }

    #[test]
    fn a_size_is_adopted_on_the_grid_inside_the_range() {
        assert_eq!(adopted_size(Vec2::new(641.0, 399.0)), [640.0, 400.0]);
        assert_eq!(adopted_size(Vec2::new(10.0, 5000.0)), [200.0, 1200.0]);
    }

    #[test]
    fn paths_pasted_to_a_shell_are_quoted_only_when_they_must() {
        assert_eq!(shell_quote("/tmp/a.png"), "/tmp/a.png");
        assert!(shell_quote("/tmp/my pics/a.png").contains("my pics") && shell_quote("/tmp/my pics/a.png").starts_with(['\'', '"']));
    }

    #[test]
    fn a_drag_selects_in_the_grid_and_a_click_clears_it() {
        let (mut app, i) = app_with_terminal();
        let id = app.scene.elements[i].id.clone();
        let (program, args): (&str, Vec<&str>) =
            if cfg!(windows) { ("cmd", vec!["/c", "echo", "pointer-test"]) } else { ("sh", vec!["-c", "echo pointer-test"]) };
        let session = TerminalSession::spawn("", "", (40, 6), None, vec![], Some((program, &args)), 100).unwrap();
        for _ in 0..100 {
            if session.handle.screen_text().contains("pointer-test") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let text = session.handle.screen_text();
        let row = text.lines().position(|l| l.contains("pointer-test")).unwrap() as f32;
        let col = text.lines().nth(row as usize).unwrap().find("pointer-test").unwrap() as f32;
        app.terminals.insert(id.clone(), Ok(session));
        let (origin, cell) = (Pos2::new(100.0, 100.0), Vec2::new(10.0, 20.0));
        let ctx = Context::default();
        let at = |dx: f32| Some(origin + Vec2::new((col + dx) * 10.0 + 1.0, row * 20.0 + 5.0));
        app.terminal_pointer(&ctx, &id, origin, cell, &PointerGesture { pos: at(0.0), drag_started: true, ..Default::default() });
        app.terminal_pointer(&ctx, &id, origin, cell, &PointerGesture { pos: at(6.6), dragging: true, ..Default::default() });
        app.terminal_pointer(&ctx, &id, origin, cell, &PointerGesture { pos: at(6.6), drag_stopped: true, ..Default::default() });
        assert_eq!(app.live_handle(&id).unwrap().selected_text(), "pointer");
        app.terminal_pointer(&ctx, &id, origin, cell, &PointerGesture { pos: at(8.0), clicked: true, double: true, ..Default::default() });
        assert_eq!(app.live_handle(&id).unwrap().selected_text(), "pointer-test");
        app.terminal_pointer(&ctx, &id, origin, cell, &PointerGesture { pos: at(1.0), clicked: true, ..Default::default() });
        assert!(!app.live_handle(&id).unwrap().has_selection());
    }
}
