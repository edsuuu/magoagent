use crate::app::ui::palette::*;
use crate::prelude::*;
mod agents;
mod drawing;
mod editing;
mod files;
mod interface;
mod scripted;
mod shells;
mod transcripts;

fn rect_element(x: f32, y: f32, width: f32, height: f32, fill: Option<[u8; 4]>) -> Element {
    let mut e = DrawingApp::default().new_element(ElementKind::Rectangle { x, y, width, height });
    e.style.fill = fill;
    e
}

/// Through the element JSON the clipboard and the MCP server use.
pub(crate) fn round_trip(elements: &[Element]) -> Vec<Element> {
    from_interop_elements(&json!(interop_elements(elements)))
}
