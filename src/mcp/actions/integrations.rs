//! Ajustes › Integrações through the MCP's `integrations`: what each one shows, its switch and its buttons. A secret
//! (GitHub's token) never comes through here: the person pastes it in Ajustes, so it stays out of the agents' logs.

use crate::prelude::*;

impl DrawingApp {
    pub(super) fn act_integrations(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_lowercase();
        let (integration, action) = (text("integration"), text("action"));
        match (integration.as_str(), action.as_str()) {
            ("", "" | "status") => Ok(self.github_report()),
            ("github", "" | "status") => Ok(self.github_report()),
            ("github", "on" | "off") => {
                self.prefs.integrations.github = action == "on";
                Ok(format!("GitHub {}.", if action == "on" { "ligado" } else { "desligado" }))
            }
            ("github", "test") if !self.prefs.integrations.github => Err("o GitHub está desligado: ligue antes (action on)".into()),
            ("github", "test") => match self.github_test() {
                true => Ok("Pedido ao GitHub: quem é o token e os pull requests de agora. A resposta vem num aviso e em status.".into()),
                false => Err("sem token do GitHub (ou um teste já em curso): a pessoa cola o token em Ajustes › Integrações".into()),
            },
            ("github", "remove_token") => self.github_remove_token(),
            _ => Err(format!("integração ou ação desconhecida: {integration} {action}")),
        }
    }
}
