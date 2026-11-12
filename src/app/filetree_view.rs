//! A file tree node's live view: its folders as read (off the UI thread), the git state, and the
//! rows they make; what the node shows between frames.

use crate::app::filetree::*;
use crate::ide::lsp::path_key;
use crate::ide::{Status, parse_status};
use crate::prelude::*;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};

/// Entries shown per folder; the rest is counted ("Exibindo um número limitado de itens").
const MAX_ENTRIES: usize = 2000;
/// Filename search hits shown.
const MAX_HITS: usize = 200;
/// Seconds between reads of the open folders and the git state.
const POLL: f64 = 2.0;

/// One thing in a folder.
#[derive(Clone, Debug)]
pub(crate) struct TreeEntry {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) dir: bool,
    size: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
}

/// A folder as read: still loading, its entries, or why it couldn't be read.
pub(crate) enum Listing {
    Loading,
    Ready(Vec<TreeEntry>),
    Failed(String),
}

/// The folder's entries as the disk gives them.
pub(crate) fn list_dir(path: &Path) -> Listing {
    let read = match fs::read_dir(path) {
        Ok(read) => read,
        Err(e) => return Listing::Failed(e.to_string()),
    };
    Listing::Ready(
        read.flatten()
            .map(|e| {
                let path = e.path();
                let kind = e.file_type().ok();
                // a link to a folder is a folder
                let dir = kind.is_some_and(|k| k.is_dir() || (k.is_symlink() && path.is_dir()));
                let meta = e.metadata().ok();
                TreeEntry {
                    name: e.file_name().to_string_lossy().into_owned(),
                    dir,
                    size: if dir { 0 } else { meta.as_ref().map_or(0, |m| m.len()) },
                    modified: meta.as_ref().and_then(|m| m.modified().ok()),
                    created: meta.as_ref().and_then(|m| m.created().ok()),
                    path,
                }
            })
            .collect(),
    )
}

/// "file2" before "file10", letters without regard to case.
fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let number = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut n = String::new();
                    while let Some(d) = it.next_if(char::is_ascii_digit) {
                        n.push(d);
                    }
                    let n = n.trim_start_matches('0').to_string();
                    (n.len(), n)
                };
                match number(&mut a).cmp(&number(&mut b)) {
                    Ordering::Equal => {}
                    other => return other,
                }
            }
            (Some(x), Some(y)) => {
                match x.to_lowercase().cmp(y.to_lowercase()) {
                    Ordering::Equal => {}
                    other => return other,
                }
                a.next();
                b.next();
            }
        }
    }
}

fn extension(name: &str) -> String {
    name.rfind('.').filter(|i| *i > 0).map_or(String::new(), |i| name[i + 1..].to_lowercase())
}

/// The reference app's `sortEntries`: folders first, then by `spec.sort`; `reversed` flips each group.
fn order(a: &TreeEntry, b: &TreeEntry, spec: &FileTreeSpec) -> Ordering {
    if a.dir != b.dir {
        return b.dir.cmp(&a.dir);
    }
    let by_name = natural(&a.name, &b.name);
    let up = match spec.sort {
        SortBy::Name => by_name,
        SortBy::Kind => extension(&a.name).cmp(&extension(&b.name)).then(by_name),
        // newest and biggest first
        SortBy::DateModified => b.modified.cmp(&a.modified).then(by_name),
        SortBy::DateCreated => b.created.or(b.modified).cmp(&a.created.or(a.modified)).then(by_name),
        SortBy::Size => b.size.cmp(&a.size).then(by_name),
    };
    if spec.reversed { up.reverse() } else { up }
}

/// What shows of a folder: hidden ones left out if asked, ordered, at most `MAX_ENTRIES`, and how many were cut.
fn visible<'a>(entries: &'a [TreeEntry], spec: &FileTreeSpec) -> (Vec<&'a TreeEntry>, usize) {
    let mut shown: Vec<&TreeEntry> = entries.iter().filter(|e| !(spec.hide_hidden && e.name.starts_with('.'))).collect();
    shown.sort_by(|a, b| order(a, b, spec));
    let cut = shown.len().saturating_sub(MAX_ENTRIES);
    shown.truncate(MAX_ENTRIES);
    (shown, cut)
}

/// The git state of the tree's folder: the branch and a mark per changed path.
#[derive(Clone, Default)]
pub(crate) struct TreeGit {
    pub(crate) repo: bool,
    pub(crate) branch: String,
    pub(crate) changes: usize,
    statuses: HashMap<String, Status>,
    /// Every changed file against the HEAD, as `git status` lists them (the Alterações panel).
    pub(crate) files: Vec<(PathBuf, Status)>,
    /// What each folder holds, the most pressing mark first (the reference app's roll-up).
    folders: HashMap<String, Status>,
    /// Folders git lists whole (`?? dir/`): all they hold is untracked.
    untracked: HashSet<String>,
    /// What git ignores (`!! path`): a file, or a folder and all it holds.
    ignored: HashSet<String>,
}

/// Lower is more pressing.
fn rank(s: Status) -> u8 {
    match s {
        Status::Conflict => 0,
        Status::Deleted => 1,
        Status::Modified => 2,
        Status::Renamed => 3,
        Status::Added => 4,
        Status::Untracked => 5,
    }
}

impl TreeGit {
    /// `git status` of `root` (None when it isn't in a repository).
    pub(crate) fn read(root: &Path) -> TreeGit {
        let git = crate::ide::run_git;
        // where `root` sits in its repository: git's paths are all from the repository's top
        let Ok(prefix) = git(root, &["rev-parse", "--show-prefix"]) else { return TreeGit::default() };
        let prefix = prefix.trim().to_string();
        let branch = git(root, &["symbolic-ref", "--short", "-q", "HEAD"])
            .or_else(|_| git(root, &["rev-parse", "--short", "HEAD"]))
            .map_or(String::new(), |b| b.trim().to_string());
        // every untracked file on its own, as the reference app counts: a new folder is its files
        // and what it ignores, a folder as one line (`matching`; a git before 2.16 has no such mode, and grays nothing)
        let status = ["--no-optional-locks", "status", "--porcelain=v1", "-z", "--untracked-files=all"];
        let out = git(root, &[&status[..], &["--ignored=matching"]].concat()).or_else(|_| git(root, &status)).unwrap_or_default();
        // the ignored come last (`!! path`), and are not changes
        // ponytail: a rename from a file named "!! …" would cut the list there; walk the entries if that ever shows
        let (out, ignored) = match out.find("\0!! ") {
            Some(i) => out.split_at(i + 1),
            None if out.starts_with("!! ") => ("", out.as_str()),
            None => (out.as_str(), ""),
        };
        let root_key = path_key(root);
        let mut info = TreeGit { repo: true, branch, ..Default::default() };
        let inside = |e: &str| Path::new(e).strip_prefix(&prefix).ok().map(|p| path_key(&root.join(p)).trim_end_matches('/').to_string());
        info.ignored = ignored.split('\0').filter_map(|e| e.strip_prefix("!! ")).filter_map(inside).collect();
        for change in parse_status(Path::new(""), out) {
            let Ok(inside) = change.path.strip_prefix(&prefix) else { continue };
            let full = root.join(inside);
            let key = path_key(&full).trim_end_matches('/').to_string();
            if path_key(&change.path).ends_with('/') {
                info.untracked.insert(key.clone());
            }
            let mut up = full.parent();
            while let Some(dir) = up {
                let dir_key = path_key(dir);
                if !dir_key.starts_with(&root_key) || dir_key.len() < root_key.len() {
                    break;
                }
                let best = info.folders.entry(dir_key).or_insert(change.status);
                if rank(change.status) < rank(*best) {
                    *best = change.status;
                }
                up = dir.parent();
            }
            info.files.push((full, change.status));
            info.statuses.insert(key, change.status);
        }
        info.changes = info.statuses.len();
        info
    }

    /// The mark of `path` and whether it is only rolled up from what a folder holds.
    pub(crate) fn of(&self, path: &Path, dir: bool) -> Option<(Status, bool)> {
        if !self.repo {
            return None;
        }
        let key = path_key(path);
        if let Some(s) = self.statuses.get(&key) {
            return Some((*s, false));
        }
        if let Some(s) = self.folders.get(&key).filter(|_| dir) {
            return Some((*s, true));
        }
        let mut up = path.parent();
        while let Some(parent) = up {
            if self.untracked.contains(&path_key(parent)) {
                return Some((Status::Untracked, dir));
            }
            up = parent.parent();
        }
        None
    }

    /// Git ignores `path`: itself, or a folder it is in.
    pub(crate) fn ignored(&self, path: &Path) -> bool {
        !self.ignored.is_empty() && path.ancestors().any(|a| self.ignored.contains(&path_key(a)))
    }
}

/// A row on screen.
#[derive(Clone, Debug)]
pub(crate) struct TreeRow {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) dir: bool,
    pub(crate) depth: usize,
    pub(crate) open: bool,
    /// A filename search hit's folder, shown after the name.
    pub(crate) hint: String,
}

/// What the readers send back.
enum Msg {
    Dir(PathBuf, Listing),
    Git(TreeGit),
    Files(PathBuf, Vec<PathBuf>),
    Done,
}

/// One node's live tree.
pub(crate) struct TreeView {
    pub(crate) listings: HashMap<PathBuf, Listing>,
    pub(crate) expanded: BTreeSet<PathBuf>,
    pub(crate) selected: Option<PathBuf>,
    /// How far the rows are scrolled, in the node's own points.
    pub(crate) scroll: f32,
    /// Where "Voltar" came from, for "Avançar".
    pub(crate) forward: Vec<PathBuf>,
    pub(crate) filter: String,
    pub(crate) git: TreeGit,
    /// The files with an error and the folders over them (filetree_problems.rs): their names are red and wavy.
    pub(crate) problems: std::collections::HashSet<PathBuf>,
    pub(crate) rows: Vec<TreeRow>,
    /// How many entries the 2000-a-folder cap left out.
    pub(crate) dropped: usize,
    /// The local branches, read when the branch chip is clicked.
    pub(crate) branches: Vec<String>,
    /// The rows the tree has, searching or not (the search bar's "exibidos").
    pub(crate) tree_rows: usize,
    /// The filename search found more than it shows.
    pub(crate) hits_cut: bool,
    /// The last key was the keyboard's: the row in focus wears its ring.
    pub(crate) kb: bool,
    /// A row to bring into view once it is listed (a few frames to be found).
    pub(crate) reveal: Option<(PathBuf, u8)>,
    /// The rows' height on screen, in the node's own points (set when drawn).
    pub(crate) body_h: f32,
    /// The files under the shown folder, for the filename search.
    index: Option<(PathBuf, Vec<PathBuf>)>,
    indexing: bool,
    pub(crate) dirty: bool,
    /// Read the open folders again now, not at the next poll.
    pub(crate) stale: bool,
    busy: bool,
    last_poll: f64,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl TreeView {
    pub(crate) fn new() -> Self {
        let (tx, rx) = channel();
        TreeView {
            listings: HashMap::new(),
            expanded: BTreeSet::new(),
            selected: None,
            scroll: 0.0,
            forward: Vec::new(),
            filter: String::new(),
            git: TreeGit::default(),
            problems: Default::default(),
            rows: Vec::new(),
            dropped: 0,
            branches: Vec::new(),
            tree_rows: 0,
            hits_cut: false,
            kb: false,
            reveal: None,
            body_h: 0.0,
            index: None,
            indexing: false,
            dirty: true,
            stale: true,
            busy: false,
            last_poll: f64::NEG_INFINITY,
            tx,
            rx,
        }
    }

    /// Reads the shown folder and the open ones, and git, off the UI thread.
    fn refresh(&mut self, spec: &FileTreeSpec, ctx: &Context) {
        let shown = spec.shown();
        let mut dirs: Vec<PathBuf> = vec![shown.clone()];
        dirs.extend(self.expanded.iter().filter(|p| p.starts_with(&shown)).cloned());
        for dir in &dirs {
            self.listings.entry(dir.clone()).or_insert(Listing::Loading);
        }
        (self.busy, self.stale) = (true, false);
        let (tx, ctx, root) = (self.tx.clone(), ctx.clone(), spec.root_path());
        std::thread::spawn(move || {
            for dir in dirs {
                let _ = tx.send(Msg::Dir(dir.clone(), list_dir(&dir)));
                ctx.request_repaint();
            }
            let _ = tx.send(Msg::Git(TreeGit::read(&root)));
            let _ = tx.send(Msg::Done);
            ctx.request_repaint();
        });
    }

    /// Takes in what the readers sent.
    pub(crate) fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.dirty = true;
            match msg {
                Msg::Dir(path, listing) => {
                    self.listings.insert(path, listing);
                }
                Msg::Git(git) => self.git = git,
                Msg::Files(base, files) => (self.index, self.indexing) = (Some((base, files)), false),
                Msg::Done => self.busy = false,
            }
        }
    }

    /// Each frame: takes in the readers' news and, every `POLL` seconds (or at once when stale),
    /// starts reading again.
    pub(crate) fn poll(&mut self, spec: &FileTreeSpec, ctx: &Context, now: f64) {
        self.drain();
        if !self.busy && (self.stale || now - self.last_poll >= POLL) {
            self.last_poll = now;
            self.refresh(spec, ctx);
        }
        // the filename search wants the files under the folder shown
        let shown = spec.shown();
        if self.filter.trim().is_empty() {
            self.index = None;
        } else if !self.indexing && self.index.as_ref().is_none_or(|(base, _)| *base != shown) {
            self.indexing = true;
            let (tx, ctx) = (self.tx.clone(), ctx.clone());
            std::thread::spawn(move || {
                let files = crate::ide::tree::list_files(&shown);
                let _ = tx.send(Msg::Files(shown, files));
                ctx.request_repaint();
            });
        }
    }

    /// Reads the files under `shown` now, for a search an agent asked for.
    pub(crate) fn read_index_now(&mut self, shown: &Path) {
        self.index = Some((shown.to_path_buf(), crate::ide::tree::list_files(shown)));
        self.dirty = true;
    }

    /// Reads one folder now (an agent asked, or the rows can't wait).
    pub(crate) fn read_now(&mut self, dir: &Path) {
        self.listings.insert(dir.to_path_buf(), list_dir(dir));
        self.dirty = true;
    }

    /// Rows again from what is listed and open.
    pub(crate) fn rebuild(&mut self, spec: &FileTreeSpec) {
        self.dirty = false;
        self.rows.clear();
        self.dropped = 0;
        let shown = spec.shown();
        let query = self.filter.trim().to_lowercase();
        let (mut tree, mut dropped) = (Vec::new(), 0);
        self.walk(&shown, 0, spec, &mut tree, &mut dropped);
        self.tree_rows = tree.len();
        self.hits_cut = false;
        if !query.is_empty() {
            let files = self.index.iter().flat_map(|(_, files)| files);
            let mut hits = files.filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&query)));
            for path in hits.by_ref().take(MAX_HITS) {
                let hint = path.parent().and_then(|p| p.strip_prefix(&shown).ok()).map_or(String::new(), |p| p.to_string_lossy().replace('\\', "/"));
                let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
                self.rows.push(TreeRow { path: path.clone(), name, dir: false, depth: 0, open: false, hint });
            }
            self.hits_cut = hits.next().is_some();
            return;
        }
        (self.rows, self.dropped) = (tree, dropped);
    }

    fn walk(&self, dir: &Path, depth: usize, spec: &FileTreeSpec, rows: &mut Vec<TreeRow>, dropped: &mut usize) {
        let Some(Listing::Ready(entries)) = self.listings.get(dir) else { return };
        let (shown, cut) = visible(entries, spec);
        *dropped += cut;
        for e in shown {
            let open = e.dir && self.expanded.contains(&e.path);
            rows.push(TreeRow { path: e.path.clone(), name: e.name.clone(), dir: e.dir, depth, open, hint: String::new() });
            if open {
                self.walk(&e.path, depth + 1, spec, rows, dropped);
            }
        }
    }

    /// Why the list is empty (or None when it isn't): the words the empty tree shows.
    pub(crate) fn empty_note(&self, spec: &FileTreeSpec) -> Option<String> {
        if !self.rows.is_empty() {
            return None;
        }
        if !self.filter.trim().is_empty() {
            return Some(if self.index.is_none() { "Buscando...".into() } else { "Nenhuma correspondência".into() });
        }
        Some(match self.listings.get(&spec.shown()) {
            Some(Listing::Failed(why)) => format!("Não foi possível ler esta pasta. {why}"),
            Some(Listing::Ready(_)) => "Esta pasta está vazia.".into(),
            _ => "Lendo esta pasta…".into(),
        })
    }
}
