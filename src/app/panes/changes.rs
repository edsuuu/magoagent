//! Alterações: the files changed against the HEAD in the folders of the canvas's file trees (the
//! git the trees already read, filetree/view.rs), a group per folder with the agents working in
//! it, each file with its lines added and removed. A click shows its diff; "Abrir na Árvore" opens
//! it in the tree's editor, as the tree's own Alterações panel does.
// ponytail: no "Descartar": the app has no action that throws a whole file's changes away (only
// file_tree's revert_hunk, one range at a time); add one to filetree/ and its button here.

use super::{card, live};
use crate::app::filetree::diff::head_text;
use crate::editor::{Status, icons};
use crate::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::Receiver;

/// How often the counts of lines are read again while the panel is open, in seconds.
const RECOUNT: f64 = 5.0;
/// The most lines of a diff the panel shows.
const DIFF_LINES: usize = 400;

/// The lines added and removed of each changed file, by `key`.
type Counts = HashMap<String, (usize, usize)>;

#[derive(Default)]
pub(crate) struct ChangesPane {
    counts: Counts,
    reading: Option<Receiver<Counts>>,
    pub(crate) read_at: Option<f64>,
    /// The file whose diff shows: its tree's id and its path, and the diff.
    pub(crate) selected: Option<(String, PathBuf)>,
    pub(crate) diff: Vec<(char, String)>,
}

/// One folder's changes: its tree's id, the header, the folder, the files.
pub(crate) struct ChangeGroup {
    pub(crate) tree: String,
    pub(crate) title: String,
    pub(crate) root: PathBuf,
    pub(crate) files: Vec<(PathBuf, Status)>,
}

/// A path as a key that is the same whichever slash git or the system wrote.
fn key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// `git diff --numstat` of `root`: added and removed lines by path (binary files have none).
pub(crate) fn parse_numstat(root: &Path, out: &str) -> Counts {
    out.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let (added, removed, path) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?);
            Some((key(&root.join(path)), (added, removed)))
        })
        .collect()
}

/// The diff of `old` to `new` as lines (`+`, `-`, ` `), three around each change, `…` between
/// stretches apart; `DIFF_LINES` at most.
pub(crate) fn diff_lines(old: &str, new: &str) -> Vec<(char, String)> {
    let diff = similar::TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    for (index, group) in diff.grouped_ops(3).iter().enumerate() {
        if index > 0 {
            lines.push(('…', String::new()));
        }
        for op in group {
            for change in diff.iter_changes(op) {
                let mark = match change.tag() {
                    similar::ChangeTag::Insert => '+',
                    similar::ChangeTag::Delete => '-',
                    similar::ChangeTag::Equal => ' ',
                };
                lines.push((mark, change.value().trim_end_matches(['\n', '\r']).to_string()));
            }
        }
    }
    lines.truncate(DIFF_LINES);
    lines
}

/// The counts of every group's folder, read away from the frame: the diff against the HEAD, and
/// the lines of each new file.
fn count_lines(groups: Vec<(PathBuf, Vec<PathBuf>)>) -> Counts {
    let mut counts = Counts::new();
    for (root, untracked) in groups {
        if let Ok(out) = crate::editor::run_git(&root, &["diff", "--numstat", "--relative", "HEAD"]) {
            counts.extend(parse_numstat(&root, &out));
        }
        for file in untracked {
            if let Ok(text) = fs::read_to_string(&file) {
                counts.insert(key(&file), (text.lines().count(), 0));
            }
        }
    }
    counts
}

impl DrawingApp {
    /// The changed files of each file tree's folder (once per folder), with the agents whose
    /// folder it is.
    pub(crate) fn change_groups(&self) -> Vec<ChangeGroup> {
        let rows = self.agent_rows();
        let mut groups: Vec<ChangeGroup> = Vec::new();
        for element in &self.scene.elements {
            let Some(spec) = &element.filetree else { continue };
            let root = spec.root_path();
            if groups.iter().any(|g| key(&g.root) == key(&root)) {
                continue;
            }
            let Some(view) = self.filetrees.views.get(&element.id).filter(|v| v.git.repo) else { continue };
            let agents: Vec<&str> =
                rows.iter().filter(|r| !r.spec.cwd.is_empty() && key(Path::new(&r.spec.cwd)).starts_with(&key(&root))).map(|r| r.spec.label()).collect();
            let name = root.file_name().map_or_else(|| root.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned());
            let title = if agents.is_empty() { name } else { format!("{name} · {}", agents.join(", ")) };
            let files = crate::app::filetree::changes::sorted(&root, &view.git.files);
            groups.push(ChangeGroup { tree: element.id.clone(), title, root, files });
        }
        groups
    }

    /// The lines added and removed of `path`, once counted.
    pub(crate) fn change_line_counts(&self, path: &Path) -> Option<(usize, usize)> {
        self.panes.changes.counts.get(&key(path)).copied()
    }

    /// The changed files of all the trees' folders, for the Alterações button's badge.
    pub(crate) fn changed_file_count(&self) -> usize {
        self.change_groups().iter().map(|g| g.files.len()).sum()
    }

    pub(crate) fn changes_pane(&mut self, ui: &mut egui::Ui) {
        let (colors, dark) = (self.colors(), self.dark());
        let groups = self.change_groups();
        self.recount_changes(ui.input(|i| i.time), &groups);
        if groups.iter().all(|g| g.files.is_empty()) {
            let none = if groups.is_empty() { "Nenhuma Árvore de Arquivos de um repositório git no canvas." } else { "Nenhuma alteração pendente." };
            ui.label(egui::RichText::new(none).color(colors.muted));
        }
        let mut pressed = None;
        for group in groups.iter().filter(|g| !g.files.is_empty()) {
            ui.label(egui::RichText::new(&group.title).size(12.0).color(colors.muted));
            // the files close together under their folder
            ui.spacing_mut().item_spacing.y = 2.0;
            for (path, status) in &group.files {
                let selected = self.panes.changes.selected.as_ref().is_some_and(|(_, p)| p == path);
                let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
                let p = ui.painter();
                if selected || response.hovered() {
                    p.rect_filled(row, 7, if selected { colors.selected } else { glass_ink(dark).gamma_multiply(0.06) });
                }
                icons::file_on(p, path, Pos2::new(row.left() + 14.0, row.center().y), 1.0, !dark);
                let font = FontId::monospace(12.0);
                // the counts once read (a binary file has none)
                let mut end = row.right() - 8.0;
                if let Some((added, removed)) = self.change_line_counts(path) {
                    let minus = p.text(Pos2::new(end, row.center().y), egui::Align2::RIGHT_CENTER, format!("−{removed}"), font.clone(), colors.danger);
                    let plus = p.text(minus.left_center() - Vec2::new(6.0, 0.0), egui::Align2::RIGHT_CENTER, format!("+{added}"), font.clone(), ADDED_GREEN);
                    end = plus.left();
                }
                let name = path.strip_prefix(&group.root).unwrap_or(path).to_string_lossy().replace('\\', "/");
                let room = Rect::from_min_max(Pos2::new(row.left() + 28.0, row.top()), Pos2::new(end - 8.0, row.bottom()));
                let color = crate::app::filetree::ui::status_color(*status, dark);
                p.with_clip_rect(room).text(room.left_center(), egui::Align2::LEFT_CENTER, &name, font, color);
                let shown = name.clone();
                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &shown));
                if response.clicked() {
                    pressed = Some((group.tree.clone(), path.clone()));
                }
            }
            ui.spacing_mut().item_spacing.y = 10.0;
        }
        if let Some((tree, path)) = pressed.filter(|_| live(ui)) {
            self.select_change(&tree, &path);
        }
        let Some((tree, path)) = self.panes.changes.selected.clone() else { return };
        diff_box(ui, dark, &self.panes.changes.diff);
        ui.horizontal(|ui| {
            if ui.button("Abrir na Árvore").clicked() && live(ui) {
                self.open_change_in_tree(&tree, &path);
            }
        });
    }

    /// A file of the list clicked: its diff shows under it (a second click hides it).
    pub(crate) fn select_change(&mut self, tree: &str, path: &Path) {
        let changes = &mut self.panes.changes;
        if changes.selected.as_ref().is_some_and(|(_, p)| p == path) {
            (changes.selected, changes.diff) = (None, Vec::new());
            return;
        }
        let old = head_text(path).unwrap_or_default();
        let new = fs::read_to_string(path).unwrap_or_default().replace("\r\n", "\n");
        changes.diff = diff_lines(&old, &new);
        changes.selected = Some((tree.to_string(), path.to_path_buf()));
    }

    /// "Abrir na Árvore": the tree enlarged with the file's diff in its editor.
    pub(crate) fn open_change_in_tree(&mut self, tree: &str, path: &Path) {
        let Some(index) = self.scene.elements.iter().position(|e| e.id == tree) else { return };
        self.show_tree(index);
        self.ft_open_change(tree, path);
    }

    /// Every `RECOUNT` seconds (or on opening), the counts are read again on a thread of their own.
    fn recount_changes(&mut self, now: f64, groups: &[ChangeGroup]) {
        let changes = &mut self.panes.changes;
        if let Some(counts) = changes.reading.as_ref().and_then(|r| r.try_recv().ok()) {
            (changes.counts, changes.reading) = (counts, None);
        }
        if changes.reading.is_some() || changes.read_at.is_some_and(|at| now - at < RECOUNT) {
            return;
        }
        changes.read_at = Some(now);
        let work: Vec<(PathBuf, Vec<PathBuf>)> = groups
            .iter()
            .filter(|g| !g.files.is_empty())
            .map(|g| (g.root.clone(), g.files.iter().filter(|(_, s)| *s == Status::Untracked).map(|(p, _)| p.clone()).collect()))
            .collect();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || sender.send(count_lines(work)));
        changes.reading = Some(receiver);
    }
}

/// The added lines' green (the tree's own).
const ADDED_GREEN: Color32 = crate::app::filetree::diff::ADDED;

/// The diff in a box, a row per line, green for added and red for removed.
fn diff_box(ui: &mut egui::Ui, dark: bool, lines: &[(char, String)]) {
    card(ui, dark, |ui| {
        if lines.is_empty() {
            ui.label("Não há diferenças com o HEAD.");
            return;
        }
        let ink = glass_ink(dark);
        let font = FontId::monospace(12.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        for (mark, text) in lines {
            let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 17.0), Sense::hover());
            let p = ui.painter().with_clip_rect(row);
            match mark {
                '+' => drop(p.rect_filled(row, 0, Color32::from_rgba_unmultiplied(46, 160, 67, 40))),
                '-' => drop(p.rect_filled(row, 0, Color32::from_rgba_unmultiplied(248, 81, 73, 40))),
                _ => {}
            }
            let shown = if *mark == '…' { "…".to_string() } else { format!("{mark} {text}") };
            p.text(row.left_center(), egui::Align2::LEFT_CENTER, shown, font.clone(), if *mark == ' ' { ink.gamma_multiply(0.7) } else { ink });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numstat_counts_by_path_and_skips_binaries() {
        let root = Path::new("/repo");
        let counts = parse_numstat(root, "3\t1\tsrc/main.rs\n-\t-\tlogo.png\n10\t0\tREADME.md\n");
        assert_eq!(counts.get(&key(&root.join("src").join("main.rs"))), Some(&(3, 1)));
        assert_eq!(counts.get(&key(&root.join("README.md"))), Some(&(10, 0)));
        assert_eq!(counts.len(), 2, "the binary file has no lines");
    }

    #[test]
    fn the_diff_keeps_three_lines_around_each_change() {
        let old: String = (1..=20).map(|n| format!("linha {n}\n")).collect();
        let new = old.replace("linha 2\n", "linha dois\n").replace("linha 18\n", "");
        let lines = diff_lines(&old, &new);
        assert!(lines.contains(&('-', "linha 2".into())) && lines.contains(&('+', "linha dois".into())));
        assert!(lines.contains(&('-', "linha 18".into())));
        assert!(lines.contains(&('…', String::new())), "the stretches apart are split: {lines:?}");
        assert!(!lines.iter().any(|(_, text)| text == "linha 10"), "far from the changes");
        assert!(diff_lines("igual\n", "igual\n").is_empty());
    }
}
