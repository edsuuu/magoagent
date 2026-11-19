//! Atualizações: a última release do GitHub contra a versão deste programa, as notas dela
//! (changelog), o instalador baixado e conferido (download.rs) e o fluxo salvar → parar → atualizar →
//! reiniciar (updater.rs). A rede vai por curl numa thread; a interface só lê o resultado (`poll_updates`).

use crate::app::ui::changelog::Changelog;
use crate::prelude::*;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};

const LATEST: &str = "https://api.github.com/repos/edsuuu/magoagent/releases/latest";
pub(crate) const RELEASES_PAGE: &str = "https://github.com/edsuuu/magoagent/releases";
const HOUR_MS: i64 = 3_600_000;
/// With "Sempre" (`Prefs.update_check` 0) the app looks again every hour it stays open; a check that failed (no
/// network) is tried again sooner.
pub(crate) const CHECK_EVERY_MS: i64 = HOUR_MS;
pub(crate) const RETRY_MS: i64 = 30 * 60_000;
/// "Atualizar depois" lasts this long, or until the computer restarts.
pub(crate) const LATER_MS: i64 = 6 * HOUR_MS;
/// What the person is told before an update that closes and reopens the app.
pub(crate) const CLOSES_AND_REOPENS: &str =
    "O MagoAgent salva o seu trabalho, encerra os terminais e agentes abertos, fecha e reabre já na versão nova, com tudo no lugar.";

/// A release as the app uses it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Release {
    pub(crate) version: String,
    pub(crate) notes: String,
    /// The release's address on GitHub.
    pub(crate) page: String,
    /// The installer for this system (`installer_name`), when the release carries it.
    pub(crate) installer: Option<String>,
    /// GitHub's `digest` of the installer (`sha256:<hex>`), which is checked before it runs.
    pub(crate) digest: Option<String>,
    /// The installer's size in bytes, said on the card before it downloads.
    pub(crate) size: Option<u64>,
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
    /// The work was saved and stopped: the updater has the turn and the app closes.
    Started(Release),
}

enum Msg {
    Checked(Result<Option<Release>, String>),
    Downloaded(Result<PathBuf, String>, Release),
    /// The .deb went through `pkexec apt-get install` (deb_update.rs).
    Installed(Result<(), String>, Release),
}

#[derive(Default)]
pub(crate) struct Updates {
    pub(crate) state: UpdateState,
    receiver: Option<Receiver<Msg>>,
    /// The check made when the app opened was already tried.
    started: bool,
    /// The check running was the app's own (it warns instead of waiting to be looked at).
    automatic: bool,
    /// "Atualizar" was clicked once: the warning that the app closes and reopens is on screen.
    pub(crate) confirming: bool,
    /// When the next check by itself is due (ms since 1970), with "Sempre".
    pub(crate) next_check: Option<i64>,
    /// A newer version found by the app's own check, its card waiting for a quiet moment (`changelog_ui`): no agent
    /// working, the window in front, nothing being typed.
    pub(crate) offer: Option<Release>,
    /// The version whose card was said to wait for the agents (once per version).
    pub(crate) held_told: Option<String>,
    /// When a key was last pressed while a card waited (egui's seconds): it waits 2 s more.
    pub(crate) typed_at: Option<f64>,
}

/// The release in GitHub's `releases/latest` answer; `None` when the repository has none.
pub(crate) fn parse_release(json: &str) -> Result<Option<Release>, String> {
    parse_release_in(json, false)
}

/// The same; `local` (a test feed, see `local_feed`) also takes an installer that is a file on this computer.
pub(crate) fn parse_release_in(json: &str, local: bool) -> Result<Option<Release>, String> {
    parse_release_for(json, local, cfg!(all(target_os = "linux", target_arch = "x86_64")))
}

/// The same, the installer being the .deb (`deb`) or the MSI.
fn parse_release_for(json: &str, local: bool, deb: bool) -> Result<Option<Release>, String> {
    let parsed: Value = serde_json::from_str(json).map_err(|_| "a resposta do GitHub não foi entendida".to_string())?;

    let Some(tag) = parsed["tag_name"].as_str() else {
        return match parsed["message"].as_str() {
            Some("Not Found") => Ok(None),
            Some(other) => Err(format!("o GitHub respondeu: {other}")),
            None => Err("a resposta do GitHub não tem versão".into()),
        };
    };

    let version = tag.trim().trim_start_matches('v').to_string();
    let wanted = installer_name_on(&version, deb);
    let asset = parsed["assets"].as_array().into_iter().flatten().find(|asset| asset["name"] == wanted.as_str());

    let installer = asset
        .and_then(|asset| asset["browser_download_url"].as_str())
        // msiexec asks for administrator: only this repository's releases are installed
        .filter(|url| url.starts_with("https://github.com/edsuuu/magoagent/releases/download/") && super::download::allowed_url(url) || local)
        .map(String::from);

    Ok(Some(Release {
        notes: parsed["body"].as_str().unwrap_or_default().replace('\r', "").trim().to_string(),
        page: parsed["html_url"].as_str().unwrap_or(RELEASES_PAGE).to_string(),
        digest: asset.and_then(|asset| asset["digest"].as_str()).map(String::from),
        size: asset.and_then(|asset| asset["size"].as_u64()),
        installer,
        version,
    }))
}

/// What tells a person updating a MagoAgent installed by the system's packages (the .deb, in /usr/bin) to do.
pub(crate) const APT_UPDATE: &str = "instalado pelo apt: atualize com sudo apt update && sudo apt upgrade magoagent";

/// Whether the program at `exe` came from the system's packages (the .deb puts it in /usr/bin, on Linux): apt updates it.
pub(crate) fn packaged(exe: &Path, linux: bool) -> bool {
    linux && exe.starts_with("/usr/")
}

fn packaged_here() -> bool {
    std::env::current_exe().is_ok_and(|exe| packaged(&exe, cfg!(target_os = "linux")))
}

/// Whether `release` installs itself from the app (it closes and reopens): the MSI on Windows; on Linux the .deb of an
/// app the .deb installed, through the system's password window (deb_update.rs).
pub(crate) fn installs_in_place(release: &Release) -> bool {
    release.installer.is_some() && (cfg!(windows) || packaged_here() && super::deb_update::pkexec_here())
}

/// The release's installer for this system: the MSI (installer/build-msi.sh), and on a Linux PC the .deb
/// (installer/build-deb.sh).
pub(crate) fn installer_name(version: &str) -> String {
    installer_name_on(version, cfg!(all(target_os = "linux", target_arch = "x86_64")))
}

fn installer_name_on(version: &str, deb: bool) -> String {
    if deb { format!("magoagent_{version}_amd64.deb") } else { format!("MagoAgent-{version}.msi") }
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

/// Whether "Atualizar depois", pressed at `later`, still holds at `now`: for `LATER_MS`, and not past a restart of the
/// computer (`boot`, when it is known).
pub(crate) fn later_holds(later: Option<i64>, now: i64, boot: Option<i64>) -> bool {
    later.is_some_and(|at| now - at < LATER_MS && boot.is_none_or(|boot| boot < at))
}

/// When this computer started (ms since 1970), from how long it has been up; None where that is not known. By the real
/// clock: the check's `now` may be a time asked for (the tests'), and the uptime is always today's.
pub(crate) fn boot_ms() -> Option<i64> {
    let now = now_ms();

    #[cfg(windows)]
    {
        // SAFETY: a plain call with no arguments
        let up = unsafe { windows::Win32::System::SystemInformation::GetTickCount64() };

        Some(now - up as i64)
    }

    #[cfg(target_os = "linux")]
    {
        let uptime = fs::read_to_string("/proc/uptime").ok()?;
        let seconds: f64 = uptime.split_whitespace().next()?.parse().ok()?;

        Some(now - (seconds * 1000.0) as i64)
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = now;

        None
    }
}

/// On Windows, asks to be opened again when Windows Update restarts the computer with the app open: the app comes back
/// with the work it saved, and its first check finds the version waiting ("Atualizar depois" ends with the restart).
pub(crate) fn reopen_after_windows_restarts() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Recovery::{REGISTER_APPLICATION_RESTART_FLAGS, RegisterApplicationRestart};
        // not after a crash or a hang (1 | 2): only a restart for updates
        // SAFETY: a constant empty command line
        let _ = unsafe { RegisterApplicationRestart(windows::core::w!(""), REGISTER_APPLICATION_RESTART_FLAGS(1 | 2)) };
    }
}

/// Whether the changelog's card may open by itself: never under a roteiro (MAGOAGENT_SCRIPT), unless a test scene asks
/// for it with MAGOAGENT_CHANGELOG (to see it in a `shot`).
fn changelog_allowed(scripted: bool) -> bool {
    !scripted || std::env::var_os("MAGOAGENT_SCENE").is_some() && std::env::var_os("MAGOAGENT_CHANGELOG").is_some()
}

/// A test feed: the answer of `releases/latest` read from this file instead of GitHub, with the installer a
/// file on this computer. Only when `MAGOAGENT_SCENE` is also set (a test instance with a scene of its own).
fn local_feed() -> Option<PathBuf> {
    let set = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());

    set("MAGOAGENT_SCENE").and(set("MAGOAGENT_UPDATE_FEED")).map(PathBuf::from)
}

fn fetch_latest() -> Result<Option<Release>, String> {
    if let Some(feed) = local_feed() {
        return parse_release_in(&fs::read_to_string(&feed).map_err(|error| format!("{}: {error}", feed.display()))?, true);
    }

    let out = crate::editor::command("curl")
        .args(["-sS", "-L", "--max-time", "15", "--connect-timeout", "10", "-H", "Accept: application/vnd.github+json", "-H", "User-Agent: MagoAgent", LATEST])
        .output()
        .map_err(|error| format!("curl: {error}"))?;

    if !out.status.success() {
        return Err("sem conexão com o GitHub; confira a internet".into());
    }

    parse_release(&String::from_utf8_lossy(&out.stdout))
}

/// The installer into the work folder: downloaded to `.part`, checked against GitHub's sha256, then renamed.
fn download_installer(release: &Release) -> Result<PathBuf, String> {
    let url = release.installer.as_deref().ok_or("essa release não traz o instalador")?;
    let directory = super::updater::work_dir();

    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let file = directory.join(installer_name(&release.version));
    let part = file.with_extension("part");

    let downloaded = if local_feed().is_some() && !url.starts_with("https://") {
        fs::copy(url, &part).map(|_| ()).map_err(|error| format!("{url}: {error}"))
    } else {
        super::download::fetch(url, &part)
    };

    let checked = downloaded.and_then(|()| super::download::verify_file(&part, release.digest.as_deref()));

    if let Err(why) = checked {
        let _ = fs::remove_file(&part);

        return Err(why);
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

    /// Downloads and checks the installer of the release found; the app then updates itself (Windows; elsewhere the release's page).
    pub(crate) fn update_install(&mut self) -> Result<(), String> {
        let UpdateState::Available(release) = self.updates.state.clone() else { return Err("não há atualização à espera; verifique primeiro".into()) };
        let deb = packaged_here();

        if deb && !installs_in_place(&release) {
            return Err(APT_UPDATE.into());
        }

        if !(cfg!(windows) || deb) || release.installer.is_none() {
            crate::app::browser::open_external(&release.page);

            return Ok(());
        }

        self.updates.confirming = false;
        // saved with the work before the app closes: the new version shows them once (`show_whats_new`)
        self.prefs.whats_new = Some((release.version.clone(), release.notes.clone()));
        let (sender, receiver) = channel();

        self.updates.receiver = Some(receiver);
        self.updates.state = UpdateState::Downloading(release.clone());
        std::thread::spawn(move || _ = sender.send(Msg::Downloaded(download_installer(&release), release)));
        Ok(())
    }

    /// Once per frame: the checks by themselves (on opening; with "Sempre", every hour) and whatever the thread answered.
    pub(crate) fn poll_updates(&mut self) {
        // never the network in a test
        if !cfg!(test) {
            self.schedule_checks(now_ms());
        }

        self.poll_updates_at(now_ms());
    }

    /// The check on opening (unless "Nunca") and, with "Sempre", the next one when it is due.
    pub(crate) fn schedule_checks(&mut self, now: i64) {
        if !self.updates.started {
            self.updates.started = true;

            if self.prefs.update_check < 2 {
                self.update_check(true);
            }
        }

        let due = self.updates.next_check.is_some_and(|at| now >= at);

        if due && self.prefs.update_check == 0 && self.updates.receiver.is_none() && !matches!(self.updates.state, UpdateState::Started(_)) {
            self.update_check(true);
        }
    }

    /// What the check's thread answered, at `now`. A newer version found by the app's own check waits for a quiet
    /// moment to open the changelog's card (ui/changelog.rs), unless it was skipped or put off ("Atualizar depois").
    pub(crate) fn poll_updates_at(&mut self, now: i64) {
        let Some(message) = self.updates.receiver.as_ref().and_then(|receiver| receiver.try_recv().ok()) else { return };

        self.updates.receiver = None;

        if let Msg::Checked(answer) = &message {
            self.updates.next_check = Some(now + if answer.is_err() { RETRY_MS } else { CHECK_EVERY_MS });
        }

        match message {
            Msg::Checked(Ok(Some(release))) => {
                if is_newer(&release.version, env!("CARGO_PKG_VERSION")) {
                    let wanted = release.version != self.prefs.update_skipped && !later_holds(self.prefs.update_later, now, boot_ms());

                    if self.updates.automatic && wanted && self.changelog.is_none() {
                        if changelog_allowed(self.script.is_some()) {
                            self.updates.offer = Some(release.clone());
                        } else {
                            self.toast(format!("Versão {} do MagoAgent disponível: veja em Ajustes › Sobre.", release.version));
                        }
                    }

                    self.updates.state = UpdateState::Available(release);
                } else {
                    self.updates.state = UpdateState::UpToDate(release);
                }
            }
            Msg::Checked(Ok(None)) => self.updates.state = UpdateState::NoRelease,
            // an automatic check that failed stays quiet; Sobre shows it when looked at
            Msg::Checked(Err(why)) => self.updates.state = UpdateState::Failed(why),
            // the .deb: installed with the person's password while the app stays open; then it reopens
            Msg::Downloaded(Ok(installer), release) if packaged_here() => {
                self.toast(format!("Confirme com a sua senha na janela do sistema para instalar a versão {}.", release.version));
                let (sender, receiver) = channel();

                self.updates.receiver = Some(receiver);
                self.updates.state = UpdateState::Started(release.clone());
                std::thread::spawn(move || _ = sender.send(Msg::Installed(super::deb_update::install(&installer), release)));
            }
            Msg::Installed(Ok(()), release) => {
                if let Err(why) = self.reopen_updated(&release) {
                    self.updates.state = UpdateState::Failed(why);
                }
            }
            Msg::Installed(Err(why), _) => self.updates.state = UpdateState::Failed(why),
            // downloaded and checked: save, stop, hand over to the updater, close (update_flow.rs)
            Msg::Downloaded(Ok(installer), release) => match self.begin_update(&installer, &release) {
                Ok(()) => {
                    self.toast("Trabalho salvo e agentes parados: o MagoAgent fecha, atualiza e reabre sozinho.");
                    self.updates.state = UpdateState::Started(release);
                }
                Err(why) => self.updates.state = UpdateState::Failed(why),
            },
            Msg::Downloaded(Err(why), _) => self.updates.state = UpdateState::Failed(why),
        }
    }

    /// On opening: the notes kept when the app went to install `current` ("O que há de novo"), once. Notes of a later
    /// version wait for it; those of an earlier one go.
    pub(crate) fn show_whats_new(&mut self, current: &str) {
        let Some((version, notes)) = self.prefs.whats_new.clone() else { return };

        if is_newer(&version, current) {
            return;
        }

        self.prefs.whats_new = None;

        if version == current && changelog_allowed(self.script.is_some()) {
            self.changelog = Some(Changelog { version, notes, installed: true });
        }
    }

    /// "Atualizar depois" (`skip` false: for `LATER_MS`, or until the computer restarts) or "Pular esta versão": the card
    /// closes, and the version waits in Ajustes › Sobre.
    pub(crate) fn update_put_off(&mut self, skip: bool) -> Result<String, String> {
        let version = match &self.updates.state {
            UpdateState::Available(release) => release.version.clone(),
            _ => return Err("não há versão nova à espera".into()),
        };

        if skip {
            self.prefs.update_skipped = version.clone();
        } else {
            self.prefs.update_later = Some(now_ms());
        }

        self.changelog = None;
        self.updates.offer = None;
        self.updates.confirming = false;

        Ok(if skip {
            format!("A versão {version} foi pulada: o aviso só volta numa versão mais nova (ela continua em Ajustes › Sobre).")
        } else {
            format!("O aviso da versão {version} volta em 6 horas, ou quando o computador reiniciar.")
        })
    }

    /// The `update` action of the MCP: asks for a check (or the install), or puts the new version off (`later`) or skips
    /// it (`skip`), and says where it stands. On Windows the install closes and reopens the app: without `confirmed` it
    /// only says so.
    pub(crate) fn act_update(&mut self, install: bool, confirmed: bool) -> Result<String, String> {
        if install {
            let asks_to_close = matches!(&self.updates.state, UpdateState::Available(release) if installs_in_place(release));

            if asks_to_close && !confirmed {
                return Ok(format!(
                    "{CLOSES_AND_REOPENS} Nada foi feito ainda: avise a pessoa e, com o aval dela, chame `update` de novo com install e confirmed true."
                ));
            }

            self.update_install()?;

            return Ok("Baixando e conferindo o instalador; depois o app salva o trabalho, para os agentes, fecha, atualiza e reabre sozinho.".into());
        }

        let said = self.updates.state_line();

        self.update_check(false);
        Ok(format!("{said} Uma nova verificação foi pedida; pergunte de novo em instantes."))
    }
}

impl Updates {
    /// What the check's thread would answer, without the network; `automatic`: the day's own check.
    #[cfg(test)]
    pub(crate) fn answer_for_test(&mut self, answer: Result<Option<Release>, String>, automatic: bool) {
        let (sender, receiver) = channel();

        sender.send(Msg::Checked(answer)).unwrap();
        (self.receiver, self.automatic, self.started) = (Some(receiver), automatic, true);
    }

    /// The state as a sentence.
    pub(crate) fn state_line(&self) -> String {
        let now = env!("CARGO_PKG_VERSION");

        match &self.state {
            UpdateState::Idle => format!("Versão {now}, ainda sem verificação."),
            UpdateState::Checking => "Verificando…".into(),
            UpdateState::UpToDate(release) => format!("Versão {now}: é a última ({}).", release.version),
            UpdateState::Available(release) if packaged_here() && !installs_in_place(release) => {
                format!("Versão {now}; a {} está disponível ({APT_UPDATE}).", release.version)
            }
            UpdateState::Available(release) => format!("Versão {now}; a {} está disponível.", release.version),
            UpdateState::NoRelease => "Nenhuma release publicada ainda.".into(),
            UpdateState::Failed(why) => format!("Não foi possível verificar: {why}."),
            UpdateState::Downloading(release) => format!("Baixando a versão {}…", release.version),
            UpdateState::Started(release) => format!("Atualizando para a versão {}: o app fecha e reabre sozinho.", release.version),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The answer read as Windows reads it (the MSI).
    fn parse_msi(json: &str) -> Result<Option<Release>, String> {
        parse_release_for(json, false, false)
    }

    const ANSWER: &str = r###"{"tag_name":"v0.1.0","html_url":"https://github.com/edsuuu/magoagent/releases/tag/v0.1.0","body":"## Novo\r\n- **Ajustes** maior","assets":[{"name":"outro.zip","browser_download_url":"https://x/outro.zip"},{"name":"MagoAgent-0.1.0.msi","browser_download_url":"https://github.com/edsuuu/magoagent/releases/download/v0.1.0/MagoAgent-0.1.0.msi","digest":"sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}]}"###;

    #[test]
    fn the_release_is_read_from_githubs_answer() {
        let release = parse_msi(ANSWER).unwrap().unwrap();

        assert_eq!(release.version, "0.1.0");
        assert_eq!(release.installer.as_deref(), Some("https://github.com/edsuuu/magoagent/releases/download/v0.1.0/MagoAgent-0.1.0.msi"));
        assert_eq!(release.digest.as_deref(), Some("sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));

        // an installer from anywhere else is no installer, and a test feed's file is one only for a test feed
        assert_eq!(
            parse_release_for(
                &ANSWER.replace("https://github.com/edsuuu/magoagent/releases/download/v0.1.0/MagoAgent-0.1.0.msi", "C:/t/MagoAgent-0.1.0.msi"),
                false,
                false
            )
            .unwrap()
            .unwrap()
            .installer,
            None
        );

        let feed = ANSWER.replace("https://github.com/edsuuu/magoagent/releases/download/v0.1.0/MagoAgent-0.1.0.msi", "C:/t/MagoAgent-0.1.0.msi");

        assert_eq!(parse_release_for(&feed, true, false).unwrap().unwrap().installer.as_deref(), Some("C:/t/MagoAgent-0.1.0.msi"));
        let foreign = ANSWER.replace("github.com/edsuuu/magoagent", "evil.com/x");

        assert_eq!(parse_msi(&foreign).unwrap().unwrap().installer, None);
        assert_eq!(plain_notes(&release.notes), "Novo\n- Ajustes maior");
        // a release without the installer still reads
        let bare = parse_msi(r#"{"tag_name":"0.2.0","assets":[]}"#).unwrap().unwrap();

        assert_eq!((bare.version.as_str(), bare.installer, bare.digest), ("0.2.0", None, None));
        assert_eq!(parse_msi(r#"{"message":"Not Found"}"#), Ok(None));
        assert!(parse_msi(r#"{"message":"rate limit"}"#).unwrap_err().contains("rate limit"));
        assert!(parse_msi("<html>").is_err());
    }

    #[test]
    fn on_a_linux_pc_the_release_s_deb_is_the_installer() {
        let deb = "https://github.com/edsuuu/magoagent/releases/download/v0.1.0/magoagent_0.1.0_amd64.deb";

        let answer = ANSWER.replace(
            r#"{"name":"outro.zip""#,
            &format!(r#"{{"name":"magoagent_0.1.0_amd64.deb","browser_download_url":"{deb}","digest":"sha256:00"}},{{"name":"outro.zip""#),
        );

        let release = parse_release_for(&answer, false, true).unwrap().unwrap();

        assert_eq!((release.installer.as_deref(), release.digest.as_deref()), (Some(deb), Some("sha256:00")));
        assert!(parse_msi(&answer).unwrap().unwrap().installer.unwrap().ends_with(".msi"), "Windows still takes the MSI");
        assert_eq!(parse_release_for(ANSWER, false, true).unwrap().unwrap().installer, None, "a release with no .deb: none");
    }

    #[test]
    fn a_linux_install_from_the_deb_is_updated_by_apt() {
        assert!(packaged(Path::new("/usr/bin/magoagent"), true));
        assert!(!packaged(Path::new("/home/ana/magoagent/target/release/magoagent"), true), "a build of the repo updates itself");
        assert!(!packaged(Path::new("/usr/bin/magoagent"), false), "only on Linux");
        assert!(APT_UPDATE.contains("sudo apt upgrade magoagent"));
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
        let newer = Release { version: "99.0.0".into(), notes: String::new(), page: String::new(), installer: None, digest: None, size: None };

        sender.send(Msg::Checked(Ok(Some(newer.clone())))).unwrap();
        app.poll_updates();
        assert_eq!(app.updates.state, UpdateState::Available(newer));
        assert!(app.updates.next_check.is_some(), "the next check is set");
        let (sender, receiver) = channel();

        app.updates.receiver = Some(receiver);
        sender.send(Msg::Checked(Ok(None))).unwrap();
        app.poll_updates();
        assert_eq!(app.updates.state, UpdateState::NoRelease);
    }

    #[test]
    fn installing_on_windows_asks_for_the_go_ahead_first() {
        let mut app = DrawingApp::default();

        let release = Release {
            version: "99.0.0".into(),
            notes: String::new(),
            page: String::new(),
            installer: Some("https://github.com/x".into()),
            digest: None,
            size: None,
        };

        app.updates.state = UpdateState::Available(release);

        if cfg!(windows) {
            let said = app.act_update(true, false).unwrap();

            assert!(said.contains("fecha e reabre") && said.contains("Nada foi feito"), "{said}");
            assert!(matches!(app.updates.state, UpdateState::Available(_)), "nothing started");
        }

        assert!(DrawingApp::default().act_update(true, true).is_err(), "no update waiting");
    }
}
