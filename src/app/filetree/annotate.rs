//! "Anotar com Git Blame": a column left of the line numbers of the tree's editor with the date (dd/mm/aaaa) and the author
//! of each line, as the reference IDE's. Pressed over the author above a declaration, or from the gutter's right-click
//! menu, it is turned on; the same again turns it off. The line under the pointer in the column is washed in soft green, and
//! its commit shows beside the pointer (hash, author, date and message); a click lists the files that commit changed.

use crate::app::filetree::menu::{Lead, Row, menu_row};
use crate::app::filetree::vision::Commit;
use crate::app::ui::menu_look::menu_frame;
use crate::ide::theme as t;
use crate::prelude::*;
use std::collections::HashMap;

/// The wash over the code line whose annotation the pointer is on.
pub(crate) fn hover_line() -> Color32 {
    Color32::from_rgba_unmultiplied(0x4c, 0xaf, 0x50, 0x2e)
}

/// Who wrote each line and when, as the column prints it ("27/10/2026  Maria"); None for a line not committed yet.
#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct Annot {
    pub(crate) lines: Vec<Option<String>>,
    /// The longest of them: the column is as wide as it.
    widest: String,
    /// The commit of each line (an index into `commits`), and each commit with its hint.
    at: Vec<Option<usize>>,
    commits: Vec<(Commit, String)>,
}

/// A day number (days since 1970-01-01) as year, month and day in the civil calendar.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// `time` (Unix seconds) at the zone git wrote beside it (`+0200`).
fn local(time: i64, zone: &str) -> i64 {
    let sign = if zone.starts_with('-') { -1 } else { 1 };
    let digits: i64 = zone.trim_start_matches(['+', '-']).parse().unwrap_or(0);
    time + sign * ((digits / 100) * 3600 + (digits % 100) * 60)
}

/// `time` at `zone` as dd/mm/aaaa.
fn date(time: i64, zone: &str) -> String {
    let (y, m, d) = civil(local(time, zone).div_euclid(86_400));
    format!("{d:02}/{m:02}/{y:04}")
}

/// `time` at `zone` as hh:mm.
fn clock(time: i64, zone: &str) -> String {
    let s = local(time, zone).rem_euclid(86_400);
    format!("{:02}:{:02}", s / 3600, s % 3600 / 60)
}

/// What git blame says of a commit.
#[derive(Clone, Copy, Default)]
struct Meta<'a> {
    name: &'a str,
    mail: &'a str,
    time: i64,
    zone: &'a str,
    summary: &'a str,
}

impl Annot {
    /// From `git blame --porcelain`: each content line takes the author and time of the commit it came from.
    pub(crate) fn parse(out: &str) -> Annot {
        let mut commits: HashMap<&str, Meta> = HashMap::new();
        let (mut sha, mut lines) = ("", vec![]);
        for l in out.lines() {
            if l.starts_with('\t') {
                lines.push(sha);
            } else if let Some(name) = l.strip_prefix("author ") {
                commits.entry(sha).or_default().name = name;
            } else if let Some(mail) = l.strip_prefix("author-mail ") {
                commits.entry(sha).or_default().mail = mail;
            } else if let Some(time) = l.strip_prefix("author-time ") {
                commits.entry(sha).or_default().time = time.trim().parse().unwrap_or(0);
            } else if let Some(zone) = l.strip_prefix("author-tz ") {
                commits.entry(sha).or_default().zone = zone.trim();
            } else if let Some(summary) = l.strip_prefix("summary ") {
                commits.entry(sha).or_default().summary = summary;
            } else if let Some((first, _)) = l.split_once(' ')
                && first.len() == 40
                && first.bytes().all(|b| b.is_ascii_hexdigit())
            {
                sha = first;
            }
        }
        // a line not committed yet is all zeros
        let mut order: Vec<&str> = Vec::new();
        let at: Vec<Option<usize>> = lines
            .iter()
            .map(|s| {
                commits.get(s).filter(|_| !s.bytes().all(|b| b == b'0'))?;
                Some(order.iter().position(|o| o == s).unwrap_or_else(|| {
                    order.push(s);
                    order.len() - 1
                }))
            })
            .collect();
        let lines: Vec<Option<String>> = at
            .iter()
            .map(|k| {
                let m = commits[order[(*k)?]];
                Some(format!("{}  {}", date(m.time, m.zone), m.name))
            })
            .collect();
        let commits: Vec<(Commit, String)> = order
            .iter()
            .map(|s| {
                let m = commits[s];
                let commit = Commit { hash: s.to_string(), author: m.name.into(), time: m.time, subject: m.summary.into() };
                let hint = format!("commit {s}\nAutor: {} {}\nData: {} {}\n\n{}", m.name, m.mail, date(m.time, m.zone), clock(m.time, m.zone), m.summary);
                (commit, hint)
            })
            .collect();
        let widest = lines.iter().flatten().max_by_key(|l| l.chars().count()).cloned().unwrap_or_default();
        Annot { lines, widest, at, commits }
    }

    /// The commit of `line` and its hint, if the line is committed.
    pub(crate) fn commit_at(&self, line: usize) -> Option<&(Commit, String)> {
        self.commits.get((*self.at.get(line)?)?)
    }

    /// The column's width at zoom `z`: the longest line printed in `font`, with room around it.
    pub(crate) fn width(&self, p: &Painter, font: &FontId, z: f32) -> f32 {
        (p.layout_no_wrap(self.widest.clone(), font.clone(), Color32::WHITE).size().x + 18.0 * z).min(280.0 * z)
    }
}

impl DrawingApp {
    /// Turns the annotation column of the open text `path` in tree `id` on or off (`want`; None: the other way). Back: whether it is on.
    pub(crate) fn ft_annotate(&mut self, id: &str, path: &std::path::Path, want: Option<bool>) -> Option<bool> {
        let tab = self.filetrees.editors.get_mut(id)?.tabs.iter_mut().find(|t| t.path == path)?;
        let crate::app::filetree::editor::Body::Text(text) = &mut tab.body else { return None };
        text.annotate = want.unwrap_or(!text.annotate);
        Some(text.annotate)
    }

    /// The `file_tree` action `annotate`: opens `path` in the pane if it is not there, and turns its column on (`show` false: off).
    pub(crate) fn act_tree_annotate(&mut self, id: &str, path: std::path::PathBuf, show: Option<bool>) -> Result<String, String> {
        self.ft_open(id, path.clone(), true);
        let on = self.ft_annotate(id, &path, Some(show.unwrap_or(true))).ok_or("só um arquivo de texto tem anotação de autor.")?;
        Ok(format!("Anotação de autor {} em {}.", if on { "ligada" } else { "desligada" }, path.display()))
    }
}

/// The MCP's `annotate` with `line` (from 1): the commit of that line of `path`, as the pointer over the column shows it,
/// and the files it changed, as a press on it lists them.
pub(crate) fn line_commit(path: &std::path::Path, line: usize) -> Result<String, String> {
    let (dir, name) = (path.parent().ok_or("diga o arquivo")?, path.file_name().ok_or("diga o arquivo")?.to_string_lossy());
    let range = format!("-L{line},{line}");
    let a = Annot::parse(&crate::ide::run_git(dir, &["blame", "--porcelain", range.as_str(), "--", name.as_ref()])?);
    let (commit, hint) = a.commit_at(0).ok_or(format!("A linha {line} ainda não foi commitada."))?;
    let files = crate::app::filetree::vision::commit_files(path, &commit.hash)?;
    let list: String = files.iter().map(|(s, _, rel)| format!("{} {rel}\n", crate::app::filetree::ui::badge(*s))).collect();
    Ok(format!("{hint}\n\nArquivos ({}):\n{list}", files.len()))
}

/// The column in `rect` (the left part of the gutter): each visible line's text. `spans` are the lines' rows from `origin_y`.
/// Back: the line the pointer is on, if it is in the column.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_column(
    p: &Painter,
    rect: Rect,
    origin_y: f32,
    spans: &[(f32, f32)],
    visible: std::ops::Range<usize>,
    a: &Annot,
    font: &FontId,
    pointer: Option<Pos2>,
    z: f32,
) -> Option<usize> {
    let mut over = None;
    for line in visible {
        let (top, bottom) = (origin_y + spans[line].0, origin_y + spans[line].1);
        if pointer.is_some_and(|q| rect.x_range().contains(q.x) && (top..bottom).contains(&q.y)) {
            over = Some(line);
        }
        if let Some(Some(text)) = a.lines.get(line) {
            let color = if over == Some(line) { t::TEXT } else { t::AUTHOR };
            let shown = crate::app::filetree::ui::fit(p, text, font, rect.width() - 12.0 * z);
            p.text(Pos2::new(rect.left() + 6.0 * z, (top + bottom) / 2.0), egui::Align2::LEFT_CENTER, shown, font.clone(), color);
        }
    }
    p.vline(rect.right(), rect.y_range(), Stroke::new(1.0, t::POPUP_BORDER));
    over
}

/// The gutter's right-click menu: "Anotar com Git Blame" (ticked while the column is on). Drawn over the whole gutter, under the
/// change markers, which keep their own presses.
pub(crate) fn gutter_menu(ui: &mut egui::Ui, gutter: Rect, id: egui::Id, on: &mut bool) -> egui::Response {
    let r = ui.interact(gutter.intersect(ui.clip_rect()), id.with("gutter_menu"), Sense::click());
    let dark = ui.visuals().dark_mode;
    egui::Popup::context_menu(&r).frame(menu_frame(dark)).show(|ui| {
        ui.set_width(190.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        if menu_row(ui, Row { lead: Lead::Tick(*on), ..Row::new(None, "Anotar com Git Blame") }).clicked() {
            *on = !*on;
            ui.close();
        }
    });
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blame_becomes_a_date_and_an_author_per_line() {
        let sha = |c: char| c.to_string().repeat(40);
        let out = format!(
            "{a} 1 1 2\nauthor Maria\nauthor-mail <maria@x>\nauthor-time 1793102400\nauthor-tz -0300\nsummary x\nfilename f.rs\n\tlinha um\n{a} 2 2\n\tlinha dois\n{z} 3 3 1\nauthor Not Committed Yet\nauthor-time 0\nauthor-tz +0000\n\tnova\n",
            a = sha('a'),
            z = "0".repeat(40)
        );
        let a = Annot::parse(&out);
        // 1793102400 is 2026-10-27 12:00 UTC; at -0300 it is still the 27th
        assert_eq!(a.lines, vec![Some("27/10/2026  Maria".to_string()), Some("27/10/2026  Maria".to_string()), None]);
        assert_eq!(a.widest, "27/10/2026  Maria");
        // the commit of a line, and what the pointer over it shows (the time at the commit's zone)
        let (commit, hint) = a.commit_at(1).unwrap();
        assert_eq!((commit.hash.as_str(), commit.subject.as_str()), (sha('a').as_str(), "x"));
        assert_eq!(hint, &format!("commit {}\nAutor: Maria <maria@x>\nData: 27/10/2026 09:00\n\nx", sha('a')));
        assert!(a.commit_at(2).is_none() && a.commit_at(9).is_none());
        // the zone moves the day: 01:00 UTC on the 1st of March is the last day of February at -0300 (2024, a leap year)
        assert_eq!(date(1_709_254_800, "-0300"), "29/02/2024");
        assert_eq!(date(1_709_254_800, "+0000"), "01/03/2024");
        assert_eq!(date(0, "+0000"), "01/01/1970");
    }
}
