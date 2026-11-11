//! The History panel: the commit log with a simple graph (a lane per line of history), a commit's
//! files and its diff (the Commit window's diff, read-only), the merge in progress with its
//! conflicting files, and the stash (stash.rs).

use super::branches::{Ask, GitOp};
use super::diffbase::rev_text;
use super::git::{Change, Diff, Status, git};
use super::*;

pub struct Commit {
    pub hash: String,
    pub parents: Vec<String>,
    pub author: String,
    pub date: String,
    /// `HEAD -> main`, `origin/main`, `tag: v1`…
    pub refs: Vec<String>,
    pub subject: String,
}

pub fn short(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

/// A lane of the graph: the commit it waits for and its color.
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    pub hash: String,
    pub color: usize,
}

/// One commit's row of the graph: its lane, the lanes coming in from above and leaving below, the
/// lanes that end in it (`merged`) and the ones its extra parents open (`branched`).
#[derive(Debug, Default, PartialEq)]
pub struct GraphRow {
    pub lane: usize,
    pub color: usize,
    pub incoming: Vec<Option<Lane>>,
    pub outgoing: Vec<Option<Lane>>,
    pub merged: Vec<usize>,
    pub branched: Vec<usize>,
}

pub const LANE_COLORS: usize = 7;

/// The graph rows of `commits` (newest first, parents after their children) and how many lanes
/// the widest row has.
pub fn graph(commits: &[Commit]) -> (Vec<GraphRow>, usize) {
    let mut active: Vec<Option<Lane>> = vec![];
    let (mut rows, mut color, mut widest) = (vec![], 0, 1);
    let mut next_color = || {
        color += 1;
        color - 1
    };
    for c in commits {
        let incoming = active.clone();
        let waiting: Vec<usize> = (0..active.len()).filter(|&i| active[i].as_ref().is_some_and(|l| l.hash == c.hash)).collect();
        let (lane, lane_color) = match waiting.first() {
            Some(&i) => (i, active[i].as_ref().map_or(0, |l| l.color)),
            None => (free(&mut active), next_color()),
        };
        let merged: Vec<usize> = waiting.iter().copied().filter(|&i| i != lane).collect();
        for &i in &merged {
            active[i] = None;
        }
        active[lane] = c.parents.first().map(|p| Lane { hash: p.clone(), color: lane_color });
        let mut branched = vec![];
        for p in c.parents.iter().skip(1) {
            let slot = free(&mut active);
            active[slot] = Some(Lane { hash: p.clone(), color: next_color() });
            branched.push(slot);
        }
        while active.last().is_some_and(Option::is_none) {
            active.pop();
        }
        widest = widest.max(incoming.len()).max(active.len());
        rows.push(GraphRow { lane, color: lane_color % LANE_COLORS, incoming, outgoing: active.clone(), merged, branched });
    }
    (rows, widest)
}

/// The first empty lane, or a new one.
fn free(active: &mut Vec<Option<Lane>>) -> usize {
    active.iter().position(Option::is_none).unwrap_or_else(|| {
        active.push(None);
        active.len() - 1
    })
}

/// `git log` output in `FORMAT` -> commits.
pub fn parse_log(out: &str) -> Vec<Commit> {
    out.split('\x1e')
        .filter_map(|r| {
            let f: Vec<&str> = r.trim_matches('\n').split('\x1f').collect();
            let [hash, parents, author, date, refs, subject] = f[..] else { return None };
            Some(Commit {
                hash: hash.into(),
                parents: parents.split_whitespace().map(String::from).collect(),
                author: author.into(),
                date: date.into(),
                refs: refs.split(", ").filter(|r| !r.is_empty()).map(String::from).collect(),
                subject: subject.into(),
            })
        })
        .collect()
}

/// `git log` with `args` after the fixed ones; a repository without commits has none.
pub fn read_commits(root: &Path, args: &[&str]) -> Result<Vec<Commit>, String> {
    let mut a = vec!["log", "--date=format-local:%d/%m/%Y %H:%M", "--format=%H%x1f%P%x1f%an%x1f%ad%x1f%D%x1f%s%x1e"];
    a.extend(args);
    match git(root, &a) {
        Ok(out) => Ok(parse_log(&out)),
        Err(e) if e.contains("does not have any commits") => Ok(vec![]),
        Err(e) => Err(e),
    }
}

/// Every branch, newest first by topology, without the stash's own commits.
fn read_all(root: &Path, limit: usize) -> Result<Vec<Commit>, String> {
    read_commits(root, &["--topo-order", "--exclude=refs/stash", "--all", "-n", &limit.to_string()])
}

/// The files `commit` changed (against its first parent; a root commit against nothing).
pub fn commit_files(root: &Path, commit: &Commit) -> Result<Vec<(Status, String)>, String> {
    let out = match commit.parents.first() {
        Some(p) => git(root, &["diff", "--name-status", "--no-renames", "-z", p, &commit.hash])?,
        None => git(root, &["diff-tree", "-r", "--root", "--no-commit-id", "--name-status", "--no-renames", "-z", &commit.hash])?,
    };
    let mut it = out.split('\0');
    let mut v = vec![];
    while let (Some(st), Some(path)) = (it.next(), it.next()) {
        let status = match st.as_bytes().first() {
            Some(b'A') => Status::Added,
            Some(b'D') => Status::Deleted,
            _ => Status::Modified,
        };
        v.push((status, path.to_string()));
    }
    Ok(v)
}

/// The commit `rev` names, with its parents.
pub fn find_commit(root: &Path, rev: &str) -> Result<Commit, String> {
    if rev.is_empty() || rev.starts_with('-') {
        return Err(format!("«{rev}» não é um commit"));
    }
    let hash = git(root, &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])
        .map_err(|_| format!("«{rev}» não é um ramo, uma etiqueta nem um commit deste repositório"))?;
    read_commits(root, &["-n", "1", hash.trim(), "--"])?.into_iter().next().ok_or_else(|| format!("«{rev}» não tem commit"))
}

/// The MCP's `log`: the newest commits of every branch, a line each.
pub fn agent_log(root: &Path, limit: usize) -> Result<String, String> {
    let commits = read_all(root, limit)?;
    if commits.is_empty() {
        return Ok("Nenhum commit ainda.".into());
    }
    Ok(commits
        .iter()
        .map(|c| {
            let refs = if c.refs.is_empty() { String::new() } else { format!(" [{}]", c.refs.join(", ")) };
            format!("{} {} {} {}{refs}", short(&c.hash), c.date, c.author, c.subject)
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

enum Loaded {
    Log(Result<Vec<Commit>, String>),
    Files(String, Result<Vec<(Status, String)>, String>),
}

pub struct History {
    pub commits: Vec<Commit>,
    pub rows: Vec<GraphRow>,
    pub lanes: usize,
    /// how many commits the log asks for; "Carregar mais" raises it
    pub limit: usize,
    pub selected: Option<String>,
    /// the selected commit's files
    pub files: Vec<(Status, String)>,
    pub error: String,
    rx: Option<Receiver<Loaded>>,
    files_rx: Option<Receiver<Loaded>>,
}

impl Default for History {
    fn default() -> Self {
        History { commits: vec![], rows: vec![], lanes: 1, limit: PAGE, selected: None, files: vec![], error: String::new(), rx: None, files_rx: None }
    }
}

const PAGE: usize = 200;

impl History {
    #[cfg(test)]
    pub fn loading(&self) -> bool {
        self.rx.is_some() || self.files_rx.is_some()
    }
}

const ROW_H: f32 = 40.0;
const LANE_W: f32 = 12.0;

fn lane_color(i: usize) -> Color32 {
    const C: [u32; LANE_COLORS] = [0x3871E1, 0x73BD79, 0xCF8E6D, 0xC77DBB, 0x2AACB8, 0xB3AE60, 0xE88F89];
    let v = C[i % LANE_COLORS];
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

impl Ide {
    /// Shows the panel and loads the log.
    pub(crate) fn open_history(&mut self) {
        self.left = Some(Left::History);
        self.history_load();
    }

    pub(crate) fn toggle_history(&mut self) {
        if self.left == Some(Left::History) {
            self.left = None;
        } else {
            self.open_history();
        }
    }

    /// Reads the log on a thread; the answer is `history_poll`'s.
    pub(crate) fn history_load(&mut self) {
        let Some(root) = self.root.clone().filter(|_| self.git.repo) else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let (ctx, limit) = (self.ctx.clone(), self.git.history.limit);
        std::thread::spawn(move || {
            let _ = tx.send(Loaded::Log(read_all(&root, limit)));
            ctx.request_repaint();
        });
        self.git.history.rx = Some(rx);
    }

    pub(crate) fn history_poll(&mut self) {
        let h = &mut self.git.history;
        if let Some(Ok(Loaded::Log(res))) = h.rx.as_ref().map(|rx| rx.try_recv()) {
            h.rx = None;
            match res {
                Ok(commits) => {
                    (h.rows, h.lanes) = graph(&commits);
                    (h.commits, h.error) = (commits, String::new());
                }
                Err(e) => h.error = e,
            }
        }
        if let Some(Ok(Loaded::Files(hash, res))) = h.files_rx.as_ref().map(|rx| rx.try_recv()) {
            h.files_rx = None;
            if h.selected.as_deref() == Some(hash.as_str()) {
                match res {
                    Ok(files) => h.files = files,
                    Err(e) => h.error = e,
                }
            }
        }
    }

    /// Picks a commit: its files load on a thread.
    pub(crate) fn history_select(&mut self, hash: &str) {
        let Some(root) = self.root.clone() else { return };
        let Some(commit) = self.git.history.commits.iter().find(|c| c.hash == hash) else { return };
        let (hash, parents) = (commit.hash.clone(), commit.parents.clone());
        self.git.history.selected = Some(hash.clone());
        self.git.history.files.clear();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let commit = Commit { hash: hash.clone(), parents, author: String::new(), date: String::new(), refs: vec![], subject: String::new() };
            let _ = tx.send(Loaded::Files(hash, commit_files(&root, &commit)));
            ctx.request_repaint();
        });
        self.git.history.files_rx = Some(rx);
    }

    /// A commit's change of one file in a diff tab: its first parent on the left, the commit on the
    /// right, both read-only (the Commit window's diff, with no buffer behind it).
    pub(crate) fn open_commit_diff(&mut self, commit: &Commit, rel: &str) -> Result<(), String> {
        let root = self.root.clone().ok_or("o IDE não está numa pasta")?;
        let path = root.join(rel);
        let title = format!("{}: {}", short(&commit.hash), file_name(&path));
        if let Some(i) = self.tabs.iter().position(|t| t.path == path && t.diff.as_ref().is_some_and(|d| d.title.as_deref() == Some(title.as_str()))) {
            self.active = i;
            return Ok(());
        }
        let old = commit.parents.first().and_then(|p| rev_text(&root, p, &path)).unwrap_or_default();
        let new = rev_text(&root, &commit.hash, &path).unwrap_or_default();
        if old.contains('\0') || new.contains('\0') {
            return Err("Arquivo binário: o git não tem texto para comparar.".into());
        }
        let mut diff = Diff::new(&old, &new);
        diff.title = Some(title);
        let left = commit.parents.first().map_or("vazio".to_string(), |p| short(p).to_string());
        diff.sides = Some((format!("{left} (somente leitura)"), format!("{} (somente leitura)", short(&commit.hash))));
        let mut tab = Tab::new(path, String::new(), false);
        tab.diff = Some(Box::new(diff));
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        Ok(())
    }

    /// The MCP's `show`: the panel on the commit `rev`, and its files; with `path`, that file's diff opens.
    pub(crate) fn agent_show(&mut self, rev: &str, path: &str) -> Result<String, String> {
        let root = self.root.clone().ok_or("o IDE não está numa pasta")?;
        let commit = find_commit(&root, rev)?;
        let files = commit_files(&root, &commit)?;
        let rel = path.trim().replace('\\', "/");
        if !rel.is_empty() {
            if !files.iter().any(|(_, f)| *f == rel) {
                return Err(format!("{rel} não mudou em {} (o caminho é relativo à pasta do IDE)", short(&commit.hash)));
            }
            self.open_commit_diff(&commit, &rel)?;
        }
        self.open_history();
        self.git.history.selected = Some(commit.hash.clone());
        self.git.history.files = files.clone();
        let list = files.iter().map(|(s, f)| format!("{} {f}", status_letter(*s))).collect::<Vec<_>>().join("\n");
        Ok(format!("{} {} {}: {}\n{list}", short(&commit.hash), commit.date, commit.author, commit.subject))
    }

    pub(crate) fn history_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.set_height(36.0);
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Histórico").color(t::TEXT).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if icons::button(ui, icons::REFRESH, "Atualizar o histórico", false).clicked() {
                    self.git_refresh();
                    self.history_load();
                }
            });
        });
        if !self.git.repo {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Esta pasta não é um repositório Git").color(t::MUTED));
            });
            return;
        }
        self.merge_banner(ui);
        self.stash_section(ui);
        ui.separator();
        let selected = self.git.history.selected.clone();
        let detail_h = if selected.is_some() { 190.0 } else { 0.0 };
        let list_h = (ui.available_height() - detail_h).max(80.0);
        let mut pick = None;
        egui::ScrollArea::vertical().id_salt("history_list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let h = &self.git.history;
            if !h.error.is_empty() {
                ui.colored_label(t::ERROR, super::branches::explain(&h.error));
            } else if h.commits.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new(if h.rx.is_some() { "Lendo o histórico…" } else { "Nenhum commit ainda." }).color(t::MUTED));
                });
            }
            let remotes = &self.git.snap.remotes;
            for (c, g) in h.commits.iter().zip(&h.rows) {
                let (resp, rect) = row(ui, selected.as_deref() == Some(c.hash.as_str()), ROW_H);
                let resp = named(resp, format!("{}, {}, {}, {}", c.subject, c.author, c.date, short(&c.hash)));
                paint_commit(ui, rect, c, g, h.lanes, remotes);
                if resp.clicked() {
                    pick = Some(c.hash.clone());
                }
            }
            if h.commits.len() >= h.limit {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    if ui.button("Carregar mais commits").clicked() {
                        pick = Some(String::new());
                    }
                });
            }
        });
        match pick {
            Some(hash) if hash.is_empty() => {
                self.git.history.limit += PAGE;
                self.history_load();
            }
            Some(hash) => self.history_select(&hash),
            None => {}
        }
        if let Some(hash) = selected {
            self.commit_detail(ui, &hash);
        }
    }

    /// The selected commit: who, when, and its files (a click opens the diff).
    fn commit_detail(&mut self, ui: &mut Ui, hash: &str) {
        let Some(i) = self.git.history.commits.iter().position(|c| c.hash == hash) else { return };
        ui.separator();
        let c = &self.git.history.commits[i];
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.add(egui::Label::new(egui::RichText::new(&c.subject).color(t::TEXT).strong()).truncate());
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new(format!("{} · {} · {}", c.author, c.date, short(&c.hash))).color(t::SECONDARY).small());
            if ui.small_button("Copiar hash").clicked() {
                ui.ctx().copy_text(c.hash.clone());
            }
        });
        let mut open = None;
        egui::ScrollArea::vertical().id_salt("commit_files").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if self.git.history.files.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new(if self.git.history.files_rx.is_some() {
                            "Lendo os arquivos…"
                        } else {
                            "Este commit não alterou nenhum arquivo."
                        })
                        .color(t::MUTED),
                    );
                });
            }
            for (status, rel) in &self.git.history.files {
                let (resp, rect) = row(ui, false, 22.0);
                let resp = named(resp, format!("{} {rel}", status_letter(*status)));
                let p = ui.painter().with_clip_rect(rect);
                p.text(pos2(rect.left() + 14.0, rect.center().y), Align2::LEFT_CENTER, status_letter(*status), t::ui(t::UI_SIZE), status.color());
                p.text(pos2(rect.left() + 32.0, rect.center().y), Align2::LEFT_CENTER, rel, t::ui(t::UI_SIZE), t::TEXT);
                if resp.on_hover_text("Mostra as alterações deste commit neste arquivo").clicked() {
                    open = Some(rel.clone());
                }
            }
        });
        if let Some(rel) = open {
            let commit = &self.git.history.commits[i];
            let commit = Commit {
                hash: commit.hash.clone(),
                parents: commit.parents.clone(),
                author: String::new(),
                date: String::new(),
                refs: vec![],
                subject: String::new(),
            };
            if let Err(why) = self.open_commit_diff(&commit, &rel) {
                self.notify(why);
            }
        }
    }

    /// While a merge waits: its conflicting files (a click opens one in the editor) and the way out.
    fn merge_banner(&mut self, ui: &mut Ui) {
        if !self.git.snap.merging {
            return;
        }
        let conflicts: Vec<PathBuf> = self.git.changes.iter().filter(|c: &&Change| c.status == Status::Conflict).map(|c| c.path.clone()).collect();
        let root = self.root.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Merge em andamento").color(t::ERROR).strong());
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let text = if conflicts.is_empty() {
                "Sem conflitos: falta concluir o merge.".to_string()
            } else {
                format!("{} em conflito: resolva no editor.", conflicts.len())
            };
            ui.label(egui::RichText::new(text).color(t::MUTED));
        });
        let mut open = None;
        for path in &conflicts {
            let (resp, rect) = row(ui, false, 22.0);
            let rel = path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/");
            let resp = named(resp, format!("Conflito em {rel}"));
            let p = ui.painter().with_clip_rect(rect);
            p.text(pos2(rect.left() + 14.0, rect.center().y), Align2::LEFT_CENTER, "!", t::ui(t::UI_SIZE), Status::Conflict.color());
            p.text(pos2(rect.left() + 32.0, rect.center().y), Align2::LEFT_CENTER, &rel, t::ui(t::UI_SIZE), t::TEXT);
            if resp.clicked() {
                open = Some(path.clone());
            }
        }
        if let Some(path) = open {
            self.open_file(path, None);
        }
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            if ui.button("Concluir merge").on_hover_text("Prepara os arquivos resolvidos e faz o commit do merge").clicked() {
                self.git_op_or_say(GitOp::MergeFinish);
            }
            if ui.button("Interromper merge").clicked() {
                self.git.ask = Some(Ask::MergeAbort);
            }
        });
        ui.separator();
    }
}

fn status_letter(s: Status) -> &'static str {
    match s {
        Status::Added | Status::Untracked => "A",
        Status::Deleted => "D",
        Status::Conflict => "!",
        _ => "M",
    }
}

/// One commit's row: its piece of the graph, ref chips and subject, then author, date and hash.
fn paint_commit(ui: &Ui, rect: Rect, c: &Commit, g: &GraphRow, lanes: usize, remotes: &[String]) {
    let p = ui.painter().with_clip_rect(rect);
    let shown = lanes.min(8);
    let x = |lane: usize| rect.left() + 14.0 + lane.min(7) as f32 * LANE_W;
    let (top, mid, bottom) = (rect.top(), rect.center().y, rect.bottom());
    let line = |a: egui::Pos2, b: egui::Pos2, color: usize| p.line_segment([a, b], Stroke::new(1.5, lane_color(color)));
    for (i, lane) in g.incoming.iter().enumerate() {
        let Some(lane) = lane else { continue };
        if lane.hash == c.hash {
            line(pos2(x(i), top), pos2(x(g.lane), mid), lane.color);
        } else {
            line(pos2(x(i), top), pos2(x(i), bottom), lane.color);
        }
    }
    if let Some(next) = g.outgoing.get(g.lane).and_then(Option::as_ref).filter(|_| !c.parents.is_empty()) {
        line(pos2(x(g.lane), mid), pos2(x(g.lane), bottom), next.color);
    }
    for &s in &g.branched {
        if let Some(l) = g.outgoing.get(s).and_then(Option::as_ref) {
            line(pos2(x(g.lane), mid), pos2(x(s), bottom), l.color);
        }
    }
    p.circle_filled(pos2(x(g.lane), mid), 4.0, lane_color(g.color));
    if c.parents.len() > 1 {
        p.circle_filled(pos2(x(g.lane), mid), 2.0, t::ISLAND);
    }
    let mut tx = rect.left() + 14.0 + shown as f32 * LANE_W + 6.0;
    let y1 = rect.top() + 12.0;
    for r in c.refs.iter().take(2) {
        let (text, tint) = match r.strip_prefix("HEAD -> ") {
            Some(b) => (b, t::ACCENT),
            None if r.starts_with("tag: ") => (&r[5..], t::METADATA),
            None if remotes.iter().any(|m| r.starts_with(&format!("{m}/"))) => (r.as_str(), t::MUTED),
            None => (r.as_str(), lane_color(1)),
        };
        let galley = p.layout_no_wrap(text.to_string(), t::ui(t::UI_SIZE - 2.0), tint);
        let chip = Rect::from_min_size(pos2(tx, y1 - 8.0), vec2(galley.size().x + 10.0, 16.0));
        p.rect_filled(chip, 4, tint.gamma_multiply(0.18));
        p.galley(pos2(chip.left() + 5.0, chip.center().y - galley.size().y / 2.0), galley, tint);
        tx = chip.right() + 4.0;
    }
    p.text(pos2(tx, y1), Align2::LEFT_CENTER, &c.subject, t::ui(t::UI_SIZE), t::TEXT);
    p.text(
        pos2(rect.left() + 14.0 + shown as f32 * LANE_W + 6.0, rect.top() + 28.0),
        Align2::LEFT_CENTER,
        format!("{} · {} · {}", c.author, c.date, short(&c.hash)),
        t::ui(t::UI_SIZE - 2.0),
        t::SECONDARY,
    );
}

#[cfg(test)]
mod tests {
    use super::gittest::*;
    use super::*;

    fn c(hash: &str, parents: &[&str]) -> Commit {
        Commit {
            hash: hash.into(),
            parents: parents.iter().map(|p| p.to_string()).collect(),
            author: String::new(),
            date: String::new(),
            refs: vec![],
            subject: String::new(),
        }
    }

    #[test]
    fn the_graph_gives_a_branch_its_own_lane_and_closes_it_in_the_merge() {
        // m merges b into a; b branched from base
        let commits = [c("m", &["a", "b"]), c("b", &["base"]), c("a", &["base"]), c("base", &[])];
        let (rows, lanes) = graph(&commits);
        assert_eq!(lanes, 2);
        // the merge: lane 0, opens lane 1 for its second parent
        assert_eq!((rows[0].lane, rows[0].branched.clone()), (0, vec![1]));
        // b sits in lane 1, a in lane 0, both waiting for base
        assert_eq!((rows[1].lane, rows[2].lane), (1, 0));
        // base closes the lane that came from b into lane 0
        assert_eq!((rows[3].lane, rows[3].merged.clone()), (0, vec![1]));
        assert!(rows[3].outgoing.is_empty(), "the root ends every lane");
        // a straight history stays in one lane
        let (rows, lanes) = graph(&[c("3", &["2"]), c("2", &["1"]), c("1", &[])]);
        assert_eq!((lanes, rows.iter().map(|r| r.lane).collect::<Vec<_>>()), (1, vec![0, 0, 0]));
    }

    #[test]
    fn the_log_of_two_branches_lists_them_with_graph_files_and_diff() {
        let dir = repo("log");
        let mut ide = ide(&dir);
        git(&dir, &["checkout", "-q", "-b", "lado"]).unwrap();
        std::fs::write(dir.join("a.txt"), "um\nLADO\ntres\n").unwrap();
        std::fs::write(dir.join("novo.txt"), "n\n").unwrap();
        commit(&dir, "no lado");
        git(&dir, &["checkout", "-q", "main"]).unwrap();
        std::fs::write(dir.join("m.txt"), "m\n").unwrap();
        commit(&dir, "na main");
        git(&dir, &["tag", "v1"]).unwrap();
        ide.open_history();
        settle(&mut ide);
        let h = &ide.git.history;
        let subjects: Vec<&str> = h.commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects.len(), 3);
        assert!(subjects.contains(&"no lado") && subjects.contains(&"na main") && subjects.contains(&"base"), "{subjects:?}");
        assert_eq!(h.lanes, 2, "two lines of history side by side");
        let side = h.commits.iter().find(|c| c.subject == "no lado").unwrap();
        assert!(side.refs.iter().any(|r| r == "lado") && side.author == "Teste" && side.date.contains('/'), "{:?}", side.refs);
        assert!(h.commits.iter().find(|c| c.subject == "na main").unwrap().refs.iter().any(|r| r.contains("tag: v1")));
        // a commit's files (against its first parent), and its diff in a read-only tab
        let hash = side.hash.clone();
        ide.history_select(&hash);
        settle(&mut ide);
        let mut files: Vec<_> = ide.git.history.files.iter().map(|(s, f)| (*s, f.clone())).collect();
        files.sort_by(|a, b| a.1.cmp(&b.1));
        assert_eq!(files, vec![(Status::Modified, "a.txt".to_string()), (Status::Added, "novo.txt".to_string())]);
        let commit = ide.git.history.commits.iter().find(|c| c.hash == hash).unwrap();
        let commit = Commit { hash: commit.hash.clone(), parents: commit.parents.clone(), ..c("", &[]) };
        ide.open_commit_diff(&commit, "a.txt").unwrap();
        let tab = &ide.tabs[ide.active];
        let d = tab.diff.as_ref().unwrap();
        assert!(!d.is_plain() && d.old.contains(&"dois".to_string()) && d.new.contains(&"LADO".to_string()));
        assert!(d.sides.as_ref().unwrap().0.ends_with("(somente leitura)"));
        let tabs = ide.tabs.len();
        ide.open_commit_diff(&commit, "a.txt").unwrap();
        assert_eq!(ide.tabs.len(), tabs, "the same commit's diff opens once");
        // a root commit's files are all additions; the working copy's own diff is another tab
        let root = find_commit(&dir, "main~1").unwrap();
        let first = find_commit(&dir, &format!("{}~1", root.hash)).unwrap_or(root);
        assert!(!commit_files(&dir, &first).unwrap().is_empty());
        ide.open_diff(dir.join("a.txt"));
        assert!(ide.tabs[ide.active].diff.as_ref().unwrap().is_plain());
        // the MCP's reads
        let log = agent_log(&dir, 10).unwrap();
        assert!(log.contains("no lado") && log.contains("tag: v1") && log.lines().count() == 3, "{log}");
        let shown = ide.agent_show(&hash[..8], "novo.txt").unwrap();
        assert!(shown.contains("no lado") && shown.contains("A novo.txt"), "{shown}");
        assert!(ide.agent_show("nao-existe", "").is_err() && ide.agent_show("--all", "").is_err());
        assert!(ide.agent_show(&hash, "m.txt").unwrap_err().contains("não mudou"));
        drop(ide);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_repository_without_commits_has_an_empty_history() {
        let dir = std::env::temp_dir().join(format!("magoagent-history-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q"]).unwrap();
        assert!(read_all(&dir, 10).unwrap().is_empty());
        assert_eq!(agent_log(&dir, 10).unwrap(), "Nenhum commit ainda.");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
