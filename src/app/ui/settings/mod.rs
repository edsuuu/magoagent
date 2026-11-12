//! Ajustes (⌘,) in the reference app's Configurações window: a 760 x 640 sheet over a veil, the
//! sections as tabs along the top, each with its groups of rows. The controls are in `kit`; the
//! panels by subject in `general`, `terminal`, `agents`, `voice` and `data`.

use crate::prelude::*;

mod about;
mod agents;
mod data;
mod general;
pub(in crate::app::ui) mod kit;
mod terminal;
#[cfg(test)]
mod tests;
mod voice;

use kit::K;

pub(crate) const SETTINGS_TABS: [&str; 7] = ["Geral", "Terminal", "Agentes", "Voz", "Atalhos", "Dados", "Sobre"];
const TAB_ICONS: [Icon; 7] = [Icon::Gear, Icon::CodeBlock, Icon::Agents, Icon::Composer, Icon::Menu, Icon::Open, Icon::Eye];
/// Tabs kept in the code but not shown (the indexes of the others do not move).
const HIDDEN_TABS: [&str; 1] = ["Voz"];
pub(crate) const LANGUAGES: [&str; 3] = ["Automático", "Português", "English"];

/// The sheet: at most 760 x 640 (`max-w-[760px] h-[640px] max-h-[86vh]`), a pad of 26 (`p-4`) to the window.
const SHEET: Vec2 = Vec2::new(760.0, 640.0);
const COLUMN: f32 = 700.0;
/// Where the tabs sit (`pt-3`) and how tall they are, and the sheet's rounding (`rounded-xl`).
const TABS_TOP: f32 = 9.75;
const TAB_HEIGHT: f32 = 34.25;
const SHEET_RADIUS: f32 = 12.125;
/// How long the veil fades and the sheet grows in.
const OPEN_TIME: f64 = 0.16;

/// The chat's agent run on an API key instead of its own login: the variable and the key, when
/// the person turned that on and typed one.
pub(crate) fn api_env(prefs: &Prefs, agent: &str) -> Option<(&'static str, String)> {
    if !prefs.use_api_keys {
        return None;
    }
    let (name, key) = if agent == "codex" { ("OPENAI_API_KEY", &prefs.openai_key) } else { ("ANTHROPIC_API_KEY", &prefs.anthropic_key) };
    let key = key.trim();
    (!key.is_empty()).then(|| (name, key.to_string()))
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
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape)) && !egui::Popup::is_any_open(ctx);

        // the veil (`bg-black/30`): a click on it, outside the sheet, closes
        let veil = egui::Area::new(egui::Id::unique("settings_veil")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0, Color32::from_black_alpha((77.0 * eased) as u8));
            response
        });
        if veil.inner.clicked() && !ctx.pointer_interact_pos().is_some_and(|p| sheet.contains(p)) {
            close = true;
        }

        let area = egui::Area::new(egui::Id::unique("settings")).order(egui::Order::Foreground).fixed_pos(sheet.min).show(ctx, |ui| {
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
        let mut x = sheet.center().x - total.min(COLUMN) / 2.0;
        for (&i, width) in shown.iter().zip(&widths) {
            let name = &SETTINGS_TABS[i];
            let tab = Rect::from_min_size(Pos2::new(x, top), Vec2::new(*width, TAB_HEIGHT));
            let response = ui.interact(tab, ui.scope_id().with(("settings_tab", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Ajustes: {name}")));
            let chosen = self.settings_tab == i;
            let color = if chosen || response.hovered() { k.fg } else { k.muted_fg };
            if chosen {
                ui.painter().rect_filled(Rect::from_min_size(Pos2::new(tab.left(), tab.bottom() - 2.0), Vec2::new(tab.width(), 2.0)), 0, k.primary);
            }
            paint_icon_scaled(ui.painter(), TAB_ICONS[i], Pos2::new(tab.left() + 6.5 + 6.5, tab.top() + 15.7), color, 13.0 / 16.0, 0.0);
            ui.painter().text(Pos2::new(tab.left() + 6.5 + 13.0 + 3.25, tab.top() + 15.7), egui::Align2::LEFT_CENTER, name, font.clone(), color);
            if response.clicked() && !chosen {
                self.settings_tab = i;
                self.settings_edit = None;
            }
            x += width + 1.625;
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
                    match tab {
                        0 => self.settings_general(ui, k),
                        1 => self.settings_terminal(ui, k),
                        2 => self.settings_agents(ui, k),
                        3 => self.settings_voice(ui, k),
                        4 => data::settings_shortcuts(ui, k),
                        5 => data::settings_data(ui, k),
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
