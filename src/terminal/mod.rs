//! Terminals on the canvas: a shell in a pseudo-terminal (alacritty_terminal does the pty on
//! every OS and the escape-sequence emulation), keys to bytes, and painting the grid.

use crate::prelude::*;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term, TermMode, cell::Flags, color::Colors, point_to_viewport, test::TermSize};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

mod select;
mod theme;
pub(crate) use select::{Found, Pick, cell_at};
pub(crate) use theme::{FONT_SIZES, GridView, Look, TermLook, TermPrefs, cell, font_size, grid_size_at, theme_choices};
use theme::{MATCH, MATCH_CURRENT};
mod glyphs;
mod handle;
mod keys;
mod session;
pub(crate) use glyphs::*;
pub(crate) use handle::*;
pub(crate) use keys::*;
pub(crate) use session::*;

pub(crate) const TERMINAL_FONT: f32 = 14.0;
/// One grid cell in world units: Hack advances 1233/2048 em.
pub(crate) const CELL: Vec2 = Vec2::new(TERMINAL_FONT * 1233.0 / 2048.0, 17.0);
/// Title strip on top (the handle that drags the terminal), folder strip at the bottom and
/// padding around the grid.
pub(crate) const TERMINAL_HEADER: f32 = 28.0;
pub(crate) const TERMINAL_FOOTER: f32 = 22.0;
pub(crate) const TERMINAL_PAD: f32 = 8.0;
pub(crate) const TERMINAL_BG: Color32 = Color32::from_rgb(0x18, 0x18, 0x18);
pub(crate) const TERMINAL_FG: Color32 = Color32::from_rgb(0xd8, 0xd8, 0xd8);
/// Alacritty's default palette: normal colors, then bright.
pub(super) const ANSI: [[u8; 3]; 16] = [
    [0x18, 0x18, 0x18],
    [0xac, 0x42, 0x42],
    [0x90, 0xa9, 0x59],
    [0xf4, 0xbf, 0x75],
    [0x6a, 0x9f, 0xb5],
    [0xaa, 0x75, 0x9f],
    [0x75, 0xb5, 0xaa],
    [0xd8, 0xd8, 0xd8],
    [0x6b, 0x6b, 0x6b],
    [0xc5, 0x55, 0x55],
    [0xaa, 0xc4, 0x74],
    [0xfe, 0xca, 0x88],
    [0x82, 0xb8, 0xc8],
    [0xc2, 0x8c, 0xb8],
    [0x93, 0xd3, 0xc3],
    [0xf8, 0xf8, 0xf8],
];

/// Columns and rows that fit a terminal of this size (world units) at the default font size.
#[cfg(test)]
pub(crate) fn grid_size(size: Vec2) -> (usize, usize) {
    let cols = ((size.x - 2.0 * TERMINAL_PAD) / CELL.x).floor().max(2.0) as usize;
    let rows = ((size.y - TERMINAL_HEADER - TERMINAL_FOOTER - TERMINAL_PAD) / CELL.y).floor().max(1.0) as usize;
    (cols, rows)
}

/// Size of a terminal with this many columns and rows.
pub(crate) fn terminal_size(cols: usize, rows: usize) -> Vec2 {
    let height = rows as f32 * CELL.y + TERMINAL_HEADER + TERMINAL_FOOTER + TERMINAL_PAD;
    Vec2::new(cols as f32 * CELL.x + 2.0 * TERMINAL_PAD, height).ceil()
}

/// Milliseconds since the app started, for stamps shared across threads.
pub(crate) fn uptime_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hidden_terminal_repaints_the_app_only_when_it_ends() {
        let ctx = Context::default();
        let settle = |ctx: &Context| (0..3).for_each(|_| ctx.run_ui(Default::default(), |_| {}).textures_delta.clear());
        let listener = Listener {
            ctx: Some(ctx.clone()),
            pty: Default::default(),
            title: Default::default(),
            exited: Default::default(),
            last_output: Default::default(),
            hidden: Arc::new(AtomicBool::new(true)),
            look: Default::default(),
        };
        settle(&ctx);
        listener.send_event(Event::Wakeup);
        assert!(!ctx.has_requested_repaint(), "its output waits for the idle repaint");
        listener.send_event(Event::Exit);
        assert!(ctx.has_requested_repaint(), "its end shows now");
        settle(&ctx);
        listener.hidden.store(false, Ordering::Relaxed);
        listener.send_event(Event::Wakeup);
        assert!(ctx.has_requested_repaint(), "on the screen, its output repaints");
    }
}
