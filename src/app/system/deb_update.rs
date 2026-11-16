//! A atualização no Linux instalado pelo .deb (em /usr): o .deb da release, baixado e conferido (download.rs), vai pelo
//! `pkexec apt-get install`, que pede a senha da pessoa numa janela do sistema; instalado, o app salva o trabalho, para
//! os agentes, fecha e reabre na versão nova. Sem o pkexec (ou com a senha negada) nada muda e vale o apt.

use super::updates::Release;
use crate::prelude::*;
use std::path::Path;

/// Where the system keeps `pkexec` (polkit), the graphical sudo of the desktops.
const PKEXEC: &str = "/usr/bin/pkexec";

/// Whether this system can install a .deb with the person's password in a window of its own.
pub(crate) fn pkexec_here() -> bool {
    cfg!(target_os = "linux") && Path::new(PKEXEC).is_file()
}

/// The answer of `pkexec apt-get install`: its exit code and what apt wrote on stderr.
pub(crate) fn install_answer(code: Option<i32>, stderr: &str) -> Result<(), String> {
    match code {
        Some(0) => Ok(()),
        // pkexec: the window was closed (126) or the password not accepted (127)
        Some(126 | 127) => Err("a senha não foi confirmada; nada mudou".into()),
        _ => {
            let said = stderr.lines().map(str::trim).rfind(|line| !line.is_empty() && !line.starts_with("W:") && !line.starts_with("N:"));
            Err(format!("o apt não instalou o pacote ({})", said.unwrap_or("sem detalhes")))
        }
    }
}

/// Installs `deb` (an absolute path: apt takes it as a file, not a package's name) and waits for it.
pub(crate) fn install(deb: &Path) -> Result<(), String> {
    let output = crate::editor::command(PKEXEC)
        .args(["apt-get", "install", "-y", "--allow-downgrades"])
        .arg(deb)
        .output()
        .map_err(|error| format!("o pkexec não abriu: {error}"))?;
    install_answer(output.status.code(), &String::from_utf8_lossy(&output.stderr))
}

/// The program as it was before dpkg put the new one in its place (Linux then calls it "… (deleted)").
fn program() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let text = exe.to_string_lossy();
    Some(PathBuf::from(text.strip_suffix(" (deleted)").unwrap_or(&text)))
}

/// The shell line that waits for the app with `pid` to close and then opens `program`: `sh -c LINE name pid program`.
pub(crate) const REOPEN: &str = r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done; exec "$2""#;

impl DrawingApp {
    /// The new version is installed: save, stop, leave the answer for the app that opens, reopen and close.
    pub(crate) fn reopen_updated(&mut self, release: &Release) -> Result<(), String> {
        let version = &release.version;
        let program = program().ok_or("não sei onde o MagoAgent está")?;
        self.save_everything().map_err(|why| format!("a versão {version} foi instalada, mas o trabalho não foi salvo ({why}); salve e reabra o MagoAgent"))?;
        self.stop_everything();
        if let Some(path) = super::updater::result_path() {
            let _ = fs::write(path, serde_json::json!({ "version": version, "ok": true }).to_string());
        }
        let mut reopen = crate::editor::command("sh");
        reopen.args(["-c", REOPEN, "magoagent-reabrir"]).arg(std::process::id().to_string()).arg(&program);
        reopen.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        // a group of its own: it outlives the app
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut reopen, 0);
        reopen.spawn().map_err(|error| format!("a versão {version} foi instalada, mas o app não pôde se reabrir ({error}); abra o MagoAgent de novo"))?;
        self.quitting = true;
        self.viewport_commands.push(egui::ViewportCommand::Close);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_pkexec_answered_is_told_plainly() {
        assert_eq!(install_answer(Some(0), ""), Ok(()));
        assert_eq!(install_answer(Some(126), ""), Err("a senha não foi confirmada; nada mudou".into()));
        assert_eq!(install_answer(Some(127), ""), Err("a senha não foi confirmada; nada mudou".into()));
        let apt = "W: Download is performed unsandboxed as root\nE: Unable to locate package x\n";
        assert_eq!(install_answer(Some(100), apt), Err("o apt não instalou o pacote (E: Unable to locate package x)".into()));
        assert_eq!(install_answer(None, ""), Err("o apt não instalou o pacote (sem detalhes)".into()));
    }

    #[cfg(unix)]
    #[test]
    fn the_reopening_waits_for_the_app_and_then_opens_the_program() {
        // a process that ends at once stands for the app; `echo` for the program
        let mut gone = std::process::Command::new("true").spawn().unwrap();
        let pid = gone.id().to_string();
        gone.wait().unwrap();
        let output = std::process::Command::new("sh").args(["-c", REOPEN, "teste", &pid, "echo"]).output().unwrap();
        assert!(output.status.success());
    }
}
