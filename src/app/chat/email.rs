//! The bridge to a mail MCP server (Gmail or another) set up through the MCP's `email` (`mcp/actions/email.rs`). The app
//! starts nothing and downloads nothing: it only hands the server's command to the chats' agents, next to the app's own
//! MCP, so they can read and write mail with it. Claude Code's own Gmail connector (claude.ai) needs none of this.

use crate::prelude::*;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct EmailPrefs {
    pub(crate) on: bool,
    /// The program of the MCP server ("npx") and its arguments on one line (`-y pacote`; quotes group a word).
    pub(crate) command: String,
    pub(crate) args: String,
}

/// An example the person may fill in with a button: the Gmail MCP server most people use (it asks for its own OAuth setup).
pub(crate) const GMAIL_EXAMPLE: (&str, &str) = ("npx", "-y @gongrzhe/server-gmail-autoauth-mcp");

/// The name the agents see the server under.
pub(crate) const SERVER_NAME: &str = "email";

/// `line` as words, a double-quoted stretch being one word.
pub(crate) fn split_args(line: &str) -> Vec<String> {
    let (mut words, mut word, mut quoted, mut started) = (Vec::new(), String::new(), false, false);

    for ch in line.chars() {
        match ch {
            '"' => (quoted, started) = (!quoted, true),
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }

    if started {
        words.push(word);
    }

    words
}

impl EmailPrefs {
    /// The server for the agents' MCP config, when the bridge is on and has a program.
    pub(crate) fn server(&self) -> Option<Value> {
        let command = self.command.trim();

        (self.on && !command.is_empty()).then(|| json!({ "command": command, "args": split_args(&self.args) }))
    }

    /// Codex takes its servers as `-c` overrides, in TOML.
    pub(crate) fn codex_args(&self) -> Vec<String> {
        let Some(server) = self.server() else { return Vec::new() };
        let quoted = |text: &str| format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
        let args: Vec<String> = split_args(&self.args).iter().map(|arg| quoted(arg)).collect();

        vec![
            "-c".into(),
            format!("mcp_servers.{SERVER_NAME}.command={}", quoted(server["command"].as_str().unwrap_or_default())),
            "-c".into(),
            format!("mcp_servers.{SERVER_NAME}.args=[{}]", args.join(", ")),
        ]
    }

    /// Whether the program is found on this computer's PATH.
    pub(crate) fn found(&self) -> bool {
        crate::editor::lsp::which(self.command.trim()).is_some()
    }

    /// What is wrong with the setup, if anything: for the MCP's `email` status.
    pub(crate) fn status(&self) -> String {
        match (self.command.trim().is_empty(), self.found()) {
            (true, _) => "Falta o programa do servidor.".into(),
            (false, false) => format!("“{}” não está no PATH deste computador.", self.command.trim()),
            _ if !self.on => "Desligado: os agentes não veem o servidor.".into(),
            _ => "Ligado: os chats novos recebem o servidor.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_of_arguments_splits_on_spaces_and_quotes() {
        assert_eq!(split_args("-y @gongrzhe/server-gmail-autoauth-mcp"), ["-y", "@gongrzhe/server-gmail-autoauth-mcp"]);
        assert_eq!(split_args(r#"  run "C:\Meus Programas\mail.js"  --x "" "#), ["run", r"C:\Meus Programas\mail.js", "--x", ""]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn only_a_bridge_that_is_on_and_has_a_program_reaches_the_agents() {
        let mut email = EmailPrefs { on: false, command: "npx".into(), args: "-y pacote".into() };

        assert!(email.server().is_none() && email.codex_args().is_empty());
        email.on = true;
        assert_eq!(email.server(), Some(json!({ "command": "npx", "args": ["-y", "pacote"] })));
        assert_eq!(email.codex_args()[1], "mcp_servers.email.command=\"npx\"");
        assert_eq!(email.codex_args()[3], "mcp_servers.email.args=[\"-y\", \"pacote\"]");
        email.command = "  ".into();
        assert!(email.server().is_none());
    }
}
