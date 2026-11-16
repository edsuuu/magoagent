//! Ajustes (⌘,): an 880 wide window over a veil, the pages in a sidebar at its left, each with its name, a line on
//! what it holds and its groups of rows in cards. The controls are in `kit`; the
//! panels by subject in `general`, `terminal`, `agents`, `voice` (with `studio`), `integrations` (with `hooks`) and `data`.

use crate::prelude::*;

mod about;
mod agents;
mod chat;
mod data;
mod general;
mod hooks;
mod hooks_preview;
mod hotkeys;
mod integrations;
pub(in crate::app::ui) mod kit;
mod ok_mago;
mod studio;
mod terminal;
#[cfg(test)]
mod tests;
mod voice;

use kit::K;

pub(crate) const SETTINGS_TABS: [&str; 10] = ["Geral", "Terminal", "Agentes", "Chat", "Voz", "Ok Mago", "Integrações", "Atalhos", "Dados", "Sobre"];
const TAB_ICONS: [Icon; 10] =
    [Icon::Gear, Icon::CodeBlock, Icon::Agents, Icon::Tool(Tool::Chat), Icon::Composer, Icon::Palette, Icon::Connect, Icon::Menu, Icon::Open, Icon::Eye];
/// Tabs kept in the code but not shown (the indexes of the others do not move).
const HIDDEN_TABS: [&str; 0] = [];
pub(crate) const LANGUAGES: [&str; 3] = ["Automático", "Português", "English"];

/// The window: 880 wide and at most 720 high (86% of the screen), radius 14; the sidebar is 200 wide.
const SHEET: Vec2 = Vec2::new(880.0, 720.0);
const SHEET_RADIUS: u8 = 14;
/// How far in from the sheet's right edge and from its top and bottom the page scrolls (past the corners' curve).
const SCROLL_INSET: Vec2 = Vec2::new(2.0, 10.0);
const SIDEBAR: f32 = 200.0;
/// The main area's padding (28 above and below, 32 at the sides).
const MAIN_TOP: f32 = 28.0;
const MAIN_SIDE: f32 = 32.0;
/// What each page says under its name.
const SUBTITLES: [&str; 10] = [
    "Aparência, como o app abre, a ilha e o canvas.",
    "Os atalhos de início, a aparência e o shell dos terminais.",
    "O que cada agente gastou e o que segura o Maestro.",
    "Como um chat novo começa: agente, pasta, permissão e estilo.",
    "Quem ouve e fala na chamada com o Maestro e no Ditar.",
    "Chamar o Maestro pela voz no meio da chamada.",
    "Os hooks das sessões de fora e os serviços mostrados na ilha. Segredos vão para o cofre do sistema.",
    "Os da ilha valem com qualquer programa na frente. Clique nas teclas para trocar.",
    "Onde ficam os desenhos do canvas, os dados do app e os chats, neste computador.",
    "A versão, as novidades, a atualização e a privacidade.",
];
/// How long the veil fades and the sheet grows in.
const OPEN_TIME: f64 = 0.16;

impl DrawingApp {
    /// Ajustes opened on a page by its name (the usage pill's "Configurações…" on Agentes, the island's on Geral).
    pub(crate) fn open_settings_at(&mut self, tab: &str) {
        self.settings_open = true;
        self.settings_tab = SETTINGS_TABS.iter().position(|t| *t == tab).unwrap_or(0);
    }
}

/// Ajustes' veil: the whole window, the drawn title bar too (`caption_drag` lets the bar's stretches under it move the
/// window).
pub(crate) fn settings_veil() -> egui::Id {
    egui::Id::unique("settings_veil")
}

/// How far into its opening the sheet is, 0 to 1: it starts over whenever Ajustes was not drawn
/// in the pass before.
fn opening(ctx: &Context) -> f32 {
    let (start, seen) = (egui::Id::unique("settings_start"), egui::Id::unique("settings_seen"));
    let (now, pass) = (ctx.input(|i| i.time), ctx.cumulative_pass_nr());
    let t = ctx.data_mut(|d| {
        // drawn the pass before: still the same opening; a gap: it was closed in between
        if d.get_temp::<u64>(seen).is_none_or(|s| pass - s > 1) {
            d.insert_temp(start, now);
        }
        d.insert_temp(seen, pass);
        ((now - d.get_temp::<f64>(start).unwrap_or(now)) / OPEN_TIME).clamp(0.0, 1.0) as f32
    });
    if t < 1.0 {
        ctx.request_repaint();
    }
    t
}

impl DrawingApp {
    pub(crate) fn settings_ui(&mut self, ctx: &Context) {
        let k = K::with_cards(self.dark());
        let t = opening(ctx);
        let eased = 1.0 - (1.0 - t).powi(3);
        let screen = ctx.content_rect();
        let size = Vec2::new(SHEET.x.min(screen.width() - 26.0), SHEET.y.min(screen.height() * 0.86));
        let sheet = Rect::from_center_size(screen.center(), size);
        // Esc while a global shortcut is being recorded only ends the recording (settings/hotkeys.rs); with a question
        // over the sheet (the hooks' preview), it is the question's
        let mut close =
            ctx.input(|i| i.key_pressed(Key::Escape)) && !egui::Popup::is_any_open(ctx) && self.hotkeys.recording.is_none() && !self.dialog_over_settings();

        // the veil (`bg-black/30`): a click on it, outside the sheet, closes
        // not movable: egui brings a movable area to the top when it is pressed or dragged, and a veil over the
        // sheet takes every click (the sheet's buttons stopped answering)
        let veil = egui::Area::new(settings_veil()).order(egui::Order::Foreground).fixed_pos(screen.min).movable(false).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0, Color32::from_black_alpha((77.0 * eased) as u8));
            response
        });
        if veil.inner.clicked() && !ctx.pointer_interact_pos().is_some_and(|p| sheet.contains(p)) {
            close = true;
        }

        let area = egui::Area::new(egui::Id::unique("settings")).order(egui::Order::Foreground).fixed_pos(sheet.min).movable(false).show(ctx, |ui| {
            ui.set_opacity(eased);
            // takes the clicks the sheet's blank parts get, so the veil never sees them
            ui.interact(sheet, ui.scope_id().with("sheet"), Sense::click());
            // no shadow: the owner wants every surface flat
            ui.painter().rect(sheet, SHEET_RADIUS, k.bg, Stroke::new(1.0, k.hairline), StrokeKind::Inside);
            self.settings_sidebar(ui, &k, sheet);
            // the page's scroll area inside the sheet's rounded corners and edge: its bar ran out of the sheet
            let main = Rect::from_min_max(Pos2::new(sheet.left() + SIDEBAR, sheet.top() + SCROLL_INSET.y), sheet.max - SCROLL_INSET);
            ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| {
                ui.set_clip_rect(main.intersect(ui.clip_rect()));
                self.settings_content(ui, &k);
            });
            close |= settings_close(ui, &k, sheet);
        });
        // the sheet grows from 96% about its middle
        let scale = 0.96 + 0.04 * eased;
        let transform = egui::emath::TSTransform::new(sheet.center().to_vec2() * (1.0 - scale), scale);
        ctx.set_transform_layer(area.response.layer_id, transform);
        if t == 0.0 {
            ctx.move_to_top(veil.response.layer_id);
        }
        // the sheet over everything every frame (anything shown later, a press on the veil), as the dialogs do;
        // not while a menu of its own is open (a select): that one is newer and stays above it. Nor under a question
        // of its own (`dialog_over`): asking too, the sheet would keep whatever order it had over the question
        let covered = self.dialog_over_settings();
        if (t == 0.0 || !egui::Popup::is_any_open(ctx)) && !covered {
            ctx.move_to_top(area.response.layer_id);
        }
        if close {
            self.settings_open = false;
            self.settings_edit = None;
        }
    }

    /// A question shown over Ajustes (`dialog_over`): the hooks' preview.
    pub(crate) fn dialog_over_settings(&self) -> bool {
        self.claude_hooks_preview.is_some()
    }

    /// The sidebar (#111215, a line at its right): the brand, then a row for each page, icon and name, the chosen one
    /// lit.
    fn settings_sidebar(&mut self, ui: &mut egui::Ui, k: &K, sheet: Rect) {
        let side = Rect::from_min_size(sheet.min, Vec2::new(SIDEBAR, sheet.height()));
        let radius = egui::CornerRadius { nw: SHEET_RADIUS, sw: SHEET_RADIUS, ne: 0, se: 0 };
        ui.painter().rect_filled(side.shrink(0.5), radius, k.well);
        ui.painter().vline(side.right(), side.y_range(), Stroke::new(1.0, k.hairline));
        // the brand: a 26 square in the app's grey (the owner wants no purple), and the name
        let mark = Rect::from_min_size(Pos2::new(side.left() + 12.0 + 10.0, side.top() + 20.0 + 4.0), Vec2::splat(26.0));
        ui.painter().rect(mark, 7, k.segment, Stroke::new(1.0, k.hairline), StrokeKind::Inside);
        paint_icon_scaled(ui.painter(), Icon::Gear, mark.center(), k.fg, 14.0 / 16.0, 0.0);
        ui.painter().text(Pos2::new(mark.right() + 10.0, mark.center().y), egui::Align2::LEFT_CENTER, "Ajustes", kit::semibold(14.0), k.fg);
        let font = FontId::proportional(kit::BODY);
        let mut top = mark.bottom() + 18.0;
        for (i, name) in SETTINGS_TABS.iter().enumerate().filter(|(_, name)| !HIDDEN_TABS.contains(name)) {
            let item = Rect::from_min_size(Pos2::new(side.left() + 12.0, top), Vec2::new(SIDEBAR - 24.0, 34.0));
            top += 34.0 + 2.0;
            let response = ui.interact(item, ui.scope_id().with(("settings_tab", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Ajustes: {name}")));
            let chosen = self.settings_tab == i;
            if chosen || response.hovered() {
                ui.painter().rect_filled(item, 8, if chosen { k.chosen } else { k.chosen.gamma_multiply(0.5) });
            }
            let color = if chosen { k.fg } else { k.soft_fg };
            paint_icon(ui.painter(), TAB_ICONS[i], Pos2::new(item.left() + 10.0 + 8.0, item.center().y), color);
            ui.painter().text(Pos2::new(item.left() + 10.0 + 16.0 + 10.0, item.center().y), egui::Align2::LEFT_CENTER, name, font.clone(), color);
            if response.clicked() && !chosen {
                self.settings_tab = i;
                self.settings_edit = None;
            }
        }
    }

    /// The page: its name and what it holds, then its groups, in a scroll area with the column inside the main
    /// area's padding (the cards reach 16 past it).
    fn settings_content(&mut self, ui: &mut egui::Ui, k: &K) {
        let tab = self.settings_tab.min(SETTINGS_TABS.len() - 1);
        // the bar sits in the page's right margin, which already keeps the column away from it
        ui.spacing_mut().scroll.floating_allocated_width = 0.0;
        egui::ScrollArea::vertical().id_salt(("settings_scroll", tab)).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.25);
            ui.horizontal_top(|ui| {
                ui.add_space(MAIN_SIDE + kit::INSET);
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - MAIN_SIDE - kit::INSET).min(kit::COLUMN));
                    ui.add_space(MAIN_TOP - SCROLL_INSET.y);
                    page_header(ui, k, SETTINGS_TABS[tab], SUBTITLES[tab]);
                    match SETTINGS_TABS[tab] {
                        "Geral" => self.settings_general(ui, k),
                        "Terminal" => self.settings_terminal(ui, k),
                        "Agentes" => self.settings_agents(ui, k),
                        "Chat" => self.settings_chat(ui, k),
                        "Voz" => self.settings_voice(ui, k),
                        "Ok Mago" => self.settings_ok_mago(ui, k),
                        "Integrações" => self.settings_integrations(ui, k),
                        "Atalhos" => self.settings_hotkeys(ui, k),
                        "Dados" => self.settings_data(ui, k),
                        _ => self.settings_about(ui, k),
                    }
                    kit::close_group(ui, k);
                    ui.add_space(MAIN_TOP - SCROLL_INSET.y);
                });
            });
        });
    }
}

/// The page's name (20, weight 600) and the line under it (12.5, muted), from the cards' edge, and the gap after.
fn page_header(ui: &mut egui::Ui, k: &K, name: &str, subtitle: &str) {
    let left = ui.cursor().left() - kit::INSET;
    let name = ui.painter().layout_no_wrap(name.to_string(), kit::semibold(20.0), k.fg);
    let line = kit::galley(ui, subtitle, 12.5, 17.0, k.muted_fg, ui.available_width() + kit::INSET - 40.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), name.size().y + 4.0 + line.size().y), Sense::hover());
    let below = rect.top() + name.size().y + 4.0;
    ui.painter().galley(Pos2::new(left, rect.top()), name, k.fg);
    ui.painter().galley(Pos2::new(left, below), line, k.muted_fg);
    ui.add_space(kit::GROUP_GAP - 3.25);
}

/// The X: 28 square, radius 8, 18 from the window's top right corner. True when it was pressed.
fn settings_close(ui: &mut egui::Ui, k: &K, sheet: Rect) -> bool {
    let x = Rect::from_min_size(Pos2::new(sheet.right() - 18.0 - 28.0, sheet.top() + 18.0), Vec2::splat(28.0));
    let response = ui.interact(x, ui.scope_id().with("settings_close"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Fechar"));
    if response.hovered() {
        ui.painter().rect_filled(x, 8, k.chosen);
    }
    let ink = if response.hovered() { k.fg } else { k.muted_fg };
    for (a, b) in [(Vec2::new(-4.5, -4.5), Vec2::new(4.5, 4.5)), (Vec2::new(-4.5, 4.5), Vec2::new(4.5, -4.5))] {
        ui.painter().line_segment([x.center() + a, x.center() + b], Stroke::new(1.5, ink));
    }
    response.clicked()
}
