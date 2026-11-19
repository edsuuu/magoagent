//! The small widgets the file tree, the merge window and the database panel share.

use super::*;

/// `resp` named for the accessibility tree (what `agent-desktop` and the screen readers read) as a button.
pub(super) fn named(resp: egui::Response, label: impl ToString) -> egui::Response {
    let label = label.to_string();

    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
    resp
}

/// Full-width selectable row with PhpStorm hover/selection colours.
pub(crate) fn row(ui: &mut Ui, selected: bool, h: f32) -> (egui::Response, Rect) {
    row_colored(ui, if selected { t::SELECTION } else { Color32::TRANSPARENT }, h)
}

pub(super) fn row_colored(ui: &mut Ui, fill: Color32, h: f32) -> (egui::Response, Rect) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());

    let bg = if fill != Color32::TRANSPARENT {
        fill
    } else if resp.hovered() {
        t::ROW_HOVER
    } else {
        Color32::TRANSPARENT
    };

    ui.painter().rect_filled(rect.shrink2(vec2(6.0, 0.0)), 5, bg);
    (resp, rect)
}
