//! The drawing data: elements, styles, scene and preferences.

use crate::prelude::*;

pub(crate) const APP_TITLE: &str = "MagoAgent";
pub(crate) const PRIMARY: Color32 = Color32::from_rgb(0x69, 0x65, 0xdb);
pub(crate) const BLACK: [u8; 4] = [0x1e, 0x1e, 0x1e, 255];
pub(crate) const WHITE: [u8; 4] = [255, 255, 255, 255];
pub(crate) const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];
pub(crate) const LINE_HEIGHT: f32 = 1.25;
/// The hand-drawn font family (OFL, see assets/hand-font-license.txt).
pub(crate) const HAND_FONT: &str = "MagoHand";
pub(crate) const LINE_CURVE_STEPS: usize = 12;
// ponytail: average hand-font advance; measure glyphs if text bounds must be pixel exact
pub(crate) const CHAR_WIDTH: f32 = 0.55;

// Values from packages/common/src/colors.ts
pub(crate) const STROKE_PICKS: [[u8; 4]; 5] = [
    BLACK,
    [0xe0, 0x31, 0x31, 255],
    [0x2f, 0x9e, 0x44, 255],
    [0x19, 0x71, 0xc2, 255],
    [0xf0, 0x8c, 0x00, 255],
];
pub(crate) const BACKGROUND_PICKS: [[u8; 4]; 5] = [
    TRANSPARENT,
    [0xff, 0xc9, 0xc9, 255],
    [0xb2, 0xf2, 0xbb, 255],
    [0xa5, 0xd8, 0xff, 255],
    [0xff, 0xec, 0x99, 255],
];
pub(crate) const CANVAS_PICKS: [[u8; 4]; 5] = [
    WHITE,
    [0xf8, 0xf9, 0xfa, 255],
    [0xf5, 0xfa, 0xff, 255],
    [0xff, 0xfc, 0xe8, 255],
    [0xfd, 0xf8, 0xf6, 255],
];

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StrokeStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FillStyle {
    Hachure,
    CrossHatch,
    #[default]
    Solid,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct Style {
    pub(crate) stroke: [u8; 4],
    pub(crate) fill: Option<[u8; 4]>,
    pub(crate) stroke_width: f32,
    pub(crate) stroke_style: StrokeStyle,
    pub(crate) fill_style: FillStyle,
    pub(crate) roughness: u8,
    pub(crate) round: bool,
    pub(crate) opacity: u8,
    pub(crate) font_size: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            stroke: BLACK,
            fill: None,
            stroke_width: 2.0,
            stroke_style: StrokeStyle::Solid,
            fill_style: FillStyle::Solid,
            roughness: 1,
            round: true,
            opacity: 100,
            font_size: 20.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ElementKind {
    Rectangle { x: f32, y: f32, width: f32, height: f32 },
    Ellipse { x: f32, y: f32, width: f32, height: f32 },
    Diamond { x: f32, y: f32, width: f32, height: f32 },
    Line { points: Vec<[f32; 2]>, arrow: bool },
    FreeDraw { points: Vec<[f32; 2]> },
    /// `x`, `y` is the top-left corner of the first line.
    Text { x: f32, y: f32, value: String },
}

/// Arrowheads from the reference app's "Pontas" picker (the cardinality set is not ported).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Head {
    Arrow,
    Bar,
    #[serde(alias = "dot")]
    Circle,
    CircleOutline,
    Triangle,
    TriangleOutline,
    Diamond,
    DiamondOutline,
}

impl Head {
    /// Picker order from actionProperties.tsx getArrowheadOptions.
    pub(crate) const PICKS: [Option<Head>; 9] = [
        None,
        Some(Head::Arrow),
        Some(Head::Triangle),
        Some(Head::TriangleOutline),
        Some(Head::Circle),
        Some(Head::CircleOutline),
        Some(Head::Diamond),
        Some(Head::DiamondOutline),
        Some(Head::Bar),
    ];

    pub(crate) fn label(head: Option<Head>) -> &'static str {
        match head {
            None => "Nenhuma",
            Some(Head::Arrow) => "Flecha",
            Some(Head::Bar) => "Barra",
            Some(Head::Circle) => "Círculo",
            Some(Head::CircleOutline) => "Círculo (contorno)",
            Some(Head::Triangle) => "Triângulo",
            Some(Head::TriangleOutline) => "Triângulo (contorno)",
            Some(Head::Diamond) => "Diamante",
            Some(Head::DiamondOutline) => "Diamante (contorno)",
        }
    }

    /// Some(true) filled with the stroke color, Some(false) filled with the canvas, None open.
    pub(crate) fn fill(self) -> Option<bool> {
        match self {
            Head::Arrow | Head::Bar => None,
            Head::Circle | Head::Triangle | Head::Diamond => Some(true),
            Head::CircleOutline | Head::TriangleOutline | Head::DiamondOutline => Some(false),
        }
    }
}

/// the reference app's FixedPointBinding: `fixed` is the anchor as a ratio of the target's
/// unrotated box; "orbit" stops on the outline, `inside` goes to the anchor itself.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Binding {
    pub(crate) element_id: String,
    pub(crate) fixed: [f32; 2],
    #[serde(default)]
    pub(crate) inside: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct ArrowEnds {
    pub(crate) start_head: Option<Head>,
    pub(crate) end_head: Option<Head>,
    pub(crate) start: Option<Binding>,
    pub(crate) end: Option<Binding>,
}

impl Default for ArrowEnds {
    fn default() -> Self {
        Self { start_head: None, end_head: Some(Head::Arrow), start: None, end: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Element {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) group_id: Option<String>,
    pub(crate) kind: ElementKind,
    /// Radians around the center; lines and freehand bake rotation into their points.
    #[serde(default)]
    pub(crate) angle: f32,
    /// Arrowheads and shape bindings; only meaningful for arrows.
    #[serde(default)]
    pub(crate) ends: ArrowEnds,
    /// Locked elements can't be selected, moved or erased until unlocked.
    #[serde(default)]
    pub(crate) locked: bool,
    /// Some(name) makes a rectangle a frame: moving it carries everything inside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) frame: Option<String>,
    /// Some(data URL) makes a rectangle an image; interop files keep it in `files` instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) image: Option<String>,
    /// Some(creation time, ms since 1970; 0 when unknown) makes a rectangle a sticky note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) created: Option<i64>,
    #[serde(flatten)]
    pub(crate) style: Style,
}

pub(crate) fn rot(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

impl Element {
    pub(crate) fn rotatable(&self) -> bool {
        !matches!(self.kind, ElementKind::Line { .. } | ElementKind::FreeDraw { .. })
    }

    pub(crate) fn center(&self) -> Pos2 {
        local_bounds(self).center()
    }

    /// World point -> element's unrotated frame.
    pub(crate) fn to_local(&self, point: Pos2) -> Pos2 {
        if self.angle == 0.0 { point } else { self.center() + rot(point - self.center(), -self.angle) }
    }

    pub(crate) fn rotate_around(&mut self, pivot: Pos2, delta: f32) {
        if self.rotatable() {
            let center = self.center();
            self.move_by(pivot + rot(center - pivot, delta) - center);
            self.angle = (self.angle + delta).rem_euclid(std::f32::consts::TAU);
        } else if let ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } = &mut self.kind {
            for point in points {
                let p = pivot + rot(Pos2::new(point[0], point[1]) - pivot, delta);
                *point = [p.x, p.y];
            }
        }
    }

    pub(crate) fn move_by(&mut self, delta: Vec2) {
        match &mut self.kind {
            ElementKind::Rectangle { x, y, .. }
            | ElementKind::Ellipse { x, y, .. }
            | ElementKind::Diamond { x, y, .. }
            | ElementKind::Text { x, y, .. } => {
                *x += delta.x;
                *y += delta.y;
            }
            ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } => {
                for point in points {
                    point[0] += delta.x;
                    point[1] += delta.y;
                }
            }
        }
    }

    pub(crate) fn scale(&mut self, from: Rect, to: Rect) {
        let sx = if from.width() > 0.0 { to.width() / from.width() } else { 1.0 };
        let sy = if from.height() > 0.0 { to.height() / from.height() } else { 1.0 };
        let map = |px: f32, py: f32| {
            [
                to.min.x + (px - from.min.x) * sx,
                to.min.y + (py - from.min.y) * sy,
            ]
        };
        match &mut self.kind {
            ElementKind::Rectangle { x, y, width, height }
            | ElementKind::Ellipse { x, y, width, height }
            | ElementKind::Diamond { x, y, width, height } => {
                [*x, *y] = map(*x, *y);
                *width = (*width * sx).max(1.0);
                *height = (*height * sy).max(1.0);
            }
            ElementKind::Text { x, y, .. } => {
                [*x, *y] = map(*x, *y);
                self.style.font_size = (self.style.font_size * sy).clamp(4.0, 400.0);
            }
            ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } => {
                for point in points {
                    *point = map(point[0], point[1]);
                }
            }
        }
    }

    pub(crate) fn fillable(&self) -> bool {
        matches!(
            self.kind,
            ElementKind::Rectangle { .. } | ElementKind::Ellipse { .. } | ElementKind::Diamond { .. }
        )
    }

    /// Transparent shapes are only hit near their outline, like in the reference app.
    pub(crate) fn contains(&self, point: Pos2, tolerance: f32) -> bool {
        let point = self.to_local(point);
        let p = [point.x, point.y];
        let near = |poly: &[[f32; 2]], closed: bool| {
            let n = poly.len();
            let edges = if closed { n } else { n.saturating_sub(1) };
            (0..edges).any(|i| distance_to_segment(p, poly[i], poly[(i + 1) % n]) < tolerance)
        };
        match &self.kind {
            ElementKind::Text { .. } => local_bounds(self).expand(tolerance / 2.0).contains(point),
            ElementKind::Line { .. } => near(&line_path(self), false),
            ElementKind::FreeDraw { points } => near(points, false),
            _ => {
                let poly = shape_polygon(self).unwrap_or_default();
                if let Some(name) = &self.frame {
                    let b = local_bounds(self);
                    let tag = Rect::from_min_size(b.min - Vec2::new(0.0, 22.0), Vec2::new((name.chars().count() as f32 * 8.0).max(40.0), 22.0));
                    return near(&poly, true) || tag.contains(point);
                }
                near(&poly, true) || ((self.style.fill.is_some() || self.image.is_some()) && polygon_contains(&poly, p))
            }
        }
    }
}

pub(crate) fn default_background() -> [u8; 4] {
    WHITE
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Scene {
    pub(crate) version: u32,
    pub(crate) elements: Vec<Element>,
    #[serde(default = "default_background")]
    pub(crate) background: [u8; 4],
    /// The AI client that wrote this file through the MCP server, shown while the app
    /// replays what it drew.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent: Option<String>,
}

impl Default for Scene {
    fn default() -> Self {
        Self { version: 1, elements: Vec::new(), background: WHITE, agent: None }
    }
}

/// Main menu → Preferências (DefaultItems.tsx Preferences), remembered between launches.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct Prefs {
    pub(crate) box_overlap: bool,
    pub(crate) mouse_wheel_zooms: bool,
    pub(crate) tool_locked: bool,
    pub(crate) snap_objects: bool,
    pub(crate) grid: bool,
    pub(crate) zen: bool,
    pub(crate) view_mode: bool,
    pub(crate) stats: bool,
    pub(crate) arrow_binding: bool,
    pub(crate) midpoint_snapping: bool,
    pub(crate) show_hints: bool,
    pub(crate) theme: Theme,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            box_overlap: false,
            mouse_wheel_zooms: false,
            tool_locked: false,
            snap_objects: false,
            grid: false,
            zen: false,
            view_mode: false,
            stats: false,
            arrow_binding: true,
            midpoint_snapping: true,
            show_hints: true,
            theme: Theme::Light,
        }
    }
}

pub(crate) const GRID: f32 = 20.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Theme {
    #[default]
    Light,
    Dark,
    System,
}
