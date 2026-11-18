//! Text that holds still while the canvas zooms (asked: "o conteúdo de alguns elementos fica tremendo, como notas", and
//! "dar zoom no Linux dá uma sensação de estar travado, em notebooks que não têm placa de vídeo"): laid out at the zoom
//! the app holds while the zoom moves (`DrawingApp::text_zoom`) and drawn scaled to the frame's zoom. A note's lines
//! break where they break at 100%, whatever the zoom, and only the lines on screen are laid out: its plan keeps where
//! each line is (asked: "deixar só o que está na visual da tela, o restante vai aparecendo").

use crate::prelude::*;
use std::sync::Arc;

/// `galley` drawn `scale` times its size: its rows and their quads moved and grown, the glyphs' pictures the same (a
/// little soft until the zoom rests and the text is laid out at its size again).
pub(crate) fn scaled_galley(galley: &Arc<egui::Galley>, scale: f32) -> Arc<egui::Galley> {
    if (scale - 1.0).abs() < 1e-4 {
        return galley.clone();
    }
    let grow = |rect: Rect| Rect::from_min_max((rect.min.to_vec2() * scale).to_pos2(), (rect.max.to_vec2() * scale).to_pos2());
    let mut scaled = (**galley).clone();
    for placed in &mut scaled.rows {
        placed.pos = (placed.pos.to_vec2() * scale).to_pos2();
        let row = Arc::make_mut(&mut placed.row);
        row.size *= scale;
        row.visuals.mesh_bounds = grow(row.visuals.mesh_bounds);
        for vertex in &mut row.visuals.mesh.vertices {
            vertex.pos = (vertex.pos.to_vec2() * scale).to_pos2();
        }
        for glyph in &mut row.glyphs {
            glyph.pos = (glyph.pos.to_vec2() * scale).to_pos2();
            glyph.advance_width *= scale;
        }
    }
    scaled.rect = grow(scaled.rect);
    scaled.mesh_bounds = grow(scaled.mesh_bounds);
    Arc::new(scaled)
}

/// `text` laid out at `zoom` (screen points of a text at `size` world points) the way the canvas draws it while
/// `text_zoom` is held: at `text_zoom`, scaled to `zoom`.
pub(crate) fn steady_text(painter: &Painter, text: &str, font: impl Fn(f32) -> FontId, color: Color32, zoom: f32, text_zoom: f32) -> Arc<egui::Galley> {
    let galley = painter.layout_no_wrap(text.to_owned(), font(text_zoom), color);
    scaled_galley(&galley, zoom / text_zoom)
}

/// One source line of a note's text as it sits on the card: its top under the text box's (note points, unscrolled), its
/// height, and the characters where its text breaks into the next row.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PlannedLine {
    pub(crate) top: f32,
    pub(crate) height: f32,
    pub(crate) breaks: Vec<usize>,
}

/// Where the lines of a note's text go at 100%, and where the text ends.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NotePlan {
    pub(crate) lines: Vec<PlannedLine>,
    pub(crate) height: f32,
}

/// The plans of the notes painted lately, by their text, width and letter size: made again only when one of them
/// changes, not at every frame nor at every zoom. ponytail: emptied whole past 256 plans, a few KB each.
pub(crate) fn cached_plan(text: &str, width: f32, size: f32, make: impl FnOnce() -> NotePlan) -> Arc<NotePlan> {
    thread_local! {
        static PLANS: std::cell::RefCell<std::collections::HashMap<(u64, u32, u32), Arc<NotePlan>>> = Default::default();
    }
    let key = (fingerprint(text.as_bytes()), width.to_bits(), size.to_bits());
    if let Some(plan) = PLANS.with_borrow(|plans| plans.get(&key).cloned()) {
        return plan;
    }
    let plan = Arc::new(make());
    PLANS.with_borrow_mut(|plans| {
        if plans.len() >= 256 {
            plans.clear();
        }
        plans.insert(key, plan.clone());
    });
    plan
}

/// Where a note's text is written: measured at 100% for its plan, or painted at the frame's zoom.
pub(crate) enum Pen<'a> {
    Measure(&'a Painter),
    Paint { painter: &'a Painter, place: &'a dyn Fn(Pos2) -> Pos2, angle: f32, zoom: f32, text_zoom: f32 },
}

impl Pen<'_> {
    /// One run of a note's text at `at` (note points): its height in note points and where it broke into rows.
    /// Painting, it breaks at `breaks` (the plan's) instead of wrapping at this zoom.
    pub(crate) fn text(&self, at: Pos2, spans: &[markdown::Span], size: f32, ink: Color32, wrap: f32, breaks: &[usize]) -> (f32, Vec<usize>) {
        let least = size * markdown::NOTE_LINE;
        match self {
            Pen::Measure(painter) => {
                let galley = painter.layout_job(markdown::job(spans, size, ink, wrap, false));
                let mut at_char = 0;
                let mut broke = Vec::new();
                for placed in &galley.rows {
                    if at_char > 0 && !broke.contains(&at_char) {
                        broke.push(at_char);
                    }
                    at_char += placed.row.glyphs.len() + usize::from(placed.ends_with_newline);
                }
                (galley.size().y.max(least), broke)
            }
            Pen::Paint { painter, place, angle, zoom, text_zoom } => {
                let spans = broken(spans, breaks);
                let scale = zoom / text_zoom;
                let pos = place(at);
                let galley = painter.layout_job(markdown::job(&spans, size * text_zoom, ink, f32::INFINITY, false));
                painter.add(egui::epaint::TextShape::new(pos, scaled_galley(&galley, scale), ink).with_angle(*angle));
                if spans.iter().any(|span| span.bold) {
                    // egui has no bold face: the bold spans are drawn again half a point to the right
                    let bold = scaled_galley(&painter.layout_job(markdown::job(&spans, size * text_zoom, ink, f32::INFINITY, true)), scale);
                    for dx in [0.0, 0.5 * zoom] {
                        painter.add(egui::epaint::TextShape::new(pos + Vec2::new(dx, 0.0), bold.clone(), ink).with_angle(*angle));
                    }
                }
                ((breaks.len() + 1) as f32 * least, Vec::new())
            }
        }
    }

    /// Painting: the shape `make` gives (from screen points). Measuring: nothing.
    pub(crate) fn shape(&self, make: impl FnOnce(&Painter, &dyn Fn(Pos2) -> Pos2, f32)) {
        if let Pen::Paint { painter, place, zoom, .. } = self {
            make(painter, place, *zoom);
        }
    }
}

/// `spans` with a new row started at each of `breaks` (characters counted across the spans).
pub(crate) fn broken(spans: &[markdown::Span], breaks: &[usize]) -> Vec<markdown::Span> {
    if breaks.is_empty() {
        return spans.to_vec();
    }
    let mut at_char = 0;
    spans
        .iter()
        .map(|span| {
            let mut text = String::with_capacity(span.text.len() + breaks.len());
            for character in span.text.chars() {
                if at_char > 0 && breaks.contains(&at_char) {
                    text.push('\n');
                }
                text.push(character);
                at_char += 1;
            }
            markdown::Span { text, ..span.clone() }
        })
        .collect()
}
