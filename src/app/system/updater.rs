//! A atualização no Windows, na ordem da regra do dono: salvar o trabalho, parar os processos,
//! atualizar e reiniciar. O app faz as duas primeiras (`prepare`) e passa a vez a uma cópia do próprio
//! exe fora da pasta instalada (`magoagent atualizar ...`, `cli`), que espera o app sair, encerra o que
//! sobrou da pasta instalada, roda o MSI, confere o resultado e reabre o app (sempre: na versão nova, ou
//! na que estava se o MSI falhou). O resultado vai num arquivo que o app lê ao abrir (`result_message`).

use crate::prelude::*;
use std::path::Path;
use std::time::{Duration, Instant};

// ── O lado do app: salvar e parar, nesta ordem, e só então passar a vez ────────────────────────────────

pub(crate) trait Prepare {
    /// What can fail before anything is touched (the updater's copy is made).
    fn check(&mut self) -> Result<(), String>;
    /// The work on disk and confirmed.
    fn save(&mut self) -> Result<(), String>;
    /// The terminals and agents of the app, closed.
    fn stop(&mut self);
    /// The updater starts; the app closes after this.
    fn hand_off(&mut self) -> Result<(), String>;
}

/// The stages in their order; the first to fail ends it, and what comes after never runs.
pub(crate) fn prepare(stages: &mut dyn Prepare) -> Result<(), String> {
    stages.check()?;
    stages.save()?;
    stages.stop();
    stages.hand_off()
}

// ── O lado do atualizador (outro processo) ──────────────────────────────────────────────────────────────

pub(crate) trait Steps {
    /// The app that asked for the update is gone (closed, or ended after a wait).
    fn wait_for_app(&mut self) -> Result<(), String>;
    /// Every other `magoagent.exe` of the installed folder ended; how many.
    fn stop_leftovers(&mut self) -> Result<usize, String>;
    /// The MSI ran and the installed version is the wanted one; `true` when Windows asks for a restart.
    fn install(&mut self) -> Result<bool, String>;
    /// The installed app, opened again.
    fn reopen(&mut self) -> Result<(), String>;
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Outcome {
    pub(crate) ok: bool,
    pub(crate) message: String,
    pub(crate) reboot: bool,
}

/// Wait, stop, install: the first to fail ends it (the installer never runs over a process still using the
/// files). The app is opened again either way: on the new version, or on the one that was there.
pub(crate) fn finish(steps: &mut dyn Steps) -> Outcome {
    let installed = steps.wait_for_app().and_then(|()| steps.stop_leftovers()).and_then(|_| steps.install());
    let reopened = steps.reopen();
    let (ok, mut message, reboot) = match installed {
        Ok(reboot) => (true, String::new(), reboot),
        Err(why) => (false, why, false),
    };
    if let Err(why) = reopened {
        message = format!("{message} Não foi possível reabrir o MagoAgent: {why}").trim().to_string();
    }
    Outcome { ok, message, reboot }
}

/// Whether a process of this image is one the update stops: a `magoagent.exe` running from the installed
/// folder, never the updater itself. A process whose path is unknown is not touched.
pub(crate) fn should_stop(image: Option<&Path>, install_dir: &Path, pid: u32, own_pid: u32) -> bool {
    let normal = |path: &Path| path.to_string_lossy().replace('/', "\\").trim_start_matches("\\\\?\\").trim_end_matches('\\').to_lowercase();
    let Some(image) = image else { return false };
    pid != own_pid
        && image.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("magoagent.exe"))
        && image.parent().is_some_and(|dir| normal(dir) == normal(install_dir))
}

/// What `msiexec` answered: `Ok(restart asked)` or the reason in words.
pub(crate) fn msi_result(code: i32) -> Result<bool, String> {
    match code {
        0 => Ok(false),
        // ERROR_SUCCESS_REBOOT_REQUIRED
        3010 | 1641 => Ok(true),
        1602 => Err("a instalação foi cancelada (a permissão de administrador não foi dada?)".into()),
        1618 => Err("outra instalação do Windows está em andamento; tente de novo depois".into()),
        1619 | 1620 => Err("o pacote do instalador não abriu (corrompido ou incompleto, erro 1619/1620)".into()),
        1603 => Err("o instalador falhou (erro fatal, 1603); o Windows desfez a instalação".into()),
        other => Err(format!("o instalador terminou com o código {other}")),
    }
}

/// The text the app shows when it opens after an update, from the updater's result file; `None` for a file
/// that is not ours. `now` is the version running.
pub(crate) fn result_message(result: &Value, now: &str) -> Option<String> {
    let wanted = result["version"].as_str()?;
    if result["ok"].as_bool()? {
        let reboot = if result["reboot"].as_bool() == Some(true) { " O Windows pediu para reiniciar o computador e concluir." } else { "" };
        return Some(if wanted == now {
            format!("Atualizado para a versão {now}.{reboot}")
        } else {
            format!("A instalação da versão {wanted} terminou, mas o app aberto é o da versão {now}.{reboot}")
        });
    }
    let why = result["message"].as_str().unwrap_or("motivo desconhecido");
    Some(format!("A atualização para a versão {wanted} não foi concluída: {why} O MagoAgent voltou na versão {now}."))
}

/// Where the updater leaves its answer for the app that opens next: beside the drawing.
pub(crate) fn result_path() -> Option<PathBuf> {
    autosave_path().map(|path| path.with_file_name("atualizacao.json"))
}

/// The folder the update works in: the installer, the updater's copy.
pub(crate) fn work_dir() -> PathBuf {
    std::env::temp_dir().join("magoagent-atualizacao")
}

/// The program's own copy outside the installed folder (Windows won't replace an exe that runs).
pub(crate) fn copy_self() -> Result<PathBuf, String> {
    let directory = work_dir();
    fs::create_dir_all(&directory).map_err(|error| format!("não foi possível preparar {}: {error}", directory.display()))?;
    let copy = directory.join(format!("atualizador-{}.exe", std::process::id()));
    let own = std::env::current_exe().map_err(|error| error.to_string())?;
    fs::copy(own, &copy).map_err(|error| format!("não foi possível copiar o atualizador: {error}"))?;
    Ok(copy)
}

/// Starts the updater (the copy), detached, for `installer` and the app with `pid` in `install_dir`.
pub(crate) fn launch(copy: &Path, installer: &Path, pid: u32, install_dir: &Path, version: &str) -> Result<(), String> {
    let result = result_path().ok_or("sem pasta de dados para o resultado")?;
    let mut command = crate::editor::command(copy);
    command.arg("atualizar").arg(installer).arg(pid.to_string()).arg(install_dir).arg(version).arg(result).current_dir(work_dir());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window, and its own group: a Ctrl+C aimed at the app does not reach it
        command.creation_flags(0x0800_0000 | 0x0000_0200);
    }
    command.spawn().map(|_| ()).map_err(|error| format!("não foi possível iniciar o atualizador: {error}"))
}

fn wait_until(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    done()
}

/// The path of the image a process runs; `None` when it can't be asked (another user's, protected).
#[cfg(windows)]
fn image_path(pid: u32) -> Option<PathBuf> {
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn QueryFullProcessImageNameW(process: isize, flags: u32, name: *mut u16, size: *mut u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    // SAFETY: the handle is checked and closed once; the buffer is as long as the size says
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process == 0 {
            return None;
        }
        let mut buffer = vec![0u16; 1024];
        let mut size = buffer.len() as u32;
        let read = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size) != 0;
        CloseHandle(process);
        read.then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize])))
    }
}

#[cfg(not(windows))]
fn image_path(_pid: u32) -> Option<PathBuf> {
    None
}

/// Every `magoagent.exe` of `install_dir` running now (never `own_pid`).
fn running_in(install_dir: &Path, own_pid: u32) -> Vec<u32> {
    crate::transcript::process::table()
        .into_iter()
        .filter(|(_, (_, name))| name.eq_ignore_ascii_case("magoagent.exe"))
        .filter(|(pid, _)| should_stop(image_path(*pid).as_deref(), install_dir, *pid, own_pid))
        .map(|(pid, _)| pid)
        .collect()
}

fn kill(pid: u32) {
    let _ = crate::editor::command("taskkill").args(["/F", "/PID", &pid.to_string()]).output();
}

struct Real<'a> {
    installer: &'a Path,
    app_pid: u32,
    install_dir: &'a Path,
    version: &'a str,
    log: PathBuf,
}

impl Steps for Real<'_> {
    fn wait_for_app(&mut self) -> Result<(), String> {
        let gone = |pid: u32| !crate::transcript::process::alive(pid);
        if wait_until(Duration::from_secs(60), || gone(self.app_pid)) {
            return Ok(());
        }
        // the work was saved before the update began: an app that does not close in a minute is ended
        kill(self.app_pid);
        if wait_until(Duration::from_secs(10), || gone(self.app_pid)) { Ok(()) } else { Err("o MagoAgent aberto não fechou.".into()) }
    }

    fn stop_leftovers(&mut self) -> Result<usize, String> {
        let own = std::process::id();
        let found = running_in(self.install_dir, own);
        found.iter().for_each(|pid| kill(*pid));
        if wait_until(Duration::from_secs(15), || running_in(self.install_dir, own).is_empty()) {
            Ok(found.len())
        } else {
            Err("ainda há processos do MagoAgent instalado em execução e não foi possível encerrá-los.".into())
        }
    }

    fn install(&mut self) -> Result<bool, String> {
        let mut command = crate::editor::command("msiexec");
        command.arg("/i").arg(self.installer).args(["/passive", "/norestart", "/L*v"]).arg(&self.log);
        let status = command.status().map_err(|error| format!("não foi possível abrir o instalador: {error}"))?;
        let reboot = msi_result(status.code().unwrap_or(-1))?;
        let installed = fs::read_to_string(self.install_dir.join("versao")).unwrap_or_default();
        if installed.trim() != self.version {
            return Err(format!("o instalador terminou, mas a versão instalada é a {:?} (esperava a {}).", installed.trim(), self.version));
        }
        Ok(reboot)
    }

    fn reopen(&mut self) -> Result<(), String> {
        let exe = self.install_dir.join("magoagent.exe");
        if !exe.is_file() {
            return Err(format!("{} não existe", exe.display()));
        }
        let mut command = crate::editor::command(exe);
        command.current_dir(self.install_dir);
        command.spawn().map(|_| ()).map_err(|error| error.to_string())
    }
}

/// `magoagent atualizar <msi> <pid do app> <pasta instalada> <versão> <arquivo do resultado>`: returns the exit code.
pub(crate) fn cli(args: &[String]) -> i32 {
    let [installer, pid, install_dir, version, result] = args else { return 2 };
    // never inside the folder the installer is about to replace
    let _ = std::env::set_current_dir(work_dir());
    let (installer, install_dir, result) = (PathBuf::from(installer), PathBuf::from(install_dir), PathBuf::from(result));
    let mut steps = Real {
        installer: &installer,
        app_pid: pid.parse().unwrap_or(0),
        install_dir: &install_dir,
        version,
        log: result.with_file_name("atualizacao-msi.log"),
    };
    let outcome = finish(&mut steps);
    let answer = json!({ "ok": outcome.ok, "version": version, "message": outcome.message, "reboot": outcome.reboot });
    let _ = write_atomic(&result, answer.to_string());
    if outcome.ok { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Log {
        calls: Vec<&'static str>,
        failing: Option<&'static str>,
    }

    impl Log {
        fn step(&mut self, name: &'static str) -> Result<(), String> {
            self.calls.push(name);
            if self.failing == Some(name) { Err(format!("{name} falhou")) } else { Ok(()) }
        }
    }

    impl Prepare for Log {
        fn check(&mut self) -> Result<(), String> {
            self.step("check")
        }
        fn save(&mut self) -> Result<(), String> {
            self.step("save")
        }
        fn stop(&mut self) {
            self.calls.push("stop");
        }
        fn hand_off(&mut self) -> Result<(), String> {
            self.step("hand_off")
        }
    }

    impl Steps for Log {
        fn wait_for_app(&mut self) -> Result<(), String> {
            self.step("wait")
        }
        fn stop_leftovers(&mut self) -> Result<usize, String> {
            self.step("leftovers").map(|()| 2)
        }
        fn install(&mut self) -> Result<bool, String> {
            self.step("install").map(|()| false)
        }
        fn reopen(&mut self) -> Result<(), String> {
            self.step("reopen")
        }
    }

    #[test]
    fn the_stages_run_in_the_owners_order_and_a_failure_stops_the_rest() {
        let mut all = Log::default();
        assert_eq!(prepare(&mut all), Ok(()));
        assert_eq!(all.calls, ["check", "save", "stop", "hand_off"]);
        // saving fails: nothing is stopped and the updater never starts
        let mut unsaved = Log { failing: Some("save"), ..Default::default() };
        assert_eq!(prepare(&mut unsaved), Err("save falhou".into()));
        assert_eq!(unsaved.calls, ["check", "save"]);
        let mut unchecked = Log { failing: Some("check"), ..Default::default() };
        assert!(prepare(&mut unchecked).is_err());
        assert_eq!(unchecked.calls, ["check"]);
    }

    #[test]
    fn the_updater_installs_only_over_a_stopped_app_and_always_reopens() {
        let mut good = Log::default();
        assert_eq!(finish(&mut good), Outcome { ok: true, message: String::new(), reboot: false });
        assert_eq!(good.calls, ["wait", "leftovers", "install", "reopen"]);
        // a process that can't be stopped: no installer, the app that was there opens again
        let mut stuck = Log { failing: Some("leftovers"), ..Default::default() };
        let outcome = finish(&mut stuck);
        assert!(!outcome.ok && outcome.message.contains("leftovers"));
        assert_eq!(stuck.calls, ["wait", "leftovers", "reopen"]);
        // the installer fails: reopened all the same, with the reason
        let mut failed = Log { failing: Some("install"), ..Default::default() };
        assert!(!finish(&mut failed).ok);
        assert_eq!(failed.calls, ["wait", "leftovers", "install", "reopen"]);
        // and a reopen that fails is said too
        let mut closed = Log { failing: Some("reopen"), ..Default::default() };
        let outcome = finish(&mut closed);
        assert!(outcome.ok && outcome.message.contains("reabrir"));
    }

    #[test]
    fn only_a_magoagent_from_the_installed_folder_is_stopped() {
        let installed = Path::new(r"C:\Program Files\MagoAgent");
        let stop = |image: &str, pid| should_stop(Some(Path::new(image)), installed, pid, 99);
        assert!(stop(r"C:\Program Files\MagoAgent\magoagent.exe", 1));
        assert!(stop(r"c:\program files\magoagent\MAGOAGENT.EXE", 1) && stop("C:/Program Files/MagoAgent/magoagent.exe", 1));
        assert!(stop(r"\\?\C:\Program Files\MagoAgent\magoagent.exe", 1));
        // the updater's copy, another install, another program of the folder, a sibling folder
        assert!(!stop(r"C:\Users\x\AppData\Local\Temp\magoagent-atualizacao\atualizador-1.exe", 1));
        assert!(!stop(r"C:\Users\x\AppData\Local\Temp\magoagent.exe", 1));
        assert!(!stop(r"C:\Program Files\MagoAgent\outro.exe", 1) && !stop(r"C:\Program Files\MagoAgent2\magoagent.exe", 1));
        assert!(!stop(r"C:\Program Files\MagoAgent\data\magoagent.exe", 1));
        // this very process, and a process whose path is unknown
        assert!(!stop(r"C:\Program Files\MagoAgent\magoagent.exe", 99));
        assert!(!should_stop(None, installed, 1, 99));
    }

    #[test]
    fn the_installers_answer_is_read() {
        assert_eq!(msi_result(0), Ok(false));
        assert_eq!(msi_result(3010), Ok(true));
        assert!(msi_result(1602).unwrap_err().contains("cancelada"));
        assert!(msi_result(1618).is_err() && msi_result(1603).is_err());
        assert!(msi_result(1620).unwrap_err().contains("não abriu"));
        assert!(msi_result(1).unwrap_err().contains("código 1"));
    }

    #[test]
    fn the_app_says_how_the_update_ended() {
        let said = |value: Value| result_message(&value, "0.0.17");
        assert_eq!(said(json!({ "ok": true, "version": "0.0.17" })).unwrap(), "Atualizado para a versão 0.0.17.");
        assert!(said(json!({ "ok": true, "version": "0.0.17", "reboot": true })).unwrap().contains("reiniciar"));
        assert!(said(json!({ "ok": true, "version": "0.0.18" })).unwrap().contains("o app aberto é o da versão 0.0.17"));
        let failed = said(json!({ "ok": false, "version": "0.0.18", "message": "o instalador falhou." })).unwrap();
        assert!(failed.contains("não foi concluída") && failed.contains("o instalador falhou.") && failed.contains("voltou na versão 0.0.17"));
        assert_eq!(said(json!({ "x": 1 })), None);
    }

    #[test]
    fn the_updater_needs_its_five_arguments() {
        assert_eq!(cli(&[]), 2);
    }
}
