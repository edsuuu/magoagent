//! MagoAgent no terminal: o canvas inteiro dentro de um terminal só. Vários terminais (shells de
//! verdade, com o Claude Code ou o Codex dentro) ficam soltos num quadro infinito que você arrasta;
//! cada um se move, muda de tamanho e amplia. A disposição fica salva num JSON e volta na próxima vez.
//!
//! Teclas: no canvas, `9` + clique abre um terminal, Enter entra no de cima, `z` amplia, Tab passa
//! para o próximo, setas movem o canvas e Ctrl+Q sai. Dentro de um terminal tudo vai para ele, menos
//! Ctrl+Espaço, que volta ao canvas.

use alacritty_terminal::event::{Event as TermEvent, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term, TermMode, cell::Flags, point_to_viewport, test::TermSize};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color as TermColor, CursorShape, NamedColor, Rgb};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// The star on the app's icon: the focused terminal and the active tool.
const AMBER: Color = Color::Rgb(0xF0, 0xA0, 0x20);
const BAR: Color = Color::Indexed(235);
/// A new terminal's grid, and the smallest window a drag leaves.
const NEW_GRID: (i32, i32) = (80, 24);
const MIN_WINDOW: (i32, i32) = (12, 4);
/// The toolbar takes the first row; the canvas starts under it.
const TOP: i32 = 1;

fn main() -> std::io::Result<()> {
    // started from inside a Claude Code session: the `claude` in our terminals would run as its child
    if std::env::var_os("CLAUDECODE").is_some() {
        for (key, _) in std::env::vars_os() {
            if key.to_str().is_some_and(|k| k == "CLAUDECODE" || k.starts_with("CLAUDE_CODE_")) {
                // SAFETY: first thing in main, before any other thread starts
                unsafe { std::env::remove_var(key) };
            }
        }
    }
    let path = std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(default_path);
    let (tx, rx) = mpsc::channel();
    let mut app = App::new(path, tx.clone());
    app.load();
    let mut terminal = ratatui::init();
    execute!(std::io::stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    let restore = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(std::io::stdout(), DisableMouseCapture, DisableBracketedPaste);
        restore(info);
    }));
    std::thread::spawn(move || {
        while let Ok(event) = event::read() {
            if tx.send(Ev::Input(event)).is_err() {
                break;
            }
        }
    });
    while !app.quit {
        terminal.draw(|frame| app.draw(frame))?;
        let Ok(ev) = rx.recv() else { break };
        app.handle(ev);
        // ponytail: one frame per batch of events, add a frame cap if a flood of output ever lags typing
        while let Ok(ev) = rx.try_recv() {
            app.handle(ev);
        }
        app.reap();
    }
    app.save();
    drop(app); // the shells end with their windows
    execute!(std::io::stdout(), DisableMouseCapture, DisableBracketedPaste)?;
    ratatui::restore();
    Ok(())
}

/// Where the canvas is kept: next to the window app's data (Application Support, %APPDATA% or
/// ~/.local/share), in a file of its own. A path on the command line opens another canvas.
fn default_path() -> PathBuf {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library/Application Support/MagoAgent"))
    } else if cfg!(windows) {
        var("APPDATA").map(|appdata| appdata.join("MagoAgent"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|home| home.join(".local/share"))).map(|dir| dir.join("magoagent"))
    };
    dir.unwrap_or_default().join("canvas-terminal.json")
}

enum Ev {
    Input(Event),
    /// A terminal printed, changed its title or ended.
    Wake,
}

// ---------------------------------------------------------------------------------------------
// One terminal: a shell in a pseudo-terminal (alacritty_terminal does the pty and the emulation)

#[derive(Clone)]
struct Listener {
    wake: mpsc::Sender<Ev>,
    pty: Arc<Mutex<Option<EventLoopSender>>>,
    title: Arc<Mutex<Option<String>>>,
    exited: Arc<AtomicBool>,
}

impl Listener {
    fn reply(&self, text: String) {
        if let Some(pty) = &*self.pty.lock().unwrap() {
            let _ = pty.send(Msg::Input(text.into_bytes().into()));
        }
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: TermEvent) {
        match event {
            // replies the program asked for (cursor position, device attributes…)
            TermEvent::PtyWrite(text) => self.reply(text),
            // OSC 4/10/11 queries: programs pick their colors from the terminal's
            TermEvent::ColorRequest(index, format) => {
                let [r, g, b] = match index {
                    257 => ANSI[0],
                    i if i < 256 => indexed(i),
                    _ => ANSI[7],
                };
                self.reply(format(Rgb { r, g, b }));
            }
            TermEvent::Title(title) => *self.title.lock().unwrap() = Some(title),
            TermEvent::ResetTitle => *self.title.lock().unwrap() = None,
            TermEvent::ChildExit(_) | TermEvent::Exit => self.exited.store(true, Ordering::Relaxed),
            _ => {}
        }
        let _ = self.wake.send(Ev::Wake);
    }
}

/// A running shell. Dropping it ends the shell.
struct Pane {
    term: Arc<FairMutex<Term<Listener>>>,
    pty: EventLoopSender,
    title: Arc<Mutex<Option<String>>>,
    exited: Arc<AtomicBool>,
    size: (usize, usize),
    /// The folder it started in.
    cwd: PathBuf,
    #[cfg(target_os = "linux")]
    pid: u32,
}

fn window_size((cols, rows): (usize, usize)) -> WindowSize {
    WindowSize { num_lines: rows as u16, num_cols: cols as u16, cell_width: 8, cell_height: 16 }
}

impl Pane {
    /// The user's login shell in `cwd` (empty: home), as the window app opens it; or `program` (tests).
    fn spawn(cwd: &str, size: (usize, usize), wake: mpsc::Sender<Ev>, program: Option<(&str, &[&str])>) -> std::io::Result<Self> {
        let home = std::env::home_dir();
        let cwd = match cwd.trim().strip_prefix('~') {
            _ if cwd.trim().is_empty() => home,
            Some(rest) => home.map(|h| h.join(rest.trim_start_matches(['/', '\\']))),
            None => Some(PathBuf::from(cwd.trim())),
        };
        // Windows: PowerShell 7 when it's installed, else alacritty's default (Windows PowerShell)
        let login = if cfg!(windows) {
            let path = std::env::var_os("PATH").unwrap_or_default();
            std::env::split_paths(&path).any(|dir| dir.join("pwsh.exe").is_file()).then(|| tty::Shell::new("pwsh".into(), vec![]))
        } else {
            let fallback = if cfg!(target_os = "macos") { "/bin/zsh" } else { "/bin/sh" };
            let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.into());
            Some(tty::Shell::new(shell, vec!["-l".into()]))
        };
        let shell = program.map(|(p, args)| tty::Shell::new(p.into(), args.iter().map(|a| a.to_string()).collect())).or(login);
        let env = [("TERM", "xterm-256color"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "MagoAgent")].map(|(k, v)| (k.to_string(), v.to_string())).into();
        let cwd = cwd.filter(|dir| dir.is_dir()).or_else(|| std::env::current_dir().ok());
        let options = tty::Options { shell, working_directory: cwd.clone(), env, ..Default::default() };
        let pty = tty::new(&options, window_size(size), 0)?;
        #[cfg(target_os = "linux")]
        let pid = pty.child().id();
        let listener = Listener { wake, pty: Default::default(), title: Default::default(), exited: Default::default() };
        let term = Arc::new(FairMutex::new(Term::new(Config::default(), &TermSize::new(size.0, size.1), listener.clone())));
        // drain on exit: on Linux the child's exit can land before its last output is read
        let event_loop = EventLoop::new(term.clone(), listener.clone(), pty, true, false)?;
        let sender = event_loop.channel();
        *listener.pty.lock().unwrap() = Some(sender.clone());
        event_loop.spawn();
        Ok(Self {
            term,
            pty: sender,
            title: listener.title,
            exited: listener.exited,
            size,
            cwd: cwd.unwrap_or_default(),
            #[cfg(target_os = "linux")]
            pid,
        })
    }

    fn write(&self, bytes: Vec<u8>) {
        self.term.lock().scroll_display(Scroll::Bottom);
        let _ = self.pty.send(Msg::Input(bytes.into()));
    }

    fn mode(&self) -> TermMode {
        *self.term.lock().mode()
    }

    fn resize(&mut self, size: (usize, usize)) {
        if size != self.size {
            self.size = size;
            self.term.lock().resize(TermSize::new(size.0, size.1));
            let _ = self.pty.send(Msg::Resize(window_size(size)));
        }
    }

    /// Positive scrolls up into the history.
    fn scroll(&self, lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(lines));
    }

    /// The folder the shell is in now (Linux reads it from /proc), else the one it started in.
    // ponytail: only Linux follows `cd`, macOS/Windows would need libproc / the process's PEB
    fn cwd(&self) -> PathBuf {
        #[cfg(target_os = "linux")]
        if let Ok(dir) = std::fs::read_link(format!("/proc/{}/cwd", self.pid)) {
            return dir;
        }
        self.cwd.clone()
    }

    /// The program's title (OSC 0/2), else the folder's name.
    fn title(&self) -> String {
        let folder = || self.cwd.file_name().map_or_else(|| self.cwd.display().to_string(), |n| n.to_string_lossy().into());
        self.title.lock().unwrap().clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(folder)
    }

    /// Paints the grid with its top-left cell at (x, y) on the screen; where the cursor is, when it shows.
    fn paint(&self, buf: &mut Buffer, x: i32, y: i32) -> Option<(i32, i32)> {
        let term = self.term.lock();
        let content = term.renderable_content();
        for cell in content.display_iter {
            let Some(p) = point_to_viewport(content.display_offset, cell.point) else { continue };
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let mut symbol = String::from(cell.c);
            symbol.extend(cell.zerowidth().into_iter().flatten());
            let mut style = Style::default().fg(color(cell.fg)).bg(color(cell.bg));
            for (flag, modifier) in [
                (Flags::BOLD, Modifier::BOLD),
                (Flags::ITALIC, Modifier::ITALIC),
                (Flags::DIM, Modifier::DIM),
                (Flags::ALL_UNDERLINES, Modifier::UNDERLINED),
                (Flags::INVERSE, Modifier::REVERSED),
                (Flags::HIDDEN, Modifier::HIDDEN),
                (Flags::STRIKEOUT, Modifier::CROSSED_OUT),
            ] {
                if cell.flags.intersects(flag) {
                    style = style.add_modifier(modifier);
                }
            }
            put(buf, x + p.column.0 as i32, y + p.line as i32, &symbol, style);
        }
        let cursor = content.cursor;
        let at = (cursor.shape != CursorShape::Hidden).then(|| point_to_viewport(content.display_offset, cursor.point)).flatten();
        at.map(|p| (x + p.column.0 as i32, y + p.line as i32))
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        let _ = self.pty.send(Msg::Shutdown);
    }
}

/// Alacritty's default palette (normal, then bright): only for programs that ask for the colors;
/// the cells themselves use the outer terminal's own palette.
const ANSI: [[u8; 3]; 16] = [
    [0x18, 0x18, 0x18],
    [0xac, 0x42, 0x42],
    [0x90, 0xa9, 0x59],
    [0xf4, 0xbf, 0x75],
    [0x6a, 0x9f, 0xb5],
    [0xaa, 0x75, 0x9f],
    [0x75, 0xb5, 0xaa],
    [0xd8, 0xd8, 0xd8],
    [0x6b, 0x6b, 0x6b],
    [0xc5, 0x55, 0x55],
    [0xaa, 0xc4, 0x74],
    [0xfe, 0xca, 0x88],
    [0x82, 0xb8, 0xc8],
    [0xc2, 0x8c, 0xb8],
    [0x93, 0xd3, 0xc3],
    [0xf8, 0xf8, 0xf8],
];

fn indexed(i: usize) -> [u8; 3] {
    match i {
        0..16 => ANSI[i],
        16..232 => {
            let level = |n: usize| if n == 0 { 0 } else { (55 + n * 40) as u8 };
            let i = i - 16;
            [level(i / 36), level(i / 6 % 6), level(i % 6)]
        }
        _ => [(8 + (i.min(255) - 232) * 10) as u8; 3],
    }
}

/// A cell's color in the outer terminal: its default colors and its palette, so the panes follow
/// the theme the person already uses.
fn color(color: TermColor) -> Color {
    match color {
        TermColor::Spec(c) => Color::Rgb(c.r, c.g, c.b),
        TermColor::Indexed(i) => Color::Indexed(i),
        TermColor::Named(name) if (name as usize) < 16 => Color::Indexed(name as u8),
        TermColor::Named(name) if (NamedColor::DimBlack..=NamedColor::DimWhite).contains(&name) => {
            Color::Indexed((name as usize - NamedColor::DimBlack as usize) as u8)
        }
        TermColor::Named(_) => Color::Reset,
    }
}

/// Bytes a key sends to the program (xterm's sequences, as the window app sends them).
fn key_bytes(key: KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    let m = key.modifiers;
    let (shift, alt, ctrl) = (m.contains(KeyModifiers::SHIFT), m.contains(KeyModifiers::ALT), m.contains(KeyModifiers::CONTROL));
    let code = 1 + shift as u8 + 2 * alt as u8 + 4 * ctrl as u8;
    let cursor = |c: char| match (code, app_cursor) {
        (1, true) => format!("\x1bO{c}"),
        (1, false) => format!("\x1b[{c}"),
        _ => format!("\x1b[1;{code}{c}"),
    };
    let tilde = |n: u8| if code > 1 { format!("\x1b[{n};{code}~") } else { format!("\x1b[{n}~") };
    let word = alt && !ctrl && !shift;
    let text = match key.code {
        KeyCode::Left if word => "\x1bb".into(),
        KeyCode::Right if word => "\x1bf".into(),
        KeyCode::Up => cursor('A'),
        KeyCode::Down => cursor('B'),
        KeyCode::Right => cursor('C'),
        KeyCode::Left => cursor('D'),
        KeyCode::Home => cursor('H'),
        KeyCode::End => cursor('F'),
        // Shift/Alt+Enter: a new line in agent prompts instead of sending
        KeyCode::Enter if shift || alt => "\x1b\r".into(),
        KeyCode::Enter => "\r".into(),
        KeyCode::Backspace if alt => "\x1b\x7f".into(),
        KeyCode::Backspace if ctrl => "\x08".into(),
        KeyCode::Backspace => "\x7f".into(),
        KeyCode::BackTab => "\x1b[Z".into(),
        KeyCode::Tab if shift => "\x1b[Z".into(),
        KeyCode::Tab => "\t".into(),
        KeyCode::Esc => "\x1b".into(),
        KeyCode::Insert => tilde(2),
        KeyCode::Delete => tilde(3),
        KeyCode::PageUp => tilde(5),
        KeyCode::PageDown => tilde(6),
        KeyCode::F(n @ 1..=4) => format!("\x1bO{}", (b'P' + n - 1) as char),
        KeyCode::F(n @ 5..=12) => tilde([15, 17, 18, 19, 20, 21, 23, 24][n as usize - 5]),
        // AltGr comes as Ctrl+Alt on Windows, with the character it typed
        KeyCode::Char(c) if ctrl && alt && cfg!(windows) => c.to_string(),
        KeyCode::Char(c) if ctrl => {
            // Unix terminals send Ctrl+\ ] ^ _ as the bytes of Ctrl+4..7, and crossterm reads them so
            let byte = match c {
                ' ' | '@' | '2' => 0,
                '[' | '3' => 0x1b,
                '\\' | '4' => 0x1c,
                ']' | '5' => 0x1d,
                '^' | '6' => 0x1e,
                '-' | '_' | '/' | '7' => 0x1f,
                '?' | '8' => 0x7f,
                c if c.is_ascii_alphabetic() => c.to_ascii_uppercase() as u8 - b'A' + 1,
                _ => return None,
            };
            return Some(if alt { vec![0x1b, byte] } else { vec![byte] });
        }
        KeyCode::Char(c) if alt => format!("\x1b{c}"),
        KeyCode::Char(c) => c.to_string(),
        _ => return None,
    };
    Some(text.into_bytes())
}

/// Pasted text: line breaks as Enter, inside bracketed-paste markers when the program asked for them.
fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if bracketed {
        // an ESC inside could end the paste early and run the rest as typed keys
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.into_bytes()
    }
}

// ---------------------------------------------------------------------------------------------
// The canvas

/// A box in cells: world cells for a window on the canvas, screen cells once placed.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
struct Area {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Area {
    fn contains(&self, x: i32, y: i32) -> bool {
        (self.x..self.x + self.w).contains(&x) && (self.y..self.y + self.h).contains(&y)
    }
}

/// A terminal on the canvas: its box (border included) and its shell.
struct Win {
    area: Area,
    pane: Pane,
}

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    camera: (i32, i32),
    terminals: Vec<SavedTerminal>,
}

#[derive(Serialize, Deserialize)]
struct SavedTerminal {
    #[serde(flatten)]
    area: Area,
    cwd: String,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Hand,
    Select,
    Terminal,
}

/// The toolbar, in the window app's order and with its shortcuts (letter, then number); the
/// terminal has no key there, here it takes the free 9.
const TOOLS: [(Tool, &str, char, &str); 3] = [(Tool::Hand, "h", '✥', "Mão"), (Tool::Select, "v1", '↖', "Seleção"), (Tool::Terminal, "9", '▣', "Terminal")];
/// Every toolbar button is " {key}{icon} ".
const BUTTON: i32 = 4;

enum Drag {
    None,
    Pan { from: (i32, i32), camera: (i32, i32) },
    Move { grab: (i32, i32) },
    Resize { right: bool, bottom: bool },
}

struct App {
    path: PathBuf,
    /// The world cell at the canvas's top-left corner.
    camera: (i32, i32),
    /// Bottom to top: the last one is the current terminal.
    wins: Vec<Win>,
    /// Keys go to the current terminal.
    typing: bool,
    /// The current terminal fills the screen.
    zoomed: bool,
    tool: Tool,
    drag: Drag,
    last_click: (Instant, i32, i32),
    screen: (i32, i32),
    notice: Option<String>,
    wake: mpsc::Sender<Ev>,
    quit: bool,
}

impl App {
    fn new(path: PathBuf, wake: mpsc::Sender<Ev>) -> Self {
        Self {
            path,
            camera: (0, 0),
            wins: Vec::new(),
            typing: false,
            zoomed: false,
            tool: Tool::Select,
            drag: Drag::None,
            last_click: (Instant::now() - Duration::from_secs(10), -1, -1),
            screen: (80, 24),
            notice: None,
            wake,
            quit: false,
        }
    }

    /// The saved canvas, its terminals opened again in their folders; the first time, one terminal.
    fn load(&mut self) {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(_) => {
                self.open(Area { x: 2, y: 1, w: NEW_GRID.0 + 2, h: NEW_GRID.1 + 2 }, "", None);
                return;
            }
        };
        match serde_json::from_str::<Saved>(&text) {
            Ok(saved) => {
                self.camera = saved.camera;
                for t in saved.terminals {
                    self.open(t.area, &t.cwd, None);
                }
            }
            Err(e) => {
                // never write over a canvas it couldn't read: set it aside
                let aside = self.path.with_extension("ilegivel.json");
                let _ = std::fs::rename(&self.path, &aside);
                self.notice = Some(format!("o canvas salvo não abriu ({e}); ficou em {}", aside.display()));
            }
        }
    }

    fn save(&mut self) {
        let terminals = self.wins.iter().map(|w| SavedTerminal { area: w.area, cwd: w.pane.cwd().to_string_lossy().into() }).collect();
        let saved = Saved { camera: self.camera, terminals };
        let tmp = self.path.with_extension("tmp");
        let written = self.path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| {
            std::fs::write(&tmp, serde_json::to_string_pretty(&saved).unwrap_or_default())?;
            std::fs::rename(&tmp, &self.path)
        });
        if let Err(e) = written {
            self.notice = Some(format!("não salvou o canvas em {}: {e}", self.path.display()));
        }
    }

    fn open(&mut self, area: Area, cwd: &str, program: Option<(&str, &[&str])>) {
        let grid = ((area.w - 2).max(2) as usize, (area.h - 2).max(1) as usize);
        match Pane::spawn(cwd, grid, self.wake.clone(), program) {
            Ok(pane) => self.wins.push(Win { area, pane }),
            Err(e) => self.notice = Some(format!("o terminal não abriu: {e}")),
        }
    }

    /// Windows whose shell ended close.
    fn reap(&mut self) {
        let before = self.wins.len();
        self.wins.retain(|w| !w.pane.exited.load(Ordering::Relaxed));
        if self.wins.len() != before {
            self.typing &= !self.wins.is_empty();
            self.zoomed &= self.typing;
            self.save();
        }
    }

    fn world(&self, x: i32, y: i32) -> (i32, i32) {
        (x + self.camera.0, y - TOP + self.camera.1)
    }

    /// Window `i` on the screen: where the canvas puts it, or all of the canvas when it's zoomed.
    fn screen_area(&self, i: usize) -> Area {
        if self.zoomed && i + 1 == self.wins.len() {
            return Area { x: 0, y: TOP, w: self.screen.0, h: self.screen.1 - TOP };
        }
        let a = self.wins[i].area;
        Area { x: a.x - self.camera.0, y: a.y - self.camera.1 + TOP, ..a }
    }

    fn window_at(&self, x: i32, y: i32) -> Option<usize> {
        (0..self.wins.len()).rev().find(|&i| self.screen_area(i).contains(x, y))
    }

    fn raise(&mut self, i: usize) {
        let w = self.wins.remove(i);
        self.wins.push(w);
    }

    /// The canvas moves so the current terminal sits in the middle.
    fn center(&mut self) {
        if let Some(w) = self.wins.last() {
            self.camera = (w.area.x + w.area.w / 2 - self.screen.0 / 2, w.area.y + w.area.h / 2 - (self.screen.1 - TOP) / 2);
        }
    }

    fn handle(&mut self, ev: Ev) {
        match ev {
            Ev::Input(Event::Key(key)) if key.kind != KeyEventKind::Release => self.key(key),
            Ev::Input(Event::Mouse(mouse)) => self.mouse(mouse),
            Ev::Input(Event::Paste(text)) if self.typing => {
                if let Some(w) = self.wins.last() {
                    w.pane.write(paste_bytes(&text, w.pane.mode().contains(TermMode::BRACKETED_PASTE)));
                }
            }
            _ => {}
        }
    }

    fn key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char(' ' | '\0')) {
            (self.typing, self.zoomed) = (false, false);
            return;
        }
        if self.typing {
            if let Some(w) = self.wins.last()
                && let Some(bytes) = key_bytes(key, w.pane.mode().contains(TermMode::APP_CURSOR))
            {
                w.pane.write(bytes);
            }
            return;
        }
        self.notice = None;
        match key.code {
            KeyCode::Char('q' | 'Q') if ctrl => self.quit = true,
            KeyCode::Enter if !self.wins.is_empty() => self.typing = true,
            KeyCode::Char('z') if !self.wins.is_empty() => (self.zoomed, self.typing) = (true, true),
            KeyCode::Tab if !self.wins.is_empty() => {
                self.raise(0);
                self.center();
            }
            KeyCode::Left => self.camera.0 -= 8,
            KeyCode::Right => self.camera.0 += 8,
            KeyCode::Up => self.camera.1 -= 4,
            KeyCode::Down => self.camera.1 += 4,
            KeyCode::Esc => self.tool = Tool::Select,
            KeyCode::Char(c) if !ctrl => {
                if let Some(&(tool, ..)) = TOOLS.iter().find(|t| t.1.contains(c.to_ascii_lowercase())) {
                    self.tool = tool;
                }
            }
            _ => {}
        }
    }

    fn mouse(&mut self, m: MouseEvent) {
        let (x, y) = (m.column as i32, m.row as i32);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => self.press(x, y),
            MouseEventKind::Down(MouseButton::Middle) => self.drag = Drag::Pan { from: (x, y), camera: self.camera },
            MouseEventKind::Drag(_) => self.drag_to(x, y),
            MouseEventKind::Up(_) => {
                if !matches!(self.drag, Drag::None) {
                    self.drag = Drag::None;
                    self.save();
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let up = matches!(m.kind, MouseEventKind::ScrollUp);
                match self.window_at(x, y) {
                    Some(i) => self.wins[i].pane.scroll(if up { 3 } else { -3 }),
                    None if m.modifiers.contains(KeyModifiers::SHIFT) => self.camera.0 += if up { -6 } else { 6 },
                    None => self.camera.1 += if up { -3 } else { 3 },
                }
            }
            MouseEventKind::ScrollLeft => self.camera.0 -= 6,
            MouseEventKind::ScrollRight => self.camera.0 += 6,
            _ => {}
        }
    }

    fn press(&mut self, x: i32, y: i32) {
        self.notice = None;
        let double = self.last_click.0.elapsed() < Duration::from_millis(400) && (self.last_click.1, self.last_click.2) == (x, y);
        self.last_click = (Instant::now(), x, y);
        if y < TOP {
            if let Some(&(tool, ..)) = TOOLS.get((x / BUTTON) as usize) {
                self.tool = tool;
            }
            return;
        }
        if self.tool == Tool::Hand {
            self.drag = Drag::Pan { from: (x, y), camera: self.camera };
            return;
        }
        if let Some(i) = self.window_at(x, y) {
            self.raise(i);
            let a = self.screen_area(self.wins.len() - 1);
            let (right, bottom) = (x == a.x + a.w - 1, y == a.y + a.h - 1);
            if y == a.y && double {
                // the title's double click zooms in and back, as on the window app
                self.zoomed = !self.zoomed;
                self.typing = self.zoomed;
            } else if self.zoomed {
                self.typing = true;
            } else if right || bottom {
                self.drag = Drag::Resize { right, bottom };
            } else if y == a.y || x == a.x {
                self.drag = Drag::Move { grab: (x - a.x, y - a.y) };
            } else {
                self.typing = true;
            }
            return;
        }
        self.typing = false;
        let at = self.world(x, y);
        match self.tool {
            Tool::Terminal => {
                self.open(Area { x: at.0, y: at.1, w: NEW_GRID.0 + 2, h: NEW_GRID.1 + 2 }, "", None);
                self.typing = true;
                self.tool = Tool::Select;
                self.save();
            }
            _ => self.drag = Drag::Pan { from: (x, y), camera: self.camera },
        }
    }

    fn drag_to(&mut self, x: i32, y: i32) {
        let at = self.world(x, y);
        let Some(top) = self.wins.len().checked_sub(1) else {
            if let Drag::Pan { from, camera } = self.drag {
                self.camera = (camera.0 - (x - from.0), camera.1 - (y - from.1));
            }
            return;
        };
        match self.drag {
            Drag::None => {}
            Drag::Pan { from, camera } => self.camera = (camera.0 - (x - from.0), camera.1 - (y - from.1)),
            Drag::Move { grab } => {
                let area = &mut self.wins[top].area;
                (area.x, area.y) = (at.0 - grab.0, at.1 - grab.1);
            }
            Drag::Resize { right, bottom } => {
                let area = &mut self.wins[top].area;
                if right {
                    area.w = (at.0 - area.x + 1).max(MIN_WINDOW.0);
                }
                if bottom {
                    area.h = (at.1 - area.y + 1).max(MIN_WINDOW.1);
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let size = frame.area();
        self.screen = (size.width as i32, size.height as i32);
        for i in 0..self.wins.len() {
            let a = self.screen_area(i);
            self.wins[i].pane.resize(((a.w - 2).max(2) as usize, (a.h - 2).max(1) as usize));
        }
        let buf = frame.buffer_mut();
        if !self.zoomed {
            // a dot every few cells: the canvas, and how far it moved
            for y in TOP..self.screen.1 {
                for x in 0..self.screen.0 {
                    let (wx, wy) = self.world(x, y);
                    if wx.rem_euclid(8) == 0 && wy.rem_euclid(4) == 0 {
                        put(buf, x, y, "·", Style::default().fg(Color::DarkGray));
                    }
                }
            }
        }
        let mut cursor = None;
        let first = if self.zoomed { self.wins.len().saturating_sub(1) } else { 0 };
        for i in first..self.wins.len() {
            let a = self.screen_area(i);
            if a.x >= self.screen.0 || a.y >= self.screen.1 || a.x + a.w <= 0 || a.y + a.h <= TOP {
                continue;
            }
            let current = i + 1 == self.wins.len();
            let border = match (current, self.typing) {
                (true, true) => Style::default().fg(AMBER),
                (true, false) => Style::default(),
                _ => Style::default().fg(Color::DarkGray),
            };
            frame_box(buf, a, border);
            let title: String = format!(" {} ", self.wins[i].pane.title()).chars().take((a.w - 4).max(0) as usize).collect();
            put_str(buf, a.x + 2, a.y, &title, border);
            let at = self.wins[i].pane.paint(buf, a.x + 1, a.y + 1);
            if current && self.typing {
                cursor = at;
            }
        }
        self.paint_toolbar(buf);
        if let Some((x, y)) = cursor
            && (0..self.screen.0).contains(&x)
            && (TOP..self.screen.1).contains(&y)
        {
            frame.set_cursor_position((x as u16, y as u16));
        }
    }

    fn paint_toolbar(&self, buf: &mut Buffer) {
        for x in 0..self.screen.0 {
            put(buf, x, 0, " ", Style::default().bg(BAR));
        }
        let mut x = 0;
        for (tool, keys, icon, _) in TOOLS {
            let style = if tool == self.tool { Style::default().bg(AMBER).fg(Color::Black) } else { Style::default().bg(BAR) };
            put_str(buf, x, 0, &format!(" {}{icon} ", keys.chars().last().unwrap_or(' ')), style);
            x += BUTTON;
        }
        let label = TOOLS.iter().find(|t| t.0 == self.tool).map_or("", |t| t.3);
        let hint = match (&self.notice, self.typing) {
            (Some(notice), _) => notice.clone(),
            (None, true) => "Ctrl+Espaço: voltar ao canvas".into(),
            (None, false) => format!("{label} · Enter: entrar no terminal · z: ampliar · Tab: próximo · Ctrl+Q: sair"),
        };
        let style = Style::default().bg(BAR).fg(if self.notice.is_some() { AMBER } else { Color::Gray });
        put_str(buf, x + 1, 0, &hint, style);
    }
}

fn put(buf: &mut Buffer, x: i32, y: i32, symbol: &str, style: Style) {
    let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else { return };
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.reset();
        cell.set_symbol(symbol).set_style(style);
    }
}

fn put_str(buf: &mut Buffer, x: i32, y: i32, text: &str, style: Style) {
    for (i, c) in text.chars().enumerate() {
        put(buf, x + i as i32, y, c.encode_utf8(&mut [0; 4]), style);
    }
}

/// A window's rounded border, its inside cleared.
fn frame_box(buf: &mut Buffer, a: Area, style: Style) {
    for y in a.y..a.y + a.h {
        for x in a.x..a.x + a.w {
            let (left, right, top, bottom) = (x == a.x, x == a.x + a.w - 1, y == a.y, y == a.y + a.h - 1);
            let symbol = match (left, right, top, bottom) {
                (true, _, true, _) => "╭",
                (_, true, true, _) => "╮",
                (true, _, _, true) => "╰",
                (_, true, _, true) => "╯",
                (_, _, true, _) | (_, _, _, true) => "─",
                (true, ..) | (_, true, ..) => "│",
                _ => " ",
            };
            put(buf, x, y, symbol, if symbol == " " { Style::default() } else { style });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Option<Vec<u8>> {
        key_bytes(KeyEvent::new(code, modifiers), false)
    }

    #[test]
    fn keys_become_the_bytes_a_terminal_expects() {
        assert_eq!(key(KeyCode::Char('c'), KeyModifiers::CONTROL), Some(vec![3]));
        assert_eq!(key(KeyCode::Char('x'), KeyModifiers::ALT), Some(b"\x1bx".to_vec()));
        assert_eq!(key(KeyCode::Enter, KeyModifiers::SHIFT), Some(b"\x1b\r".to_vec()));
        assert_eq!(key(KeyCode::Up, KeyModifiers::CONTROL), Some(b"\x1b[1;5A".to_vec()));
        assert_eq!(key_bytes(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), true), Some(b"\x1bOA".to_vec()));
        assert_eq!(key(KeyCode::F(5), KeyModifiers::NONE), Some(b"\x1b[15~".to_vec()));
        assert_eq!(key(KeyCode::F(2), KeyModifiers::NONE), Some(b"\x1bOQ".to_vec()));
        assert_eq!(key(KeyCode::Char('ç'), KeyModifiers::NONE), Some("ç".as_bytes().to_vec()));
    }

    /// The text of screen row `y`.
    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// The screen column where `text` starts in a row.
    fn column(row: &str, text: &str) -> Option<usize> {
        row.find(text).map(|byte| row[..byte].chars().count())
    }

    #[test]
    fn a_terminal_shows_its_output_where_it_sits_on_the_canvas() {
        let (tx, _rx) = mpsc::channel();
        let mut app = App::new(std::env::temp_dir().join("magoagent-tui-teste.json"), tx);
        #[cfg(windows)]
        let program: (&str, &[&str]) = ("cmd", &["/c", "echo oi-mago& ping -n 6 127.0.0.1 >nul"]);
        #[cfg(unix)]
        let program: (&str, &[&str]) = ("sh", &["-c", "printf oi-mago; sleep 5"]);
        app.open(Area { x: 5, y: 3, w: 30, h: 6 }, "", Some(program));
        let mut terminal = ratatui::Terminal::new(TestBackend::new(60, 16)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let found = loop {
            terminal.draw(|frame| app.draw(frame)).unwrap();
            // the window's top border on screen row 4 (the toolbar is row 0), its first line on row 5
            let line = row(terminal.backend().buffer(), 5);
            if line.contains("oi-mago") || Instant::now() > deadline {
                break line;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        assert_eq!(column(&found, "oi-mago"), Some(6), "inside the border, one cell right of the window's x: {found:?}");
        assert_eq!(column(&row(terminal.backend().buffer(), 4), "╭"), Some(5), "the border where the window is");
        // panning moves the window with the canvas
        app.camera = (2, 1);
        terminal.draw(|frame| app.draw(frame)).unwrap();
        assert_eq!(column(&row(terminal.backend().buffer(), 4), "oi-mago"), Some(4));
    }
}
