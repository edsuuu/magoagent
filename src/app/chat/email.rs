//! Ajustes › Email: the bridge to a mail MCP server (Gmail or another) that the person sets up. The app starts
//! nothing and downloads nothing: it only hands the server's command to the chats' agents, next to the app's own
//! MCP, so they can read and write mail with it. Claude Code's own Gmail connector (claude.ai) needs none of this.

use crate::prelude::*;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

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

    /// What is wrong with the setup, if anything: for the line under the fields.
    pub(crate) fn status(&self) -> String {
        match (self.command.trim().is_empty(), self.found()) {
            (true, _) => "Falta o programa do servidor.".into(),
            (false, false) => format!("“{}” não está no PATH deste computador.", self.command.trim()),
            _ if !self.on => "Desligado: os agentes não veem o servidor.".into(),
            _ => "Ligado: os chats novos recebem o servidor.".into(),
        }
    }
}

/// Starts the server and asks it to `initialize` (MCP over stdin/stdout, one JSON per line): the name it answers with,
/// or why it did not. It is stopped right after; a server that takes more than `wait` to answer fails.
pub(crate) fn probe(command: &str, args: &[String], wait: Duration) -> Result<String, String> {
    let mut child = crate::editor::command(command)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| format!("não consegui iniciar {command}: {error}"))?;
    let hello = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": { "name": "magoagent", "version": env!("CARGO_PKG_VERSION") } } });
    let (mut stdin, stdout) = (child.stdin.take().ok_or("sem stdin")?, child.stdout.take().ok_or("sem stdout")?);
    writeln!(stdin, "{hello}").map_err(|error| format!("o servidor fechou: {error}"))?;
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // a server may print other lines first: the answer is the one that has a `result`
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(answer) = serde_json::from_str::<Value>(&line)
                && answer["id"] == 1
            {
                let _ = sender.send(answer);
                return;
            }
        }
    });
    let answer = receiver.recv_timeout(wait);
    let _ = child.kill();
    let _ = child.wait();
    let answer = answer.map_err(|_| format!("o servidor não respondeu em {} s", wait.as_secs()))?;
    match answer["result"]["serverInfo"]["name"].as_str() {
        Some(name) => Ok(name.to_string()),
        None => Err(answer["error"]["message"].as_str().unwrap_or("resposta sem serverInfo").to_string()),
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

    /// A fake MCP server (a shell echoing an `initialize` answer) proves the probe's handshake without any real server.
    #[cfg(unix)]
    #[test]
    fn the_probe_reads_the_servers_name() {
        let script = r#"read line; echo noise; echo '{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"gmail-falso"}}}'; sleep 5"#;
        assert_eq!(probe("sh", &["-c".into(), script.into()], Duration::from_secs(5)), Ok("gmail-falso".into()));
        assert!(probe("sh", &["-c".into(), "sleep 5".into()], Duration::from_millis(300)).is_err());
        assert!(probe("programa-que-nao-existe-1234", &[], Duration::from_secs(1)).is_err());
    }
}
