use super::*;

// Values from packages/common/src/colors.ts
pub(crate) const STROKE_PICKS: [[u8; 4]; 5] = [BLACK, [0xe0, 0x31, 0x31, 255], [0x2f, 0x9e, 0x44, 255], [0x19, 0x71, 0xc2, 255], [0xf0, 0x8c, 0x00, 255]];
pub(crate) const BACKGROUND_PICKS: [[u8; 4]; 5] =
    [TRANSPARENT, [0xff, 0xc9, 0xc9, 255], [0xb2, 0xf2, 0xbb, 255], [0xa5, 0xd8, 0xff, 255], [0xff, 0xec, 0x99, 255]];

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
