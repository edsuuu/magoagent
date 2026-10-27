//! The IDE node's editor: a port of Ilha, the user's PhpStorm-like IDE. One `Ide` per IDE node,
//! alive while the app runs; the canvas glue (app/ide.rs) creates it, runs `background` every
//! frame and shows `ui` while the node is enlarged.
//! Ilha's terminal and Claude Code tool windows are the canvas terminals roped to the node.

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, Frame, Id, Key, KeyboardShortcut, Margin, Modifiers, Order, Rect,
    Sense, Shape, Stroke, TextEdit, Ui, pos2, text::CCursor, text::LayoutJob, text_selection::CCursorRange, vec2,
};
use chrome::{menu_item, row, tool_header};
use highlight::Lang;
use lsp::path_key;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant, SystemTime};
use text::*;
use theme as t;
use tree::TreeNode;
pub(crate) use fileops::{FILE_MENU, FileAction, FileAsk, file_menu};

mod chrome;
pub(crate) mod card;
mod claude;
mod commit;
mod db;
mod docs;
mod editor;
mod fileops;
mod find;
mod git;
mod highlight;
mod hints;
pub(crate) mod icons;
mod index;
mod inspection;
mod langs;
mod lsp;
mod search;
mod text;
pub(crate) mod theme;
mod tree;

pub(crate) use find::project_search;

/// `Command` that never flashes a console window on Windows.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[cfg_attr(not(windows), expect(unused_mut))]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

#[derive(Clone, Copy, PartialEq)]
enum Left {
    Project,
    Commit,
    Database,
}

#[derive(Clone, Copy, PartialEq)]
enum ToolWindow {
    Problems,
    Find,
}

struct Tab {
    path: PathBuf,
    text: String,
    lang: &'static Lang,
    crlf: bool,
    dirty: bool,
    mtime: Option<SystemTime>,
    /// Its file went away on disk (the 2 s check): the tab stays, marked, and saving writes it back.
    gone: bool,
    cache: Option<(u64, LayoutJob)>,
    cursor: (usize, usize), // 0-based line, col
    sel: (usize, usize),    // char range
    select: Option<(usize, usize)>,
    scroll_to: Option<usize>,
    lsp_pending: Option<Instant>,
    /// set for a diff tab (HEAD vs working copy)
    diff: Option<Box<git::Diff>>,
    /// set for a table's data or an SQL console (`path` is the database)
    sheet: Option<Box<db::Sheet>>,
    /// HEAD content for gutter change markers (None = not loaded yet)
    head: Option<Option<String>>,
    marks: Option<(u64, Vec<git::Mark>)>,
    /// Code Vision per declaration line, keyed by (text hash, index generation)
    vision: Option<((u64, u64), Vec<(usize, String, String)>)>,
    /// Code Author per declaration line, keyed by (text hash, blame generation)
    authors: Option<((u64, u64), Vec<(usize, String)>)>,
    /// parameter hints, keyed by (text hash, index generation, answer generation)
    inlay: Option<((u64, u64, u64), Vec<hints::Hint>)>,
}

impl Tab {
    fn new(path: PathBuf, text: String, crlf: bool) -> Self {
        let lang = highlight::detect(&path);
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        Tab {
            path, text, lang, crlf, mtime, dirty: false, gone: false, cache: None, cursor: (0, 0), sel: (0, 0),
            select: None, scroll_to: None, lsp_pending: None, diff: None, sheet: None, head: None, marks: None, vision: None, authors: None, inlay: None,
        }
    }

    /// A file being edited, not a diff or a table.
    fn is_text(&self) -> bool {
        self.diff.is_none() && self.sheet.is_none()
    }
}

#[derive(Clone)]
enum Cmd {
    Open(PathBuf, Option<(usize, usize)>),
    Line(usize),
    SaveAll,
    CloseTab,
    Find,
    GotoLine,
    ToggleProject,
    Database,
    Tool(ToolWindow),
    Comment,
    Duplicate,
    GotoDeclaration,
    Usages,
    Implementations,
    Back,
    Forward,
    Commit,
    Push,
    Pull,
    Diff(PathBuf),
    FindInFiles,
    ClaudeCode,
    MentionClaude,
}

pub(crate) struct Ide {
    ctx: egui::Context,
    /// Root of every id the editor uses, so two IDE nodes never share widget state.
    id: Id,
    /// The folder as the node asked for it (the glue reopens the IDE when it changes).
    pub(crate) folder: String,
    pub(crate) root: Option<PathBuf>,
    tree: Option<TreeNode>,
    files: Vec<PathBuf>,
    files_rx: Option<Receiver<Vec<PathBuf>>>,
    pub(crate) branch: String,
    tabs: Vec<Tab>,
    pub(crate) active: usize,
    left: Option<Left>,
    claude: Option<claude::Claude>,
    claude_sel: Option<(PathBuf, (usize, usize))>,
    git: git::Git,
    toast: Option<(String, Instant)>,
    /// The last message, for the canvas toast while the IDE isn't on screen.
    pub(crate) notice: Option<String>,
    bottom: Option<ToolWindow>,
    db: db::Db,
    palette: Option<search::Everywhere>,
    find_files: Option<find::Find>,
    find_results: Option<(String, Vec<find::Hit>)>,
    index: index::Index,
    index_rx: Option<Receiver<index::Index>>,
    index_gen: u64,
    code_vision: bool,
    /// `git blame` per file (None = an uncommitted line), filled by `blame_request`
    blame: std::collections::HashMap<PathBuf, Vec<Option<String>>>,
    blame_rx: Option<Receiver<(PathBuf, Vec<Option<String>>)>>,
    blame_gen: u64,
    find: Option<String>,
    insp: Option<lsp::Inspections>,
    /// the Quick Documentation popup, while the pointer rests on a name or is on the popup
    doc: Option<docs::Popup>,
    pub(crate) status: String,
    font_size: f32,
    focused: bool,
    shift_tap: Option<f64>,
    last_disk_check: Instant,
    /// navigation history for Back/Forward (path, line, col)
    back: Vec<(PathBuf, usize, usize)>,
    forward: Vec<(PathBuf, usize, usize)>,
    decl_rx: Option<Receiver<Option<(PathBuf, usize, usize)>>>,
    /// word being resolved, for the native fallback when the language server has no answer
    decl_word: Option<String>,
    /// the word whose usages or implementations were asked for, and the line a signature hint
    /// belongs to (it goes away when the caret leaves that line)
    usage_word: String,
    sig_line: Option<(PathBuf, usize)>,
    /// Where the IDE is on screen this frame (its popups center on it).
    rect: Rect,
    /// Asks of the canvas: go back to it, or open a Claude Code terminal roped to this node.
    pub(crate) wants_canvas: bool,
    pub(crate) wants_claude: bool,
    /// prompts for the agents roped to this IDE (a comment on a change); the canvas sends them
    pub(crate) to_agent: Vec<String>,
    /// an agent is roped to this IDE (the canvas tells, each frame): comments have somewhere to go
    pub(crate) has_agent: bool,
    /// The card's scroll, in points: the tree's rows and the panel's lines, above their top.
    pub(crate) card_scroll: f32,
    pub(crate) panel_scroll: f32,
    /// The last row pressed on the card and when: a second press on it is the double-click.
    pub(crate) card_press: Option<(usize, f64)>,
    /// The card's selected row (a click or the arrows), lit in the tree; the Project panel shares it.
    pub(crate) card_sel: Option<usize>,
    /// The Project panel was clicked last: the arrows move its selection.
    tree_keys: bool,
    /// A name or a confirmation the tree is waiting for (ide/fileops.rs).
    pub(crate) file_ask: Option<FileAsk>,
    /// The trash asks first (the app's preference); `dont_ask_trash` tells the app it was turned off here.
    pub(crate) confirm_trash: bool,
    pub(crate) dont_ask_trash: bool,
    /// Lines an agent opened or changed (file, first, last), lit in the card's panel.
    pub(crate) panel_mark: Option<(PathBuf, usize, usize)>,
    /// The panel's colored text, by its text, language and zoom: laid out again only when one changes.
    pub(crate) panel_job: std::cell::RefCell<Option<(u64, LayoutJob)>>,
}

impl Drop for Ide {
    /// The node was removed, its folder changed or the app is closing.
    fn drop(&mut self) {
        self.save_all();
    }
}

impl Ide {
    /// The IDE of node `key` on `folder` (`~` is the home folder).
    pub(crate) fn new(ctx: egui::Context, key: &str, folder: &str) -> Self {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default();
        let root = match folder.trim().strip_prefix('~') {
            Some(rest) => home.join(rest.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(folder.trim()),
        };
        let mut ide = Ide {
            ctx,
            id: Id::unique(("ide", key)),
            folder: folder.to_string(),
            root: None,
            tree: None,
            files: vec![],
            files_rx: None,
            branch: String::new(),
            tabs: vec![],
            active: 0,
            left: Some(Left::Project),
            claude: None,
            claude_sel: None,
            git: git::Git::default(),
            toast: None,
            notice: None,
            bottom: None,
            db: Default::default(),
            palette: None,
            find_files: None,
            find_results: None,
            index: index::Index::default(),
            index_rx: None,
            index_gen: 0,
            code_vision: true,
            blame: Default::default(),
            blame_rx: None,
            blame_gen: 0,
            find: None,
            insp: None,
            doc: None,
            status: String::new(),
            font_size: t::EDITOR_SIZE,
            focused: true,
            shift_tap: None,
            last_disk_check: Instant::now(),
            back: vec![],
            forward: vec![],
            decl_rx: None,
            decl_word: None,
            usage_word: String::new(),
            sig_line: None,
            rect: Rect::NOTHING,
            wants_canvas: false,
            wants_claude: false,
            to_agent: Vec::new(),
            has_agent: false,
            card_scroll: 0.0,
            panel_scroll: 0.0,
            card_press: None,
            card_sel: None,
            tree_keys: false,
            file_ask: None,
            confirm_trash: true,
            dont_ask_trash: false,
            panel_mark: None,
            panel_job: Default::default(),
        };
        if root.is_dir() {
            ide.open_folder(root);
        } else {
            ide.status = format!("Pasta não encontrada: {}", root.display());
        }
        ide
    }

    fn open_folder(&mut self, root: PathBuf) {
        let mut tree = TreeNode::new(root.clone(), true);
        tree.open = true;
        tree.children(); // the card lists them
        self.tree = Some(tree);
        self.branch = std::fs::read_to_string(root.join(".git/HEAD")).ok()
            .map(|h| h.trim().trim_start_matches("ref: refs/heads/").chars().take(40).collect())
            .unwrap_or_default();
        self.reload_files(&root);
        self.insp = Some(lsp::Inspections::new(self.ctx.clone(), root.clone()));
        self.claude = claude::Claude::start(self.ctx.clone(), &root).map_err(|e| self.status = format!("Integração com o Claude Code: {e}")).ok();
        self.root = Some(root);
        self.git_refresh();
    }

    /// Lists the project's files again (Go to File, the index) on a thread.
    fn reload_files(&mut self, root: &Path) {
        let (tx, rx) = std::sync::mpsc::channel();
        let (r, ctx) = (root.to_path_buf(), self.ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(tree::list_files(&r));
            ctx.request_repaint();
        });
        self.files_rx = Some(rx);
    }

    /// Env for terminals roped to this IDE, so `claude` started there connects to it.
    pub(crate) fn claude_env(&self) -> Vec<(String, String)> {
        self.claude.as_ref().map(|c| c.env()).unwrap_or_default()
    }

    pub(crate) fn claude_connected(&self) -> bool {
        self.claude.as_ref().is_some_and(|c| c.connected())
    }

    pub(crate) fn open_file(&mut self, path: PathBuf, at: Option<(usize, usize)>) {
        let i = match self.tabs.iter().position(|t| t.path == path && t.is_text()) {
            Some(i) => i,
            None => {
                if std::fs::metadata(&path).is_ok_and(|m| m.len() > 4 << 20) {
                    self.status = "Arquivo maior que 4 MB, não foi aberto".into(); // ponytail: no huge-file mode yet
                    return;
                }
                let text = match std::fs::read(&path).map(String::from_utf8) {
                    Ok(Ok(s)) => s,
                    Ok(Err(_)) => return self.status = "Arquivo binário, não foi aberto".into(),
                    Err(e) => return self.status = format!("{e}"),
                };
                let crlf = text.contains("\r\n");
                let text = if crlf { text.replace("\r\n", "\n") } else { text };
                let tab = Tab::new(path, text, crlf);
                if let Some(insp) = &mut self.insp {
                    insp.open(&tab.path, tab.lang, &tab.text);
                }
                self.tabs.push(tab);
                self.tabs.len() - 1
            }
        };
        self.active = i;
        if let Some((line, col)) = at {
            let tab = &mut self.tabs[i];
            let c = char_at(&tab.text, line, col);
            tab.select = Some((c, c));
        }
    }

    /// An agent's edit (the MCP's ide_edit): `old`, found exactly once in `path`, becomes `new`,
    /// in the open tab (the file comes forward) and on disk. Back: the changed lines (first, last).
    pub(crate) fn agent_edit(&mut self, path: &Path, old: &str, new: &str) -> Result<(usize, usize), String> {
        let (old, new) = (old.replace("\r\n", "\n"), new.replace("\r\n", "\n"));
        if old.is_empty() {
            return Err("`old` vazio: mande o trecho a trocar".into());
        }
        self.open_file(path.to_path_buf(), None);
        let i = self.active;
        let tab = self.tabs.get_mut(i).filter(|t| t.path == path && t.is_text()).ok_or_else(|| self.status.clone())?;
        // a tab nobody is typing in follows the disk (the agent may have written the file itself)
        if !tab.dirty
            && let Ok(text) = std::fs::read_to_string(path)
        {
            tab.text = text.replace("\r\n", "\n");
            tab.mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        }
        let at = match tab.text.match_indices(&old).map(|(at, _)| at).collect::<Vec<_>>()[..] {
            [at] => at,
            [] => return Err(format!("o trecho não está em {}", file_name(path))),
            ref many => return Err(format!("o trecho aparece {} vezes em {}: mande mais contexto", many.len(), file_name(path))),
        };
        tab.text.replace_range(at..at + old.len(), &new);
        let first = tab.text[..at].matches('\n').count();
        let last = first + new.matches('\n').count();
        (tab.dirty, tab.lsp_pending, tab.scroll_to) = (true, Some(Instant::now()), Some(first));
        self.save(i, false);
        if self.tabs[i].dirty {
            return Err(format!("não foi possível salvar: {}", self.status));
        }
        self.panel_mark = Some((path.to_path_buf(), first, last));
        Ok((first, last))
    }

    fn save(&mut self, i: usize, force: bool) {
        let tab = &self.tabs[i];
        if !tab.dirty || !tab.is_text() {
            return;
        }
        // Claude Code, git or a formatter rewrote it meanwhile: only an explicit Ctrl+S overwrites that
        // ponytail: no reload/discard action, so a conflicting tab is lost silently when the node goes
        // away or the app closes; a "Recarregar do disco" command (or PhpStorm's conflict dialog) is the next step
        let on_disk = std::fs::metadata(&tab.path).and_then(|m| m.modified()).ok();
        if !force && tab.mtime.is_some() && on_disk.is_none() {
            return self.notify(format!("{} foi excluído do disco; {} grava de novo", file_name(&tab.path), chrome::keys("Ctrl+S")));
        }
        if !force && tab.mtime.is_some() && on_disk != tab.mtime {
            return self.notify(format!("{} mudou no disco; {} sobrescreve", file_name(&tab.path), chrome::keys("Ctrl+S")));
        }
        let tab = &mut self.tabs[i];
        let text = if tab.crlf { tab.text.replace('\n', "\r\n") } else { tab.text.clone() };
        match std::fs::write(&tab.path, text) {
            Ok(()) => {
                tab.dirty = false;
                tab.mtime = std::fs::metadata(&tab.path).and_then(|m| m.modified()).ok();
                tab.head.get_or_insert(None); // keep markers, HEAD did not move
                // the blame is of the file on disk: the next frame asks for it again
                self.blame.remove(&tab.path);
                self.blame_gen += 1;
                if self.root.as_ref().is_some_and(|r| index::indexable(r, &tab.path)) {
                    self.index.update(&tab.path);
                    self.index_gen += 1;
                }
                if let Some(insp) = &mut self.insp {
                    if tab.lsp_pending.take().is_some() {
                        insp.change(&tab.path, tab.lang, &tab.text);
                    }
                    insp.save(&tab.path, tab.lang);
                }
            }
            Err(e) => self.status = format!("Não foi possível salvar {}: {e}", tab.path.display()),
        }
    }

    /// PhpStorm-style: no "save?" prompts, everything is saved on tab close, focus loss, leaving
    /// the IDE, removing its node (`Drop`) and exit — except a file that changed on disk meanwhile,
    /// which only Ctrl+S overwrites.
    pub(crate) fn save_all(&mut self) {
        for i in 0..self.tabs.len() {
            self.save(i, false);
        }
    }

    pub(crate) fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        if self.tabs[i].sheet.as_ref().is_some_and(|s| s.pending()) {
            self.notify("Envie ou reverta as pendências da tabela antes de fechar a aba.".into());
            return;
        }
        self.save(i, false);
        if self.tabs[i].dirty {
            return; // not saved (changed on disk, or the write failed): the status says why, the tab stays
        }
        // the card's panel goes to another file, at its top
        if self.card_tab() == Some(i) {
            self.panel_scroll = 0.0;
        }
        let mut tab = self.tabs.remove(i);
        if let Some((reply, _)) = tab.diff.as_mut().and_then(|d| d.claude.take()) {
            let _ = reply.send(vec!["DIFF_REJECTED".into()]);
        }
        if let Some(insp) = &mut self.insp
            && tab.is_text()
        {
            insp.close(&tab.path, tab.lang);
        }
        if self.active >= i && self.active > 0 {
            self.active -= 1;
        }
    }

    fn toggle_left(&mut self, w: Left) {
        self.left = if self.left == Some(w) { None } else { Some(w) };
    }

    fn git_push(&mut self) {
        self.save_all();
        self.git_run("Push", vec![vec!["push".into()]]);
    }

    fn git_pull(&mut self) {
        self.save_all();
        self.git_run("Atualizar projeto", vec![vec!["pull".into(), "--ff-only".into()]]);
    }

    /// Balloon notification, bottom-right like PhpStorm's.
    fn notify(&mut self, msg: String) {
        self.status = msg.clone();
        self.notice = Some(msg.clone());
        self.toast = Some((msg, Instant::now()));
    }

    fn run(&mut self, cmd: Cmd) {
        let text = self.tabs.get(self.active).is_some_and(Tab::is_text);
        match cmd {
            // a diff or a table: nothing to find in or edit (an edit would leave the tab dirty, and never closable)
            Cmd::Find | Cmd::Comment | Cmd::Duplicate if !text => {}
            Cmd::Open(p, at) => self.open_file(p, at),
            Cmd::Line(l) => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    let c = char_at(&tab.text, l.saturating_sub(1), 0);
                    tab.select = Some((c, c));
                }
            }
            Cmd::SaveAll => {
                // a line still being typed on a diff's right side goes in first, as Enter would put it
                let typing = self.tabs.get_mut(self.active).and_then(|t| Some((t.path.clone(), t.diff.as_mut()?.editing.take()?)));
                if let Some((path, (n, line))) = typing
                    && let Some(b) = self.buffer_tab(&path)
                {
                    let text = self.tabs[b].text.clone();
                    if let Some(next) = self.tabs[self.active].diff.as_mut().and_then(|d| d.apply(&text, git::DiffEdit::Line(n, line))) {
                        (self.tabs[b].text, self.tabs[b].dirty) = (next, true);
                    }
                }
                for i in 0..self.tabs.len() {
                    self.save(i, true);
                }
            }
            Cmd::CloseTab => self.close_tab(self.active),
            Cmd::Find => self.find = Some(self.find.take().unwrap_or_default()),
            Cmd::GotoLine => self.palette = Some(search::Everywhere::new(search::Tab::All, ":")),
            Cmd::FindInFiles => self.open_find(None),
            Cmd::ClaudeCode => self.wants_claude = true,
            Cmd::MentionClaude => self.claude_mention(),
            Cmd::ToggleProject => self.toggle_left(Left::Project),
            Cmd::Database => self.toggle_left(Left::Database),
            Cmd::Commit => {
                self.toggle_left(Left::Commit);
                self.git_refresh();
            }
            Cmd::Push => self.git_push(),
            Cmd::Diff(p) => self.open_diff(p),
            Cmd::Pull => self.git_pull(),
            Cmd::Tool(tool) => self.bottom = if self.bottom == Some(tool) { None } else { Some(tool) },
            Cmd::Comment => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    toggle_comment(tab);
                }
            }
            Cmd::Duplicate => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    duplicate(tab);
                }
            }
            Cmd::GotoDeclaration => {
                if let Some(tab) = self.tabs.get(self.active) {
                    let c = char_at(&tab.text, tab.cursor.0, tab.cursor.1);
                    self.goto_declaration(c);
                }
            }
            Cmd::Back => {
                if let Some(to) = self.back.pop() {
                    if let Some(here) = self.here() {
                        self.forward.push(here);
                    }
                    self.open_file(to.0, Some((to.1, to.2)));
                }
            }
            Cmd::Usages => self.ask_at_cursor(lsp::Ask::Usages),
            Cmd::Implementations => self.ask_at_cursor(lsp::Ask::Implementations),
            Cmd::Forward => {
                if let Some(to) = self.forward.pop() {
                    if let Some(here) = self.here() {
                        self.back.push(here);
                    }
                    self.open_file(to.0, Some((to.1, to.2)));
                }
            }
        }
    }

    // ------------------------------------------------------------ frame

    /// Every frame, on screen or not: results from the background threads, Claude Code's tool
    /// calls (they wait for an answer), saving on focus loss and reloading files changed outside.
    /// What changed on disk meanwhile: the tree, clean tabs reloaded, the tabs of files gone marked.
    fn check_disk(&mut self) {
        self.sync_tree();
        for tab in self.tabs.iter_mut().filter(|t| t.is_text()) {
            let m = std::fs::metadata(&tab.path).and_then(|m| m.modified()).ok();
            tab.gone = m.is_none();
            if !tab.dirty
                && m != tab.mtime
                && let Ok(s) = std::fs::read_to_string(&tab.path)
            {
                tab.crlf = s.contains("\r\n");
                tab.text = if tab.crlf { s.replace("\r\n", "\n") } else { s };
                tab.mtime = m;
                tab.lsp_pending = Some(Instant::now());
            }
        }
    }

    pub(crate) fn background(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.files_rx
            && let Ok(files) = rx.try_recv()
        {
            self.files = files;
            self.files_rx = None;
            if let Some(root) = self.root.clone() {
                let (tx, rx) = std::sync::mpsc::channel();
                let (files, ctx) = (self.files.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(index::Index::build(&root, &files));
                    ctx.request_repaint();
                });
                self.index_rx = Some(rx);
            }
        }
        if let Some(rx) = &self.index_rx
            && let Ok(ix) = rx.try_recv()
        {
            self.index = ix;
            self.index_rx = None;
            self.index_gen += 1;
        }
        let (mut jump, mut usages) = (None, None);
        if let Some(insp) = &mut self.insp {
            insp.poll();
            jump = insp.jump.take();
            usages = insp.usages.take();
            // debounce didChange: send 300 ms after the last keystroke
            for tab in &mut self.tabs {
                if let Some(at) = tab.lsp_pending {
                    if at.elapsed() > Duration::from_millis(300) {
                        tab.lsp_pending = None;
                        insp.change(&tab.path, tab.lang, &tab.text);
                    } else {
                        ctx.request_repaint_after(Duration::from_millis(300));
                    }
                }
            }
        }
        if let Some((ask, places)) = usages {
            self.show_usages(ask, places);
        }
        match jump {
            // already on the declaration (the target is the word under the caret): like PhpStorm, Ctrl+click lists its usages instead
            Some(Some((p, l, c))) if self.tabs.get(self.active).is_some_and(|t| t.path == p && { let (a, b) = word_at(&t.text, char_at(&t.text, t.cursor.0, t.cursor.1)); (a..b).contains(&char_at(&t.text, l, c)) }) => self.ask_at_cursor(lsp::Ask::Usages),
            Some(Some((p, l, c))) => self.jump_to(p, l, c),
            Some(None) => self.native_declaration(),
            None => {}
        }
        if let Some(rx) = &self.decl_rx
            && let Ok(found) = rx.try_recv()
        {
            self.decl_rx = None;
            match found {
                Some((p, l, c)) => self.jump_to(p, l, c),
                None => self.status = format!("Declaração de '{}' não encontrada", self.decl_word.take().unwrap_or_default()),
            }
        }
        if let Some(rx) = &self.blame_rx
            && let Ok((path, lines)) = rx.try_recv()
        {
            self.blame_rx = None;
            self.blame.insert(path, lines);
            self.blame_gen += 1;
        }
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if self.focused && !focused {
            self.save_all();
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_all();
        }
        // pick up edits made outside (Claude Code, git, formatters) for clean tabs
        self.git_poll();
        self.db_background();
        self.claude_calls();
        self.claude_selection();
        if focused && !self.focused {
            self.git_refresh();
        }
        if (focused && !self.focused) || self.last_disk_check.elapsed() > Duration::from_secs(2) {
            self.last_disk_check = Instant::now();
            self.check_disk();
        }
        if !self.tabs.is_empty() {
            ctx.request_repaint_after(Duration::from_secs(2));
        }
        self.focused = focused;
        self.file_ask_ui(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_tab_never_overwrites_a_file_changed_on_disk() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, "um\n").unwrap();
        let mut ide = Ide::new(egui::Context::default(), "k", &dir.to_string_lossy());
        ide.open_file(f.clone(), None);
        ide.tabs[0].text = "meu\n".into();
        ide.tabs[0].dirty = true;
        // Claude Code (or git) rewrote it meanwhile
        std::fs::write(&f, "deles\n").unwrap();
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
        ide.save_all();
        ide.close_tab(0);
        assert_eq!((std::fs::read_to_string(&f).unwrap().as_str(), ide.tabs.len(), ide.tabs[0].dirty), ("deles\n", 1, true));
        assert!(ide.status.contains("mudou no disco"), "{}", ide.status);
        ide.run(Cmd::SaveAll); // the explicit save is the user's decision
        assert_eq!((std::fs::read_to_string(&f).unwrap().as_str(), ide.tabs[0].dirty), ("meu\n", false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_tree_picks_up_files_made_and_removed_outside() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-sync-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.rs"), "").unwrap();
        std::fs::write(dir.join("z.txt"), "").unwrap();
        let mut ide = Ide::new(egui::Context::default(), "s", &dir.to_string_lossy());
        ide.card_click(0); // opens src: src, a.rs, z.txt
        ide.card_click(2);
        // a terminal adds and removes files meanwhile
        std::fs::write(dir.join("src/b.rs"), "").unwrap();
        std::fs::remove_file(dir.join("src/a.rs")).unwrap();
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        ide.sync_tree();
        let rows = ide.card_rows();
        assert_eq!(rows.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(), ["docs", "src", "b.rs", "z.txt"]);
        assert!(rows[1].3, "src stays open");
        assert_eq!(ide.card_sel, Some(3), "the selection stays on z.txt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_cards_panel_fits_its_tabs_and_keeps_a_file_under_a_diff() {
        let (_, mut ide) = shown(12);
        let panel = Rect::from_min_size(egui::Pos2::ZERO, vec2(card::PANEL_W, 400.0));
        let tabs = ide.panel_tabs(panel);
        assert_eq!(tabs.len(), 12);
        assert!(tabs.iter().all(|(r, _)| r.right() <= panel.right() - 8.0 + 0.01), "{:?}", tabs.last());
        // a diff comes forward: the panel keeps the last file; closing that one shows another from its top
        let mut diff = Tab::new(ide.tabs[0].path.clone(), String::new(), false);
        diff.diff = Some(Box::new(git::Diff::new("um\n", "dois\n")));
        ide.tabs.push(diff);
        ide.active = 12;
        assert_eq!(ide.card_tab(), Some(11));
        ide.panel_scroll = 40.0;
        ide.close_tab(11);
        assert_eq!((ide.card_tab(), ide.panel_scroll), (Some(10), 0.0));
    }

    #[test]
    fn a_tab_whose_file_goes_stays_marked_and_only_ctrl_s_writes_it_back() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-gone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, "um").unwrap();
        let mut ide = Ide::new(egui::Context::default(), "g", &dir.to_string_lossy());
        ide.open_file(f.clone(), None);
        (ide.tabs[0].text, ide.tabs[0].dirty) = ("um dois".into(), true);
        std::fs::remove_file(&f).unwrap();
        ide.check_disk();
        assert!(ide.tabs[0].gone);
        // saving on the way out leaves it deleted; Ctrl+S writes it again
        ide.save_all();
        assert!(!f.exists());
        ide.save(0, true);
        ide.check_disk();
        assert!(!ide.tabs[0].gone);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "um dois");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_keeps_the_line_being_typed_in_a_diff() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-diffsave-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, "um\ndois\n").unwrap();
        let mut ide = Ide::new(egui::Context::default(), "d", &dir.to_string_lossy());
        ide.open_diff(f.clone());
        // a double click on line 2 of the right side, "DOIS" typed, then Ctrl+S without Enter
        ide.tabs[ide.active].diff.as_mut().unwrap().editing = Some((1, "DOIS".into()));
        ide.run(Cmd::SaveAll);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "um\nDOIS\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An IDE on a missing folder (no servers, no IDE integration) with `tabs` empty text tabs.
    fn shown(tabs: usize) -> (egui::Context, Ide) {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        for (name, family) in [("ide", egui::FontFamily::Proportional), ("ide-mono", egui::FontFamily::Monospace)] {
            fonts.families.insert(egui::FontFamily::Name(name.into()), fonts.families[&family].clone());
        }
        ctx.set_fonts(fonts);
        let root = std::env::temp_dir().join(format!("magoagent-ui-{}", uuid::Uuid::new_v4()));
        let mut ide = Ide::new(ctx.clone(), "ui", &root.to_string_lossy());
        ide.code_vision = false;
        for i in 0..tabs {
            ide.tabs.push(Tab::new(root.join(format!("tabela_{i}.txt")), String::new(), false));
        }
        (ctx, ide)
    }
    /// `n` frames, `events` in the first; the last one's output.
    fn frames(ctx: &egui::Context, ide: &mut Ide, events: Vec<egui::Event>, n: usize) -> egui::FullOutput {
        let mut out = None;
        for events in std::iter::once(events).chain(std::iter::repeat_with(Vec::new)).take(n) {
            let screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000.0, 600.0)));
            let mut o = ctx.run_ui(egui::RawInput { screen_rect, events, ..Default::default() }, |ui| ide.ui(ui, true));
            o.textures_delta.clear();
            out = Some(o);
        }
        out.unwrap()
    }

    #[test]
    fn many_tabs_show_the_active_one_and_scroll_with_the_wheel() {
        let (ctx, mut ide) = shown(30);
        ide.active = 29;
        // the selected tab's box and the clip it is painted in
        let selected = |out: egui::FullOutput| out.shapes.iter().find_map(|c| match &c.shape {
            Shape::Rect(r) if r.fill == t::TAB_SELECTED => Some((c.clip_rect, r.rect)),
            _ => None,
        }).expect("the selected tab is painted");
        let (clip, tab) = selected(frames(&ctx, &mut ide, vec![], 40));
        assert!(clip.contains_rect(tab), "the last tab opens in view: {tab:?} in {clip:?}");
        let wheel = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, 300.0), phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Wheel, modifiers: Modifiers::NONE };
        let (_, scrolled) = selected(frames(&ctx, &mut ide, vec![egui::Event::PointerMoved(tab.center()), wheel], 40));
        assert!(scrolled.left() > tab.left() + 100.0, "the wheel scrolls the tabs back: {scrolled:?}");
    }

    #[test]
    fn the_esc_that_closes_a_menu_keeps_the_ide() {
        let (ctx, mut ide) = shown(1);
        let esc = || egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
        frames(&ctx, &mut ide, vec![], 2);
        egui::Popup::open_id(&ctx, egui::Id::unique("menu"));
        frames(&ctx, &mut ide, vec![esc()], 1);
        assert!(!ide.wants_canvas, "the Esc closed the menu");
        // a popup nobody shows lasts one more frame
        frames(&ctx, &mut ide, vec![], 1);
        frames(&ctx, &mut ide, vec![esc()], 1);
        assert!(ide.wants_canvas, "a second Esc goes back to the canvas");
    }

    #[test]
    fn the_project_panel_selects_on_a_click_and_opens_on_the_second() {
        let (ctx, mut ide) = shown(0);
        let dir = std::env::temp_dir().join(format!("magoagent-project-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        let mut tree = tree::TreeNode::new(dir.clone(), true);
        tree.open = true;
        ide.tree = Some(tree);
        let out = frames(&ctx, &mut ide, vec![], 2);
        let at = out.shapes.iter().find_map(|c| match &c.shape {
            Shape::Text(s) if s.galley.text() == "b.txt" => Some(s.pos + vec2(4.0, 6.0)),
            _ => None,
        }).expect("b.txt is listed");
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        let click = || vec![egui::Event::PointerMoved(at), button(true), button(false)];
        let key = |key| vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }];
        frames(&ctx, &mut ide, click(), 1);
        assert_eq!((ide.card_sel, ide.tabs.len()), (Some(1), 0), "a click selects b.txt");
        frames(&ctx, &mut ide, key(Key::ArrowDown), 1);
        frames(&ctx, &mut ide, key(Key::Enter), 1);
        assert_eq!((ide.card_sel, ide.tabs.last().map(|t| file_name(&t.path))), (Some(2), Some("c.txt".into())), "the arrows move it, Enter opens");
        frames(&ctx, &mut ide, vec![], 20);
        frames(&ctx, &mut ide, click(), 1);
        frames(&ctx, &mut ide, click(), 1);
        assert_eq!(ide.tabs.last().map(|t| file_name(&t.path)).as_deref(), Some("b.txt"), "a second click opens");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_shift_chord_is_not_taken_by_its_plain_shortcut() {
        let (ctx, mut ide) = shown(1);
        let key = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
        // Ctrl+Shift+F (Cmd on the Mac) finds in the files, Ctrl+F in the file
        frames(&ctx, &mut ide, vec![key(Key::F, Modifiers::COMMAND | Modifiers::SHIFT)], 1);
        assert!(ide.find_files.is_some() && ide.find.is_none());
        ide.find_files = None;
        frames(&ctx, &mut ide, vec![key(Key::F, Modifiers::COMMAND)], 1);
        assert!(ide.find.is_some() && ide.find_files.is_none());
    }

    #[test]
    fn claude_code_opens_with_ctrl_esc_and_ctrl_alt_esc() {
        // Windows keeps Ctrl+Esc for the Start menu: its labels say Ctrl+Alt+Esc, the same binding;
        // the Mac takes Cmd, like the rest of the IDE's keys
        let (ctx, mut ide) = shown(1);
        for modifiers in [Modifiers::COMMAND, Modifiers::COMMAND | Modifiers::ALT] {
            ide.wants_claude = false;
            frames(&ctx, &mut ide, vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers }], 1);
            assert!(ide.wants_claude, "{modifiers:?}");
        }
    }
}
