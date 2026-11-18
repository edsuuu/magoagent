use super::*;

/// Organizar's move for each box: into cells as big as the biggest box, GRID apart, in reading
/// order (top to bottom, then left to right) from the boxes' top left corner. Layout 0 is a grid
/// of ⌈√n⌉ columns, 1 a row, 2 a column. None under two boxes.
// ponytail: the reference doesn't say which layouts it cycles through; grid, row and column are ours
pub(crate) fn tidy_deltas(boxes: &[Rect], layout: usize) -> Option<Vec<Vec2>> {
    if boxes.len() < 2 {
        return None;
    }
    let union = boxes.iter().copied().reduce(|first, second| first.union(second))?;
    let cell = boxes.iter().fold(Vec2::ZERO, |largest, bounds| largest.max(bounds.size())) + Vec2::splat(GRID);
    let columns = match layout {
        1 => boxes.len(),
        2 => 1,
        _ => (boxes.len() as f32).sqrt().ceil() as usize,
    };
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&first, &second| boxes[first].min.y.total_cmp(&boxes[second].min.y).then(boxes[first].min.x.total_cmp(&boxes[second].min.x)));
    let mut deltas = vec![Vec2::ZERO; boxes.len()];
    for (position, &index) in order.iter().enumerate() {
        let at = union.min + Vec2::new((position % columns) as f32 * cell.x, (position / columns) as f32 * cell.y);
        deltas[index] = at - boxes[index].min;
    }
    Some(deltas)
}

/// Organizar o canvas's move for each box: taken in `order`, left to right in rows from the
/// boxes' top left corner, `gap` apart, each row as tall as its tallest box and as wide as a 16:10
/// view of them all (the widest box at least), so the whole fits a screen.
pub(crate) fn arrange_deltas(boxes: &[Rect], order: &[usize], gap: f32) -> Vec<Vec2> {
    let Some(union) = boxes.iter().copied().reduce(|first, second| first.union(second)) else { return Vec::new() };
    let area: f32 = boxes.iter().map(|bounds| (bounds.width() + gap) * (bounds.height() + gap)).sum();
    let widest = boxes.iter().map(Rect::width).fold(0.0, f32::max);
    let row_width = (area * 1.6).sqrt().max(widest);
    let mut deltas = vec![Vec2::ZERO; boxes.len()];
    let (mut at, mut row_height) = (union.min, 0.0_f32);
    for &index in order {
        let size = boxes[index].size();
        if at.x > union.min.x && at.x + size.x > union.min.x + row_width {
            at = Pos2::new(union.min.x, at.y + row_height + gap);
            row_height = 0.0;
        }
        deltas[index] = at - boxes[index].min;
        at.x += size.x + gap;
        row_height = row_height.max(size.y);
    }
    deltas
}

/// Alinhar's move for each box: to the boxes' common edge or center (0 left, 1 center, 2 right,
/// 3 top, 4 middle, 5 bottom) or spread with even gaps (6 across, 7 down). None under two boxes.
pub(crate) fn align_deltas(boxes: &[Rect], how: usize) -> Option<Vec<Vec2>> {
    if boxes.len() < 2 {
        return None;
    }
    let union = boxes.iter().copied().reduce(|first, second| first.union(second))?;
    match how {
        0..=5 => Some(
            boxes
                .iter()
                .map(|bounds| match how {
                    0 => Vec2::new(union.min.x - bounds.min.x, 0.0),
                    1 => Vec2::new(union.center().x - bounds.center().x, 0.0),
                    2 => Vec2::new(union.max.x - bounds.max.x, 0.0),
                    3 => Vec2::new(0.0, union.min.y - bounds.min.y),
                    4 => Vec2::new(0.0, union.center().y - bounds.center().y),
                    _ => Vec2::new(0.0, union.max.y - bounds.max.y),
                })
                .collect(),
        ),
        6 | 7 => {
            let horizontal = how == 6;
            let start = |bounds: &Rect| if horizontal { bounds.min.x } else { bounds.min.y };
            let span = |bounds: &Rect| if horizontal { bounds.width() } else { bounds.height() };
            let mut order: Vec<usize> = (0..boxes.len()).collect();
            order.sort_by(|first, second| start(&boxes[*first]).total_cmp(&start(&boxes[*second])));
            let total: f32 = boxes.iter().map(span).sum();
            let gap = (span(&union) - total) / (boxes.len() - 1) as f32;
            let mut deltas = vec![Vec2::ZERO; boxes.len()];
            let mut at = start(&union);
            for index in order {
                let distance = at - start(&boxes[index]);
                deltas[index] = if horizontal { Vec2::new(distance, 0.0) } else { Vec2::new(0.0, distance) };
                at += span(&boxes[index]) + gap;
            }
            Some(deltas)
        }
        _ => None,
    }
}
