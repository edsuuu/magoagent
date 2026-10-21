//! Painting elements on the egui canvas, image textures and sticky notes.

use crate::prelude::*;

pub(crate) mod replay;
pub(crate) mod rough;
pub(crate) mod svg;

pub(crate) struct View {
    pub(crate) origin: Vec2,
    pub(crate) zoom: f32,
    pub(crate) dark: bool,
    pub(crate) background: Color32,
}

pub(crate) fn rgba(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// The web's dark-mode filter, `invert(93%) hue-rotate(180deg)` (applyDarkModeFilter): flips
/// lightness, keeps hue.
pub(crate) fn themed(color: Color32, dark: bool) -> Color32 {
    if !dark {
        return color;
    }
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let [r, g, b] = [r, g, b].map(|v| (v as f32 * 0.07 + (255.0 - v as f32) * 0.93).round() / 255.0);
    let out = [
        -0.574 * r + 1.43 * g + 0.144 * b,
        0.426 * r + 0.43 * g + 0.144 * b,
        0.426 * r + 1.43 * g - 0.856 * b,
    ]
    .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgba_unmultiplied(out[0], out[1], out[2], a)
}

pub(crate) fn with_opacity(color: [u8; 4], opacity: u8) -> Color32 {
    let a = (color[3] as f32 * opacity as f32 / 100.0).round() as u8;
    Color32::from_rgba_unmultiplied(color[0], color[1], color[2], a)
}

pub(crate) fn text_lines(value: &str) -> Vec<&str> {
    value.split('\n').collect()
}

pub(crate) fn paint_element(painter: &Painter, element: &Element, view: &View, faded: bool, hide_text: bool, halo: bool) {
    let opacity = if faded { element.style.opacity / 3 } else { element.style.opacity };
    let stroke_color = themed(with_opacity(element.style.stroke, opacity), view.dark);
    let (center, angle) = (element.center(), element.angle);
    let to = |p: &[f32; 2]| {
        let p = if angle == 0.0 { Pos2::new(p[0], p[1]) } else { center + rot(Pos2::new(p[0], p[1]) - center, angle) };
        Pos2::new(view.origin.x + p.x * view.zoom, view.origin.y + p.y * view.zoom)
    };
    if let Some(url) = &element.image {
        let b = local_bounds(element);
        let corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].map(|c| to(&[c.x, c.y]));
        match image_texture(painter.ctx(), url) {
            Some(texture) => {
                let mut mesh = egui::Mesh::with_texture(texture.id());
                let tint = Color32::WHITE.gamma_multiply(opacity as f32 / 100.0);
                for (pos, uv) in corners.into_iter().zip([Pos2::new(0.0, 0.0), Pos2::new(1.0, 0.0), Pos2::new(1.0, 1.0), Pos2::new(0.0, 1.0)]) {
                    mesh.vertices.push(egui::epaint::Vertex { pos, uv, color: tint });
                }
                mesh.indices.extend([0, 1, 2, 0, 2, 3]);
                painter.add(mesh);
            }
            None => {
                painter.add(Shape::closed_line(corners.to_vec(), Stroke::new(1.0, stroke_color)));
            }
        }
        return;
    }
    if let Some(created) = element.created {
        // sticky note: soft drop shadow, paper, faint inner edge and the creation date
        let b = local_bounds(element);
        let paper: Vec<Pos2> = sticky_outline(b, element.style.round, 0.0).iter().map(to).collect();
        let shadow = sticky_outline(b, element.style.round, 3.0).iter().map(to).collect();
        let alpha = opacity as f32 / 100.0;
        painter.add(Shape::convex_polygon(shadow, Color32::from_black_alpha((0.16 * alpha * 255.0) as u8), Stroke::NONE));
        let paper_color = themed(with_opacity(element.style.fill.unwrap_or([0xff, 0xdf, 0x6b, 255]), opacity), view.dark);
        painter.add(Shape::convex_polygon(paper.clone(), paper_color, Stroke::new(0.5 * view.zoom, Color32::from_black_alpha((0.08 * alpha * 255.0) as u8))));
        if b.width() >= 75.0 && b.height() >= 75.0 {
            if let Some(date) = sticky_date(created, b.width() - 32.0 < 80.0) {
                let at = to(&[b.max.x - 16.0, b.max.y - 11.0]);
                painter.text(at, egui::Align2::RIGHT_BOTTOM, date, egui::FontId::proportional(12.0 * view.zoom), stroke_color);
            }
        }
        return;
    }
    if let (Some(fill), Some(poly)) = (element.style.fill, shape_polygon(element)) {
        let fill = themed(with_opacity(fill, opacity), view.dark);
        if element.style.fill_style == FillStyle::Solid {
            painter.add(Shape::convex_polygon(poly.iter().map(to).collect(), fill, Stroke::NONE));
        } else {
            let s = Stroke::new((element.style.stroke_width / 2.0).max(0.5) * view.zoom, fill);
            for line in fill_strokes(element) {
                painter.add(Shape::line(line.iter().map(to).collect(), s));
            }
        }
    }
    let width = if matches!(element.kind, ElementKind::FreeDraw { .. }) {
        element.style.stroke_width * 1.5
    } else {
        element.style.stroke_width
    };
    let stroke = Stroke::new(width * view.zoom, stroke_color);
    for outline in rough_outlines(element) {
        if outline.len() < 2 {
            continue;
        }
        let pts: Vec<Pos2> = outline.iter().map(to).collect();
        let sw = element.style.stroke_width;
        match element.style.stroke_style {
            StrokeStyle::Solid => {
                painter.add(Shape::line(pts, stroke));
            }
            StrokeStyle::Dashed => painter.extend(Shape::dashed_line(&pts, stroke, 8.0 * view.zoom, (8.0 + sw) * view.zoom)),
            StrokeStyle::Dotted => painter.extend(Shape::dashed_line(&pts, stroke, 1.5 * view.zoom, (6.0 + sw) * view.zoom)),
        }
    }
    for (shape, head) in arrowheads(element) {
        let pts: Vec<Pos2> = shape.iter().map(to).collect();
        match head.fill() {
            Some(solid) => {
                let fill = if solid { stroke_color } else { view.background };
                painter.add(Shape::convex_polygon(pts, fill, stroke));
            }
            None => {
                painter.add(Shape::line(pts, stroke));
            }
        }
    }
    if let (Some(name), ElementKind::Rectangle { x, y, .. }) = (&element.frame, &element.kind) {
        if !name.is_empty() {
            let color = themed(Color32::from_rgb(0x99, 0x99, 0x99), view.dark);
            painter.text(to(&[*x, y - 4.0]), egui::Align2::LEFT_BOTTOM, name, FontId::proportional(13.0 * view.zoom.max(0.5)), color);
        }
    }
    if let (ElementKind::Text { x, y, value }, false) = (&element.kind, hide_text) {
        let size = element.style.font_size;
        let font = FontId::new(size * view.zoom, egui::FontFamily::Name(HAND_FONT.into()));
        let width = local_bounds(element).width() * view.zoom;
        for (i, line) in text_lines(value).into_iter().enumerate() {
            let galley = painter.layout_no_wrap(line.to_string(), font.clone(), stroke_color);
            // labels are centered like the reference app's container text
            let indent = if is_label(element) { (width - galley.size().x).max(0.0) / 2.0 / view.zoom } else { 0.0 };
            let pos = to(&[*x + indent, y + i as f32 * size * LINE_HEIGHT]);
            if halo {
                // background-colored outline keeps text readable over lines
                let ring = (size * view.zoom * 0.08).clamp(1.0, 3.0);
                for k in 0..12 {
                    let offset = Vec2::angled(k as f32 / 12.0 * std::f32::consts::TAU) * ring;
                    let shadow = painter.layout_no_wrap(line.to_string(), font.clone(), view.background);
                    painter.add(egui::epaint::TextShape::new(pos + offset, shadow, view.background).with_angle(angle));
                }
            }
            painter.add(egui::epaint::TextShape::new(pos, galley, stroke_color).with_angle(angle));
        }
    }
}

pub(crate) const IMAGE_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp"];

/// Decoded image textures, keyed by a hash of the data URL so the same picture uploads once.
pub(crate) fn image_texture(ctx: &Context, url: &str) -> Option<egui::TextureHandle> {
    use std::hash::{Hash, Hasher};
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<u64, Option<egui::TextureHandle>>> = Default::default();
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let key = hasher.finish();
    CACHE.with_borrow_mut(|cache| {
        cache
            .entry(key)
            .or_insert_with(|| {
                let bytes = base64_decode(url.split_once(";base64,")?.1)?;
                let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
                let image = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba);
                Some(ctx.load_texture(format!("image-{key}"), image, egui::TextureOptions::LINEAR))
            })
            .clone()
    })
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    };
    let digits: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').map(value).collect::<Option<_>>()?;
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, d)| n | (*d as u32) << (18 - 6 * i));
        out.extend(n.to_be_bytes()[1..chunk.len()].iter());
    }
    Some(out)
}

pub(crate) fn sticky_style(style: Style) -> Style {
    Style { stroke: BLACK, fill: Some([0xff, 0xdf, 0x6b, 255]), fill_style: FillStyle::Solid, roughness: 1, round: true, ..style }
}

/// (year, month 1-12, day) of a UTC day count, Howard Hinnant's civil_from_days.
pub(crate) fn civil_date(ms: i64) -> (i64, usize, i64) {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month as usize, doy - (153 * mp + 2) / 5 + 1)
}

/// The note's creation-date footer: "20 Oct", plus the year when it isn't this one.
// ponytail: UTC day, not the local time zone; add a tz crate if notes made near midnight matter.
pub(crate) fn sticky_date(created: i64, short: bool) -> Option<String> {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    if created <= 0 {
        return None;
    }
    let (year, month, day) = civil_date(created);
    let label = format!("{day} {}", MONTHS[month - 1]);
    Some(if short || year == civil_date(now_ms()).0 { label } else { format!("{label} {year}") })
}

/// Rounded-rectangle outline, corner radius 4% of the short side up to 16 (sticky notes).
pub(crate) fn sticky_outline(b: Rect, round: bool, shift: f32) -> Vec<[f32; 2]> {
    let r = if round { (b.width().min(b.height()) * 0.04).min(16.0) } else { 0.0 };
    let b = b.translate(Vec2::splat(shift));
    let corners = [(b.right_bottom(), 0.0), (b.left_bottom(), 0.25), (b.left_top(), 0.5), (b.right_top(), 0.75)];
    corners
        .into_iter()
        .flat_map(|(c, turn)| {
            let center = c + Vec2::new(if turn == 0.0 || turn == 0.75 { -r } else { r }, if turn < 0.5 { -r } else { r });
            (0..=6).map(move |k| {
                let a = (turn + k as f32 / 24.0) * std::f32::consts::TAU;
                [center.x + r * a.cos(), center.y + r * a.sin()]
            })
        })
        .collect()
}
