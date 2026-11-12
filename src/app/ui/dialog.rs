//! The dialogs and every question the app asks: the reference app's sheet (1rem = 13px there). A card
//! 480 wide in the middle of the window over a 30% veil, with Cancelar, the title and the confirm at
//! its top and the fields under; every control is drawn from the tokens of Ajustes (`kit::K`).

use super::settings::kit::{self, K};
use crate::prelude::*;
use egui::{Role, WidgetInfo};

/// The sheet (`max-w-[480px]`), what it keeps at the sides (`px-4`) and what is left for the fields.
const SHEET: f32 = 480.0;
const SIDE: f32 = 13.0;
pub(crate) const DIALOG_WIDTH: f32 = SHEET - 2.0 * SIDE;
/// The top bar's height (`min-h-[44px]`) and the line the buttons and the title sit on.
const BAR: f32 = 44.0;
const BAR_MIDDLE: f32 = 26.5;
/// `gap-5`, what follows each field, and `gap-1.5`, what a label has under it.
const GAP: f32 = 16.25;
const LABEL_GAP: f32 = 4.875;
const FIELD: f32 = 33.0;
const RADIUS: f32 = 8.125;

fn tokens(c: &UiColors) -> K {
    K::new(c.text.r() > 0x80)
}

/// The gray of the small texts: `text-muted-foreground`.
pub(crate) fn dialog_label(c: &UiColors) -> Color32 {
    tokens(c).muted_fg
}

/// The node dialogs (new terminal, portal, IDE) and the questions, as the reference app's sheet: Cancelar
/// at the left of the bar, the title in the middle, the confirm at the right, the fields under. Esc
/// cancels, Enter confirms. Returns (confirm, cancel).
pub(crate) fn dialog_ui(ctx: &Context, c: &UiColors, title: &str, confirm: &str, body: impl FnOnce(&mut egui::Ui)) -> (bool, bool) {
    dialog_sheet(ctx, c, title, confirm, GAP, body)
}

/// `dialog_ui` with `top` points between the bar and the body (13 for a body that starts with a
/// section, 16.25 for one that starts with a field).
pub(crate) fn dialog_sheet(ctx: &Context, c: &UiColors, title: &str, confirm: &str, top: f32, body: impl FnOnce(&mut egui::Ui)) -> (bool, bool) {
    let (mut ok, mut cancel) = (false, false);
    let k = tokens(c);
    let id = egui::Id::unique(("dialog", title));
    // it opens in 0.16 s (the veil and the card fade in); one that was not drawn the frame before starts over
    let (now, frame) = (ctx.input(|i| i.time), ctx.cumulative_frame_nr());
    let (start, seen) = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id.with("open"), || (now, frame)));
    let start = if frame > seen + 1 { now } else { start };
    ctx.data_mut(|d| d.insert_temp(id.with("open"), (start, frame)));
    let open = ((now - start) / 0.16).clamp(0.0, 1.0) as f32;
    if open < 1.0 {
        ctx.request_repaint();
    }
    let max = (ctx.content_rect().height() * 0.8 - BAR).max(120.0);
    // the veil (it takes every click, so nothing under the sheet answers) and the sheet over it
    let screen = ctx.content_rect();
    egui::Area::new(id.with("veil")).order(egui::Order::Foreground).fixed_pos(screen.min).movable(false).show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0, Color32::from_black_alpha((77.0 * open) as u8));
    });
    egui::Area::new(id).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
        egui::Frame::new().fill(k.bg).corner_radius(11).shadow(egui::epaint::Shadow::NONE).show(ui, |ui| {
            ui.set_opacity(open);
            ui.set_width(SHEET);
            ui.set_max_height(max + BAR);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (bar, _) = ui.allocate_exact_size(Vec2::new(SHEET, BAR), Sense::hover());
            ui.painter().hline(bar.x_range(), bar.bottom() - 0.5, Stroke::new(1.0, k.border));
            let middle = bar.top() + BAR_MIDDLE;
            ui.painter().text(Pos2::new(bar.center().x, middle), egui::Align2::CENTER_CENTER, title, FontId::proportional(17.0), k.fg);
            cancel = bar_button(ui, &k, Pos2::new(bar.left() + 9.0, middle), false, "Cancelar", 17.0);
            ok = bar_button(ui, &k, Pos2::new(bar.right() - 9.0, middle), true, confirm, 15.0);
            egui::ScrollArea::vertical().max_height(max).auto_shrink([false, true]).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin { left: SIDE as i8, right: SIDE as i8, top: top as i8, bottom: 10 }).show(ui, |ui| {
                    ui.set_width(DIALOG_WIDTH);
                    body(ui);
                });
            });
        });
    });
    // a click on the veil brought it up; the sheet goes back over it, unless a menu is open from
    // it (Recentes): that one is newer and must stay above, or the sheet would hide it
    if !egui::Popup::is_any_open(ctx) {
        ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, id));
    }
    ok |= ctx.input(|i| i.key_pressed(Key::Enter));
    cancel |= ctx.input(|i| i.key_pressed(Key::Escape));
    (ok, cancel)
}

/// A ghost button of the bar: its text in the accent, `edge` its left (or right) side, `middle` its line.
fn bar_button(ui: &mut egui::Ui, k: &K, edge: Pos2, right: bool, text: &str, size: f32) -> bool {
    let galley = ui.painter().layout_no_wrap(text.into(), FontId::proportional(size), k.primary);
    let size = Vec2::new(galley.size().x + 20.0, 27.0);
    let rect =
        if right { Rect::from_min_size(Pos2::new(edge.x - size.x, edge.y - 13.5), size) } else { Rect::from_min_size(Pos2::new(edge.x, edge.y - 13.5), size) };
    let response = ui.interact(rect, ui.scope_id().with(("bar", text)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, text));
    if response.hovered() {
        ui.painter().rect_filled(rect, RADIUS, k.muted.gamma_multiply(if k.dark { 0.5 } else { 1.0 }));
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, k.primary);
    response.clicked()
}

/// A field's name: 13 px, muted, with the gap before its box.
pub(crate) fn dialog_caption(ui: &mut egui::Ui, text: &str, c: &UiColors) {
    let k = tokens(c);
    let galley = kit::galley(ui, text, 13.0, 18.2, k.muted_fg, f32::INFINITY);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), galley.size().y + LABEL_GAP), Sense::hover());
    ui.painter().galley(rect.min, galley, k.muted_fg);
}

/// `Input`: the box (a `dark:bg-input/30` fill, a border that goes to the accent with a soft ring on
/// focus), the text inside it. `width` and `height` are its size.
fn input_box(ui: &mut egui::Ui, k: &K, label: &str, text: &mut String, hint: &str, size: Vec2, font: FontId, password: bool) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let id = ui.scope_id().with(("dialog_input", label));
    let focused = ui.memory(|m| m.has_focus(id));
    if k.dark {
        ui.painter().rect_filled(rect, RADIUS, k.input.gamma_multiply(0.3));
    }
    if focused {
        ui.painter().rect_stroke(rect, RADIUS, Stroke::new(2.0, k.primary.gamma_multiply(0.15)), StrokeKind::Outside);
    }
    ui.painter().rect_stroke(rect, RADIUS, Stroke::new(1.0, if focused { k.primary } else { k.input }), StrokeKind::Inside);
    let edit = egui::TextEdit::singleline(text)
        .id(id)
        .password(password)
        .hint_text(egui::RichText::new(hint).color(k.muted_fg))
        .font(font)
        .text_color(k.fg)
        .frame(egui::Frame::NONE)
        .align(egui::Align2::LEFT_CENTER)
        .desired_width(size.x - 2.0 * RADIUS);
    let response = ui.put(Rect::from_min_max(rect.min + Vec2::new(RADIUS, 0.0), rect.max - Vec2::new(RADIUS, 0.0)), edit);
    response.widget_info(|| WidgetInfo::labeled(Role::TextInput, true, label));
    response
}

/// A field of a dialog: its name above, then the box. The first one a dialog draws takes the keyboard.
pub(crate) fn dialog_field(ui: &mut egui::Ui, label: &str, text: &mut String, hint: &str, width: f32, c: &UiColors) -> egui::Response {
    field(ui, label, text, hint, width, FontId::proportional(15.0), c)
}

/// The same for a command or a path: the monospace font, a little smaller.
pub(crate) fn dialog_field_mono(ui: &mut egui::Ui, label: &str, text: &mut String, hint: &str, width: f32, c: &UiColors) -> egui::Response {
    field(ui, label, text, hint, width, FontId::monospace(13.0), c)
}

fn field(ui: &mut egui::Ui, label: &str, text: &mut String, hint: &str, width: f32, font: FontId, c: &UiColors) -> egui::Response {
    dialog_caption(ui, label, c);
    let response = input_box(ui, &tokens(c), label, text, hint, Vec2::new(width, FIELD), font, false);
    // a dialog opens typing in its first field
    if ui.memory(|m| m.focused().is_none()) {
        response.request_focus();
    }
    ui.add_space(GAP);
    response
}

/// A button the height of the folder field, with `text` in it: the outline of a button (`variant="outline"`).
fn field_button(ui: &mut egui::Ui, k: &K, rect: Rect, text: &str) -> egui::Response {
    let response = ui.interact(rect, ui.scope_id().with(("field_button", text)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, text));
    let fill = if response.hovered() {
        k.muted
    } else if k.dark {
        k.input.gamma_multiply(0.3)
    } else {
        k.bg
    };
    ui.painter().rect_filled(rect, RADIUS, fill);
    ui.painter().rect_stroke(rect, RADIUS, Stroke::new(1.0, if k.dark { k.input } else { k.border }), StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, FontId::proportional(11.5), k.fg);
    response
}

/// A folder field with Procurar... beside it and, once there are folders to offer, Recentes between the two.
pub(crate) fn dialog_folder(ui: &mut egui::Ui, label: &str, folder: &mut String, hint: &str, title: &str, c: &UiColors, folders: &mut Folders) {
    const HEIGHT: f32 = 30.0;
    let k = tokens(c);
    let (browse, recent) = (83.0, if folders.is_empty() { 0.0 } else { 78.0 });
    dialog_caption(ui, label, c);
    let (row, _) = ui.allocate_exact_size(Vec2::new(DIALOG_WIDTH, HEIGHT), Sense::hover());
    let field = Rect::from_min_max(row.min, Pos2::new(row.right() - browse - 5.0 - if recent > 0.0 { recent + 5.0 } else { 0.0 }, row.bottom()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(field), |ui| input_box(ui, &k, label, folder, hint, field.size(), FontId::monospace(13.0), false));
    if recent > 0.0 {
        let spot = Rect::from_min_size(Pos2::new(row.right() - browse - 5.0 - recent, row.top()), Vec2::new(recent, HEIGHT));
        let response = field_button(ui, &k, spot, "Recentes").tip("Pastas recentes e favoritas");
        // starring keeps the menu open; picking a folder closes it
        egui::Popup::menu(&response).frame(kit::popup_frame(&k)).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_width(320.0);
            ui.spacing_mut().item_spacing.y = 1.0;
            match folder_rows(ui, folders, c) {
                Some(FolderPick::Use(path)) => {
                    *folder = path;
                    ui.close();
                }
                Some(FolderPick::Star(path)) => _ = folders.toggle_favorite(&path),
                None => {}
            }
        });
    }
    let spot = Rect::from_min_size(Pos2::new(row.right() - browse, row.top()), Vec2::new(browse, HEIGHT));
    if field_button(ui, &k, spot, "Procurar...").clicked()
        && let Some(dir) = rfd::FileDialog::new().set_title(title).pick_folder()
    {
        *folder = dir.to_string_lossy().into();
    }
    ui.add_space(GAP);
}

/// `SettingsToggleRow`: the name (and under it, in small, what it does) at the left, the switch at the
/// right. The response is the row's, for a tip.
pub(crate) fn dialog_toggle(ui: &mut egui::Ui, on: &mut bool, label: &str, hint: &str, c: &UiColors) -> egui::Response {
    let top = ui.cursor().min;
    kit::toggle(ui, &tokens(c), label, hint, on);
    let row = Rect::from_min_max(top, Pos2::new(top.x + ui.available_width(), ui.cursor().min.y));
    ui.add_space(GAP);
    ui.interact(row, ui.scope_id().with(("toggle_row", label)), Sense::hover())
}

/// A switch with only its name.
pub(crate) fn dialog_check(ui: &mut egui::Ui, on: &mut bool, label: &str, c: &UiColors) -> egui::Response {
    dialog_toggle(ui, on, label, "", c)
}

/// The muted paragraph under a dialog's fields.
pub(crate) fn dialog_note(ui: &mut egui::Ui, text: &str, c: &UiColors) {
    kit::note(ui, &tokens(c), text);
    ui.add_space(GAP);
}

/// `TabBar`: the tabs across a dark track, the chosen one lifted. True when the choice changed.
pub(crate) fn dialog_tabs(ui: &mut egui::Ui, chosen: &mut usize, labels: &[&str], c: &UiColors) -> bool {
    let k = tokens(c);
    let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 29.0), Sense::hover());
    ui.painter().rect_filled(strip, 9.0, Color32::from_black_alpha(if k.dark { 71 } else { 15 }));
    ui.painter().rect_stroke(
        strip,
        9.0,
        Stroke::new(0.5, if k.dark { Color32::from_white_alpha(13) } else { Color32::from_black_alpha(10) }),
        StrokeKind::Inside,
    );
    let n = labels.len() as f32;
    let width = (strip.width() - 3.25 - 1.625 * (n - 1.0)) / n;
    let mut changed = false;
    for (i, label) in labels.iter().enumerate() {
        let tab = Rect::from_min_size(strip.min + Vec2::new(1.625 + i as f32 * (width + 1.625), 1.625), Vec2::new(width, 26.0));
        let response = ui.interact(tab, ui.scope_id().with(("dialog_tab", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        response.widget_info(|| WidgetInfo::selected(Role::Button, true, i == *chosen, *label));
        if i == *chosen {
            ui.painter().rect_filled(tab, 7.0, if k.dark { Color32::from_white_alpha(36) } else { k.card });
        }
        let color = if i == *chosen || response.hovered() {
            k.fg
        } else if k.dark {
            k.fg.gamma_multiply(0.75)
        } else {
            k.muted_fg
        };
        ui.painter().text(tab.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(13.0), color);
        if response.clicked() && i != *chosen {
            *chosen = i;
            changed = true;
        }
    }
    ui.add_space(14.5);
    changed
}

/// A pick-list in the field's look (the box of `SheetSelect`): `shown` is what it says now.
pub(crate) fn dialog_combo(ui: &mut egui::Ui, id: &str, shown: String, width: f32, c: &UiColors, items: impl FnOnce(&mut egui::Ui)) {
    let k = tokens(c);
    ui.scope(|ui| {
        let fill = if k.dark { k.input.gamma_multiply(0.3) } else { k.bg };
        let visuals = ui.visuals_mut();
        for w in [&mut visuals.widgets.inactive, &mut visuals.widgets.hovered, &mut visuals.widgets.active, &mut visuals.widgets.open] {
            w.weak_bg_fill = fill;
            w.bg_fill = fill;
            w.bg_stroke = Stroke::new(1.0, k.input);
            w.corner_radius = egui::CornerRadius::same(8);
            w.fg_stroke.color = k.fg;
        }
        ui.spacing_mut().interact_size.y = 30.0;
        egui::ComboBox::from_id_salt(id).selected_text(egui::RichText::new(shown).size(13.0).color(k.fg)).width(width).show_ui(ui, items);
    });
    ui.add_space(GAP);
}

/// The balloon that asks for a name, hung under `anchor` (256 wide): a field, Cancelar and Salvar. Enter
/// saves, Esc and a press outside cancel. `Some(true)` = save, `Some(false)` = cancel. The field takes the
/// keyboard and selects its text the first frame (`first`).
pub(crate) fn name_balloon(ctx: &Context, c: &UiColors, anchor: Rect, label: &str, hint: &str, text: &mut String, first: bool) -> Option<bool> {
    let k = tokens(c);
    let (mut save, mut cancel) = (false, false);
    let area = egui::Area::new(egui::Id::unique("name_balloon"))
        .fixed_pos(anchor.left_bottom() + Vec2::new(0.0, 6.0))
        .order(egui::Order::Foreground)
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::new().fill(k.bg).stroke(Stroke::new(1.0, k.border)).corner_radius(RADIUS).inner_margin(9.75).show(ui, |ui| {
                ui.set_width(256.0 - 2.0 * 9.75 - 2.0);
                ui.spacing_mut().item_spacing = Vec2::new(6.5, 8.0);
                let width = ui.available_width();
                let response = input_box(ui, &k, label, text, hint, Vec2::new(width, 26.0), FontId::proportional(13.0), false);
                if first {
                    response.request_focus();
                    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                        let end = egui::text::CCursor::new(text.chars().count());
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), end)));
                        state.store(ui.ctx(), response.id);
                    }
                }
                if text.chars().count() > 120 {
                    *text = text.chars().take(120).collect();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    save = kit::button(ui, &k, "Salvar").clicked();
                    cancel = kit::button(ui, &k, "Cancelar").clicked();
                });
            });
        });
    let outside = ctx.input(|i| i.pointer.any_pressed())
        && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p));
    save |= ctx.input(|i| i.key_pressed(Key::Enter));
    cancel |= outside || ctx.input(|i| i.key_pressed(Key::Escape));
    if save { Some(true) } else { cancel.then_some(false) }
}
