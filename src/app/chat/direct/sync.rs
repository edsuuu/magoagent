use super::*;

impl DrawingApp {
    /// Esc: the answer stops where it is.
    pub(crate) fn chat_stop(&mut self, id: &str) {
        self.cmd_cancel(id);
        if let Some(mut run) = self.chat_runs.remove(id) {
            let _ = run.child.kill();
            let _ = run.child.wait();
            self.chat_transcript(id).running.clear();
        }
    }

    /// The chat's conversation, made when first asked for. A Codex chat's comes back then from what
    /// the app kept of it (chats/<id>.json): when the chat is opened or written to, not when the app
    /// opens, and always before anything is added (what is kept is the whole conversation).
    pub(crate) fn chat_transcript(&mut self, id: &str) -> &mut Transcript {
        let codex = self.chats.iter().any(|c| c.id == id && c.agent == "codex");
        self.transcripts.entry(id.to_string()).or_insert_with(|| {
            let mut transcript = Transcript::new(PathBuf::new());
            let saved = if codex { chat_items_path(id).and_then(|p| fs::read_to_string(p).ok()) } else { None };
            if let Some(items) = saved.and_then(|data| serde_json::from_str::<Vec<ChatItem>>(&data).ok()) {
                transcript.items = items;
            }
            transcript
        })
    }

    /// Where Claude Code writes the chat's session, by the folder as the agent names it.
    pub(crate) fn claude_transcript(cwd: &str, session: &str) -> Option<PathBuf> {
        Some(claude_dir()?.join("projects").join(project_folder(cwd)).join(format!("{session}.jsonl")))
    }

    /// What the running answers printed since the last frame; an answer that ended leaves its
    /// process, and its errors when it failed.
    pub(crate) fn poll_chats(&mut self) {
        let ids: Vec<String> = self.chat_runs.keys().cloned().collect();
        for id in ids {
            let mut done = None;
            let events: Vec<ChatEvent> = self.chat_runs.get(&id).map(|r| r.events.try_iter().collect()).unwrap_or_default();
            let Some(i) = self.chats.iter().position(|c| c.id == id) else { continue };
            let (agent, now) = (self.chats[i].agent.clone(), self.now);
            for event in events {
                if let ChatEvent::Line(value) = &event {
                    // the answer to a question another agent asked it (chat/ask.rs)
                    self.chat_heard(&id, value, agent == "codex");
                }
                match event {
                    ChatEvent::Line(value) if agent == "codex" => {
                        if value["type"] == "thread.started"
                            && let Some(thread) = value["thread_id"].as_str()
                        {
                            self.chats[i].session = Some(thread.to_string());
                            self.save_chats();
                        }
                        codex_event(self.chat_transcript(&id), &value, now);
                    }
                    ChatEvent::Line(value) => {
                        // a limit event of the stream: how much of the plan is used (usage.rs)
                        self.usage.plan.merge(crate::app::usage::plan_of_event(&value), now_ms() / 1000);
                        self.context_event(&id, &value);
                        // the folder as Claude Code names it, and the session it keeps (a resumed
                        // one may go on as a copy)
                        if value["type"] == "system"
                            && value["subtype"] == "init"
                            && let Some(cwd) = value["cwd"].as_str()
                        {
                            self.chats[i].cwd = cwd.to_string();
                        }
                        if let Some(session) = value["session_id"].as_str().filter(|s| self.chats[i].session.as_deref() != Some(s)) {
                            self.chats[i].session = Some(session.to_string());
                            self.save_chats();
                        }
                        if value["is_error"] == true {
                            let text = value["result"]
                                .as_str()
                                .map(str::to_string)
                                .or_else(|| value["errors"].as_array().map(|e| e.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n")));
                            self.chat_transcript(&id).items.push(ChatItem::Error(text.unwrap_or_else(|| "O agente falhou.".into())));
                        }
                    }
                    ChatEvent::Done(errors) => done = Some(errors),
                }
            }
            if let Some(errors) = done
                && let Some(mut run) = self.chat_runs.remove(&id)
            {
                let ok = run.child.wait().is_ok_and(|s| s.success());
                self.avatar_listen();
                self.avatar_set(AvatarState::Idle);
                let transcript = self.chat_transcript(&id);
                transcript.running.clear();
                if !ok {
                    let tail: Vec<&str> = errors.lines().filter(|l| !l.trim().is_empty() && !l.contains("rmcp::")).collect();
                    let tail = tail[tail.len().saturating_sub(4)..].join("\n");
                    transcript.items.push(ChatItem::Error(if tail.is_empty() { format!("{agent} terminou com erro.") } else { tail }));
                }
                self.save_chat_items(&id);
            }
        }
    }

    /// Reads what the chat's agent wrote: Claude Code's session file, when there is one.
    pub(crate) fn chat_refresh(&mut self, id: &str) {
        let Some(chat) = self.chats.iter().find(|c| c.id == id).cloned() else { return };
        if chat.agent != "codex"
            && let Some(path) = chat.session.as_deref().and_then(|s| Self::claude_transcript(&chat.cwd, s))
            && self.transcripts.get(id).is_none_or(|t| t.path() != path)
        {
            // the errors shown so far stay
            // (and the chat's own notes: a command's output, /clear)
            // (and the messages just sent, waiting for the file's copy)
            let echoes = self.transcripts.get(id).map_or(0, |t| t.echoes);
            let mut users = echoes;
            let mut kept: Vec<ChatItem> = self
                .transcripts
                .get(id)
                .map(|t| {
                    t.items
                        .iter()
                        .rev()
                        .filter(|i| match i {
                            ChatItem::User(_) if users > 0 => {
                                users -= 1;
                                true
                            }
                            ChatItem::Error(_) | ChatItem::System(_) => true,
                            _ => false,
                        })
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            kept.reverse();
            let mut transcript = Transcript::new(path);
            transcript.items = kept;
            transcript.echoes = echoes;
            self.transcripts.insert(id.to_string(), transcript);
        }
        self.chat_transcript(id).poll();
    }

    pub(crate) fn load_chats(&mut self) {
        if let Some(path) = chats_path() {
            let (chats, kept) = read_chats(&path);
            self.chats = chats;
            if let Some(kept) = kept {
                // after the drawing's own warning, if it had one
                let earlier = std::mem::take(&mut self.status);
                let message = format!("Não foi possível ler chats.json: ficou guardado em {} e a lista de chats começou vazia.", kept.display());
                self.toast(if earlier.is_empty() { message } else { format!("{earlier}\n{message}") });
            }
        }
        // what Codex chats said comes back when each is opened (`chat_transcript`), not here
        // a chat in an earlier build's folder outside the app starts over in the app's own
        let Some(projects) = projects_dir() else { return };
        for chat in self.chats.iter_mut().filter(|c| OLD_FOLDERS.contains(&c.cwd.trim())) {
            chat.cwd = projects.display().to_string();
            chat.session = None;
            let agent = chat.name.split(" · ").next().unwrap_or(&chat.agent).to_string();
            chat.name = format!("{agent} · {}", folder_label(&chat.cwd));
        }
    }

    pub(crate) fn save_chats(&self) {
        if let (Some(path), true) = (chats_path(), self.autosave.is_some()) {
            let _ = serde_json::to_string_pretty(&self.chats).map(|json| write_atomic(&path, json));
        }
    }

    /// A Codex chat's conversation to its file (Claude Code's is in its own session file).
    pub(crate) fn save_chat_items(&self, id: &str) {
        if !self.chats.iter().any(|c| c.id == id && c.agent == "codex") || self.autosave.is_none() {
            return;
        }
        if let (Some(path), Some(t)) = (chat_items_path(id), self.transcripts.get(id)) {
            let _ = fs::create_dir_all(path.parent().unwrap_or(&path));
            let _ = serde_json::to_string(&t.items).map(|json| write_atomic(&path, json));
        }
    }
}
