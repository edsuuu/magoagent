//! The buttons at the end of a file tree's bottom bar, beside its search field: the editor pane, the Banco de Dados panel and
//! the Alterações panel. Each is lit while its part is shown.

use crate::app::filetree::*;
use crate::app::filetree_menu::{G, arc, paint_g};
use crate::app::filetree_ui::{Act, Palette, cr, outline};
use crate::prelude::*;

/// lucide's git-compare: two nodes and the paths between them.
pub(super) fn glyph_changes(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    outline(p, c, size, &arc(18.0, 18.0, 3.0, 0.0, 360.0), true, stroke);
    outline(p, c, size, &arc(6.0, 6.0, 3.0, 0.0, 360.0), true, stroke);
    outline(p, c, size, &[(13.0, 6.0), (16.0, 6.0), (18.0, 8.0), (18.0, 15.0)], false, stroke);
    outline(p, c, size, &[(11.0, 18.0), (8.0, 18.0), (6.0, 16.0), (6.0, 9.0)], false, stroke);
}

/// The buttons in `bar`, from its right end. Back: the x where the field may end.
#[allow(clippy::too_many_arguments)]
pub(super) fn buttons(ui: &mut egui::Ui, p: &Painter, bar: Rect, z: f32, spec: &FileTreeSpec, c: &Palette, dark: bool, acts: &mut Vec<Act>) -> f32 {
    let accent = if dark { Color32::from_rgb(0x0a, 0x84, 0xff) } else { Color32::from_rgb(0x00, 0x7a, 0xff) };
    let stroke = |color: Color32| Stroke::new((1.0 * z).max(0.75), color);
    let mut edge = bar.right() - 6.5 * z;
    // (key, tip when off, tip when on, lit, what a press asks, how it is drawn)
    type Draw = fn(&Painter, Pos2, f32, Stroke);
    let editor: Draw = |p, c, size, stroke| paint_g(p, G::PanelRight, c, size, stroke.color);
    let buttons: [(&str, &str, &str, bool, Act, Draw); 3] = [
        ("editor", "Mostrar Editor", "Ocultar Editor", spec.editor, Act::ToggleEditor, editor),
        ("db", "Mostrar Banco de Dados", "Ocultar Banco de Dados", spec.db, Act::ToggleDb, crate::app::filetree_db::glyph_db),
        ("changes", "Mostrar Alterações", "Ocultar Alterações", spec.changes, Act::ToggleChanges, glyph_changes),
    ];
    for (key, off, on, lit, act, draw) in buttons {
        let rect = Rect::from_center_size(Pos2::new(edge - 11.375 * z, bar.center().y), Vec2::new(22.75 * z, 19.5 * z));
        let r = ui.interact(rect, ui.scope_id().with(key), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        if r.hovered() {
            p.rect_filled(rect, cr(6.0 * z), c.hover);
        }
        draw(p, rect.center(), 11.375 * z, stroke(if lit { accent } else { c.muted }));
        if r.tip(if lit { on } else { off }).clicked() {
            acts.push(act);
            acts.push(Act::Focus);
        }
        edge = rect.left() - 4.875 * z;
    }
    edge
}
