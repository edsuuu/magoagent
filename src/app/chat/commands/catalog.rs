use super::*;

/// A command of the menu: its name (no slash), the argument it takes, what it does.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) args: String,
    pub(crate) desc: String,
}

/// (name, argument, what it does, Codex has it too)
pub(super) const DEFS: [(&str, &str, &str, bool); 14] = [
    ("login", "[console|sso]", "Entrar na sua conta (abre o navegador)", true),
    ("logout", "", "Sair da conta", true),
    ("status", "", "Mostra quem está logado e os ajustes do chat", true),
    ("mcp", "", "Lista os servidores MCP", true),
    ("doctor", "", "Confere a instalação do Claude Code", false),
    ("update", "", "Atualiza o Claude Code", false),
    ("clear", "", "Começa uma conversa nova neste chat", true),
    ("model", "<nome>", "Troca o modelo do agente", true),
    ("effort", "<nível>", "Troca o esforço do agente", true),
    ("help", "", "Lista os comandos", true),
    ("btw", "<pergunta>", "Pergunta à parte, mesmo com o agente trabalhando; não entra na conversa", false),
    ("usage", "", "Mostra o consumo do plano: 5 horas e 7 dias", true),
    ("context", "", "Abre o painel Contexto", false),
    ("parar", "", "Para todos os agentes e esvazia a fila: nada continua sem uma mensagem sua", true),
];

/// The commands the chat answers itself, for `agent`; /compact is the agent's own.
pub(crate) fn catalog(agent: &str) -> Vec<Entry> {
    let mut list: Vec<Entry> =
        DEFS.iter().filter(|d| d.3 || agent != "codex").map(|d| Entry { name: d.0.into(), args: d.1.into(), desc: d.2.into() }).collect();

    if agent != "codex" {
        list.push(Entry { name: "compact".into(), args: "[instruções]".into(), desc: "Resume a conversa para liberar contexto (vai ao agente)".into() });
    }

    list
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Local {
    Login(String),
    Logout,
    Status,
    Mcp,
    Doctor,
    Update,
    Clear,
    Model(String),
    Effort(String),
    Help,
    /// A question on the side: answered from the conversation so far, without entering it.
    Btw(String),
    Usage,
    Context,
    /// Parar tudo (chat/stop.rs): also "para tudo" written alone.
    StopAll,
}

/// What the box's text means: None, it goes to the agent as a message; Some(Err), a local command
/// this agent doesn't have.
pub(crate) fn parse(text: &str, agent: &str) -> Option<Result<Local, String>> {
    if crate::app::chat::stop::stop_words(text) {
        return Some(Ok(Local::StopAll));
    }

    let rest = text.trim().strip_prefix('/')?;
    let (word, arg) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(w, a)| (w, a.trim()));
    let arg = arg.to_string();

    let local = match word.to_lowercase().as_str() {
        "login" => Local::Login(arg),
        "logout" => Local::Logout,
        "status" => Local::Status,
        "mcp" => Local::Mcp,
        "doctor" => Local::Doctor,
        "update" => Local::Update,
        "clear" => Local::Clear,
        "model" => Local::Model(arg),
        "effort" => Local::Effort(arg),
        "help" => Local::Help,
        "btw" => Local::Btw(arg),
        "usage" => Local::Usage,
        "context" => Local::Context,
        "parar" => Local::StopAll,
        _ => return None,
    };

    if agent == "codex" && matches!(local, Local::Doctor | Local::Update | Local::Btw(_) | Local::Context) {
        return Some(Err(format!("/{word} só existe no Claude Code.")));
    }

    Some(Ok(local))
}

/// What `/btw` asks the agent: the question, answered short from what the conversation already has, without tools and
/// without changing anything.
pub(crate) fn btw_prompt(question: &str) -> String {
    format!(
        "Pergunta à parte, fora da tarefa em andamento: responda curto, só com o que já está nesta conversa, sem usar \
         ferramentas e sem mudar nada.\n\n{question}"
    )
}

/// The agent CLI's arguments for a command that runs it.
pub(crate) fn cli_args(agent: &str, local: &Local) -> Result<Vec<String>, String> {
    let words: &[&str] = match (agent == "codex", local) {
        (false, Local::Login(arg)) => match arg.to_lowercase().as_str() {
            "" => &["auth", "login"],
            "claudeai" => &["auth", "login", "--claudeai"],
            "console" => &["auth", "login", "--console"],
            "sso" => &["auth", "login", "--sso"],
            _ => return Err("Use /login, /login console ou /login sso.".into()),
        },
        (true, Local::Login(arg)) if arg.is_empty() => &["login"],
        (true, Local::Login(_)) => return Err("No Codex, use só /login.".into()),
        (false, Local::Logout) => &["auth", "logout"],
        (true, Local::Logout) => &["logout"],
        (false, Local::Status) => &["auth", "status", "--json"],
        (true, Local::Status) => &["login", "status"],
        (_, Local::Mcp) => &["mcp", "list"],
        (false, Local::Doctor) => &["doctor"],
        (false, Local::Update) => &["update"],
        _ => return Err("Esse comando não roda o agente.".into()),
    };

    Ok(words.iter().map(|w| w.to_string()).collect())
}

/// `claude auth status --json` in words: whether there is a login, and who.
pub(crate) fn describe_status(json: &str) -> (bool, String) {
    let Ok(v) = serde_json::from_str::<Value>(json.trim()) else {
        return (false, json.trim().to_string());
    };

    if v["loggedIn"] != true {
        return (false, "Você não está logado. Digite /login para entrar.".into());
    }

    let get = |k: &str| v[k].as_str().filter(|s| !s.is_empty() && *s != "none").map(str::to_string);
    let mut text = format!("Logado como {}", get("email").unwrap_or_else(|| "uma conta".into()));

    for (key, prefix) in [("subscriptionType", "plano "), ("orgName", ""), ("authMethod", "acesso ")] {
        if let Some(value) = get(key) {
            text += &format!(" · {prefix}{value}");
        }
    }

    (true, text)
}

/// The text of a command's output with the terminal's colors off.
pub(super) fn clean(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }

    out.trim().to_string()
}

/// What a command found in `.claude/` says it is for: the front matter's description, else its
/// first line.
/// A description cut at 100 characters, with an ellipsis.
pub(super) fn clip(text: &str) -> String {
    if text.chars().count() > 100 { format!("{}…", text.chars().take(99).collect::<String>().trim_end()) } else { text.to_string() }
}

pub(super) fn describe(text: &str) -> String {
    let body = match text.strip_prefix("---").and_then(|t| t.split_once("\n---")) {
        Some((front, rest)) => {
            let found = front.lines().find_map(|l| l.trim().strip_prefix("description:")).map(|d| d.trim().trim_matches(['"', '\'']).to_string());

            if let Some(d) = found.filter(|d| !d.is_empty()) {
                return clip(&d);
            }

            rest
        }
        None => text,
    };

    clip(body.lines().map(|l| l.trim().trim_start_matches('#').trim()).find(|l| !l.is_empty()).unwrap_or_default())
}

/// The custom commands and skills of a `.claude` folder: `commands/*.md` (a subfolder is a prefix,
/// `pasta:nome`) and `skills/*/SKILL.md`.
pub(super) fn scan(claude: &Path, from: &str, out: &mut Vec<Entry>) {
    fn walk(dir: &Path, prefix: &str, from: &str, depth: usize, out: &mut Vec<Entry>) {
        let Ok(read) = fs::read_dir(dir) else { return };
        let mut files: Vec<_> = read.flatten().collect();

        files.sort_by_key(|e| e.file_name());

        for e in files {
            let (path, name) = (e.path(), e.file_name().to_string_lossy().to_string());

            if path.is_dir() && depth < 3 {
                walk(&path, &format!("{prefix}{name}:"), from, depth + 1, out);
            } else if let Some(stem) = name.strip_suffix(".md") {
                let desc = describe(&fs::read_to_string(&path).unwrap_or_default());

                out.push(Entry { name: format!("{prefix}{stem}"), args: String::new(), desc: format!("{desc} ({from})").trim().to_string() });
            }
        }
    }

    walk(&claude.join("commands"), "", from, 0, out);
    let Ok(skills) = fs::read_dir(claude.join("skills")) else { return };
    let mut dirs: Vec<_> = skills.flatten().collect();

    dirs.sort_by_key(|e| e.file_name());

    for e in dirs {
        if let Ok(text) = fs::read_to_string(e.path().join("SKILL.md")) {
            let named = text.strip_prefix("---").and_then(|t| t.split_once("\n---")).and_then(|(f, _)| f.lines().find_map(|l| l.trim().strip_prefix("name:")));
            let name = named.map_or_else(|| e.file_name().to_string_lossy().to_string(), |n| n.trim().trim_matches(['"', '\'']).to_string());

            out.push(Entry { name, args: String::new(), desc: format!("Skill · {} ({from})", describe(&text)) });
        }
    }
}

/// What Claude Code said it has when an answer began (the `init` of `claude -p`): its skills by the names it takes
/// (`plugin:nome` for a plugin's), and the plugins' folders, whose SKILL.md say what each one is for.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Known {
    pub(crate) skills: Vec<String>,
    pub(crate) plugins: Vec<(String, PathBuf)>,
}

impl Known {
    /// From an `init` line of the answer; None for any other.
    pub(crate) fn of_init(event: &Value) -> Option<Known> {
        if event["type"] != "system" || event["subtype"] != "init" {
            return None;
        }

        let skills = event["skills"].as_array()?.iter().filter_map(Value::as_str).map(String::from).collect();
        let plugins = event["plugins"].as_array().into_iter().flatten();
        let plugins = plugins.filter_map(|plugin| Some((plugin["name"].as_str()?.to_string(), PathBuf::from(plugin["path"].as_str()?))));

        Some(Known { skills, plugins: plugins.filter(|(_, path)| path.is_absolute()).collect() })
    }
}

/// What each SKILL.md under `dir` says it is for, by `prefix` and its folder's name; a folder without one is looked
/// into (the skills synced from the account are a few levels down).
fn skill_descs(dir: &Path, prefix: &str, depth: usize, out: &mut std::collections::HashMap<String, String>) {
    let Ok(read) = fs::read_dir(dir) else { return };

    for entry in read.flatten() {
        let path = entry.path();

        match fs::read_to_string(path.join("SKILL.md")) {
            Ok(text) => {
                out.entry(format!("{prefix}{}", entry.file_name().to_string_lossy())).or_insert_with(|| describe(&text));
            }
            Err(_) if depth < 3 && path.is_dir() => skill_descs(&path, prefix, depth + 1, out),
            Err(_) => {}
        }
    }
}

/// The folder's own commands first, then the person's, then the skills Claude Code said it has that no folder shows
/// (`known`: its plugins', those synced from the account, its own); a name already taken stays with the first.
pub(crate) fn discover(cwd: &Path, home: Option<&Path>, known: &Known) -> Vec<Entry> {
    let mut all = Vec::new();

    scan(&cwd.join(".claude"), "projeto", &mut all);

    if let Some(home) = home {
        scan(&home.join(".claude"), "pessoal", &mut all);
    }

    let mut seen = std::collections::HashSet::new();

    all.retain(|e| seen.insert(e.name.clone()));
    let mut descs = std::collections::HashMap::new();

    for (plugin, path) in &known.plugins {
        skill_descs(&path.join("skills"), &format!("{plugin}:"), 0, &mut descs);
    }

    if let Some(home) = home {
        skill_descs(&home.join(".claude/skills"), "", 0, &mut descs);
    }

    for name in known.skills.iter().filter(|name| seen.insert(name.to_string())) {
        let bare = name.rsplit(':').next().unwrap_or(name);
        let desc = descs.get(name).or_else(|| descs.get(bare)).map_or_else(|| "Skill do Claude Code".to_string(), |desc| format!("Skill · {desc}"));

        all.push(Entry { name: name.clone(), args: String::new(), desc });
    }

    all
}

/// The entries `query` (the text after the slash) leaves: the names that start with it, then the
/// ones that hold it.
pub(crate) fn filter(all: &[Entry], query: &str) -> Vec<Entry> {
    let q = query.to_lowercase();
    let starts = all.iter().filter(|e| e.name.to_lowercase().starts_with(&q));
    let holds = all.iter().filter(|e| !e.name.to_lowercase().starts_with(&q) && e.name.to_lowercase().contains(&q));

    starts.chain(holds).cloned().collect()
}
