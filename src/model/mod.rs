//! The drawing data: elements, styles, scene and preferences.

use crate::prelude::*;
mod element;
mod scene;
mod specs;
mod style;
pub(crate) use element::*;
pub(crate) use scene::*;
pub(crate) use specs::*;
pub(crate) use style::*;

pub(crate) const APP_TITLE: &str = "MagoAgent";
pub(crate) const PRIMARY: Color32 = Color32::from_rgb(0x0a, 0x84, 0xff);
pub(crate) const BLACK: [u8; 4] = [0x1e, 0x1e, 0x1e, 255];
pub(crate) const WHITE: [u8; 4] = [255, 255, 255, 255];
pub(crate) const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];
pub(crate) const LINE_HEIGHT: f32 = 1.25;
/// The hand-drawn font family (OFL, see assets/hand-font-license.txt).
pub(crate) const HAND_FONT: &str = "MagoHand";
pub(crate) const LINE_CURVE_STEPS: usize = 12;
// ponytail: average hand-font advance; measure glyphs if text bounds must be pixel exact
pub(crate) const CHAR_WIDTH: f32 = 0.55;

pub(crate) fn now_ms() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ElementKind {
    Rectangle {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Ellipse {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Diamond {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Line {
        points: Vec<[f32; 2]>,
        arrow: bool,
    },
    FreeDraw {
        points: Vec<[f32; 2]>,
    },
    /// `x`, `y` is the top-left corner of the first line.
    Text {
        x: f32,
        y: f32,
        value: String,
    },
}
