//! The workspaces' sidebar, left of the canvas: each space with its color, its name and its
//! agents (running, needing you). A click switches; ⋯ or a right click renames, recolors or
//! removes; + makes a new one. Folded, a button in the bottom left brings it back.

use crate::app::workspaces::{SpaceDialog, space_colors};
use crate::prelude::*;

/// What the open sidebar takes from the canvas's left.
pub(crate) const SIDEBAR: f32 = 220.0;

enum SpaceMenu {
    Rename,
    Color([u8; 3]),
    Remove,
}

/// A space's menu: Renomear, its colors, Remover.
fn space_menu(ui: &mut egui::Ui, current: [u8; 3], c: &UiColors) -> Option<SpaceMenu> {
    ui.set_width(200.0);
    let mut picked = None;
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
            if color == current || response.hovered() {
                ui.painter().circle_stroke(rect.center(), 9.0, Stroke::new(1.5, c.text));
            }
            if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                picked = Some(SpaceMenu::Color(color));
            }
        }
    });
    if menu_item(ui, Icon::Trash, "Remover…", "", c.danger, c) {
        picked = Some(SpaceMenu::Remove);
    }
    if picked.is_some() {
        ui.close();
    }
    picked
}

impl DrawingApp {
    /// The canvas's left strip the sidebar keeps: none folded or under the chat.
    pub(crate) fn sidebar_width(&self) -> f32 {
        if self.chat_open || self.prefs.sidebar_hidden { 0.0 } else { SIDEBAR }
    }

    pub(crate) fn sidebar(&mut self, ctx: &Context) {
        self.load_spaces();
        let c = self.colors();
        if !self.chat_open && self.prefs.sidebar_hidden {
            let open = egui::Area::new(egui::Id::unique("sidebar_open")).anchor(egui::Align2::LEFT_BOTTOM, (16.0, -16.0)).show(ctx, |ui| {
                group_frame(&c)
                    .show(ui, |ui| icon_button(ui, 36.0, false, Color32::TRANSPARENT, Icon::Sidebar, c.text, &c).on_hover_text("Mostrar os espaços"))
                    .inner
            });
            if open.inner.clicked() {
                self.prefs.sidebar_hidden = false;
            }
        } else if !self.chat_open {
            self.sidebar_panel(ctx, &c);
        }
        self.space_dialog_ui(ctx, &c);
    }

    fn sidebar_panel(&mut self, ctx: &Context, c: &UiColors) {
        // under the window's own buttons (Ajustes and the chat), from the title bar down
        let top = TITLE_BAR.max(TOP_ROW + 44.0) + 8.0;
        let spaces = self.spaces.list.clone();
        let current = self.spaces.current.clone();
        let counts = self.space_counts();
        let height = (ctx.content_rect().height() - top - 16.0).max(120.0);
        // 8 of gap on each side, the island's margins and rim
        let width = SIDEBAR - 16.0 - 18.0;
        let (mut pick, mut action, mut fold, mut new, mut renamed) = (None, None, false, false, None);
        let frame = self.island();
        egui::Area::new(egui::Id::unique("sidebar")).fixed_pos((8.0, top)).order(egui::Order::Foreground).show(ctx, |ui| {
            frame.inner_margin(8).show(ui, |ui| {
                ui.set_width(width);
                ui.set_min_height(height - 18.0);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Espaços").size(12.0).color(c.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        fold = icon_button(ui, 28.0, false, Color32::TRANSPARENT, Icon::Sidebar, c.text, c).on_hover_text("Recolher a barra").clicked();
                        new = icon_button(ui, 28.0, false, Color32::TRANSPARENT, Icon::Plus, c.text, c).on_hover_text("Novo espaço").clicked();
                    });
                });
                ui.add_space(4.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for space in &spaces {
                        let (rect, row) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
                        let (on, over) = (space.id == current, ui.rect_contains_pointer(rect));
                        let p = ui.painter().clone();
                        if on || over {
                            p.rect_filled(rect, 8, if on { c.selected } else { c.surface_high });
                        }
                        let [r, g, b] = space.color;
                        p.circle_filled(Pos2::new(rect.left() + 14.0, rect.center().y), 5.0, Color32::from_rgb(r, g, b));
                        if let Some((_, text)) = self.space_rename.as_mut().filter(|(id, _)| *id == space.id) {
                            let spot = Rect::from_min_max(Pos2::new(rect.left() + 24.0, rect.top() + 4.0), Pos2::new(rect.right() - 4.0, rect.bottom() - 4.0));
                            let field = ui.put(spot, egui::TextEdit::singleline(text).font(FontId::proportional(13.5)).margin(egui::Margin::symmetric(6, 3)));
                            if field.lost_focus() {
                                renamed = Some(!ctx.input(|i| i.key_pressed(Key::Escape)));
                            } else if !field.has_focus() {
                                field.request_focus();
                            }
                            continue;
                        }
                        // the agents running and, in amber, the ones that need you; ⋯ beside them
                        let (running, needs) = counts.get(&space.id).copied().unwrap_or_default();
                        let mut x = rect.right() - 30.0;
                        for (n, color) in [(needs, c.warning), (running, c.success)] {
                            if n > 0 {
                                let at = Pos2::new(x, rect.center().y);
                                let text = p.text(at, egui::Align2::RIGHT_CENTER, n.to_string(), FontId::proportional(12.0), c.muted);
                                p.circle_filled(Pos2::new(text.left() - 7.0, at.y), 3.5, color);
                                x = text.left() - 16.0;
                            }
                        }
                        let name = Rect::from_min_max(Pos2::new(rect.left() + 26.0, rect.top()), Pos2::new(x, rect.bottom()));
                        p.with_clip_rect(name).text(name.left_center(), egui::Align2::LEFT_CENTER, &space.name, FontId::proportional(13.5), c.text);
                        let more = Rect::from_center_size(Pos2::new(rect.right() - 15.0, rect.center().y), Vec2::splat(24.0));
                        let more = ui.interact(more, ui.scope_id().with(("space_more", &space.id)), Sense::click());
                        if over {
                            paint_icon(&p, Icon::More, more.rect.center(), if more.hovered() { c.text } else { c.muted });
                        }
                        let tip = match (running, needs) {
                            (0, _) => "Nenhum agente rodando".to_string(),
                            (n, 0) => format!("{n} agente(s) rodando"),
                            (n, m) => format!("{n} agente(s) rodando, {m} precisa(m) de você"),
                        };
                        let tip = if space.root.is_empty() { tip } else { format!("{tip}\n{}", display_folder(&space.root)) };
                        let row = row.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
                        if row.clicked() {
                            pick = Some(space.id.clone());
                        }
                        let menu = |ui: &mut egui::Ui| space_menu(ui, space.color, c);
                        if let Some(m) = egui::Popup::menu(&more).show(menu).and_then(|r| r.inner) {
                            action = Some((space.id.clone(), m));
                        }
                        row.context_menu(|ui| {
                            if let Some(m) = space_menu(ui, space.color, c) {
                                action = Some((space.id.clone(), m));
                            }
                        });
                    }
                });
            });
        });
        if fold {
            self.prefs.sidebar_hidden = true;
        }
        if new {
            self.space_dialog = Some(SpaceDialog::New { name: String::new(), folder: String::new() });
        }
        if let Some(Err(error)) = pick.map(|id| self.switch_space(&id)) {
            self.toast(error);
        }
        match action {
            Some((id, SpaceMenu::Rename)) => self.space_rename = Some((id.clone(), self.space_name(&id))),
            Some((id, SpaceMenu::Color(color))) => self.color_space(&id, color),
            Some(_) if self.spaces.list.len() <= 1 => self.toast("O último espaço não pode ser removido."),
            Some((id, SpaceMenu::Remove)) => self.space_dialog = Some(SpaceDialog::Remove(id)),
            None => {}
        }
        if let (Some(keep), Some((id, name))) = (renamed, self.space_rename.clone()) {
            self.space_rename = None;
            if let Err(error) = keep.then(|| self.rename_space(&id, &name)).unwrap_or(Ok(())) {
                self.toast(error);
            }
        }
    }

    /// Novo espaço (name and root folder) and Remover espaço (confirmed: it ends that space's shells).
    fn space_dialog_ui(&mut self, ctx: &Context, c: &UiColors) {
        match self.space_dialog.take() {
            None => {}
            Some(SpaceDialog::New { mut name, mut folder }) => {
                let folders = &mut self.prefs.folders;
                let (ok, cancel) = dialog_ui(ctx, c, "Novo espaço", "Criar", |ui| {
                    dialog_field(ui, "Nome", &mut name, "Projeto", DIALOG_WIDTH, c);
                    dialog_folder(ui, "Pasta raiz", &mut folder, "~/projetos/loja", "Pasta raiz do espaço", c, folders);
                    dialog_note(ui, "Terminais e IDEs novos deste espaço começam nessa pasta.", c);
                });
                if cancel {
                    return;
                }
                if !ok {
                    self.space_dialog = Some(SpaceDialog::New { name, folder });
                    return;
                }
                match self.create_space(&name, &folder) {
                    Ok(id) => {
                        self.prefs.folders.remember(&folder);
                        if let Err(error) = self.switch_space(&id) {
                            self.toast(error);
                        }
                    }
                    Err(error) => {
                        self.toast(error);
                        self.space_dialog = Some(SpaceDialog::New { name, folder });
                    }
                }
            }
            Some(SpaceDialog::Remove(id)) => {
                let Some(space) = self.spaces.list.iter().find(|w| w.id == id).cloned() else { return };
                let running = self.space_counts().get(&id).map_or(0, |n| n.0);
                let ends = match running {
                    0 => String::new(),
                    1 => " e o terminal que roda nele é encerrado".into(),
                    n => format!(" e os {n} terminais que rodam nele são encerrados"),
                };
                let text = format!("\"{}\" sai da lista{ends}. O desenho dele continua no disco ({}).", space.name, space.scene);
                let (ok, cancel) = dialog_ui(ctx, c, "Remover espaço", "Remover", |ui| {
                    ui.label(egui::RichText::new(text).size(13.5).color(c.text));
                });
                if ok {
                    match self.remove_space(&id) {
                        Ok(message) | Err(message) => self.toast(message),
                    }
                } else if !cancel {
                    self.space_dialog = Some(SpaceDialog::Remove(id));
                }
            }
        }
    }
}
