//! The chat's picture viewer: one image over the whole chat (the conversation isn't drawn behind
//! it), with its name, size, sender and time, Copiar and Salvar como…, the zoom (− % +, Ajustar,
//! 1:1, the wheel), dragging to move, a minimap and the previous and next image.

use super::chat_images::{Decoding, clipped, cross, decode, details, picture};
use crate::app::chat::image_key;
use crate::prelude::*;
use crate::transcript::ChatItem;

/// The viewer's − and + go through these (screen pixels per picture pixel).
const STEPS: [f32; 12] = [0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0];

pub(crate) struct Viewer {
    pub(super) key: (usize, String),
    pub(super) full: Decoding,
    /// Screen pixels per picture pixel; None fits it in the view (never past 1:1).
    pub(super) zoom: Option<f32>,
    /// The picture's center off the view's, in points.
    pan: Vec2,
}

/// What the viewer's buttons ask for that needs the app.
enum Act {
    Copy(String),
    Save(String, String, String),
}

/// The colors of the design's dark table; the light look takes the app's own.
struct Look {
    bg: Color32,
    panel: Color32,
    surface: Color32,
    chip: Color32,
    text: Color32,
    muted: Color32,
    line: Color32,
    edge: Color32,
    accent: Color32,
}

fn look(c: &UiColors, dark: bool) -> Look {
    let accent = Color32::from_rgb(0x8b, 0x87, 0xff);
    if dark {
        let rgb = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        Look {
            bg: rgb(0x0e0e10),
            panel: rgb(0x222229),
            surface: rgb(0x31303c),
            chip: rgb(0x1b1b20),
            text: rgb(0xececf1),
            muted: rgb(0x8e8e96),
            line: rgb(0x1f1f25),
            edge: rgb(0x33323d),
            accent,
        }
    } else {
        Look {
            bg: Color32::WHITE,
            panel: c.surface_low,
            surface: c.surface_low,
            chip: Color32::WHITE,
            text: c.text,
            muted: c.muted,
            line: Color32::from_black_alpha(20),
            edge: Color32::from_black_alpha(36),
            accent,
        }
    }
}

impl DrawingApp {
    /// The picture open over the whole chat, drawn instead of it; false when none is open.
    /// `agent` names who sent the agent's pictures.
    pub(crate) fn image_viewer(&mut self, ctx: &Context, agent: &str) -> bool {
        if self.chat_pics.get_mut().viewer.is_none() {
            return false;
        }
        match self.viewer_ui(ctx, agent) {
            None => {}
            Some(Act::Copy(data)) => {
                // ponytail: decoded again on the UI thread at the click, a hitch for a large one
                match crate::render::base64_decode(&data).and_then(|b| image::load_from_memory(&b).ok()) {
                    Some(image) => {
                        let rgba = image.to_rgba8();
                        ctx.copy_image(egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba));
                        self.toast("Imagem copiada");
                    }
                    None => self.toast("Não foi possível copiar a imagem"),
                }
            }
            Some(Act::Save(data, name, ext)) => {
                let file = if name.contains('.') { name } else { format!("{name}.{ext}") };
                let path = rfd::FileDialog::new().set_title("Salvar imagem").set_file_name(file).add_filter("Imagem", &[ext.as_str()]).save_file();
                if let Some(path) = path {
                    match crate::render::base64_decode(&data).ok_or("imagem ilegível".to_string()).and_then(|b| fs::write(&path, b).map_err(|e| e.to_string()))
                    {
                        Ok(()) => self.toast("Imagem salva"),
                        Err(error) => self.toast(format!("Falha ao salvar: {error}")),
                    }
                }
            }
        }
        true
    }

    fn viewer_ui(&self, ctx: &Context, agent: &str) -> Option<Act> {
        let list = self.chat_image_names();
        let mut pics = self.chat_pics.borrow_mut();
        let viewer = pics.viewer.as_mut()?;
        let found = list.iter().position(|(item, _)| matches!(item, ChatItem::Image { data, .. } if image_key(data) == viewer.key));
        // its message left the memory, or another agent's chat shows
        let Some(at) = found else {
            pics.viewer = None;
            return None;
        };
        let (ChatItem::Image { data, media, user, at: when }, name) = &list[at] else { return None };
        let l = look(&self.colors(), self.dark());
        let area = Rect::from_min_max(Pos2::new(0.0, TOP_ROW + 52.0), ctx.content_rect().max);
        // the full picture once read, the thumbnail meanwhile
        let thumb = self.chat_images.borrow().get(&viewer.key).cloned();
        let pic = viewer.full.get().and_then(Option::as_ref).or(thumb.as_ref().and_then(|t| t.get()).and_then(Option::as_ref));
        let mut act = None;
        let mut go = None;
        let mut close = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowLeft)) {
            go = at.checked_sub(1);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowRight)) {
            go = Some(at + 1);
        }
        egui::Area::new(egui::Id::unique("chat_viewer")).fixed_pos(area.min).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_min_size(area.size());
            let p = ui.painter().clone();
            p.rect_filled(area, 0, l.bg);
            // the header: what it is, whose, which of how many; Copiar, Salvar como… and close
            let head = Rect::from_min_size(area.min, Vec2::new(area.width(), 56.0));
            p.hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0, l.line));
            paint_icon(&p, Icon::Image, Pos2::new(head.left() + 27.0, head.center().y), l.muted);
            let mut sub = pic.map(details).unwrap_or_default();
            let who = if *user { "você" } else { agent };
            sub += &format!("{}enviada por {who}", if sub.is_empty() { "" } else { " · " });
            if let Some(time) = (*when > 0).then(|| local_clock(*when)).flatten() {
                sub += &format!(" às {time}");
            }
            let mut x = head.right() - 12.0;
            let close_rect = Rect::from_min_size(Pos2::new(x - 32.0, head.center().y - 16.0), Vec2::splat(32.0));
            let r = ui.interact(close_rect, ui.scope_id().with("close"), Sense::click()).on_hover_text("Fechar (Esc)");
            p.rect_filled(close_rect, 8, if r.hovered() { l.surface.gamma_multiply(1.3) } else { l.surface });
            cross(&p, close_rect.center(), 5.5, Stroke::new(1.6, l.text));
            close |= r.on_hover_cursor(CursorIcon::PointingHand).clicked();
            x = close_rect.left() - 4.0;
            p.vline(x - 0.5, head.center().y - 12.0..=head.center().y + 12.0, Stroke::new(1.0, l.edge));
            x -= 4.0 + 8.0;
            if header_button(ui, &p, &mut x, head.center().y, Icon::Download, "Salvar como…", &l) {
                act = Some(Act::Save(data.clone(), name.clone(), media.rsplit('/').next().unwrap_or("png").replace("jpeg", "jpg")));
            }
            x -= 8.0;
            if header_button(ui, &p, &mut x, head.center().y, Icon::Duplicate, "Copiar", &l) {
                act = Some(Act::Copy(data.clone()));
            }
            let room = x - 16.0 - (head.left() + 48.0);
            let title = clipped(&p, name.clone(), 14.5, l.text, room - 60.0);
            let line = clipped(&p, sub, 12.0, l.muted, room - 60.0);
            p.galley(Pos2::new(head.left() + 48.0, head.center().y - 18.0), title.clone(), l.text);
            p.galley(Pos2::new(head.left() + 48.0, head.center().y + 1.0), line.clone(), l.muted);
            let count = p.layout_no_wrap(format!("{} de {}", at + 1, list.len()), FontId::proportional(12.0), l.muted);
            let pill = Rect::from_min_size(
                Pos2::new(head.left() + 48.0 + title.size().x.max(line.size().x) + 12.0, head.center().y - 10.0),
                Vec2::new(count.size().x + 16.0, 20.0),
            );
            p.rect_filled(pill, 10, l.panel);
            p.galley(pill.center() - count.size() / 2.0, count, l.muted);
            // the picture, under the pointer's wheel and drag
            let foot = Rect::from_min_max(Pos2::new(area.left(), area.bottom() - 64.0), area.max);
            let body = Rect::from_min_max(Pos2::new(area.left(), head.bottom()), Pos2::new(area.right(), foot.top()));
            let ppp = ctx.pixels_per_point();
            let real = pic.map_or(Vec2::splat(1.0), |p| Vec2::new(p.size[0] as f32, p.size[1] as f32));
            let fit = ((body.width() / real.x).min(body.height() / real.y) * ppp).min(1.0);
            let zoom = viewer.zoom.unwrap_or(fit);
            let drag = ui.interact(body, ui.scope_id().with("picture"), Sense::click_and_drag());
            viewer.pan += drag.drag_delta();
            if drag.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }
            if let Some(pointer) = drag.hover_pos() {
                let (wheel, pinch) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
                let factor = pinch * (wheel / 400.0).exp();
                if factor != 1.0 {
                    let to = (zoom * factor).clamp(0.05, 16.0);
                    viewer.pan = zoomed_pan(viewer.pan, pointer - body.center(), zoom, to);
                    viewer.zoom = Some(to);
                }
            }
            let zoom = viewer.zoom.unwrap_or(fit);
            let shown = real * zoom / ppp;
            let slack = ((shown - body.size()) / 2.0).max(Vec2::ZERO);
            viewer.pan = viewer.pan.clamp(-slack, slack);
            let rect = Rect::from_center_size(body.center() + viewer.pan, shown);
            match (pic, viewer.full.get()) {
                (_, Some(None)) => {
                    p.text(body.center(), egui::Align2::CENTER_CENTER, "Não foi possível abrir a imagem.", FontId::proportional(14.0), l.muted);
                }
                (Some(pic), _) => {
                    p.with_clip_rect(body).image(pic.texture.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
                _ => {}
            }
            // the previous and the next picture
            for (left, to) in [(true, at.checked_sub(1)), (false, (at + 1 < list.len()).then_some(at + 1))] {
                let center = Pos2::new(if left { body.left() + 36.0 } else { body.right() - 36.0 }, body.center().y);
                let r = ui.interact(Rect::from_center_size(center, Vec2::splat(40.0)), ui.scope_id().with(("go", left)), Sense::click());
                p.circle(center, 20.0, l.chip, Stroke::new(1.0, l.edge));
                let ink = if to.is_some() { l.text } else { l.muted.gamma_multiply(0.6) };
                let d = if left { 1.0 } else { -1.0 };
                p.line(vec![center + Vec2::new(2.5 * d, -6.0), center + Vec2::new(-3.5 * d, 0.0), center + Vec2::new(2.5 * d, 6.0)], Stroke::new(2.0, ink));
                if let Some(to) = to {
                    let tip = if left { "Imagem anterior (←)" } else { "Próxima imagem (→)" };
                    if r.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip).clicked() {
                        go = Some(to);
                    }
                }
            }
            // the minimap, while the picture is larger than the view
            if let Some(pic) = pic.filter(|_| slack != Vec2::ZERO) {
                let m = (176.0 / real.x).min(110.0 / real.y);
                let inner = Rect::from_min_size(body.right_bottom() - Vec2::splat(16.0 + 6.0) - real * m, real * m);
                p.rect(inner.expand(6.0), 10, l.chip, Stroke::new(1.0, l.edge), StrokeKind::Inside);
                picture(&p, Some(pic), inner, 4, &UiColors::new(true));
                let seen = rect.intersect(body);
                let part =
                    Rect::from_min_max(inner.min + (seen.min - rect.min) / shown * inner.size(), inner.min + (seen.max - rect.min) / shown * inner.size());
                p.rect(part, 2, l.accent.gamma_multiply(0.12), Stroke::new(2.0, l.accent), StrokeKind::Inside);
            }
            // the zoom: − 150% +, Ajustar and 1:1
            p.hline(foot.x_range(), foot.top() + 0.5, Stroke::new(1.0, l.line));
            let font = FontId::proportional(13.0);
            let fit_label = p.layout_no_wrap("Ajustar".into(), font.clone(), l.text);
            let one = p.layout_no_wrap("1:1".into(), font.clone(), l.text);
            let (fit_w, one_w) = (10.0 + 15.0 + 6.0 + fit_label.size().x + 10.0, one.size().x + 20.0);
            let width = 4.0 + 32.0 + 2.0 + 58.0 + 2.0 + 32.0 + 13.0 + fit_w + 2.0 + one_w + 4.0;
            let bar = Rect::from_center_size(foot.center(), Vec2::new(width, 40.0));
            p.rect_filled(bar, 12, l.panel);
            let mut x = bar.left() + 4.0;
            let cell = |x: &mut f32, w: f32, id: &str| {
                let r = Rect::from_min_size(Pos2::new(*x, bar.top() + 4.0), Vec2::new(w, 32.0));
                *x += w + 2.0;
                let response = ui.interact(r, ui.scope_id().with(id), Sense::click());
                if response.hovered() {
                    p.rect_filled(r, 8, l.surface);
                }
                (r, response.on_hover_cursor(CursorIcon::PointingHand))
            };
            let (r, minus) = cell(&mut x, 32.0, "minus");
            paint_icon(&p, Icon::Minus, r.center(), l.text);
            let label = Rect::from_min_size(Pos2::new(x, bar.top()), Vec2::new(58.0, 40.0));
            p.text(label.center(), egui::Align2::CENTER_CENTER, format!("{:.0}%", zoom * 100.0), font.clone(), l.text);
            x += 60.0;
            let (r, plus) = cell(&mut x, 32.0, "plus");
            paint_icon(&p, Icon::Plus, r.center(), l.text);
            p.vline(x + 4.5, bar.center().y - 10.0..=bar.center().y + 10.0, Stroke::new(1.0, l.edge));
            x += 11.0;
            let (r, fit_button) = cell(&mut x, fit_w, "fit");
            corners(&p, Pos2::new(r.left() + 10.0 + 7.5, r.center().y), Stroke::new(1.4, l.text));
            p.galley(Pos2::new(r.left() + 31.0, r.center().y - fit_label.size().y / 2.0), fit_label, l.text);
            let (r, one_button) = cell(&mut x, one_w, "one");
            p.galley(r.center() - one.size() / 2.0, one, l.text);
            let step = if minus.on_hover_text("Diminuir zoom").clicked() {
                STEPS.iter().rev().find(|s| **s < zoom * 0.99).copied()
            } else if plus.on_hover_text("Aumentar zoom").clicked() {
                STEPS.iter().find(|s| **s > zoom * 1.01).copied()
            } else {
                None
            };
            if let Some(to) = step {
                viewer.pan = zoomed_pan(viewer.pan, Vec2::ZERO, zoom, to);
                viewer.zoom = Some(to);
            }
            if fit_button.clicked() {
                (viewer.zoom, viewer.pan) = (None, Vec2::ZERO);
            }
            if one_button.clicked() {
                viewer.pan = zoomed_pan(viewer.pan, Vec2::ZERO, zoom, 1.0);
                viewer.zoom = Some(1.0);
            }
            let hint = p.layout_no_wrap("Roda do mouse: zoom · Arrastar: mover · Esc: fechar".into(), FontId::proportional(12.0), l.muted);
            if foot.right() - 20.0 - hint.size().x > bar.right() + 16.0 {
                p.galley(Pos2::new(foot.right() - 20.0 - hint.size().x, foot.center().y - hint.size().y / 2.0), hint, l.muted);
            }
        });
        if let Some((ChatItem::Image { data, .. }, _)) = go.and_then(|k| list.get(k)) {
            pics.viewer = Some(Viewer::open(ctx, image_key(data), data));
        }
        if close {
            pics.viewer = None;
        }
        act
    }
}

impl Viewer {
    pub(super) fn open(ctx: &Context, key: (usize, String), data: &str) -> Viewer {
        let data = data.to_string();
        Viewer { key, full: decode(ctx, u32::MAX, move || crate::render::base64_decode(&data)), zoom: None, pan: Vec2::ZERO }
    }
}

/// The pan that keeps the point `at` (off the view's center) still while the zoom goes `from` → `to`.
fn zoomed_pan(pan: Vec2, at: Vec2, from: f32, to: f32) -> Vec2 {
    at - (at - pan) * (to / from)
}

/// "Ajustar"'s four corners.
fn corners(p: &Painter, at: Pos2, stroke: Stroke) {
    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = at + Vec2::new(6.0 * x, 6.0 * y);
        p.line(vec![corner - Vec2::new(0.0, 4.0 * y), corner, corner - Vec2::new(4.0 * x, 0.0)], stroke);
    }
}

/// A header button (icon and words) ending at `x`, which moves to its left edge.
fn header_button(ui: &egui::Ui, p: &Painter, x: &mut f32, y: f32, icon: Icon, label: &str, l: &Look) -> bool {
    let text = p.layout_no_wrap(label.into(), FontId::proportional(13.5), l.text);
    let rect = Rect::from_min_max(Pos2::new(*x - (10.0 + 15.0 + 7.0 + text.size().x + 12.0), y - 16.0), Pos2::new(*x, y + 16.0));
    *x = rect.left();
    let response = ui.interact(rect, ui.scope_id().with(label), Sense::click());
    p.rect_filled(rect, 8, if response.hovered() { l.surface.gamma_multiply(1.3) } else { l.surface });
    paint_icon(p, icon, Pos2::new(rect.left() + 17.5, y), l.text);
    p.galley(Pos2::new(rect.left() + 32.0, y - text.size().y / 2.0), text, l.text);
    response.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

/// "16:52": the local time of `ms` since 1970.
fn local_clock(ms: i64) -> Option<String> {
    let (hour, minute) = local_hour_minute(ms)?;
    Some(format!("{hour:02}:{minute:02}"))
}

#[cfg(windows)]
fn local_hour_minute(ms: i64) -> Option<(u16, u16)> {
    unsafe extern "system" {
        fn FileTimeToSystemTime(file: *const u64, out: *mut [u16; 8]) -> i32;
        fn SystemTimeToTzSpecificLocalTime(zone: *const u8, utc: *const [u16; 8], local: *mut [u16; 8]) -> i32;
    }
    // 100 ns steps since 1601
    let file = (ms.checked_add(11_644_473_600_000)? as u64).checked_mul(10_000)?;
    let (mut utc, mut local) = ([0u16; 8], [0u16; 8]);
    // SAFETY: plain values in and out, sized as Windows declares them (SYSTEMTIME: 8 WORDs)
    let done = unsafe { FileTimeToSystemTime(&file, &mut utc) != 0 && SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) != 0 };
    done.then_some((local[4], local[5]))
}

#[cfg(unix)]
fn local_hour_minute(ms: i64) -> Option<(u16, u16)> {
    /// Larger than any platform's struct tm, aligned as it: its first ints are sec, min, hour.
    #[repr(C, align(8))]
    struct Tm([i32; 16]);
    unsafe extern "C" {
        fn localtime_r(time: *const i64, out: *mut Tm) -> *mut Tm;
    }
    let (time, mut tm) = (ms.div_euclid(1000), Tm([0; 16]));
    // SAFETY: both pointers are valid for the call
    let done = unsafe { !localtime_r(&time, &mut tm).is_null() };
    done.then_some((tm.0[2] as u16, tm.0[1] as u16))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_zoom_keeps_the_pointer_and_the_time_is_local() {
        use eframe::egui::Vec2;
        assert_eq!(super::zoomed_pan(Vec2::ZERO, Vec2::new(100.0, 0.0), 1.0, 2.0), Vec2::new(-100.0, 0.0), "the point under the pointer stays");
        assert_eq!(super::STEPS.iter().find(|s| **s > 0.92 * 1.01), Some(&1.0), "+ from a fit goes to the next step");
        assert_eq!(crate::transcript::iso_ms("2026-10-28T19:52:03.123Z"), Some(1_793_217_123_000));
        assert_eq!(crate::transcript::iso_ms("1970-01-01T00:00:00Z"), Some(0));
        assert!(super::local_clock(1_793_217_123_000).is_some_and(|t| t.len() == 5 && t.as_bytes()[2] == b':'), "16:52 or so, by the zone");
    }
}
