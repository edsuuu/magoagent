//! Labels centered on their owner, arrow-label masks and text halos.

use crate::prelude::*;

/// Text that sits on a line or arrow gets a background-colored outline, so it keeps its
/// contrast (the MCP diagrams put labels on arrows all the time).
pub(crate) fn text_needs_halo(elements: &[Element], text: &Element) -> bool {
    if !matches!(text.kind, ElementKind::Text { .. }) {
        return false;
    }
    if is_arrow_label(elements, text) {
        return true;
    }
    let area = local_bounds(text).expand(2.0);
    let edges = [
        (area.left_top(), area.right_top()),
        (area.right_top(), area.right_bottom()),
        (area.right_bottom(), area.left_bottom()),
        (area.left_bottom(), area.left_top()),
    ];
    elements.iter().filter(|e| e.id != text.id).any(|e| {
        let path = match &e.kind {
            ElementKind::Line { .. } => line_path(e),
            ElementKind::FreeDraw { points } => points.clone(),
            _ => return false,
        };
        path.windows(2).any(|w| {
            let (a, b) = (Pos2::new(w[0][0], w[0][1]), Pos2::new(w[1][0], w[1][1]));
            area.contains(a)
                || area.contains(b)
                || edges.iter().any(|(c, d)| segment_intersection(a, b, [c.x, c.y], [d.x, d.y]).is_some())
        })
    })
}

/// Shapes and arrows take a label on double-click. Frames, images, terminals, IDEs and portals
/// keep their own double-click.
pub(crate) fn takes_label(e: &Element) -> bool {
    match &e.kind {
        ElementKind::Rectangle { .. } | ElementKind::Ellipse { .. } | ElementKind::Diamond { .. } => {
            e.frame.is_none() && e.image.is_none() && e.created.is_none() && e.portal.is_none() && e.terminal.is_none() && e.ide.is_none()
        }
        // a rope is a connection, not a drawing: no words on it
        ElementKind::Line { arrow, .. } => *arrow && !e.rope,
        _ => false,
    }
}

pub(crate) fn is_label(e: &Element) -> bool {
    matches!(e.kind, ElementKind::Text { .. }) && e.id.ends_with("-label")
}

/// The text element holding a sticky note's words (`<note id>-label`), by the note's index.
pub(crate) fn note_label(elements: &[Element], note: usize) -> Option<usize> {
    let id = format!("{}-label", elements.get(note)?.id);
    elements.iter().position(|e| e.id == id)
}

/// The sticky note a label belongs to, by its index.
pub(crate) fn note_of_label(elements: &[Element], label: &Element) -> Option<usize> {
    let owner = label.id.strip_suffix("-label")?;
    elements.iter().position(|e| e.id == owner && e.created.is_some())
}

pub(crate) fn is_arrow_label(elements: &[Element], e: &Element) -> bool {
    is_label(e)
        && e.id.strip_suffix("-label").is_some_and(|owner| {
            elements.iter().any(|o| o.id == owner && matches!(o.kind, ElementKind::Line { .. }))
        })
}

/// Labels (`<owner>-label` text, made by the MCP `label` shortcut) stay centered on their
/// shape, or on the middle of their arrow, whatever moved.
pub(crate) fn recenter_labels(elements: &mut [Element]) {
    for i in 0..elements.len() {
        let Some(owner) = elements[i].id.strip_suffix("-label") else {
            continue;
        };
        let Some(owner) = elements.iter().find(|e| e.id == owner && !matches!(e.kind, ElementKind::Text { .. })) else {
            continue;
        };
        let target = match &owner.kind {
            ElementKind::Line { .. } => {
                let path = line_path(owner);
                let mid = path[path.len() / 2];
                // 2-point paths have no middle sample: average the ends
                if path.len() == 2 {
                    Pos2::new((path[0][0] + path[1][0]) / 2.0, (path[0][1] + path[1][1]) / 2.0)
                } else {
                    Pos2::new(mid[0], mid[1])
                }
            }
            _ => element_bounds(owner).center(),
        };
        let delta = target - local_bounds(&elements[i]).center();
        if delta.length_sq() > 0.01 {
            elements[i].move_by(delta);
        }
    }
}
