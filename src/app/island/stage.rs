//! What the agent in focus is doing, read from its conversation (`transcript/`): the tool it is on (Read, Edit, Bash...),
//! the file or command, the change an Edit made, what a command printed, and how the turn ended. Only what the
//! conversation has: what it does not say is not shown.

use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use crate::transcript::{ChatItem, Step};

/// Where the agent is, which picks the page of the Home card (working, editing, done and error).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Working,
    Editing,
    Done,
    Failed,
}

/// One tool of the turn, as the steps' list shows it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Move {
    pub(crate) verb: String,
    pub(crate) running: bool,
    pub(crate) failed: bool,
}

/// What the Home card tells about the turn in progress or just over.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Activity {
    pub(crate) phase: Phase,
    /// The tool now (or last): its name, its target (the file or the command) and the file when it names one.
    pub(crate) verb: String,
    pub(crate) target: String,
    pub(crate) file: Option<String>,
    pub(crate) folder: Option<String>,
    /// The step's place in the turn: this one of that many.
    pub(crate) place: (usize, usize),
    pub(crate) moves: Vec<Move>,
    /// An Edit's change: its lines ("+", "-" or " " first), where the first is, and how many were added and removed.
    pub(crate) patch: Vec<String>,
    pub(crate) patch_line: usize,
    pub(crate) diff: Option<(usize, usize)>,
    /// A command's tail of output (or why it failed).
    pub(crate) output: Vec<String>,
    /// The agent's last words of the turn, one line: what it says it did.
    pub(crate) headline: Option<String>,
}

/// The tools that write: the Home card shows their file as a tab with its change.
pub(crate) fn writes(name: &str) -> bool {
    matches!(name, "Edit" | "MultiEdit" | "Write" | "NotebookEdit")
}

/// An agent at work or whose turn ended a moment ago: the ones whose conversation the island reads.
pub(crate) fn recent(state: AgentState, now: f64) -> bool {
    match state {
        AgentState::Working(_) => true,
        AgentState::Idle(Some(since)) => now - since < crate::app::mascot::mago::CELEBRATE_FOR,
        _ => false,
    }
}

/// The agent's program as its card names it.
pub(crate) fn program_name(program: &str) -> String {
    match program {
        "claude" => "Claude Code".into(),
        "codex" => "Codex".into(),
        "gemini" => "Gemini".into(),
        other => other.to_string(),
    }
}

impl DrawingApp {
    /// For how long the agent in focus has been at this step (the change's lines appear over it), and the glimpse of the
    /// file a Read looks at, read once per file.
    pub(super) fn island_stage(&mut self, focus: Option<&str>, activity: Option<&Activity>, time: f64) -> (f32, Vec<(usize, String)>) {
        let island = &mut self.island;
        let key = activity.map(|a| format!("{}|{:?}|{}|{}|{}", focus.unwrap_or_default(), a.phase, a.verb, a.target, a.place.0)).unwrap_or_default();

        if key != island.stage_key {
            island.stage_key = key;
            island.stage_since = time;
        }

        let wanted = activity.filter(|a| a.phase == Phase::Working && a.verb == "Read").and_then(|a| Some(resolve(a.file.as_ref()?, a.folder.as_deref())));

        match wanted {
            Some(path) if island.glimpse_for.as_ref() != Some(&path) => {
                island.glimpse = glimpse(&path, 3);
                island.glimpse_for = Some(path);
            }
            Some(_) => {}
            None => (island.glimpse_for, island.glimpse) = (None, Vec::new()),
        }

        ((time - island.stage_since) as f32, island.glimpse.clone())
    }
}

/// The words of the step on the list: its tool, as Claude Code names it.
fn verb(step: &Step) -> String {
    step.name.rsplit("__").next().unwrap_or(&step.name).to_string()
}

fn last_lines(text: &str, count: usize) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim_end).filter(|line| !line.trim().is_empty()).collect();

    lines[lines.len().saturating_sub(count)..].iter().map(|line| line.to_string()).collect()
}

/// The activity in `items` (a conversation): the turn since the last message of the person. `working`: the agent is on
/// it now; `just_ended`: it ended a moment ago (the end is celebrated, or mourned). None when there is nothing to say.
pub(crate) fn activity(items: &[ChatItem], working: bool, just_ended: bool) -> Option<Activity> {
    if !working && !just_ended {
        return None;
    }

    let start = items.iter().rposition(|item| matches!(item, ChatItem::User(_))).map_or(0, |at| at + 1);
    let turn = &items[start..];
    let steps: Vec<&Step> = turn.iter().filter_map(|item| if let ChatItem::Tool(step) = item { Some(step) } else { None }).collect();
    let last = steps.last().copied();
    let after_last_step_is_error = turn.iter().rev().take_while(|item| !matches!(item, ChatItem::Tool(_))).any(|item| matches!(item, ChatItem::Error(_)));
    let failed = last.is_some_and(|step| step.failed) || after_last_step_is_error;

    let phase = match (failed, working) {
        (true, _) => Phase::Failed,
        (false, false) => Phase::Done,
        (false, true) if last.is_some_and(|step| writes(&step.name)) => Phase::Editing,
        (false, true) => Phase::Working,
    };

    let headline = turn.iter().rev().find_map(|item| match item {
        ChatItem::Agent(text) => text.lines().map(str::trim).find(|line| !line.is_empty()).map(|line| line.to_string()),
        _ => None,
    });

    // the command whose output tells how it went: the last Bash of the turn (a failed one first)
    let command = steps.iter().rev().find(|step| step.name == "Bash");

    let shown = match phase {
        Phase::Done | Phase::Failed => command.copied().or(last),
        _ => last,
    };

    let moves = steps
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(|step| Move { verb: verb(step), running: working && step.ended.is_none() && !step.failed, failed: step.failed })
        .collect();

    let output = shown
        .map(|step| match (&step.error, step.log.is_empty()) {
            (Some(error), _) if step.failed => last_lines(error, 3),
            (_, false) => last_lines(&step.log, 3),
            _ => step.output.iter().cloned().collect(),
        })
        .unwrap_or_default();

    let file = shown.and_then(|step| step.files.first().cloned());

    Some(Activity {
        phase,
        verb: shown.map(verb).unwrap_or_default(),
        target: shown.map(|step| step.what.clone()).unwrap_or_default(),
        folder: shown.and_then(|step| step.cwd.clone()),
        file,
        place: (steps.len(), steps.len()),
        moves,
        patch: shown.map(|step| step.patch.clone()).unwrap_or_default(),
        patch_line: shown.map_or(1, |step| step.patch_line),
        diff: shown.and_then(|step| step.diff),
        output,
        headline,
    })
}

/// Where a file the step names is: as given, or under the folder it ran in.
pub(crate) fn resolve(file: &str, folder: Option<&str>) -> std::path::PathBuf {
    let path = std::path::Path::new(file);

    match folder {
        Some(folder) if path.is_relative() => std::path::Path::new(folder).join(path),
        _ => path.to_path_buf(),
    }
}

/// The first lines of a file with something in them, numbered: a glimpse of what a Read looked at.
pub(crate) fn glimpse(path: &std::path::Path, count: usize) -> Vec<(usize, String)> {
    use std::io::Read;
    let mut head = Vec::new();
    let Ok(file) = std::fs::File::open(path) else { return Vec::new() };

    if file.take(8192).read_to_end(&mut head).is_err() {
        return Vec::new();
    }

    String::from_utf8_lossy(&head)
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .take(count)
        .map(|(index, line)| (index + 1, line.chars().take(90).collect()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, what: &str) -> Step {
        Step { id: name.into(), name: name.into(), what: what.into(), files: vec![what.into()], ..Default::default() }
    }

    fn done(mut step: Step) -> ChatItem {
        step.ended = Some(1.0);
        ChatItem::Tool(step)
    }

    #[test]
    fn nothing_is_said_while_the_agent_is_at_rest() {
        assert_eq!(activity(&[ChatItem::User("oi".into())], false, false), None);
    }

    #[test]
    fn a_read_in_progress_is_working_and_an_edit_is_editing_with_its_change() {
        let items = vec![ChatItem::User("conserta".into()), done(tool("Read", "src/invoice.ts")), ChatItem::Tool(tool("Bash", "ls"))];
        let now = activity(&items, true, false).unwrap();

        assert_eq!((now.phase, now.verb.as_str(), now.target.as_str(), now.place), (Phase::Working, "Bash", "ls", (2, 2)));
        assert_eq!(now.moves.iter().map(|m| (m.verb.as_str(), m.running)).collect::<Vec<_>>(), vec![("Read", false), ("Bash", true)]);
        let mut edit = tool("Edit", "src/invoice.ts");

        (edit.diff, edit.patch, edit.patch_line) = (Some((1, 1)), vec!["-a".into(), "+b".into()], 12);
        let items = vec![ChatItem::User("conserta".into()), done(tool("Read", "src/invoice.ts")), done(edit)];
        let now = activity(&items, true, false).unwrap();

        assert_eq!((now.phase, now.file.as_deref(), now.diff, now.patch_line), (Phase::Editing, Some("src/invoice.ts"), Some((1, 1)), 12));
        assert_eq!(now.patch, vec!["-a", "+b"]);
    }

    #[test]
    fn a_turn_that_just_ended_says_how_the_command_went_and_the_agents_last_words() {
        let mut bash = tool("Bash", "npm test");

        bash.log = "PASS tests/invoice.test.ts\nTests: 48 passed, 48 total\n".into();
        let items = vec![ChatItem::User("testa".into()), done(bash), ChatItem::Agent("48 testes passaram.\nTudo certo.".into())];
        let end = activity(&items, false, true).unwrap();

        assert_eq!((end.phase, end.target.as_str(), end.headline.as_deref()), (Phase::Done, "npm test", Some("48 testes passaram.")));
        assert_eq!(end.output, vec!["PASS tests/invoice.test.ts", "Tests: 48 passed, 48 total"]);
    }

    #[test]
    fn a_failed_tool_or_an_error_ending_the_turn_is_a_failure_with_the_reason() {
        let mut bash = tool("Bash", "npm test");

        (bash.failed, bash.error) = (true, Some("exit 1\nboom".into()));
        let items = vec![ChatItem::User("testa".into()), done(bash)];
        let fail = activity(&items, false, true).unwrap();

        assert_eq!((fail.phase, fail.output), (Phase::Failed, vec!["exit 1".to_string(), "boom".to_string()]));
        let items = vec![ChatItem::User("x".into()), done(tool("Read", "a.ts")), ChatItem::Error("API: 529".into())];

        assert_eq!(activity(&items, true, false).unwrap().phase, Phase::Failed);
        // an error before the last tool is old news
        let items = vec![ChatItem::User("x".into()), ChatItem::Error("API: 529".into()), done(tool("Read", "a.ts"))];

        assert_eq!(activity(&items, true, false).unwrap().phase, Phase::Working);
    }

    #[test]
    fn a_file_is_found_under_the_folder_it_ran_in_and_glimpsed() {
        assert_eq!(resolve("src/a.ts", Some("/p")), std::path::Path::new("/p").join("src/a.ts"));
        assert_eq!(resolve("/x/a.ts", Some("/p")), std::path::PathBuf::from("/x/a.ts"));
        let file = std::env::temp_dir().join(format!("ilha-glimpse-{}.ts", std::process::id()));

        std::fs::write(&file, "\nimport x\n\nexport const y = 1\nmais\n").unwrap();
        assert_eq!(glimpse(&file, 2), vec![(2, "import x".to_string()), (4, "export const y = 1".to_string())]);
        let _ = std::fs::remove_file(file);

        assert!(glimpse(std::path::Path::new("nao/existe.ts"), 2).is_empty());
    }
}
