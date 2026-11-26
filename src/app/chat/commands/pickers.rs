//! The lists a command's argument is picked from, as Claude Code's own screens give them in its terminal: after
//! `/model ` the models, `/effort ` the efforts, `/permissions ` the modes, `/resume ` this folder's earlier
//! conversations, `/mcp ` the servers, `/agents ` the subagents. A row is named "comando valor": Enter takes it.

use super::*;
use crate::app::chat::direct::CHAT_MODES;

/// A row of a command's list: `word value`, with what it is.
fn row(word: &str, value: &str, desc: impl Into<String>) -> Entry {
    Entry { name: format!("{word} {value}"), args: String::new(), desc: desc.into() }
}

/// The models, the chat's own marked.
pub(super) fn model_rows(chat: &Chat) -> Vec<Entry> {
    let mut all = models(&chat.agent);

    all.extend(more_models(&chat.agent).into_iter().filter(|more| !all.iter().any(|one| one.id == more.id)).collect::<Vec<_>>());
    let now = |id: &str| if chat.model == id { " · atual" } else { "" };
    let mut rows = vec![row("model", "padrão", format!("O padrão do agente{}", now("")))];

    rows.extend(all.iter().map(|choice| row("model", &choice.id, format!("{}{}", choice.name, now(&choice.id)))));
    rows
}

/// The efforts of the chat's model, its own marked.
pub(super) fn effort_rows(chat: &Chat) -> Vec<Entry> {
    let now = |id: &str| if chat.effort == id { " · atual" } else { "" };
    let mut rows = vec![row("effort", "padrão", format!("O padrão do modelo{}", now("")))];

    rows.extend(efforts(&chat.agent, &chat.model).iter().map(|choice| row("effort", &choice.id, format!("{}{}", choice.name, now(&choice.id)))));
    rows
}

/// The permission modes, the chat's own marked.
pub(super) fn permission_rows(chat: &Chat) -> Vec<Entry> {
    CHAT_MODES.iter().enumerate().map(|(index, mode)| row("permissions", mode, if chat.mode == index { "Atual" } else { "" })).collect()
}

/// The mode `arg` names (by its name or the start of it).
pub(crate) fn resolve_mode(arg: &str) -> Option<usize> {
    let arg = arg.trim().to_lowercase();

    (!arg.is_empty()).then_some(())?;
    CHAT_MODES.iter().position(|mode| mode.to_lowercase() == arg).or_else(|| CHAT_MODES.iter().position(|mode| mode.to_lowercase().starts_with(&arg)))
}

/// Claude Code's earlier conversations in `projects` (its folder for the chat's), newest first, but `held` (those open
/// in a chat already): (session id, first message, seconds since it was last written).
pub(crate) fn sessions(projects: &Path, held: &[String], now: std::time::SystemTime) -> Vec<(String, String, f64)> {
    let Ok(read) = fs::read_dir(projects) else { return Vec::new() };

    let mut found: Vec<(String, std::time::SystemTime, PathBuf)> = read
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let id = path.file_name()?.to_str()?.strip_suffix(".jsonl")?.to_string();
            let when = entry.metadata().ok()?.modified().ok()?;

            (!held.contains(&id)).then_some((id, when, path))
        })
        .collect();

    found.sort_by_key(|(_, when, _)| std::cmp::Reverse(*when));

    // ponytail: the 30 newest, each one's first message read from its file; an index if folders hold thousands
    found
        .into_iter()
        .take(30)
        .filter_map(|(id, when, path)| {
            let said = crate::transcript::first_prompt(&path)?;

            Some((id, said, now.duration_since(when).map_or(0.0, |age| age.as_secs_f64())))
        })
        .collect()
}

/// How long ago, in days past two of them ("há 6 dias").
fn when(seconds: f64) -> String {
    if seconds < 48.0 * 3600.0 { crate::app::ui::agents::ago(seconds) } else { format!("há {} dias", (seconds / 86400.0) as u64) }
}

/// The rows of `/resume`: a conversation by the start of its id and its first message.
pub(super) fn resume_rows(found: &[(String, String, f64)]) -> Vec<Entry> {
    found
        .iter()
        .map(|(id, said, age)| row("resume", &id[..id.len().min(8)], format!("{} · {}", clip(said.lines().next().unwrap_or_default()), when(*age))))
        .collect()
}

/// The MCP servers Claude Code has for `cwd`: the person's and the folder's in `~/.claude.json`, and the folder's
/// `.mcp.json`; by name, with how each one runs.
pub(crate) fn mcp_servers(claude_json: Option<&Value>, mcp_json: Option<&Value>, cwd: &str) -> Vec<(String, String)> {
    let mut servers = Vec::new();

    let groups =
        [claude_json.map(|json| &json["mcpServers"]), claude_json.map(|json| &json["projects"][cwd]["mcpServers"]), mcp_json.map(|json| &json["mcpServers"])];

    for (name, server) in groups.into_iter().flatten().filter_map(Value::as_object).flatten() {
        let how = server["url"].as_str().or(server["command"].as_str()).unwrap_or_default();

        if !servers.iter().any(|(known, _): &(String, String)| known == name) {
            servers.push((name.clone(), how.to_string()));
        }
    }

    servers
}

/// The subagents of a `.claude` folder (`agents/*.md`): by the name in their front matter, with what they are for.
pub(crate) fn subagents(claude: &Path, from: &str, out: &mut Vec<(String, String)>) {
    let Ok(read) = fs::read_dir(claude.join("agents")) else { return };
    let mut files: Vec<PathBuf> = read.flatten().map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|ext| ext == "md")).collect();

    files.sort();

    for path in files {
        let text = fs::read_to_string(&path).unwrap_or_default();

        let named = text
            .strip_prefix("---")
            .and_then(|rest| rest.split_once("\n---"))
            .and_then(|(front, _)| front.lines().find_map(|line| line.trim().strip_prefix("name:")));

        let stem = path.file_stem().map(|stem| stem.to_string_lossy().to_string()).unwrap_or_default();
        let name = named.map_or(stem, |name| name.trim().trim_matches(['"', '\'']).to_string());

        if !out.iter().any(|(known, _)| *known == name) {
            out.push((name, format!("{} ({from})", describe(&text))));
        }
    }
}

/// What the box says after picking subagent `name` in `/agents`: the request starts, for the person to finish.
pub(crate) fn subagent_request(name: &str) -> String {
    format!("Use o subagente {name} para ")
}

impl DrawingApp {
    /// `/permissions <modo>`: the chat's mode; alone, which it is and the others.
    pub(super) fn chat_set_mode(&mut self, id: &str, arg: &str) {
        let names = CHAT_MODES.join(", ");

        let Some(mode) = resolve_mode(arg) else {
            let now = self.chat_of(id).map_or(0, |chat| chat.mode);

            return self.chat_note(id, format!("Modo de permissão: {}\nUso: /permissions <modo>: {names}.", CHAT_MODES[now.min(CHAT_MODES.len() - 1)]));
        };

        if let Some(chat) = self.chats.iter_mut().find(|chat| chat.id == id) {
            chat.mode = mode;
        }

        self.save_chats();
        self.chat_note(id, format!("Modo de permissão: {}", CHAT_MODES[mode]));
    }

    /// `/resume <conversa>`: the earlier conversation of the folder whose id starts so goes on in this chat; the one
    /// it had stays in the list Conversas, as /clear leaves it.
    pub(super) fn chat_resume(&mut self, id: &str, arg: &str) {
        let Some(chat) = self.chat_of(id) else { return };
        let projects = crate::transcript::claude_dir().map(|dir| dir.join("projects").join(crate::transcript::project_folder(&chat.cwd)));
        let held: Vec<String> = self.chats.iter().filter_map(|other| other.session.clone()).collect();
        let found = projects.map(|dir| sessions(&dir, &held, std::time::SystemTime::now())).unwrap_or_default();
        let arg = arg.trim();
        let matching: Vec<&(String, String, f64)> = found.iter().filter(|(session, ..)| !arg.is_empty() && session.starts_with(arg)).collect();

        let [(session, said, _)] = matching.as_slice() else {
            let hint = if found.is_empty() { "Nenhuma conversa anterior nesta pasta." } else { "Escolha a conversa na lista do /resume." };

            return self.chat_note(id, hint);
        };

        let (session, said) = (session.clone(), said.clone());

        self.chat_stop(id);
        self.archive_conversation(id);

        if let Some(chat) = self.chats.iter_mut().find(|chat| chat.id == id) {
            chat.session = Some(session);
        }

        // read again from the session's file
        self.transcripts.remove(id);
        self.save_chats();
        self.chat_refresh(id);
        self.chat_note(id, format!("Conversa retomada: “{}”. A próxima mensagem continua dela.", clip(said.lines().next().unwrap_or_default())));
    }

    /// The rows `word`'s argument is picked from for chat `chat`; None when it takes no list. Looked up again every 5 s.
    pub(super) fn slash_args(&mut self, chat: &Chat, word: &str) -> Option<Vec<Entry>> {
        let word = word.to_lowercase();
        let key = format!("{}/{word}", chat.id);
        let claude = chat.agent != "codex";
        let fresh = self.slash.args.as_ref().is_some_and(|(at_key, at, _)| *at_key == key && self.now - at < 5.0);

        if !fresh {
            let rows = match word.as_str() {
                "model" => model_rows(chat),
                "effort" => effort_rows(chat),
                "permissions" => permission_rows(chat),
                "resume" if claude => {
                    let held: Vec<String> = self.chats.iter().filter_map(|other| other.session.clone()).collect();
                    let projects = crate::transcript::claude_dir().map(|dir| dir.join("projects").join(crate::transcript::project_folder(&chat.cwd)));

                    resume_rows(&projects.map(|dir| sessions(&dir, &held, std::time::SystemTime::now())).unwrap_or_default())
                }
                "mcp" if claude => {
                    let home = crate::transcript::claude_dir().and_then(|dir| dir.parent().map(|home| home.join(".claude.json")));
                    let user = home.and_then(|path| read_json::<Value>(&path));
                    let folder = read_json::<Value>(&Path::new(&chat.cwd).join(".mcp.json"));

                    mcp_servers(user.as_ref(), folder.as_ref(), &chat.cwd).into_iter().map(|(name, how)| row("mcp", &name, how)).collect()
                }
                "agents" if claude => {
                    let mut found = Vec::new();

                    subagents(&Path::new(&chat.cwd).join(".claude"), "projeto", &mut found);

                    if let Some(home) = std::env::home_dir() {
                        subagents(&home.join(".claude"), "pessoal", &mut found);
                    }

                    found.into_iter().map(|(name, desc)| row("agents", &name, desc)).collect()
                }
                _ => return None,
            };

            self.slash.args = Some((key, self.now, rows));
        }

        self.slash.args.as_ref().map(|(.., rows)| rows.clone())
    }
}
