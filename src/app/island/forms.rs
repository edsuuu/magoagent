//! The island's "+" page: a new chat in a workspace of its own, optionally in a git worktree; and its small buttons.

use super::paint::{DIM, DIM_3, INK, Out};
use super::{NewForm, Tab};
use crate::app::chat::direct::folder_label;
use crate::app::chat::list::{Look, clipped};
use crate::prelude::*;

const CARD: Color32 = Color32::from_rgb(0x14, 0x15, 0x18);
const FLAT: Color32 = Color32::from_rgb(0x0e, 0x0f, 0x11);
const ACCENT: Color32 = Color32::from_rgb(0xd9, 0x77, 0x57);

/// The page: the workspace's name, the folder, the worktree, Criar. Its content's height goes to `out.body`.
pub(super) fn new_chat_page(ui: &mut egui::Ui, page: Rect, form: &mut Option<NewForm>, _look: &Look, out: &mut Out) {
    ui.painter().rect(page, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
    let Some(form) = form.as_mut() else { return };
    let inner = page.shrink(14.0);
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
        let scroll = egui::ScrollArea::vertical().id_salt("island_new").auto_shrink([false, true]).max_height(inner.height()).show(ui, |ui| {
            ui.set_width(inner.width() - 6.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(egui::RichText::new("Chat novo num espaço novo").size(13.0).strong().color(INK));
            ui.add(egui::Label::new(egui::RichText::new("O que está em andamento no espaço de agora segue como está.").size(11.5).color(DIM)).wrap());
            egui::Frame::new()
                .fill(FLAT)
                .stroke(Stroke::new(1.0, Color32::from_white_alpha(24)))
                .corner_radius(10)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    let hint = format!("Nome do espaço (vazio: {})", folder_label(&form.folder));
                    ui.add(egui::TextEdit::singleline(&mut form.name).hint_text(hint).frame(egui::Frame::NONE).text_color(INK).desired_width(f32::INFINITY));
                });
            ui.horizontal(|ui| {
                let room = ui.available_width() - 84.0;
                let path = clipped(ui.painter(), form.folder.clone(), FontId::proportional(11.5), DIM_3, room);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(room, 24.0), Sense::hover());
                ui.painter().galley(Pos2::new(rect.left(), rect.center().y - path.size().y / 2.0), path, DIM_3);
                let at = ui.cursor().min;
                if chip(ui, at, "Pasta…", false).clicked() {
                    out.pick_folder = true;
                }
            });
            ui.horizontal(|ui| {
                let (rect, response) = ui.allocate_exact_size(Vec2::new(32.0, 20.0), Sense::click());
                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                if response.clicked() {
                    form.worktree = !form.worktree;
                }
                let t = ui.ctx().animate_bool_with_time(response.id, form.worktree, 0.12);
                ui.painter().rect_filled(rect, 10, Color32::from_white_alpha(28).lerp_to_gamma(ACCENT, t));
                ui.painter().circle_filled(Pos2::new(rect.left() + 10.0 + 12.0 * t, rect.center().y), 7.5, Color32::WHITE);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(egui::RichText::new("Worktree próprio (git)").size(12.5).color(INK));
                    ui.label(egui::RichText::new("Uma cópia do repositório, sem mexer nas alterações atuais.").size(11.0).color(DIM));
                });
            });
            ui.horizontal(|ui| {
                let at = ui.cursor().min;
                out.create = chip(ui, at, "Criar", true).clicked();
                let at = at + Vec2::new(64.0, 0.0);
                if chip(ui, at, "Cancelar", false).clicked() {
                    out.tab = Some(Tab::Home);
                }
            });
        });
        out.body = scroll.content_size.y + 14.0;
    });
}

/// A small round button at `at` (its top left); `strong` is the one that does it. Allocates its room.
pub(super) fn chip(ui: &mut egui::Ui, at: Pos2, text: &str, strong: bool) -> egui::Response {
    let ink = if strong { Color32::BLACK } else { INK };
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(11.5), ink);
    let rect = Rect::from_min_size(at, Vec2::new(galley.size().x + 22.0, 26.0));
    let response = ui.interact(rect, ui.scope_id().with(("island_chip", text)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let ground = match (strong, response.hovered()) {
        (true, hovered) => Color32::from_rgb(0xf5, 0xf6, 0xf8).gamma_multiply(if hovered { 1.0 } else { 0.92 }),
        (false, true) => Color32::from_white_alpha(34),
        (false, false) => Color32::from_white_alpha(18),
    };
    ui.painter().rect_filled(rect, 13, ground);
    ui.painter().galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    ui.advance_cursor_after_rect(rect);
    response
}
