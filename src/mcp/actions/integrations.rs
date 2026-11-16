//! Ajustes › Integrações through the MCP's `integrations`: what each one shows, its switch and its buttons. A secret
//! (GitHub's token) never comes through here: the person pastes it in Ajustes, so it stays out of the agents' logs.

use crate::app::integrations::spotify::Command;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn act_integrations(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_lowercase();
        let (integration, action) = (text("integration"), text("action"));
        match (integration.as_str(), action.as_str()) {
            ("", "" | "status") => {
                let hermes = match (crate::app::integrations::hermes::available(), self.prefs.integrations.hermes) {
                    (false, _) => "desativado por enquanto.",
                    (true, true) => "ligado.",
                    (true, false) => "desligado.",
                };
                let soloist = if cfg!(any(windows, target_os = "macos")) { String::new() } else { format!("\nSoloist: {}", self.soloist_report()) };
                Ok(format!("{}\nSpotify: {}{soloist}\nHermes Agent: {hermes}\n{}", self.github_report(), self.spotify_report(), self.whatsapp_report()))
            }
            ("hermes", _) if !crate::app::integrations::hermes::available() => Err("o Hermes Agent está desativado por enquanto".into()),
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
            ("github", "prs") => self.github_prs_report(&text("filter")),
            // the pull request's id and the text keep their case and their lines
            ("github", "pr" | "pr_body" | "pr_comment") => {
                let pr = args["pr"].as_str().unwrap_or_default().trim().to_string();
                if pr.is_empty() {
                    return Err("diga o `pr`: dono/repo#número ou o endereço dele (como `prs` lista)".into());
                }
                let words = args["text"].as_str().unwrap_or_default();
                match action.as_str() {
                    "pr" => self.github_pr_report(&pr),
                    "pr_body" => self
                        .github_save_body(&pr, words)
                        .map(|()| format!("Enviando a descrição nova de {pr} ao GitHub: o resultado aparece em `status` em alguns segundos.")),
                    _ => self
                        .github_comment(&pr, words)
                        .map(|()| format!("Enviando o comentário em {pr} ao GitHub: o resultado aparece em `status` em alguns segundos.")),
                }
            }
            ("spotify", "" | "status") => Ok(format!("Spotify: {}", self.spotify_report())),
            ("spotify", "on" | "off") => {
                self.prefs.integrations.spotify = action == "on";
                Ok(format!("Spotify {}.", if action == "on" { "ligado" } else { "desligado" }))
            }
            ("soloist", _) if cfg!(any(windows, target_os = "macos")) => Err("o Soloist do Spotify só existe no Linux".into()),
            ("soloist", "" | "status") => Ok(format!("Soloist: {}", self.soloist_report())),
            ("soloist", "on" | "off") => {
                self.prefs.integrations.soloist = action == "on";
                Ok(format!("Soloist {}.", if action == "on" { "ligado" } else { "desligado" }))
            }
            ("spotify" | "soloist", command) if Command::parse(command).is_some() => {
                match Command::parse(command).is_some_and(|command| self.spotify_control(command)) {
                    true => Ok("Pedido ao Spotify.".into()),
                    false => Err("o Spotify está desligado em Ajustes › Integrações, ou não está aberto neste computador".into()),
                }
            }
            ("hermes", "on" | "off") => {
                self.prefs.integrations.hermes = action == "on";
                Ok(format!("Hermes Agent {} na ilha.", if action == "on" { "ligado" } else { "desligado" }))
            }
            ("hermes", "" | "status") => {
                Ok(format!("Hermes Agent: {} {}", if self.prefs.integrations.hermes { "ligado." } else { "desligado." }, self.act_hermes("status")?))
            }
            ("hermes", "install" | "remove") => self.act_hermes(&action),
            ("whatsapp", "" | "status") => Ok(self.whatsapp_report()),
            ("whatsapp", "on" | "off") => {
                self.prefs.integrations.whatsapp = action == "on";
                Ok(format!("WhatsApp {}.", if action == "on" { "ligado" } else { "desligado" }))
            }
            ("whatsapp", "test") if !self.prefs.integrations.whatsapp => Err("o WhatsApp está desligado: ligue antes (action on)".into()),
            ("whatsapp", "test") => match self.whatsapp_test() {
                true => Ok("Testando o gateway: a resposta vem em status.".into()),
                false => Err("URL inválida ou teste já em curso: a URL precisa ser http://127.0.0.1:porta ou http://localhost:porta".into()),
            },
            ("whatsapp", "remove_credential") => self.whatsapp_remove_credential(),
            _ => Err(format!("integração ou ação desconhecida: {integration} {action}")),
        }
    }
}
