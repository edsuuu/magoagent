use super::http::{self, Request};
use crate::prelude::*;
use std::sync::mpsc::{Receiver, Sender, channel};

pub(crate) const CREDENTIAL_ID: &str = "integration-whatsapp-gowa-credential";
pub(crate) const DEFAULT_BASE: &str = "http://127.0.0.1:3000";

pub(crate) fn default_base() -> String {
    DEFAULT_BASE.into()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Link {
    #[default]
    Unknown,
    Connected,
    NeedsPairing,
    Unavailable,
}

#[derive(Default)]
pub(crate) struct WhatsApp {
    pub(crate) credential_field: String,
    pub(crate) link: Link,
    pub(crate) checking: bool,
    generation: u64,
    channel: Option<(Sender<(u64, Link)>, Receiver<(u64, Link)>)>,
    credential: Option<String>,
}

impl WhatsApp {
    fn load_credential(&mut self) {
        #[cfg(test)]
        mock_keyring();

        if self.credential.is_none() {
            self.credential = Some(take_secret(CREDENTIAL_ID).unwrap_or_default());
        }
    }

    pub(crate) fn forget(&mut self) {
        self.generation += 1;
        self.link = Link::Unknown;
        self.checking = false;
        self.credential = None;
    }

    pub(crate) fn report(&self) -> &'static str {
        match self.link {
            Link::Unknown => "ainda não testado",
            Link::Connected => "gateway conectado",
            Link::NeedsPairing => "gateway disponível; WhatsApp precisa ser pareado nele",
            Link::Unavailable => "gateway indisponível ou resposta inválida",
        }
    }
}

pub(crate) fn local_base(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_end_matches('/');
    let rest = raw.strip_prefix("http://")?;
    let (host, port) = rest.split_once(':')?;

    if !matches!(host, "127.0.0.1" | "localhost") || port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let port = port.parse::<u16>().ok().filter(|port| *port >= 1024)?;

    Some(format!("http://{host}:{port}"))
}

fn gowa_status(body: &str) -> Link {
    let Ok(root) = serde_json::from_str::<Value>(body) else { return Link::Unavailable };
    let Some(devices) = root.get("results").and_then(Value::as_array) else { return Link::Unavailable };

    if devices.iter().any(|device| device.get("state").and_then(Value::as_str) == Some("logged_in")) { Link::Connected } else { Link::NeedsPairing }
}

fn check_gateway(base: &str, credential: &str) -> Link {
    let Some(base) = local_base(base) else { return Link::Unavailable };
    let headers = if credential.is_empty() { Vec::new() } else { vec![format!("Authorization: Basic {}", crate::render::base64(credential.as_bytes()))] };
    let request = Request { method: "GET", url: &format!("{base}/devices"), headers, body: None, timeout_seconds: 5 };

    match http::send(&request) {
        Ok(answer) if (200..300).contains(&answer.status) => gowa_status(&answer.body),
        _ => Link::Unavailable,
    }
}

impl DrawingApp {
    pub(crate) fn whatsapp_tick(&mut self, ctx: &Context) {
        let enabled = self.prefs.integrations.whatsapp;
        let whatsapp = &mut self.integrations.whatsapp;

        while let Some((generation, link)) = whatsapp.channel.as_ref().and_then(|(_, receiver)| receiver.try_recv().ok()) {
            if generation == whatsapp.generation {
                whatsapp.link = link;
                whatsapp.checking = false;
            }
        }

        if !enabled {
            if whatsapp.link != Link::Unknown || whatsapp.checking {
                whatsapp.forget();
            }

            return;
        }

        if whatsapp.checking {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    pub(crate) fn whatsapp_save_credential(&mut self, credential: &str) -> Result<String, String> {
        #[cfg(test)]
        mock_keyring();

        let credential = credential.trim();

        if !credential.is_empty() && (!credential.contains(':') || credential.chars().any(char::is_control)) {
            return Err("A credencial GOWA deve estar no formato usuário:senha, sem quebras de linha.".into());
        }

        set_secret(CREDENTIAL_ID, credential).map_err(|_| "O cofre do sistema recusou a credencial.".to_string())?;
        self.integrations.whatsapp.credential = Some(credential.to_string());
        Ok("Credencial guardada no cofre do sistema.".into())
    }

    pub(crate) fn whatsapp_remove_credential(&mut self) -> Result<String, String> {
        #[cfg(test)]
        mock_keyring();

        remove_secret(CREDENTIAL_ID).map_err(|_| "A credencial não foi removida do cofre do sistema.".to_string())?;
        self.integrations.whatsapp.credential = Some(String::new());
        Ok("Credencial removida do cofre do sistema.".into())
    }

    pub(crate) fn whatsapp_test(&mut self) -> bool {
        let whatsapp = &mut self.integrations.whatsapp;
        let Some(base) = local_base(&self.prefs.integrations.whatsapp_base) else { return false };

        if whatsapp.checking || cfg!(test) {
            return false;
        }

        whatsapp.load_credential();
        let credential = whatsapp.credential.clone().unwrap_or_default();
        let (sender, receiver) = channel();

        whatsapp.generation += 1;
        let generation = whatsapp.generation;

        whatsapp.checking = true;
        whatsapp.channel = Some((sender.clone(), receiver));

        std::thread::spawn(move || {
            let _ = sender.send((generation, check_gateway(&base, &credential)));
        });

        true
    }

    /// A click on WhatsApp's line on the island (and the MCP's `integrations` whatsapp open): with the gateway connected,
    /// its own page in the system's browser (only loopback, `local_base`); otherwise Ajustes › Integrações at its card,
    /// where it is turned on, set and tested.
    pub(crate) fn whatsapp_open(&mut self) -> String {
        if self.integrations.whatsapp.link == Link::Connected
            && let Some(base) = local_base(&self.prefs.integrations.whatsapp_base)
        {
            if !cfg!(test) {
                open_external(&base);
            }

            return "Abri a página do gateway do WhatsApp no navegador.".into();
        }

        self.open_settings_at("Integrações");
        self.integrations.scroll_to_whatsapp = true;
        self.raise_window = true;
        "Abri Ajustes › Integrações no WhatsApp.".into()
    }

    pub(crate) fn whatsapp_report(&self) -> String {
        if !self.prefs.integrations.whatsapp {
            return "WhatsApp: desligado.".into();
        }

        format!("WhatsApp: {}.", self.integrations.whatsapp.report())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("whatsapp/tests.rs");
}
