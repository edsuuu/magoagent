//! A chat with an agent installed here, with no terminal in between: each message runs the agent's
//! own headless mode (`claude -p`, `codex exec`) in the chat's folder, under the login the agent
//! keeps, and the conversation goes on by its session id. Claude Code's words come from the session
//! file it writes (transcript.rs); Codex's from the events it prints. The chats live in chats.json
//! beside the drawing.

use crate::app::avatar::AvatarState;
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::transcript::{ChatItem, LOG_MAX, Step, Transcript, claude_dir, log_tail, project_folder, step_mut};
use std::io::{BufRead, Read, Write};
use std::sync::mpsc::{Receiver, channel};

/// What a chat runs, where, and how far it got.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Chat {
    pub(crate) id: String,
    /// The quick start's program: "claude" or "codex".
    pub(crate) agent: String,
    pub(crate) name: String,
    pub(crate) cwd: String,
    /// The agent's session (Claude Code's UUID, Codex's thread), once a message went.
    #[serde(default)]
    pub(crate) session: Option<String>,
    /// An index in `CHAT_MODES`.
    #[serde(default)]
    pub(crate) mode: usize,
    /// The agent's model and effort for the next message; empty, its defaults (chat_footer.rs).
    #[serde(default)]
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) effort: String,
}

/// What the agent may do on its own: Claude Code's auto and bypassPermissions modes; for Codex,
/// approvals reviewed for you in the sandbox, or no approvals and no sandbox.
pub(crate) const CHAT_MODES: [&str; 2] = ["Auto", "Bypass permissions"];

/// The form of a new chat: which installed agent, in which folder, with which leave.
pub(crate) struct NewChat {
    pub(crate) agent: usize,
    pub(crate) cwd: String,
    pub(crate) mode: usize,
}

impl NewChat {
    /// As Ajustes presets it: the agent (among the installed ones), the folder and the leave.
    pub(crate) fn from_prefs(prefs: &Prefs, installed: &[Preset]) -> NewChat {
        let agent = installed.iter().position(|p| p.command.starts_with(&prefs.chat_agent)).unwrap_or(0);
        let cwd = if OLD_FOLDERS.contains(&prefs.chat_folder.trim()) { "" } else { prefs.chat_folder.trim() };
        let cwd = if cwd.is_empty() { projects_dir().map(|d| d.display().to_string()).unwrap_or_default() } else { cwd.to_string() };
        NewChat { agent, cwd, mode: prefs.chat_mode.min(CHAT_MODES.len() - 1) }
    }
}

/// The chats' folders of earlier builds, outside the app: chats in them move to `projects_dir`.
const OLD_FOLDERS: [&str; 2] = ["~/Desktop/MagoAgent", "~/Documents/MagoAgent"];

pub(crate) enum ChatEvent {
    Line(Value),
    /// The process printed its last line; what stderr said, for when it failed.
    Done(String),
}

/// A message being answered: the process and what it prints.
pub(crate) struct ChatRun {
    child: std::process::Child,
    events: Receiver<ChatEvent>,
    pub(crate) since: f64,
}

/// The agent's command line for one message: headless, its events as JSON lines, the prompt on
/// stdin, and the session carried on when there is one.
pub(crate) fn chat_args(agent: &str, mode: usize, session: &str, resume: bool) -> Vec<String> {
    let mode = mode.min(1);
    let mut args: Vec<String> = match agent {
        "codex" => {
            let mut args = vec!["exec", "--json", "--skip-git-repo-check"];
            args.push(if mode == 1 { "--dangerously-bypass-approvals-and-sandbox" } else { "--approve-for-me" });
            if resume {
                args.extend(["resume", session]);
            }
            args.push("-");
            args
        }
        _ => {
            let permission = ["auto", "bypassPermissions"][mode];
            vec![
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--permission-mode",
                permission,
                if resume { "--resume" } else { "--session-id" },
                session,
            ]
        }
    }
    .into_iter()
    .map(str::to_string)
    .collect();
    args.shrink_to_fit();
    args
}

/// The chat's folder on disk: empty or "~" is home.
pub(crate) fn folder(cwd: &str) -> Option<PathBuf> {
    let cwd = cwd.trim();
    let home = std::env::home_dir();
    match cwd.strip_prefix('~') {
        Some(rest) => home.map(|h| if rest.is_empty() { h } else { h.join(rest.trim_start_matches(['/', '\\'])) }),
        None if cwd.is_empty() => home,
        None => Some(PathBuf::from(cwd)),
    }
}

/// The folder's last name, for the chat's own: "magoagent", "~" for home.
pub(crate) fn folder_label(cwd: &str) -> String {
    let cwd = cwd.trim().trim_end_matches(['/', '\\']);
    if cwd.is_empty() || cwd == "~" {
        return "~".into();
    }
    cwd.rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).unwrap_or(cwd).to_string()
}

/// One of Codex's events (`codex exec --json`) into the conversation: its words, the commands it
/// runs and the files it changes as steps, its errors.
pub(crate) fn codex_event(transcript: &mut Transcript, event: &Value, now: f64) {
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    match event["type"].as_str() {
        Some(kind @ ("item.started" | "item.updated" | "item.completed")) => {
            let (item, done) = (&event["item"], kind == "item.completed");
            let id = text(&item["id"]);
            match item["type"].as_str() {
                Some("agent_message") if done => transcript.items.push(ChatItem::Agent(text(&item["text"]))),
                Some("command_execution") => {
                    if !transcript.items.iter().any(|i| matches!(i, ChatItem::Tool(s) if s.id == id)) {
                        let what = text(&item["command"]);
                        transcript.running.push((id.clone(), format!("Bash {what}")));
                        let step = Step { id: id.clone(), name: "Bash".into(), input: what.clone(), what, started: Some(now), ..Default::default() };
                        transcript.items.push(ChatItem::Tool(step));
                    }
                    if let (true, Some(step)) = (done, step_mut(&mut transcript.items, &id)) {
                        step.ended = Some(now);
                        let code = item["exit_code"].as_i64().filter(|c| *c != 0);
                        (step.failed, step.error) = (code.is_some(), code.map(|c| format!("Saiu com o código {c}")));
                        let out = item["aggregated_output"].as_str().unwrap_or_default();
                        step.output = out.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_string);
                        step.log = log_tail(out.trim_end(), LOG_MAX);
                        transcript.running.retain(|(r, _)| *r != id);
                    }
                }
                Some("file_change") if done => {
                    let changes = item["changes"].as_array().map(Vec::as_slice).unwrap_or_default();
                    let what = changes.iter().map(|c| text(&c["path"])).collect::<Vec<_>>().join(", ");
                    let note = changes.iter().map(|c| text(&c["kind"])).collect::<Vec<_>>().join(", ");
                    let files = changes.iter().map(|c| text(&c["path"])).collect();
                    let step = Step { id, name: "Edit".into(), what, started: Some(now), ended: Some(now), note: Some(note), files, ..Default::default() };
                    transcript.items.push(ChatItem::Tool(step));
                }
                Some("error") if done => transcript.items.push(ChatItem::Error(text(&item["message"]))),
                _ => {}
            }
        }
        Some("turn.failed") => transcript.items.push(ChatItem::Error(text(&event["error"]["message"]))),
        Some("error") => transcript.items.push(ChatItem::Error(text(&event["message"]))),
        _ => {}
    }
}

impl DrawingApp {
    /// The quick starts that run an agent found on this machine (a shell isn't one), looked up
    /// again every 10 s: one installed meanwhile shows up.
    pub(crate) fn installed_agents(&mut self) -> Vec<Preset> {
        if self.chat_installed.as_ref().is_none_or(|(at, _)| self.now - at > 10.0) {
            let found = |p: &Preset| p.command.split_whitespace().next().is_some_and(|program| crate::ide::lsp::which(program).is_some());
            self.chat_installed = Some((self.now, self.prefs.presets.iter().filter(|p| found(p)).cloned().collect()));
        }
        self.chat_installed.as_ref().map(|(_, list)| list.clone()).unwrap_or_default()
    }

    pub(crate) fn is_chat(&self, id: &str) -> bool {
        self.chats.iter().any(|c| c.id == id)
    }

    /// The chats as rows of the agents list, ahead of the terminals'.
    pub(crate) fn chat_rows(&self) -> Vec<AgentRow> {
        self.chats
            .iter()
            .map(|c| {
                let spec = TerminalSpec { name: c.name.clone(), cwd: c.cwd.clone(), command: c.agent.clone(), chat: true, ..Default::default() };
                AgentRow { id: c.id.clone(), spec, state: self.chat_state(&c.id).unwrap_or(AgentState::Idle(None)), mission: None, space: String::new() }
            })
            .collect()
    }

    /// Working while a message is being answered; None for what isn't a chat.
    pub(crate) fn chat_state(&self, id: &str) -> Option<AgentState> {
        self.is_chat(id).then(|| match self.chat_runs.get(id).map(|r| r.since).or_else(|| self.chat_cmds.get(id).map(|r| r.since)) {
            Some(since) => AgentState::Working(since),
            None => AgentState::Idle(None),
        })
    }

    /// A new chat right away, as Ajustes presets it (no form); false when no agent is installed,
    /// said out loud unless `quiet`.
    pub(crate) fn start_chat(&mut self, quiet: bool) -> bool {
        let installed = self.installed_agents();
        if installed.is_empty() {
            if !quiet {
                self.toast("Nenhum agente instalado: instale o claude ou o codex e volte aqui.");
            }
            return false;
        }
        self.create_chat(&NewChat::from_prefs(&self.prefs, &installed), &installed);
        true
    }

    /// A chat from the form: named after its agent and folder, chosen in the chat.
    pub(crate) fn create_chat(&mut self, form: &NewChat, installed: &[Preset]) {
        let Some(preset) = installed.get(form.agent).or(installed.first()) else { return };
        let agent = preset.command.split_whitespace().next().unwrap_or("claude").to_string();
        let id = uuid::Uuid::new_v4().simple().to_string();
        let name = format!("{} · {}", preset.name, folder_label(&form.cwd));
        self.chats.push(Chat {
            id: id.clone(),
            agent,
            name,
            cwd: form.cwd.trim().to_string(),
            session: None,
            mode: form.mode,
            model: String::new(),
            effort: String::new(),
        });
        self.chat_agent = Some(id);
        self.save_chats();
    }

    pub(crate) fn remove_chat(&mut self, id: &str) {
        self.chat_stop(id);
        self.transcripts.remove(id);
        if let Some(path) = chat_items_path(id) {
            let _ = fs::remove_file(path);
        }
        self.chats.retain(|c| c.id != id);
        if self.chat_agent.as_deref() == Some(id) {
            self.chat_agent = None;
        }
        self.save_chats();
    }

    /// Runs the agent on `prompt` in the chat's folder; false when it couldn't start (said why).
    pub(crate) fn chat_send(&mut self, ctx: &Context, id: &str, prompt: &str) -> bool {
        let Some(i) = self.chats.iter().position(|c| c.id == id) else { return false };
        let chat = self.chats[i].clone();
        if self.chat_runs.contains_key(id) {
            self.toast(format!("{} ainda está respondendo: espere ou pare com Esc.", chat.name));
            return false;
        }
        let Some(program) = crate::ide::lsp::which(&chat.agent) else {
            self.toast(format!("{} não foi encontrado no PATH.", chat.agent));
            return false;
        };
        let resume = chat.session.is_some();
        let session = chat.session.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let mut command = crate::ide::command(program);
        command.args(chat_args(&chat.agent, chat.mode, &session, resume));
        command.args(crate::app::chat_footer::tuning_args(&chat.agent, &chat.model, &chat.effort));
        if chat.agent != "codex" {
            command.args(crate::app::chat_context::autocompact_args(&self.prefs));
        }
        // the Maestro's hands: the app's MCP and who it is (chat_tools.rs)
        command.args(crate::app::chat_tools::maestro_args(&chat.agent, &self.prefs, &chat));
        // Ajustes › Agentes › Controle › Acompanhar os agentes: its hooks report to the app as this chat
        if chat.agent != "codex"
            && let (Some(settings), Some(bridge)) = (self.hook_settings(), &self.bridge)
        {
            command.arg("--settings").arg(settings);
            command.env("MAGOAGENT_BRIDGE", &bridge.address).env("MAGOAGENT_TOKEN", &bridge.token).env("MAGOAGENT_CHAT_ID", id);
        }
        // Ajustes › Agentes › Contas: the agent on a key instead of its login
        if let Some((name, key)) = crate::app::ui::settings::api_env(&self.prefs, &chat.agent) {
            command.env(name, key);
        }
        command.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        if let Some(dir) = folder(&chat.cwd) {
            // the app's own projects folder is made on the first message; a folder elsewhere is
            // the person's, never made here
            if !dir.is_dir() {
                if data_dir().is_some_and(|d| dir.starts_with(d)) {
                    let _ = fs::create_dir_all(&dir);
                } else {
                    self.toast(format!("A pasta {} não existe.", dir.display()));
                    return false;
                }
            }
            command.current_dir(dir);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.toast(format!("Não foi possível rodar {}: {error}", chat.agent));
                return false;
            }
        };
        // the prompt, then its end: the agent reads stdin whole (Codex has no system prompt
        // flag: its instructions go in front of the first message)
        if let Some(mut stdin) = child.stdin.take() {
            let text = if chat.agent == "codex" && !resume {
                format!("{}\n\n{prompt}", crate::app::chat_tools::system_prompt(&self.prefs, &chat))
            } else {
                prompt.to_string()
            };
            std::thread::spawn(move || {
                let _ = stdin.write_all(text.as_bytes());
            });
        }
        let (tx, rx) = channel();
        let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let errors = std::thread::spawn(move || {
                let mut text = String::new();
                if let Some(mut stderr) = stderr {
                    let _ = stderr.read_to_string(&mut text);
                }
                text
            });
            if let Some(stdout) = stdout {
                for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Ok(value) = serde_json::from_str::<Value>(&line) {
                        let _ = tx.send(ChatEvent::Line(value));
                        ctx.request_repaint();
                    }
                }
            }
            let _ = tx.send(ChatEvent::Done(errors.join().unwrap_or_default()));
            ctx.request_repaint();
        });
        // Claude Code keeps the session under the id given; Codex names its thread (`poll_chats`)
        if chat.agent != "codex" && !resume {
            self.chats[i].session = Some(session);
            self.save_chats();
        }
        // the message shows at once; Claude Code's copy of it, read from its file later, is skipped
        let transcript = self.chat_transcript(id);
        transcript.items.push(ChatItem::User(prompt.to_string()));
        if chat.agent != "codex" {
            transcript.echoes += 1;
        }
        self.chat_runs.insert(id.to_string(), ChatRun { child, events: rx, since: self.now });
        // what Mandar de novo sends again if the login turns out to be what failed
        self.slash.sent.insert(id.to_string(), prompt.to_string());
        self.slash.retry.remove(id);
        // the avatar follows this chat: thinking now, saying each answer as it comes
        self.avatar_follow(id);
        self.avatar_set(AvatarState::Thinking);
        true
    }

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
    fn claude_transcript(cwd: &str, session: &str) -> Option<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_message_runs_the_agent_headless_and_carries_its_session() {
        let first = chat_args("claude", 0, "abc", false);
        assert_eq!(first, ["-p", "--output-format", "stream-json", "--verbose", "--permission-mode", "auto", "--session-id", "abc"]);
        assert!(chat_args("claude", 1, "abc", true).ends_with(&["bypassPermissions".into(), "--resume".into(), "abc".into()]));
        assert_eq!(chat_args("codex", 0, "t1", false), ["exec", "--json", "--skip-git-repo-check", "--approve-for-me", "-"]);
        assert_eq!(
            chat_args("codex", 1, "t1", true),
            ["exec", "--json", "--skip-git-repo-check", "--dangerously-bypass-approvals-and-sandbox", "resume", "t1", "-"]
        );
        assert_eq!(folder_label("C:\\Users\\edsu\\magoagent\\"), "magoagent");
        assert_eq!(folder_label("~"), "~");
    }

    #[test]
    fn codex_events_become_words_steps_and_errors() {
        let mut t = Transcript::new(PathBuf::new());
        let line = |s: &str| serde_json::from_str::<Value>(s).unwrap();
        codex_event(&mut t, &line(r#"{"type":"item.started","item":{"id":"c1","type":"command_execution","command":"ls"}}"#), 1.0);
        assert_eq!(t.running, [("c1".to_string(), "Bash ls".to_string())]);
        codex_event(
            &mut t,
            &line(r#"{"type":"item.completed","item":{"id":"c1","type":"command_execution","command":"ls","exit_code":0,"aggregated_output":"a\nb\n"}}"#),
            2.0,
        );
        codex_event(&mut t, &line(r#"{"type":"item.completed","item":{"id":"m1","type":"agent_message","text":"pronto"}}"#), 3.0);
        codex_event(&mut t, &line(r#"{"type":"turn.failed","error":{"message":"sem crédito"}}"#), 4.0);
        assert!(t.running.is_empty());
        let step = match &t.items[0] {
            ChatItem::Tool(s) => s.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!((step.name.as_str(), step.ended, step.failed, step.output.as_deref()), ("Bash", Some(2.0), false, Some("b")));
        assert_eq!(&t.items[1..], [ChatItem::Agent("pronto".into()), ChatItem::Error("sem crédito".into())]);
    }

    #[test]
    fn a_codex_command_keeps_its_whole_text_and_the_end_of_what_it_printed() {
        let mut t = Transcript::new(PathBuf::new());
        let huge = format!("começo\n{}\nfim", "x".repeat(40_000));
        let item = json!({"id": "c2", "type": "command_execution", "command": "cd web\nnpm test", "exit_code": 2, "aggregated_output": huge});
        codex_event(&mut t, &json!({"type": "item.completed", "item": item}), 5.0);
        let ChatItem::Tool(step) = &t.items[0] else { panic!("{:?}", t.items) };
        assert_eq!((step.input.as_str(), step.failed, step.error.as_deref()), ("cd web\nnpm test", true, Some("Saiu com o código 2")));
        assert!(step.log.starts_with("… (cortado") && step.log.ends_with("\nfim") && !step.log.contains("começo"), "only its end");
        assert!(step.log.len() < LOG_MAX + 64, "{}", step.log.len());
        let change = json!({"type": "item.completed", "item": {"id": "f1", "type": "file_change", "changes": [{"path": "a.rs", "kind": "update"}, {"path": "b.rs", "kind": "add"}]}});
        codex_event(&mut t, &change, 6.0);
        let ChatItem::Tool(edit) = &t.items[1] else { panic!() };
        assert_eq!(edit.files, ["a.rs", "b.rs"]);
        // a chat saved before these fields reads back with them empty
        let old: Step =
            serde_json::from_str(r#"{"id":"x","name":"Bash","what":"ls","started":1.0,"ended":2.0,"failed":false,"note":null,"diff":null,"output":"b"}"#)
                .unwrap();
        assert_eq!((old.input.as_str(), old.cwd, old.log.as_str(), old.files.len()), ("", None, "", 0));
    }

    #[test]
    fn a_chat_is_a_row_of_the_list_until_removed() {
        let mut app = DrawingApp::default();
        let installed = [Preset { name: "Codex".into(), command: "codex".into(), on: true }];
        app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 1 }, &installed);
        let rows = app.agent_rows();
        assert_eq!((rows.len(), rows[0].spec.label(), rows[0].spec.command.as_str()), (1, "Codex · projeto", "codex"));
        assert_eq!((app.chat_agent.as_ref(), app.agent_state(&rows[0].id)), (Some(&rows[0].id), AgentState::Idle(None)));
        app.remove_chat(&rows[0].id.clone());
        assert!(app.agent_rows().is_empty() && app.chat_agent.is_none());
    }
}

#[cfg(test)]
mod keep {
    use super::*;

    #[test]
    fn a_chats_conversation_survives_the_agents_check() {
        let mut app = DrawingApp::default();
        let installed = [Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
        app.create_chat(&NewChat { agent: 0, cwd: String::new(), mode: 0 }, &installed);
        let id = app.chat_agent.clone().unwrap();
        app.chat_transcript(&id).items.push(ChatItem::Error("x".into()));
        app.now = 10.0;
        app.check_agents();
        assert_eq!(app.transcripts.get(&id).map(|t| t.items.len()), Some(1));
    }
}
