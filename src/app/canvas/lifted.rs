//! The canvas while a window is enlarged. In the middle of the screen the canvas behind it is blurred: a picture of it
//! taken as the window rises, shrunk and blurred once off the UI thread. Its place on the canvas keeps a skeleton of
//! the window, which shows when it is docked at a side and the canvas is worked on beside it.

use crate::prelude::*;
use std::sync::{Arc, OnceLock};

/// How many times smaller the picture is blurred, and the box blur's passes and reach (in the small picture's pixels):
/// three passes of a box come close to a Gaussian, about 30 px wide on the screen.
const SHRINK: usize = 8;
const PASSES: usize = 3;
const REACH: usize = 2;
/// The canvas's own ground over the blur, so the enlarged window stands out.
const TINT: f32 = 0.35;

pub(crate) struct Backdrop {
    /// The window it was taken for.
    id: String,
    /// The blurred picture once its thread is done, and its texture once loaded.
    blurred: Arc<OnceLock<egui::ColorImage>>,
    texture: Option<egui::TextureHandle>,
}

impl DrawingApp {
    /// The window enlarged in the middle of the screen, or on its way back from there: the canvas is blurred behind it.
    fn blurred_for(&self) -> Option<(&str, f32)> {
        if let Some(id) = self.terminal_elevated.as_deref().filter(|_| self.dock.is_none()) {
            return Some((id, self.elevate_progress(self.elevate_since)));
        }
        let (id, since) = self.leaving.as_ref()?;
        (self.now - since < 0.2).then(|| (id.as_str(), 1.0 - self.elevate_progress(*since)))
    }

    /// The window painted last, over the rest: the enlarged one, or the one shrinking back.
    pub(crate) fn lift_index(&self) -> Option<usize> {
        let id = self.terminal_elevated.as_ref().or(self.leaving.as_ref().filter(|(_, since)| self.now - since < 0.2).map(|(id, _)| id))?;
        self.scene.elements.iter().position(|element| &element.id == id)
    }

    /// A window that draws its own parts over the canvas (a chat card) hides them behind the blur.
    pub(crate) fn behind_blur(&self, id: &str) -> bool {
        self.blurred_for().is_some_and(|(lifted, _)| lifted != id)
    }

    /// Each frame: a window enlarged in the middle asks for its picture of the canvas once (the frame it starts to rise
    /// is still the canvas as it was); no window there, the picture goes.
    pub(crate) fn lift_backdrop(&mut self, ctx: &Context) {
        let Some(id) = self.blurred_for().map(|(id, _)| id.to_string()) else {
            self.backdrop = None;
            return;
        };
        if self.backdrop.as_ref().is_none_or(|backdrop| backdrop.id != id) {
            let blurred: Arc<OnceLock<egui::ColorImage>> = Default::default();
            let (slot, repaint) = (blurred.clone(), ctx.clone());
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |image| {
                let _ = std::thread::Builder::new().name("blur".into()).spawn(move || {
                    let _ = slot.set(blur(&image));
                    repaint.request_repaint();
                });
            })));
            self.backdrop = Some(Backdrop { id, blurred, texture: None });
        }
        if let Some(backdrop) = self.backdrop.as_mut()
            && backdrop.texture.is_none()
            && let Some(image) = backdrop.blurred.get()
        {
            backdrop.texture = Some(ctx.load_texture("lift_backdrop", image.clone(), egui::TextureOptions::LINEAR));
        }
    }

    /// Under the lifted window as the canvas paints it: its skeleton in its place, and the blur over the canvas while it
    /// is in the middle (the canvas's own ground until the picture is ready).
    pub(crate) fn paint_lift_place(&self, painter: &Painter, element: &Element, rect: Rect) {
        if self.terminal_elevated.as_ref() == Some(&element.id) {
            self.paint_skeleton(painter, element);
        }
        let Some((_, shown)) = self.blurred_for().filter(|(id, _)| *id == element.id) else { return };
        let ground = self.ground();
        match self.backdrop.as_ref().and_then(|backdrop| backdrop.texture.as_ref()) {
            Some(texture) => {
                let screen = painter.ctx().content_rect();
                let uv = Rect::from_min_max(((rect.min - screen.min) / screen.size()).to_pos2(), ((rect.max - screen.min) / screen.size()).to_pos2());
                painter.image(texture.id(), rect, uv, Color32::WHITE.gamma_multiply(shown));
                painter.rect_filled(rect, 0.0, ground.gamma_multiply(TINT * shown));
            }
            None => {
                painter.rect_filled(rect, 0.0, ground.gamma_multiply(0.85 * shown));
            }
        }
    }

    /// The window's place on the canvas while it is enlarged: its box dashed, its title strip with its name, and a few
    /// grey lines where its content was.
    fn paint_skeleton(&self, painter: &Painter, element: &Element) {
        let bounds = local_bounds(element);
        let rect = Rect::from_min_max(self.to_screen(bounds.min), self.to_screen(bounds.max));
        if !rect.intersects(painter.clip_rect()) {
            return;
        }
        let zoom = self.zoom;
        let ink = if self.dark() { Color32::WHITE } else { Color32::BLACK };
        let corner = (10.0 * zoom).clamp(2.0, 14.0);
        painter.rect_filled(rect, corner, ink.gamma_multiply(0.03));
        let outline = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
        painter.extend(Shape::dashed_line(&outline, Stroke::new(1.0, ink.gamma_multiply(0.28)), 6.0 * zoom.max(0.5), 5.0 * zoom.max(0.5)));
        let head = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + (TERMINAL_HEADER * zoom).min(rect.height())));
        painter.rect_filled(head, egui::CornerRadius { nw: corner as u8, ne: corner as u8, sw: 0, se: 0 }, ink.gamma_multiply(0.04));
        let pad = 14.0 * zoom;
        if zoom >= 0.35 {
            let name = skeleton_name(element);
            let font = FontId::proportional(12.0 * zoom);
            let galley = painter.layout_no_wrap(name, font, ink.gamma_multiply(0.4));
            if galley.size().x < head.width() - 2.0 * pad {
                painter.galley(Pos2::new(head.left() + pad, head.center().y - galley.size().y / 2.0), galley, ink.gamma_multiply(0.4));
            }
        }
        // the content as grey bars, as a page loading
        let (bar, gap) = (8.0 * zoom, 14.0 * zoom);
        let mut y = head.bottom() + pad;
        for share in SKELETON_LINES.iter().cycle() {
            if y + bar > rect.bottom() - pad {
                break;
            }
            let width = (rect.width() - 2.0 * pad) * share;
            painter.rect_filled(Rect::from_min_size(Pos2::new(rect.left() + pad, y), Vec2::new(width, bar)), bar / 2.0, ink.gamma_multiply(0.06));
            y += bar + gap;
        }
        if zoom >= 0.35 && rect.height() > 90.0 * zoom {
            let words = if self.dock.is_some() { "Aberto ao lado" } else { "Em tela cheia" };
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, words, FontId::proportional(13.0 * zoom), ink.gamma_multiply(0.45));
        }
    }
}

/// How long each grey line of a skeleton is, as a share of its width.
const SKELETON_LINES: [f32; 6] = [0.62, 0.86, 0.48, 0.74, 0.55, 0.8];

/// The window's name on its skeleton.
fn skeleton_name(element: &Element) -> String {
    let named = |name: &str| (!name.trim().is_empty()).then(|| name.trim().to_string());
    element
        .terminal
        .as_ref()
        .and_then(|terminal| named(&terminal.name))
        .or_else(|| element.chat.as_ref().and_then(|chat| named(&chat.name)))
        .or_else(|| element.portal.as_ref().and_then(|portal| named(&portal.name).or_else(|| named(&portal.url))))
        .or_else(|| element.filetree.as_ref().and_then(|tree| named(&crate::app::chat::direct::folder_label(&tree.root))))
        .unwrap_or_else(|| if element.created.is_some() { "Nota".into() } else { "Janela".into() })
}

/// The picture shrunk `SHRINK` times (each cell the mean of its block) and blurred by `PASSES` boxes across and down.
fn blur(image: &egui::ColorImage) -> egui::ColorImage {
    let [width, height] = image.size;
    let (small_width, small_height) = (width.div_ceil(SHRINK).max(1), height.div_ceil(SHRINK).max(1));
    let mut cells = vec![[0.0f32; 4]; small_width * small_height];
    for (index, pixel) in image.pixels.iter().enumerate() {
        let (x, y) = (index % width, index / width);
        let cell = &mut cells[(y / SHRINK) * small_width + x / SHRINK];
        for (channel, value) in [pixel.r(), pixel.g(), pixel.b()].into_iter().enumerate() {
            cell[channel] += f32::from(value);
        }
        cell[3] += 1.0;
    }
    let mut cells: Vec<[f32; 3]> = cells.into_iter().map(|[r, g, b, count]| [r, g, b].map(|sum| sum / count.max(1.0))).collect();
    for _ in 0..PASSES {
        cells = box_pass(&cells, small_width, small_height, true);
        cells = box_pass(&cells, small_width, small_height, false);
    }
    let rgb: Vec<u8> = cells.iter().flat_map(|cell| cell.map(|value| value.round().clamp(0.0, 255.0) as u8)).collect();
    egui::ColorImage::from_rgb([small_width, small_height], &rgb)
}

/// One pass of a box `REACH` wide on each side, across or down, the edges held.
fn box_pass(cells: &[[f32; 3]], width: usize, height: usize, across: bool) -> Vec<[f32; 3]> {
    let mut out = vec![[0.0; 3]; cells.len()];
    let reach = REACH as isize;
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0.0; 3];
            for step in -reach..=reach {
                let (from_x, from_y) = if across {
                    ((x as isize + step).clamp(0, width as isize - 1) as usize, y)
                } else {
                    (x, (y as isize + step).clamp(0, height as isize - 1) as usize)
                };
                let cell = cells[from_y * width + from_x];
                for channel in 0..3 {
                    sum[channel] += cell[channel];
                }
            }
            out[y * width + x] = sum.map(|value| value / (2 * reach + 1) as f32);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blur_shrinks_the_picture_and_spreads_an_edge() {
        // half black, half white, 64 by 16
        let pixels = (0..64 * 16).map(|index| if index % 64 < 32 { Color32::BLACK } else { Color32::WHITE }).collect();
        let blurred = blur(&egui::ColorImage::new([64, 16], pixels));
        assert_eq!(blurred.size, [8, 2], "eight times smaller");
        let row: Vec<u8> = blurred.pixels[..8].iter().map(|pixel| pixel.r()).collect();
        assert!(row[0] < 40 && row[7] > 215, "the far sides keep their colour: {row:?}");
        assert!(row[3] > 40 && row[4] < 215, "the edge is soft: {row:?}");
        assert!(row.windows(2).all(|pair| pair[0] <= pair[1]), "and goes one way: {row:?}");
    }

    #[test]
    fn a_skeleton_is_named_after_its_window() {
        let app = DrawingApp::default();
        let mut element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 100.0, height: 80.0 });
        assert_eq!(skeleton_name(&element), "Janela");
        element.created = Some(0);
        assert_eq!(skeleton_name(&element), "Nota");
        element.terminal = Some(TerminalSpec { name: "Dev".into(), ..Default::default() });
        assert_eq!(skeleton_name(&element), "Dev");
    }
}
