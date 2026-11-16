//! A picture of the island's conversation on the whole screen: a click on it in the log opens it over everything, on
//! the island's monitor, as big as it fits (never past its own pixels); a click, Esc or the ✕ closes it.

use crate::app::chat::image_key;
use crate::app::chat::images::{Decoding, Pic, decode};
use crate::prelude::*;

/// The picture open: read whole in a thread; the log's thumbnail shows meanwhile.
pub(crate) struct Shown {
    full: Decoding,
    key: (usize, String),
}

/// The room left around the picture.
const MARGIN: f32 = 32.0;

impl DrawingApp {
    pub(super) fn island_picture_open(&mut self, ctx: &Context, data: String) {
        let key = image_key(&data);
        let full = decode(ctx, u32::MAX, move || crate::render::base64_decode(&data));
        self.island.picture = Some(Shown { full, key });
    }

    /// The picture open, in a window of its own over the whole monitor (over the canvas with no windows of its own).
    pub(super) fn island_picture_window(&mut self, ctx: &Context) {
        let Some(shown) = self.island.picture.as_ref() else { return };
        let full = shown.full.clone();
        let thumb = self.chat_images.borrow().get(&shown.key).cloned();
        let close = if ctx.embed_viewports() {
            let screen = ctx.content_rect();
            let area = egui::Area::new(egui::Id::unique("island_picture")).order(egui::Order::Tooltip).fixed_pos(screen.min).constrain(false);
            area.show(ctx, |ui| paint_picture(ui, screen, &full, thumb.as_ref())).inner
        } else {
            let anchor = self.island.anchor.unwrap_or([0.0, 0.0]);
            let work = crate::app::chat::mini::screen_work(ctx, Pos2::new(anchor[0], anchor[1] + 1.0));
            let builder = egui::ViewportBuilder::default()
                .with_title("Imagem")
                .with_app_id(crate::window_names().1)
                .with_decorations(false)
                .with_always_on_top()
                .with_taskbar(false)
                .with_position(work.min)
                .with_inner_size(work.size())
                .with_fullscreen(true)
                .with_active(true);
            ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("island_picture"), builder, |ui, _| {
                let screen = ui.ctx().content_rect();
                paint_picture(ui, screen, &full, thumb.as_ref()) || ui.ctx().input(|input| input.viewport().close_requested())
            })
        };
        if close {
            self.island.picture = None;
        }
    }
}

/// The picture fitted in `screen` on a dark ground, its size under it and a ✕; true when it is to close.
fn paint_picture(ui: &mut egui::Ui, screen: Rect, full: &Decoding, thumb: Option<&Decoding>) -> bool {
    let painter = ui.painter().clone();
    painter.rect_filled(screen, 0, Color32::from_rgb(0x0b, 0x0c, 0x0e).gamma_multiply(0.96));
    let ground = ui.interact(screen, ui.scope_id().with("island_picture_ground"), Sense::click());
    let pic: Option<&Pic> = full.get().and_then(Option::as_ref).or_else(|| thumb.and_then(|thumb| thumb.get()).and_then(Option::as_ref));
    let ppp = ui.ctx().pixels_per_point();
    if let Some(pic) = pic {
        let rect = fitted(pic.size, screen.shrink(MARGIN), ppp);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(pic.texture.id(), rect, uv, Color32::WHITE);
        let caption = format!("{} × {}  ·  Esc ou clique para fechar", pic.size[0], pic.size[1]);
        painter.text(
            Pos2::new(screen.center().x, screen.bottom() - MARGIN / 2.0),
            egui::Align2::CENTER_CENTER,
            caption,
            FontId::proportional(12.0),
            Color32::from_white_alpha(140),
        );
    } else if full.get().is_none() {
        painter.text(screen.center(), egui::Align2::CENTER_CENTER, "Abrindo a imagem…", FontId::proportional(14.0), Color32::from_white_alpha(160));
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    }
    let cross = Rect::from_center_size(Pos2::new(screen.right() - 36.0, screen.top() + 36.0), Vec2::splat(36.0));
    let button = ui.interact(cross, ui.scope_id().with("island_picture_close"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    painter.circle_filled(cross.center(), 18.0, Color32::from_white_alpha(if button.hovered() { 40 } else { 22 }));
    let ink = Stroke::new(1.8, Color32::from_white_alpha(220));
    for (from, to) in [(Vec2::new(-6.0, -6.0), Vec2::new(6.0, 6.0)), (Vec2::new(6.0, -6.0), Vec2::new(-6.0, 6.0))] {
        painter.line_segment([cross.center() + from, cross.center() + to], ink);
    }
    let escape = ui.input(|input| input.key_pressed(Key::Escape));
    button.clicked() || ground.clicked() || escape
}

/// A picture of `size` pixels as big as it fits in `room`, its shape kept and never past one of its pixels to one of
/// the screen's (`ppp`: the screen's pixels per point), in the middle of it.
fn fitted(size: [u32; 2], room: Rect, ppp: f32) -> Rect {
    let natural = Vec2::new(size[0].max(1) as f32, size[1].max(1) as f32) / ppp.max(0.1);
    let scale = (room.width() / natural.x).min(room.height() / natural.y).min(1.0);
    Rect::from_center_size(room.center(), natural * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_fits_the_screen_and_a_small_one_keeps_its_size() {
        let room = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0));
        let big = fitted([3200, 900], room, 1.0);
        assert_eq!(big.size(), Vec2::new(1600.0, 450.0), "halved to fit, its shape kept");
        assert_eq!(big.center(), room.center());
        assert_eq!(fitted([400, 300], room, 1.0).size(), Vec2::new(400.0, 300.0), "a small one is not blown up");
        assert_eq!(fitted([400, 300], room, 2.0).size(), Vec2::new(200.0, 150.0), "one picture pixel to one screen pixel");
    }
}
