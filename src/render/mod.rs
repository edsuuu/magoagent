//! Painting elements on the egui canvas, image textures and sticky notes.

use crate::prelude::*;

pub(crate) mod markdown;
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
    if element.created.is_some() {
        // a sticky note without its text (replays): the card alone
        paint_note(painter, element, "", false, view, faded);
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

/// The reference app's note papers, in its order, with their names; yellow is the default.
pub(crate) const NOTE_PAPERS: [([u8; 4], &str); 10] = [
    ([0xff, 0xf9, 0xc4, 255], "Amarelo"),
    ([0xf8, 0xbb, 0xd0, 255], "Rosa"),
    ([0xbb, 0xde, 0xfb, 255], "Azul"),
    ([0xc8, 0xe6, 0xc9, 255], "Verde"),
    ([0xff, 0xe0, 0xb2, 255], "Laranja"),
    ([0xe1, 0xbe, 0xe7, 255], "Roxo"),
    ([0xff, 0xff, 0xff, 255], "Branco"),
    ([0x2d, 0x2d, 0x2d, 255], "Antracito"),
    ([0x37, 0x47, 0x4f, 255], "Ardósia"),
    ([0x1a, 0x23, 0x7e, 255], "Meia-noite"),
];
pub(crate) const STICKY_PAPER: [u8; 4] = NOTE_PAPERS[0].0;
/// The header strip above a note's text, and the padding around the text, in points.
pub(crate) const NOTE_HEADER: f32 = 16.0;
pub(crate) const NOTE_PAD: f32 = 10.0;

pub(crate) fn sticky_style(style: Style) -> Style {
    Style { stroke: BLACK, fill: Some(STICKY_PAPER), fill_style: FillStyle::Solid, roughness: 1, round: true, ..style }
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

/// Rounded-rectangle outline of a sticky note: 8-point corners (rounded-lg), square when not round.
pub(crate) fn sticky_outline(b: Rect, round: bool, shift: f32) -> Vec<[f32; 2]> {
    let r = if round { 8.0 } else { 0.0 };
    corner_outline(b.translate(Vec2::splat(shift)), [r; 4]).into_iter().map(|p| [p.x, p.y]).collect()
}

/// Outline of a rectangle with each corner rounded by its radius, from the top-left clockwise
/// (nw, ne, se, sw). A zero radius is a sharp corner.
pub(crate) fn corner_outline(b: Rect, radii: [f32; 4]) -> Vec<Pos2> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let corners = [
        (Pos2::new(b.min.x + radii[0], b.min.y + radii[0]), PI),
        (Pos2::new(b.max.x - radii[1], b.min.y + radii[1]), 3.0 * FRAC_PI_2),
        (Pos2::new(b.max.x - radii[2], b.max.y - radii[2]), 0.0),
        (Pos2::new(b.min.x + radii[3], b.max.y - radii[3]), FRAC_PI_2),
    ];
    corners
        .into_iter()
        .zip(radii)
        .flat_map(|((c, start), r)| {
            let steps = if r > 0.0 { 6 } else { 0 };
            (0..=steps).map(move |k| {
                let a = start + k as f32 / 6.0 * FRAC_PI_2;
                c + Vec2::new(r * a.cos(), r * a.sin())
            })
        })
        .collect()
}

/// The text box of a note in its own points: under the header, inside the padding.
pub(crate) fn note_text_box(note: &Element) -> Rect {
    let b = local_bounds(note);
    Rect::from_min_max(b.min + Vec2::new(NOTE_PAD, NOTE_HEADER + NOTE_PAD), b.max - Vec2::splat(NOTE_PAD))
}

/// The paper (the palette's color as it is, papers don't turn over in dark mode) and the ink on
/// it: dark on light paper, light on dark, the way the reference app's contrastingTextColor picks it.
pub(crate) fn note_colors(note: &Element, opacity: u8) -> (Color32, Color32) {
    let paper = with_opacity(note.style.fill.unwrap_or(STICKY_PAPER), opacity);
    let [r, g, b, _] = paper.to_srgba_unmultiplied();
    let linear = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let light = 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b) > 0.4;
    (paper, if light { Color32::from_rgb(0x33, 0x33, 0x33) } else { Color32::WHITE })
}

/// A sticky note like the reference app's card: a soft shadow, the paper with a hairline edge, a
/// header strip with the title and the date, and the text as markdown. `body` is false while the
/// editor types over the text.
/// Returns where the text ends (the note's own points, y); without `body`, the top of its box.
pub(crate) fn paint_note(painter: &Painter, note: &Element, text: &str, body: bool, view: &View, faded: bool) -> f32 {
    let opacity = if faded { note.style.opacity / 3 } else { note.style.opacity };
    let alpha = opacity as f32 / 100.0;
    let (center, angle, zoom) = (note.center(), note.angle, view.zoom);
    let place = |p: Pos2| {
        let p = if angle == 0.0 { p } else { center + rot(p - center, angle) };
        Pos2::new(view.origin.x + p.x * zoom, view.origin.y + p.y * zoom)
    };
    let outline = |rect: Rect, radii: [f32; 4]| -> Vec<Pos2> { corner_outline(rect, radii).into_iter().map(|p| place(p)).collect() };
    let b = local_bounds(note);
    let radius = if note.style.round { 8.0 } else { 0.0 };
    let (paper, ink) = note_colors(note, opacity);
    let header = Rect::from_min_size(b.min, Vec2::new(b.width(), NOTE_HEADER));
    let strip = if ink == Color32::WHITE { Color32::from_white_alpha(26) } else { Color32::from_black_alpha(15) };
    let card = Rect::from_two_pos(place(b.min), place(b.max));
    // the reference app's --shadow-node: 0 3px 18px, a hairline ring; the dark theme's is darker
    let shadow = egui::Shadow {
        offset: [0, (3.0 * zoom) as i8],
        blur: (18.0 * zoom) as u8,
        spread: 0,
        color: Color32::from_black_alpha(if view.dark { 0x52 } else { 0x24 }).gamma_multiply(alpha),
    };
    painter.add(shadow.as_shape(card, radius * zoom));
    painter.add(Shape::convex_polygon(outline(b, [radius; 4]), paper, Stroke::new(0.5 * zoom, Color32::from_black_alpha(20).gamma_multiply(alpha))));
    painter.add(Shape::convex_polygon(outline(header, [radius, radius, 0.0, 0.0]), strip.gamma_multiply(alpha), Stroke::NONE));
    let ink = ink.gamma_multiply(alpha);
    let muted = ink.gamma_multiply(0.6);
    let header_clip = painter.with_clip_rect(Rect::from_two_pos(place(header.min), place(header.max)));
    let at = |x: f32| place(Pos2::new(x, b.min.y + NOTE_HEADER / 2.0));
    if !text.trim().is_empty() {
        header_clip.text(at(b.min.x + NOTE_PAD), egui::Align2::LEFT_CENTER, note_name(text), FontId::proportional(10.0 * zoom), muted);
    }
    if let (Some(created), true) = (note.created, b.width() >= 75.0 && b.height() >= 75.0) {
        if let Some(date) = sticky_date(created, b.width() - 32.0 < 80.0) {
            header_clip.text(at(b.max.x - NOTE_PAD), egui::Align2::RIGHT_CENTER, date, FontId::proportional(10.0 * zoom), muted);
        }
    }
    let text_box = note_text_box(note);
    if !body {
        return text_box.min.y;
    }
    // ponytail: the text is clipped to the card, not scrolled; the editor scrolls it while typing
    let clip = painter.with_clip_rect(card);
    let fs = note.note.font_size;
    if note.note.raw {
        // the markdown source as it is typed, in the editor's monospace
        let galley = painter.layout_job(markdown::edit_job(text, fs * zoom, ink, text_box.width() * zoom));
        let bottom = text_box.min.y + galley.size().y / zoom;
        clip.galley(place(text_box.min), galley, ink);
        return bottom;
    }
    let (left, width, lh) = (text_box.min.x, text_box.width(), fs * markdown::NOTE_LINE);
    let mut y = text_box.min.y;
    for line in markdown::lines(text) {
        y += match line {
            markdown::Line::Blank => lh,
            markdown::Line::Rule => {
                let at = y + 6.0;
                clip.line_segment([place(Pos2::new(left, at)), place(Pos2::new(left + width, at))], Stroke::new(zoom, ink.gamma_multiply(0.2)));
                12.0
            }
            markdown::Line::Heading(level, t) => {
                note_line(&clip, &place, angle, zoom, Pos2::new(left, y), &markdown::inline(t, true), markdown::size_of(level, fs), ink, width)
            }
            markdown::Line::Quote(t) => {
                let bar = Rect::from_min_size(Pos2::new(left, y), Vec2::new(2.5, lh));
                clip.rect_filled(Rect::from_two_pos(place(bar.min), place(bar.max)), 0.0, ink.gamma_multiply(0.2));
                note_line(&clip, &place, angle, zoom, Pos2::new(left + 12.5, y), &markdown::inline(t, false), fs, ink.gamma_multiply(0.7), width - 12.5)
            }
            markdown::Line::Code(t) => {
                let row = Rect::from_min_size(Pos2::new(left, y), Vec2::new(width, lh));
                clip.rect_filled(Rect::from_two_pos(place(row.min), place(row.max)), 0.0, ink.gamma_multiply(0.05));
                let plain = [markdown::Span { text: t.into(), ..Default::default() }];
                note_line(&clip, &place, angle, zoom, Pos2::new(left, y), &plain, fs, ink, width)
            }
            markdown::Line::Para(t) => note_line(&clip, &place, angle, zoom, Pos2::new(left, y), &markdown::inline(t, false), fs, ink, width),
            markdown::Line::Item { depth, mark, text: t } => {
                let x = left + depth as f32 * 16.0;
                // numbers are wider than a bullet, so their text starts further in
                let gap = if matches!(mark, markdown::Mark::Number(_)) { 24.0 } else { 18.0 };
                let mut line_ink = ink;
                let mut done = false;
                match mark {
                    markdown::Mark::Bullet => {
                        clip.circle_filled(place(Pos2::new(x + 4.0, y + lh / 2.0)), 2.0 * zoom, ink.gamma_multiply(0.4));
                    }
                    markdown::Mark::Number(n) => {
                        let marker = [markdown::Span { text: n, ..Default::default() }];
                        note_line(&clip, &place, angle, zoom, Pos2::new(x, y), &marker, fs, ink.gamma_multiply(0.4), f32::INFINITY);
                    }
                    markdown::Mark::Task(checked) => {
                        let box_world = Rect::from_min_size(Pos2::new(x, y + (lh - 12.0) / 2.0), Vec2::splat(12.0));
                        let r = Rect::from_two_pos(place(box_world.min), place(box_world.max));
                        if checked {
                            clip.rect_filled(r, 2.0 * zoom, ink.gamma_multiply(0.5));
                        } else {
                            clip.rect_stroke(r, 2.0 * zoom, Stroke::new(zoom, ink.gamma_multiply(0.5)), StrokeKind::Inside);
                        }
                        done = checked;
                        line_ink = if checked { ink.gamma_multiply(0.5) } else { ink };
                    }
                }
                // a done task is struck through and faded
                let spans: Vec<markdown::Span> = markdown::inline(t, false).into_iter().map(|s| markdown::Span { strike: done || s.strike, ..s }).collect();
                note_line(&clip, &place, angle, zoom, Pos2::new(x + gap, y), &spans, fs, line_ink, (width - (x + gap - left)).max(1.0))
            }
        };
    }
    y
}

/// Paints one run of a note's text at `at` (points in the card) and returns its height in points.
fn note_line(
    painter: &Painter,
    place: &dyn Fn(Pos2) -> Pos2,
    angle: f32,
    zoom: f32,
    at: Pos2,
    spans: &[markdown::Span],
    size: f32,
    ink: Color32,
    wrap: f32,
) -> f32 {
    let galley = painter.layout_job(markdown::job(spans, size * zoom, ink, wrap * zoom, false));
    let pos = place(at);
    painter.add(egui::epaint::TextShape::new(pos, galley.clone(), ink).with_angle(angle));
    if spans.iter().any(|s| s.bold) {
        // egui has no bold face: the bold spans are drawn again half a point to the right
        let bold = painter.layout_job(markdown::job(spans, size * zoom, ink, wrap * zoom, true));
        for dx in [0.0, 0.5 * zoom] {
            painter.add(egui::epaint::TextShape::new(pos + Vec2::new(dx, 0.0), bold.clone(), ink).with_angle(angle));
        }
    }
    (galley.size().y / zoom).max(size * markdown::NOTE_LINE)
}
