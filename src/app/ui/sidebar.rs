//! The workspaces' sidebar, the window's left column in the reference app's look: Filtrar and +
//! (a new space), then each space with its color, its name and its agents (running, needing
//! you), and Ajustes' gear at the foot. A click switches; ⋯ or a right click renames, recolors or
//! removes. ☰ in the window's top row folds it and brings it back (ui/titlebar.rs).

use crate::app::workspaces::SpaceDialog;
use crate::app::workspaces::sidebar::{Picks, SpaceMenu};
use crate::prelude::*;

/// What the open sidebar takes from the canvas's left.
pub(crate) const SIDEBAR: f32 = 260.0;
/// Its foot: a line, and Ajustes' gear at the right.
const FOOT: f32 = 30.0;

/// How far along the sidebar is `t` seconds after it started to open or fold: the reference
/// app's spring (stiffness 380, damping 36, mass 0.9: all but critically damped, so it eases in
/// without a bounce and is there in under half a second).
fn slide(t: f32) -> f32 {
    let w = 20.55 * t.max(0.0);
    if t >= 0.5 { 1.0 } else { 1.0 - (1.0 + w) * (-w).exp() }
}

/// The sidebar's tones (the reference app's `--sidebar-*` and muted text).
pub(crate) struct Tones {
    pub(crate) ground: Color32,
    pub(crate) hover: Color32,
    pub(crate) active: Color32,
    pub(crate) line: Color32,
    pub(crate) text: Color32,
    pub(crate) muted: Color32,
}

fn tones(dark: bool) -> Tones {
    if dark {
        Tones {
            ground: Color32::from_rgb(0x2c, 0x2c, 0x2e),
            hover: Color32::from_white_alpha(0x0f),
            active: Color32::from_white_alpha(0x1c),
            line: Color32::from_white_alpha(0x1a),
            text: Color32::from_rgb(0xf5, 0xf5, 0xf7),
            muted: Color32::from_rgba_unmultiplied(0xeb, 0xeb, 0xf5, 0x99),
        }
    } else {
        Tones {
            ground: Color32::from_rgb(0xe9, 0xe9, 0xeb),
            hover: Color32::from_black_alpha(0x0d),
            active: Color32::from_black_alpha(0x14),
            line: Color32::from_black_alpha(0x1a),
            text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
            muted: Color32::from_rgba_unmultiplied(0x3c, 0x3c, 0x43, 0x99),
        }
    }
}

impl DrawingApp {
    /// Where the sidebar comes to rest: none folded or under the chat.
    fn sidebar_rest(&self) -> f32 {
        if self.chat_open || self.prefs.sidebar_hidden { 0.0 } else { SIDEBAR }
    }

    /// The canvas's left strip the sidebar keeps, sliding between open and folded. A change no
    /// frame has drawn yet is already where it rests.
    pub(crate) fn sidebar_width(&self) -> f32 {
        let (start, from, toward) = self.sidebar_slide;
        let rest = self.sidebar_rest();
        if toward == rest { from + (rest - from) * slide((self.now - start) as f32) } else { rest }
    }

    pub(crate) fn sidebar(&mut self, ctx: &Context) {
        self.load_spaces();
        let c = self.colors();
        // a change of where it rests starts the slide from where it stands (the first frame has it there)
        let rest = self.sidebar_rest();
        let (start, from, toward) = self.sidebar_slide;
        if toward != rest {
            let stands = if toward < 0.0 { rest } else { from + (toward - from) * slide((self.now - start) as f32) };
            self.sidebar_slide = (self.now, stands, rest);
        }
        let width = self.sidebar_width();
        if width != rest {
            ctx.request_repaint();
        }
        if width > 0.0 {
            self.sidebar_panel(ctx, &c, width - SIDEBAR);
        }
        self.space_dialog_ui(ctx, &c);
    }

    /// Where Ajustes' gear sits, in the foot of a sidebar `height` tall.
    pub(crate) fn sidebar_gear(height: f32) -> Rect {
        Rect::from_center_size(Pos2::new(SIDEBAR - 18.0, height - FOOT / 2.0), Vec2::splat(20.0))
    }

    /// The panel, `left` off the window's edge while it slides (negative, none once open).
    fn sidebar_panel(&mut self, ctx: &Context, c: &UiColors, left: f32) {
        let t = tones(self.dark());
        let height = ctx.content_rect().height();
        let wanted = self.space_filter.trim().to_lowercase();
        let spaces: Vec<_> = self.spaces.list.iter().filter(|w| w.name.to_lowercase().contains(&wanted)).cloned().collect();
        // the window's own buttons sit over its top on the Mac
        let top = if TITLE_BAR > 0.0 { 12.0 } else { LIGHTS_Y + 22.0 };
        let (mut picks, mut new, mut new_group, mut settings) = (Default::default(), false, false, false);
        egui::Area::new(egui::Id::unique("sidebar")).fixed_pos(Pos2::new(left, 0.0)).constrain(false).order(egui::Order::Foreground).show(ctx, |ui| {
            let (panel, _) = ui.allocate_exact_size(Vec2::new(SIDEBAR, height), Sense::click());
            let p = ui.painter().clone();
            p.rect_filled(panel, 0, t.ground);
            p.vline(panel.right() - 0.5, panel.y_range(), Stroke::new(1.0, t.line));
            // Filtrar, and + beside it (the reference app's measures: 10 in, 32 tall, the + 8 past it)
            let field = Rect::from_min_max(Pos2::new(left + 10.0, top), Pos2::new(left + 209.0, top + 32.0));
            p.rect_filled(field, 16, t.hover);
            paint_icon_scaled(&p, Icon::Search, Pos2::new(field.left() + 16.0, field.center().y), t.muted, 0.75, 0.0);
            let text = Rect::from_min_max(Pos2::new(field.left() + 28.0, field.center().y - 10.0), Pos2::new(field.right() - 10.0, field.center().y + 10.0));
            let filter = egui::TextEdit::singleline(&mut self.space_filter)
                .hint_text(egui::RichText::new("Filtrar").color(t.muted))
                .font(FontId::proportional(13.0))
                .text_color(t.text)
                .frame(egui::Frame::NONE);
            ui.put(text, filter);
            let plus = Rect::from_min_size(Pos2::new(left + 217.0, top), Vec2::splat(32.0));
            let button = ui.interact(plus, egui::Id::unique("space_new"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if button.hovered() {
                p.circle_filled(plus.center(), 16.0, t.text.gamma_multiply(0.1));
            }
            paint_icon_scaled(&p, Icon::Plus, plus.center(), if button.hovered() { t.text } else { t.text.gamma_multiply(0.7) }, GLASS_ICON, 0.0);
            new = button.tip("Novo espaço de trabalho").clicked();
            p.text(Pos2::new(left + 13.0, top + 48.0), egui::Align2::LEFT_TOP, "ESPAÇOS DE TRABALHO", FontId::proportional(11.0), t.muted);
            // the spaces, down to the foot
            let list = Rect::from_min_max(Pos2::new(left + 5.0, top + 71.0), Pos2::new(left + SIDEBAR - 5.0, height - FOOT - 4.0));
            picks = self.sidebar_list(ui, ctx, c, &t, list, &spaces);
            // the foot: a line, and Ajustes' gear
            p.hline(panel.x_range(), height - FOOT + 0.5, Stroke::new(1.0, t.line));
            let group = Rect::from_min_size(Pos2::new(left + 8.0, height - 24.0), Vec2::new(75.0, 17.0));
            let button = ui.interact(group, egui::Id::unique("space_new_group"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let ink = if button.hovered() { t.text } else { t.muted };
            paint_icon_scaled(&p, Icon::Open, Pos2::new(group.left() + 6.0, group.center().y), ink, 0.7, 0.0);
            p.text(Pos2::new(group.left() + 17.0, group.center().y), egui::Align2::LEFT_CENTER, "Novo grupo", FontId::proportional(11.0), ink);
            new_group = button.tip("Novo grupo").clicked();
            let gear = Self::sidebar_gear(height).translate(Vec2::new(left, 0.0));
            let button = ui.interact(gear, egui::Id::unique("sidebar_settings"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let lit = self.settings_open || button.hovered();
            if lit {
                p.rect_filled(gear, 5, t.hover);
            }
            paint_icon_scaled(&p, Icon::Gear, gear.center(), if lit { t.text } else { t.muted }, 0.75, 0.0);
            settings = button.tip(keys_label("Ajustes — ⌘,")).clicked();
        });
        if settings {
            self.settings_open = !self.settings_open;
        }
        if new {
            self.space_dialog = Some(SpaceDialog::New { name: String::new(), folder: String::new() });
        }
        if new_group {
            self.space_dialog = Some(SpaceDialog::NewGroup { name: self.new_group_name() });
        }
        let Picks { pick, action, renamed, group } = picks;
        if let Some(Err(error)) = pick.map(|id| self.switch_space(&id)) {
            self.toast(error);
        }
        if let Some((id, menu)) = group {
            self.group_act(&id, menu);
        }
        match action {
            Some((id, SpaceMenu::Rename)) => self.space_rename = Some((id.clone(), self.space_name(&id))),
            Some((id, SpaceMenu::Color(color))) => self.color_space(&id, color),
            Some((_, SpaceMenu::Remove)) if self.spaces.list.len() <= 1 => self.toast("O último espaço não pode ser removido."),
            Some((id, SpaceMenu::Remove)) => self.space_dialog = Some(SpaceDialog::Remove(id)),
            Some((id, menu)) => self.space_menu_act(ctx, &id, menu),
            None => {}
        }
        if let (Some(keep), Some((id, name))) = (renamed, self.space_rename.clone()) {
            self.space_rename = None;
            if keep && let Err(error) = self.rename_space(&id, &name) {
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
                    dialog_note(ui, "Terminais e árvores novos deste espaço começam nessa pasta.", c);
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
            Some(dialog @ (SpaceDialog::NewGroup { .. } | SpaceDialog::RenameGroup { .. } | SpaceDialog::RemoveGroup(_))) => {
                self.group_dialog_ui(ctx, c, dialog)
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
