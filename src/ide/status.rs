//! The status bar (Islands: on the window background, under the islands): breadcrumbs of the open
//! file on the left; on the right the last message (or the language servers'), cursor, line
//! separator and encoding. StatusBar.Widget.foreground is text-muted, hover text-default.

use super::*;

pub(super) const HEIGHT: f32 = 28.0;
const SIZE: f32 = t::UI_SIZE - 1.0;

/// The cursor as "line:column" (1-based, what the editors show).
pub(super) fn position(cursor: (usize, usize)) -> String {
    format!("{}:{}", cursor.0 + 1, cursor.1 + 1)
}

/// The names from the project folder down to the file: `["proj", "src", "main.rs"]`.
pub(super) fn crumbs(root: &Path, path: &Path) -> Vec<String> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    std::iter::once(file_name(root)).chain(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned())).collect()
}

fn widget(ui: &mut Ui, text: &str) {
    let galley = ui.painter().layout_no_wrap(text.into(), t::ui(SIZE), t::MUTED);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 22.0), Sense::hover());
    let color = if resp.hovered() {
        ui.painter().rect_filled(rect, 6, t::HOVER);
        t::TEXT
    } else {
        t::MUTED
    };
    ui.painter().galley_with_override_text_color(pos2(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0), galley, color);
}

impl Ide {
    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("status_bar")
            .exact_size(HEIGHT)
            .show_separator_line(false)
            .frame(Frame::NONE.fill(t::FRAME).inner_margin(Margin::symmetric(8, 0)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let tab = self.tabs.get(self.active).filter(|t| t.is_text());
                    if let (Some(tab), Some(root)) = (tab, &self.root) {
                        let names = crumbs(root, &tab.path);
                        for (i, name) in names.iter().enumerate() {
                            if i > 0 {
                                let (r, _) = ui.allocate_exact_size(vec2(12.0, 22.0), Sense::hover());
                                icons::paint(ui.painter(), icons::CHEVRON_RIGHT, Rect::from_center_size(r.center(), vec2(12.0, 12.0)));
                            }
                            widget(ui, name);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if let Some(tab) = tab {
                            widget(ui, "UTF-8");
                            widget(ui, if tab.crlf { "CRLF" } else { "LF" });
                            widget(ui, &position(tab.cursor));
                        }
                        let msg = if self.status.is_empty() { self.insp.as_ref().map(|i| i.status.clone()).unwrap_or_default() } else { self.status.clone() };
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(egui::RichText::new(msg).size(SIZE).color(t::SECONDARY)).truncate());
                    });
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breadcrumbs_run_from_the_project_down_to_the_file() {
        assert_eq!(crumbs(Path::new("/w/proj"), Path::new("/w/proj/src/main.rs")), ["proj", "src", "main.rs"]);
        assert_eq!(crumbs(Path::new("/w/proj"), Path::new("/w/proj/README.md")), ["proj", "README.md"]);
        assert_eq!(position((0, 0)), "1:1");
    }
}
