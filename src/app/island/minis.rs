//! The mini agents of the compact island, where Coucou puts its own (style.css `#mini-grid`, island.ts
//! `applyGeometry`): a grid at the right, up to four, each a mini wizard in its agent's colour.

use crate::prelude::*;

const BODY: f32 = 13.0;
const GAP: f32 = 3.0;
pub(super) const MOST: usize = 4;

/// Minis sit in one centred row, following the wizard with an even margin at either end.
pub(crate) fn mini_cells(island: Rect, count: usize) -> Vec<Rect> {
    let count = count.min(MOST);
    if count == 0 {
        return Vec::new();
    }
    let width = count as f32 * BODY + count.saturating_sub(1) as f32 * GAP;
    let left = island.center().x - width / 2.0;
    (0..count).map(|index| Rect::from_min_size(Pos2::new(left + index as f32 * (BODY + GAP), island.center().y - BODY / 2.0), Vec2::splat(BODY))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::island::{COMPACT_H, COMPACT_W};

    #[test]
    fn minis_in_one_centred_row() {
        let island = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(COMPACT_W, COMPACT_H));
        let cells = mini_cells(island, 3);
        assert_eq!(cells.len(), 3);
        for cell in &cells {
            assert_eq!(cell.size(), Vec2::splat(13.0));
            assert!(cell.center().y == island.center().y);
        }
        assert_eq!(cells[1].left() - cells[0].right(), 3.0);
        assert!((cells[0].left() + cells[2].right()) / 2.0 == island.center().x, "centred");
        assert_eq!(mini_cells(island, 0).len(), 0);
        assert_eq!(mini_cells(island, 5).len(), 4, "four at the most");
    }
}
