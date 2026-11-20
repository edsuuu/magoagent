//! Ajustes › Integrações through the MCP's `integrations`: what each one shows, its switch and its buttons. A secret
//! (GitHub's token, Soloist's key) never comes through here: the person pastes it in Ajustes, so it stays out of the
//! agents' logs.

use crate::app::integrations::spotify::Command;
use crate::app::integrations::spotify::setup::Job;
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

                Ok(format!("{}\nSpotify: {}{soloist}\nHermes Agent: {hermes}", self.github_report(), self.spotify_report()))
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
            ("github", "pr" | "pr_body" | "pr_comment" | "pr_approve" | "pr_merge") => {
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
                    "pr_approve" => self
                        .github_approve(&pr)
                        .map(|()| format!("Enviando a aprovação de {pr} ao GitHub: o resultado aparece em `status` em alguns segundos.")),
                    "pr_merge" => {
                        self.github_merge(&pr).map(|()| format!("Pedindo ao GitHub para mesclar {pr}: o resultado aparece em `status` em alguns segundos."))
                    }
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
            // one switch for every player
            ("soloist", "on" | "off") => {
                self.prefs.integrations.spotify = action == "on";
                Ok(format!("Spotify e Soloist {}.", if action == "on" { "ligados" } else { "desligados" }))
            }
            // Configurar (integrations/spotify/setup): its steps, and what the person presses there; never the key
            ("soloist", "setup") => Ok(self.soloist_setup_report()),
            ("soloist", "key") => Err(
                "a chave do Soloist nunca passa pelo MCP: a pessoa gera no painel do Spotify e cola em Ajustes › Integrações › Spotify › Configurar o Soloist"
                    .into(),
            ),
            ("soloist", "update" | "install") => self.soloist_setup_run(Job::Download),
            ("soloist", "enable") => {
                let name = self.integrations.soloist_setup.name_field.clone();

                self.soloist_setup_run(Job::TurnOn { key: None, name })
            }
            ("soloist", "start") => self.soloist_setup_run(Job::Start),
            ("soloist", "stop") => self.soloist_setup_run(Job::Stop),
            ("spotify" | "soloist", "dance") => Ok(self.luna_report()),
            ("spotify" | "soloist", "dance_on" | "dance_off") => {
                self.prefs.integrations.luna_dances = action == "dance_on";
                Ok(self.luna_report())
            }
            ("spotify" | "soloist", action) if super::spotify::ACTIONS.contains(&action) => self.act_spotify(action, args),
            ("spotify" | "soloist", command) if Command::parse(command).is_some() => {
                Command::parse(command).map_or(Err(String::new()), |command| self.spotify_do(command)).map(|()| "Pedido ao Spotify.".into())
            }
            ("hermes", "on" | "off") => {
                self.prefs.integrations.hermes = action == "on";
                Ok(format!("Hermes Agent {} na ilha.", if action == "on" { "ligado" } else { "desligado" }))
            }
            ("hermes", "" | "status") => {
                Ok(format!("Hermes Agent: {} {}", if self.prefs.integrations.hermes { "ligado." } else { "desligado." }, self.act_hermes("status")?))
            }
            ("hermes", "install" | "remove") => self.act_hermes(&action),
            _ => Err(format!("integração ou ação desconhecida: {integration} {action}")),
        }
    }
}
