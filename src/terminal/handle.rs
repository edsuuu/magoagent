use super::*;

#[derive(Clone)]
pub(crate) struct Listener {
    pub(super) ctx: Option<Context>,
    pub(super) pty: Arc<Mutex<Option<EventLoopSender>>>,
    pub(super) title: Arc<Mutex<Option<String>>>,
    pub(super) exited: Arc<AtomicBool>,
    pub(super) last_output: Arc<AtomicU64>,
    pub(super) hidden: Arc<AtomicBool>,
    /// The colors programs get when they ask for them (OSC 4/10/11): the theme's.
    pub(super) look: Arc<Mutex<Look>>,
}

impl Listener {
    pub(super) fn reply(&self, text: String) {
        if let Some(pty) = &*self.pty.lock().unwrap() {
            let _ = pty.send(Msg::Input(text.into_bytes().into()));
        }
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        // a terminal nobody sees doesn't repaint the app for its output (the idle repaint still
        // reads it); its end always shows
        let wake = !self.hidden.load(Ordering::Relaxed) || matches!(event, Event::ChildExit(_) | Event::Exit);
        match event {
            // replies the program asked for (cursor position, device attributes…)
            Event::PtyWrite(text) => self.reply(text),
            // OSC 4/10/11 queries: programs pick their colors from the terminal's
            Event::ColorRequest(index, format) => {
                let look = self.look.lock().unwrap().clone();
                let c = match index {
                    257 => look.bg,
                    i if i < 256 => indexed(i, &look),
                    _ => look.fg,
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
        if wake && let Some(ctx) = &self.ctx {
            ctx.request_repaint();
        }
    }
}

/// What other threads (the agent bridge) need to read and drive a shell.
#[derive(Clone)]
pub(crate) struct TermHandle {
    pub(super) term: Arc<FairMutex<Term<Listener>>>,
    pub(super) pty: EventLoopSender,
    pub(super) last_output: Arc<AtomicU64>,
    pub(super) exited: Arc<AtomicBool>,
    /// Why a prompt typed now wouldn't reach the agent, as the app last saw it (its Claude Code
    /// session waits on a question, or the claude it was made for is gone); None: it would.
    pub(super) gate: Arc<Mutex<Option<String>>>,
    /// Its Claude Code session says what it is doing: the screen's guess counts less (`ready`).
    pub(super) proven: Arc<AtomicBool>,
}

/// The agent stopped on a menu only its terminal answers: Claude Code's permission prompt
/// (numbered options under a question) or any "enter to confirm / esc to cancel" footer. A
/// screen with the free-typing box ("? for shortcuts") is never waiting, whatever it asked.
pub(crate) fn asks_for_you(screen: &str) -> bool {
    menu_on(screen, true)
}

/// `asks_for_you`, with the numbered options under a question counted only when `numbered`: an
/// answer can end that way too, and a Claude Code session that says it is idle is believed.
pub(super) fn menu_on(screen: &str, numbered: bool) -> bool {
    let tail: Vec<String> = screen.lines().rev().filter(|l| !l.trim().is_empty()).take(14).map(str::to_lowercase).collect();
    let typing = tail.iter().any(|l| l.contains("? for shortcuts") || l.trim_start().starts_with("│ >"));
    let footer = tail.iter().any(|l| l.contains("esc to cancel") || l.contains("enter to confirm"));
    let options = tail
        .iter()
        .filter(|l| {
            let option = l.trim_start().trim_start_matches(['❯', '›', '>']).trim_start();
            option.chars().next().is_some_and(|c| c.is_ascii_digit()) && option.get(1..3) == Some(". ")
        })
        .count();
    !typing && (footer || (numbered && options >= 2 && tail.iter().any(|l| l.contains('?'))))
}

/// A menu a screen ends on: the lines over it (its question), its choices and the one the marker
/// (❯ or ›) is on.
#[derive(Debug, PartialEq)]
pub(crate) struct Menu {
    pub(crate) question: Vec<String>,
    pub(crate) choices: Vec<String>,
    pub(crate) marked: usize,
}

/// The menu on a screen, when there is one to pick from: the choices are the lines that start where
/// the marked one's text does (a deeper line goes with the choice over it), a box's side bars aside.
pub(crate) fn menu_choices(screen: &str) -> Option<Menu> {
    let lines: Vec<String> = screen.lines().map(|l| l.replace('│', " ").trim_end().to_string()).collect();
    let at = lines.iter().rposition(|l| l.trim_start().starts_with(['❯', '›']))?;
    let lead = |l: &str| l.chars().take_while(|c| c.is_whitespace()).count();
    let after: String = lines[at].trim_start().chars().skip(1).collect();
    let column = lead(&lines[at]) + 1 + lead(&after);
    let inside = |l: &String| !l.trim().is_empty() && lead(l) >= column;
    let start = (0..at).rev().take_while(|&i| inside(&lines[i])).last().unwrap_or(at);
    let end = (at + 1..lines.len()).take_while(|&i| inside(&lines[i])).last().map_or(at + 1, |i| i + 1);
    let picks: Vec<usize> = (start..end).filter(|&i| i == at || lead(&lines[i]) == column).collect();
    if picks.len() < 2 {
        return None;
    }
    let choices = picks.iter().map(|&i| if i == at { after.trim().to_string() } else { lines[i].trim().to_string() }).collect();
    // over it, up to a rule or a box's top: what it asks
    let rule = |l: &str| l.trim().chars().all(|c| "─━╭╮╰╯┌┐└┘═ ".contains(c));
    let mut question: Vec<String> = lines[..start]
        .iter()
        .rev()
        .skip_while(|l| l.trim().is_empty())
        .take_while(|l| !rule(l) || l.trim().is_empty())
        .take(6)
        .map(|l| l.trim().to_string())
        .collect();
    question.reverse();
    question.retain(|l| !l.is_empty());
    Some(Menu { question, choices, marked: picks.iter().position(|&i| i == at).unwrap_or(0) })
}

impl TermHandle {
    /// Answers its menu as the arrows and Enter would: from the marked choice to `to`, a key at a
    /// time (a program reading the keys may take a burst as one). Sleeps: call it off the UI thread.
    pub(crate) fn pick(&self, from: usize, to: usize) {
        let app = self.mode().contains(TermMode::APP_CURSOR);
        let step = if to > from { Key::ArrowDown } else { Key::ArrowUp };
        for key in std::iter::repeat_n(step, from.abs_diff(to)).chain([Key::Enter]) {
            if let Some(bytes) = key_bytes(key, Modifiers::NONE, app) {
                self.write(bytes);
            }
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
    }

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
    /// on its own 150 ms later. Refused (`ready`) when it would land somewhere else.
    pub(crate) fn type_prompt(&self, text: &str) -> Result<(), String> {
        self.ready()?;
        self.write(paste_bytes(text, self.mode().contains(TermMode::BRACKETED_PASTE)));
        std::thread::sleep(std::time::Duration::from_millis(150));
        self.write(b"\r".to_vec());
        Ok(())
    }

    /// Whether a prompt typed now reaches the agent; why not, to follow its name, when it would
    /// answer a menu on its screen or the app knows the agent isn't there to read it.
    pub(crate) fn ready(&self) -> Result<(), String> {
        if let Some(why) = self.gate.lock().unwrap().clone() {
            return Err(why);
        }
        // the session's word over the screen's: only a menu's own footer still counts then (its
        // file may lag half a second behind the menu)
        if !self.exited() && menu_on(&self.screen_text(), !self.proven.load(Ordering::Relaxed)) {
            return Err("espera uma resposta no próprio terminal (um menu de permissão ou de escolha): o texto responderia a ele. Responda lá primeiro.".into());
        }
        Ok(())
    }

    /// Stopped on a question only its terminal answers (`asks_for_you`).
    pub(crate) fn waiting(&self) -> bool {
        !self.exited() && asks_for_you(&self.screen_text())
    }

    pub(crate) fn set_gate(&self, why: Option<String>, proven: bool) {
        *self.gate.lock().unwrap() = why;
        self.proven.store(proven, Ordering::Relaxed);
    }

    pub(crate) fn exited(&self) -> bool {
        self.exited.load(Ordering::Relaxed)
    }

    /// Printed something in the last 1.5 s: the program is working.
    pub(crate) fn busy(&self) -> bool {
        !self.exited() && uptime_ms().saturating_sub(self.last_output.load(Ordering::Relaxed)) < 1500
    }

    /// The screen's lines, trailing spaces trimmed: the live bottom of the grid, even while the
    /// person has scrolled up into the history.
    pub(crate) fn screen_text(&self) -> String {
        let term = self.term.lock();
        let grid = term.grid();
        let lines: Vec<String> = (0..term.screen_lines())
            .map(|i| {
                let row = &grid[Line(i as i32)];
                let cells = (0..term.columns()).map(|c| &row[Column(c)]);
                let line: String =
                    cells.filter(|cell| !cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)).map(|cell| cell.c).collect();
                line.trim_end().to_string()
            })
            .collect();
        lines.join("\n").trim_end().to_string()
    }
}
