#[allow(unused_imports)]
use crate::prelude::*;

#[cfg(windows)]
fn set_run_at_login_with<T>(enabled: bool, write: T) -> Result<(), String>
where
    T: FnOnce(bool) -> Result<(), String>,
{
    write(enabled)
}

#[cfg(windows)]
fn write_run_key(enabled: bool) -> Result<(), String> {
    let mut command = crate::editor::command("reg.exe");
    if enabled {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        command.args(["add", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "MagoAgent", "/t", "REG_SZ", "/d"]);
        command.arg(format!("\"{}\"", executable.display()));
        command.arg("/f");
    } else {
        command.args(["delete", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "MagoAgent", "/f"]);
    }
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
    let output = command.output().map_err(|error| format!("Não consegui atualizar o início com o Windows: {error}"))?;
    if output.status.success() || (!enabled && output.status.code() == Some(1)) {
        Ok(())
    } else {
        Err("Não consegui atualizar o início com o Windows.".into())
    }
}

#[cfg(windows)]
pub(crate) fn set_run_at_login(enabled: bool) -> Result<(), String> {
    set_run_at_login_with(enabled, write_run_key)
}

#[cfg(not(windows))]
pub(crate) fn set_run_at_login(_enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg_attr(not(windows), allow(unused_imports))]
    use super::*;

    #[cfg(windows)]
    #[test]
    fn run_key_writer_is_swappable_for_tests() {
        let mut written = None;
        set_run_at_login_with(true, |enabled| {
            written = Some(enabled);
            Ok(())
        })
        .unwrap();
        assert_eq!(written, Some(true));
    }
}
