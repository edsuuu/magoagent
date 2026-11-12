//! The canvas: pointer and keyboard gestures, selection, editing, arrows, painting the scene, drops, the minimap.

mod arrows;
mod cover;
#[cfg_attr(target_os = "linux", allow(dead_code))] // Design runs in portals, which Linux doesn't open yet
mod dock;
mod drops;
pub(crate) mod editing;
pub(crate) mod minimap;
mod paint;
pub(crate) mod pointer;
mod selection;
pub(crate) mod shortcuts;
pub(crate) mod tool;
