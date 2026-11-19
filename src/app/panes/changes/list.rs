//! The list of Alterações: a field that looks for a file by its name or for a stretch of code in its changes (the
//! lines added and removed, read with the counts), then the files of each repository by the folder they are in, in a
//! box six rows tall that scrolls for the rest; a repository and a folder fold under their header, which counts their
//! files. A file found by its code shows the line that has it.

use super::{ChangeGroup, Changed, key};
use crate::editor::{Status, icons};
use crate::prelude::*;
use std::path::Path;

/// A file's row, the room between rows, and a repository's or a folder's header.
const ROW: f32 = 26.0;
const ROW_GAP: f32 = 2.0;
const HEADER: f32 = 20.0;
/// The rows the list shows before it scrolls.
pub(crate) const LIST_ROWS: usize = 6;
/// The changed lines kept of a file, and the characters of each: enough to find a stretch, not a copy of the repository.
// ponytail: a stretch past a file's first 2000 changed lines (or a line's 300th character) is not found; to evolve, ask
// `git diff -G<text>` for the files when the search finds nothing in what is kept.
const KEPT_LINES: usize = 2000;
const KEPT_CHARS: usize = 300;

/// Why a file answers the search: its name, or a changed line that has the text (that line, trimmed).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FileMatch {
    Name,
    Code(String),
}

/// Whether the file `name` (its path in the folder) answers `query`, any case: by its name, else by the first of its
/// changed `lines` that has it. An empty query takes every file.
pub(crate) fn file_match(name: &str, lines: Option<&Vec<String>>, query: &str) -> Option<FileMatch> {
    let query = query.trim().to_lowercase();

    if query.is_empty() || name.to_lowercase().contains(&query) {
        return Some(FileMatch::Name);
    }

    lines?.iter().find(|line| line.to_lowercase().contains(&query)).map(|line| FileMatch::Code(line.trim().to_string()))
}

/// Lines kept for the search: `KEPT_LINES` at most, each cut at `KEPT_CHARS`.
pub(crate) fn kept_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<String> {
    lines.take(KEPT_LINES).map(|line| line.chars().take(KEPT_CHARS).collect()).collect()
}

/// `git diff -U0` of `root`: the lines each file added or removed, by `key`. The headers end at a file's first `@@`, so
/// a removed line that reads `-- x` is not taken for one.
pub(crate) fn parse_changed_lines(root: &Path, out: &str) -> Changed {
    let mut changed = Changed::new();
    let (mut header, mut old, mut path): (bool, Option<String>, Option<String>) = (false, None, None);

    for line in out.lines() {
        if line.starts_with("diff --git ") {
            (header, old, path) = (true, None, None);

            continue;
        }

        if header {
            if let Some(rest) = line.strip_prefix("--- ") {
                old = rest.strip_prefix("a/").map(String::from);
            } else if let Some(rest) = line.strip_prefix("+++ ") {
                // a deleted file is "+++ /dev/null": its old name
                path = rest.strip_prefix("b/").map(String::from).or_else(|| old.clone());
            } else if line.starts_with("@@") {
                header = false;
            }

            continue;
        }

        let (Some(file), Some(text)) = (&path, line.strip_prefix(['+', '-'])) else { continue };
        let lines = changed.entry(key(&root.join(file))).or_default();

        if lines.len() < KEPT_LINES {
            lines.push(text.chars().take(KEPT_CHARS).collect());
        }
    }

    changed
}

/// `files` by the folder each is in under `root` (its path with `/`, "" for the root's own), the folders by name and
/// the files in the order they came.
pub(crate) fn by_folder<T>(root: &Path, files: impl IntoIterator<Item = T>, path: impl Fn(&T) -> &Path) -> Vec<(String, Vec<T>)> {
    let mut folders: Vec<(String, Vec<T>)> = Vec::new();

    for file in files {
        let folder = path(&file).strip_prefix(root).ok().and_then(Path::parent).map_or(String::new(), |parent| parent.to_string_lossy().replace('\\', "/"));

        match folders.iter_mut().find(|(name, _)| *name == folder) {
            Some((_, list)) => list.push(file),
            None => folders.push((folder, vec![file])),
        }
    }

    folders.sort_by_key(|(name, _)| name.to_lowercase());
    folders
}

/// A repository's section in `collapsed` (a folder's is its `key`, which the root's own would share).
pub(crate) fn group_key(root: &Path) -> String {
    format!("repo:{}", key(root))
}

/// A file of the list: its path, its git mark and why the search kept it.
type Shown<'a> = (&'a PathBuf, Status, FileMatch);
/// How far a folder's header and its files sit in from the repository's.
const INDENT: f32 = 12.0;

impl DrawingApp {
    /// The search field and the files that answer it, a folder at a time, in a box `LIST_ROWS` tall; gives the one
    /// pressed (its tree and path).
    pub(super) fn changes_list(&mut self, ui: &mut egui::Ui, groups: &[ChangeGroup]) -> Option<(String, PathBuf)> {
        let colors = self.colors();

        let field = egui::TextEdit::singleline(&mut self.panes.changes.search)
            .hint_text("Buscar arquivo ou trecho de código")
            .background_color(colors.surface_low)
            .desired_width(f32::INFINITY);

        ui.add_sized(Vec2::new(ui.available_width(), 28.0), field);
        let query = self.panes.changes.search.clone();

        let shown: Vec<(&ChangeGroup, Vec<Shown>)> = groups
            .iter()
            .map(|group| {
                let files = group
                    .files
                    .iter()
                    .filter_map(|(path, status)| {
                        let name = path.strip_prefix(&group.root).unwrap_or(path).to_string_lossy().replace('\\', "/");

                        file_match(&name, self.changed_lines(path), &query).map(|found| (path, *status, found))
                    })
                    .collect::<Vec<_>>();

                (group, files)
            })
            .filter(|(_, files)| !files.is_empty())
            .collect();

        let found: usize = shown.iter().map(|(_, files)| files.len()).sum();

        if !query.trim().is_empty() {
            let total: usize = groups.iter().map(|g| g.files.len()).sum();
            let said = if found == 0 { format!("Nada com “{}” no nome nem nas mudanças.", query.trim()) } else { format!("{found} de {total} arquivos") };

            ui.label(egui::RichText::new(said).size(11.0).color(colors.muted));
        }

        let (mut pressed, mut folded) = (None, None);
        // the rows under a repository's header and its first folder's
        let tallest = LIST_ROWS as f32 * (ROW + ROW_GAP) + 2.0 * (HEADER + ROW_GAP);
        // a search shows every file it found, folded or not
        let open = |section: &String| !query.trim().is_empty() || !self.panes.changes.collapsed.contains(section);

        egui::ScrollArea::vertical().id_salt("changes_files").max_height(tallest).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = ROW_GAP;

            for (group, files) in &shown {
                let section = group_key(&group.root);

                if self.change_header(ui, &group.title, files.len(), open(&section), 0.0) {
                    folded = Some(section.clone());
                }

                if !open(&section) {
                    continue;
                }

                for (folder, files) in by_folder(&group.root, files, |(path, ..)| path.as_path()) {
                    let section = key(&group.root.join(&folder));
                    let name = if folder.is_empty() { "(raiz)" } else { folder.as_str() };

                    if self.change_header(ui, name, files.len(), open(&section), INDENT) {
                        folded = Some(section.clone());
                    }

                    for (path, status, found) in files.into_iter().filter(|_| open(&section)) {
                        if self.change_row(ui, group, path, *status, found) {
                            pressed = Some((group.tree.clone(), (*path).clone()));
                        }
                    }
                }
            }
        });

        if let Some(section) = folded.filter(|_| super::live(ui)) {
            let collapsed = &mut self.panes.changes.collapsed;

            if !collapsed.remove(&section) {
                collapsed.insert(section);
            }
        }

        pressed
    }

    /// A section's header: the fold's arrow, its name and how many files it has, `indent` in. True when pressed.
    fn change_header(&self, ui: &mut egui::Ui, title: &str, count: usize, open: bool, indent: f32) -> bool {
        let colors = self.colors();
        let (header, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), HEADER), Sense::click());
        let p = ui.painter();

        if response.hovered() {
            p.rect_filled(header, 5, glass_ink(self.dark()).gamma_multiply(0.05));
        }

        // the arrow: right when folded, down when open
        let center = Pos2::new(header.left() + indent + 7.0, header.center().y);

        let points = if open {
            [Vec2::new(-4.0, -2.0), Vec2::new(4.0, -2.0), Vec2::new(0.0, 3.0)]
        } else {
            [Vec2::new(-2.0, -4.0), Vec2::new(-2.0, 4.0), Vec2::new(3.0, 0.0)]
        };

        p.add(egui::Shape::convex_polygon(points.iter().map(|point| center + *point).collect(), colors.muted, Stroke::NONE));
        let font = FontId::proportional(12.0);
        let counted = p.text(Pos2::new(header.right() - 8.0, header.center().y), egui::Align2::RIGHT_CENTER, count.to_string(), font.clone(), colors.muted);
        let room = Rect::from_min_max(Pos2::new(center.x + 9.0, header.top()), Pos2::new(counted.left() - 8.0, header.bottom()));

        p.with_clip_rect(room).text(room.left_center(), egui::Align2::LEFT_CENTER, title, font, colors.muted);
        let response = response.on_hover_cursor(CursorIcon::PointingHand);

        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::DisclosureTriangle, true, format!("{title} ({count})")));
        response.clicked()
    }

    /// A file's row, in from its folder's header: its icon, its name in its git color, its counts; under it, the line
    /// its code was found in. True when pressed.
    fn change_row(&self, ui: &mut egui::Ui, group: &ChangeGroup, path: &Path, status: Status, found: &FileMatch) -> bool {
        let (colors, dark) = (self.colors(), self.dark());

        let code = match found {
            FileMatch::Code(line) => Some(line),
            FileMatch::Name => None,
        };

        let height = if code.is_some() { ROW + 14.0 } else { ROW };
        let selected = self.panes.changes.selected.as_ref().is_some_and(|(_, p)| p == path);
        let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
        // in from its folder's header
        let line = Rect::from_min_max(Pos2::new(row.left() + INDENT, row.top()), Pos2::new(row.right(), row.top() + ROW));
        let p = ui.painter();

        if selected || response.hovered() {
            p.rect_filled(row, 7, if selected { colors.selected } else { glass_ink(dark).gamma_multiply(0.06) });
        }

        icons::file_on(p, path, Pos2::new(line.left() + 14.0, line.center().y), 1.0, !dark);
        let font = FontId::monospace(12.0);
        // the counts once read (a binary file has none)
        let mut end = line.right() - 8.0;

        if let Some((added, removed)) = self.change_line_counts(path) {
            let minus = p.text(Pos2::new(end, line.center().y), egui::Align2::RIGHT_CENTER, format!("−{removed}"), font.clone(), colors.danger);

            let plus = p.text(
                minus.left_center() - Vec2::new(6.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                format!("+{added}"),
                font.clone(),
                crate::app::filetree::diff::ADDED,
            );

            end = plus.left();
        }

        let name = path.strip_prefix(&group.root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        // its folder is the header's: the name alone
        let file = path.file_name().map_or(name.clone(), |file| file.to_string_lossy().into_owned());
        let room = Rect::from_min_max(Pos2::new(line.left() + 28.0, line.top()), Pos2::new(end - 8.0, line.bottom()));
        let color = crate::app::filetree::ui::status_color(status, dark);

        p.with_clip_rect(room).text(room.left_center(), egui::Align2::LEFT_CENTER, &file, font, color);

        if let Some(code) = code {
            let room = Rect::from_min_max(Pos2::new(line.left() + 28.0, line.bottom() - 4.0), Pos2::new(row.right() - 8.0, row.bottom()));
            let shown = format!("no código: {code}");

            p.with_clip_rect(room).text(room.left_center(), egui::Align2::LEFT_CENTER, shown, FontId::monospace(10.5), colors.muted);
        }

        let response = response.on_hover_cursor(CursorIcon::PointingHand);

        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
        response.clicked()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_search_finds_a_file_by_its_name_or_by_a_stretch_of_its_changes() {
        let lines = vec!["    let total = soma(a, b);".to_string(), "fn velha() {}".to_string()];

        assert_eq!(file_match("src/main.rs", Some(&lines), ""), Some(FileMatch::Name), "an empty search takes everything");
        assert_eq!(file_match("src/main.rs", Some(&lines), "MAIN"), Some(FileMatch::Name), "any case");
        assert_eq!(file_match("src/main.rs", Some(&lines), "soma("), Some(FileMatch::Code("let total = soma(a, b);".into())), "the line, trimmed");
        assert_eq!(file_match("src/main.rs", Some(&lines), "Velha"), Some(FileMatch::Code("fn velha() {}".into())), "a removed line too");
        assert_eq!(file_match("src/main.rs", Some(&lines), "nada"), None);
        assert_eq!(file_match("src/main.rs", None, "soma"), None, "lines not read yet: by the name only");
    }

    #[test]
    fn the_files_go_by_their_folder_with_the_roots_own_first() {
        let root = Path::new("/repo");
        let files = ["/repo/src/a.rs", "/repo/src/a/x.rs", "/repo/src/ab.rs", "/repo/README.md", "/repo/Docs/b.md"].map(PathBuf::from);
        let folders = by_folder(root, &files, |path| path.as_path());
        let names = |list: &Vec<&PathBuf>| list.iter().map(|path| path.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>();

        assert_eq!(folders.iter().map(|(folder, _)| folder.as_str()).collect::<Vec<_>>(), ["", "Docs", "src", "src/a"], "by name, any case");
        assert_eq!(names(&folders[2].1), ["a.rs", "ab.rs"], "a folder's files together, in the order they came");
        assert_eq!(names(&folders[0].1), ["README.md"]);
    }

    #[test]
    fn the_changed_lines_come_by_file_from_the_diff_without_its_headers() {
        let root = Path::new("/repo");

        let out = "diff --git a/src/a.rs b/src/a.rs\nindex 1..2 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-velho\n+novo\n@@ -9,0 +10 @@\n+--- não é cabeçalho\n\
                   diff --git a/gone.sql b/gone.sql\ndeleted file mode 100644\n--- a/gone.sql\n+++ /dev/null\n@@ -1 +0,0 @@\n--- comentário sql\n\\ No newline at end of file\n";

        let changed = parse_changed_lines(root, out);

        assert_eq!(changed[&key(&root.join("src/a.rs"))], vec!["velho", "novo", "--- não é cabeçalho"]);
        assert_eq!(changed[&key(&root.join("gone.sql"))], vec!["-- comentário sql"], "a deleted file goes by its old name");
        assert_eq!(changed.len(), 2);
    }
}
