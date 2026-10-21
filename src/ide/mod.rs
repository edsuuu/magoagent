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

mod chrome;
mod claude;
mod commit;
mod editor;
mod find;
mod git;
mod highlight;
pub(crate) mod icons;
mod index;
mod langs;
mod lsp;
mod search;
mod text;
pub(crate) mod theme;
mod tree;

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
    cache: Option<(u64, LayoutJob)>,
    cursor: (usize, usize), // 0-based line, col
    sel: (usize, usize),    // char range
    select: Option<(usize, usize)>,
    scroll_to: Option<usize>,
    lsp_pending: Option<Instant>,
    /// set for a diff tab (HEAD vs working copy)
    diff: Option<Box<git::Diff>>,
    /// HEAD content for gutter change markers (None = not loaded yet)
    head: Option<Option<String>>,
    marks: Option<(u64, Vec<git::Mark>)>,
    /// Code Vision per declaration line, keyed by (text hash, index generation)
    vision: Option<((u64, u64), Vec<(usize, String, String)>)>,
}

impl Tab {
    fn new(path: PathBuf, text: String, crlf: bool) -> Self {
        let lang = highlight::detect(&path);
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        Tab {
            path, text, lang, crlf, mtime, dirty: false, cache: None, cursor: (0, 0), sel: (0, 0),
            select: None, scroll_to: None, lsp_pending: None, diff: None, head: None, marks: None, vision: None,
        }
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
    Tool(ToolWindow),
    Comment,
    Duplicate,
    GotoDeclaration,
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

const ACTIONS: &[(&str, &str, fn() -> Cmd)] = &[
    ("Salvar tudo", "Ctrl+S", || Cmd::SaveAll),
    ("Fechar aba", "Ctrl+W", || Cmd::CloseTab),
    ("Localizar no arquivo", "Ctrl+F", || Cmd::Find),
    ("Localizar nos arquivos…", "Ctrl+Shift+F", || Cmd::FindInFiles),
    ("Ir para a linha…", "Ctrl+G", || Cmd::GotoLine),
    ("Projeto", "Alt+1", || Cmd::ToggleProject),
    ("Problemas", "Alt+6", || Cmd::Tool(ToolWindow::Problems)),
    ("Claude Code (terminal ligado ao IDE)", "Ctrl+Esc", || Cmd::ClaudeCode),
    ("Enviar seleção ao Claude Code", "Ctrl+Alt+K", || Cmd::MentionClaude),
    ("Commit…", "Ctrl+K · Alt+0", || Cmd::Commit),
    ("Push…", "Ctrl+Shift+K", || Cmd::Push),
    ("Atualizar projeto…", "Ctrl+T", || Cmd::Pull),
    ("Comentar com comentário de linha", "Ctrl+/", || Cmd::Comment),
    ("Duplicar linha ou seleção", "Ctrl+D", || Cmd::Duplicate),
    ("Ir para a declaração", "Ctrl+B · Ctrl+Clique · F12", || Cmd::GotoDeclaration),
    ("Voltar", "Ctrl+Alt+Esquerda · Alt+Esquerda", || Cmd::Back),
    ("Avançar", "Ctrl+Alt+Direita · Alt+Direita", || Cmd::Forward),
];

pub(crate) struct Ide {
    ctx: egui::Context,
    /// Root of every id the editor uses, so two IDE nodes never share widget state.
    id: Id,
    /// The folder as the node asked for it (the glue reopens the IDE when it changes).
    pub(crate) folder: String,
    root: Option<PathBuf>,
    tree: Option<TreeNode>,
    /// The project's first entries (name, is a folder), for the node's card on the canvas.
    pub(crate) preview: Vec<(String, bool)>,
    files: Vec<PathBuf>,
    files_rx: Option<Receiver<Vec<PathBuf>>>,
    pub(crate) branch: String,
    tabs: Vec<Tab>,
    active: usize,
    left: Option<Left>,
    claude: Option<claude::Claude>,
    claude_sel: Option<(PathBuf, (usize, usize))>,
    git: git::Git,
    toast: Option<(String, Instant)>,
    /// The last message, for the canvas toast while the IDE isn't on screen.
    pub(crate) notice: Option<String>,
    bottom: Option<ToolWindow>,
    palette: Option<search::Everywhere>,
    find_files: Option<find::Find>,
    find_results: Option<(String, Vec<find::Hit>)>,
    index: index::Index,
    index_rx: Option<Receiver<index::Index>>,
    index_gen: u64,
    code_vision: bool,
    find: Option<String>,
    insp: Option<lsp::Inspections>,
    status: String,
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
    /// Where the IDE is on screen this frame (its popups center on it).
    rect: Rect,
    /// Asks of the canvas: go back to it, or open a Claude Code terminal roped to this node.
    pub(crate) wants_canvas: bool,
    pub(crate) wants_claude: bool,
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
            preview: vec![],
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
            palette: None,
            find_files: None,
            find_results: None,
            index: index::Index::default(),
            index_rx: None,
            index_gen: 0,
            code_vision: true,
            find: None,
            insp: None,
            status: String::new(),
            font_size: t::EDITOR_SIZE,
            focused: true,
            shift_tap: None,
            last_disk_check: Instant::now(),
            back: vec![],
            forward: vec![],
            decl_rx: None,
            decl_word: None,
            rect: Rect::NOTHING,
            wants_canvas: false,
            wants_claude: false,
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
        self.preview = tree.children().iter().filter(|c| !tree::HIDDEN.contains(&c.name.as_str())).map(|c| (c.name.clone(), c.dir)).collect();
        self.tree = Some(tree);
        self.branch = std::fs::read_to_string(root.join(".git/HEAD")).ok()
            .map(|h| h.trim().trim_start_matches("ref: refs/heads/").chars().take(40).collect())
            .unwrap_or_default();
        let (tx, rx) = std::sync::mpsc::channel();
        let (r, ctx) = (root.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(tree::list_files(&r));
            ctx.request_repaint();
        });
        self.files_rx = Some(rx);
        self.insp = Some(lsp::Inspections::new(self.ctx.clone(), root.clone()));
        self.claude = claude::Claude::start(self.ctx.clone(), &root).map_err(|e| self.status = format!("Integração com o Claude Code: {e}")).ok();
        self.root = Some(root);
        self.git_refresh();
    }

    /// Env for terminals roped to this IDE, so `claude` started there connects to it.
    pub(crate) fn claude_env(&self) -> Vec<(String, String)> {
        self.claude.as_ref().map(|c| c.env()).unwrap_or_default()
    }

    pub(crate) fn claude_connected(&self) -> bool {
        self.claude.as_ref().is_some_and(|c| c.connected())
    }

    fn open_file(&mut self, path: PathBuf, at: Option<(usize, usize)>) {
        let i = match self.tabs.iter().position(|t| t.path == path && t.diff.is_none()) {
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

    fn save(&mut self, i: usize) {
        let tab = &mut self.tabs[i];
        if !tab.dirty || tab.diff.is_some() {
            return;
        }
        let text = if tab.crlf { tab.text.replace('\n', "\r\n") } else { tab.text.clone() };
        match std::fs::write(&tab.path, text) {
            Ok(()) => {
                tab.dirty = false;
                tab.mtime = std::fs::metadata(&tab.path).and_then(|m| m.modified()).ok();
                tab.head.get_or_insert(None); // keep markers, HEAD did not move
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
    /// the IDE, removing its node and exit.
    pub(crate) fn save_all(&mut self) {
        for i in 0..self.tabs.len() {
            self.save(i);
        }
    }

    fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        self.save(i);
        let mut tab = self.tabs.remove(i);
        if let Some((reply, _)) = tab.diff.as_mut().and_then(|d| d.claude.take()) {
            let _ = reply.send(vec!["DIFF_REJECTED".into()]);
        }
        if let Some(insp) = &mut self.insp
            && tab.diff.is_none()
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
        match cmd {
            Cmd::Open(p, at) => self.open_file(p, at),
            Cmd::Line(l) => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    let c = char_at(&tab.text, l.saturating_sub(1), 0);
                    tab.select = Some((c, c));
                }
            }
            Cmd::SaveAll => self.save_all(),
            Cmd::CloseTab => self.close_tab(self.active),
            Cmd::Find => self.find = Some(self.find.take().unwrap_or_default()),
            Cmd::GotoLine => self.palette = Some(search::Everywhere::new(search::Tab::All, ":")),
            Cmd::FindInFiles => self.open_find(None),
            Cmd::ClaudeCode => self.wants_claude = true,
            Cmd::MentionClaude => self.claude_mention(),
            Cmd::ToggleProject => self.toggle_left(Left::Project),
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
        let mut jump = None;
        if let Some(insp) = &mut self.insp {
            insp.poll();
            jump = insp.jump.take();
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
        match jump {
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
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if self.focused && !focused {
            self.save_all();
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_all();
        }
        // pick up edits made outside (Claude Code, git, formatters) for clean tabs
        self.git_poll();
        self.claude_calls();
        self.claude_selection();
        if focused && !self.focused {
            self.git_refresh();
        }
        if (focused && !self.focused) || self.last_disk_check.elapsed() > Duration::from_secs(2) {
            self.last_disk_check = Instant::now();
            for tab in self.tabs.iter_mut().filter(|t| !t.dirty && t.diff.is_none()) {
                let m = std::fs::metadata(&tab.path).and_then(|m| m.modified()).ok();
                if m != tab.mtime
                    && let Ok(s) = std::fs::read_to_string(&tab.path)
                {
                    tab.crlf = s.contains("\r\n");
                    tab.text = if tab.crlf { s.replace("\r\n", "\n") } else { s };
                    tab.mtime = m;
                    tab.lsp_pending = Some(Instant::now());
                }
            }
        }
        if !self.tabs.is_empty() {
            ctx.request_repaint_after(Duration::from_secs(2));
        }
        self.focused = focused;
    }

    /// Only while the IDE has the keyboard (the canvas's shortcuts are off then).
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let sc = |m, k| KeyboardShortcut::new(m, k);
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::CTRL, Key::Escape))) {
            self.run(Cmd::ClaudeCode);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::CTRL | Modifiers::ALT, Key::K))) {
            self.run(Cmd::MentionClaude);
        }
        let (ctrl, shift, alt) = (Modifiers::COMMAND, Modifiers::COMMAND | Modifiers::SHIFT, Modifiers::ALT);
        let table: [(KeyboardShortcut, Cmd); 21] = [
            (sc(ctrl, Key::S), Cmd::SaveAll),
            (sc(ctrl, Key::W), Cmd::CloseTab),
            (sc(ctrl, Key::F4), Cmd::CloseTab),
            (sc(ctrl, Key::F), Cmd::Find),
            (sc(ctrl, Key::G), Cmd::GotoLine),
            (sc(alt, Key::Num1), Cmd::ToggleProject),
            (sc(shift, Key::E), Cmd::ToggleProject),
            (sc(alt, Key::Num6), Cmd::Tool(ToolWindow::Problems)),
            (sc(shift, Key::M), Cmd::Tool(ToolWindow::Problems)),
            (sc(ctrl, Key::Slash), Cmd::Comment),
            (sc(ctrl, Key::D), Cmd::Duplicate),
            (sc(ctrl, Key::B), Cmd::GotoDeclaration),
            (sc(Modifiers::NONE, Key::F12), Cmd::GotoDeclaration),
            (sc(ctrl | Modifiers::ALT, Key::ArrowLeft), Cmd::Back),
            (sc(alt, Key::ArrowLeft), Cmd::Back),
            (sc(ctrl | Modifiers::ALT, Key::ArrowRight), Cmd::Forward),
            (sc(alt, Key::ArrowRight), Cmd::Forward),
            (sc(ctrl, Key::K), Cmd::Commit),
            (sc(alt, Key::Num0), Cmd::Commit),
            (sc(shift, Key::K), Cmd::Push),
            (sc(ctrl, Key::T), Cmd::Pull),
        ];
        for (s, cmd) in table {
            if ctx.input_mut(|i| i.consume_shortcut(&s)) {
                self.run(cmd);
            }
        }
        // VSCode: Ctrl+P files, Ctrl+Shift+P actions. PhpStorm: Ctrl+Shift+N files, Ctrl+Shift+A actions, Shift Shift everything.
        if ctx.input_mut(|i| i.consume_shortcut(&sc(shift, Key::P)) || i.consume_shortcut(&sc(shift, Key::A))) {
            self.palette = Some(search::Everywhere::new(search::Tab::Actions, ""));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(ctrl | Modifiers::ALT | Modifiers::SHIFT, Key::N))) {
            self.palette = Some(search::Everywhere::new(search::Tab::Symbols, ""));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(ctrl, Key::P)) || i.consume_shortcut(&sc(shift, Key::N))) {
            self.palette = Some(search::Everywhere::new(search::Tab::Files, ""));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(ctrl, Key::N))) {
            self.palette = Some(search::Everywhere::new(search::Tab::Classes, ""));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(shift, Key::F))) {
            self.open_find(None);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::CTRL, Key::Tab))) && !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
        // PhpStorm "Search Everywhere": Shift pressed twice, nothing in between.
        let (time, events) = ctx.input(|i| (i.time, i.events.clone()));
        for e in events {
            match e {
                egui::Event::Key { key: Key::ShiftLeft | Key::ShiftRight, pressed: true, repeat: false, .. } => {
                    if self.shift_tap.is_some_and(|t0| time - t0 < 0.4) {
                        self.palette = Some(search::Everywhere::new(search::Tab::All, ""));
                        self.shift_tap = None;
                    } else {
                        self.shift_tap = Some(time);
                    }
                }
                egui::Event::Key { key: Key::ShiftLeft | Key::ShiftRight, .. } => {}
                egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::PointerButton { .. } => self.shift_tap = None,
                _ => {}
            }
        }
    }
}
