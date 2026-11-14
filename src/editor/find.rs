//! Find in Files (Ctrl+Shift+F) with preview, the Find tool window, and the text search behind
//! them and Search Everywhere's Text tab.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub len: usize,
    pub text: String,
}

/// What a line is searched for: the text as typed (with the case told apart or not), or a regular expression.
pub(crate) enum Needle {
    Text(String, bool),
    Regex(regex::Regex),
    /// An expression run over the whole text, so a match may cross lines ("várias linhas").
    Whole(regex::Regex),
}

impl Needle {
    /// `q` as text or, with `regex`, as an expression; Err says what is wrong with the expression.
    pub(crate) fn new(q: &str, case: bool, regex: bool) -> Result<Needle, String> {
        if !regex {
            return Ok(Needle::Text(q.to_string(), case));
        }
        Self::build(q, case, false)
    }

    /// `q` as text or an expression, run over the whole text: a match may cross lines (`\n`, `^` and `$` at each line).
    pub(crate) fn whole(q: &str, case: bool, regex: bool) -> Result<Needle, String> {
        Self::build(&if regex { q.to_string() } else { regex::escape(q) }, case, true)
    }

    fn build(q: &str, case: bool, whole: bool) -> Result<Needle, String> {
        let built = regex::RegexBuilder::new(q).case_insensitive(!case).multi_line(whole).build();
        built.map(|re| if whole { Needle::Whole(re) } else { Needle::Regex(re) }).map_err(|e| e.to_string().lines().last().unwrap_or_default().to_string())
    }

    /// The next match in `line` from byte `from`: its start and end. An empty match (an empty text, an expression
    /// that matches nothing) ends the line.
    fn next(&self, line: &str, from: usize) -> Option<(usize, usize)> {
        let found = match self {
            Needle::Text(q, true) => line[from..].find(q.as_str()).map(|i| (from + i, from + i + q.len())),
            Needle::Text(q, false) => {
                let n = q.as_bytes();
                line.as_bytes()[from..].windows(n.len().max(1)).position(|w| w.eq_ignore_ascii_case(n)).map(|i| (from + i, from + i + n.len()))
            }
            Needle::Regex(re) | Needle::Whole(re) => re.find_at(line, from).map(|m| (m.start(), m.end())),
        };
        found.filter(|(at, end)| end > at)
    }
}

/// The matches of `needle` in one file's `text`, appended to `out` up to `limit`.
fn scan(path: &Path, text: &str, needle: &Needle, words: bool, out: &mut Vec<Hit>, limit: usize) {
    let is_id = |c: char| c.is_alphanumeric() || c == '_';
    if let Needle::Whole(_) = needle {
        for range in ranges(text, needle, words) {
            let line_start = text[..range.start].rfind('\n').map_or(0, |i| i + 1);
            let line_end = text[range.start..].find('\n').map_or(text.len(), |i| range.start + i);
            out.push(Hit {
                path: path.to_path_buf(),
                line: text[..line_start].matches('\n').count(),
                col: text[line_start..range.start].chars().count(),
                len: text[range.start..range.end.min(line_end)].chars().count().max(1),
                text: text[line_start..line_end].trim_end_matches('\r').chars().take(400).collect(),
            });
            if out.len() >= limit {
                return;
            }
        }
        return;
    }
    for (ln, line) in text.lines().enumerate() {
        let mut from = 0;
        while let Some((at, end)) = needle.next(line, from) {
            from = end;
            if words && (line[..at].ends_with(is_id) || line[end..].starts_with(is_id)) {
                continue;
            }
            let col = line[..at].chars().count();
            out.push(Hit { path: path.to_path_buf(), line: ln, col, len: line[at..end].chars().count(), text: line.chars().take(400).collect() });
            if out.len() >= limit {
                return;
            }
        }
    }
}

/// Every match of `needle` in `text` as byte ranges (one of `Whole` may cross lines); `words` keeps whole words only.
pub(crate) fn ranges(text: &str, needle: &Needle, words: bool) -> Vec<std::ops::Range<usize>> {
    let is_id = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = vec![];
    if let Needle::Whole(re) = needle {
        for found in re.find_iter(text).filter(|found| !found.is_empty()) {
            if !(words && (text[..found.start()].ends_with(is_id) || text[found.end()..].starts_with(is_id))) {
                out.push(found.range());
            }
        }
        return out;
    }
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let mut from = 0;
        while let Some((at, end)) = needle.next(body, from) {
            from = end;
            if !(words && (body[..at].ends_with(is_id) || body[end..].starts_with(is_id))) {
                out.push(offset + at..offset + end);
            }
        }
        offset += line.len();
    }
    out
}

/// Case-(in)sensitive, optionally whole-word text search over files read from the disk. Stops
/// early when `epoch` moves on.
pub fn search(files: &[PathBuf], q: &str, case: bool, words: bool, cancel: &AtomicUsize, epoch: usize, limit: usize) -> Vec<Hit> {
    let (mut out, needle) = (vec![], Needle::Text(q.to_string(), case));
    for p in files {
        if q.is_empty() || cancel.load(Ordering::Relaxed) != epoch || out.len() >= limit {
            break;
        }
        if std::fs::metadata(p).map_or(true, |m| m.len() > textindex::MAX_FILE) {
            continue;
        }
        if let Some(text) = textindex::read_text(p) {
            scan(p, &text, &needle, words, &mut out, limit);
        }
    }
    out
}

/// The matches of any `needle` in the files of the text index that `keep` lets through (a folder, a file mask).
pub(crate) fn search_in(
    ix: &textindex::TextIndex,
    needle: &Needle,
    words: bool,
    keep: impl Fn(&Path) -> bool,
    cancel: &AtomicUsize,
    epoch: usize,
    limit: usize,
) -> Vec<Hit> {
    let mut out = vec![];
    for e in ix.entries.iter().filter(|e| keep(&e.path)) {
        if cancel.load(Ordering::Relaxed) != epoch || out.len() >= limit {
            break;
        }
        match &e.text {
            Some(text) => scan(&e.path, text, needle, words, &mut out, limit),
            None => {
                if let Some(text) = textindex::read_text(&e.path) {
                    scan(&e.path, &text, needle, words, &mut out, limit);
                }
            }
        }
    }
    out
}

/// Line of text with the matched range on a highlight background.
pub(crate) fn match_text(ui: &Ui, pos: egui::Pos2, h: &Hit, max_w: f32) -> f32 {
    let font = t::mono(t::EDITOR_SIZE - 1.0);
    let trimmed = h.text.trim_start();
    let skip = h.text.chars().count() - trimmed.chars().count();
    let before: String = trimmed.chars().take(h.col.saturating_sub(skip)).collect();
    let hit: String = trimmed.chars().skip(h.col.saturating_sub(skip)).take(h.len).collect();
    let after: String = trimmed.chars().skip(h.col.saturating_sub(skip) + h.len).collect();
    let p = ui.painter().with_clip_rect(Rect::from_min_size(pos2(pos.x, pos.y - 12.0), vec2(max_w, 24.0)));
    let g1 = p.layout_no_wrap(before, font.clone(), t::EDITOR_TEXT);
    let g2 = p.layout_no_wrap(hit, font.clone(), Color32::WHITE);
    let g3 = p.layout_no_wrap(after, font, t::EDITOR_TEXT);
    let y = pos.y - g1.size().y / 2.0;
    let x2 = pos.x + g1.size().x;
    p.rect_filled(Rect::from_min_size(pos2(x2 - 1.0, y), vec2(g2.size().x + 2.0, g2.size().y)), 2, t::SEARCH_MATCH);
    let x3 = x2 + g2.size().x;
    let end = x3 + g3.size().x;
    p.galley(pos2(pos.x, y), g1, t::EDITOR_TEXT);
    p.galley(pos2(x2, y), g2, Color32::WHITE);
    p.galley(pos2(x3, y), g3, t::EDITOR_TEXT);
    end
}

pub(crate) fn popup_frame(ui: &Ui) -> Frame {
    Frame::popup(ui.style()).fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(12).inner_margin(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_search_options() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-find-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.php");
        std::fs::write(&f, "$user = User::find(1);\n// users ação User\n").unwrap();
        let files = vec![f.clone()];
        let c = AtomicUsize::new(0);
        assert_eq!(search(&files, "user", false, false, &c, 0, 100).len(), 4);
        assert_eq!(search(&files, "User", true, false, &c, 0, 100).len(), 2);
        let w = search(&files, "user", false, true, &c, 0, 100);
        assert_eq!(w.len(), 3);
        // char columns survive multi-byte text before the match
        let last = w.last().unwrap();
        assert_eq!((last.line, last.col, last.len), (1, 14, 4));
        // cancelled searches stop
        assert!(search(&files, "user", false, false, &c, 7, 100).is_empty());
        // a regular expression, any case or not, whole words too; a broken one says why, an empty match finds nothing
        let ix = textindex::TextIndex::build(&files, &Default::default());
        let by = |q: &str, case: bool, words: bool| search_in(&ix, &Needle::new(q, case, true).unwrap(), words, |_| true, &c, 0, 100);
        let found: Vec<_> = by(r"us\w+", false, false).iter().map(|h| (h.line, h.col, h.len)).collect();
        assert_eq!(found, [(0, 1, 4), (0, 8, 4), (1, 3, 5), (1, 14, 4)]);
        assert_eq!(by(r"U\w+", true, true).len(), 2);
        assert_eq!(by("x*", false, false).len(), 0);
        assert!(Needle::new("(", false, true).is_err());
        // only the files let through
        assert!(search_in(&ix, &Needle::new("user", false, false).unwrap(), false, |p| p.extension().is_some_and(|e| e == "rs"), &c, 0, 100).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
