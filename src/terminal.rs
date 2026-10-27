//! Terminals on the canvas: a shell in a pseudo-terminal (alacritty_terminal does the pty on
//! every OS and the escape-sequence emulation), keys to bytes, and painting the grid.

use crate::prelude::*;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term, TermMode, cell::Flags, color::Colors, point_to_viewport, test::TermSize};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub(crate) const TERMINAL_FONT: f32 = 14.0;
/// One grid cell in world units: Hack advances 1233/2048 em.
pub(crate) const CELL: Vec2 = Vec2::new(TERMINAL_FONT * 1233.0 / 2048.0, 17.0);
/// Title strip on top (the handle that drags the terminal), folder strip at the bottom and
/// padding around the grid.
pub(crate) const TERMINAL_HEADER: f32 = 26.0;
pub(crate) const TERMINAL_FOOTER: f32 = 22.0;
pub(crate) const TERMINAL_PAD: f32 = 8.0;
pub(crate) const TERMINAL_BG: Color32 = Color32::from_rgb(0x18, 0x18, 0x18);
const FG: Color32 = Color32::from_rgb(0xd8, 0xd8, 0xd8);
/// Alacritty's default palette: normal colors, then bright.
const ANSI: [[u8; 3]; 16] = [
    [0x18, 0x18, 0x18], [0xac, 0x42, 0x42], [0x90, 0xa9, 0x59], [0xf4, 0xbf, 0x75],
    [0x6a, 0x9f, 0xb5], [0xaa, 0x75, 0x9f], [0x75, 0xb5, 0xaa], [0xd8, 0xd8, 0xd8],
    [0x6b, 0x6b, 0x6b], [0xc5, 0x55, 0x55], [0xaa, 0xc4, 0x74], [0xfe, 0xca, 0x88],
    [0x82, 0xb8, 0xc8], [0xc2, 0x8c, 0xb8], [0x93, 0xd3, 0xc3], [0xf8, 0xf8, 0xf8],
];

/// Columns and rows that fit a terminal of this size (world units).
pub(crate) fn grid_size(size: Vec2) -> (usize, usize) {
    let cols = ((size.x - 2.0 * TERMINAL_PAD) / CELL.x).floor().max(2.0) as usize;
    let rows = ((size.y - TERMINAL_HEADER - TERMINAL_FOOTER - TERMINAL_PAD) / CELL.y).floor().max(1.0) as usize;
    (cols, rows)
}

/// Size of a terminal with this many columns and rows.
pub(crate) fn terminal_size(cols: usize, rows: usize) -> Vec2 {
    let height = rows as f32 * CELL.y + TERMINAL_HEADER + TERMINAL_FOOTER + TERMINAL_PAD;
    Vec2::new(cols as f32 * CELL.x + 2.0 * TERMINAL_PAD, height).ceil()
}

/// Milliseconds since the app started, for stamps shared across threads.
pub(crate) fn uptime_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
}

#[derive(Clone)]
pub(crate) struct Listener {
    ctx: Option<Context>,
    pty: Arc<Mutex<Option<EventLoopSender>>>,
    title: Arc<Mutex<Option<String>>>,
    exited: Arc<AtomicBool>,
    last_output: Arc<AtomicU64>,
}

impl Listener {
    fn reply(&self, text: String) {
        if let Some(pty) = &*self.pty.lock().unwrap() {
            let _ = pty.send(Msg::Input(text.into_bytes().into()));
        }
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            // replies the program asked for (cursor position, device attributes…)
            Event::PtyWrite(text) => self.reply(text),
            // OSC 4/10/11 queries: programs pick their colors from the terminal's
            Event::ColorRequest(index, format) => {
                let c = match index {
                    257 => TERMINAL_BG,
                    i if i < 256 => indexed(i),
                    _ => FG,
                };
                self.reply(format(Rgb { r: c.r(), g: c.g(), b: c.b() }));
            }
            Event::Title(title) => *self.title.lock().unwrap() = Some(title),
            Event::ResetTitle => *self.title.lock().unwrap() = None,
            Event::ClipboardStore(_, text) => {
                if let Some(ctx) = &self.ctx {
                    ctx.copy_text(text);
                }
            }
            Event::Wakeup => self.last_output.store(uptime_ms(), Ordering::Relaxed),
            Event::ChildExit(_) | Event::Exit => self.exited.store(true, Ordering::Relaxed),
            _ => {}
        }
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint();
        }
    }
}

/// What other threads (the agent bridge) need to read and drive a shell.
#[derive(Clone)]
pub(crate) struct TermHandle {
    term: Arc<FairMutex<Term<Listener>>>,
    pty: EventLoopSender,
    last_output: Arc<AtomicU64>,
    exited: Arc<AtomicBool>,
}

impl TermHandle {
    pub(crate) fn write(&self, bytes: Vec<u8>) {
        self.term.lock().scroll_display(Scroll::Bottom);
        let _ = self.pty.send(Msg::Input(bytes.into()));
    }

    /// "Limpar buffer" (⌘K): the screen and the scrollback go, the cursor back to the top; the
    /// program redraws itself on its next output.
    pub(crate) fn clear(&self) {
        use alacritty_terminal::vte::ansi::{ClearMode, Handler};
        let mut term = self.term.lock();
        term.grid_mut().clear_history();
        term.clear_screen(ClearMode::All);
        term.goto(0, 0);
        drop(term);
        // ConPTY keeps its own copy of the screen and paints it back on the next resize: ^L has
        // the shell clear it too and redraw the prompt, as Windows Terminal's Clear Buffer does
        if cfg!(windows) {
            self.write(b"\x0c".to_vec());
        }
    }

    pub(crate) fn mode(&self) -> TermMode {
        *self.term.lock().mode()
    }

    /// Types `text` as a person would: pasted (so a multi-line prompt arrives whole), then Enter
    /// on its own 150 ms later.
    pub(crate) fn type_prompt(&self, text: &str) {
        self.write(paste_bytes(text, self.mode().contains(TermMode::BRACKETED_PASTE)));
        std::thread::sleep(std::time::Duration::from_millis(150));
        self.write(b"\r".to_vec());
    }

    pub(crate) fn exited(&self) -> bool {
        self.exited.load(Ordering::Relaxed)
    }

    /// Printed something in the last 1.5 s: the program is working.
    pub(crate) fn busy(&self) -> bool {
        !self.exited() && uptime_ms().saturating_sub(self.last_output.load(Ordering::Relaxed)) < 1500
    }

    /// The visible lines, trailing spaces trimmed.
    pub(crate) fn screen_text(&self) -> String {
        let term = self.term.lock();
        let content = term.renderable_content();
        let mut lines = vec![String::new(); term.screen_lines()];
        for cell in content.display_iter {
            if let Some(line) = point_to_viewport(content.display_offset, cell.point).and_then(|p| lines.get_mut(p.line)) {
                if !cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    line.push(cell.c);
                }
            }
        }
        lines.iter().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n").trim_end().to_string()
    }
}

/// A running shell. Dropping it ends the shell (the pty sends it SIGHUP).
pub(crate) struct TerminalSession {
    pub(crate) handle: TermHandle,
    /// The folder it started in and when (ms since 1970): how the chat finds its conversation.
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) started_ms: i64,
    size: (usize, usize),
    title: Arc<Mutex<Option<String>>>,
}

fn window_size((cols, rows): (usize, usize)) -> WindowSize {
    WindowSize { num_lines: rows as u16, num_cols: cols as u16, cell_width: CELL.x as u16, cell_height: CELL.y as u16 }
}

impl TerminalSession {
    /// The user's login shell in `cwd` (empty: home), typing `command` once it starts, like
    /// someone at the keyboard (so a shell is left when the command ends); or `program` (tests).
    pub(crate) fn spawn(
        cwd: &str,
        command: &str,
        size: (usize, usize),
        ctx: Option<Context>,
        env: Vec<(String, String)>,
        program: Option<(&str, &[&str])>,
    ) -> std::io::Result<Self> {
        let home = std::env::home_dir();
        let cwd = match cwd.trim().strip_prefix('~') {
            _ if cwd.trim().is_empty() => home,
            Some(rest) => home.map(|h| h.join(rest.trim_start_matches(['/', '\\']))),
            None => Some(PathBuf::from(cwd.trim())),
        };
        // straight to a login shell: alacritty's macOS default goes through login(1), which
        // prints "Last login". Windows: PowerShell 7 when it's installed, else alacritty's default,
        // Windows PowerShell, whose default script policy (Restricted) won't run npm's claude.ps1
        // ponytail: without PowerShell 7, `claude` there needs the person to allow scripts
        let login = if cfg!(windows) {
            let path = std::env::var_os("PATH").unwrap_or_default();
            std::env::split_paths(&path).any(|dir| dir.join("pwsh.exe").is_file()).then(|| tty::Shell::new("pwsh".into(), vec![]))
        } else {
            let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
            Some(tty::Shell::new(shell, vec!["-l".into()]))
        };
        let shell = program.map(|(p, args)| tty::Shell::new(p.into(), args.iter().map(|a| a.to_string()).collect())).or(login);
        let mut vars: std::collections::HashMap<String, String> =
            [("TERM", "xterm-256color"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", APP_TITLE)]
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .into();
        vars.extend(env);
        let cwd = cwd.filter(|dir| dir.is_dir());
        let options = tty::Options { shell, working_directory: cwd.clone(), env: vars, ..Default::default() };
        let pty = tty::new(&options, window_size(size), 0)?;
        let listener = Listener {
            ctx,
            pty: Default::default(),
            title: Default::default(),
            exited: Default::default(),
            last_output: Default::default(),
        };
        let term = Arc::new(FairMutex::new(Term::new(Config::default(), &TermSize::new(size.0, size.1), listener.clone())));
        // drain on exit: on Linux the child's exit can land before its last output is read
        let event_loop = EventLoop::new(term.clone(), listener.clone(), pty, true, false)?;
        let sender = event_loop.channel();
        *listener.pty.lock().unwrap() = Some(sender.clone());
        event_loop.spawn();
        let handle = TermHandle { term, pty: sender, last_output: listener.last_output, exited: listener.exited };
        if !command.trim().is_empty() {
            handle.write(format!("{}\r", command.trim()).into_bytes());
        }
        let cwd = cwd.or_else(|| std::env::current_dir().ok());
        Ok(Self { handle, size, title: listener.title, cwd, started_ms: now_ms() })
    }

    pub(crate) fn resize(&mut self, size: (usize, usize)) {
        if size != self.size {
            self.size = size;
            self.handle.term.lock().resize(TermSize::new(size.0, size.1));
            let _ = self.handle.pty.send(Msg::Resize(window_size(size)));
        }
    }

    /// Positive scrolls up into the history.
    pub(crate) fn scroll(&self, lines: i32) {
        self.handle.term.lock().scroll_display(Scroll::Delta(lines));
    }

    pub(crate) fn title(&self) -> Option<String> {
        self.title.lock().unwrap().clone()
    }

    pub(crate) fn exited(&self) -> bool {
        self.handle.exited()
    }

    /// Paints the grid with its top-left corner at `origin` (screen).
    pub(crate) fn paint(&self, painter: &Painter, origin: Pos2, zoom: f32, focused: bool) {
        let term = self.handle.term.lock();
        let content = term.renderable_content();
        let (cw, ch) = (CELL.x * zoom, CELL.y * zoom);
        let font = FontId::monospace(TERMINAL_FONT * zoom);
        let at = |line: usize, col: usize| origin + Vec2::new(col as f32 * cw, line as f32 * ch);
        // runs of same-styled ASCII share one text shape; anything else is placed on its own cell
        let mut run: Option<(usize, usize, String, Color32, Flags)> = None;
        let flush = |run: &mut Option<(usize, usize, String, Color32, Flags)>| {
            if let Some((line, col, text, fg, flags)) = run.take() {
                let pos = at(line, col);
                let rect = painter.text(pos, egui::Align2::LEFT_TOP, &text, font.clone(), fg);
                if flags.contains(Flags::BOLD) {
                    painter.text(pos + Vec2::new(0.6 * zoom.min(1.5), 0.0), egui::Align2::LEFT_TOP, &text, font.clone(), fg);
                }
                if flags.contains(Flags::UNDERLINE) {
                    painter.hline(rect.x_range(), pos.y + ch - zoom, Stroke::new(zoom, fg));
                }
            }
        };
        // every background goes behind all the text: a run's color is only known once it ends
        let backgrounds_slot = painter.add(Shape::Noop);
        let mut backgrounds = Vec::new();
        let mut bg_run: Option<(usize, usize, usize, Color32)> = None;
        let fill = |bg_run: &mut Option<(usize, usize, usize, Color32)>, backgrounds: &mut Vec<Shape>| {
            if let Some((line, from, to, bg)) = bg_run.take() {
                backgrounds.push(Shape::rect_filled(Rect::from_min_max(at(line, from), at(line + 1, to)), 0.0, bg));
            }
        };
        for cell in content.display_iter {
            let Some(p) = point_to_viewport(content.display_offset, cell.point) else { continue };
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let (line, col) = (p.line, p.column.0);
            let width = if cell.flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
            let (mut fg, mut bg) = (resolve(cell.fg, content.colors), resolve(cell.bg, content.colors));
            if cell.flags.contains(Flags::INVERSE) {
                (fg, bg) = (bg, fg);
            }
            if cell.flags.contains(Flags::DIM) {
                fg = fg.gamma_multiply(0.66);
            }
            if cell.flags.contains(Flags::HIDDEN) {
                fg = bg;
            }
            match &mut bg_run {
                Some((l, _, to, c)) if *l == line && *to == col && *c == bg => *to += width,
                _ => {
                    fill(&mut bg_run, &mut backgrounds);
                    if bg != TERMINAL_BG {
                        bg_run = Some((line, col, col + width, bg));
                    }
                }
            }
            if let Some((parts, alpha)) = block_parts(cell.c) {
                // block elements fill their cell exactly (a font's glyph leaves gaps between
                // rows, which breaks drawings like an agent's logo)
                flush(&mut run);
                let cell_rect = Rect::from_min_size(at(line, col), Vec2::new(cw * width as f32, ch));
                for [x0, y0, x1, y1] in parts {
                    let part = Rect::from_min_max(cell_rect.lerp_inside(Vec2::new(x0, y0)), cell_rect.lerp_inside(Vec2::new(x1, y1)));
                    painter.rect_filled(part, 0.0, fg.gamma_multiply(alpha));
                }
                continue;
            }
            let style = cell.flags & (Flags::BOLD | Flags::UNDERLINE);
            match &mut run {
                Some((l, c, text, f, s)) if *l == line && *c + text.len() == col && *f == fg && *s == style && cell.c.is_ascii() => {
                    text.push(cell.c);
                }
                _ => {
                    flush(&mut run);
                    if cell.c != ' ' || style.contains(Flags::UNDERLINE) {
                        run = Some((line, col, cell.c.to_string(), fg, style));
                    }
                }
            }
            if !cell.c.is_ascii() {
                flush(&mut run);
            }
        }
        fill(&mut bg_run, &mut backgrounds);
        painter.set(backgrounds_slot, Shape::Vec(backgrounds));
        flush(&mut run);
        let cursor = content.cursor;
        if cursor.shape != CursorShape::Hidden {
            if let Some(p) = point_to_viewport(content.display_offset, cursor.point) {
                let rect = Rect::from_min_size(at(p.line, p.column.0), Vec2::new(cw, ch));
                if focused {
                    painter.rect_filled(rect, 0.0, FG.gamma_multiply(0.7));
                } else {
                    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, FG.gamma_multiply(0.7)), StrokeKind::Inside);
                }
            }
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.handle.pty.send(Msg::Shutdown);
    }
}

/// The filled parts of a block element (U+2580–U+259F) as fractions of the cell, and the
/// shade for ░▒▓; None for any other character.
pub(crate) fn block_parts(c: char) -> Option<(Vec<[f32; 4]>, f32)> {
    let eighth = |n: u32| n as f32 / 8.0;
    let (ul, ur, ll, lr) = ([0.0, 0.0, 0.5, 0.5], [0.5, 0.0, 1.0, 0.5], [0.0, 0.5, 0.5, 1.0], [0.5, 0.5, 1.0, 1.0]);
    let parts = match c as u32 {
        0x2580 => vec![[0.0, 0.0, 1.0, 0.5]],
        n @ 0x2581..=0x2588 => vec![[0.0, 1.0 - eighth(n - 0x2580), 1.0, 1.0]],
        n @ 0x2589..=0x258F => vec![[0.0, 0.0, eighth(0x2590 - n), 1.0]],
        0x2590 => vec![[0.5, 0.0, 1.0, 1.0]],
        0x2591 => return Some((vec![[0.0, 0.0, 1.0, 1.0]], 0.25)),
        0x2592 => return Some((vec![[0.0, 0.0, 1.0, 1.0]], 0.5)),
        0x2593 => return Some((vec![[0.0, 0.0, 1.0, 1.0]], 0.75)),
        0x2594 => vec![[0.0, 0.0, 1.0, 0.125]],
        0x2595 => vec![[0.875, 0.0, 1.0, 1.0]],
        0x2596 => vec![ll],
        0x2597 => vec![lr],
        0x2598 => vec![ul],
        0x2599 => vec![ul, ll, lr],
        0x259A => vec![ul, lr],
        0x259B => vec![ul, ur, ll],
        0x259C => vec![ul, ur, lr],
        0x259D => vec![ur],
        0x259E => vec![ur, ll],
        0x259F => vec![ur, ll, lr],
        _ => return None,
    };
    Some((parts, 1.0))
}

fn indexed(i: usize) -> Color32 {
    match i {
        0..16 => Color32::from_rgb(ANSI[i][0], ANSI[i][1], ANSI[i][2]),
        16..232 => {
            let level = |n: usize| if n == 0 { 0 } else { (55 + n * 40) as u8 };
            let i = i - 16;
            Color32::from_rgb(level(i / 36), level(i / 6 % 6), level(i % 6))
        }
        232..256 => Color32::from_gray((8 + (i - 232) * 10) as u8),
        _ => FG,
    }
}

fn resolve(color: Color, colors: &Colors) -> Color32 {
    let rgb = |c: Rgb| Color32::from_rgb(c.r, c.g, c.b);
    match color {
        Color::Spec(c) => rgb(c),
        Color::Indexed(i) => colors[i as usize].map(rgb).unwrap_or_else(|| indexed(i as usize)),
        Color::Named(name) => colors[name].map(rgb).unwrap_or_else(|| match name {
            NamedColor::Background => TERMINAL_BG,
            NamedColor::DimForeground => FG.gamma_multiply(0.66),
            NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::Cursor => FG,
            name if (name as usize) < 16 => indexed(name as usize),
            dim => indexed(dim as usize - NamedColor::DimBlack as usize).gamma_multiply(0.66),
        }),
    }
}

/// Bytes a key sends; printable text arrives on its own as `Event::Text`.
pub(crate) fn key_bytes(key: Key, m: Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    let code = 1 + m.shift as u8 + 2 * m.alt as u8 + 4 * m.ctrl as u8;
    let cursor = |c: char| match (code, app_cursor) {
        (1, true) => format!("\x1bO{c}"),
        (1, false) => format!("\x1b[{c}"),
        _ => format!("\x1b[1;{code}{c}"),
    };
    let tilde = |n: u8| if code > 1 { format!("\x1b[{n};{code}~") } else { format!("\x1b[{n}~") };
    let word = m.alt && !m.ctrl && !m.shift;
    let text = match key {
        // macOS line editing: Cmd+←/→ to the line's start/end, Cmd+⌫ clears it, Option+←/→ by word
        Key::ArrowLeft if m.mac_cmd => "\x01".into(),
        Key::ArrowRight if m.mac_cmd => "\x05".into(),
        Key::Backspace if m.mac_cmd => "\x15".into(),
        Key::ArrowLeft if word => "\x1bb".into(),
        Key::ArrowRight if word => "\x1bf".into(),
        Key::ArrowUp => cursor('A'),
        Key::ArrowDown => cursor('B'),
        Key::ArrowRight => cursor('C'),
        Key::ArrowLeft => cursor('D'),
        Key::Home => cursor('H'),
        Key::End => cursor('F'),
        // Shift/Option+Enter: a new line in agent prompts instead of sending
        Key::Enter if m.shift || m.alt => "\x1b\r".into(),
        Key::Enter => "\r".into(),
        Key::Backspace if m.alt => "\x1b\x7f".into(),
        Key::Backspace if m.ctrl => "\x08".into(),
        Key::Backspace => "\x7f".into(),
        Key::Tab if m.shift => "\x1b[Z".into(),
        Key::Tab => "\t".into(),
        Key::Escape => "\x1b".into(),
        Key::Insert => tilde(2),
        Key::Delete => tilde(3),
        Key::PageUp => tilde(5),
        Key::PageDown => tilde(6),
        Key::F1 => "\x1bOP".into(),
        Key::F2 => "\x1bOQ".into(),
        Key::F3 => "\x1bOR".into(),
        Key::F4 => "\x1bOS".into(),
        Key::F5 => tilde(15),
        Key::F6 => tilde(17),
        Key::F7 => tilde(18),
        Key::F8 => tilde(19),
        Key::F9 => tilde(20),
        Key::F10 => tilde(21),
        Key::F11 => tilde(23),
        Key::F12 => tilde(24),
        _ if m.ctrl && !m.alt => {
            let byte = match key {
                Key::Space | Key::Num2 => 0,
                Key::OpenBracket => 0x1b,
                Key::Backslash => 0x1c,
                Key::CloseBracket => 0x1d,
                Key::Num6 => 0x1e,
                Key::Minus => 0x1f,
                _ => match key.name().as_bytes() {
                    [c @ b'A'..=b'Z'] => c - b'A' + 1,
                    _ => return None,
                },
            };
            return Some(vec![byte]);
        }
        _ => return None,
    };
    Some(text.into_bytes())
}

/// Pasted text: line breaks as Enter, inside bracketed-paste markers when the program asked
/// for them (shells then insert it instead of running each line).
pub(crate) fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if bracketed {
        // an ESC inside could end the paste early and run the rest as typed keys
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.into_bytes()
    }
}
