use super::*;

/// A running shell. Dropping it ends the shell (the pty sends it SIGHUP).
pub(crate) struct TerminalSession {
    pub(crate) handle: TermHandle,
    /// The folder it started in and when (ms since 1970): how the chat finds its conversation.
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) started_ms: i64,
    /// The shell's process: a Claude Code run in it descends from it, whatever folder it went to.
    pub(crate) shell_pid: u32,
    pub(super) size: (usize, usize),
    pub(super) title: Arc<Mutex<Option<String>>>,
    pub(super) hidden: Arc<AtomicBool>,
    pub(super) look: Arc<Mutex<Look>>,
}

pub(super) fn window_size((cols, rows): (usize, usize)) -> WindowSize {
    WindowSize { num_lines: rows as u16, num_cols: cols as u16, cell_width: CELL.x as u16, cell_height: CELL.y as u16 }
}

pub(crate) fn terminal_config(history: usize) -> Config {
    Config { scrolling_history: history, ..Default::default() }
}

impl TerminalSession {
    /// The user's login shell in `cwd` (empty: home), typing `command` once it starts, like
    /// someone at the keyboard (so a shell is left when the command ends); or `program` (tests).
    #[cfg(test)]
    pub(crate) fn spawn(
        cwd: &str,
        command: &str,
        size: (usize, usize),
        ctx: Option<Context>,
        env: Vec<(String, String)>,
        program: Option<(&str, &[&str])>,
        history: usize,
    ) -> std::io::Result<Self> {
        Self::spawn_shell("", cwd, command, size, ctx, env, program, history, Look::default())
    }

    /// `spawn` in the shell `choice` names (a program or its path; empty: the login shell) and with
    /// the colors of `look` for the programs that ask.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn_shell(
        choice: &str,
        cwd: &str,
        command: &str,
        size: (usize, usize),
        ctx: Option<Context>,
        env: Vec<(String, String)>,
        program: Option<(&str, &[&str])>,
        history: usize,
        look: Look,
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
        let login = if !choice.trim().is_empty() {
            Some(tty::Shell::new(choice.trim().into(), if cfg!(windows) { vec![] } else { vec!["-l".into()] }))
        } else if cfg!(windows) {
            let path = std::env::var_os("PATH").unwrap_or_default();
            std::env::split_paths(&path).any(|dir| dir.join("pwsh.exe").is_file()).then(|| tty::Shell::new("pwsh".into(), vec![]))
        } else {
            let fallback = if cfg!(target_os = "macos") { "/bin/zsh" } else { "/bin/sh" };
            let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.into());
            Some(tty::Shell::new(shell, vec!["-l".into()]))
        };
        let shell = program.map(|(p, args)| tty::Shell::new(p.into(), args.iter().map(|a| a.to_string()).collect())).or(login);
        let mut vars: std::collections::HashMap<String, String> =
            [("TERM", "xterm-256color"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", APP_TITLE)].map(|(k, v)| (k.to_string(), v.to_string())).into();
        vars.extend(env);
        let cwd = cwd.filter(|dir| dir.is_dir());
        let options = tty::Options { shell, working_directory: cwd.clone(), env: vars, ..Default::default() };
        let pty = tty::new(&options, window_size(size), 0)?;
        #[cfg(windows)]
        let shell_pid = pty.child_watcher().pid().map_or(0, |p| p.get());
        #[cfg(unix)]
        let shell_pid = pty.child().id();
        let listener = Listener {
            ctx,
            pty: Default::default(),
            title: Default::default(),
            exited: Default::default(),
            last_output: Default::default(),
            hidden: Default::default(),
            look: Arc::new(Mutex::new(look)),
        };
        let config = terminal_config(history);
        let term = Arc::new(FairMutex::new(Term::new(config, &TermSize::new(size.0, size.1), listener.clone())));
        // drain on exit: on Linux the child's exit can land before its last output is read
        let event_loop = EventLoop::new(term.clone(), listener.clone(), pty, true, false)?;
        let sender = event_loop.channel();
        *listener.pty.lock().unwrap() = Some(sender.clone());
        event_loop.spawn();
        let handle =
            TermHandle { term, pty: sender, last_output: listener.last_output, exited: listener.exited, gate: Default::default(), proven: Default::default() };
        if !command.trim().is_empty() {
            handle.write(format!("{}\r", command.trim()).into_bytes());
        }
        let cwd = cwd.or_else(|| std::env::current_dir().ok());
        Ok(Self { handle, size, title: listener.title, cwd, started_ms: now_ms(), shell_pid, hidden: listener.hidden, look: listener.look })
    }

    /// The colors programs get when they ask for them from now on.
    pub(crate) fn set_look(&self, look: &Look) {
        *self.look.lock().unwrap() = look.clone();
    }

    /// Whether it shows: off the screen its output stops repainting the app.
    pub(crate) fn set_visible(&self, visible: bool) {
        self.hidden.store(!visible, Ordering::Relaxed);
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
    pub(crate) fn paint(&self, painter: &Painter, origin: Pos2, zoom: f32, focused: bool, view: &GridView) {
        let term = self.handle.term.lock();
        let content = term.renderable_content();
        let look = view.look;
        let (cw, ch) = (cell(view.font).x * zoom, cell(view.font).y * zoom);
        let font = FontId::monospace(view.font * zoom);
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
            let (mut fg, mut bg) = (resolve(cell.fg, content.colors, look), resolve(cell.bg, content.colors, look));
            if cell.flags.contains(Flags::INVERSE) {
                (fg, bg) = (bg, fg);
            }
            if content.selection.is_some_and(|range| range.contains(cell.point)) {
                bg = over(bg, look.selection);
            }
            if let Some(&(.., current)) = view.marks.get(line).and_then(|row| row.iter().find(|(a, b, _)| (*a..=*b).contains(&col))) {
                bg = over(bg, if current { MATCH_CURRENT } else { MATCH });
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
                    if bg != look.bg {
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
                    painter.rect_filled(rect, 0.0, look.cursor);
                } else {
                    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, look.cursor), StrokeKind::Inside);
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
