//! The floors of the space in use, in the canvas's bottom right row: the first pill of the row,
//! with the floor's name. A click opens the overview (the floors as cards; Ctrl+Shift+\), a right
//! click the menu to switch with Novo Andar; the sheets for a new, a renamed and a deleted floor;
//! and the floor rows and the Layers button of the workspaces' sidebar (`floors_toggle`, `floor_rows`).

use crate::app::floors::GROUND;
use crate::app::floors::fx::{ITEM, STAGGER, Spring};
use crate::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub(crate) enum FloorDialog {
    New { name: String, copy: bool },
    Rename { space: String, id: String, name: String },
    Delete { space: String, id: String },
}

/// What the pill, its menu and the floors' dialogs keep between frames.
#[derive(Default)]
pub(crate) struct FloorsUi {
    pub(crate) dialog: Option<FloorDialog>,
    pub(crate) overview: bool,
    /// What each floor held when the overview opened ("" the ground), for its cards.
    pub(crate) counts: HashMap<String, usize>,
    counted: bool,
    /// The overview's 3D stage (app/floors/fx.rs), and what it was painted for (the pictures of floors go stale with it).
    pub(crate) fx: crate::app::floors::fx::FloorFx,
    pub(crate) stale: (String, bool),
    /// How far each chip of the overview has popped in ("+" is Novo Andar), and whether the chips were up last frame.
    chips: HashMap<String, Spring>,
    chips_up: bool,
    /// The spaces whose floor rows the sidebar shows.
    // ponytail: not saved, the rows fold again on a restart; keep it in Prefs if that bothers
    shown: HashSet<String>,
}

impl FloorsUi {
    /// Whether a floors' surface takes the keys.
    pub(crate) fn is_open(&self) -> bool {
        self.dialog.is_some() || self.overview
    }
}

/// The sidebar's tones for a floor row, from its own `tones` (that file keeps them private).
pub(crate) struct RowInk {
    pub(crate) text: Color32,
    pub(crate) muted: Color32,
    pub(crate) hover: Color32,
    pub(crate) active: Color32,
}

/// What a floor's menu picked.
enum FloorPick {
    Switch(String),
    /// Show it and put the overview away (a double click on a chip).
    Commit(String),
    New,
    Rename(String),
    Up(String),
    Down(String),
    Delete(String),
}

/// A floor's menu: Renomear, Mover para cima, Mover para baixo, Excluir Andar, then Novo Andar (the ground has only that).
fn floor_menu(ui: &mut egui::Ui, id: &str, c: &UiColors) -> Option<FloorPick> {
    ui.set_width(200.0);
    let mut picked = None;
    if !id.is_empty() {
        if menu_item(ui, Icon::Edit, "Renomear andar", "", c.text, c) {
            picked = Some(FloorPick::Rename(id.into()));
        }
        if menu_item(ui, Icon::Backward, "Mover para cima", "", c.text, c) {
            picked = Some(FloorPick::Up(id.into()));
        }
        if menu_item(ui, Icon::Forward, "Mover para baixo", "", c.text, c) {
            picked = Some(FloorPick::Down(id.into()));
        }
        if menu_item(ui, Icon::Trash, "Excluir Andar", "", c.danger, c) {
            picked = Some(FloorPick::Delete(id.into()));
        }
        ui.separator();
    }
    if menu_item(ui, Icon::Layers(false), "Novo Andar", "", c.text, c) {
        picked = Some(FloorPick::New);
    }
    if picked.is_some() {
        ui.close();
    }
    picked
}

/// The outline of a stadium (a pill) `rect`, for a dashed border.
fn stadium(rect: Rect) -> Vec<Pos2> {
    let r = rect.height() / 2.0;
    let arc = |center: Pos2, from: f32| (0..=16).map(move |i| center + Vec2::angled(from + std::f32::consts::PI * i as f32 / 16.0) * r);
    let (left, right) = (Pos2::new(rect.left() + r, rect.center().y), Pos2::new(rect.right() - r, rect.center().y));
    let mut points: Vec<Pos2> = arc(right, -std::f32::consts::FRAC_PI_2).chain(arc(left, std::f32::consts::FRAC_PI_2)).collect();
    points.push(points[0]);
    points
}

/// "Andar vazio" or "N itens".
pub(crate) fn items_text(n: usize) -> String {
    match n {
        0 => "Andar vazio".into(),
        1 => "1 item".into(),
        n => format!("{n} itens"),
    }
}

impl DrawingApp {
    /// The pill, its right end `right` from the window's right edge (negative, as an anchor's
    /// offset). Gives how wide it drew: none while there is nothing to show.
    pub(crate) fn floors_pill(&mut self, ctx: &Context, right: f32) -> f32 {
        self.load_spaces();
        let (dark, c) = (self.dark(), self.colors());
        let ink = glass_ink(dark);
        let (has, open) = (!self.floors().is_empty(), self.floors_ui.overview);
        // the floor's name, "Andares" while the overview is open
        let (name, title) = (if open { "Andares".to_string() } else { self.floor_name() }, self.floor_name());
        // the button is 40 wide (more with a name beside the icon, up to 120 of it); the glass a hair around it
        let galley = ctx.layer_painter(egui::LayerId::background()).layout_no_wrap(name.clone(), FontId::proportional(10.0), ink);
        let label = if has { galley.size().x.min(120.0) } else { 0.0 };
        let button_w = if has { (16.0 + 13.0 + 4.0 + label).max(40.0) } else { 40.0 };
        let glass_w = button_w + 2.0;
        let shown = egui::Area::new(egui::Id::unique("floors_pill")).anchor(egui::Align2::RIGHT_BOTTOM, (right, -CORNER_GAP)).show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(glass_w, GLASS_HEIGHT), Sense::hover());
            paint_glass(ui.painter(), rect, GLASS_HEIGHT / 2.0, dark, false);
            let button = Rect::from_center_size(rect.center(), Vec2::new(button_w, 35.0));
            let response = ui.interact(button, ui.scope_id().with("floors_button"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if response.hovered() {
                ui.painter().rect_filled(button, 17, ink.gamma_multiply(0.1));
            }
            let tint = if open { glass_accent(dark) } else { ink.gamma_multiply(0.85) };
            let icon_at = if has { Pos2::new(button.left() + 8.0 + 6.5, button.center().y) } else { button.center() };
            paint_icon_scaled(ui.painter(), Icon::Layers(open), icon_at, tint, 0.85, 0.0);
            if has {
                let text = Rect::from_min_size(Pos2::new(button.left() + 8.0 + 13.0 + 4.0, button.top()), Vec2::new(label, button.height()));
                ui.painter().with_clip_rect(text).galley(Pos2::new(text.left(), text.center().y - galley.size().y / 2.0), galley, ink);
            }
            // the name of the button for the system's accessibility: "Andares: Térreo" (the tip is the title)
            let response = response.tip("Andares");
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Andares: {title}")));
            response
        });
        let response = shown.inner;
        if response.clicked() {
            self.floors_ui.overview = !self.floors_ui.overview;
        }
        let mut pick = None;
        let mut new = false;
        crate::app::ui::menu_look::context_menu(&response, |ui| {
            ui.set_width(200.0);
            let active = self.current_space().map(|w| w.floor.clone()).unwrap_or_default();
            let stops: Vec<(String, String)> =
                [(String::new(), GROUND.to_string())].into_iter().chain(self.floors().iter().map(|f| (f.id.clone(), f.name.clone()))).collect();
            for (i, (id, label)) in stops.iter().enumerate() {
                if i == 1 {
                    ui.separator();
                }
                if menu_item(ui, if *id == active { Icon::Check } else { Icon::Layers(false) }, label, "", c.text, &c) {
                    pick = Some(id.clone());
                    ui.close();
                }
            }
            ui.separator();
            if menu_item(ui, Icon::Plus, "Novo Andar", "", c.text, &c) {
                new = true;
                ui.close();
            }
        });
        if let Some(Err(error)) = pick.map(|id| self.switch_floor(&id)) {
            self.toast(error);
        }
        if new {
            self.floors_ui.dialog = Some(FloorDialog::New { name: String::new(), copy: false });
        }
        if self.floors_ui.overview {
            self.floors_overview(ctx, &c, right);
        } else {
            (self.floors_ui.counted, self.floors_ui.chips_up) = (false, false);
            self.floors_ui.chips.clear();
        }
        self.floors_dialog(ctx, &c);
        shown.response.rect.width()
    }

    /// The overview: the stage (app/floors/fx.rs) shows the floors in 3D, and over it, above the pill, a chip for each
    /// floor from the top down with Novo Andar over them all. A click on a chip shows its floor, a double click shows it
    /// and puts the overview away, a right click has its menu, the X on the one on view deletes it; Esc or a press on
    /// the stage puts the overview away.
    fn floors_overview(&mut self, ctx: &Context, c: &UiColors, right: f32) {
        let (dark, screen, now) = (self.dark(), ctx.content_rect(), ctx.input(|i| i.time));
        let Some(space) = self.current_space().cloned() else { return };
        if !self.floors_ui.counted {
            let counts = std::iter::once(String::new()).chain(space.floors.iter().map(|f| f.id.clone())).map(|id| (self.floor_items(&space.id, &id), id));
            self.floors_ui.counts = counts.map(|(n, id)| (id, n)).collect();
            self.floors_ui.counted = true;
        }
        // the stage takes the presses the canvas would (the chrome stays above it)
        let stage = self.canvas_rect;
        let back = egui::Area::new(egui::Id::unique("floors_back"))
            .order(egui::Order::Background)
            .fixed_pos(stage.min)
            .show(ctx, |ui| ui.allocate_exact_size(stage.size(), Sense::click()).1);
        let mut close = back.inner.clicked() || (self.floors_ui.dialog.is_none() && ctx.input(|i| i.key_pressed(Key::Escape)));
        // the chips from the top down: the floors above, then the ground (the chips that were there when the overview
        // opened stay put; the ones added after, and Novo Andar, pop in a beat after the one below)
        let stops: Vec<(String, String)> =
            space.floors.iter().rev().map(|f| (f.id.clone(), f.name.clone())).chain([(String::new(), GROUND.to_string())]).collect();
        let (up, count) = (self.floors_ui.chips_up, stops.len());
        for (i, (id, _)) in stops.iter().rev().enumerate() {
            self.floors_ui.chips.entry(id.clone()).or_insert_with(|| {
                let mut s = Spring::new(if up { 0.0 } else { 1.0 }, ITEM);
                s.go(1.0, now, i as f64 * STAGGER);
                s
            });
        }
        self.floors_ui.chips.entry("+".into()).or_insert_with(|| {
            let mut s = Spring::new(0.0, ITEM);
            s.go(1.0, now, count as f64 * STAGGER);
            s
        });
        self.floors_ui.chips.retain(|id, _| id == "+" || stops.iter().any(|(s, _)| s == id));
        self.floors_ui.chips_up = true;
        let pops: HashMap<String, f32> = self.floors_ui.chips.iter().map(|(id, s)| (id.clone(), s.value(now).clamp(0.0, 1.0))).collect();
        if self.floors_ui.chips.values().any(|s| !s.done(now)) {
            ctx.request_repaint();
        }
        let ink = glass_ink(dark);
        let accent = glass_accent(dark);
        let (mut pick, mut new) = (None, false);
        // 13 points of the reference app's rem (px-3.5, py-2, gap-1.5): the chips' padding and the room around the list
        const PAD: f32 = 11.375;
        const GAPS: f32 = 4.875;
        let right_edge = screen.right() + right - 6.5;
        let mut y = screen.bottom() - CORNER_GAP - GLASS_HEIGHT - 13.0;
        let width_of = |text: &str| ctx.layer_painter(egui::LayerId::background()).layout_no_wrap(text.to_string(), FontId::proportional(12.0), ink).size().x;
        egui::Area::new(egui::Id::unique("floors_chips")).order(egui::Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            // the area is as big as the list, so a press on it is the chips' and not the stage's
            let widest = stops.iter().map(|(_, name)| width_of(name) + 17.0).fold(width_of("Novo Andar") + 8.0 + GAPS + 3.0, f32::max) + PAD * 2.0;
            let height = stops.len() as f32 * (30.0 + GAPS) + 32.0;
            ui.allocate_rect(Rect::from_min_max(Pos2::new(right_edge - widest - 8.0, y - height - 8.0), Pos2::new(right_edge + 8.0, y + 8.0)), Sense::hover());
            for (id, name) in &stops {
                let q = pops.get(id).copied().unwrap_or(1.0);
                let ground = id.is_empty();
                let on = space.floor == *id;
                let w = PAD * 2.0 + width_of(name) + if ground { 12.0 + 5.0 } else { 0.0 };
                let rest = Rect::from_min_size(Pos2::new(right_edge - w, y - 30.0), Vec2::new(w, 30.0));
                y -= 30.0 + GAPS;
                let response = ui.interact(rest, ui.scope_id().with(("floor_chip", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                let shown = Rect::from_center_size(rest.center(), rest.size() * (0.8 + 0.2 * q));
                let mut p = ui.painter().clone();
                p.set_opacity(q);
                let text = if on { Color32::WHITE } else { ink };
                if on {
                    p.rect_filled(shown, shown.height() / 2.0, accent);
                } else {
                    paint_glass(&p, shown, shown.height() / 2.0, dark, false);
                }
                let galley = p.layout_no_wrap(name.clone(), FontId::proportional(12.0 * (0.8 + 0.2 * q)), text);
                let mut x = shown.center().x - (galley.size().x + if ground { 17.0 } else { 0.0 }) / 2.0;
                if ground {
                    paint_icon_scaled(&p, Icon::Layers(false), Pos2::new(x + 6.0, shown.center().y), text, 0.75 * (0.8 + 0.2 * q), 0.0);
                    x += 17.0;
                }
                p.galley(Pos2::new(x, shown.center().y - galley.size().y / 2.0), galley, text);
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
                if response.double_clicked() {
                    pick = Some(FloorPick::Commit(id.clone()));
                } else if response.clicked() {
                    pick = Some(FloorPick::Switch(id.clone()));
                }
                crate::app::ui::menu_look::context_menu(&response, |ui| pick = floor_menu(ui, id, c).or(pick.take()));
                if on && !ground {
                    // the X over the chip's corner (22 points, 4.9 out of the corner)
                    let x = Rect::from_center_size(Pos2::new(rest.right() - 6.1, rest.top() + 6.1), Vec2::splat(22.0));
                    let hit = ui.interact(x, ui.scope_id().with(("floor_chip_x", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                    paint_glass(&p, x, 11.0, dark, false);
                    let tint = ink.gamma_multiply(if hit.hovered() { 1.0 } else { 0.7 });
                    for (a, b) in [((-3.0, -3.0), (3.0, 3.0)), ((-3.0, 3.0), (3.0, -3.0))] {
                        p.line_segment([x.center() + Vec2::new(a.0, a.1), x.center() + Vec2::new(b.0, b.1)], Stroke::new(1.5, tint));
                    }
                    if hit.tip("Apagar").clicked() {
                        pick = Some(FloorPick::Delete(id.clone()));
                    }
                }
            }
            // Novo Andar: a dashed pill in the accent
            let q = pops.get("+").copied().unwrap_or(1.0);
            let w = PAD * 2.0 + 8.0 + GAPS + width_of("Novo Andar") + 3.0;
            let rest = Rect::from_min_size(Pos2::new(right_edge - w, y - 32.0), Vec2::new(w, 32.0));
            let response = ui.interact(rest, ui.scope_id().with("floor_chip_new"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let shown = Rect::from_center_size(rest.center(), rest.size() * (0.8 + 0.2 * q));
            let mut p = ui.painter().clone();
            p.set_opacity(q);
            if response.hovered() {
                p.rect_filled(shown, shown.height() / 2.0, accent.gamma_multiply(0.12));
            }
            p.add(Shape::dashed_line(&stadium(shown.shrink(0.75)), Stroke::new(1.5, accent.gamma_multiply(0.5)), 4.5, 3.0));
            let galley = p.layout_no_wrap("Novo Andar".into(), FontId::proportional(12.0 * (0.8 + 0.2 * q)), accent);
            let x = shown.center().x - (galley.size().x + 8.0 + GAPS) / 2.0;
            paint_icon_scaled(&p, Icon::Plus, Pos2::new(x + 4.0, shown.center().y), accent, 0.5, 0.0);
            p.galley(Pos2::new(x + 8.0 + GAPS, shown.center().y - galley.size().y / 2.0), galley, accent);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Novo Andar"));
            new = response.clicked();
        });
        if new {
            pick = Some(FloorPick::New);
        }
        if let Some(pick) = pick {
            close |= matches!(pick, FloorPick::Commit(_));
            self.apply_floor_pick(&space.id, pick);
            // what a menu changed shows on the next frame
            self.floors_ui.counted = false;
        }
        if close {
            self.floors_ui.overview = false;
        }
    }

    /// What a floor's menu or row asked for, on `space`'s floor.
    fn apply_floor_pick(&mut self, space: &str, pick: FloorPick) {
        let index = |app: &Self, id: &str| app.spaces.list.iter().find(|w| w.id == space).and_then(|w| w.floors.iter().position(|f| f.id == id));
        let done = match pick {
            FloorPick::Switch(id) => self.switch_scope(space, &id),
            FloorPick::Commit(id) => {
                self.floors_ui.overview = false;
                self.switch_scope(space, &id)
            }
            FloorPick::New => {
                self.floors_ui.dialog = Some(FloorDialog::New { name: String::new(), copy: false });
                Ok(())
            }
            FloorPick::Rename(id) => {
                let name = self.spaces.list.iter().find(|w| w.id == space).and_then(|w| w.floors.iter().find(|f| f.id == id)).map(|f| f.name.clone());
                self.floors_ui.dialog = name.map(|name| FloorDialog::Rename { space: space.into(), id, name });
                Ok(())
            }
            FloorPick::Up(id) => index(self, &id).map_or(Ok(()), |i| self.move_floor(space, &id, i + 2)),
            FloorPick::Down(id) => index(self, &id).map_or(Ok(()), |i| self.move_floor(space, &id, i)),
            FloorPick::Delete(id) => {
                self.floors_ui.dialog = Some(FloorDialog::Delete { space: space.into(), id });
                Ok(())
            }
        };
        if let Err(error) = done {
            self.toast(error);
        }
    }

    /// The sheets: Novo Andar (nome, Clonar layout do Térreo), Renomear andar, and Apagar {name}.
    fn floors_dialog(&mut self, ctx: &Context, c: &UiColors) {
        match self.floors_ui.dialog.take() {
            None => {}
            Some(FloorDialog::New { mut name, mut copy }) => {
                let (ok, cancel) = dialog_ui(ctx, c, "Novo Andar", "Criar", |ui| {
                    dialog_field(ui, "Nome do andar", &mut name, "", DIALOG_WIDTH, c);
                    ui.add_space(8.0);
                    dialog_check(ui, &mut copy, "Clonar layout do Térreo", c);
                });
                if cancel {
                    return;
                }
                if !ok {
                    self.floors_ui.dialog = Some(FloorDialog::New { name, copy });
                    return;
                }
                match self.create_floor(&name, copy).and_then(|id| self.switch_floor(&id)) {
                    Ok(()) => self.floors_ui.counted = false,
                    Err(error) => {
                        self.toast(error);
                        self.floors_ui.dialog = Some(FloorDialog::New { name, copy });
                    }
                }
            }
            Some(FloorDialog::Rename { space, id, mut name }) => {
                let (ok, cancel) = dialog_ui(ctx, c, "Renomear andar", "Salvar", |ui| {
                    dialog_field(ui, "Nome do andar", &mut name, "", DIALOG_WIDTH, c);
                });
                if cancel {
                    return;
                }
                if !ok {
                    self.floors_ui.dialog = Some(FloorDialog::Rename { space, id, name });
                } else if let Err(error) = self.rename_floor(&space, &id, &name) {
                    self.toast(error);
                    self.floors_ui.dialog = Some(FloorDialog::Rename { space, id, name });
                }
            }
            Some(FloorDialog::Delete { space, id }) => {
                let found = self.spaces.list.iter().find(|w| w.id == space).and_then(|w| w.floors.iter().find(|f| f.id == id)).map(|f| f.name.clone());
                let Some(name) = found else { return };
                let items = self.floor_items(&space, &id);
                let message = match items {
                    0 => format!("{name} está vazio. Excluir remove apenas o andar."),
                    n => format!("Excluir {name} também exclui tudo o que está nele: {}.", items_text(n)),
                };
                let (ok, cancel) = dialog_ui(ctx, c, &format!("Apagar {name}"), "Apagar", |ui| {
                    ui.label(egui::RichText::new(message).size(13.5).color(c.text));
                    dialog_note(ui, "Isso não pode ser desfeito.", c);
                });
                if ok {
                    match self.delete_floor(&space, &id) {
                        Ok(message) => self.toast(message),
                        Err(error) => self.toast(error),
                    }
                } else if !cancel {
                    self.floors_ui.dialog = Some(FloorDialog::Delete { space, id });
                }
            }
        }
    }

    /// The Layers button of a space's row in the sidebar, centered at `at` (14 points): shows or hides the
    /// space's floor rows (`floor_rows`). The row's loop draws it while the pointer is over the row or the rows show.
    pub(crate) fn floors_toggle(&mut self, ui: &mut egui::Ui, at: Pos2, space: &str, ink: &RowInk) {
        let shown = self.floors_ui.shown.contains(space);
        let button = Rect::from_center_size(at, Vec2::splat(20.0));
        let response = ui.interact(button, ui.scope_id().with(("floors_toggle", space)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let tint = if shown || response.hovered() { ink.text } else { ink.muted };
        paint_icon_scaled(ui.painter(), Icon::Layers(false), at, tint, 14.0 / 16.0, 0.0);
        if response.tip(if shown { "Ocultar Andares" } else { "Mostrar Andares" }).clicked() && !self.floors_ui.shown.remove(space) {
            self.floors_ui.shown.insert(space.into());
        }
    }

    /// The floor rows under a space's row in the sidebar, `width` wide (none while hidden): the floors from the
    /// top down and the Térreo last; a click shows one (its space too), a right click has the menu.
    pub(crate) fn floor_rows(&mut self, ui: &mut egui::Ui, space: &str, width: f32, ink: &RowInk) {
        if !self.floors_ui.shown.contains(space) {
            return;
        }
        let Some(w) = self.spaces.list.iter().find(|w| w.id == space).cloned() else { return };
        let on_space = self.spaces.current == w.id;
        let c = self.colors();
        let stops: Vec<(String, String, [u8; 3])> =
            w.floors.iter().rev().map(|f| (f.id.clone(), f.name.clone(), f.color)).chain([(String::new(), GROUND.to_string(), w.color)]).collect();
        let mut pick = None;
        for (id, name, color) in &stops {
            let (rect, row) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click());
            let (on, over) = (on_space && w.floor == *id, ui.rect_contains_pointer(rect));
            let p = ui.painter().clone();
            if on || over {
                p.rect_filled(rect, 6, if on { ink.active } else { ink.hover });
            }
            let [r, g, b] = *color;
            paint_icon_scaled(&p, Icon::Layers(false), Pos2::new(rect.left() + 36.0, rect.center().y), Color32::from_rgb(r, g, b), 0.8, 0.0);
            let text = Rect::from_min_max(Pos2::new(rect.left() + 50.0, rect.top()), Pos2::new(rect.right() - 6.0, rect.bottom()));
            p.with_clip_rect(text).text(text.left_center(), egui::Align2::LEFT_CENTER, name, FontId::proportional(12.5), ink.text);
            row.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
            if row.clicked() {
                pick = Some(FloorPick::Switch(id.clone()));
            }
            crate::app::ui::menu_look::context_menu(&row, |ui| pick = floor_menu(ui, id, &c).or(pick.take()));
        }
        if let Some(pick) = pick {
            self.apply_floor_pick(space, pick);
        }
    }
}
