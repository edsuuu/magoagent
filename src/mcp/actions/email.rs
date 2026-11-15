//! Ajustes › Email through the MCP's `email`: the mail MCP server the chats' agents get. The test starts the server, so
//! it is the person's to ask for in Ajustes; here the Maestro only reads and sets what is configured.

use crate::app::chat::email::GMAIL_EXAMPLE;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn act_email(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let email = &mut self.prefs.email;
        match text("action").as_str() {
            "" | "status" => Ok(format!("Servidor de email: {} {} — {}", email.command, email.args, email.status())),
            "on" if email.command.trim().is_empty() => Err("falta o programa: use action set com `command` (ou gmail_example)".into()),
            "on" | "off" => {
                email.on = text("action") == "on";
                Ok(if email.on { "Servidor de email dado aos agentes dos chats." } else { "Servidor de email desligado." }.into())
            }
            "set" if text("command").is_empty() => Err("diga o programa em `command` e os argumentos em `args`".into()),
            "set" => {
                (email.command, email.args) = (text("command"), text("args"));
                Ok(format!("Servidor de email: {} {}. {}", email.command, email.args, email.status()))
            }
            "gmail_example" => {
                (email.command, email.args) = (GMAIL_EXAMPLE.0.into(), GMAIL_EXAMPLE.1.into());
                Ok(format!(
                    "Preenchi {} {} (o login do Google é do servidor, pelas instruções do pacote). Use action on para dar aos agentes.",
                    email.command, email.args
                ))
            }
            other => Err(format!("ação desconhecida: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maestro_sets_the_mail_bridge_up() {
        let mut app = DrawingApp::default();
        assert!(app.act_email(&json!({ "action": "on" })).is_err(), "nothing to give yet");
        app.act_email(&json!({ "action": "set", "command": "npx", "args": "-y pacote" })).unwrap();
        app.act_email(&json!({ "action": "on" })).unwrap();
        assert!(app.prefs.email.server().is_some());
        app.act_email(&json!({ "action": "off" })).unwrap();
        assert!(app.prefs.email.server().is_none());
        app.act_email(&json!({ "action": "gmail_example" })).unwrap();
        assert_eq!(app.prefs.email.command, "npx");
        assert!(app.act_email(&json!({ "action": "x" })).is_err());
    }
}
