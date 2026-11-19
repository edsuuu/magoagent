//! Recent and favorite folders: the menu beside a folder field and on the chat's folder chip.

use crate::prelude::*;

/// How many recent folders are kept.
const RECENT: usize = 8;

/// The path without the separator at its end (a root keeps it).
fn clean(path: &str) -> &str {
    let path = path.trim();
    let cut = path.trim_end_matches(['/', '\\']);

    if cut.is_empty() || cut.ends_with(':') { path } else { cut }
}

/// What makes two spellings the same folder: on Windows, neither the case nor the slash tells them apart.
fn key(path: &str) -> String {
    let path = clean(path);

    if cfg!(windows) { path.to_lowercase().replace('/', "\\") } else { path.to_string() }
}

impl Folders {
    pub(crate) fn is_empty(&self) -> bool {
        self.recent.is_empty() && self.favorite.is_empty()
    }

    pub(crate) fn is_favorite(&self, path: &str) -> bool {
        self.favorite.iter().any(|f| key(f) == key(path))
    }

    /// A folder that was just used: the first of the recent ones, which keep the last `RECENT`.
    pub(crate) fn remember(&mut self, path: &str) {
        let path = clean(path);

        if path.is_empty() || path == "~" {
            return;
        }

        self.recent.retain(|p| key(p) != key(path));
        self.recent.insert(0, path.to_string());
        self.recent.truncate(RECENT);
    }

    /// Stars the folder, or takes its star off; whether it is a favorite now.
    pub(crate) fn toggle_favorite(&mut self, path: &str) -> bool {
        let path = clean(path);

        if path.is_empty() {
            return false;
        }

        let before = self.favorite.len();

        self.favorite.retain(|p| key(p) != key(path));

        if self.favorite.len() == before {
            self.favorite.push(path.to_string());
        }

        self.favorite.len() > before
    }

    /// The menu's rows: the favorites, then the recent ones that are not favorites.
    pub(crate) fn choices(&self) -> Vec<(String, bool)> {
        let recent = self.recent.iter().filter(|p| !self.is_favorite(p)).map(|p| (p.clone(), false));

        self.favorite.iter().map(|p| (p.clone(), true)).chain(recent).collect()
    }
}

/// What a click on the menu asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FolderPick {
    Use(String),
    Star(String),
}

/// The menu's rows: the favorites, then the recent ones; a click uses one, the word at the right stars it.
pub(crate) fn folder_rows(ui: &mut egui::Ui, folders: &Folders, c: &UiColors) -> Option<FolderPick> {
    let rows: Vec<(String, Option<bool>)> = folders.choices().into_iter().map(|(p, f)| (p, Some(f))).collect();

    folder_list(ui, &rows, c)
}

/// How tall a folder row is.
pub(crate) const FOLDER_ROW: f32 = 32.0;

/// Folder rows: the folder icon, its path (its name alone said nothing more) and, at the right, "Favorita" on a favorite or
/// "Favoritar" under the pointer (none where the star is `None`). A folder that is gone stays listed (its
/// star can still come off) but cannot be picked. They take the width they are given.
pub(crate) fn folder_list(ui: &mut egui::Ui, rows: &[(String, Option<bool>)], c: &UiColors) -> Option<FolderPick> {
    let mut pick = None;

    for (path, favorite) in rows {
        // ponytail: one stat per row per frame while the list is open; cache it if a network folder makes it stutter
        let there = std::path::Path::new(path).is_dir();
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), FOLDER_ROW), Sense::click());
        let star = favorite.map(|_| Rect::from_min_max(Pos2::new(rect.right() - 84.0, rect.top()), rect.max));
        let star_response = star.map(|s| ui.interact(s, ui.scope_id().with(("folder_star", path)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand));
        let over = star_response.as_ref().is_some_and(egui::Response::hovered);
        let hot = response.hovered() || over;

        if hot {
            ui.painter().rect_filled(rect, 6, c.surface_high);
        }

        paint_icon(
            ui.painter(),
            Icon::Tool(Tool::FileTree),
            Pos2::new(rect.left() + 18.0, rect.center().y),
            if there { c.muted } else { c.muted.gamma_multiply(0.5) },
        );

        let left = rect.left() + 36.0;

        let word = match favorite {
            Some(true) => Some(("Favorita", c.text)),
            Some(false) if hot => Some(("Favoritar", if over { c.text } else { c.muted })),
            _ => None,
        };

        let right = star.filter(|_| word.is_some()).map_or(rect.right() - 10.0, |s| s.left() - 6.0);
        let shown = if there { path.clone() } else { format!("{path} (não existe mais)") };
        // a long path ends in "…" before the star's word
        let color = if there { c.text } else { c.muted };
        let galley = crate::app::chat::list::clipped(ui.painter(), shown, FontId::proportional(13.0), color, right - left);

        ui.painter().galley(Pos2::new(left, rect.center().y - galley.size().y / 2.0), galley, color);

        if let Some((word, color)) = word {
            ui.painter().text(Pos2::new(rect.right() - 12.0, rect.center().y), egui::Align2::RIGHT_CENTER, word, FontId::proportional(12.0), color);
        }

        let favorite = favorite.unwrap_or_default();

        if star_response.is_some_and(|r| r.tip(if favorite { "Tirar das favoritas" } else { "Deixar sempre no topo da lista" }).clicked()) {
            pick = Some(FolderPick::Star(path.clone()));
        } else if there && response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            pick = Some(FolderPick::Use(path.clone()));
        }
    }

    pick
}

/// Tab in a folder field, as a shell does: the subfolders of the typed one that start with its last part
/// (the case aside on Windows). One completes it, with its separator; more go as far as they all agree, and
/// come back to be listed.
pub(crate) fn complete_folder(text: &str) -> (String, Vec<String>) {
    let Some(at) = text.rfind(['/', '\\']) else { return (text.to_string(), vec![]) };
    let (parent, part) = (&text[..=at], &text[at + 1..]);
    let sep = &text[at..=at];
    let fold = |s: &str| if cfg!(windows) { s.to_lowercase() } else { s.to_string() };

    // "~/" reads the home folder and stays "~/" in the field
    let real = match (parent.strip_prefix('~'), std::env::home_dir()) {
        (Some(rest), Some(home)) => format!("{}{rest}", home.display()),
        _ => parent.to_string(),
    };

    let mut names: Vec<String> = fs::read_dir(&real)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| fold(n).starts_with(&fold(part)) && (!n.starts_with('.') || part.starts_with('.')))
        .collect();

    names.sort_by_key(|n| fold(n));

    match &names[..] {
        [] => (text.to_string(), vec![]),
        [one] => (format!("{parent}{one}{sep}"), vec![]),
        [first, ..] => {
            let agree = |n: usize| names.iter().all(|m| m.get(..n).is_some_and(|p| fold(p) == fold(&first[..n])));
            let common = (0..=first.len()).rev().find(|&n| first.is_char_boundary(n) && agree(n)).unwrap_or(0);
            let keep = if common > part.len() { &first[..common] } else { part };

            (format!("{parent}{keep}"), names.iter().map(|n| format!("{parent}{n}")).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_used_folder_goes_first_once_and_only_the_last_ones_stay() {
        let mut f = Folders::default();

        for path in ["/a", "/b/", "  ", "~", "/a"] {
            f.remember(path);
        }

        assert_eq!(f.recent, ["/a", "/b"]); // again goes to the front, the separator and blanks do not count

        for n in 0..20 {
            f.remember(&format!("/p{n}"));
        }

        assert_eq!(f.recent.len(), RECENT);
        assert_eq!(f.recent[0], "/p19");
        f.remember("C:\\");
        assert_eq!(f.recent[0], "C:\\"); // a root keeps its separator

        if cfg!(windows) {
            f.remember("c:/Projetos/App");
            f.remember("C:\\projetos\\app\\");
            assert_eq!(f.recent.iter().filter(|p| p.to_lowercase().contains("projetos")).count(), 1);
        }
    }

    #[test]
    fn favorites_come_first_and_a_second_star_takes_it_off() {
        let mut f = Folders::default();

        f.remember("/a");
        f.remember("/b");
        assert!(f.toggle_favorite("/a/"));
        assert!(f.toggle_favorite("/c"));
        // the favorites in the order they were starred, then what is only recent
        assert_eq!(f.choices(), [("/a".to_string(), true), ("/c".to_string(), true), ("/b".to_string(), false)]);
        assert!(!f.toggle_favorite("/a"));
        assert_eq!(f.choices(), [("/c".to_string(), true), ("/b".to_string(), false), ("/a".to_string(), false)]);
        assert!(!f.toggle_favorite("  "));
        assert!(!Folders::default().toggle_favorite(""));
        assert!(Folders::default().is_empty() && !f.is_empty());
    }

    #[test]
    fn tab_completes_a_folder_as_far_as_its_subfolders_agree() {
        let dir = std::env::temp_dir().join(format!("magoagent-tab-{}", std::process::id()));

        for sub in ["Projetos", "projetos-velhos", "Prova", ".oculta"] {
            fs::create_dir_all(dir.join(sub)).unwrap();
        }

        fs::write(dir.join("Pronto.txt"), "").unwrap();
        let base = format!("{}/", dir.display());

        // one folder: it, with the separator typed
        assert_eq!(complete_folder(&format!("{base}Prov")), (format!("{base}Prova/"), vec![]));
        // more: as far as they agree, and the list (a file is not one)
        let (done, list) = complete_folder(&format!("{base}Proj"));

        if cfg!(windows) {
            assert_eq!(done, format!("{base}Projetos"));
            assert_eq!(list.len(), 2);
        } else {
            assert_eq!((done, list), (format!("{base}Projetos/"), vec![]));
        }

        let (_, all) = complete_folder(&base);

        assert_eq!(all.len(), 3, "the hidden one only when asked: {all:?}");
        assert_eq!(complete_folder(&format!("{base}nada")).0, format!("{base}nada"));
        assert_eq!(complete_folder("sem barra"), ("sem barra".to_string(), vec![]));
        let _ = fs::remove_dir_all(&dir);
    }
}
