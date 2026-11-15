//! Ajustes (⌘,) in the reference app's Configurações window: a 760 x 640 sheet over a veil, the
//! sections as tabs along the top, each with its groups of rows. The controls are in `kit`; the
//! panels by subject in `general`, `terminal`, `agents`, `voice` (with `studio`) and `data`.

use crate::prelude::*;

mod about;
mod agents;
mod chat;
mod data;
mod dictionary;
mod email;
mod general;
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

pub(crate) const SETTINGS_TABS: [&str; 12] =
    ["Geral", "Terminal", "Agentes", "Chat", "Voz", "Ok Mago", "Integrações", "Email", "Dicionário", "Atalhos", "Dados", "Sobre"];
const TAB_ICONS: [Icon; 12] = [
    Icon::Gear,
    Icon::CodeBlock,
    Icon::Agents,
    Icon::Tool(Tool::Chat),
    Icon::Composer,
    Icon::Palette,
    Icon::Connect,
    Icon::Export,
    Icon::Heading,
    Icon::Menu,
    Icon::Open,
    Icon::Eye,
];
/// Tabs kept in the code but not shown (the indexes of the others do not move).
const HIDDEN_TABS: [&str; 0] = [];
pub(crate) const LANGUAGES: [&str; 3] = ["Automático", "Português", "English"];

/// The sheet: at most 760 x 640 (`max-w-[760px] h-[640px] max-h-[86vh]`), a pad of 26 (`p-4`) to the window.
const SHEET: Vec2 = Vec2::new(760.0, 640.0);
/// Where the tabs sit (`pt-3`) and how tall they are, and the sheet's rounding (`rounded-xl`).
const TABS_TOP: f32 = 9.75;
const TAB_HEIGHT: f32 = 34.25;
const SHEET_RADIUS: f32 = 12.125;
/// Room left of the tab strip and right of it (the X), for the scroll arrows.
const STRIP_SIDE: f32 = 26.0;
const X_SIDE: f32 = 44.0;
/// How long the veil fades and the sheet grows in.
const OPEN_TIME: f64 = 0.16;

impl DrawingApp {
    /// Ajustes opened on a tab and, inside Agentes, on one of its sub-tabs (the usage pill's "Configurações…").
    pub(crate) fn open_settings_at(&mut self, ctx: &Context, tab: &str, sub: &str) {
        self.settings_open = true;
        self.settings_tab = SETTINGS_TABS.iter().position(|t| *t == tab).unwrap_or(0);
        if let Some(i) = agents::AGENT_TABS.iter().position(|t| *t == sub) {
            ctx.data_mut(|d| d.insert_temp(egui::Id::unique(("settings_sub", "agents")), i));
        }
    }
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
        let k = K::new(self.dark());
        let t = opening(ctx);
        let eased = 1.0 - (1.0 - t).powi(3);
        let screen = ctx.content_rect();
        let size = Vec2::new(SHEET.x.min(screen.width() - 26.0), SHEET.y.min(screen.height() * 0.86).min(screen.height() - 26.0));
        let sheet = Rect::from_center_size(screen.center(), size);
        // Esc while a global shortcut is being recorded only ends the recording (settings/hotkeys.rs)
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape)) && !egui::Popup::is_any_open(ctx) && self.hotkeys.recording.is_none();

        // the veil (`bg-black/30`): a click on it, outside the sheet, closes
        // not movable: egui brings a movable area to the top when it is pressed or dragged, and a veil over the
        // sheet takes every click (the sheet's buttons stopped answering)
        let veil = egui::Area::new(egui::Id::unique("settings_veil")).order(egui::Order::Foreground).fixed_pos(screen.min).movable(false).show(ctx, |ui| {
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
            let p = ui.painter().clone();
            // no shadow: the owner wants every surface flat
            p.rect_filled(sheet, SHEET_RADIUS, k.bg);
            close |= self.settings_tabs(ui, &k, sheet);
            let content = Rect::from_min_max(Pos2::new(sheet.left(), sheet.top() + TABS_TOP + TAB_HEIGHT), sheet.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
                ui.set_clip_rect(content.intersect(ui.clip_rect()));
                self.settings_content(ui, &k);
            });
        });
        // the sheet grows from 96% about its middle
        let scale = 0.96 + 0.04 * eased;
        let transform = egui::emath::TSTransform::new(sheet.center().to_vec2() * (1.0 - scale), scale);
        ctx.set_transform_layer(area.response.layer_id, transform);
        if t == 0.0 {
            ctx.move_to_top(veil.response.layer_id);
        }
        // the sheet over everything every frame (anything shown later, a press on the veil), as the dialogs do;
        // not while a menu of its own is open (a select): that one is newer and stays above it
        if t == 0.0 || !egui::Popup::is_any_open(ctx) {
            ctx.move_to_top(area.response.layer_id);
        }
        if close {
            self.settings_open = false;
            self.settings_edit = None;
        }
    }

    /// The tab row (icon and name, the chosen one underlined in the accent), the line under it
    /// and the X. True when the X was pressed.
    fn settings_tabs(&mut self, ui: &mut egui::Ui, k: &K, sheet: Rect) -> bool {
        let font = FontId::proportional(kit::BODY);
        let shown: Vec<usize> = (0..SETTINGS_TABS.len()).filter(|i| !HIDDEN_TABS.contains(&SETTINGS_TABS[*i])).collect();
        let widths: Vec<f32> =
            shown.iter().map(|i| 13.0 + 3.25 + ui.painter().layout_no_wrap(SETTINGS_TABS[*i].to_string(), font.clone(), k.fg).size().x + 13.0).collect();
        let total = widths.iter().sum::<f32>() + 1.625 * (widths.len() as f32 - 1.0);
        let top = sheet.top() + TABS_TOP;
        // the strip lives between the sheet's edge and the X; with more tabs than fit it scrolls (wheel, or the arrows)
        let strip = Rect::from_min_max(Pos2::new(sheet.left() + STRIP_SIDE, top), Pos2::new(sheet.right() - X_SIDE, top + TAB_HEIGHT));
        let furthest = (total - strip.width()).max(0.0);
        let offset_id = egui::Id::unique("settings_tabs_offset");
        let (mut offset, seen) = ui.ctx().data(|d| (d.get_temp::<f32>(offset_id).unwrap_or(0.0), d.get_temp::<usize>(offset_id.with("tab"))));
        if ui.rect_contains_pointer(strip) {
            let wheel = ui.input(|i| i.smooth_scroll_delta);
            offset -= if wheel.x.abs() > wheel.y.abs() { wheel.x } else { wheel.y };
        }
        let positions: Vec<f32> = widths.iter().scan(0.0, |at, width| Some(std::mem::replace(at, *at + width + 1.625))).collect();
        // the chosen tab comes into view when it changed (a click, or Ajustes opened on it)
        if seen != Some(self.settings_tab)
            && let Some(at) = shown.iter().position(|i| *i == self.settings_tab)
        {
            offset = offset.min(positions[at]).max(positions[at] + widths[at] - strip.width());
        }
        offset = offset.clamp(0.0, furthest);
        ui.ctx().data_mut(|d| {
            d.insert_temp(offset_id, offset);
            d.insert_temp(offset_id.with("tab"), self.settings_tab);
        });
        let centered = if furthest == 0.0 { (strip.width() - total) / 2.0 } else { 0.0 };
        let painter = ui.painter().with_clip_rect(strip);
        for (&i, (width, position)) in shown.iter().zip(widths.iter().zip(&positions)) {
            let name = &SETTINGS_TABS[i];
            let tab = Rect::from_min_size(Pos2::new(strip.left() + centered + position - offset, top), Vec2::new(*width, TAB_HEIGHT));
            if !tab.intersects(strip) {
                continue;
            }
            let response = ui.interact(tab.intersect(strip), ui.scope_id().with(("settings_tab", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Ajustes: {name}")));
            let chosen = self.settings_tab == i;
            let color = if chosen || response.hovered() { k.fg } else { k.muted_fg };
            if chosen {
                painter.rect_filled(Rect::from_min_size(Pos2::new(tab.left(), tab.bottom() - 2.0), Vec2::new(tab.width(), 2.0)), 0, k.primary);
            }
            paint_icon_scaled(&painter, TAB_ICONS[i], Pos2::new(tab.left() + 6.5 + 6.5, tab.top() + 15.7), color, 13.0 / 16.0, 0.0);
            painter.text(Pos2::new(tab.left() + 6.5 + 13.0 + 3.25, tab.top() + 15.7), egui::Align2::LEFT_CENTER, name, font.clone(), color);
            if response.clicked() && !chosen {
                self.settings_tab = i;
                self.settings_edit = None;
            }
        }
        // the arrows at the strip's ends, only while there is more that way
        for (more, sign, side) in [(offset > 0.0, -1.0_f32, strip.left() - STRIP_SIDE / 2.0), (offset < furthest, 1.0, strip.right() + X_SIDE / 2.0 - 12.0)] {
            if !more {
                continue;
            }
            let arrow = Rect::from_center_size(Pos2::new(side, top + TAB_HEIGHT / 2.0), Vec2::splat(22.0));
            let response =
                ui.interact(arrow, ui.scope_id().with(("settings_tabs_arrow", sign as i32)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, if sign < 0.0 { "Abas anteriores" } else { "Mais abas" }));
            let ink = if response.hovered() { k.fg } else { k.muted_fg };
            let tip = arrow.center() + Vec2::new(2.5 * sign, 0.0);
            for spread in [4.5, -4.5] {
                ui.painter().line_segment([tip, arrow.center() + Vec2::new(-2.5 * sign, spread)], Stroke::new(1.4, ink));
            }
            if response.clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(offset_id, (offset + sign * strip.width() * 0.6).clamp(0.0, furthest)));
            }
        }
        let line = sheet.top() + TABS_TOP + TAB_HEIGHT;
        ui.painter().hline(sheet.x_range(), line - 0.25, Stroke::new(1.0, k.line(0.6)));
        // the X: a ghost button 24 square, 9 from the corner
        let x = Rect::from_min_size(Pos2::new(sheet.right() - 9.0 - 24.0, sheet.top() + 9.0), Vec2::splat(24.0));
        let response = ui.interact(x, ui.scope_id().with("settings_close"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Fechar"));
        if response.hovered() {
            ui.painter().rect_filled(x, 8.125, k.muted.gamma_multiply(if k.dark { 0.5 } else { 1.0 }));
        }
        let ink = if response.hovered() { k.fg } else { k.muted_fg };
        for (a, b) in [(Vec2::new(-4.0, -4.0), Vec2::new(4.0, 4.0)), (Vec2::new(-4.0, 4.0), Vec2::new(4.0, -4.0))] {
            ui.painter().line_segment([x.center() + a, x.center() + b], Stroke::new(1.4, ink));
        }
        response.clicked()
    }

    /// The tab's panel, in a scroll area with the 668 column centered and `py-5` above and below.
    fn settings_content(&mut self, ui: &mut egui::Ui, k: &K) {
        let tab = self.settings_tab.min(SETTINGS_TABS.len() - 1);
        egui::ScrollArea::vertical().id_salt(("settings_scroll", tab)).auto_shrink([false, false]).show(ui, |ui| {
            let side = ((ui.available_width() - kit::COLUMN) / 2.0).max(0.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.25);
            ui.horizontal_top(|ui| {
                ui.add_space(side);
                ui.vertical(|ui| {
                    ui.set_width(kit::COLUMN.min(ui.available_width()));
                    ui.add_space(16.25);
                    match SETTINGS_TABS[tab] {
                        "Geral" => self.settings_general(ui, k),
                        "Terminal" => self.settings_terminal(ui, k),
                        "Agentes" => self.settings_agents(ui, k),
                        "Chat" => self.settings_chat(ui, k),
                        "Voz" => self.settings_voice(ui, k),
                        "Ok Mago" => self.settings_ok_mago(ui, k),
                        "Integrações" => self.settings_integrations(ui, k),
                        "Email" => self.settings_email(ui, k),
                        "Dicionário" => self.settings_dictionary(ui, k),
                        "Atalhos" => {
                            self.settings_hotkeys(ui, k);
                            data::settings_shortcuts(ui, k);
                        }
                        "Dados" => data::settings_data(ui, k),
                        _ => self.settings_about(ui, k),
                    }
                    ui.add_space(16.25);
                });
            });
        });
    }
}

/// The strip of subtabs a panel starts with (when it has them) and the gap under it; returns the
/// one chosen, kept between frames for each panel.
fn subtabs(ui: &mut egui::Ui, k: &K, panel: &str, labels: &[&str]) -> usize {
    let id = egui::Id::unique(("settings_sub", panel));
    let chosen = ui.ctx().data(|d| d.get_temp::<usize>(id)).unwrap_or(0).min(labels.len() - 1);
    let pick = kit::segmented_tabs(ui, k, id, labels, chosen);
    if pick != chosen {
        ui.ctx().data_mut(|d| d.insert_temp(id, pick));
    }
    ui.add_space(16.25 - 3.25);
    pick
}
