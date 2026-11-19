//! A chat with an agent installed here, with no terminal in between: each message runs the agent's
//! own headless mode (`claude -p`, `codex exec`) in the chat's folder, under the login the agent
//! keeps, and the conversation goes on by its session id. Claude Code's words come from the session
//! file it writes (transcript.rs); Codex's from the events it prints. The chats live in chats.json
//! beside the drawing.

use crate::app::maestro::avatar::AvatarState;
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::transcript::{ChatItem, LOG_MAX, Step, Transcript, claude_dir, log_tail, project_folder, step_mut};
use std::io::{BufRead, Read, Write};
use std::sync::mpsc::{Receiver, channel};
mod archive;
mod sync;
mod title;
pub(crate) use archive::{is_new_session_mark, new_session_mark};
pub(crate) use title::ChatTitles;

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
    /// The agent's model and effort for the next message; empty, its defaults (chat/footer.rs).
    #[serde(default)]
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) effort: String,
    /// A recruit of the Maestro (maestro/mod.rs `recruit_in`): an ordinary agent on the canvas, named like a terminal's,
    /// without the Maestro's hands.
    #[serde(default)]
    pub(crate) recruit: bool,
    /// Renamed by the person (Renomear on its row): `name` is its title, not the conversation's own.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) named: bool,
    /// The workspace a recruit was born in (its recruiter's, the one on screen): the lists group it there with its card
    /// off view. Empty: the space whose canvas shows its card.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) space: String,
    /// Opened by the chat screen and not talked to yet: only that screen shows it, with no card on the canvas and
    /// nothing saved, and it goes when another chat is picked. Its first message makes it a chat.
    #[serde(skip)]
    pub(crate) draft: bool,
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
    /// As Ajustes presets it: the agent (among the installed ones), the folder and the leave. The folder is the last one
    /// the person worked in (the first of the recent ones, while it is there), else Ajustes'.
    pub(crate) fn from_prefs(prefs: &Prefs, installed: &[Preset]) -> NewChat {
        let agent = installed.iter().position(|p| p.command.starts_with(&prefs.chat_agent)).unwrap_or(0);
        let last = prefs.folders.recent.first().map(String::as_str).filter(|path| folder(path).is_some_and(|dir| dir.is_dir()));
        let cwd = last.unwrap_or(if OLD_FOLDERS.contains(&prefs.chat_folder.trim()) { "" } else { prefs.chat_folder.trim() });
        let cwd = if cwd.is_empty() { projects_dir().map(|d| d.display().to_string()).unwrap_or_default() } else { cwd.to_string() };
        // the agent's own default leave (Claude Code's defaultMode) before Ajustes'
        let program = installed.get(agent).and_then(|p| p.command.split_whitespace().next()).unwrap_or("claude");
        let mode = crate::app::chat::footer::agent_mode(program).unwrap_or(prefs.chat_mode);

        NewChat { agent, cwd, mode: mode.min(CHAT_MODES.len() - 1) }
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
    /// Since when its turn is over though the reader has not seen the end of its output: the agent printed its last event
    /// (`turn_over`) or its process exited. Something it left running (a server started in the background, an MCP
    /// call still waiting) holds the output open, and the end the reader waits for may never come (`RUN_GRACE`).
    over: Option<f64>,
    /// Whether this run still names the chat's folder (`takes_folder`): a session's first, until it said where it is.
    pub(super) fresh: bool,
}

/// How long a run whose turn is over waits for the end of its output before it ends all the same: the lines still on
/// their way come in meanwhile. A process still alive then is stopped (`ChatRun`'s drop), as Esc does.
pub(crate) const RUN_GRACE: f64 = 5.0;

/// Whether `event` is the agent's last of its turn: Claude Code's `result`, Codex's `turn.completed` or `turn.failed`.
pub(crate) fn turn_over(agent: &str, event: &Value) -> bool {
    match agent {
        "codex" => matches!(event["type"].as_str(), Some("turn.completed" | "turn.failed")),
        _ => event["type"] == "result",
    }
}

impl ChatRun {
    /// A run of a stand-in process, for the tests: nothing comes from it.
    #[cfg(test)]
    pub(crate) fn stand_in(child: std::process::Child, since: f64) -> Self {
        Self { child, events: channel().1, since, over: None, fresh: false }
    }

    /// A run of a stand-in process whose events are what the tests send: as long as they keep the sender, its output
    /// is open, as when a process it started holds it.
    #[cfg(test)]
    pub(crate) fn fed(child: std::process::Child, since: f64) -> (Self, std::sync::mpsc::Sender<ChatEvent>) {
        let (sender, events) = channel();

        (Self { child, events, since, over: None, fresh: false }, sender)
    }
}

impl Drop for ChatRun {
    /// Closing the app (or dropping a chat) must not leave the agent running on its own.
    fn drop(&mut self) {
        if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            // on Windows `kill` ends only the wrapper, not what it started
            if cfg!(windows) {
                let _ = crate::editor::command("taskkill").args(["/T", "/F", "/PID", &self.child.id().to_string()]).output();
            }

            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
impl ChatRun {
    /// An answer the tests hold as running since `since`: its process (the test binary listing its tests) ends at
    /// once and says nothing; the chat works until the run is taken out.
    pub(crate) fn held(since: f64) -> Self {
        let exe = std::env::current_exe().expect("the test binary");
        let child = crate::editor::command(exe).arg("--list").stdout(std::process::Stdio::null()).spawn().expect("the test binary runs");

        Self { child, events: channel().1, since, over: None, fresh: false }
    }
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
        // an agent in WSL names its folders as Linux sees them: on Windows that is the distro's share
        None if cfg!(windows) && cwd.starts_with('/') && !cwd.starts_with("//") => match crate::app::system::wsl::wsl_distro_in_use() {
            Some(distro) => Some(PathBuf::from(crate::app::system::wsl::windows_path(&distro.name, cwd))),
            None => Some(PathBuf::from(cwd)),
        },
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

/// Whether `cwd` is the app's own folder, the one a chat runs in with no folder picked (`file::projects_dir`).
pub(crate) fn no_folder(cwd: &str) -> bool {
    let plain = |path: &str| path.trim().trim_end_matches(['/', '\\']).replace('\\', "/").to_lowercase();

    crate::file::projects_dir().is_some_and(|dir| plain(cwd) == plain(&dir.display().to_string()))
}

/// A chat's name: its agent and its folder's, only the agent with no folder picked.
pub(crate) fn chat_name(agent: &str, cwd: &str) -> String {
    if no_folder(cwd) { agent.to_string() } else { format!("{agent} · {}", folder_label(cwd)) }
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
            // on Windows, also the ones inside WSL (system/wsl.rs)
            let found = |p: &Preset| p.command.split_whitespace().next().is_some_and(|program| agent_cli(program).is_some());

            self.chat_installed = Some((self.now, self.prefs.presets.iter().filter(|p| found(p)).cloned().collect()));
        }

        self.chat_installed.as_ref().map(|(_, list)| list.clone()).unwrap_or_default()
    }

    /// The installed agents a chat can run, (name, program), as the last look found them.
    pub(crate) fn chat_providers(&self) -> Vec<(String, String)> {
        let installed = self.chat_installed.as_ref().map(|(_, list)| list.as_slice()).unwrap_or_default();

        installed
            .iter()
            .filter_map(|p| p.command.split_whitespace().next().map(|program| (p.name.clone(), program.to_string())))
            .filter(|(_, program)| matches!(program.as_str(), "claude" | "codex"))
            .collect()
    }

    pub(crate) fn is_chat(&self, id: &str) -> bool {
        self.chats.iter().any(|c| c.id == id)
    }

    /// The chats as rows of the agents list, ahead of the terminals'.
    pub(crate) fn chat_rows(&self) -> Vec<AgentRow> {
        self.chats.iter().filter(|c| !c.draft).map(|c| self.chat_row(c)).collect()
    }

    /// The draft the chat screen shows (none: a chat is talked to or none is open), as a row.
    pub(crate) fn draft_rows(&self) -> Vec<AgentRow> {
        self.chats.iter().filter(|c| c.draft).map(|c| self.chat_row(c)).collect()
    }

    fn chat_row(&self, chat: &Chat) -> AgentRow {
        // a recruit is not a Maestro (the spec's default): it goes under the one that recruited it (list/view.rs)
        let spec = TerminalSpec {
            name: chat.name.clone(),
            cwd: chat.cwd.clone(),
            command: chat.agent.clone(),
            chat: true,
            maestro: !chat.recruit,
            ..Default::default()
        };

        AgentRow { id: chat.id.clone(), spec, state: self.chat_state(&chat.id).unwrap_or(AgentState::Idle(None)), mission: None, space: String::new() }
    }

    /// Working while a message is being answered, then idle since its answer ended; None for what isn't a chat.
    pub(crate) fn chat_state(&self, id: &str) -> Option<AgentState> {
        if self.person_asked(id) {
            return Some(AgentState::Waiting);
        }

        self.is_chat(id).then(|| match self.chat_runs.get(id).map(|r| r.since).or_else(|| self.chat_cmds.get(id).map(|r| r.since)) {
            Some(since) => AgentState::Working(since),
            None => AgentState::Idle(self.chat_ended.get(id).copied()),
        })
    }

    /// A new chat right away, as Ajustes presets it (no form); false when no agent is installed,
    /// said out loud unless `quiet`.
    pub(crate) fn start_chat(&mut self, quiet: bool) -> bool {
        self.start_chat_as(quiet, false)
    }

    /// `start_chat` as a draft (`Chat::draft`): the chat screen's, made a chat by its first message.
    pub(crate) fn start_draft_chat(&mut self, quiet: bool) -> bool {
        self.start_chat_as(quiet, true)
    }

    fn start_chat_as(&mut self, quiet: bool, draft: bool) -> bool {
        let installed = self.installed_agents();

        if installed.is_empty() {
            if !quiet {
                self.toast("Nenhum agente instalado: instale o claude ou o codex e volte aqui.");
            }

            return false;
        }

        self.create_chat_as(&NewChat::from_prefs(&self.prefs, &installed), &installed, draft);
        true
    }

    /// A chat from the form: named after its agent and folder, chosen in the chat.
    pub(crate) fn create_chat(&mut self, form: &NewChat, installed: &[Preset]) {
        self.create_chat_as(form, installed, false);
    }

    fn create_chat_as(&mut self, form: &NewChat, installed: &[Preset], draft: bool) {
        let Some(preset) = installed.get(form.agent).or(installed.first()) else { return };
        let agent = preset.command.split_whitespace().next().unwrap_or("claude").to_string();
        let id = uuid::Uuid::new_v4().simple().to_string();
        let name = chat_name(&preset.name, &form.cwd);

        self.chats.push(Chat {
            id: id.clone(),
            agent,
            name,
            cwd: form.cwd.trim().to_string(),
            session: None,
            mode: form.mode,
            model: String::new(),
            effort: String::new(),
            recruit: false,
            named: false,
            space: String::new(),
            draft,
        });

        self.chat_agent = Some(id.clone());

        if draft {
            return;
        }

        // its card on the canvas shown, selected (chat/node.rs)
        if let Some(i) = self.add_chat_node(&id, None) {
            self.selected = vec![i];
        }

        self.save_chats();
    }

    /// Before its first message, the chat goes to another installed agent (`agent`: claude, codex);
    /// its name follows, the model and effort go back to that agent's own.
    pub(crate) fn chat_switch_agent(&mut self, id: &str, agent: &str) -> Result<String, String> {
        let installed = self.installed_agents();
        let preset = installed.iter().find(|p| p.command.split_whitespace().next() == Some(agent)).ok_or_else(|| format!("{agent} não está instalado."))?;
        let mode = crate::app::chat::footer::agent_mode(agent).unwrap_or(self.prefs.chat_mode).min(CHAT_MODES.len() - 1);
        let chat = self.chats.iter_mut().find(|c| c.id == id).ok_or("esse chat não existe")?;

        if chat.session.is_some() {
            return Err("O provedor só muda antes da primeira mensagem.".into());
        }

        chat.agent = agent.to_string();
        chat.mode = mode;

        // a name the person gave stays
        if !chat.named {
            chat.name = chat_name(&preset.name, &chat.cwd);
        }

        chat.model.clear();
        chat.effort.clear();
        let name = chat.name.clone();

        self.save_chats();
        Ok(format!("Chat agora com {name}."))
    }

    pub(crate) fn remove_chat(&mut self, id: &str) {
        self.chat_stop(id);
        self.transcripts.remove(id);

        if let Some(path) = self.autosave.as_ref().map(|autosave| autosave.with_file_name("chats").join(format!("{id}.json"))) {
            let _ = fs::remove_file(path);
        }

        self.chats.retain(|c| c.id != id);
        self.drop_chat_node(id);
        // its team's fold, if it had one, is not remembered for nothing
        self.set_team_open(id, false);

        if self.chat_agent.as_deref() == Some(id) {
            self.chat_agent = None;
        }

        self.save_chats();
    }

    /// Runs the agent on `prompt` in the chat's folder; false when it couldn't start (said why).
    pub(crate) fn chat_send(&mut self, ctx: &Context, id: &str, prompt: &str) -> bool {
        let Some(i) = self.chats.iter().position(|c| c.id == id) else { return false };
        let chat = self.chats[i].clone();

        // still answering: in the queue, sent when it is done (`chat_nodes_tick`), as Claude Code does
        if self.chat_runs.contains_key(id) {
            self.queue_message(id, prompt.to_string());

            return true;
        }

        // WSL's first probe still runs: the message waits in the queue and goes once it ends (`chat_nodes_tick`)
        if wsl_settling() {
            self.chat_note(id, "Preparando o WSL: a mensagem sai assim que ele responder.");
            self.queue_message(id, prompt.to_string());
            ctx.request_repaint_after(std::time::Duration::from_millis(250));

            return true;
        }

        let Some(cli) = agent_cli(&chat.agent) else {
            self.toast(format!("{} não foi encontrado no PATH nem no WSL.", chat.agent));

            return false;
        };

        let wsl = cli.wsl().is_some();
        let resume = chat.session.is_some();

        // the first message names it, as Claude Code titles a conversation: its first line now, Haiku's title
        // in a few seconds (a name given with Renomear stays)
        if !resume
            && !chat.named
            // a message resuming another session is named by what that session was about
            && let source = crate::transcript::resumed_prompt(prompt).map_or_else(|| prompt.to_string(), |(_, first)| first)
            && let Some(title) = crate::transcript::prompt_title(&source)
        {
            (self.chats[i].name, self.chats[i].named) = (title, true);
            self.save_chats();
            self.ask_chat_title(Some(ctx), id, &source);
        }

        let session = chat.session.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let mut command = cli.command();

        command.args(chat_args(&chat.agent, chat.mode, &session, resume));
        command.args(crate::app::chat::footer::tuning_args(&chat.agent, &chat.model, &chat.effort));

        if chat.agent != "codex" {
            command.args(crate::app::chat::context::autocompact_args(&self.prefs));
        }

        // the Maestro's hands: the app's MCP and who it is (chat/tools.rs)
        command.args(crate::app::chat::tools::maestro_args(&chat.agent, &self.prefs, &chat, wsl));

        // Ajustes › Chat › Hooks nos agentes do app: its hooks report to the app as this chat
        if chat.agent != "codex"
            && let Some(settings) = self.hook_settings(wsl)
        {
            command.arg("--settings").arg(settings);
        }

        // the bridge, as a terminal's shell has it: `magoagent list|ask|…` as this chat, by its own token
        command.envs(self.chat_env(id));

        if let Some(dir) = folder(&chat.cwd) {
            // the app's own folders (~/.magoagent, the old data/projetos) are made on the first message; a folder
            // elsewhere is the person's, never made here
            if !dir.is_dir() {
                if data_dir().is_some_and(|d| dir.starts_with(d)) || projects_dir().is_some_and(|d| dir.starts_with(d)) {
                    let _ = fs::create_dir_all(&dir);
                } else {
                    self.toast(format!("A pasta {} não existe.", dir.display()));

                    return false;
                }
            }

            command.current_dir(dir);
        }

        // in WSL: by wsl.exe, its paths translated (system/wsl.rs)
        let mut command = cli.finish(command);

        command.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.toast(format!("Não foi possível rodar {}: {error}", chat.agent));

                return false;
            }
        };

        // a draft becomes a chat with its first message: its card on the canvas, and it is kept
        if self.chats[i].draft {
            self.chats[i].draft = false;
            self.add_chat_node(id, None);
            self.save_chats();
        }

        // the prompt, then its end: the agent reads stdin whole (Codex has no system prompt
        // flag: its instructions go in front of the first message)
        if let Some(mut stdin) = child.stdin.take() {
            let text = if chat.agent == "codex" && !resume {
                format!("{}\n\n{prompt}", crate::app::chat::tools::system_prompt(&self.prefs, &chat))
            } else {
                prompt.to_string()
            };

            let text = if wsl { crate::app::system::wsl::wsl_prompt_paths(&text) } else { text };

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

        self.chat_runs.insert(id.to_string(), ChatRun { child, events: rx, since: self.now, over: None, fresh: !resume });
        // what Mandar de novo sends again if the login turns out to be what failed
        self.slash.sent.insert(id.to_string(), prompt.to_string());
        self.slash.retry.remove(id);
        // the avatar follows this chat: thinking now, saying each answer as it comes
        self.avatar_follow(id);
        self.avatar_set(AvatarState::Thinking);
        true
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
        let installed = [Preset::new("Codex", "codex")];

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
    fn the_provider_changes_only_before_the_first_message() {
        let mut app = DrawingApp::default();
        let installed = vec![Preset::new("Claude Code", "claude"), Preset::new("Codex", "codex")];

        app.chat_installed = Some((app.now, installed.clone()));
        app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &installed);
        let id = app.chat_agent.clone().unwrap();
        let names: Vec<String> = app.chat_providers().into_iter().map(|(_, p)| p).collect();

        assert_eq!(names, ["claude", "codex"]);
        app.chats[0].model = "claude-opus-5-5".into();
        app.chat_switch_agent(&id, "codex").unwrap();
        assert_eq!((app.chats[0].agent.as_str(), app.chats[0].name.as_str(), app.chats[0].model.as_str()), ("codex", "Codex · projeto", ""));
        assert!(app.chat_switch_agent(&id, "opencode").is_err(), "not installed");
        app.chats[0].session = Some("s".into());
        assert!(app.chat_switch_agent(&id, "claude").is_err(), "after the first message it stays");
    }

    #[test]
    fn a_chats_conversation_survives_the_agents_check() {
        let mut app = DrawingApp::default();
        let installed = [Preset::new("Claude Code", "claude")];

        app.create_chat(&NewChat { agent: 0, cwd: String::new(), mode: 0 }, &installed);
        let id = app.chat_agent.clone().unwrap();

        app.chat_transcript(&id).items.push(ChatItem::Error("x".into()));
        app.now = 10.0;
        app.check_agents();
        assert_eq!(app.transcripts.get(&id).map(|t| t.items.len()), Some(1));
    }
}
