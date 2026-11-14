//! Atualizações: a última release do GitHub contra a versão deste programa, as notas dela
//! (changelog) e o instalador baixado para uma pasta temporária e aberto (ele sabe atualizar).
//! A rede vai por curl numa thread; a interface só lê o resultado (`poll_updates`).

use crate::prelude::*;
use std::sync::mpsc::{Receiver, channel};

const LATEST: &str = "https://api.github.com/repos/edsuuu/magoagent/releases/latest";
pub(crate) const RELEASES_PAGE: &str = "https://github.com/edsuuu/magoagent/releases";
const DAY_MS: i64 = 86_400_000;

/// A release as the app uses it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Release {
    pub(crate) version: String,
    pub(crate) notes: String,
    /// The release's address on GitHub.
    pub(crate) page: String,
    /// The installer (`MagoAgent-<versão>.msi`), when the release carries it.
    pub(crate) installer: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) enum UpdateState {
    #[default]
    Idle,
    Checking,
    /// The latest release is not newer than this program.
    UpToDate(Release),
    Available(Release),
    /// The repository has no release yet.
    NoRelease,
    Failed(String),
    Downloading(Release),
    /// The installer was opened.
    Started(Release),
}

enum Msg {
    Checked(Result<Option<Release>, String>),
    Downloaded(Result<PathBuf, String>, Release),
}

#[derive(Default)]
pub(crate) struct Updates {
    pub(crate) state: UpdateState,
    receiver: Option<Receiver<Msg>>,
    /// The check made when the app opened was already tried.
    started: bool,
    /// The check running was the app's own (it warns instead of waiting to be looked at).
    automatic: bool,
}

/// The release in GitHub's `releases/latest` answer; `None` when the repository has none.
pub(crate) fn parse_release(json: &str) -> Result<Option<Release>, String> {
    let parsed: Value = serde_json::from_str(json).map_err(|_| "a resposta do GitHub não foi entendida".to_string())?;
    let Some(tag) = parsed["tag_name"].as_str() else {
        return match parsed["message"].as_str() {
            Some("Not Found") => Ok(None),
            Some(other) => Err(format!("o GitHub respondeu: {other}")),
            None => Err("a resposta do GitHub não tem versão".into()),
        };
    };
    let version = tag.trim().trim_start_matches('v').to_string();
    let wanted = installer_name(&version);
    let installer = parsed["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|asset| asset["name"] == wanted.as_str())
        .and_then(|asset| asset["browser_download_url"].as_str())
        .map(String::from);
    Ok(Some(Release {
        notes: parsed["body"].as_str().unwrap_or_default().replace('\r', "").trim().to_string(),
        page: parsed["html_url"].as_str().unwrap_or(RELEASES_PAGE).to_string(),
        installer,
        version,
    }))
}

/// The release's installer: the MSI (installer/build-msi.sh).
pub(crate) fn installer_name(version: &str) -> String {
    format!("MagoAgent-{version}.msi")
}

/// Whether `remote` is a version after `local`.
pub(crate) fn is_newer(remote: &str, local: &str) -> bool {
    crate::install::version_key(remote) > crate::install::version_key(local)
}

/// The release notes as plain lines: no markdown headings or bold marks.
pub(crate) fn plain_notes(notes: &str) -> String {
    let lines: Vec<String> = notes.lines().map(|line| line.trim_start_matches('#').trim().replace("**", "")).collect();
    lines.join("\n").chars().take(4000).collect()
}

fn fetch_latest() -> Result<Option<Release>, String> {
    let out = crate::ide::command("curl")
        .args(["-sS", "-L", "--max-time", "15", "-H", "Accept: application/vnd.github+json", "-H", "User-Agent: MagoAgent", LATEST])
        .output()
        .map_err(|error| format!("curl: {error}"))?;
    if !out.status.success() {
        return Err("sem conexão com o GitHub; confira a internet".into());
    }
    parse_release(&String::from_utf8_lossy(&out.stdout))
}

/// The installer into a temporary folder.
fn download_installer(release: &Release) -> Result<PathBuf, String> {
    let url = release.installer.as_deref().ok_or("essa release não traz o instalador")?;
    let directory = std::env::temp_dir().join("magoagent-atualizacao");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let file = directory.join(installer_name(&release.version));
    let part = file.with_extension("part");
    let status = crate::ide::command("curl").args(["-L", "--fail", "-sS", "-o"]).arg(&part).arg(url).status().map_err(|error| format!("curl: {error}"))?;
    if !status.success() {
        let _ = fs::remove_file(&part);
        return Err("não foi possível baixar o instalador; confira a internet".into());
    }
    fs::rename(&part, &file).map_err(|error| error.to_string())?;
    Ok(file)
}

impl DrawingApp {
    /// Looks for a newer release in the background; `automatic` is the check made on opening.
    pub(crate) fn update_check(&mut self, automatic: bool) {
        if matches!(self.updates.state, UpdateState::Checking | UpdateState::Downloading(_)) {
            return;
        }
        let (sender, receiver) = channel();
        self.updates.receiver = Some(receiver);
        self.updates.automatic = automatic;
        self.updates.state = UpdateState::Checking;
        std::thread::spawn(move || _ = sender.send(Msg::Checked(fetch_latest())));
    }

    /// Downloads the installer of the release found and opens it (Windows; elsewhere the release's page).
    pub(crate) fn update_install(&mut self) -> Result<(), String> {
        let UpdateState::Available(release) = self.updates.state.clone() else { return Err("não há atualização à espera; verifique primeiro".into()) };
        if !cfg!(windows) || release.installer.is_none() {
            crate::app::browser::open_external(&release.page);
            return Ok(());
        }
        let (sender, receiver) = channel();
        self.updates.receiver = Some(receiver);
        self.updates.state = UpdateState::Downloading(release.clone());
        std::thread::spawn(move || _ = sender.send(Msg::Downloaded(download_installer(&release), release)));
        Ok(())
    }

    /// Once per frame: the first check of the day when the app opens, and whatever the thread answered.
    pub(crate) fn poll_updates(&mut self) {
        let today = (now_ms() / DAY_MS) as u64;
        // never the network in a test
        if !self.updates.started && !cfg!(test) {
            self.updates.started = true;
            if self.prefs.update_checked_day != today {
                self.update_check(true);
            }
        }
        let Some(message) = self.updates.receiver.as_ref().and_then(|receiver| receiver.try_recv().ok()) else { return };
        self.updates.receiver = None;
        match message {
            Msg::Checked(Ok(Some(release))) => {
                self.prefs.update_checked_day = today;
                if is_newer(&release.version, env!("CARGO_PKG_VERSION")) {
                    if self.updates.automatic {
                        self.toast(format!("Versão {} do MagoAgent disponível: veja em Ajustes › Sobre.", release.version));
                    }
                    self.updates.state = UpdateState::Available(release);
                } else {
                    self.updates.state = UpdateState::UpToDate(release);
                }
            }
            Msg::Checked(Ok(None)) => {
                self.prefs.update_checked_day = today;
                self.updates.state = UpdateState::NoRelease;
            }
            // an automatic check that failed stays quiet; Sobre shows it when looked at
            Msg::Checked(Err(why)) => self.updates.state = UpdateState::Failed(why),
            // Windows asks for the administrator and closes the app to replace it
            Msg::Downloaded(Ok(installer), release) => match crate::ide::command("msiexec").arg("/i").arg(&installer).spawn() {
                Ok(_) => {
                    self.toast("Instalador aberto: ele atualiza o MagoAgent.");
                    self.updates.state = UpdateState::Started(release);
                }
                Err(error) => self.updates.state = UpdateState::Failed(format!("não foi possível abrir o instalador: {error}")),
            },
            Msg::Downloaded(Err(why), _) => self.updates.state = UpdateState::Failed(why),
        }
    }

    /// The `update` action of the MCP: asks for a check (or the install) and says where it stands.
    pub(crate) fn act_update(&mut self, install: bool) -> Result<String, String> {
        if install {
            self.update_install()?;
            return Ok("Baixando o instalador; ele abre sozinho e atualiza o MagoAgent.".into());
        }
        let said = self.updates.state_line();
        self.update_check(false);
        Ok(format!("{said} Uma nova verificação foi pedida; pergunte de novo em instantes."))
    }
}

impl Updates {
    /// The state as a sentence.
    pub(crate) fn state_line(&self) -> String {
        let now = env!("CARGO_PKG_VERSION");
        match &self.state {
            UpdateState::Idle => format!("Versão {now}, ainda sem verificação."),
            UpdateState::Checking => "Verificando…".into(),
            UpdateState::UpToDate(release) => format!("Versão {now}: é a última ({}).", release.version),
            UpdateState::Available(release) => format!("Versão {now}; a {} está disponível.", release.version),
            UpdateState::NoRelease => "Nenhuma release publicada ainda.".into(),
            UpdateState::Failed(why) => format!("Não foi possível verificar: {why}."),
            UpdateState::Downloading(release) => format!("Baixando a versão {}…", release.version),
            UpdateState::Started(release) => format!("Instalador da versão {} aberto.", release.version),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER: &str = r###"{"tag_name":"v0.1.0","html_url":"https://github.com/edsuuu/magoagent/releases/tag/v0.1.0","body":"## Novo\r\n- **Ajustes** maior","assets":[{"name":"outro.zip","browser_download_url":"https://x/outro.zip"},{"name":"MagoAgent-0.1.0.msi","browser_download_url":"https://x/i.msi"}]}"###;

    #[test]
    fn the_release_is_read_from_githubs_answer() {
        let release = parse_release(ANSWER).unwrap().unwrap();
        assert_eq!(release.version, "0.1.0");
        assert_eq!(release.installer.as_deref(), Some("https://x/i.msi"));
        assert_eq!(plain_notes(&release.notes), "Novo\n- Ajustes maior");
        // a release without the installer still reads
        let bare = parse_release(r#"{"tag_name":"0.2.0","assets":[]}"#).unwrap().unwrap();
        assert_eq!((bare.version.as_str(), bare.installer), ("0.2.0", None));
        assert_eq!(parse_release(r#"{"message":"Not Found"}"#), Ok(None));
        assert!(parse_release(r#"{"message":"rate limit"}"#).unwrap_err().contains("rate limit"));
        assert!(parse_release("<html>").is_err());
    }

    #[test]
    fn only_a_later_version_is_an_update() {
        assert!(is_newer("0.0.13", "0.0.12") && is_newer("0.1.0", "0.0.99") && is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.0.12", "0.0.12") && !is_newer("0.0.9", "0.0.12"));
    }

    #[test]
    fn the_newer_release_is_offered_and_an_older_one_is_not() {
        let mut app = DrawingApp::default();
        let (sender, receiver) = channel();
        app.updates.receiver = Some(receiver);
        let newer = Release { version: "99.0.0".into(), notes: String::new(), page: String::new(), installer: None };
        sender.send(Msg::Checked(Ok(Some(newer.clone())))).unwrap();
        app.poll_updates();
        assert_eq!(app.updates.state, UpdateState::Available(newer));
        assert!(app.prefs.update_checked_day > 0, "checked today");
        let (sender, receiver) = channel();
        app.updates.receiver = Some(receiver);
        sender.send(Msg::Checked(Ok(None))).unwrap();
        app.poll_updates();
        assert_eq!(app.updates.state, UpdateState::NoRelease);
    }
}
