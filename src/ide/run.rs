use super::*;

impl Ide {
    pub(super) fn toggle_left(&mut self, w: Left) {
        self.left = if self.left == Some(w) { None } else { Some(w) };
    }

    pub(super) fn git_push(&mut self) {
        self.save_all();
        self.git_run("Push", vec![vec!["push".into()]]);
    }

    pub(super) fn git_pull(&mut self) {
        self.save_all();
        self.git_run("Atualizar projeto", vec![vec!["pull".into(), "--ff-only".into()]]);
    }

    /// Balloon notification, bottom-right like PhpStorm's.
    pub(super) fn notify(&mut self, msg: String) {
        self.status = msg.clone();
        self.notice = Some(msg.clone());
        self.toast = Some((msg, Instant::now()));
    }

    pub(crate) fn run(&mut self, cmd: Cmd) {
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
            Cmd::Find => {
                self.find = Some(self.find.take().unwrap_or_default());
                self.find_focus = true;
            }
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
    pub(super) fn check_disk(&mut self) {
        self.sync_tree();
        for tab in self.tabs.iter_mut().filter(|t| t.is_text()) {
            let m = std::fs::metadata(&tab.path).and_then(|m| m.modified()).ok();
            tab.gone = m.is_none();
            if !tab.dirty && m != tab.mtime {
                tab.reload();
                if self.root.is_some() && !self.excluded.contains(&tab.path) {
                    std::sync::Arc::make_mut(&mut self.text_index).update(&tab.path);
                    self.text_gen += 1;
                }
            }
        }
    }

    pub(crate) fn background(&mut self, ctx: &egui::Context) {
        for media in self.tabs.iter_mut().filter_map(|t| t.media.as_mut()) {
            media.upload(ctx);
        }
        if let Some(rx) = &self.files_rx
            && let Ok(files) = rx.try_recv()
        {
            self.files = files;
            self.files_rx = None;
            self.files_gen += 1;
            if self.root.is_some() {
                let (tx, rx) = std::sync::mpsc::channel();
                let (text_tx, text_rx) = std::sync::mpsc::channel();
                let (files, ctx) = (self.files.clone(), self.ctx.clone());
                let (text_files, old, text_ctx) = (files.clone(), self.text_index.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(index::Index::build(&files));
                    ctx.request_repaint();
                });
                std::thread::spawn(move || {
                    let _ = text_tx.send(textindex::TextIndex::build(&text_files, &old));
                    text_ctx.request_repaint();
                });
                self.index_rx = Some(rx);
                self.text_rx = Some(text_rx);
            }
        }
        if let Some(rx) = &self.text_rx
            && let Ok(ix) = rx.try_recv()
        {
            self.text_index = std::sync::Arc::new(ix);
            self.text_rx = None;
            self.text_gen += 1;
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
            Some(Some((p, l, c)))
                if self.tabs.get(self.active).is_some_and(|t| {
                    t.path == p && {
                        let (a, b) = word_at(&t.text, char_at(&t.text, t.cursor.0, t.cursor.1));
                        (a..b).contains(&char_at(&t.text, l, c))
                    }
                }) =>
            {
                self.ask_at_cursor(lsp::Ask::Usages)
            }
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
        // the window stays open once, so the person sees where the unsaved text went
        if ctx.input(|i| i.viewport().close_requested()) && self.recover().is_some() && !std::mem::replace(&mut self.close_warned, true) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        // pick up edits made outside (Claude Code, git, formatters) for clean tabs
        self.git_poll();
        self.db_background();
        self.claude_calls();
        self.claude_selection();
        if focused && !self.focused {
            self.git_refresh();
            // files changed while away (Claude Code, git): list and read them again, at most every 10 s
            if self.indexed_at.elapsed() > Duration::from_secs(10)
                && !self.indexing()
                && let Some(root) = self.root.clone()
            {
                self.reload_files(&root);
            }
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
        self.git_ask_ui(ctx);
    }
}
