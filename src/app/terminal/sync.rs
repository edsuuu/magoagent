use super::*;

impl DrawingApp {
    pub(crate) fn terminal_env(&self, id: &str, name: &str) -> Vec<(String, String)> {
        let mut env = vec![("MAGOAGENT_TERMINAL_ID".to_string(), id.to_string()), ("MAGOAGENT_TERMINAL_NAME".into(), name.into())];
        if let Some(bridge) = &self.bridge {
            env.push(("MAGOAGENT_BRIDGE".into(), bridge.address.clone()));
            // its own token, which says who it is (bridge.rs `authorize`): a new shell takes the old one's place
            env.push(("MAGOAGENT_TOKEN".into(), bridge.shared.lock().unwrap().terminal_token(id)));
        }
        // `magoagent list|ask|…` works in every terminal: this app's folder first on PATH
        env.extend(crate::app::chat::node::app_path());
        env.extend(self.ide_env(id));
        // the MCP in it draws in this drawing, its workspace's, even once another one is on screen
        // (absolute: the shell may change folder)
        if let Some(scene) = &self.autosave {
            env.push(("MAGOAGENT_SCENE".into(), scene.to_string_lossy().into()));
        }
        env
    }

    /// One shell per terminal element: new ones start, removed ones end, sizes follow the box.
    /// Also tells when an agent finishes and keeps the bridge's view of the canvas current.
    pub(crate) fn sync_terminals(&mut self, ctx: &Context) {
        let elevated = self.elevated_rect();
        let live: Vec<(String, TerminalSpec, Vec2)> = self
            .scene
            .elements
            .iter()
            .filter_map(|e| {
                let size = if self.terminal_elevated.as_ref() == Some(&e.id) { elevated.size() } else { local_bounds(e).size() };
                Some((e.id.clone(), e.terminal.clone().filter(|t| !t.unloaded)?, size))
            })
            .collect();
        // an undo that took the terminal away keeps its shell while a redo can bring it back; Delete
        // and the MCP take a snapshot, which clears the redo, so they still end it now. An unloaded
        // one is still drawn: it ends now
        // ponytail: scans the redo once per orphan per frame; index the ids if the redo grows large
        // A shell whose workspace is off view (`away`) is not orphaned: it keeps running
        let drawn = |id: &str| self.scene.elements.iter().any(|e| e.id == id);
        let gone: Vec<String> = self
            .terminals
            .keys()
            .filter(|id| {
                !live.iter().any(|(l, ..)| l == *id)
                    && !self.away.contains_key(*id)
                    && (drawn(id) || !self.redo.iter().any(|s| s.elements.iter().any(|e| e.id == **id)))
            })
            .cloned()
            .collect();
        for id in gone {
            self.terminals.remove(&id);
        }
        // a terminal's token is good while its shell runs: closed, restarting, ended or never started, it goes;
        // a chat's, while its agent answers
        if let Some(bridge) = &self.bridge {
            let running = |id: &String| self.terminals.get(id).is_some_and(|s| s.as_ref().is_ok_and(|s| !s.exited())) || self.chat_runs.contains_key(id);
            bridge.shared.lock().unwrap().keep_tokens(running);
        }
        // what shows now (with the chat open, only its agent): the others' output waits for the idle repaint
        let shown = if self.chat_open { Rect::NOTHING } else { self.canvas_rect };
        let seen: std::collections::HashSet<String> = self
            .scene
            .elements
            .iter()
            .filter(|e| e.terminal.is_some() && self.terminal_frame(e).0.intersects(shown))
            .map(|e| e.id.clone())
            .chain(self.chat_agent.clone().filter(|_| self.chat_open))
            .collect();
        for (id, spec, size) in &live {
            let (theme, font, shell) = self.prefs.terminal.resolve(&spec.look);
            let (look, shell, grid) = (Look::of(theme, self.dark()), shell.to_string(), grid_size_at(*size, font));
            match self.terminals.get_mut(id) {
                Some(Ok(session)) => {
                    session.resize(grid);
                    session.set_look(&look);
                }
                Some(Err(_)) => {}
                None => {
                    let env = self.terminal_env(id, &spec.name);
                    match self.terminal_ide(id) {
                        Some((ide, _)) => self.terminal_ides.insert(id.clone(), (ide, false)),
                        None => self.terminal_ides.remove(id),
                    };
                    let command = with_hooks(spec.launch_command(crate::transcript::claude_dir().as_deref()), self.hook_settings().as_deref());
                    // the app's skills go to a terminal's claude too (skills.rs); never the real folder in a test
                    let skills = if cfg!(test) { None } else { crate::app::maestro::skills::skills_path() };
                    let command = crate::app::maestro::skills::with_skills(command, skills.as_deref());
                    let session =
                        TerminalSession::spawn_shell(&shell, &spec.cwd, &command, grid, Some(ctx.clone()), env, None, self.prefs.terminal_history, look);
                    self.terminals.insert(id.clone(), session.map_err(|e| e.to_string()));
                }
            }
        }
        // a shell an undo took (kept for the redo) shows nowhere; with the window hidden, none does
        let window = ctx.input(|i| i.viewport().visible()) != Some(false);
        for (id, session) in &self.terminals {
            if let Ok(session) = session {
                session.set_visible(window && seen.contains(id));
            }
        }
        if self.leaving.as_ref().is_some_and(|(_, since)| self.now - since > 0.2) {
            self.leaving = None;
        }
        if self.terminal_focus.as_ref().is_some_and(|id| !live.iter().any(|(l, ..)| l == id)) {
            self.terminal_focus = None;
        }
        // an IDE is enlarged the same way and has no shell
        if self.terminal_elevated.as_ref().is_some_and(|id| !self.scene.elements.iter().any(|e| &e.id == id)) {
            self.terminal_elevated = None;
        }
        // "Monitorar atividade": an agent that worked a while and went quiet gets a toast (off view too)
        let watched: Vec<(String, TerminalSpec)> =
            live.iter().map(|(id, spec, _)| (id.clone(), spec.clone())).chain(self.away.iter().map(|(id, (_, spec))| (id.clone(), spec.clone()))).collect();
        for (id, spec) in watched.iter().filter(|(_, spec)| !spec.command.trim().is_empty() && !spec.quiet) {
            let busy = matches!(self.terminals.get(id), Some(Ok(s)) if s.handle.busy());
            if busy {
                self.terminal_busy.entry(id.clone()).or_insert(self.now);
            } else if let Some(since) = self.terminal_busy.remove(id) {
                self.terminal_idle.insert(id.clone(), self.now);
                if self.now - since > 3.0 && self.terminal_focus.as_ref() != Some(id) {
                    self.toast(format!("{} terminou", if spec.name.is_empty() { "O terminal" } else { &spec.name }));
                    self.terminal_unread.insert(id.clone());
                }
            }
        }
        if let Some(id) = &self.terminal_focus {
            self.terminal_unread.remove(id);
        }
        let Some(bridge) = &self.bridge else { return };
        let terminals = self.terminals.iter().filter_map(|(id, s)| Some((id.clone(), s.as_ref().ok()?.handle.clone()))).collect();
        let mut shared = bridge.shared.lock().unwrap();
        (shared.nodes, shared.terminals) = (self.agent_nodes(), terminals);
        shared.chats = self.chats.iter().map(|c| (c.id.clone(), self.maestro_name(&c.name))).collect();
        drop(shared);
        let commands: Vec<UiCommand> = bridge.commands.try_iter().collect();
        for command in commands {
            self.apply_agent_command(command);
        }
    }

    pub(crate) fn apply_agent_command(&mut self, command: UiCommand) {
        match command {
            UiCommand::PortalNavigate { id, url } => self.portal_navigate(&id, &url),
            UiCommand::PortalEval { id, script, reply } => self.portal_eval(&id, &script, reply),
            UiCommand::PortalHistory { id, action } => self.portal_history(&id, &action),
            UiCommand::PortalShot { id, reply } => {
                let _ = reply.send(self.portal_shot(&id));
            }
            UiCommand::WriteNote { id, text } => {
                let label_id = format!("{id}-label");
                self.snapshot();
                for e in &mut self.scene.elements {
                    if let (true, ElementKind::Text { value, .. }) = (e.id == label_id, &mut e.kind) {
                        *value = text.clone();
                    }
                }
            }
            UiCommand::CreateNote { from, text } => {
                let Some(terminal) = self.scene.elements.iter().find(|e| e.id == from) else { return };
                // to the terminal's left, like a sticky on the monitor's bezel
                let b = local_bounds(terminal);
                let note = self.new_note(Rect::from_min_size(Pos2::new(b.min.x - 290.0, b.min.y), Vec2::splat(250.0)), text);
                let note_id = note[0].id.clone();
                self.snapshot();
                self.scene.elements.extend(note);
                // one Cmd+Z takes back the note and its rope together
                self.rope_between(&note_id, &from, false);
            }
            // a chat builds its team from its card: the Maestro comes onto the canvas shown if it
            // is not there (`add_chat_node` does nothing for what is not a chat)
            UiCommand::Recruit { from, command, worktree, mode, reply } => {
                self.add_chat_node(&from, None);
                self.recruit_in(&from, &command, worktree, mode, reply)
            }
            UiCommand::CreatePortal { from, url, name, reply } => {
                self.add_chat_node(&from, None);
                self.create_portal(&from, &url, &name, reply)
            }
            UiCommand::Connect { a, b, reply } => {
                self.add_chat_node(&a, None);
                self.connect_for_agent(&a, &b, reply)
            }
            UiCommand::Notify { from, text } => self.notify(&from, &text),
            UiCommand::Ide { id, action, reply } => self.ide_agent(&id, action, reply),
            UiCommand::AskChat { id, from, asker, text, reply } => self.ask_chat(&id, &from, &asker, &text, reply),
            UiCommand::CheckChat { id, reply } => {
                let _ = reply.send(self.chat_check(&id));
            }
            other => self.apply_maestro_command(other),
        }
    }
}
