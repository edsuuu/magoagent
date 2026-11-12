//! Right-click menus: the canvas, a terminal, a note, a portal, an IDE and several selected
//! elements. They share the reference app's native dark look: a box, 24pt rows, a blue highlight
//! and ">" submenus that open beside their row and stay open while the pointer goes to them. Also
//! the Renomear dialog they open.

use crate::app::browser::extras::{SizeReadout, StorageScope};
use crate::app::notes::rename_note_text;
use crate::app::ui::menu_look::*;
use crate::ide::{FILE_MENU, FileAction};
use crate::prelude::*;
use std::path::Path;

/// The alignments, in the order of the Alinhar submenu (the index is the id's `align:N`).
const ALIGNS: [&str; 8] = [
    "Alinhar à esquerda",
    "Centralizar horizontalmente",
    "Alinhar à direita",
    "Alinhar ao topo",
    "Centralizar verticalmente",
    "Alinhar embaixo",
    "Distribuir horizontalmente",
    "Distribuir verticalmente",
];

/// What a menu is about: the canvas point it was opened at, or an element by id.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    Canvas(Pos2),
    Element(String),
    /// A canvas group's pill, by group id.
    Group(String),
}

/// A row of a menu: an action (its `id` goes to `context_action`), a submenu, or a separator.
#[derive(Clone, Debug)]
pub(crate) enum Entry {
    /// `keys`: the shortcut shown in gray at the right ("⌘K"), or empty.
    Item {
        id: String,
        label: String,
        check: Option<bool>,
        // the reference app's menus don't tint a destructive row
        #[allow(dead_code)]
        danger: bool,
        enabled: bool,
        keys: &'static str,
    },
    Sub {
        label: String,
        items: Vec<Entry>,
    },
    Sep,
}

pub(crate) fn entry(id: &str, label: &str, check: Option<bool>, danger: bool, enabled: bool) -> Entry {
    Entry::Item { id: id.into(), label: label.into(), check, danger, enabled, keys: "" }
}

/// An IDE card's tree row's file menu, for `path` (`root`: the project's own folder): each row's id
/// is `file:<n>:<path>`, `n` its place in FILE_MENU.
fn file_entries(path: &Path, root: bool) -> Vec<Entry> {
    let (mut rows, mut k) = (Vec::new(), 0);
    for group in FILE_MENU {
        for &(action, label) in group {
            rows.push(entry(&format!("file:{k}:{}", path.display()), label, None, action == FileAction::Delete, !root || action.on_root()));
            k += 1;
        }
        rows.push(Entry::Sep);
    }
    rows
}

/// A row with its shortcut at the right, as the macOS menus show it.
fn keyed(id: &str, label: &str, keys: &'static str) -> Entry {
    Entry::Item { id: id.into(), label: label.into(), check: None, danger: false, enabled: true, keys }
}

pub(crate) fn item(id: &str, label: &str) -> Entry {
    entry(id, label, None, false, true)
}

pub(crate) fn tick(id: &str, label: &str, on: bool) -> Entry {
    entry(id, label, Some(on), false, true)
}

pub(crate) fn red(id: &str, label: &str) -> Entry {
    entry(id, label, None, true, true)
}

pub(crate) fn sub(label: &str, items: Vec<Entry>) -> Entry {
    Entry::Sub { label: label.into(), items }
}

/// The menu open now: its target, its rows when they are not the target's own (the note bar's
/// popovers), the submenu open at each depth, and the panels drawn last frame.
pub(crate) struct ContextMenu {
    pub(crate) at: Pos2,
    pub(crate) target: Target,
    pub(crate) items: Option<Vec<Entry>>,
    path: Vec<usize>,
    rects: Vec<Rect>,
}

impl ContextMenu {
    pub(crate) fn new(at: Pos2, target: Target, items: Option<Vec<Entry>>) -> Self {
        Self { at, target, items, path: Vec::new(), rects: Vec::new() }
    }
}

/// One panel at `at` and, when a row of it is open (`path[depth]`), its submenu beside it, and
/// so on. Returns the id of the row picked. Drawn as the reference app's context menu (menu_look.rs):
/// as wide as its widest row, rows 26 high, an icon where the row has one, a tick where it is on.
fn panel(ctx: &Context, name: &str, at: Pos2, entries: &[Entry], path: &mut Vec<usize>, depth: usize, rects: &mut Vec<Rect>) -> Option<String> {
    let colors = menu_colors(ctx.global_style().visuals.dark_mode);
    let font = FontId::proportional(MENU_TEXT_SIZE);
    let has_ticks = entries.iter().any(|e| matches!(e, Entry::Item { check: Some(_), .. }));
    let mut picked = None;
    let mut subs: Vec<(usize, Rect)> = Vec::new();
    let area = egui::Area::new(egui::Id::unique((name, depth))).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
        // the icon column is there when any row has an icon or a tick: the labels line up after it
        // one icon column for the whole menu: a row without an icon still lines its label up with the others
        let column = has_ticks
            || entries.iter().any(|e| match e {
                Entry::Item { id, label, .. } => menu_icon(id, label).is_some(),
                Entry::Sub { label, .. } => menu_icon("", label).is_some(),
                Entry::Sep => false,
            });
        let lead = |_: &str, _: &str| column;
        let galley = |ui: &egui::Ui, text: &str| ui.painter().layout_no_wrap(text.into(), font.clone(), colors.text);
        let mut inner = if depth == 0 { MENU_MIN } else { SUB_MIN } - 2.0 * MENU_PAD - 2.0;
        for entry in entries {
            let width = match entry {
                Entry::Sep => 0.0,
                Entry::Item { id, label, keys, .. } => {
                    let lead = if lead(id, label) { ICON_SIZE + ROW_GAP } else { 0.0 };
                    let keys = if keys.is_empty() { 0.0 } else { ROW_GAP + galley(ui, &keys_label(keys)).size().x };
                    2.0 * ROW_PAD + lead + galley(ui, label).size().x + keys
                }
                Entry::Sub { label, .. } => {
                    let lead = if lead("", label) { ICON_SIZE + ROW_GAP } else { 0.0 };
                    2.0 * ROW_PAD + lead + galley(ui, label).size().x + ROW_GAP + ICON_SIZE
                }
            };
            inner = inner.max(width.ceil());
        }
        menu_frame(ctx.global_style().visuals.dark_mode).show(ui, |ui| {
            ui.set_width(inner);
            ui.spacing_mut().item_spacing.y = 0.0;
            for (k, row_entry) in entries.iter().enumerate() {
                match row_entry {
                    Entry::Sep => {
                        // -mx-1 my-1 h-px bg-border: the hairline runs under the padding to the box's edges
                        let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), SEP_HEIGHT), Sense::hover());
                        let y = line.center().y.round();
                        ui.painter().hline(line.left() - MENU_PAD..=line.right() + MENU_PAD, y, Stroke::new(1.0, colors.border));
                    }
                    Entry::Item { id, label, check, enabled, keys, .. } => {
                        let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::click());
                        let lit = response.hovered() && *enabled;
                        if lit {
                            path.truncate(depth);
                        }
                        // disabled: half the opacity, no hover
                        let fade = |c: Color32| if *enabled { c } else { c.gamma_multiply(0.5) };
                        let p = ui.painter();
                        if lit {
                            p.rect_filled(row, ITEM_RADIUS, colors.hover);
                        }
                        let ink = fade(colors.text);
                        let icon_at = Pos2::new(row.left() + ROW_PAD + ICON_SIZE / 2.0, row.center().y);
                        if *check == Some(true) {
                            paint_menu_icon(p, MenuIcon::Check, icon_at, ink);
                        } else if let Some(icon) = menu_icon(id, label) {
                            paint_menu_icon(p, icon, icon_at, fade(if lit { colors.text } else { colors.muted }));
                        }
                        let label_x = row.left() + ROW_PAD + if lead(id, label) { ICON_SIZE + ROW_GAP } else { 0.0 };
                        p.text(Pos2::new(label_x, row.center().y), egui::Align2::LEFT_CENTER, label, font.clone(), ink);
                        if !keys.is_empty() {
                            p.text(
                                Pos2::new(row.right() - ROW_PAD, row.center().y),
                                egui::Align2::RIGHT_CENTER,
                                keys_label(keys),
                                font.clone(),
                                fade(colors.muted),
                            );
                        }
                        if *enabled && response.clicked() {
                            picked = Some(id.clone());
                        }
                        if *enabled {
                            response.on_hover_cursor(CursorIcon::PointingHand);
                        }
                    }
                    Entry::Sub { label, .. } => {
                        let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::click());
                        // hovering the row opens its submenu; it stays open while the pointer is in it
                        if response.hovered() {
                            path.truncate(depth);
                            path.push(k);
                        }
                        let lit = path.get(depth) == Some(&k);
                        let p = ui.painter();
                        if lit {
                            p.rect_filled(row, ITEM_RADIUS, colors.hover);
                        }
                        let ink = if lit { colors.text } else { colors.muted };
                        if let Some(icon) = menu_icon("", label) {
                            paint_menu_icon(p, icon, Pos2::new(row.left() + ROW_PAD + ICON_SIZE / 2.0, row.center().y), ink);
                        }
                        let label_x = row.left() + ROW_PAD + if lead("", label) { ICON_SIZE + ROW_GAP } else { 0.0 };
                        p.text(Pos2::new(label_x, row.center().y), egui::Align2::LEFT_CENTER, label, font.clone(), colors.text);
                        paint_menu_icon(p, MenuIcon::Chevron, Pos2::new(row.right() - ROW_PAD - ICON_SIZE / 2.0, row.center().y), ink);
                        subs.push((k, row));
                    }
                }
            }
        });
    });
    rects.push(area.response.rect);
    if picked.is_some() {
        return picked;
    }
    let open = subs.iter().copied().find(|(k, _)| path.get(depth) == Some(k))?;
    let Entry::Sub { items, .. } = &entries[open.0] else { return None };
    // beside its row, its first row level with it (the border and the padding above it are 4.25)
    panel(ctx, name, Pos2::new(open.1.right() + MENU_PAD, open.1.top() - MENU_PAD - 1.0), items, path, depth + 1, rects)
}

impl DrawingApp {
    /// A right click at `pos` (world `world`) opens the menu for what is under it. Its element
    /// joins the selection unless it is in it already; on empty canvas the selection goes.
    pub(crate) fn open_context(&mut self, pos: Pos2, world: Pos2) {
        if let Some(gid) = self.group_pill_at(pos) {
            self.context = Some(ContextMenu::new(pos, Target::Group(gid), None));
            return;
        }
        // a rope's right click is its connections popover (ui/links.rs)
        if self.hit(world).is_some_and(|i| self.scene.elements[i].rope) {
            self.open_rope_popover(pos, world);
            return;
        }
        let mut items = None;
        let target = match self.hit(world) {
            Some(i) => {
                if !self.selected.contains(&i) {
                    self.selected = self.group_members(i).into_iter().filter(|j| !self.is_card_text(*j)).collect();
                }
                let target = Target::Element(self.scene.elements[i].id.clone());
                // on an IDE card's tree: the row's file comes first, then the card's own rows
                if let Some((path, root)) = self.card_path_at(i, world) {
                    items = Some(file_entries(&path, root).into_iter().chain(self.context_entries(&target)).collect());
                }
                target
            }
            None => {
                self.selected.clear();
                Target::Canvas(world)
            }
        };
        self.context = Some(ContextMenu::new(pos, target, items));
    }

    /// Draws the open menu, runs the row picked in it, and closes it on Esc or on a press outside
    /// its panels (the press that opened it has no panel yet).
    pub(crate) fn context_ui(&mut self, ctx: &Context) {
        let Some(mut menu) = self.context.take() else { return };
        // no tips over an open menu (Tip reads this). The frame is read first: asking the context
        // for it inside `data_mut` takes its lock twice and the app stops for good
        let frame = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("menu_frame"), frame));
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !menu.rects.iter().any(|r| r.contains(p))));
        if ctx.input(|i| i.key_pressed(Key::Escape)) || (outside && !menu.rects.is_empty()) {
            return;
        }
        let entries = menu.items.clone().unwrap_or_else(|| self.context_entries(&menu.target));
        let mut rects = Vec::new();
        match panel(ctx, "context_menu", menu.at, &entries, &mut menu.path, 0, &mut rects) {
            Some(id) => {
                let target = menu.target.clone();
                self.context_action(ctx, &target, &id);
            }
            None => {
                menu.rects = rects;
                self.context = Some(menu);
            }
        }
    }

    /// The rows for a target: the canvas's, or the element's (by its kind), or the selection's.
    pub(crate) fn context_entries(&self, target: &Target) -> Vec<Entry> {
        match target {
            Target::Canvas(_) => {
                let presets: Vec<Entry> =
                    self.prefs.presets.iter().enumerate().filter(|(_, p)| p.on).take(10).map(|(k, p)| item(&format!("preset:{k}"), &p.name)).collect();
                // as the reference app: a plain row without presets, a submenu with "Novo Terminal…" above them
                let terminal = if presets.is_empty() {
                    item("add_terminal", "Terminal")
                } else {
                    sub("Terminal", [vec![item("add_terminal", "Novo Terminal…"), Entry::Sep], presets].concat())
                };
                vec![
                    sub(
                        "Adicionar",
                        vec![
                            terminal,
                            self.chat_add_entry(),
                            item("add_note", "Nota"),
                            item("add_file", "Anexar"),
                            item("add_filetree", "Árvore de Arquivos"),
                            item("add_ide", "IDE"),
                            item("add_portal", "Navegador"),
                            item("add_text", "Texto"),
                        ],
                    ),
                    Entry::Sep,
                    item("paste", "Colar"),
                    item("select_all", "Selecionar Tudo"),
                ]
            }
            Target::Element(id) => self.element_entries(id),
            Target::Group(gid) => self.group_entries(gid),
        }
    }

    fn element_entries(&self, id: &str) -> Vec<Entry> {
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return Vec::new() };
        // several elements, not a repeated index or a note's own text: a lone image gets no Agrupar
        let mut chosen: Vec<usize> = self.selected.iter().copied().filter(|j| !self.is_card_text(*j)).collect();
        chosen.sort_unstable();
        chosen.dedup();
        if chosen.len() >= 2 && chosen.contains(&i) {
            let mut rows = vec![sub(
                "Alinhar",
                ALIGNS
                    .iter()
                    .enumerate()
                    .flat_map(|(k, label)| {
                        let sep = if k == 3 || k == 6 { Some(Entry::Sep) } else { None };
                        sep.into_iter().chain([item(&format!("align:{k}"), label)])
                    })
                    .collect(),
            )];
            rows.push(keyed("tidy", "Organizar", "⇧⌘T"));
            rows.push(Entry::Sep);
            let (loose, piles) = (self.loose_selected_notes().len(), chosen.iter().filter(|j| self.scene.elements[**j].note.stacked).count());
            if piles == 0 && loose >= 2 {
                rows.push(item("stack", "Colocar no Fichário"));
            } else if piles == 1 && loose >= 1 {
                rows.push(item("stack", "Adicionar ao Fichário"));
            }
            rows.push(item("group", "Grupo"));
            if self.selected_has_group() {
                rows.push(item("ungroup", "Desagrupar"));
            }
            rows.extend([Entry::Sep, item("front", "Trazer para Frente"), item("back", "Enviar para Trás")]);
            return rows;
        }
        if self.scene.elements[i].terminal.is_some() {
            return self.terminal_entries(i);
        }
        if self.scene.elements[i].chat.is_some() {
            return self.chat_node_entries(id);
        }
        let e = &self.scene.elements[i];
        let lock = if e.locked { "Desbloquear" } else { "Bloquear" };
        // the common rows of every element: copy, duplicate, lock, then apagar
        let common = || {
            vec![
                item("copy_el", "Copiar elemento"),
                item("dup", "Duplicar"),
                item("lock", lock),
                item("front", "Trazer para Frente"),
                item("back", "Enviar para Trás"),
                Entry::Sep,
                red("delete", "Apagar"),
            ]
        };
        let mut rows = if let Some(t) = &e.terminal {
            // the reference app's order: name and focus, the buffer, monitoring
            vec![
                item("rename", "Renomear"),
                item("edit", "Editar…"),
                entry("unfocus", "Desfocar", None, false, self.terminal_focus.as_deref() == Some(id)),
                Entry::Sep,
                // off the Mac Ctrl+K stays the shell's, so no shortcut
                keyed("clear", "Limpar buffer", if cfg!(target_os = "macos") { "⌘K" } else { "" }),
                item("reload", "Recarregar"),
                entry("unload", "Descarregar", None, false, !t.unloaded),
                Entry::Sep,
                item("monitor", if t.quiet { "Ativar monitoramento" } else { "Desativar monitoramento" }),
                Entry::Sep,
            ]
        } else if e.created.is_some() {
            // a pile's page: the pile's own menu (each page's is on its tab)
            return if e.note.stacked { self.fichario_rows(i) } else { self.note_rows(i, id) };
        } else if let Some(p) = &e.portal {
            vec![
                item("rename", "Renomear"),
                item("edit", "Editar…"),
                item("reload", "Recarregar"),
                entry("unload", "Descarregar", None, false, !p.unloaded),
                item("open_browser", "Abrir no navegador"),
                item("chrome", if p.chrome_hidden { "Mostrar Barra" } else { "Ocultar Barra" }),
                sub(
                    "Mostrar Tamanho",
                    SizeReadout::ALL.iter().map(|s| tick(&format!("portal:size:{}", s.key()), s.label(), self.prefs.portal_size == *s)).collect(),
                ),
                entry("portal:shot", "Copiar Captura de Tela", None, false, cfg!(windows) && !p.unloaded),
                entry("portal:mute", if p.muted { "Ativar som" } else { "Silenciar" }, None, false, !p.unloaded),
                sub("Armazenamento", StorageScope::ALL.iter().map(|s| tick(&format!("portal:storage:{}", s.key()), s.label(), p.storage == *s)).collect()),
                Entry::Sep,
            ]
        } else if e.filetree.is_some() {
            vec![item("show_folder", "Mostrar na Pasta"), item("edit", "Alterar Diretório Raiz"), Entry::Sep]
        } else if e.ide.is_some() {
            vec![item("show_folder", "Mostrar na pasta"), Entry::Sep]
        } else {
            vec![]
        };
        rows.extend(self.disconnect_rows(id).unwrap_or_default());
        rows.extend(common());
        rows
    }

    /// A sticky note's menu, row for row the reference app's; a pile's page (its tab's menu) has the pile's rows instead of
    /// copy, duplicate and lock.
    pub(crate) fn note_rows(&self, i: usize, id: &str) -> Vec<Entry> {
        let e = &self.scene.elements[i];
        let paper = e.style.fill;
        let colors = NOTE_PAPERS.iter().enumerate().map(|(k, (c, name))| tick(&format!("color:{k}"), name, paper == Some(*c))).collect();
        let has_text = !self.note_text(i).trim().is_empty();
        let head = vec![
            item("rename", "Renomear"),
            item("lock_content", if e.note.content_locked { "Desbloquear Conteúdo" } else { "Bloquear Conteúdo" }),
            Entry::Sep,
            entry("copy_content", "Copiar Conteúdo", None, false, has_text),
            entry("show_folder", "Mostrar na Pasta", None, false, self.autosave.is_some()),
        ];
        if e.note.stacked {
            return [head, vec![sub("Cor", colors), Entry::Sep], self.stack_rows(i)].concat();
        }
        let lock = if e.locked { "Desbloquear" } else { "Bloquear" };
        let mut rows = [
            head,
            vec![
                item("copy_el", "Copiar Elemento"),
                item("dup", "Duplicar"),
                sub("Cor", colors),
                Entry::Sep,
                item("lock", lock),
                Entry::Sep,
                red("delete", "Apagar"),
            ],
        ]
        .concat();
        if let Some(cut) = self.disconnect_rows(id) {
            rows.splice(9..9, cut);
        }
        rows
    }

    /// The nodes `id` is joined to, one row each to cut it (ui/links.rs), and a separator.
    pub(crate) fn disconnect_rows(&self, id: &str) -> Option<Vec<Entry>> {
        let joined = self.links().remove(id).filter(|j| !j.is_empty())?;
        let nodes = self.agent_nodes();
        let cuts = joined
            .iter()
            .map(|other| item(&format!("disconnect:{other}"), &nodes.iter().find(|n| &n.id == other).map(|n| n.name.clone()).unwrap_or_default()))
            .collect();
        Some(vec![sub("Desconectar", cuts), Entry::Sep])
    }

    /// Runs a row of a menu on its target.
    pub(crate) fn context_action(&mut self, ctx: &Context, target: &Target, id: &str) {
        if let Target::Group(gid) = target {
            self.group_menu_action(gid, id);
            return;
        }
        let index = match target {
            Target::Element(eid) => self.scene.elements.iter().position(|e| &e.id == eid),
            Target::Canvas(_) | Target::Group(_) => None,
        };
        let at = match target {
            Target::Canvas(p) => *p,
            Target::Element(_) | Target::Group(_) => Pos2::ZERO,
        };
        if self.terminal_menu_action(ctx, index, id) || self.chat_menu_action(target, id) {
            return;
        }
        match id {
            "rename" => {
                if let Some(i) = index {
                    self.open_rename(i);
                }
            }
            "edit" => {
                if let Some(i) = index {
                    self.open_edit(i);
                }
            }
            "monitor" => {
                if let Some(i) = index.filter(|i| self.scene.elements[*i].terminal.is_some()) {
                    self.snapshot();
                    if let Some(t) = self.scene.elements[i].terminal.as_mut() {
                        t.quiet = !t.quiet;
                    }
                }
            }
            "unfocus" => self.terminal_focus = None,
            "clear" => {
                if let Some(Ok(session)) = index.and_then(|i| self.terminals.get(&self.scene.elements[i].id)) {
                    session.handle.clear();
                }
            }
            "unload" => {
                if let Some(i) = index {
                    self.unload_node(i, true);
                }
            }
            "reload" => {
                // an unloaded one starts again; else the page reloads or the shell starts over
                if let Some(i) = index
                    && !self.unload_node(i, false)
                {
                    let eid = self.scene.elements[i].id.clone();
                    if self.scene.elements[i].portal.is_some() {
                        self.portal_reload(&eid);
                    } else {
                        // the next frame starts the shell again
                        self.terminals.remove(&eid);
                    }
                }
            }
            "copy_el" => self.copy(ctx),
            "dup" => self.duplicate(),
            "select_all" => self.select_all(),
            "tidy" => self.tidy_selection(),
            "front" => self.reorder(Layer::Front),
            "back" => self.reorder(Layer::Back),
            "chrome" => {
                if let Some(i) = index.filter(|i| self.scene.elements[*i].portal.is_some()) {
                    self.snapshot();
                    if let Some(spec) = self.scene.elements[i].portal.as_mut() {
                        spec.chrome_hidden = !spec.chrome_hidden;
                    }
                }
            }
            "lock" => self.toggle_lock_selected(),
            "delete" => self.delete_selected(),
            "copy_content" => {
                if let Some(i) = index {
                    ctx.copy_text(self.note_text(i));
                }
            }
            "lock_content" => {
                if let Some(i) = index {
                    self.snapshot();
                    self.scene.elements[i].note.content_locked = !self.scene.elements[i].note.content_locked;
                }
            }
            "open_browser" => {
                if let Some(url) = index.and_then(|i| self.scene.elements[i].portal.as_ref()).map(|p| normalize_url(&p.url)) {
                    if is_web(&url) {
                        open_external(&url);
                    } else {
                        self.toast("Só endereços http e https abrem no navegador.");
                    }
                }
            }
            "show_folder" if index.is_some_and(|i| self.scene.elements[i].filetree.is_some()) => {
                if let Some(spec) = index.and_then(|i| self.scene.elements[i].filetree.as_ref()) {
                    open_external(&spec.shown().to_string_lossy());
                }
            }
            "show_folder" if index.is_some_and(|i| self.scene.elements[i].created.is_some()) => {
                // the note's `.md` lives in `notas/` beside the scene (file.rs)
                if let (Some(scene), Some(i)) = (&self.autosave, index) {
                    if let Some(folder) = crate::file::note_file(scene, &self.scene.elements[i]).parent() {
                        open_external(&folder.to_string_lossy());
                    }
                }
            }
            "show_folder" => {
                // the editor's root folder, as the IDE's bar opens it
                if let Some(root) = index.and_then(|i| self.ides.get(&self.scene.elements[i].id)).and_then(|ide| ide.root.clone()) {
                    open_external(&root.to_string_lossy());
                }
            }
            "stack" => {
                self.stack_notes();
            }
            "stack_uniform" => {
                if let Some(i) = index {
                    self.uniform_stack_color(i);
                }
            }
            "stack_delete" => {
                if let Some(i) = index {
                    self.delete_stack_page(i);
                }
            }
            "unstack" => {
                if let Some(i) = index {
                    self.unstack(i);
                }
            }
            "stack_page" => {
                if let Some(i) = index {
                    self.new_stack_page(i);
                }
            }
            "stack_remove" => {
                if let Some(i) = index {
                    self.remove_from_stack(i);
                }
            }
            "group" => self.group_selection(),
            "ungroup" => self.ungroup_selection(),
            "paste" => self.paste(""),
            "add_terminal" => self.open_terminal_dialog(at, at),
            "add_note" => self.place_sticky(at, at),
            "add_file" => self.insert_image(),
            "add_portal" => self.open_portal_dialog(at, at),
            "add_text" => self.start_text(at),
            "add_ide" => self.open_ide_dialog(at, at),
            "add_filetree" => self.open_filetree_dialog(at, at),
            _ => {
                if let Some(preset) = id.strip_prefix("preset:").and_then(|k| k.parse::<usize>().ok()).and_then(|k| self.prefs.presets.get(k)).cloned() {
                    self.new_terminal_at(at, &preset);
                } else if let (Some(op), Some(i)) = (id.strip_prefix("navegador:"), index) {
                    let node = self.scene.elements[i].id.clone();
                    self.portal_menu(&node, op);
                } else if let Some(op) = id.strip_prefix("fmt:") {
                    if let Some(i) = index {
                        self.note_format(ctx, i, op);
                    }
                } else if let Some(paper) = id.strip_prefix("color:").and_then(|k| k.parse::<usize>().ok()).and_then(|k| NOTE_PAPERS.get(k)) {
                    if let Some(i) = index {
                        self.snapshot();
                        self.scene.elements[i].style.fill = Some(paper.0);
                    }
                } else if let Some(k) = id.strip_prefix("align:").and_then(|k| k.parse::<usize>().ok()) {
                    self.align_selection(k);
                } else if let (Some(other), Some(i)) = (id.strip_prefix("link:"), index) {
                    let node = self.scene.elements[i].id.clone();
                    self.toggle_link(&node, other);
                } else if let (Some(other), Some(i)) = (id.strip_prefix("disconnect:"), index) {
                    let node = self.scene.elements[i].id.clone();
                    self.unlink(&node, other);
                } else if let (Some((k, path)), Some(i)) = (id.strip_prefix("file:").and_then(|f| f.split_once(':')), index) {
                    let action = k.parse::<usize>().ok().and_then(|k| FILE_MENU.into_iter().flatten().nth(k));
                    if let (Some(&(action, _)), Some(ide)) = (action, self.ides.get_mut(&self.scene.elements[i].id)) {
                        ide.file_action(Path::new(path), action);
                        // a question it asks takes the keys from the card's tree
                        self.card_focus = None;
                    }
                }
            }
        }
    }

    /// A terminal from a quick start, centered on the click (the new terminal dialog's own path).
    fn new_terminal_at(&mut self, at: Pos2, preset: &Preset) {
        let spec = TerminalSpec { name: preset.name.clone(), cwd: self.new_folder(), command: preset.command.clone(), ..Default::default() };
        self.terminal_dialog = Some(TerminalDialog { target: None, at: Rect::from_center_size(at, terminal_size(80, 24)), spec, tab: 0 });
        self.confirm_terminal_dialog();
    }

    /// The edit dialog of a terminal, portal or IDE (the node's bar opens the same ones).
    fn open_edit(&mut self, i: usize) {
        let e = self.scene.elements[i].clone();
        if let Some(spec) = e.terminal {
            self.terminal_dialog = Some(TerminalDialog { target: Some(e.id), at: Rect::NOTHING, spec, tab: 0 });
        } else if let Some(spec) = e.portal {
            self.portal_dialog = Some(PortalDialog { target: Some(e.id), at: Rect::NOTHING, spec });
        } else if let Some(spec) = e.ide {
            self.ide_dialog = Some(IdeDialog { target: Some(e.id), at: Rect::NOTHING, spec });
        } else if let Some(spec) = e.filetree {
            self.filetrees.dialog = Some(FileTreeDialog { target: Some(e.id), at: Rect::NOTHING, root: spec.root });
        }
    }

    fn open_rename(&mut self, i: usize) {
        let e = &self.scene.elements[i];
        let name = match (&e.terminal, &e.portal) {
            (Some(t), _) => t.name.clone(),
            (_, Some(p)) => p.name.clone(),
            _ => note_name(&self.note_text(i)),
        };
        self.rename = Some((e.id.clone(), name));
    }

    /// Gives the element its new name: a terminal's (kept unique), a portal's, or a note's first
    /// line (the name a note shows).
    pub(crate) fn set_name(&mut self, id: &str, name: &str) {
        if let Some(gid) = id.strip_prefix("group:") {
            self.rename_frame_group(gid, name);
            return;
        }
        let name = name.trim();
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return };
        if self.scene.elements[i].terminal.is_some() {
            let name = match name {
                "" => self.command_name(&self.scene.elements[i].terminal.as_ref().map_or(String::new(), |t| t.command.clone())),
                name => name.to_string(),
            };
            let unique = self.unique_terminal_name(&name, Some(id));
            self.snapshot();
            if let Some(t) = self.scene.elements[i].terminal.as_mut() {
                t.name = unique;
            }
        } else if self.scene.elements[i].portal.is_some() {
            self.snapshot();
            if let Some(p) = self.scene.elements[i].portal.as_mut() {
                p.name = name.into();
            }
        } else if self.scene.elements[i].created.is_some() && !name.is_empty() {
            if let Some(label) = note_label(&self.scene.elements, i) {
                self.snapshot();
                if let ElementKind::Text { value, .. } = &mut self.scene.elements[label].kind {
                    *value = rename_note_text(value, name);
                }
            }
        }
    }

    /// The Renomear dialog: a name field, Cancelar and Salvar.
    pub(crate) fn rename_ui(&mut self, ctx: &Context) {
        let Some((id, mut name)) = self.rename.clone() else { return };
        let c = self.colors();
        let group = id.starts_with("group:");
        let (confirm, cancel) = dialog_ui(ctx, &c, if group { "Renomear grupo" } else { "Renomear" }, "Salvar", |ui| {
            dialog_field(ui, "Nome", &mut name, if group { "Nome do grupo" } else { "Nome" }, DIALOG_WIDTH, &c);
        });
        if cancel {
            self.rename = None;
        } else if confirm {
            self.rename = None;
            self.set_name(&id, &name);
        } else {
            self.rename = Some((id, name));
        }
    }
}
