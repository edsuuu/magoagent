use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Left {
    Project,
    Commit,
    History,
    Database,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ToolWindow {
    Problems,
    Find,
}

pub(super) struct Tab {
    pub(super) path: PathBuf,
    pub(super) text: String,
    pub(super) lang: &'static Lang,
    pub(super) crlf: bool,
    pub(super) dirty: bool,
    pub(super) mtime: Option<SystemTime>,
    /// Its file went away on disk (the 2 s check): the tab stays, marked, and saving writes it back.
    pub(super) gone: bool,
    pub(super) cache: Option<(u64, LayoutJob)>,
    pub(super) cursor: (usize, usize), // 0-based line, col
    pub(super) sel: (usize, usize),    // char range
    pub(super) select: Option<(usize, usize)>,
    pub(super) scroll_to: Option<usize>,
    pub(super) lsp_pending: Option<Instant>,
    /// set for a diff tab (HEAD vs working copy)
    pub(super) diff: Option<Box<git::Diff>>,
    /// set for a table's data or an SQL console (`path` is the database)
    pub(super) sheet: Option<Box<crate::db::Sheet>>,
    /// set for a picture or a PDF, shown instead of text
    pub(super) media: Option<Box<media::Media>>,
    /// HEAD content for gutter change markers (None = not loaded yet)
    pub(super) head: Option<Option<String>>,
    pub(super) marks: Option<(u64, Vec<git::Mark>)>,
    /// Code Vision per declaration line, keyed by (text hash, index generation)
    pub(super) vision: Option<((u64, u64), Vec<(usize, String, String)>)>,
    /// Code Author per declaration line, keyed by (text hash, blame generation)
    pub(super) authors: Option<((u64, u64), Vec<(usize, String)>)>,
    /// parameter hints, keyed by (text hash, index generation, answer generation)
    pub(super) inlay: Option<((u64, u64, u64), Vec<hints::Hint>)>,
}

/// The biggest text file the editor opens.
pub(super) const MAX_TEXT: u64 = 256 << 10;

impl Tab {
    pub(super) fn new(path: PathBuf, text: String, crlf: bool) -> Self {
        let lang = highlight::detect(&path);
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        Tab {
            path,
            text,
            lang,
            crlf,
            mtime,
            dirty: false,
            gone: false,
            cache: None,
            cursor: (0, 0),
            sel: (0, 0),
            select: None,
            scroll_to: None,
            lsp_pending: None,
            diff: None,
            sheet: None,
            media: None,
            head: None,
            marks: None,
            vision: None,
            authors: None,
            inlay: None,
        }
    }

    /// A file being edited, not a diff, a table or a picture.
    pub(super) fn is_text(&self) -> bool {
        self.diff.is_none() && self.sheet.is_none() && self.media.is_none()
    }

    /// A file the card's panel shows: text, a picture or a PDF.
    pub(super) fn in_panel(&self) -> bool {
        self.diff.is_none() && self.sheet.is_none()
    }

    /// Unsaved text whose file was rewritten on disk meanwhile: the person picks which one stays.
    pub(super) fn conflict(&self) -> bool {
        self.is_text() && self.dirty && self.mtime.is_some() && std::fs::metadata(&self.path).and_then(|m| m.modified()).is_ok_and(|m| Some(m) != self.mtime)
    }

    /// The file's text from disk again, the tab clean: it follows the disk, or "Recarregar do disco".
    pub(super) fn reload(&mut self) {
        let Ok(s) = std::fs::read_to_string(&self.path) else { return };
        self.crlf = s.contains("\r\n");
        self.text = if self.crlf { s.replace("\r\n", "\n") } else { s };
        self.mtime = std::fs::metadata(&self.path).and_then(|m| m.modified()).ok();
        (self.dirty, self.lsp_pending) = (false, Some(Instant::now()));
    }
}

#[derive(Clone)]
pub(crate) enum Cmd {
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
    pub(super) ctx: egui::Context,
    /// Root of every id the editor uses, so two IDE nodes never share widget state.
    pub(super) id: Id,
    /// The folder as the node asked for it (the glue reopens the IDE when it changes).
    pub(crate) folder: String,
    pub(crate) root: Option<PathBuf>,
    pub(super) tree: Option<TreeNode>,
    pub(super) files: Vec<PathBuf>,
    pub(super) files_rx: Option<Receiver<Vec<PathBuf>>>,
    pub(crate) branch: String,
    pub(super) tabs: Vec<Tab>,
    pub(crate) active: usize,
    pub(super) left: Option<Left>,
    pub(super) claude: Option<claude::Claude>,
    pub(super) claude_sel: Option<(PathBuf, (usize, usize))>,
    pub(super) git: git::Git,
    pub(super) toast: Option<(String, Instant)>,
    /// The last message, for the canvas toast while the IDE isn't on screen.
    pub(crate) notice: Option<String>,
    pub(super) bottom: Option<ToolWindow>,
    pub(super) db: crate::db::Db,
    pub(super) palette: Option<search::Everywhere>,
    pub(super) find_files: Option<find::Find>,
    pub(super) find_results: Option<(String, Vec<find::Hit>)>,
    /// Folders left out of the file list, the indexes and the text search
    pub(super) excluded: excluded::Excluded,
    /// the project's text, for Search Everywhere's Text tab and Find in Files
    pub(super) text_index: std::sync::Arc<textindex::TextIndex>,
    pub(super) text_rx: Option<Receiver<textindex::TextIndex>>,
    /// move when the text index or the file list does, for the searches that cache on them
    pub(super) text_gen: u64,
    pub(super) files_gen: u64,
    pub(super) indexed_at: Instant,
    pub(super) index: index::Index,
    pub(super) index_rx: Option<Receiver<index::Index>>,
    pub(super) index_gen: u64,
    pub(super) code_vision: bool,
    /// `git blame` per file (None = an uncommitted line), filled by `blame_request`
    pub(super) blame: std::collections::HashMap<PathBuf, Vec<Option<String>>>,
    pub(super) blame_rx: Option<Receiver<(PathBuf, Vec<Option<String>>)>>,
    pub(super) blame_gen: u64,
    pub(super) find: Option<String>,
    /// Ctrl+F was just pressed: the find bar's field takes the keyboard (the editor has it until then).
    pub(super) find_focus: bool,
    pub(super) insp: Option<lsp::Inspections>,
    /// the Quick Documentation popup, while the pointer rests on a name or is on the popup
    pub(super) doc: Option<docs::Popup>,
    pub(crate) status: String,
    pub(super) font_size: f32,
    pub(crate) unified_diff: bool,
    pub(super) focused: bool,
    pub(super) shift_tap: Option<f64>,
    pub(super) last_disk_check: Instant,
    /// navigation history for Back/Forward (path, line, col)
    pub(super) back: Vec<(PathBuf, usize, usize)>,
    pub(super) forward: Vec<(PathBuf, usize, usize)>,
    pub(super) decl_rx: Option<Receiver<Option<(PathBuf, usize, usize)>>>,
    /// word being resolved, for the native fallback when the language server has no answer
    pub(super) decl_word: Option<String>,
    /// the word whose usages or implementations were asked for, and the line a signature hint
    /// belongs to (it goes away when the caret leaves that line)
    pub(super) usage_word: String,
    pub(super) sig_line: Option<(PathBuf, usize)>,
    /// Where the IDE is on screen this frame (its popups center on it).
    pub(super) rect: Rect,
    /// Asks of the canvas: go back to it, or open a Claude Code terminal roped to this node.
    pub(crate) wants_canvas: bool,
    pub(crate) wants_claude: bool,
    /// prompts for the agents roped to this IDE (a comment on a change); the canvas sends them
    pub(crate) to_agent: Vec<String>,
    pub(crate) code_comment: Option<(PathBuf, usize, usize, String)>,
    pub(crate) note_req: Option<crate::app::chat::code_comment::NoteReq>,
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
    pub(super) tree_keys: bool,
    /// A name or a confirmation the tree is waiting for (ide/fileops.rs).
    pub(crate) file_ask: Option<FileAsk>,
    /// Lines an agent opened or changed (file, first, last), lit in the card's panel.
    pub(crate) panel_mark: Option<(PathBuf, usize, usize)>,
    /// The panel's colored text, by its text, language and zoom: laid out again only when one changes.
    pub(crate) panel_job: std::cell::RefCell<Option<(u64, LayoutJob)>>,
    /// Where `recover` keeps unsaved text that could not be written (None in tests: never the real data).
    pub(super) recovery: Option<PathBuf>,
    /// The app's close was held once to show where unsaved text went.
    pub(super) close_warned: bool,
}

impl Drop for Ide {
    /// The node was removed, its folder changed or the app is closing.
    fn drop(&mut self) {
        self.recover();
    }
}
