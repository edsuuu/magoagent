//! The matches of "Localizar nos Arquivos", one line each: the line's text with the match lit, and at the right, dimmed, the file
//! and the line. The popup's list and the results tab ("Abrir na janela de busca") draw them the same way.

use super::{Ask, count_note};
use crate::app::ui::UiColors;
use crate::editor::find::{Hit, match_text};
use crate::prelude::*;
use std::path::Path;

/// A row of the list.
pub(super) const ROW: f32 = 24.0;

/// What a press on the rows asked: the row selected, the one to open (a double-click).
#[derive(Default)]
pub(super) struct Picked {
    pub(super) select: Option<usize>,
    pub(super) open: Option<usize>,
}

/// The rows of `hits` in `area`, scrolled; `selected` is blue. `follow` keeps the selected row in view (the arrows moved it).
pub(super) fn rows(ui: &mut egui::Ui, area: Rect, id: egui::Id, hits: &[Hit], selected: usize, follow: bool, c: &UiColors) -> Picked {
    let mut picked = Picked::default();
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        ui.set_clip_rect(area.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show_rows(ui, ROW, hits.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for index in range {
                let hit = &hits[index];
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW), Sense::click());
                let fill = if index == selected {
                    c.selected
                } else if response.hovered() {
                    c.surface_low
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter().rect_filled(rect.shrink2(Vec2::new(6.0, 1.0)), 8, fill);
                let label = format!("{} {}", hit.path.file_name().unwrap_or_default().to_string_lossy(), hit.line + 1);
                let at = ui.painter().text(rect.right_center() - Vec2::new(16.0, 0.0), egui::Align2::RIGHT_CENTER, label, FontId::proportional(12.5), c.muted);
                match_text(ui, Pos2::new(rect.left() + 16.0, rect.center().y), hit, at.left() - 12.0 - rect.left() - 16.0);
                if index == selected && follow {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    picked.select = Some(index);
                }
                if response.double_clicked() {
                    picked.open = Some(index);
                }
            }
        });
    });
    picked
}

/// The results tab: what a search found, kept to look at while the code is edited.
pub(crate) struct Results {
    pub(crate) ask: Ask,
    pub(crate) hits: Vec<Hit>,
    pub(crate) selected: usize,
}

impl Results {
    /// The tab's name.
    pub(crate) fn title(&self) -> String {
        format!("Localizar: {}", self.ask.query.replace('\n', " "))
    }
}

/// The results tab in `body`; back, the match opened (its file, line and column).
pub(crate) fn results_ui(ui: &mut egui::Ui, body: Rect, results: &mut Results, root: &Path) -> Option<(PathBuf, usize, usize)> {
    let c = &UiColors::new(ui.visuals().dark_mode);
    ui.painter().with_clip_rect(body).rect_filled(body, 0, c.island);
    let head = Rect::from_min_size(body.min, Vec2::new(body.width(), 30.0));
    let painter = ui.painter().with_clip_rect(head);
    let scope = results.ask.dir.as_ref().map_or("No Projeto".to_string(), |dir| dir.strip_prefix(root).unwrap_or(dir).to_string_lossy().replace('\\', "/"));
    painter.text(
        Pos2::new(head.left() + 14.0, head.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{} · {scope}", count_note(&results.hits)),
        FontId::proportional(13.0),
        c.muted,
    );
    painter.hline(head.x_range(), head.bottom(), Stroke::new(1.0, c.surface_low));
    let list = Rect::from_min_max(Pos2::new(body.left(), head.bottom() + 1.0), body.max);
    let id = ui.scope_id().with(("ft_results", results.title()));
    let picked = rows(ui, list, id, &results.hits, results.selected, false, c);
    if let Some(index) = picked.select {
        results.selected = index;
    }
    picked.open.and_then(|index| results.hits.get(index)).map(|hit| (hit.path.clone(), hit.line, hit.col))
}
