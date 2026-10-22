//! Right-click menus: the canvas, a terminal, a note, a portal, an IDE and several selected
//! elements. They share the reference app's native dark look: a box, 24pt rows, a blue highlight
//! and ">" submenus that open beside their row and stay open while the pointer goes to them. Also
//! the Renomear dialog they open.

use crate::app::notes::rename_note_text;
use crate::prelude::*;

const MENU_BG: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2d);
const MENU_TEXT: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe6);
const MENU_ACCENT: Color32 = Color32::from_rgb(0x0a, 0x84, 0xff);
const MENU_DANGER: Color32 = Color32::from_rgb(0xff, 0x45, 0x3a);
const MENU_DISABLED: Color32 = Color32::from_rgb(0x6e, 0x6e, 0x70);
const MENU_WIDTH: f32 = 200.0;
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
}

/// A row of a menu: an action (its `id` goes to `context_action`), a submenu, or a separator.
#[derive(Clone, Debug)]
pub(crate) enum Entry {
    Item { id: String, label: String, check: Option<bool>, danger: bool, enabled: bool },
    Sub { label: String, items: Vec<Entry> },
    Sep,
}

fn entry(id: &str, label: &str, check: Option<bool>, danger: bool, enabled: bool) -> Entry {
    Entry::Item { id: id.into(), label: label.into(), check, danger, enabled }
}

pub(crate) fn item(id: &str, label: &str) -> Entry {
    entry(id, label, None, false, true)
}

fn tick(id: &str, label: &str, on: bool) -> Entry {
    entry(id, label, Some(on), false, true)
}

fn red(id: &str, label: &str) -> Entry {
    entry(id, label, None, true, true)
}

fn sub(label: &str, items: Vec<Entry>) -> Entry {
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
/// so on. Returns the id of the row picked.
fn panel(ctx: &Context, name: &str, at: Pos2, entries: &[Entry], path: &mut Vec<usize>, depth: usize, rects: &mut Vec<Rect>) -> Option<String> {
    let has_ticks = entries.iter().any(|e| matches!(e, Entry::Item { check: Some(_), .. }));
    let mut picked = None;
    let mut subs: Vec<(usize, Rect)> = Vec::new();
    let area = egui::Area::new(egui::Id::unique((name, depth))).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::new()
            .fill(MENU_BG)
            .corner_radius(10)
            .inner_margin(5)
            .stroke(Stroke::new(1.0, Color32::from_white_alpha(31)))
            .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(110) })
            .show(ui, |ui| {
                ui.set_width(MENU_WIDTH);
                ui.spacing_mut().item_spacing.y = 0.0;
                for (k, row_entry) in entries.iter().enumerate() {
                    match row_entry {
                        Entry::Sep => {
                            let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
                            ui.painter().hline(line.x_range().shrink(8.0), line.center().y, Stroke::new(1.0, Color32::from_white_alpha(31)));
                        }
                        Entry::Item { id, label, check, danger, enabled } => {
                            let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
                            let lit = response.hovered() && *enabled;
                            if lit {
                                path.truncate(depth);
                            }
                            let ink = match (*enabled, lit, *danger) {
                                (false, _, _) => MENU_DISABLED,
                                (true, true, _) => Color32::WHITE,
                                (true, false, true) => MENU_DANGER,
                                (true, false, false) => MENU_TEXT,
                            };
                            let p = ui.painter();
                            if lit {
                                p.rect_filled(row, 5, MENU_ACCENT);
                            }
                            if *check == Some(true) {
                                paint_icon(p, Icon::Check, Pos2::new(row.left() + 12.0, row.center().y), ink);
                            }
                            let label_x = row.left() + if has_ticks { 26.0 } else { 10.0 };
                            p.text(Pos2::new(label_x, row.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(13.0), ink);
                            if *enabled && response.clicked() {
                                picked = Some(id.clone());
                            }
                            if *enabled {
                                response.on_hover_cursor(CursorIcon::PointingHand);
                            }
                        }
                        Entry::Sub { label, .. } => {
                            let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
                            // hovering the row opens its submenu; it stays open while the pointer is in it
                            if response.hovered() {
                                path.truncate(depth);
                                path.push(k);
                            }
                            let lit = path.get(depth) == Some(&k);
                            let ink = if lit { Color32::WHITE } else { MENU_TEXT };
                            let p = ui.painter();
                            if lit {
                                p.rect_filled(row, 5, MENU_ACCENT);
                            }
                            let label_x = row.left() + if has_ticks { 26.0 } else { 10.0 };
                            p.text(Pos2::new(label_x, row.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(13.0), ink);
                            p.text(Pos2::new(row.right() - 10.0, row.center().y), egui::Align2::RIGHT_CENTER, ">", FontId::proportional(13.0), ink);
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
    panel(ctx, name, Pos2::new(area.response.rect.right() + 2.0, open.1.top() - 5.0), items, path, depth + 1, rects)
}

impl DrawingApp {
    /// A right click at `pos` (world `world`) opens the menu for what is under it. Its element
    /// joins the selection unless it is in it already; on empty canvas the selection goes.
    pub(crate) fn open_context(&mut self, pos: Pos2, world: Pos2) {
        // a rope's right click is its connections popover (ui/links.rs)
        if self.hit(world).is_some_and(|i| self.scene.elements[i].rope) {
            self.open_rope_popover(pos, world);
            return;
        }
        let target = match self.hit(world) {
            Some(i) => {
                if !self.selected.contains(&i) {
                    self.selected = self.group_members(i).into_iter().filter(|j| !self.is_card_text(*j)).collect();
                }
                Target::Element(self.scene.elements[i].id.clone())
            }
            None => {
                self.selected.clear();
                Target::Canvas(world)
            }
        };
        self.context = Some(ContextMenu::new(pos, target, None));
    }

    /// Draws the open menu, runs the row picked in it, and closes it on Esc or on a press outside
    /// its panels (the press that opened it has no panel yet).
    pub(crate) fn context_ui(&mut self, ctx: &Context) {
        let Some(mut menu) = self.context.take() else { return };
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
                let mut presets: Vec<Entry> = self.prefs.presets.iter().enumerate().filter(|(_, p)| p.on).take(10).map(|(k, p)| item(&format!("preset:{k}"), &p.name)).collect();
                if presets.is_empty() {
                    presets.push(entry("", "Nenhum início rápido ligado", None, false, false));
                }
                vec![
                    sub(
                        "Adicionar",
                        vec![
                            sub("Terminal", presets),
                            item("add_note", "Nota"),
                            item("add_file", "Anexar"),
                            item("add_ide", "Árvore de Arquivos"),
                            item("add_portal", "Portal"),
                            item("add_text", "Texto"),
                        ],
                    ),
                    Entry::Sep,
                    item("paste", "Colar"),
                ]
            }
            Target::Element(id) => self.element_entries(id),
        }
    }

    fn element_entries(&self, id: &str) -> Vec<Entry> {
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return Vec::new() };
        if self.selected.len() >= 2 && self.selected.contains(&i) {
            let mut rows = vec![sub("Alinhar", ALIGNS.iter().enumerate().flat_map(|(k, label)| {
                let sep = if k == 3 || k == 6 { Some(Entry::Sep) } else { None };
                sep.into_iter().chain([item(&format!("align:{k}"), label)])
            }).collect())];
            rows.push(Entry::Sep);
            rows.push(item("group", "Agrupar"));
            if self.selected_has_group() {
                rows.push(item("ungroup", "Desagrupar"));
            }
            return rows;
        }
        let e = &self.scene.elements[i];
        let lock = if e.locked { "Desbloquear" } else { "Bloquear" };
        // the common rows of every element: copy, duplicate, lock, then apagar
        let common = || vec![item("copy_el", "Copiar elemento"), item("dup", "Duplicar"), item("lock", lock), Entry::Sep, red("delete", "Apagar")];
        let mut rows = if let Some(t) = &e.terminal {
            vec![
                item("rename", "Renomear"),
                item("edit", "Editar…"),
                tick("monitor", "Monitorar atividade", !t.quiet),
                Entry::Sep,
                item("reload", "Recarregar"),
                Entry::Sep,
            ]
        } else if e.created.is_some() {
            let paper = e.style.fill;
            let colors = NOTE_PAPERS.iter().enumerate().map(|(k, (c, name))| tick(&format!("color:{k}"), name, paper == Some(*c))).collect();
            let has_text = !self.note_text(i).trim().is_empty();
            vec![
                item("rename", "Renomear nota"),
                entry("copy_content", "Copiar conteúdo", None, false, has_text),
                sub("Cor", colors),
                item("lock_content", if e.note.content_locked { "Desbloquear conteúdo" } else { "Bloquear conteúdo" }),
                Entry::Sep,
            ]
        } else if e.portal.is_some() {
            vec![
                item("rename", "Renomear"),
                item("edit", "Editar…"),
                item("reload", "Recarregar"),
                item("open_browser", "Abrir no navegador"),
                Entry::Sep,
            ]
        } else if e.ide.is_some() {
            vec![item("show_folder", "Mostrar na pasta"), Entry::Sep]
        } else {
            vec![]
        };
        // the nodes it is joined to, one row each to cut it (ui/links.rs)
        let mut links = self.links();
        let joined = links.remove(id).unwrap_or_default();
        if !joined.is_empty() {
            let nodes = self.agent_nodes();
            let cuts = joined.iter().map(|other| item(&format!("disconnect:{other}"), &nodes.iter().find(|n| &n.id == other).map(|n| n.name.clone()).unwrap_or_default())).collect();
            rows.push(sub("Desconectar", cuts));
            rows.push(Entry::Sep);
        }
        rows.extend(common());
        rows
    }

    /// Runs a row of a menu on its target.
    pub(crate) fn context_action(&mut self, ctx: &Context, target: &Target, id: &str) {
        let index = match target {
            Target::Element(eid) => self.scene.elements.iter().position(|e| &e.id == eid),
            Target::Canvas(_) => None,
        };
        let at = match target {
            Target::Canvas(p) => *p,
            Target::Element(_) => Pos2::ZERO,
        };
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
            "reload" => {
                if let Some(i) = index {
                    let eid = self.scene.elements[i].id.clone();
                    if self.scene.elements[i].portal.is_some() {
                        self.portal_script(&eid, "location.reload()");
                    } else {
                        // the next frame starts the shell again
                        self.terminals.remove(&eid);
                    }
                }
            }
            "copy_el" => self.copy(ctx),
            "dup" => self.duplicate(),
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
                if let Some(url) = index.and_then(|i| self.scene.elements[i].portal.as_ref()).map(|p| p.url.clone()) {
                    open_external(&normalize_url(&url));
                }
            }
            "show_folder" => {
                // the editor's root folder, as the IDE's bar opens it
                if let Some(root) = index.and_then(|i| self.ides.get(&self.scene.elements[i].id)).and_then(|ide| ide.root.clone()) {
                    open_external(&root.to_string_lossy());
                }
            }
            "group" => self.group_selection(),
            "ungroup" => self.ungroup_selection(),
            "paste" => self.paste(""),
            "add_note" => self.place_sticky(at, at),
            "add_file" => self.insert_image(),
            "add_portal" => self.open_portal_dialog(at, at),
            "add_text" => self.start_text(at),
            "add_ide" => self.open_ide_dialog(at, at),
            _ => {
                if let Some(preset) = id.strip_prefix("preset:").and_then(|k| k.parse::<usize>().ok()).and_then(|k| self.prefs.presets.get(k)).cloned() {
                    self.new_terminal_at(at, &preset);
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
                } else if let (Some(other), Some(i)) = (id.strip_prefix("disconnect:"), index) {
                    let node = self.scene.elements[i].id.clone();
                    self.unlink(&node, other);
                }
            }
        }
    }

    /// A terminal from a quick start, centered on the click (the new terminal dialog's own path).
    fn new_terminal_at(&mut self, at: Pos2, preset: &Preset) {
        let folder = self.file_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().to_string());
        let spec = TerminalSpec { name: preset.name.clone(), cwd: folder.unwrap_or_default(), command: preset.command.clone(), ..Default::default() };
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
    fn set_name(&mut self, id: &str, name: &str) {
        let name = name.trim();
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return };
        if self.scene.elements[i].terminal.is_some() {
            let unique = self.unique_terminal_name(if name.is_empty() { "Terminal" } else { name }, Some(id));
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
        let (confirm, cancel) = dialog_ui(ctx, &c, "Renomear", "Salvar", |ui| {
            dialog_field(ui, "Nome", &mut name, "Nome", DIALOG_WIDTH, &c);
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
