use super::*;

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
    /// The canvas group (the dashed named frame, `Scene.groups`) this element is in. Apart from
    /// `group_id`, which binds a note to its text and a pile's pages together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) frame_group: Option<String>,
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
    pub(crate) image: Option<std::sync::Arc<str>>,
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
    /// Some makes a rectangle a folder's file tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) filetree: Option<FileTreeSpec>,
    /// Some makes a rectangle a chat's card; its id is the chat's (app/chat/node.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) chat: Option<ChatSpec>,
    #[serde(flatten)]
    pub(crate) style: Style,
}

pub(crate) fn rot(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

/// The smallest box a terminal can be drawn or resized to.
pub(crate) const TERMINAL_MIN: Vec2 = Vec2::new(320.0, 240.0);
/// The smallest box an IDE (its file tree) can be drawn or resized to; with the editor open, wider.
pub(crate) const IDE_MIN: Vec2 = Vec2::new(300.0, 300.0);
pub(crate) const IDE_EDITOR_MIN: Vec2 = Vec2::new(540.0, 300.0);
/// The smallest box a portal can be drawn or resized to (the Novo Portal dialog starts at 400×300).
pub(crate) const PORTAL_MIN: Vec2 = Vec2::new(320.0, 240.0);
/// The smallest box a sticky note can be drawn or resized to.
pub(crate) const NOTE_MIN: Vec2 = Vec2::new(250.0, 100.0);
/// The size a click makes a note.
pub(crate) const NOTE_SIZE: Vec2 = Vec2::new(260.0, 160.0);

impl Element {
    /// A terminal without its shell or a portal without its page (Descarregar).
    pub(crate) fn unloaded(&self) -> bool {
        self.terminal.as_ref().is_some_and(|t| t.unloaded) || self.portal.as_ref().is_some_and(|p| p.unloaded)
    }

    /// Descarregar (`on`) or Recarregar a terminal or portal: whether that changed anything.
    pub(crate) fn set_unloaded(&mut self, on: bool) -> bool {
        let flag = match (&mut self.terminal, &mut self.portal) {
            (Some(t), _) => &mut t.unloaded,
            (_, Some(p)) => &mut p.unloaded,
            _ => return false,
        };
        std::mem::replace(flag, on) != on
    }

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
        } else if let Some(ide) = &self.ide {
            // the file panel is open while the tree's width is kept
            Some(if ide.tree_width.is_some() { IDE_EDITOR_MIN } else { IDE_MIN })
        } else if self.filetree.is_some() {
            Some(FILETREE_MIN)
        } else if self.chat.is_some() {
            Some(CHAT_NODE_MIN)
        } else {
            self.created.map(|_| NOTE_MIN + Vec2::new(0.0, crate::app::notes::buttons::height(&self.note.buttons, local_bounds(self).width())))
        }
    }

    /// Terminals, portals, IDEs, file trees and chats: live windows on the canvas, never rotated.
    pub(crate) fn is_node(&self) -> bool {
        self.terminal.is_some() || self.portal.is_some() || self.ide.is_some() || self.filetree.is_some() || self.chat.is_some()
    }

    /// Double-click on its title enlarges it (terminals have their own double-click).
    pub(crate) fn is_liftable(&self) -> bool {
        self.portal.is_some() || self.ide.is_some() || self.created.is_some() || self.filetree.is_some()
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
            ElementKind::Rectangle { x, y, .. } | ElementKind::Ellipse { x, y, .. } | ElementKind::Diamond { x, y, .. } | ElementKind::Text { x, y, .. } => {
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
        let map = |px: f32, py: f32| [to.min.x + (px - from.min.x) * sx, to.min.y + (py - from.min.y) * sy];
        match &mut self.kind {
            ElementKind::Rectangle { x, y, width, height } | ElementKind::Ellipse { x, y, width, height } | ElementKind::Diamond { x, y, width, height } => {
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
        matches!(self.kind, ElementKind::Rectangle { .. } | ElementKind::Ellipse { .. } | ElementKind::Diamond { .. })
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
