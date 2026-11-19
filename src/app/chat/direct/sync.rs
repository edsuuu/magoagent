use super::*;

/// The chat's folder takes the name the agent gave it; true when that is news, and chats.json has to hear of it.
fn renamed(cwd: &mut String, named: &str) -> bool {
    let news = cwd != named;

    if news {
        *cwd = named.to_string();
    }

    news
}

/// The folder a session's first run says it is in, once: it is where Claude Code files the conversation, as it names
/// it (links resolved). A resumed run's is not taken: the agent may have moved (a `cd`), and a chat that followed it
/// looked for its conversation in a folder that never had it.
fn takes_folder<'a>(fresh: &mut bool, event: &'a Value) -> Option<&'a str> {
    let cwd = event["cwd"].as_str().filter(|_| event["type"] == "system" && event["subtype"] == "init")?;

    std::mem::take(fresh).then_some(cwd)
}

impl DrawingApp {
    /// Esc: the answer stops where it is.
    pub(crate) fn chat_stop(&mut self, id: &str) {
        self.cmd_cancel(id);
        // stopped, not done: no celebration (nor the island opening for it)
        self.chat_ended.remove(id);

        if let Some(run) = self.chat_runs.remove(id) {
            // its whole tree goes (on Windows `kill` would end only the wrapper, and the agent would go on writing to the
            // session the next message resumes): ChatRun's drop
            drop(run);
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
        let file = format!("{session}.jsonl");
        let here = claude_dir().map(|dir| dir.join("projects").join(project_folder(cwd)).join(&file));

        // a Claude Code in WSL files it in the distro, by the folder as Linux names it (system/wsl.rs)
        if cfg!(windows)
            && here.as_ref().is_none_or(|path| !path.is_file())
            && let Some(distro) = wsl_distro()
            && let there = wsl_claude_dir(&distro).join("projects").join(project_folder(&linux_path(cwd))).join(&file)
            && there.is_file()
        {
            return Some(there);
        }

        here
    }

    /// What the running answers printed since the last frame; an answer that ended leaves its process, and its
    /// errors when it failed. A turn over whose output stays open (`ChatRun::over`) ends after `RUN_GRACE` all the
    /// same:
    pub(crate) fn poll_chats(&mut self) {
        let ids: Vec<String> = self.chat_runs.keys().cloned().collect();

        for id in ids {
            let mut done = None;
            let events: Vec<ChatEvent> = self.chat_runs.get(&id).map(|r| r.events.try_iter().collect()).unwrap_or_default();
            let Some(i) = self.chats.iter().position(|c| c.id == id) else { continue };
            let (agent, now) = (self.chats[i].agent.clone(), self.now);
            let mut last = false;

            for event in events {
                if let ChatEvent::Line(value) = &event {
                    // the answer to a question another agent asked it (chat/ask.rs)
                    self.chat_heard(&id, value, agent == "codex");
                    last |= turn_over(&agent, value);
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
                        let mut named = false;

                        if let Some(cwd) = self.chat_runs.get_mut(&id).and_then(|run| takes_folder(&mut run.fresh, &value)) {
                            // a Claude Code in WSL names it as Linux does (system/wsl.rs): the chat keeps Windows' name
                            let cwd = match wsl_distro() {
                                Some(distro) if cfg!(windows) && cwd.starts_with('/') => windows_path(&distro.name, cwd),
                                _ => cwd.to_string(),
                            };

                            named |= renamed(&mut self.chats[i].cwd, &cwd);
                        }

                        if let Some(session) = value["session_id"].as_str().filter(|s| self.chats[i].session.as_deref() != Some(s)) {
                            self.chats[i].session = Some(session.to_string());
                            named = true;
                        }

                        // both say where the conversation's file is: a folder kept only in memory loses the
                        // messages on the next start
                        if named {
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

            // its turn over (its last event, or its process gone) and still no end of its output: it ends after a while
            let mut forced = false;

            if done.is_none()
                && let Some(run) = self.chat_runs.get_mut(&id)
            {
                if run.over.is_none() && (last || run.child.try_wait().is_ok_and(|status| status.is_some())) {
                    run.over = Some(now);
                }

                if run.over.is_some_and(|since| now - since >= RUN_GRACE) {
                    (done, forced) = (Some(String::new()), true);
                }
            }

            if let Some(errors) = done
                && let Some(mut run) = self.chat_runs.remove(&id)
            {
                // ended all the same, its process may still be there: not waited on (its drop stops it); its last event
                // said how the turn went
                let ok = match run.child.try_wait() {
                    Ok(Some(status)) => status.success(),
                    Ok(None) if forced => true,
                    _ => run.child.wait().is_ok_and(|s| s.success()),
                };

                self.chat_ended.insert(id.clone(), now);
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
                let message = format!("A lista de chats não pôde ser lida e começou vazia. O arquivo antigo ficou na mesma pasta, como {}.", kept_name(&kept));

                self.toast(if earlier.is_empty() { message } else { format!("{earlier}\n{message}") });
            }
        }

        // chats from before the first message named them: named now by their first message, read from the agent's file
        let mut titled = false;
        let mut ask = Vec::new();
        // and the ones Haiku named after a pasted `claude --resume <id>` (the id in the name): by the session resumed
        let short_id = |name: &str| name.split_whitespace().any(|word| word.len() == 8 && word.chars().all(|letter| letter.is_ascii_hexdigit()));

        for chat in self.chats.iter_mut().filter(|chat| !chat.named || short_id(&chat.name)) {
            let Some(session) = chat.session.as_deref() else { continue };

            let first = if chat.agent == "codex" {
                chat_items_path(&chat.id)
                    .and_then(|path| fs::read_to_string(path).ok())
                    .and_then(|data| serde_json::from_str::<Vec<ChatItem>>(&data).ok())
                    .and_then(|items| items.into_iter().find_map(|item| if let ChatItem::User(text) = item { Some(text) } else { None }))
            } else {
                Self::claude_transcript(&chat.cwd, session).and_then(|path| crate::transcript::first_prompt(&path))
            };

            let Some(mut first) = first else { continue };

            match crate::transcript::resumed_prompt(&first) {
                Some((_, resumed)) => first = resumed,
                None if chat.named => continue,
                None => {}
            }

            // its first line now (not a pasted command line), Haiku's title from the whole message after
            if let Some(title) = Some(first.as_str()).filter(|text| !crate::transcript::pasted_command(text)).and_then(crate::transcript::prompt_title) {
                (chat.name, chat.named, titled) = (title, true, true);
            }

            ask.push((chat.id.clone(), first));
        }

        if titled {
            self.save_chats();
        }

        for (id, first) in ask {
            self.ask_chat_title(None, &id, &first);
        }

        // what Codex chats said comes back when each is opened (`chat_transcript`), not here
        // a chat in an earlier build's folder outside the app starts over in the app's own
        let Some(projects) = projects_dir() else { return };

        for chat in self.chats.iter_mut().filter(|c| OLD_FOLDERS.contains(&c.cwd.trim())) {
            chat.cwd = projects.display().to_string();
            chat.session = None;
            let agent = chat.name.split(" · ").next().unwrap_or(&chat.agent).to_string();

            chat.name = super::chat_name(&agent, &chat.cwd);
        }
    }

    pub(crate) fn save_chats(&self) {
        let _ = self.try_save_chats();
    }

    /// `save_chats`, saying what went wrong.
    pub(crate) fn try_save_chats(&self) -> Result<(), String> {
        // beside this drawing (what `chats_path` says in the app: the same folder), so a test with a drawing of its own stays in its folder
        let Some(path) = self.autosave.as_ref().map(|autosave| autosave.with_file_name("chats.json")) else { return Ok(()) };
        // a draft is not kept (`Chat::draft`)
        let kept: Vec<&Chat> = self.chats.iter().filter(|chat| !chat.draft).collect();
        let json = serde_json::to_string_pretty(&kept).map_err(|error| error.to_string())?;

        write_atomic(&path, json).map_err(|error| error.to_string())
    }

    /// A Codex chat's conversation to its file (Claude Code's is in its own session file).
    pub(crate) fn save_chat_items(&self, id: &str) {
        if !self.chats.iter().any(|c| c.id == id && c.agent == "codex") || self.autosave.is_none() {
            return;
        }

        let items = self.autosave.as_ref().map(|autosave| autosave.with_file_name("chats").join(format!("{id}.json")));

        if let (Some(path), Some(t)) = (items, self.transcripts.get(id)) {
            let _ = fs::create_dir_all(path.parent().unwrap_or(&path));
            let _ = serde_json::to_string(&t.items).map(|json| write_atomic(&path, json));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_folder_the_agent_names_otherwise_is_news_to_save() {
        let mut cwd = "~/p".to_string();

        assert!(super::renamed(&mut cwd, "/home/eu/p"), "chats.json has to hear of it");
        assert_eq!(cwd, "/home/eu/p");
        assert!(!super::renamed(&mut cwd, "/home/eu/p"), "nothing new, nothing to save");
    }

    #[test]
    fn only_a_sessions_first_run_says_where_the_chat_is() {
        let init = serde_json::json!({ "type": "system", "subtype": "init", "cwd": "/home/eu/p/sub" });
        let mut fresh = true;

        assert_eq!(super::takes_folder(&mut fresh, &serde_json::json!({ "type": "assistant", "cwd": "/x" })), None);
        assert_eq!(super::takes_folder(&mut fresh, &init), Some("/home/eu/p/sub"));
        assert_eq!(super::takes_folder(&mut fresh, &init), None, "once");

        let mut resumed = false;

        assert_eq!(super::takes_folder(&mut resumed, &init), None, "the agent moved: the conversation did not");
    }
}
