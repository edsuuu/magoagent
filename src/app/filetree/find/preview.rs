//! The file under the matches of "Localizar nos Arquivos": the tree's own code editor (filetree/code) over the whole file, with
//! line numbers, colors, every match lit and the selected one under the caret, scrolled to it. Editable as in the IDE: what is
//! typed is written when the popup moves to another file or closes.

use super::Ask;
use crate::app::filetree::code::{Project, text_ui};
use crate::app::filetree::editor::{Body, load};
use crate::app::ui::UiColors;
use crate::editor::find::Hit;
use crate::editor::icons;
use crate::editor::text::hash;
use crate::prelude::*;
use std::path::Path;

/// The header over the file: its name and its folder.
const HEAD: f32 = 30.0;

/// The file of the selected match, read into the editor's own text buffer.
pub(super) struct Preview {
    path: PathBuf,
    body: Body,
    /// The line and column the caret was last put at.
    at: Option<(usize, usize)>,
    /// What the lit matches were worked out for: the question and the text.
    marks_for: u64,
    /// The text field's id: it has the keys while the person types in the file.
    pub(super) id: egui::Id,
}

impl Preview {
    fn new(path: &Path, key: &str, ctx: &Context) -> Self {
        Preview { path: path.to_path_buf(), body: load(path, ctx), at: None, marks_for: 0, id: egui::Id::unique(("ft_find_preview", key, path)) }
    }

    /// Writes what was typed.
    // ponytail: the open tabs and the project's index read the file again by themselves (the pane checks the disk each second)
    pub(super) fn save(&mut self) {
        if let Body::Text(buf) = &mut self.body
            && buf.dirty
        {
            let _ = buf.save(&self.path);
        }
    }

    #[cfg(test)]
    pub(super) fn shown(&self) -> Option<(PathBuf, (usize, usize))> {
        self.at.map(|at| (self.path.clone(), at))
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.save();
    }
}

/// Draws the file of `hit` in `area` (the header and the code), reading it first when it is not the one in `slot`.
pub(super) fn show(slot: &mut Option<Preview>, ui: &mut egui::Ui, area: Rect, hit: &Hit, ask: &Ask, root: &Path, key: &str, code: &Project, c: &UiColors) {
    if slot.as_ref().is_none_or(|p| p.path != hit.path) {
        *slot = Some(Preview::new(&hit.path, key, ui.ctx()));
    }
    let Some(preview) = slot.as_mut() else { return };
    let head = Rect::from_min_size(area.min, Vec2::new(area.width(), HEAD));
    let body = Rect::from_min_max(Pos2::new(area.left(), head.bottom()), area.max);
    // the file's name, and its folder at the right of it
    let painter = ui.painter().with_clip_rect(head);
    painter.rect_filled(area, 0, c.island);
    icons::file(&painter, &hit.path, Pos2::new(head.left() + 22.0, head.center().y), 1.0);
    let name = hit.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let galley = painter.layout_no_wrap(name, FontId::proportional(13.0), c.text);
    let name_right = head.left() + 38.0 + galley.size().x;
    painter.galley(Pos2::new(head.left() + 38.0, head.center().y - galley.size().y / 2.0), galley, c.text);
    let folder = hit.path.parent().map(|d| d.strip_prefix(root).unwrap_or(d).to_string_lossy().replace('\\', "/")).unwrap_or_default();
    painter.text(Pos2::new(name_right + 10.0, head.center().y), egui::Align2::LEFT_CENTER, folder, FontId::proportional(12.0), c.muted);
    painter.hline(area.x_range(), head.bottom(), Stroke::new(1.0, c.surface_low));
    let Body::Text(buf) = &mut preview.body else {
        let note = match &preview.body {
            Body::Refused(why) | Body::External(why) => why.clone(),
            _ => "Sem pré-visualização para este arquivo.".to_string(),
        };
        ui.painter().with_clip_rect(body).text(body.center(), egui::Align2::CENTER_CENTER, note, FontId::proportional(13.0), c.muted);
        return;
    };
    // every match lit, worked out again when the question or the text changes
    let marks_for = hash(&buf.text, 0.0) ^ hash(&format!("{ask:?}"), 1.0);
    if preview.marks_for != marks_for {
        preview.marks_for = marks_for;
        buf.marks = ask.needle().map(|needle| crate::editor::find::ranges(&buf.text, &needle, ask.words)).unwrap_or_default();
    }
    // the caret on the selected match, and the file scrolled to it, without taking the keys from the search field
    if preview.at != Some((hit.line, hit.col)) {
        preview.at = Some((hit.line, hit.col));
        (buf.jump, buf.quiet) = (Some((hit.line, hit.col)), true);
    }
    let (changed, _) = text_ui(ui, body, buf, &preview.path, preview.id, 1.0, false, code, &[]);
    if changed {
        buf.dirty = true;
    }
}
