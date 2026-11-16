//! The mini agents of the compact island, where Coucou puts its own (style.css `#mini-grid`, island.ts
//! `applyGeometry`): a 2×2 grid centred 40 from the right as the wizard is 40 from the left. The reference's cells are
//! 13 px, 3 apart; here the compact island is taller (40) for bigger ones, 15 and 2 apart, with 4 px above and below.

use crate::prelude::*;

const BODY: f32 = 15.0;
const GAP: f32 = 2.0;
/// The grid's side: two cells and the gap between them.
const GRID: f32 = 2.0 * BODY + GAP;
pub(super) const MOST: usize = 4;

/// Standing up on a side of the screen (`COMPACT_TALL` tall): the wizard's centre this far from the top, the grid's from
/// the bottom.
pub(crate) const UPRIGHT_INSET: f32 = 28.0;

/// The cells of the first `count` agents (four at the most), row by row from the grid's top left, as a CSS grid fills;
/// `upright`, the grid at the bottom of the island standing up.
pub(crate) fn mini_cells(island: Rect, count: usize, upright: bool) -> Vec<Rect> {
    let center = if upright { Pos2::new(island.center().x, island.bottom() - UPRIGHT_INSET) } else { Pos2::new(island.right() - 40.0, island.center().y) };
    let corner = center - Vec2::splat(GRID / 2.0);
    (0..count.min(MOST))
        .map(|index| Rect::from_min_size(corner + Vec2::new((index % 2) as f32, (index / 2) as f32) * (BODY + GAP), Vec2::splat(BODY)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::island::{COMPACT_H, COMPACT_TALL, COMPACT_W};

    #[test]
    fn minis_fill_a_two_by_two_grid_mirroring_the_wizard() {
        let island = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(COMPACT_W, COMPACT_H));
        let cells = mini_cells(island, 4, false);
        assert_eq!(cells.len(), 4);
        assert!(cells.iter().all(|cell| cell.size() == Vec2::splat(BODY)));
        // the grid is centred 40 from the right, as the wizard is 40 from the left, and in the middle of the height
        let grid = cells.iter().fold(cells[0], |grid, cell| grid.union(*cell));
        assert_eq!(grid.size(), Vec2::splat(GRID));
        assert!(grid.top() >= 4.0 && grid.bottom() <= COMPACT_H - 4.0, "room above and below: {grid:?}");
        assert_eq!(grid.center(), Pos2::new(COMPACT_W - 40.0, COMPACT_H / 2.0));
        // row by row: the second beside the first, the third under the first
        assert_eq!(cells[1].left() - cells[0].right(), GAP);
        assert_eq!(cells[2].top() - cells[0].bottom(), GAP);
        assert_eq!(cells[2].left(), cells[0].left());
        assert_eq!(mini_cells(island, 0, false).len(), 0);
        assert_eq!(mini_cells(island, 6, false).len(), 4, "four at the most");
        // standing up on a side: the grid at the bottom, in the middle of the width, inside the island
        let upright = Rect::from_min_size(Pos2::ZERO, Vec2::new(COMPACT_H, COMPACT_TALL));
        let grid = mini_cells(upright, 4, true).iter().fold(Rect::NOTHING, |grid, cell| grid.union(*cell));
        assert_eq!(grid.center(), Pos2::new(COMPACT_H / 2.0, COMPACT_TALL - UPRIGHT_INSET));
        assert!(upright.contains_rect(grid.expand(4.0)), "room around it: {grid:?}");
    }
}
