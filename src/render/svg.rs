//! SVG and PNG export (resvg) with the hand font and symbol fallbacks.

use crate::prelude::*;

pub(crate) fn hex(c: [u8; 4]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

pub(crate) fn svg_points(points: &[[f32; 2]], offset: [f32; 2]) -> String {
    points
        .iter()
        .map(|p| format!("{:.2},{:.2}", p[0] + offset[0], p[1] + offset[1]))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn scene_svg(scene: &Scene, background: Option<[u8; 4]>) -> String {
    let bounds = scene
        .elements
        .iter()
        // a frame's name sits above it
        .map(|e| if e.frame.is_some() { element_bounds(e).union(element_bounds(e).translate(Vec2::new(0.0, -22.0))) } else { element_bounds(e) })
        .reduce(|a, b| a.union(b))
        .unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(160.0, 120.0)));
    let width = bounds.width().max(1.0) + 20.0;
    let height = bounds.height().max(1.0) + 20.0;
    let offset = [10.0 - bounds.min.x, 10.0 - bounds.min.y];
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    );
    svg.push_str("<defs><style>");
    svg.push_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-font-svg.css")));
    svg.push_str("</style></defs>");
    if let Some(bg) = background {
        svg.push_str(&format!("<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>", hex(bg)));
    }
    for element in &scene.elements {
        let st = &element.style;
        let stroke = hex(st.stroke);
        let opacity = st.opacity as f32 / 100.0;
        let transform = if element.angle == 0.0 {
            String::new()
        } else {
            let c = element.center();
            format!(" transform=\"rotate({} {} {})\"", element.angle.to_degrees(), c.x + offset[0], c.y + offset[1])
        };
        svg.push_str(&format!("<g opacity=\"{opacity}\"{transform}>"));
        if let Some(url) = &element.image {
            let b = local_bounds(element);
            svg.push_str(&format!(
                "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"/></g>",
                xml_escape(url), b.min.x + offset[0], b.min.y + offset[1], b.width(), b.height()
            ));
            continue;
        }
        if let Some(created) = element.created {
            let b = local_bounds(element);
            svg.push_str(&format!(
                "<polygon points=\"{}\" fill=\"#000000\" fill-opacity=\"0.16\"/><polygon points=\"{}\" fill=\"{}\" stroke=\"#000000\" stroke-opacity=\"0.08\" stroke-width=\"0.5\"/>",
                svg_points(&sticky_outline(b, st.round, 3.0), offset),
                svg_points(&sticky_outline(b, st.round, 0.0), offset),
                hex(st.fill.unwrap_or([0xff, 0xdf, 0x6b, 255]))
            ));
            if let Some(date) = sticky_date(created, b.width() - 32.0 < 80.0).filter(|_| b.width() >= 75.0 && b.height() >= 75.0) {
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-family=\"Helvetica, Arial, sans-serif\" font-size=\"12\" fill=\"{stroke}\">{date}</text>",
                    b.max.x - 16.0 + offset[0],
                    b.max.y - 14.0 + offset[1]
                ));
            }
            svg.push_str("</g>");
            continue;
        }
        if let (Some(fill), Some(poly)) = (st.fill, shape_polygon(element)) {
            if st.fill_style == FillStyle::Solid {
                svg.push_str(&format!("<polygon points=\"{}\" fill=\"{}\"/>", svg_points(&poly, offset), hex(fill)));
            }
            for line in fill_strokes(element) {
                svg.push_str(&format!(
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
                    svg_points(&line, offset),
                    hex(fill),
                    st.stroke_width / 2.0
                ));
            }
        }
        if let (Some(name), ElementKind::Rectangle { x, y, .. }) = (&element.frame, &element.kind) {
            if !name.is_empty() {
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-family=\"Assistant, sans-serif\" font-size=\"13\" fill=\"#999999\">{}</text>",
                    x + offset[0],
                    y + offset[1] - 5.0,
                    xml_escape(name)
                ));
            }
        }
        if let ElementKind::Text { x, y, value } = &element.kind {
            let b = local_bounds(element);
            if is_arrow_label(&scene.elements, element) {
                let m = b.expand(4.0);
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"{}\"/>",
                    m.min.x + offset[0], m.min.y + offset[1], m.width(), m.height(), hex(background.unwrap_or(scene.background))
                ));
            }
            let (anchor, tx) = if is_label(element) { ("middle", b.center().x) } else { ("start", *x) };
            for (i, line) in text_lines(value).into_iter().enumerate() {
                svg.push_str(&svg_text_line(
                    tx + offset[0],
                    y + offset[1] + st.font_size * (0.95 + i as f32 * LINE_HEIGHT),
                    anchor == "middle",
                    st.font_size,
                    &stroke,
                    text_needs_halo(&scene.elements, element).then(|| hex(background.unwrap_or(scene.background))).as_deref(),
                    line,
                ));
            }
        }
        let dash = match st.stroke_style {
            StrokeStyle::Solid => String::new(),
            StrokeStyle::Dashed => format!(" stroke-dasharray=\"8 {}\"", 8.0 + st.stroke_width),
            StrokeStyle::Dotted => format!(" stroke-dasharray=\"1.5 {}\"", 6.0 + st.stroke_width),
        };
        for outline in rough_outlines(element) {
            svg.push_str(&format!(
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{dash}/>",
                svg_points(&outline, offset),
                st.stroke_width
            ));
        }
        for (shape, head) in arrowheads(element) {
            let fill = match head.fill() {
                Some(true) => stroke.clone(),
                Some(false) => hex(background.unwrap_or(scene.background)),
                None => "none".into(),
            };
            let tag = if head.fill().is_some() { "polygon" } else { "polyline" };
            svg.push_str(&format!(
                "<{tag} points=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                svg_points(&shape, offset),
                st.stroke_width
            ));
        }
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    svg
}

pub(crate) fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// System fonts used as fallback for arrows, check marks and other symbols; missing ones are skipped.
#[cfg(target_os = "macos")]
pub(crate) const SYMBOL_FONTS: &[&str] = &["/System/Library/Fonts/Apple Symbols.ttf", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"];
#[cfg(windows)]
pub(crate) const SYMBOL_FONTS: &[&str] = &["C:\\Windows\\Fonts\\seguisym.ttf", "C:\\Windows\\Fonts\\segoeui.ttf"];
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) const SYMBOL_FONTS: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
];

/// Built once: bundled fonts first, then every system font as fallback for missing glyphs.
pub(crate) fn export_fonts() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static FONTS: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            // the hand font ships as 7 unicode-range subsets; the basic-Latin one (5) must be found first
            for data in [
                &include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-5.ttf"))[..],
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-1.ttf")),
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-2.ttf")),
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-3.ttf")),
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-4.ttf")),
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-6.ttf")),
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-7.ttf")),
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Assistant.ttf")),
            ] {
                db.load_font_data(data.to_vec());
            }
            for path in SYMBOL_FONTS.iter().copied() {
                let _ = db.load_font_file(path);
            }
            let family = db.faces().next().map(|f| f.families[0].0.clone());
            db.load_system_fonts(); // emoji and anything else, memory-mapped
            // SVGs ask for "MagoHand, cursive"; cursive resolves to the hand font's own family name.
            // Set after the system fonts: on Linux loading them applies fontconfig's generic families.
            if let Some(family) = family {
                db.set_cursive_family(family);
            }
            db.set_sans_serif_family("Assistant");
            std::sync::Arc::new(db)
        })
        .clone()
}

/// One line of SVG text. Characters the hand font lacks (→ ✓ ★ …) become their own `<text>`
/// in a font that has them, placed by real glyph advances; resvg would otherwise reshape
/// the whole line in the fallback font.
pub(crate) fn svg_text_line(x: f32, y: f32, centered: bool, size: f32, fill: &str, halo: Option<&str>, line: &str) -> String {
    let db = export_fonts();
    // the 7 bundled hand-font subsets are the first faces loaded
    let hand_faces: Vec<_> = db.faces().take(7).map(|f| f.id).collect();
    let face_of = |ch: char| -> Option<(resvg::usvg::fontdb::ID, bool)> {
        let mut hand = None;
        let mut other = None;
        for face in db.faces() {
            let covers = db
                .with_face_data(face.id, |data, index| {
                    ttf_parser::Face::parse(data, index).ok().and_then(|f| f.glyph_index(ch)).is_some()
                })
                .unwrap_or(false);
            if covers {
                if hand_faces.contains(&face.id) {
                    hand = Some(face.id);
                    break;
                }
                other = other.or(Some(face.id));
            }
        }
        hand.map(|id| (id, true)).or(other.map(|id| (id, false)))
    };
    let advance = |id, ch: char| {
        db.with_face_data(id, |data, index| {
            let face = ttf_parser::Face::parse(data, index).ok()?;
            let glyph = face.glyph_index(ch)?;
            Some(face.glyph_hor_advance(glyph)? as f32 / face.units_per_em() as f32 * size)
        })
        .flatten()
        .unwrap_or(size * CHAR_WIDTH)
    };
    // runs of (text, fallback family or None for the hand font, width)
    let mut runs: Vec<(String, Option<String>, f32)> = Vec::new();
    for ch in line.chars() {
        let (family, width) = match face_of(ch) {
            Some((id, true)) => (None, advance(id, ch)),
            Some((id, false)) => (db.face(id).map(|f| f.families[0].0.clone()), advance(id, ch)),
            None => (None, size * CHAR_WIDTH),
        };
        match runs.last_mut() {
            Some((text, f, w)) if *f == family || ch.is_whitespace() => {
                text.push(ch);
                *w += width;
            }
            _ => runs.push((ch.to_string(), family, width)),
        }
    }
    let total: f32 = runs.iter().map(|r| r.2).sum();
    let mut cursor = if centered { x - total / 2.0 } else { x };
    let halo_attr = halo.map_or(String::new(), |color| {
        format!(" stroke=\"{color}\" stroke-width=\"{:.1}\" stroke-linejoin=\"round\" paint-order=\"stroke\"", (size * 0.16).clamp(2.0, 6.0))
    });
    let mut out = String::new();
    for (text, family, width) in runs {
        out.push_str(&format!(
            "<text x=\"{cursor}\" y=\"{y}\" font-family=\"{}\" font-size=\"{size}\" fill=\"{fill}\"{halo_attr} xml:space=\"preserve\">{}</text>",
            xml_escape(family.as_deref().unwrap_or("MagoHand, cursive")),
            xml_escape(&text)
        ));
        cursor += width;
    }
    out
}

pub(crate) fn render_svg_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb = export_fonts();
    options.font_family = "Assistant".into();
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size().scale_by(scale).ok_or("Tamanho de imagem inválido")?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("Tamanho de imagem inválido")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| e.to_string())
}
