//! Ajustes › Integrações: services outside the app shown on the island, one file each, after Coucou's (MIT)
//! integrations. Each is off until its switch is turned on, and off it never reaches the network. What is secret (a
//! token, a credential) lives in the system's keychain, never in `preferences.json`, a log or an address.

use crate::app::maestro::outside::Badge;
use crate::prelude::*;

pub(crate) mod github;
pub(crate) mod hermes;
pub(crate) mod http;
pub(crate) mod spotify;
pub(crate) mod whatsapp;

/// The island's ids of the integrations start with this.
pub(crate) const ITEM_PREFIX: &str = "integration:";

/// What Ajustes keeps of the integrations: the switches and what is not secret.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct IntegrationPrefs {
    pub(crate) github: bool,
    pub(crate) spotify: bool,
    /// Spotify's Soloist (Linux): what it plays shows on the island as the Spotify app's does.
    pub(crate) soloist: bool,
    /// Hermes Agent's sessions show on the island (its plugin is installed apart, by a button).
    pub(crate) hermes: bool,
    pub(crate) whatsapp: bool,
    #[serde(default = "whatsapp::default_base")]
    pub(crate) whatsapp_base: String,
}

#[derive(Default)]
pub(crate) struct Integrations {
    pub(crate) github: github::GitHub,
    pub(crate) spotify: spotify::Spotify,
    pub(crate) whatsapp: whatsapp::WhatsApp,
    /// The token as it is typed in Ajustes, until Salvar puts it in the keychain: never in `Prefs`.
    pub(crate) token_field: String,
    /// Ajustes' "Instalar…" of Hermes was pressed: what it will write is shown, waiting for the yes.
    pub(crate) hermes_confirm: bool,
    /// Soloist's Configurar (Linux): its steps, the key's field and the work running.
    pub(crate) soloist_setup: spotify::setup::Setup,
    /// What was pressed on the island's cards (they are drawn where the app is only read): done by the next tick.
    pub(crate) pressed: std::cell::RefCell<Vec<String>>,
}

/// One line of an integration's card on the island; with an `action`, a press does it.
pub(crate) struct ItemRow {
    pub(crate) text: String,
    pub(crate) color: Color32,
    pub(crate) action: Option<String>,
}

/// An integration as the island shows it: a pill of the grid and, in focus, its card.
pub(crate) struct IslandItem {
    /// "integration:github".
    pub(crate) id: String,
    pub(crate) name: &'static str,
    /// Beside the name, as an agent's program: "@login".
    pub(crate) kind: String,
    pub(crate) color: Color32,
    pub(crate) badge: Option<Badge>,
    /// Something failed there: the pill's wizard shows it (and the island sounds its error).
    pub(crate) failed: bool,
    pub(crate) rows: Vec<ItemRow>,
    /// The card's buttons: (label, action).
    pub(crate) chips: Vec<(&'static str, String)>,
    /// A picture at the card's corner (the cover of what plays).
    pub(crate) cover: Option<egui::TextureId>,
}

const INK: Color32 = Color32::from_rgb(0xf5, 0xf6, 0xf8);
const DIM: Color32 = Color32::from_rgb(0x8e, 0x93, 0x9c);
const GREEN: Color32 = Color32::from_rgb(0x34, 0xd3, 0x99);
const RED: Color32 = Color32::from_rgb(0xff, 0x8d, 0x97);
/// Each service's mini in its own colour, Coucou's catalog (core/pills.ts): GitHub red, WhatsApp its green (Spotify has
/// its own, in spotify/).
const GITHUB: Color32 = Color32::from_rgb(0xf4, 0x50, 0x5e);
const WHATSAPP: Color32 = Color32::from_rgb(0x25, 0xd3, 0x66);
const AMBER: Color32 = Color32::from_rgb(0xf5, 0xa5, 0x24);

/// The pages a card or a button of the integrations may open: the services' own, never an address an answer made up.
pub(crate) fn page_allowed(url: &str) -> bool {
    (url.starts_with("https://github.com/") || url == spotify::setup::DASHBOARD) && !url.chars().any(|letter| letter.is_control() || letter == ' ')
}

/// Opens one of the integrations' pages in the system's browser; never in a test.
pub(crate) fn open_page(url: &str) {
    if page_allowed(url) && !cfg!(test) {
        open_external(url);
    }
}

impl DrawingApp {
    /// Each frame: every integration that is on.
    pub(crate) fn integrations_tick(&mut self, ctx: &Context) {
        for action in std::mem::take(self.integrations.pressed.get_mut()) {
            self.integration_press(&action);
        }
        self.github_tick(ctx);
        self.spotify_tick(ctx);
        self.soloist_setup_tick(ctx);
        self.whatsapp_tick(ctx);
    }

    /// What a press on a card of the island does: "open <address>" (only a page of the services themselves), "seen
    /// <integration>" (its card was shown) and "spotify <command>".
    fn integration_press(&mut self, action: &str) {
        match action.split_once(' ') {
            Some(("open", url)) => open_page(url),
            Some(("seen", "github")) => self.github_seen(),
            Some(("spotify", command)) => {
                if let Some(command) = spotify::Command::parse(command) {
                    self.spotify_control(command);
                }
            }
            _ => {}
        }
    }

    /// A press on an integration's pill: its page opens. False when `id` is not an integration's.
    pub(crate) fn integration_open(&mut self, id: &str) -> bool {
        let Some(name) = id.strip_prefix(ITEM_PREFIX) else { return false };
        if name == "github" {
            let notice = self.integrations.github.notice.as_ref().map(|notice| notice.url.clone()).filter(|url| !url.is_empty());
            open_page(&notice.unwrap_or_else(|| github::PULLS_PAGE.into()));
            self.github_seen();
        }
        true
    }

    /// The integrations that are on, for the island.
    pub(crate) fn integration_items(&self) -> Vec<IslandItem> {
        let mut items = Vec::new();
        if self.prefs.integrations.github && self.integrations.github.has_token() {
            items.push(self.github_item());
        }
        if self.spotify_sources() != spotify::Sources::default() {
            items.extend(self.spotify_item());
        }
        if self.prefs.integrations.whatsapp {
            items.push(self.whatsapp_item());
        }
        items
    }

    /// GitHub's pill: the alert first, then the pull requests with their checks and the ones to review.
    fn whatsapp_item(&self) -> IslandItem {
        let whatsapp = &self.integrations.whatsapp;
        let (text, color, failed) = match whatsapp.link {
            whatsapp::Link::Unknown => ("Gateway ainda não testado", DIM, false),
            whatsapp::Link::Connected => ("Gateway conectado", GREEN, false),
            whatsapp::Link::NeedsPairing => ("WhatsApp precisa ser pareado no gateway", AMBER, false),
            whatsapp::Link::Unavailable => ("Gateway indisponível ou resposta inválida", RED, true),
        };
        IslandItem {
            id: format!("{ITEM_PREFIX}whatsapp"),
            name: "WhatsApp",
            kind: "GOWA local".into(),
            color: WHATSAPP,
            badge: failed.then_some(Badge::Error),
            failed,
            rows: vec![ItemRow { text: text.into(), color, action: None }],
            chips: Vec::new(),
            cover: None,
        }
    }

    fn github_item(&self) -> IslandItem {
        let github = &self.integrations.github;
        let mut rows = Vec::new();
        if let Some(notice) = &github.notice {
            let action = (!notice.url.is_empty()).then(|| format!("open {}", notice.url));
            rows.push(ItemRow { text: notice.text.clone(), color: if notice.failure { RED } else { GREEN }, action });
        }
        match (&github.pulse, &github.error) {
            (_, Some(error)) => rows.push(ItemRow { text: error.clone(), color: RED, action: None }),
            (None, None) => rows.push(ItemRow { text: "Buscando os seus pull requests…".into(), color: DIM, action: None }),
            (Some(pulse), None) => {
                if pulse.mine.is_empty() && pulse.to_review.is_empty() {
                    rows.push(ItemRow { text: "Nenhum PR aberto nem revisão pedida.".into(), color: DIM, action: None });
                }
                for request in &pulse.mine {
                    let color = match request.ci {
                        github::CiState::Failure => RED,
                        github::CiState::Success => GREEN,
                        github::CiState::Pending => AMBER,
                        github::CiState::Unknown => INK,
                    };
                    rows.push(ItemRow {
                        text: format!("{} {} {}", request.ci.label().0, request.id, request.title),
                        color,
                        action: Some(format!("open {}", request.page())),
                    });
                }
                // a default branch whose checks failed: its Actions, where the run is
                for branch in pulse.main.iter().filter(|branch| branch.ci == github::CiState::Failure) {
                    let text = format!("✕ {} · CI da {} falhou", branch.repo, branch.branch);
                    rows.push(ItemRow { text, color: RED, action: Some(format!("open {}/actions", branch.url)) });
                }
                for request in &pulse.to_review {
                    rows.push(ItemRow { text: format!("Revisar {} {}", request.id, request.title), color: INK, action: Some(format!("open {}", request.url)) });
                }
            }
        }
        let login = github.pulse.as_ref().map(|pulse| format!("@{}", pulse.login)).unwrap_or_default();
        let badge = match &github.notice {
            Some(notice) if notice.failure => Some(Badge::Error),
            Some(_) => Some(Badge::Finished),
            None => None,
        };
        IslandItem {
            id: format!("{ITEM_PREFIX}github"),
            name: "GitHub",
            kind: login,
            color: GITHUB,
            badge,
            failed: github.notice.as_ref().is_some_and(|notice| notice.failure),
            rows,
            chips: Vec::new(),
            cover: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "ghp_0123456789abcdefghijABCDEFGHIJ012345";

    fn recorded_pulse() -> github::Pulse {
        github::Pulse::parse(&serde_json::from_str(include_str!("fixtures/github_pulse.json")).unwrap()).unwrap()
    }

    #[test]
    fn only_the_services_own_pages_open() {
        for good in ["https://github.com/ana-dev/loja/pull/42", github::TOKENS_PAGE, github::PULLS_PAGE, spotify::setup::DASHBOARD] {
            assert!(page_allowed(good), "{good}");
        }
        for bad in [
            "file:///C:/Windows/System32/calc.exe",
            "http://github.com/a",
            "https://github.com.evil.example/a",
            "https://github.com/a b",
            "calc.exe",
            "",
            "https://developer.spotify.com/dashboard/soloist.evil.example",
        ] {
            assert!(!page_allowed(bad), "{bad}");
        }
    }

    #[test]
    fn github_is_on_the_island_when_on_and_with_a_token() {
        let mut app = DrawingApp::default();
        app.integrations.github = github::GitHub::with_token(TOKEN);
        assert!(app.integration_items().is_empty(), "off: nothing on the island");
        app.prefs.integrations.github = true;
        let waiting = app.integration_items();
        assert_eq!((waiting.len(), waiting[0].rows[0].text.as_str()), (1, "Buscando os seus pull requests…"));
        app.integrations.github.pulse = Some(recorded_pulse());
        let item = app.integration_items().remove(0);
        assert_eq!((item.id.as_str(), item.name, item.kind.as_str()), ("integration:github", "GitHub", "@ana-dev"));
        let rows: Vec<&str> = item.rows.iter().map(|row| row.text.as_str()).collect();
        assert_eq!(rows[..3], ["✓ ana-dev/loja#42 Adiciona o checkout", "• ana-dev/loja#43 Rascunho do frete", "✕ ana-dev/site#7 Corrige o carrinho quebrado"]);
        assert!(rows[3].starts_with("Revisar time/api#310 "), "{rows:?}");
        assert_eq!(item.rows[0].action.as_deref(), Some("open https://github.com/ana-dev/loja/pull/42"));
        assert!(item.badge.is_none() && !item.failed, "no alert, no badge");
        app.integrations.github = github::GitHub::with_token("");
        assert!(app.integration_items().is_empty(), "no token: nothing on the island");
    }

    #[test]
    fn a_press_on_the_pill_is_the_integrations_and_no_one_elses() {
        let mut app = DrawingApp::default();
        assert!(app.integration_open("integration:github"));
        assert!(!app.integration_open("terminal-1") && !app.integration_open("outside:abc"));
        // what a card wrote down is done by the next tick, whatever it is
        app.integrations.pressed.borrow_mut().extend(["seen github".to_string(), "open file:///etc/passwd".to_string(), "apagar tudo".to_string()]);
        app.integrations_tick(&Context::default());
        assert!(app.integrations.pressed.borrow().is_empty());
    }
}
