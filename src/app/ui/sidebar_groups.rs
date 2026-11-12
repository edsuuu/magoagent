//! The sidebar's list: Fixados, the loose spaces, then the groups (Novo grupo) each with its heading
//! and the spaces under it, and the menus of a space and of a group (sidebar.rs draws the frame).

use super::sidebar::Tones;
use crate::app::ui::menu_look::{click_menu, context_menu};
use crate::app::workspaces::{SpaceDialog, SpaceGroup, Workspace, space_colors};
use crate::prelude::*;

/// A row's height, and a group heading's.
const ROW: f32 = 28.0;
const HEADING: f32 = 24.0;
/// How far the spaces under a group sit in (the reference app's `pl-4`).
const INDENT: f32 = 13.0;

pub(super) enum SpaceMenu {
    Pin,
    Move(Option<String>),
    CopyPath,
    Rename,
    Color([u8; 3]),
    Duplicate,
    Export,
    Import,
    Unload,
    Remove,
}

pub(super) enum GroupMenu {
    Toggle,
    Rename,
    Remove,
}

/// What the list's frame asked for: a space to open, a menu choice, an inline rename that ended
/// (true = keep it).
#[derive(Default)]
pub(super) struct Picks {
    pub(super) pick: Option<String>,
    pub(super) action: Option<(String, SpaceMenu)>,
    pub(super) renamed: Option<bool>,
    pub(super) group: Option<(String, GroupMenu)>,
}

/// A space's menu: Fixar, Mover para…, Copiar Caminho, Renomear, its colors, Duplicar, Exportar…,
/// Importar…, Descarregar, Remover.
fn space_menu(ui: &mut egui::Ui, space: &Workspace, groups: &[SpaceGroup], c: &UiColors) -> Option<SpaceMenu> {
    ui.set_width(232.0);
    let mut picked = None;
    if menu_item(ui, Icon::Arrow(true), if space.pinned { "Desfixar" } else { "Fixar" }, "", c.text, c) {
        picked = Some(SpaceMenu::Pin);
    }
    if !groups.is_empty() {
        // a row like menu_item's, that opens its groups to the side
        let (rect, row) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
        if row.hovered() {
            ui.painter().rect_filled(rect, 6, c.surface_high);
        }
        paint_icon(ui.painter(), Icon::Open, Pos2::new(rect.left() + 16.0, rect.center().y), c.text);
        ui.painter().text(Pos2::new(rect.left() + 34.0, rect.center().y), egui::Align2::LEFT_CENTER, "Mover para…", FontId::proportional(14.0), c.text);
        ui.painter().add(egui::containers::menu::SubMenuButton::arrow_shape(
            Rect::from_center_size(rect.right_center() - Vec2::new(12.0, 0.0), Vec2::splat(8.0)),
            c.text,
        ));
        egui::containers::menu::SubMenu::new().show(ui, &row, |ui| {
            ui.set_width(180.0);
            if !space.group.is_empty() && menu_item(ui, Icon::Open, "Sem Grupo", "", c.text, c) {
                picked = Some(SpaceMenu::Move(None));
            }
            for group in groups.iter().filter(|g| g.id != space.group) {
                if menu_item(ui, Icon::Open, &group.name, "", c.text, c) {
                    picked = Some(SpaceMenu::Move(Some(group.id.clone())));
                }
            }
        });
    }
    if !space.root.is_empty() && menu_item(ui, Icon::Code, "Copiar Caminho", "", c.text, c) {
        picked = Some(SpaceMenu::CopyPath);
    }
    if menu_item(ui, Icon::Edit, "Renomear", "", c.text, c) {
        picked = Some(SpaceMenu::Rename);
    }
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.x = 2.0;
        for color in space_colors() {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
            let [r, g, b] = color;
            ui.painter().circle_filled(rect.center(), 7.0, Color32::from_rgb(r, g, b));
            if color == space.color || response.hovered() {
                ui.painter().circle_stroke(rect.center(), 9.0, Stroke::new(1.5, c.text));
            }
            if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                picked = Some(SpaceMenu::Color(color));
            }
        }
    });
    for (icon, label, menu) in [
        (Icon::Duplicate, "Duplicar", SpaceMenu::Duplicate),
        (Icon::Export, "Exportar Espaço de Trabalho…", SpaceMenu::Export),
        (Icon::Download, "Importar Espaço de Trabalho…", SpaceMenu::Import),
        (Icon::Moon, "Descarregar", SpaceMenu::Unload),
    ] {
        if menu_item(ui, icon, label, "", c.text, c) {
            picked = Some(menu);
        }
    }
    if menu_item(ui, Icon::Trash, "Remover…", "", c.danger, c) {
        picked = Some(SpaceMenu::Remove);
    }
    if picked.is_some() {
        ui.close();
    }
    picked
}

/// A group's menu: Renomear, Apagar Grupo.
fn group_menu(ui: &mut egui::Ui, c: &UiColors) -> Option<GroupMenu> {
    ui.set_width(200.0);
    let mut picked = None;
    if menu_item(ui, Icon::Edit, "Renomear", "", c.text, c) {
        picked = Some(GroupMenu::Rename);
    }
    if menu_item(ui, Icon::Trash, "Apagar Grupo", "", c.danger, c) {
        picked = Some(GroupMenu::Remove);
    }
    if picked.is_some() {
        ui.close();
    }
    picked
}

/// A small uppercase title over a stretch of the list (Fixados).
fn title(ui: &mut egui::Ui, t: &Tones, width: f32, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::hover());
    ui.painter().text(Pos2::new(rect.left() + 8.0, rect.center().y), egui::Align2::LEFT_CENTER, text, FontId::proportional(11.0), t.muted);
}

impl DrawingApp {
    /// The name Novo grupo offers: free among the groups.
    pub(super) fn new_group_name(&self) -> String {
        let taken = |n: &str| self.spaces.groups.iter().any(|g| g.name.to_lowercase() == n.to_lowercase());
        (1..).map(|i| if i == 1 { "Novo grupo".to_string() } else { format!("Novo grupo {i}") }).find(|n| !taken(n)).unwrap_or_default()
    }

    /// The scrolling list in `list`: `spaces` are the ones the filter keeps (all of them when it is empty).
    pub(super) fn sidebar_list(&mut self, ui: &mut egui::Ui, ctx: &Context, c: &UiColors, t: &Tones, list: Rect, spaces: &[Workspace]) -> Picks {
        let mut out = Picks::default();
        let searching = !self.space_filter.trim().is_empty();
        let groups = self.spaces.groups.clone();
        let current = self.spaces.current.clone();
        let counts = self.space_counts();
        let width = list.width();
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            egui::ScrollArea::vertical().max_height(list.height()).show(ui, |ui| {
                if spaces.is_empty() {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Nenhum espaço de trabalho corresponde à busca.").size(12.0).color(t.muted));
                }
                let pinned: Vec<_> = spaces.iter().filter(|w| w.pinned).collect();
                if !pinned.is_empty() {
                    title(ui, t, width, "FIXADOS");
                    for space in pinned {
                        self.space_row(ui, ctx, c, t, space, &groups, (&current, &counts), 0.0, &mut out);
                    }
                }
                for space in spaces.iter().filter(|w| !w.pinned && w.group.is_empty()) {
                    self.space_row(ui, ctx, c, t, space, &groups, (&current, &counts), 0.0, &mut out);
                }
                for group in &groups {
                    let inside: Vec<_> = spaces.iter().filter(|w| !w.pinned && w.group == group.id).collect();
                    if searching && inside.is_empty() {
                        continue;
                    }
                    let open = searching || group.open;
                    ui.add_space(2.0);
                    self.group_heading(ui, c, t, group, width, open, &mut out);
                    if open {
                        for space in inside {
                            self.space_row(ui, ctx, c, t, space, &groups, (&current, &counts), INDENT, &mut out);
                        }
                    }
                }
            });
        });
        out
    }

    /// A group's heading: the chevron, its name in capitals; a click folds or opens it, ⋯ or a right click is its menu.
    fn group_heading(&self, ui: &mut egui::Ui, c: &UiColors, t: &Tones, group: &SpaceGroup, width: f32, open: bool, out: &mut Picks) {
        let (rect, row) = ui.allocate_exact_size(Vec2::new(width, HEADING), Sense::click());
        let over = ui.rect_contains_pointer(rect);
        let p = ui.painter().clone();
        if over {
            p.rect_filled(rect, 6, t.hover);
        }
        let (x, y) = (rect.left() + 14.5, rect.center().y);
        let chevron = if open {
            [Pos2::new(x - 3.5, y - 1.5), Pos2::new(x, y + 2.0), Pos2::new(x + 3.5, y - 1.5)]
        } else {
            [Pos2::new(x - 1.5, y - 3.5), Pos2::new(x + 2.0, y), Pos2::new(x - 1.5, y + 3.5)]
        };
        p.add(egui::Shape::line(chevron.to_vec(), Stroke::new(1.3, t.muted)));
        let name = Rect::from_min_max(Pos2::new(rect.left() + 27.0, rect.top()), Pos2::new(rect.right() - 28.0, rect.bottom()));
        p.with_clip_rect(name).text(name.left_center(), egui::Align2::LEFT_CENTER, group.name.to_uppercase(), FontId::proportional(11.0), t.muted);
        let more = Rect::from_center_size(Pos2::new(rect.right() - 13.0, rect.center().y), Vec2::splat(22.0));
        let more = ui.interact(more, ui.scope_id().with(("group_more", &group.id)), Sense::click());
        if over {
            paint_icon(&p, Icon::More, more.rect.center(), if more.hovered() { t.text } else { t.muted });
        }
        let tip = format!("{}: {}", group.name, if open { "Recolher grupo" } else { "Expandir grupo" });
        let row = row.on_hover_cursor(CursorIcon::PointingHand).tip(tip);
        if row.clicked() {
            out.group = Some((group.id.clone(), GroupMenu::Toggle));
        }
        if let Some(m) = click_menu(&more, |ui| group_menu(ui, c)).flatten().or_else(|| context_menu(&row, |ui| group_menu(ui, c)).flatten()) {
            out.group = Some((group.id.clone(), m));
        }
    }

    /// One space's row, `indent` in from the list's edge (the spaces of a group).
    #[allow(clippy::too_many_arguments)]
    fn space_row(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &Context,
        c: &UiColors,
        t: &Tones,
        space: &Workspace,
        groups: &[SpaceGroup],
        (current, counts): (&str, &std::collections::HashMap<String, (usize, usize)>),
        indent: f32,
        out: &mut Picks,
    ) {
        let (full, row) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW), Sense::click());
        let rect = Rect::from_min_max(full.min + Vec2::new(indent, 0.0), full.max);
        let (on, over) = (space.id == current, ui.rect_contains_pointer(rect));
        let p = ui.painter().clone();
        if on || over {
            p.rect_filled(rect, 6, if on { t.active } else { t.hover });
        }
        let [r, g, b] = space.color;
        paint_icon_scaled(&p, Icon::Open, Pos2::new(rect.left() + 14.5, rect.center().y), Color32::from_rgb(r, g, b), 0.8, 0.0);
        if let Some((_, text)) = self.space_rename.as_mut().filter(|(id, _)| *id == space.id) {
            let spot = Rect::from_min_max(Pos2::new(rect.left() + 25.0, rect.top() + 2.0), Pos2::new(rect.right() - 4.0, rect.bottom() - 2.0));
            let field = ui.put(spot, egui::TextEdit::singleline(text).font(FontId::proportional(13.0)).margin(egui::Margin::symmetric(6, 3)));
            if field.lost_focus() {
                out.renamed = Some(!ctx.input(|i| i.key_pressed(Key::Escape)));
            } else if !field.has_focus() {
                field.request_focus();
            }
            return;
        }
        // the agents running, by a terminal's mark, and in amber the ones that need you; ⋯ beside them
        let (running, needs) = counts.get(&space.id).copied().unwrap_or_default();
        let mut x = rect.right() - 28.0;
        // the Layers button of the floors, beside ⋯ (while the pointer is over the row or the space has floors)
        let ink = crate::app::ui::floors::RowInk { text: t.text, muted: t.muted, hover: t.hover, active: t.active };
        if over || !space.floors.is_empty() {
            self.floors_toggle(ui, Pos2::new(rect.right() - 36.0, rect.center().y), &space.id, &ink);
            x -= 22.0;
        }
        if running > 0 {
            let text = p.text(Pos2::new(x, rect.center().y), egui::Align2::RIGHT_CENTER, running.to_string(), FontId::proportional(10.5), t.muted);
            paint_icon_scaled(&p, Icon::Tool(Tool::Terminal), Pos2::new(text.left() - 11.0, rect.center().y), t.muted, 0.8, 0.0);
            x = text.left() - 24.0;
        }
        if needs > 0 {
            let text = p.text(Pos2::new(x, rect.center().y), egui::Align2::RIGHT_CENTER, needs.to_string(), FontId::proportional(10.5), t.muted);
            p.circle_filled(Pos2::new(text.left() - 6.0, rect.center().y), 3.0, c.warning);
            x = text.left() - 14.0;
        }
        let name = Rect::from_min_max(Pos2::new(rect.left() + 29.0, rect.top()), Pos2::new(x, rect.bottom()));
        p.with_clip_rect(name).text(name.left_center(), egui::Align2::LEFT_CENTER, &space.name, FontId::proportional(13.0), t.text);
        let more = Rect::from_center_size(Pos2::new(rect.right() - 13.0, rect.center().y), Vec2::splat(22.0));
        let more = ui.interact(more, ui.scope_id().with(("space_more", &space.id)), Sense::click());
        if over {
            paint_icon(&p, Icon::More, more.rect.center(), if more.hovered() { t.text } else { t.muted });
        }
        let tip = match (running, needs) {
            (0, _) => "Nenhum agente rodando".to_string(),
            (n, 0) => format!("{n} agente(s) rodando"),
            (n, m) => format!("{n} agente(s) rodando, {m} precisa(m) de você"),
        };
        let tip = if space.root.is_empty() { tip } else { format!("{tip}\n{}", display_folder(&space.root)) };
        let row = row.on_hover_cursor(CursorIcon::PointingHand).tip(tip);
        if row.clicked() {
            out.pick = Some(space.id.clone());
        }
        let picked = click_menu(&more, |ui| space_menu(ui, space, groups, c)).flatten();
        if let Some(m) = picked.or_else(|| context_menu(&row, |ui| space_menu(ui, space, groups, c)).flatten()) {
            out.action = Some((space.id.clone(), m));
        }
        self.floor_rows(ui, &space.id, rect.width(), &ink);
    }

    /// Does what the menu of space `id` chose, but Renomear, Cor and Remover (sidebar.rs).
    pub(super) fn space_menu_act(&mut self, ctx: &Context, id: &str, menu: SpaceMenu) {
        let done = match menu {
            SpaceMenu::Pin => self.toggle_pin(id).map(|_| String::new()),
            SpaceMenu::Move(group) => self.move_space(id, group.as_deref()).map(|_| String::new()),
            SpaceMenu::CopyPath => {
                ctx.copy_text(self.spaces.list.iter().find(|w| w.id == id).map_or(String::new(), |w| w.root.clone()));
                Ok("Caminho copiado.".into())
            }
            SpaceMenu::Duplicate => self.duplicate_space(id).map(|_| String::new()),
            SpaceMenu::Export => {
                let name = self.space_name(id);
                match rfd::FileDialog::new()
                    .set_title("Exportar espaço de trabalho")
                    .add_filter("Espaço de trabalho (.espaco)", &["espaco"])
                    .set_file_name(format!("{name}.espaco"))
                    .save_file()
                {
                    Some(path) => self.export_space(id, &path),
                    None => Ok(String::new()),
                }
            }
            SpaceMenu::Import => {
                match rfd::FileDialog::new().set_title("Importar espaço de trabalho").add_filter("Espaço de trabalho (.espaco)", &["espaco"]).pick_file() {
                    Some(path) => self.import_space(&path),
                    None => Ok(String::new()),
                }
            }
            SpaceMenu::Unload => self.unload_space(id).map(|n| format!("Descarregado ({n}).")),
            SpaceMenu::Rename | SpaceMenu::Color(_) | SpaceMenu::Remove => Ok(String::new()),
        };
        match done {
            Ok(message) if !message.is_empty() => self.toast(message),
            Err(error) => self.toast(error),
            _ => {}
        }
    }

    /// Does what the menu or the heading of group `id` chose.
    pub(super) fn group_act(&mut self, id: &str, menu: GroupMenu) {
        let name = self.spaces.groups.iter().find(|g| g.id == id).map_or(String::new(), |g| g.name.clone());
        match menu {
            GroupMenu::Toggle => self.toggle_group(id),
            GroupMenu::Rename => self.space_dialog = Some(SpaceDialog::RenameGroup { id: id.into(), name }),
            GroupMenu::Remove => self.space_dialog = Some(SpaceDialog::RemoveGroup(id.into())),
        }
    }

    /// Novo grupo, Renomear grupo and Apagar Grupo (the last one moves no space out of the list).
    pub(super) fn group_dialog_ui(&mut self, ctx: &Context, c: &UiColors, dialog: SpaceDialog) {
        match dialog {
            SpaceDialog::NewGroup { mut name } => {
                let (ok, cancel) = dialog_ui(ctx, c, "Novo grupo", "Criar", |ui| {
                    dialog_field(ui, "Nome do grupo", &mut name, "Novo grupo", DIALOG_WIDTH, c);
                });
                if cancel {
                    return;
                }
                if let Err(error) = if ok { self.create_group(&name).map(|_| ()) } else { Err(String::new()) } {
                    self.toast(error);
                    self.space_dialog = Some(SpaceDialog::NewGroup { name });
                }
            }
            SpaceDialog::RenameGroup { id, mut name } => {
                let (ok, cancel) = dialog_ui(ctx, c, "Renomear grupo", "Renomear", |ui| {
                    dialog_field(ui, "Nome do grupo", &mut name, "Nome do grupo", DIALOG_WIDTH, c);
                });
                if cancel {
                    return;
                }
                if let Err(error) = if ok { self.rename_group(&id, &name) } else { Err(String::new()) } {
                    self.toast(error);
                    self.space_dialog = Some(SpaceDialog::RenameGroup { id, name });
                }
            }
            SpaceDialog::RemoveGroup(id) => {
                let (ok, cancel) = dialog_ui(ctx, c, "Apagar Grupo", "Apagar", |ui| {
                    ui.label(
                        egui::RichText::new("Este grupo será excluído permanentemente. Os espaços de trabalho dentro dele serão movidos para não agrupados.")
                            .size(13.5)
                            .color(c.text),
                    );
                });
                if ok {
                    match self.remove_group(&id) {
                        Ok(message) | Err(message) => self.toast(message),
                    }
                } else if !cancel {
                    self.space_dialog = Some(SpaceDialog::RemoveGroup(id));
                }
            }
            _ => {}
        }
    }
}
