use crate::prelude::*;

/// The row of the other column that lines up with `row` of column `from` (0 old, 1 new). `knots` are the (old row, new row)
/// pairs where each change starts and ends: between two of them the rows interpolate (inside an unchanged stretch both
/// columns have the same rows, so it is 1:1); a zero-length side jumps over the other's lines.
// ponytail: a pure insertion jumps the other column by its whole length in one step; spread it if that bothers
pub(crate) fn map_row(knots: &[(f32, f32)], from: usize, row: f32) -> f32 {
    let pick = |pair: (f32, f32), side: usize| if side == 0 { pair.0 } else { pair.1 };
    let (to, row) = (1 - from, row.max(0.0));
    let mut before = (0.0, 0.0);
    for &knot in knots {
        let (start, end) = (pick(before, from), pick(knot, from));
        if row < end && end > start {
            return pick(before, to) + (row - start) / (end - start) * (pick(knot, to) - pick(before, to));
        }
        before = knot;
    }
    row - pick(before, from) + pick(before, to)
}

/// The strip that joins a change's lines of the left column (`left`: top and bottom y) to the right one's, from `x_left` to
/// `x_right`, with soft curved edges as the IDE draws it.
pub(crate) fn paint_band(painter: &egui::Painter, (x_left, x_right): (f32, f32), left: (f32, f32), right: (f32, f32), fill: Color32, edge: Stroke) {
    const STEPS: usize = 16;
    let thin = |(top, bottom): (f32, f32)| if bottom - top < 2.0 { (top - 1.0, top + 1.0) } else { (top, bottom) };
    let (left, right) = (thin(left), thin(right));
    let (mut tops, mut bottoms) = (vec![], vec![]);
    let mut mesh = egui::Mesh::default();
    for step in 0..=STEPS {
        let t = step as f32 / STEPS as f32;
        let (x, s) = (x_left + (x_right - x_left) * t, t * t * (3.0 - 2.0 * t));
        let (top, bottom) = (left.0 + (right.0 - left.0) * s, left.1 + (right.1 - left.1) * s);
        tops.push(Pos2::new(x, top));
        bottoms.push(Pos2::new(x, bottom));
        mesh.colored_vertex(Pos2::new(x, top), fill);
        mesh.colored_vertex(Pos2::new(x, bottom), fill);
        if step > 0 {
            let at = (step as u32 - 1) * 2;
            mesh.add_triangle(at, at + 1, at + 2);
            mesh.add_triangle(at + 1, at + 3, at + 2);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.add(egui::Shape::line(tops, edge));
    painter.add(egui::Shape::line(bottoms, edge));
}
