//! Drawings on the canvas, in world cells: strokes (lines, arrows, the pencil, diamonds, ellipses)
//! in braille dots, 2×4 per cell; rectangles in box-drawing characters; text as it was typed.

use crate::{AMBER, TOP, Tool, put, put_str};
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Shape {
    pub(crate) kind: Tool,
    /// World cells: a drag's two corners or ends, every point of a pencil stroke, a text's corner.
    pub(crate) points: Vec<(f32, f32)>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) text: String,
}

/// The bit of each braille dot, by its column (0–1) and row (0–3) in the cell.
const DOT: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

fn cell(p: (f32, f32)) -> (i32, i32) {
    (p.0.floor() as i32, p.1.floor() as i32)
}

impl Shape {
    pub(crate) fn new(kind: Tool, at: (f32, f32)) -> Self {
        let points = if matches!(kind, Tool::Draw | Tool::Text) { vec![at] } else { vec![at, at] };
        Self { kind, points, text: String::new() }
    }

    /// The pointer went on to `at` while drawing it.
    pub(crate) fn extend(&mut self, at: (f32, f32)) {
        if self.kind == Tool::Draw {
            if self.points.last() != Some(&at) {
                self.points.push(at);
            }
        } else if let Some(end) = self.points.get_mut(1) {
            *end = at;
        }
    }

    /// Nothing to keep: a drag that never left its first cell, a text left blank.
    pub(crate) fn empty(&self) -> bool {
        match self.kind {
            Tool::Text => self.text.trim().is_empty(),
            Tool::Draw => false,
            _ => self.points.first() == self.points.get(1),
        }
    }

    pub(crate) fn shift(&mut self, dx: f32, dy: f32) {
        for p in &mut self.points {
            *p = (p.0 + dx, p.1 + dy);
        }
    }

    /// The cells it spans, both corners included.
    fn bounds(&self) -> (i32, i32, i32, i32) {
        let cells = self.points.iter().map(|&p| cell(p));
        cells.fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |(x0, y0, x1, y1), (x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y)))
    }

    /// A rectangle big enough for its box-drawing border; a thinner one is a braille line.
    fn boxed(&self) -> Option<(i32, i32, i32, i32)> {
        let b = self.bounds();
        (self.kind == Tool::Rectangle && b.2 > b.0 && b.3 > b.1).then_some(b)
    }

    /// Where a text's cursor goes: after its last character.
    pub(crate) fn text_end(&self) -> (i32, i32) {
        let (x, y) = cell(self.points[0]);
        let last = self.text.split('\n').count() as i32 - 1;
        (x + self.text.rsplit('\n').next().map_or(0, |l| l.chars().count()) as i32, y + last)
    }

    /// A click on world cell `at` lands on it.
    pub(crate) fn hit(&self, at: (i32, i32)) -> bool {
        match self.kind {
            Tool::Text => {
                let (x, y) = cell(self.points[0]);
                let width = self.text.split('\n').map(|l| l.chars().count()).max().unwrap_or(0).max(1) as i32;
                (x..x + width).contains(&at.0) && (y..y + self.text.split('\n').count() as i32).contains(&at.1)
            }
            Tool::Rectangle | Tool::Diamond | Tool::Ellipse => {
                let (x0, y0, x1, y1) = self.bounds();
                (x0..=x1).contains(&at.0) && (y0..=y1).contains(&at.1)
            }
            _ => {
                let p = (at.0 as f32 + 0.5, at.1 as f32 + 0.5);
                let first = [self.points[0]; 2];
                let segments = if self.points.len() == 1 { first.windows(2) } else { self.points.windows(2) };
                segments.into_iter().any(|s| distance(p, s[0], s[1]) <= 1.0)
            }
        }
    }

    /// The braille dots its strokes cover, in world dots (a cell is 2 wide and 4 tall).
    fn dots(&self) -> Vec<(i32, i32)> {
        let points: Vec<(i32, i32)> = self.points.iter().map(|p| ((p.0 * 2.0).floor() as i32, (p.1 * 4.0).floor() as i32)).collect();
        let (a, b) = (points[0], points[points.len() - 1]);
        let (x0, y0, x1, y1) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
        let (cx, cy) = ((x0 + x1) / 2, (y0 + y1) / 2);
        let path = match self.kind {
            Tool::Line | Tool::Arrow | Tool::Draw => points,
            Tool::Rectangle => vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)],
            Tool::Diamond => vec![(cx, y0), (x1, cy), (cx, y1), (x0, cy), (cx, y0)],
            Tool::Ellipse => {
                let (rx, ry) = ((x1 - x0) as f32 / 2.0, (y1 - y0) as f32 / 2.0);
                let (cx, cy) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
                let steps = ((rx + ry) as usize * 2).max(16);
                (0..=steps)
                    .map(|k| {
                        let t = k as f32 / steps as f32 * std::f32::consts::TAU;
                        ((cx + rx * t.cos()).round() as i32, (cy + ry * t.sin()).round() as i32)
                    })
                    .collect()
            }
            _ => return Vec::new(),
        };
        let mut dots = vec![path[0]];
        for pair in path.windows(2) {
            line(pair[0], pair[1], &mut dots);
        }
        if self.kind == Tool::Arrow {
            // the head: two strokes back from the tip, 30° to each side of the shaft
            let (dx, dy) = ((b.0 - a.0) as f32, (b.1 - a.1) as f32);
            let length = dx.hypot(dy);
            if length > 0.0 {
                let size = (length / 2.0).min(5.0);
                for angle in [0.5f32, -0.5] {
                    let (sin, cos) = angle.sin_cos();
                    let (ux, uy) = ((dx * cos - dy * sin) / length, (dx * sin + dy * cos) / length);
                    line(b, (b.0 - (ux * size).round() as i32, b.1 - (uy * size).round() as i32), &mut dots);
                }
            }
        }
        dots
    }
}

/// From a point to the segment a–b.
fn distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length2 = dx * dx + dy * dy;
    let t = if length2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length2).clamp(0.0, 1.0) };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// Bresenham: every dot from a to b.
fn line((mut x, mut y): (i32, i32), (x1, y1): (i32, i32), out: &mut Vec<(i32, i32)>) {
    let (dx, dy) = ((x1 - x).abs(), -(y1 - y).abs());
    let (sx, sy) = ((x1 - x).signum(), (y1 - y).signum());
    let mut err = dx + dy;
    loop {
        out.push((x, y));
        if (x, y) == (x1, y1) {
            return;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Paints the drawings with world cell `camera` at the canvas's top-left; the selected one in amber.
pub(crate) fn paint(buf: &mut Buffer, shapes: &[Shape], camera: (i32, i32), selected: Option<usize>) {
    let at = |x: i32, y: i32| (x - camera.0, y - camera.1 + TOP);
    let style = |selected: bool| if selected { Style::default().fg(AMBER) } else { Style::default() };
    // strokes first: their dots share cells, so a cell is only known once every stroke is in
    let mut braille: HashMap<(i32, i32), (u8, bool)> = HashMap::new();
    for (i, shape) in shapes.iter().enumerate() {
        if shape.kind == Tool::Text || shape.boxed().is_some() {
            continue;
        }
        for (x, y) in shape.dots() {
            let cell = braille.entry((x.div_euclid(2), y.div_euclid(4))).or_default();
            cell.0 |= DOT[x.rem_euclid(2) as usize][y.rem_euclid(4) as usize];
            cell.1 |= selected == Some(i);
        }
    }
    for ((x, y), (bits, selected)) in braille {
        let (sx, sy) = at(x, y);
        put(buf, sx, sy, char::from_u32(0x2800 + bits as u32).unwrap_or(' ').encode_utf8(&mut [0; 4]), style(selected));
    }
    for (i, shape) in shapes.iter().enumerate() {
        let style = style(selected == Some(i));
        if let Some((x0, y0, x1, y1)) = shape.boxed() {
            for x in x0 + 1..x1 {
                put(buf, at(x, y0).0, at(x, y0).1, "─", style);
                put(buf, at(x, y1).0, at(x, y1).1, "─", style);
            }
            for y in y0 + 1..y1 {
                put(buf, at(x0, y).0, at(x0, y).1, "│", style);
                put(buf, at(x1, y).0, at(x1, y).1, "│", style);
            }
            for (x, y, corner) in [(x0, y0, "╭"), (x1, y0, "╮"), (x0, y1, "╰"), (x1, y1, "╯")] {
                put(buf, at(x, y).0, at(x, y).1, corner, style);
            }
        } else if shape.kind == Tool::Text {
            let (x, y) = cell(shape.points[0]);
            for (n, text) in shape.text.split('\n').enumerate() {
                let (sx, sy) = at(x, y + n as i32);
                put_str(buf, sx, sy, text, style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    fn rows(shapes: &[Shape]) -> Vec<String> {
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 5));
        paint(&mut buf, shapes, (0, 0), None);
        (1..5).map(|y| (0..8).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end().to_string()).collect()
    }

    #[test]
    fn strokes_are_braille_and_rectangles_are_boxes() {
        let line = Shape { kind: Tool::Line, points: vec![(0.5, 0.5), (3.5, 0.5)], text: String::new() };
        // dots 1..=7 on the cells' third dot row: the first cell only has its right column
        assert_eq!(rows(std::slice::from_ref(&line))[0], "⠠⠤⠤⠤");
        let rectangle = Shape { kind: Tool::Rectangle, points: vec![(3.5, 2.5), (0.5, 0.5)], text: String::new() };
        assert_eq!(rows(&[rectangle])[..3], ["╭──╮", "│  │", "╰──╯"]);
        let text = Shape { kind: Tool::Text, points: vec![(1.5, 1.5)], text: "oi\nmago".into() };
        assert_eq!(rows(std::slice::from_ref(&text))[1..3], [" oi", " mago"]);
        assert!(text.hit((4, 2)) && !text.hit((5, 1)) && !text.hit((1, 3)), "a text is clicked inside the box of its lines");
        assert!(line.hit((2, 1)) && !line.hit((2, 3)), "a line is clicked up to a cell away");
    }

    #[test]
    fn an_arrow_has_a_head_at_its_end() {
        let arrow = Shape { kind: Tool::Arrow, points: vec![(0.5, 1.5), (6.5, 1.5)], text: String::new() };
        let line = Shape { kind: Tool::Line, ..arrow.clone() };
        assert!(arrow.dots().len() > line.dots().len());
        // the shaft stays on its row; the head's lower stroke reaches into the row under it
        let (with_head, without) = (rows(&[arrow]), rows(&[line]));
        assert!(without[2].is_empty() && !with_head[2].is_empty(), "{with_head:?}");
    }
}
