//! What the app's agents are told about the system's temporary folder: a restart may empty it, and with it the clones,
//! worktrees and drafts an agent left there. With Ajustes › Agentes › "Trabalho fora da pasta temporária" on
//! (`Prefs::work_outside_temp`, the default) the chats' agents get it in their instructions (chat/tools.rs) and a Claude
//! Code recruited into a terminal gets it as `--append-system-prompt`.

use crate::prelude::*;

/// The system's temporary folders, as the agents here meet them (an agent on Windows may run in WSL).
pub(crate) fn temp_folders() -> &'static str {
    if cfg!(windows) {
        "%TEMP% nem no /tmp do WSL"
    } else if cfg!(target_os = "macos") {
        "/tmp nem em /var/folders"
    } else {
        "/tmp nem em outra pasta temporária do sistema"
    }
}

/// The rule, a sentence to append to an agent's instructions; empty with the setting off. No quote, dollar, backquote
/// or exclamation mark: a terminal's shell gets it typed between double quotes (`with_temp_rule`).
pub(crate) fn temp_rule(prefs: &Prefs) -> String {
    if !prefs.work_outside_temp {
        return String::new();
    }
    format!(
        " Pasta temporária: não guarde em {} nada que precise durar (clones, worktrees, rascunhos, arquivos do projeto), porque ela pode ser \
         apagada quando o computador reinicia. Trabalhe na pasta do projeto; worktrees do git ficam em <repo>/.magoagent/worktrees, onde o \
         MagoAgent já os põe. A pasta temporária é só para o descartável: logs, capturas, a saída de um comando de rascunho.",
        temp_folders()
    )
}

/// A recruit's terminal command with the rule as Claude Code's `--append-system-prompt`, after the model and effort the
/// Maestro gave it (where they are read back, maestro/tuning.rs) and before its task. Any other program, a command that
/// already appends to the prompt, or the setting off: as it was.
// ponytail: a Codex recruited into a terminal is not told (Codex takes no flag that appends to its instructions); give it
// the rule in front of its task when one is seen working in /tmp
pub(crate) fn with_temp_rule(command: &str, prefs: &Prefs) -> String {
    let command = command.trim();
    let rule = temp_rule(prefs);
    let Some(rest) = command.strip_prefix("claude").filter(|rest| rest.starts_with(char::is_whitespace) || rest.is_empty()) else {
        return command.to_string();
    };
    if rule.is_empty() || command.contains("--append-system-prompt") {
        return command.to_string();
    }
    let (_, task) = crate::app::maestro::recruit_flags(rest);
    let flags = rest[..rest.len() - task.len()].trim();
    let words = ["claude", flags, &format!("--append-system-prompt \"{}\"", rule.trim()), task.trim()];
    words.into_iter().filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_keeps_work_out_of_the_temp_folder_and_says_where_it_goes() {
        let mut prefs = Prefs::default();
        assert!(prefs.work_outside_temp, "on by default");
        let rule = temp_rule(&prefs);
        assert!(rule.contains(temp_folders()) && rule.contains("<repo>/.magoagent/worktrees") && rule.contains("logs"), "{rule}");
        // typed between double quotes into a terminal's shell
        assert!(!rule.contains(['"', '$', '`', '!', '\n']), "{rule}");
        prefs.work_outside_temp = false;
        assert_eq!(temp_rule(&prefs), "");
    }

    #[test]
    fn a_claude_recruited_into_a_terminal_gets_the_rule_after_its_model_and_before_its_task() {
        let mut prefs = Prefs::default();
        let told = with_temp_rule("claude --model sonnet --effort high \"rode os testes\"", &prefs);
        let flag = format!("--append-system-prompt \"{}\"", temp_rule(&prefs).trim());
        assert_eq!(told, format!("claude --model sonnet --effort high {flag} \"rode os testes\""));
        // the card still reads its model and effort
        assert_eq!(crate::app::maestro::tuning::of_command(&told, ""), ("claude".into(), "sonnet".into(), "high".into()));
        assert_eq!(with_temp_rule("claude", &prefs), format!("claude {flag}"));
        // another program, one already told, a shell's own command, or the setting off: as it was
        for command in ["codex \"rode os testes\"", "claude --append-system-prompt \"x\" oi", "claudette oi", "npm test"] {
            assert_eq!(with_temp_rule(command, &prefs), command);
        }
        prefs.work_outside_temp = false;
        assert_eq!(with_temp_rule("claude oi", &prefs), "claude oi");
    }
}
