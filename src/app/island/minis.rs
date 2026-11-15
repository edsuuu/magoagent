//! The mini agents of the compact island, where Coucou puts its own (style.css `#mini-grid`, island.ts
//! `applyGeometry`): a grid at the right, up to four, each a mini wizard in its agent's colour.

use crate::prelude::*;

/// The grid: a box of 29 points with two columns and rows of 13 and 3 between them, its centre 40 points from the
/// island's right edge.
const BOX: f32 = 29.0;
const GAP: f32 = 3.0;
const FROM_RIGHT: f32 = 40.0;
/// The least room above and under it.
const MARGIN: f32 = 2.0;
pub(super) const MOST: usize = 4;

/// Where the mini agents of the compact island `island` go, `count` of them (four at the most): row by row from the
/// left, as in Coucou's grid, the rows filled centred on the island's height; the grid is smaller where the island is
/// too low for it, so that all of it is inside with room to spare above and under.
pub(crate) fn mini_cells(island: Rect, count: usize) -> Vec<Rect> {
    let count = count.min(MOST);
    let side = BOX.min(island.height() - 2.0 * MARGIN).max(GAP);
    let cell = (side - GAP) / 2.0;
    let rows = count.div_ceil(2);
    let tall = rows as f32 * cell + rows.saturating_sub(1) as f32 * GAP;
    let (left, top) = (island.right() - FROM_RIGHT - side / 2.0, island.center().y - tall / 2.0);
    (0..count)
        .map(|index| Rect::from_min_size(Pos2::new(left + (index % 2) as f32 * (cell + GAP), top + (index / 2) as f32 * (cell + GAP)), Vec2::splat(cell)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::island::{COMPACT_H, COMPACT_W};

    #[test]
    fn the_mini_agents_are_all_inside_the_compact_island_with_room_above_and_under() {
        for height in [COMPACT_H, 32.0, 28.0, 24.0, 40.0] {
            let island = Rect::from_min_size(Pos2::new(300.0, 0.0), Vec2::new(COMPACT_W, height));
            for count in 1..=4 {
                let cells = mini_cells(island, count);
                assert_eq!(cells.len(), count);
                for cell in &cells {
                    assert!(cell.top() >= island.top() + 2.0 && cell.bottom() <= island.bottom() - 2.0, "{count} in {height}: {cell:?}");
                    assert!(cell.left() > island.center().x && cell.right() < island.right(), "{count} in {height}: {cell:?}");
                    assert!(cell.width() > 6.0 && cell.width() <= 13.0);
                }
                // the rows filled are centred on the island's height
                let (top, bottom) = (cells[0].top(), cells[count - 1].bottom());
                assert!(((top + bottom) / 2.0 - island.center().y).abs() < 0.01, "{count} in {height}");
            }
            // the third goes to the second row, at the left, under the first
            let cells = mini_cells(island, 3);
            assert!(cells[2].top() > cells[0].bottom() && cells[2].left() == cells[0].left() && cells[1].left() > cells[0].right());
        }
        // Coucou's own measures where the island is tall enough: 13 with 3 between, the middle 40 from the right edge
        let cells = mini_cells(Rect::from_min_size(Pos2::ZERO, Vec2::new(288.0, 40.0)), 4);
        assert_eq!((cells[0].size(), cells[1].left() - cells[0].right()), (Vec2::splat(13.0), 3.0));
        assert_eq!((cells[0].left() + cells[1].right()) / 2.0, 288.0 - 40.0);
        assert_eq!(mini_cells(Rect::from_min_size(Pos2::ZERO, Vec2::new(288.0, 32.0)), 9).len(), 4, "four at the most");
    }
}
