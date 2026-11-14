//! The small parts of the "Localizar nos Arquivos" popup, in the app's own look (`UiColors`, the proportional face, the
//! settings' fields and switches, `PRIMARY` buttons with round corners): an icon button, a word toggle, a text field, the
//! action button and the spinner. The switches are `dialog_check`, the scope's tabs `segmented`.

use crate::app::filetree::menu::arc;
use crate::app::filetree::ui::outline;
use crate::app::ui::UiColors;
use crate::prelude::*;

/// A text field as the app's (the tasks' panel): its own margin, the low surface, round corners, a blue edge when it has the
/// keys. `multiline` lets a Shift+Enter break the line.
#[allow(clippy::too_many_arguments)]
pub(super) fn text_field(
    ui: &mut egui::Ui,
    text: &mut String,
    id: egui::Id,
    hint: &str,
    size: f32,
    width: f32,
    multiline: bool,
    c: &UiColors,
) -> egui::Response {
    ui.scope(|ui| {
        let visuals = ui.visuals_mut();
        for widget in [&mut visuals.widgets.inactive, &mut visuals.widgets.hovered, &mut visuals.widgets.active, &mut visuals.widgets.open] {
            widget.corner_radius = egui::CornerRadius::same(8);
        }
        visuals.selection.stroke = Stroke::new(1.0, PRIMARY);
        let field = if multiline { egui::TextEdit::multiline(text).desired_rows(1) } else { egui::TextEdit::singleline(text) };
        ui.add(
            field
                .id(id)
                .hint_text(egui::RichText::new(hint).color(c.muted))
                .desired_width(width)
                .margin(egui::Margin::symmetric(10, 7))
                .background_color(c.surface_low)
                .text_color(c.text)
                .font(FontId::proportional(size)),
        )
    })
    .inner
}

/// An icon button: `paint` draws the icon inside its box, in the color it is given (lit when `on`). True when pressed.
pub(super) fn icon_button(ui: &mut egui::Ui, tip: &str, on: bool, c: &UiColors, paint: impl FnOnce(&Painter, Rect, Color32)) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::click());
    if on {
        ui.painter().rect_filled(rect, 8, PRIMARY.gamma_multiply(0.28));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 8, c.surface_low);
    }
    paint(
        ui.painter(),
        rect,
        if on {
            PRIMARY
        } else if response.hovered() {
            c.text
        } else {
            c.muted
        },
    );
    response.on_hover_cursor(CursorIcon::PointingHand).tip(tip).clicked()
}

/// A word toggle ("Cc", "W", ".*"): lit when on.
pub(super) fn word_toggle(ui: &mut egui::Ui, label: &str, tip: &str, on: &mut bool, c: &UiColors) {
    if icon_button(ui, tip, *on, c, |painter, rect, color| {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, FontId::monospace(13.0), color);
    }) {
        *on = !*on;
    }
}

/// The pin: the popup stays when the focus leaves it.
pub(super) fn paint_pin(painter: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.3, color);
    let body = [
        (9.0, 7.0),
        (9.0, 10.8),
        (7.9, 12.5),
        (5.0, 15.2),
        (5.0, 16.0),
        (6.0, 17.0),
        (18.0, 17.0),
        (19.0, 16.0),
        (19.0, 15.2),
        (16.1, 12.5),
        (15.0, 10.8),
        (15.0, 7.0),
        (16.0, 6.0),
        (17.0, 5.0),
        (16.0, 2.0),
        (8.0, 2.0),
        (7.0, 5.0),
        (8.0, 6.0),
    ];
    outline(painter, rect.center(), 16.0, &body, true, stroke);
    outline(painter, rect.center(), 16.0, &[(12.0, 17.0), (12.0, 22.0)], false, stroke);
}

/// "Várias linhas": a line that wraps back under itself.
pub(super) fn paint_wrap(painter: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.3, color);
    let centre = rect.center();
    outline(painter, centre, 16.0, &[(3.0, 6.0), (21.0, 6.0)], false, stroke);
    let mut turn = vec![(3.0, 12.0), (16.0, 12.0)];
    turn.extend(arc(16.0, 15.0, 3.0, -90.0, 90.0));
    turn.push((13.0, 18.0));
    outline(painter, centre, 16.0, &turn, false, stroke);
    outline(painter, centre, 16.0, &[(15.5, 15.5), (13.0, 18.0), (15.5, 20.5)], false, stroke);
    outline(painter, centre, 16.0, &[(3.0, 18.0), (8.0, 18.0)], false, stroke);
}

/// The lens at the left of the search field.
pub(super) fn paint_lens(painter: &Painter, centre: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let ring = centre - Vec2::splat(1.5);
    painter.circle_stroke(ring, 5.0, stroke);
    painter.line_segment([ring + Vec2::splat(3.7), ring + Vec2::splat(8.0)], stroke);
}

/// The action button: `PRIMARY`, round corners, as the app's.
pub(super) fn action_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let button = egui::Button::new(egui::RichText::new(label).size(13.0).color(Color32::WHITE)).fill(PRIMARY).corner_radius(8).min_size(Vec2::new(0.0, 28.0));
    ui.add(button).on_hover_cursor(CursorIcon::PointingHand)
}

/// A turning arc: something is being searched.
pub(super) fn spinner(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
    let time = ui.input(|i| i.time) as f32;
    let start = time * 6.0;
    let points: Vec<Pos2> = (0..=14).map(|i| rect.center() + Vec2::angled(start + i as f32 * 0.28) * 6.0).collect();
    ui.painter().add(Shape::line(points, Stroke::new(1.8, PRIMARY)));
    ui.ctx().request_repaint();
}
