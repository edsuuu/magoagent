//! How the Maestro recruits: the way the person set the agents to run (Ajustes › Agentes › Invocação dos agentes), and
//! the model and the effort it picks for each recruit. `recruit_agent`'s `model` and `effort` go into the recruit's
//! command as its agent's own flags, right after the program; a chat recruit reads them back (`recruit_flags`), a
//! terminal runs them; and the dispatch says them: "modelo Sonnet 5.5 · esforço Alto".

use crate::app::chat::footer::{agent_default, claude_alias, effort_name, label, models, more_models, tuning_args};
use crate::prelude::*;

/// The efforts a recruit can be given, as Claude Code and Codex name them.
pub(crate) const EFFORTS: [&str; 6] = ["low", "medium", "high", "xhigh", "max", "ultra"];

/// `command` with `model` and `effort` as its agent's flags right after the program (`claude --model sonnet --effort
/// high "…"`, `codex -m gpt-5-codex -c model_reasoning_effort=high "…"`). A command with no program (only the task) gets
/// Claude Code's in front, which a chat recruit reads whatever its agent. What the command names already stays; an
/// effort that does not exist, or a model a shell would read as more than a word, is dropped.
pub(crate) fn with_tuning(command: &str, model: &str, effort: &str) -> String {
    let command = command.trim();
    let model = model.trim();
    let model = if model.chars().all(|c| c.is_ascii_alphanumeric() || "-._:/".contains(c)) { model } else { "" };
    let effort = effort.trim().to_lowercase();
    let effort = if EFFORTS.contains(&effort.as_str()) { effort.as_str() } else { "" };
    let (program, rest) = command.split_once(char::is_whitespace).unwrap_or((command, ""));
    let agent = matches!(program, "claude" | "codex");
    let (named, _) = crate::app::maestro::recruit_flags(if agent { rest } else { command });
    let model = if named.model.is_some() { "" } else { model };
    let effort = if named.effort.is_some() { "" } else { effort };
    let flags = tuning_args(if program == "codex" { "codex" } else { "claude" }, model, effort).join(" ");
    let words = if agent { [program, flags.as_str(), rest.trim()] } else { ["", flags.as_str(), command] };

    words.into_iter().filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The agent, model and effort a recruit's command starts with: its program (`fallback` when it names none) and the
/// flags before its task.
pub(crate) fn of_command(command: &str, fallback: &str) -> (String, String, String) {
    let command = command.trim();
    let (program, rest) = command.split_once(char::is_whitespace).unwrap_or((command, ""));
    let (agent, rest) = if matches!(program, "claude" | "codex") { (program, rest) } else { (fallback, command) };
    let (flags, _) = crate::app::maestro::recruit_flags(rest);

    (agent.to_string(), flags.model.unwrap_or_default(), flags.effort.unwrap_or_default())
}

/// The names of a recruit's model and effort, as the chat's menu gives them ("Sonnet 5.5", "Alto"); what was not
/// picked, its agent's own default (Claude Code's settings, Codex's config), else "padrão".
pub(crate) fn names(agent: &str, model: &str, effort: &str) -> (String, String) {
    let (default_model, default_effort) = agent_default(agent);
    let model = if model.is_empty() { default_model } else { model.to_string() };
    let effort = if effort.is_empty() { default_effort } else { effort.to_string() };

    let model = match model.as_str() {
        "" => "padrão".to_string(),
        id => {
            let id = if agent == "codex" { id.to_string() } else { claude_alias(id) };

            label(&models(agent).into_iter().chain(more_models(agent)).collect::<Vec<_>>(), &id)
        }
    };

    let effort = if effort.is_empty() { "padrão".to_string() } else { effort_name(&effort) };

    (model, effort)
}

/// What the dispatch says of them: "modelo Sonnet 5.5 · esforço Alto".
pub(crate) fn describe(agent: &str, model: &str, effort: &str) -> String {
    let (model, effort) = names(agent, model, effort);

    format!("modelo {model} · esforço {effort}")
}

/// What the Maestro is told about recruiting: how the person set the agents to run, followed unless they ask for the
/// other way in so many words; that an agent is always a recruit (a terminal it opens by hand with `claude` would skip
/// the setting); and that it picks every recruit's model and effort and says them.
pub(crate) fn recruit_brief(prefs: &Prefs) -> String {
    let (mode, other) = match prefs.agent_mode {
        AgentMode::Chat => ("num chat do app (um cartão no canvas)", "terminal"),
        AgentMode::Terminal => ("num terminal com o Claude Code completo", "chat"),
    };

    format!(
        " Recrutar: a pessoa escolheu em Ajustes › Agentes › Invocação dos agentes que os agentes rodem {mode}. Chame recruit_agent sem mode e ele \
         segue esse ajuste; só passe mode \"{other}\" quando ela pedir com essas palavras para aquele agente. Um agente (claude, codex) é sempre \
         recruit_agent: create_terminal e add_elements são para comandos e shells e não seguem o ajuste. Em cada recrutamento escolha model e \
         effort à altura da tarefa: haiku com low ou medium para o simples e rápido, sonnet com medium ou high para o dia a dia, opus com high ou \
         xhigh para o difícil e longo (no Codex, um modelo da lista dele); e diga à pessoa o que escolheu como a resposta do recruit_agent traz \
         (\"modelo Sonnet 5.5 · esforço Alto\")."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_and_effort_go_in_as_the_agents_own_flags_and_read_back_by_name() {
        assert_eq!(with_tuning("claude", "sonnet", "high"), "claude --model sonnet --effort high");
        assert_eq!(with_tuning("claude \"revise o diff\"", "opus", "XHigh"), "claude --model opus --effort xhigh \"revise o diff\"");
        assert_eq!(with_tuning("codex \"faça X\"", "gpt-5-codex", "low"), "codex -m gpt-5-codex -c model_reasoning_effort=low \"faça X\"");
        // only the task: Claude Code's flags in front, which a chat recruit reads whatever its agent
        assert_eq!(with_tuning("revise o diff", "haiku", ""), "--model haiku revise o diff");
        // nothing picked, a command as it was; what it names already stays, an effort that does not exist goes
        assert_eq!(with_tuning(" claude  ", "", ""), "claude");
        assert_eq!(with_tuning("claude --model opus \"x\"", "haiku", "high"), "claude --effort high --model opus \"x\"");
        assert_eq!(with_tuning("claude", "sonnet", "enorme"), "claude --model sonnet");
        assert_eq!(with_tuning("claude", "opus[1m]; rm -rf ~", "max"), "claude --effort max", "never more than a word to the shell");
        // and read back, Codex's flags too
        let tuned = |command: &str| of_command(command, "claude");

        assert_eq!(tuned("claude --model sonnet --effort high \"x\""), ("claude".into(), "sonnet".into(), "high".into()));
        assert_eq!(tuned("codex -m gpt-5-codex -c model_reasoning_effort=low \"x\""), ("codex".into(), "gpt-5-codex".into(), "low".into()));
        assert_eq!(of_command("--model haiku revise", "codex"), ("codex".into(), "haiku".into(), String::new()));
        // by the menu's names
        assert_eq!(describe("claude", "sonnet", "high"), "modelo Sonnet 5.5 · esforço Alto");
        assert_eq!(describe("claude", "claude-opus-5-5", "xhigh"), "modelo Opus 5.5 · esforço Extra");
        assert_eq!(names("claude", "haiku", "low"), ("Haiku 5.5".into(), "Baixo".into()));
    }

    /// What the Maestro sends with no mode follows Invocação dos agentes, and the dispatch says the model and effort it
    /// picked: in its answer, on the timeline and on a terminal recruit's header.
    #[test]
    fn a_recruit_follows_the_setting_and_says_its_model_and_effort() {
        let mut app = DrawingApp::default();

        app.prefs.agent_mode = AgentMode::Chat;
        let mut luna = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 600.0, height: 400.0 });

        luna.terminal = Some(TerminalSpec { name: "Luna".into(), cwd: "/p".into(), maestro: true, ..Default::default() });
        let from = luna.id.clone();

        app.scene.elements.push(luna);

        let recruit = |app: &mut DrawingApp, command: &str, mode: Option<AgentMode>| {
            let (reply, said) = std::sync::mpsc::channel();

            app.recruit_in(&from, command, false, mode, reply);
            said.recv().unwrap().unwrap()
        };

        let terminals = |app: &DrawingApp| app.scene.elements.iter().filter_map(|e| e.terminal.clone()).filter(|t| !t.maestro).collect::<Vec<_>>();
        // as the bridge has it from recruit_agent: no mode, its model and effort
        let said = recruit(&mut app, &with_tuning("claude \"revise o diff\"", "sonnet", "high"), None);
        let chat = app.chats.iter().find(|chat| chat.recruit).expect("a chat, as the setting says");

        assert_eq!((chat.model.as_str(), chat.effort.as_str()), ("sonnet", "high"));
        assert!(terminals(&app).is_empty(), "no terminal");
        assert!(said.contains("e conectado (um chat do app; modelo Sonnet 5.5 · esforço Alto)"), "{said}");
        let event = app.agent_events.back().unwrap();

        assert_eq!((event.kind, event.what.as_str()), (EventKind::Started, "recrutado por Luna (um chat do app): modelo Sonnet 5.5 · esforço Alto"));
        // a terminal only when asked in so many words; its header says them too
        let said = recruit(&mut app, &with_tuning("claude", "opus", "xhigh"), Some(AgentMode::Terminal));

        assert!(said.contains("(um terminal; modelo Opus 5.5 · esforço Extra)"), "{said}");
        let spec = terminals(&app).pop().unwrap();
        // and the temporary folder's rule after them (maestro/temp_folder.rs)
        let told = |command: &str, prefs: &Prefs| crate::app::maestro::temp_folder::with_temp_rule(command, prefs);

        assert_eq!(spec.command, told("claude --model opus --effort xhigh", &app.prefs));
        assert!(spec.command.contains("--append-system-prompt"));
        assert_eq!(crate::app::terminal::card::recruit_tag(&spec).as_deref(), Some("Opus 5.5 · Extra"));
        // no model picked: Ajustes' for the recruits, read back all the same
        app.prefs.recruit_model = "haiku".into();
        let said = recruit(&mut app, "claude --effort low \"x\"", Some(AgentMode::Terminal));

        assert_eq!(terminals(&app).pop().unwrap().command, told("claude --model haiku --effort low \"x\"", &app.prefs));
        assert!(said.contains("modelo Haiku 5.5 · esforço Baixo"), "{said}");
        // a shell or a Maestro's terminal has no tag
        assert_eq!(crate::app::terminal::card::recruit_tag(&TerminalSpec { command: "cargo test".into(), ..Default::default() }), None);
        assert_eq!(crate::app::terminal::card::recruit_tag(&TerminalSpec { command: "claude --model opus".into(), maestro: true, ..Default::default() }), None);
    }

    #[test]
    fn the_maestro_is_told_how_the_person_set_the_agents_and_to_pick_the_model_and_effort() {
        let mut prefs = Prefs { agent_mode: AgentMode::Chat, ..Default::default() };
        let brief = recruit_brief(&prefs);

        assert!(brief.contains("rodem num chat do app") && brief.contains("só passe mode \"terminal\" quando ela pedir"), "{brief}");
        assert!(brief.contains("create_terminal e add_elements") && brief.contains("escolha model e effort"), "{brief}");
        prefs.agent_mode = AgentMode::Terminal;
        let brief = recruit_brief(&prefs);

        assert!(brief.contains("num terminal com o Claude Code") && brief.contains("só passe mode \"chat\""), "{brief}");

        // in the Maestro's own prompt, not in a recruit's
        let chat = |recruit: bool| crate::app::chat::direct::Chat {
            id: "c".into(),
            agent: "claude".into(),
            name: "Luna".into(),
            cwd: "/p".into(),
            session: None,
            mode: 0,
            model: String::new(),
            effort: String::new(),
            recruit,
            named: false,
            draft: false,
        };

        assert!(crate::app::chat::tools::system_prompt(&prefs, &chat(false)).contains(&brief));
        assert!(!crate::app::chat::tools::system_prompt(&prefs, &chat(true)).contains(&brief));
    }
}
