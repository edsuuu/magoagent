//! The canvas: pointer and keyboard gestures, selection, editing, arrows, painting the scene, drops, the minimap.

mod arrows;
pub(crate) mod cover;
mod dock;
pub(crate) mod drops;
pub(crate) mod editing;
pub(crate) mod lifted;
pub(crate) mod minimap;
#[cfg(target_os = "macos")]
mod native;
mod paint;
pub(crate) mod pointer;
mod selection;
pub(crate) mod shortcuts;
pub(crate) mod tool;
