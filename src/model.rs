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

pub(crate) fn now_ms() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

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

/// A terminal's shell: the name agents call it by, its starting folder (empty: home) and the
/// command typed once the shell starts (empty: just the shell).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct TerminalSpec {
    pub(crate) name: String,
    pub(crate) cwd: String,
    pub(crate) command: String,
    /// The mark in its title strip (`TERMINAL_ICONS`; empty: by the command) and its tint.
    pub(crate) icon: String,
    pub(crate) color: Option<[u8; 3]>,
    /// "Monitorar atividade" off: no "terminou" when its work ends.
    pub(crate) quiet: bool,
    /// Maestro: may recruit, open portals and connect. Terminals the person makes are Maestros, and
    /// so are scenes saved before this field; agents' children are not.
    pub(crate) maestro: bool,
}

impl Default for TerminalSpec {
    fn default() -> Self {
        Self { name: String::new(), cwd: String::new(), command: String::new(), icon: String::new(), color: None, quiet: false, maestro: true }
    }
}

impl TerminalSpec {
    /// Its name, or "Terminal" while it has none.
    pub(crate) fn label(&self) -> &str {
        if self.name.is_empty() { "Terminal" } else { &self.name }
    }
}

/// A web page on the canvas. Its data keeps every tab's address (the bar shows one page for now).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct PortalSpec {
    pub(crate) name: String,
    /// The open tab's address (what agents navigate and read).
    pub(crate) url: String,
    /// Every tab's address; empty means just `url`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) tabs: Vec<String>,
    pub(crate) active: usize,
    /// Hides the header and the address bar, leaving just the page.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) chrome_hidden: bool,
}

impl PortalSpec {
    pub(crate) fn tab_urls(&self) -> Vec<String> {
        if self.tabs.is_empty() { vec![self.url.clone()] } else { self.tabs.clone() }
    }

    pub(crate) fn active_tab(&self) -> usize {
        self.active.min(self.tab_urls().len() - 1)
    }

    fn set_tabs(&mut self, tabs: Vec<String>, active: usize) {
        self.active = active.min(tabs.len().saturating_sub(1));
        self.url = tabs.get(self.active).cloned().unwrap_or_default();
        self.tabs = tabs;
    }

    pub(crate) fn set_tab_url(&mut self, tab: usize, url: String) {
        let mut tabs = self.tab_urls();
        if let Some(slot) = tabs.get_mut(tab) {
            *slot = url;
        }
        self.set_tabs(tabs, self.active_tab());
    }
}


/// A sticky note's settings (the card's own; its text is the label element).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct NoteSpec {
    pub(crate) font_size: f32,
    /// Shows the markdown source instead of the formatted text.
    pub(crate) raw: bool,
    /// The text can't be edited (Mais → Bloquear conteúdo).
    pub(crate) content_locked: bool,
}

/// The note text sizes (Maestri's MIN/MAX_FONT_SIZE and FONT_STEP for notes).
pub(crate) const NOTE_FONT_MIN: f32 = 9.0;
pub(crate) const NOTE_FONT_MAX: f32 = 24.0;
pub(crate) const NOTE_FONT_STEP: f32 = 2.0;

impl Default for NoteSpec {
    fn default() -> Self {
        Self { font_size: crate::render::markdown::NOTE_FONT, raw: false, content_locked: false }
    }
}

impl NoteSpec {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// An IDE node's default size on the canvas (it is used enlarged, at 100%).
pub(crate) const IDE_SIZE: Vec2 = Vec2::new(320.0, 380.0);

/// A project folder on the canvas: the IDE node (the editor itself lives only while the app runs).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct IdeSpec {
    pub(crate) name: String,
    pub(crate) root: String,
    /// The tree column's width while the file panel is open (the node grew by the panel): the
    /// width it goes back to on close, as the reference app's treeColumnWidth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tree_width: Option<f32>,
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
    /// Some makes a rectangle a terminal. The shell itself lives only while the app runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) terminal: Option<TerminalSpec>,
    /// A line hanging like a rope between the two elements its ends are bound to: the
    /// connection agents see (drawn sagging under its own weight).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) rope: bool,
    /// Some makes a rectangle a web page (portal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) portal: Option<PortalSpec>,
    /// A sticky note's own settings: text size, raw markdown view, content lock.
    #[serde(default, skip_serializing_if = "NoteSpec::is_default")]
    pub(crate) note: NoteSpec,
    /// Some makes a rectangle an IDE on a project folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ide: Option<IdeSpec>,
    #[serde(flatten)]
    pub(crate) style: Style,
}

pub(crate) fn rot(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

/// The smallest box a terminal can be drawn or resized to.
pub(crate) const TERMINAL_MIN: Vec2 = Vec2::new(200.0, 120.0);
/// The smallest box an IDE (its file tree) can be drawn or resized to.
pub(crate) const IDE_MIN: Vec2 = Vec2::new(280.0, 200.0);
/// The smallest box a portal can be drawn or resized to: less leaves no room for a page.
pub(crate) const PORTAL_MIN: Vec2 = Vec2::new(400.0, 300.0);
/// The smallest side a sticky note can be drawn or resized to.
pub(crate) const NOTE_MIN: f32 = 75.0;

impl Element {
    /// Windows and notes stay upright: a turned one could no longer be resized by its edges.
    pub(crate) fn rotatable(&self) -> bool {
        // a note turned by an older build can still be turned back upright
        !matches!(self.kind, ElementKind::Line { .. } | ElementKind::FreeDraw { .. }) && !self.is_node() && (self.created.is_none() || self.angle != 0.0)
    }

    /// How small a window or note may get; drawings have no floor.
    pub(crate) fn min_size(&self) -> Option<Vec2> {
        if self.terminal.is_some() {
            Some(TERMINAL_MIN)
        } else if self.portal.is_some() {
            Some(PORTAL_MIN)
        } else if self.ide.is_some() {
            Some(IDE_MIN)
        } else {
            self.created.map(|_| Vec2::splat(NOTE_MIN))
        }
    }

    /// Terminals, portals and IDEs: live windows on the canvas, grabbed anywhere and never rotated.
    pub(crate) fn is_node(&self) -> bool {
        self.terminal.is_some() || self.portal.is_some() || self.ide.is_some()
    }

    /// Double-click on its title enlarges it (terminals have their own double-click).
    pub(crate) fn is_liftable(&self) -> bool {
        self.portal.is_some() || self.ide.is_some() || self.created.is_some()
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
        } else {
            // windows and notes stay upright, but go round with the rest
            let center = self.center();
            self.move_by(pivot + rot(center - pivot, delta) - center);
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
                near(&poly, true) || ((self.style.fill.is_some() || self.image.is_some() || self.is_node()) && polygon_contains(&poly, p))
            }
        }
    }
}

pub(crate) fn default_background() -> [u8; 4] {
    WHITE
}

/// The `.mago` format this build reads and writes. Bump it when an old build would lose data
/// rewriting a new file: old builds then only read such files.
pub(crate) const SCENE_VERSION: u32 = 1;

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
    /// The canvas terminal that AI runs in, when it does: what it draws gets tied to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent_terminal: Option<String>,
}

impl Default for Scene {
    fn default() -> Self {
        Self { version: SCENE_VERSION, elements: Vec::new(), background: WHITE, agent: None, agent_terminal: None }
    }
}

/// The workspaces (`workspaces.json`, beside the main drawing). `main` is that drawing; the
/// others are `space-<id>.mago` next to it. With no file there is only `main`, and nothing is
/// written until a second space exists.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct Spaces {
    pub(crate) version: u32,
    pub(crate) active: String,
    pub(crate) spaces: Vec<Space>,
}

impl Default for Spaces {
    fn default() -> Self {
        Self { version: 1, active: MAIN_SPACE.into(), spaces: vec![Space { id: MAIN_SPACE.into(), name: "Principal".into(), ..Default::default() }] }
    }
}

pub(crate) const MAIN_SPACE: &str = "main";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct Space {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) color: Option<[u8; 3]>,
    /// Where its new terminals and IDEs start ("" = no folder of its own).
    pub(crate) root: String,
    /// The camera when it was left: pan x, pan y and zoom (here, not in the drawing, so a pan
    /// never rewrites the drawing).
    pub(crate) view: Option<[f32; 3]>,
    /// A floor: the workspace it belongs to, and its own checkout when it has one.
    pub(crate) parent: Option<String>,
    pub(crate) worktree: Option<Worktree>,
}

/// A floor's git worktree: the repository it came from, its folder, its branch and where that
/// branch started.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct Worktree {
    pub(crate) repo: String,
    pub(crate) path: String,
    pub(crate) branch: String,
    pub(crate) base: String,
}

/// A quick start of the new terminal dialog (Ajustes → Terminal → Início rápido).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Preset {
    pub(crate) name: String,
    pub(crate) command: String,
    /// Shown in the dialog (the first 10 that are).
    pub(crate) on: bool,
}

pub(crate) fn default_presets() -> Vec<Preset> {
    [("Claude Code", "claude"), ("Codex", "codex"), ("Shell", "")].map(|(name, command)| Preset { name: name.into(), command: command.into(), on: true }).to_vec()
}

/// Main menu → Preferências (DefaultItems.tsx Preferences), remembered between launches.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
    pub(crate) presets: Vec<Preset>,
    /// Ask before a file tree sends something to the trash (off with "Não perguntar novamente").
    pub(crate) confirm_trash: bool,
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
            theme: Theme::Dark,
            presets: default_presets(),
            confirm_trash: true,
        }
    }
}

pub(crate) const GRID: f32 = 20.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Theme {
    Light,
    #[default]
    Dark,
    System,
}
