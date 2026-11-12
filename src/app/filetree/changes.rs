//! The "Alterações" panel of a file tree node: the files changed against the HEAD (modified, new, deleted, renamed, in conflict),
//! each with its git letter and color, over the file rows. Pressing one opens its diff in the editor pane (the Ctrl+D tab). The
//! MCP's `changes` action of `file_tree` lists the same files.

use crate::app::filetree::ui::{Palette, area, badge, cr, fit, status_color};
use crate::app::filetree::*;
use crate::ide::{Status, icons};
use crate::prelude::*;
use std::path::Path;

/// The panel's header: its title and the count of files.
const HEAD: f32 = 26.0;

/// The changed files in the order the panel and the MCP list them: by their path in the tree.
pub(crate) fn sorted(root: &Path, files: &[(PathBuf, Status)]) -> Vec<(PathBuf, Status)> {
    let mut files = files.to_vec();
    files.sort_by_key(|(path, _)| path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_lowercase());
    files
}

/// "Uma alteração" / "N alterações".
fn count(n: usize) -> String {
    if n == 1 { "1 arquivo alterado".into() } else { format!("{n} arquivos alterados") }
}

/// The word git's letter stands for.
fn word(s: Status) -> &'static str {
    match s {
        Status::Modified => "modificado",
        Status::Added => "adicionado",
        Status::Deleted => "excluído",
        Status::Renamed => "renomeado",
        Status::Untracked => "novo, não rastreado",
        Status::Conflict => "em conflito",
    }
}

/// The panel in `rect` (over the rows, inside `clip`) for the changed `files` of the tree rooted at `root`. Back: the file pressed.
pub(crate) fn changes_ui(ctx: &Context, id: &str, rect: Rect, clip: Rect, z: f32, dark: bool, root: &Path, files: &[(PathBuf, Status)]) -> Option<PathBuf> {
    let c = Palette::new(dark);
    let files = sorted(root, files);
    let mut pressed = None;
    area(ctx, ("ft_changes", id), rect, clip, |ui| {
        let p = ui.painter().with_clip_rect(rect.intersect(clip));
        p.rect_filled(rect, cr(4.0 * z), c.bg);
        let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEAD * z));
        p.text(head.left_center() + Vec2::new(8.125 * z, 0.0), egui::Align2::LEFT_CENTER, "Alterações", FontId::proportional(13.0 * z), c.fg);
        p.text(head.right_center() - Vec2::new(8.125 * z, 0.0), egui::Align2::RIGHT_CENTER, count(files.len()), FontId::proportional(11.0 * z), c.muted);
        p.hline(rect.x_range(), head.bottom(), Stroke::new(1.0, c.border));
        let body = Rect::from_min_max(Pos2::new(rect.left(), head.bottom() + 1.0), rect.max);
        if files.is_empty() {
            p.text(
                body.left_top() + Vec2::new(9.75 * z, 6.5 * z),
                egui::Align2::LEFT_TOP,
                "Nenhuma alteração contra o HEAD.",
                FontId::proportional(12.0 * z),
                c.muted,
            );
            return;
        }
        let row_h = FT_ROW * z;
        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
            ui.set_clip_rect(body.intersect(clip));
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical().id_salt(("ft_changes_scroll", id)).auto_shrink(false).show_rows(ui, row_h, files.len(), |ui, range| {
                for (path, status) in &files[range] {
                    let (row, r) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                    let p = ui.painter();
                    if r.hovered() {
                        p.rect_filled(row, cr(4.0 * z), c.hover);
                    }
                    let cy = row.center().y;
                    let color = status_color(*status, dark);
                    p.text(Pos2::new(row.left() + 14.0 * z, cy), egui::Align2::CENTER_CENTER, badge(*status), FontId::monospace(10.0 * z), color);
                    icons::file_on(p, path, Pos2::new(row.left() + 36.0 * z, cy), z, !dark);
                    let font = FontId::proportional(13.0 * z);
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let left = row.left() + 48.0 * z;
                    let text = p.text(Pos2::new(left, cy), egui::Align2::LEFT_CENTER, fit(p, &name, &font, row.right() - 6.5 * z - left), font, color);
                    let dir = path.strip_prefix(root).ok().and_then(Path::parent).map(|d| d.to_string_lossy().replace('\\', "/")).unwrap_or_default();
                    if !dir.is_empty() {
                        let small = FontId::proportional(11.0 * z);
                        let room = row.right() - 6.5 * z - text.right() - 6.0 * z;
                        p.text(Pos2::new(text.right() + 6.0 * z, cy), egui::Align2::LEFT_CENTER, fit(p, &dir, &small, room), small, c.muted);
                    }
                    let r = r.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(format!("{} ({})", path.display(), word(*status)));
                    if r.clicked() {
                        pressed = Some(path.clone());
                    }
                }
            });
        });
    });
    pressed
}

/// The `changes` of `file_tree`: one line a file, git's letter then the path from the tree's root.
pub(crate) fn changes_text(root: &Path, files: &[(PathBuf, Status)]) -> String {
    let files = sorted(root, files);
    if files.is_empty() {
        return "Nenhuma alteração contra o HEAD.".into();
    }
    let lines: Vec<String> =
        files.iter().map(|(path, s)| format!("{} {}", badge(*s), path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/"))).collect();
    format!("{}:\n{}", count(files.len()), lines.join("\n"))
}

impl DrawingApp {
    /// Shows or hides the Alterações panel of tree `id` (it takes the place of the Banco de Dados panel).
    pub(crate) fn ft_show_changes(&mut self, id: &str, show: bool) {
        if let Some(spec) = self.ft_spec_mut(id) {
            spec.changes = show;
            spec.db &= !show;
        }
        if show {
            self.ft_refresh(id);
        }
    }

    /// A file of the panel pressed: its diff against the HEAD in the editor pane.
    pub(crate) fn ft_open_change(&mut self, id: &str, file: &Path) {
        self.ft_show_editor(id, true);
        if let Err(why) = self.ft_editor(id).open_change(file) {
            self.toast(why);
        }
    }

    /// The `file_tree` action `changes`: the files changed against the HEAD, read now (`show` opens the panel; with a `path`
    /// that is one of them, its diff opens too).
    pub(crate) fn act_tree_changes(&mut self, id: &str, args: &Value, file: Option<PathBuf>) -> Result<String, String> {
        let root = self.ft_spec(id).map(FileTreeSpec::root_path).ok_or("árvore não encontrada")?;
        let git = TreeGit::read(&root);
        if !git.repo {
            return Err("Esta pasta não está num repositório git.".into());
        }
        if let Some(show) = args["show"].as_bool() {
            self.ft_show_changes(id, show);
        }
        if let Some(file) = file {
            self.ft_open_change(id, &file);
        }
        Ok(changes_text(&root, &git.files))
    }
}
