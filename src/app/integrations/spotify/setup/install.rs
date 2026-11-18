//! What Soloist's setup changes, each only when the person asks: the program downloaded from Spotify into
//! `~/.local/bin`, the key and the device's name in `~/.config/soloist/soloist.env` (only its owner reads it, written
//! all at once), the user's unit (written only when there is none) and systemd told to start it at each login. No sudo:
//! everything is the person's own. The key goes only into that file: never on a command line, an error or a log.

use super::detect::{Program, arch_ok, version_of};
use super::{Paths, Ran, Runner, SERVICE};
use crate::prelude::*;
use std::io::Write;
use std::path::Path;

/// The build Spotify publishes for Linux (x86_64 only).
pub(crate) const DOWNLOAD: &str = "https://soloist-builds.spotifycdn.com/soloist_release_x86_64.tar.gz";
/// The user's unit, as the person's own script writes it: the key and the name come from the env file.
pub(crate) const UNIT: &str = "[Unit]
Description=Spotify Soloist (dispositivo Spotify Connect)
Wants=pipewire.service pipewire-pulse.service
After=pipewire.service pipewire-pulse.service network-online.target

[Service]
EnvironmentFile=%h/.config/soloist/soloist.env
ExecStart=%h/.local/bin/soloist -n ${SOLOIST_DEVICE_NAME} -k ${SOLOIST_API_KEY} -D %h/.local/share/soloist -C %h/.cache/soloist -w 127.0.0.1:0
Restart=on-failure
RestartSec=10
# Código 10 = versão expirada: não adianta reiniciar, precisa atualizar.
RestartPreventExitStatus=10

[Install]
WantedBy=default.target
";

/// The developer key: it never shows, not even in a `{:?}` (it goes through a thread and a `Result`).
pub(crate) struct Secret(String);

impl Secret {
    /// The key as pasted, without the spaces and line breaks a copy brings along.
    pub(crate) fn new(pasted: &str) -> Secret {
        Secret(pasted.chars().filter(|letter| !letter.is_whitespace()).collect())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret(***)")
    }
}

/// A path's permission bits (None where there are none, on Windows).
pub(crate) fn mode_of(meta: &fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    return Some(std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o777);
    #[cfg(not(unix))]
    {
        let _ = meta;
        None
    }
}

fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    return fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(mode));
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

/// A new file only its owner reads from the first byte (600), failing when it is there.
fn create_private(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// A folder that only its owner enters (700), the ones above it made as usual.
fn private_folder(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    set_mode(path, 0o700)
}

fn named(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default()
}

/// What a command printed with a key out of it: what follows `-k` or `--api-key`, and anything shaped as one
/// (`spak_…`). Every text of Soloist and systemd shown goes through here.
pub(crate) fn mask(text: &str) -> String {
    static FLAGS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"(--api-key|-k)([= ]+)\S+").unwrap());
    static KEYS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"spak_[A-Za-z0-9_-]+").unwrap());
    KEYS.replace_all(&FLAGS.replace_all(text, "$1$2***"), "spak_***").into_owned()
}

/// `systemctl --user <args>`; an error when it fails, with the first line it printed (masked).
pub(crate) fn systemctl(runner: &dyn Runner, args: &[&str]) -> Result<Ran, String> {
    let all: Vec<&str> = ["--user"].into_iter().chain(args.iter().copied()).collect();
    let ran = runner.run(Path::new("systemctl"), &all)?;
    if ran.code == Some(0) {
        return Ok(ran);
    }
    let why = ran.output.lines().map(str::trim).find(|line| !line.is_empty()).map(mask).unwrap_or_default();
    Err(format!("systemctl --user {} falhou{}", args.join(" "), if why.is_empty() { String::new() } else { format!(": {why}") }))
}

/// Whether an address is Spotify's download, over https: its own host, or another of its CDN it redirects to.
pub(crate) fn download_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.to_ascii_lowercase();
    !authority.contains(['@', '\\', ':']) && (host == "soloist-builds.spotifycdn.com" || host.ends_with(".spotifycdn.com"))
}

/// Downloads `url` into `part` (`fetch_hops`): nothing outside Spotify's CDN is asked.
pub(crate) fn fetch(url: &str, part: &Path) -> Result<(), String> {
    use crate::app::system::download::{Hop, fetch_hops};
    fetch_hops(url, part, download_allowed, 600).map_err(|hop| match hop {
        Hop::Outside(_) => "o download foi para fora do Spotify: recusado".into(),
        Hop::Start(error) => format!("curl não pôde ser iniciado: {error}"),
        Hop::Failed => "não foi possível baixar o Soloist (sem conexão, tempo esgotado ou recusado); confira a internet".into(),
        Hop::Unexpected(_) => "o servidor do Spotify respondeu de um jeito inesperado".into(),
        Hop::TooMany => "redirecionamentos demais".into(),
    })
}

/// Puts the program at `to` all at once: a copy beside it, executable (755), renamed over the old one (a Soloist
/// running the old one keeps it until it restarts).
fn place(from: &Path, to: &Path) -> Result<(), String> {
    let folder = to.parent().ok_or("caminho do programa sem pasta")?;
    fs::create_dir_all(folder).map_err(|error| format!("não foi possível criar {}: {error}", folder.display()))?;
    let temp = folder.join(format!(".soloist.new-{}", std::process::id()));
    let result = fs::copy(from, &temp).and_then(|_| set_mode(&temp, 0o755)).and_then(|()| fs::rename(&temp, to));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|error| format!("não foi possível instalar em {}: {error}", to.display()))
}

/// Downloads Soloist (`fetch` puts Spotify's archive at the path it is given), takes the `soloist` out of it, checks
/// that it runs (`--version`) and installs it at `paths.program` (755). The version it said, or why not. `scratch`: where
/// the temporary folder goes (removed after).
pub(crate) fn install_program(
    paths: &Paths,
    runner: &dyn Runner,
    arch: &str,
    scratch: &Path,
    fetch: impl Fn(&str, &Path) -> Result<(), String>,
) -> Result<String, String> {
    arch_ok(arch)?;
    let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |since| since.as_nanos());
    let folder = scratch.join(format!("magoagent-soloist-{}-{stamp}", std::process::id()));
    private_folder(&folder).map_err(|error| format!("não foi possível criar a pasta temporária: {error}"))?;
    let result = (|| {
        let archive = folder.join("soloist.tar.gz");
        fetch(DOWNLOAD, &archive)?;
        let (Some(archive_text), Some(folder_text)) = (archive.to_str(), folder.to_str()) else {
            return Err("a pasta temporária tem um nome que o tar não lê".to_string());
        };
        // only the program comes out (no other name of the archive is written)
        let ran = runner.run(Path::new("tar"), &["-xzf", archive_text, "-C", folder_text, "soloist"])?;
        let program = folder.join("soloist");
        // a link in the archive could point anywhere: only a plain file is installed
        let plain = fs::symlink_metadata(&program).is_ok_and(|meta| meta.file_type().is_file());
        if ran.code != Some(0) || !plain {
            return Err("o arquivo baixado não tem o programa soloist".into());
        }
        set_mode(&program, 0o755).map_err(|error| format!("não foi possível preparar o programa: {error}"))?;
        let version = match version_of(&program, runner) {
            Program::Ready(version) => version,
            Program::Expired => return Err("a versão que o Spotify publica agora já expirou; tente de novo mais tarde".into()),
            Program::Broken(why) => return Err(format!("o programa baixado não rodou ({why})")),
            Program::Missing => return Err("o arquivo baixado não tem o programa soloist".into()),
        };
        place(&program, &paths.program)?;
        Ok(version)
    })();
    let _ = fs::remove_dir_all(&folder);
    result
}

/// A value as the shell and systemd's `EnvironmentFile=` both read it: in double quotes, with `\`, `"`, `$` and the
/// backquote escaped (inside double quotes both take those four escapes the same way).
pub(crate) fn shell_quote(value: &str) -> String {
    let mut quoted = String::from("\"");
    for letter in value.chars() {
        if matches!(letter, '\\' | '"' | '$' | '`') {
            quoted.push('\\');
        }
        quoted.push(letter);
    }
    quoted.push('"');
    quoted
}

/// The device's name to save: trimmed, `default` when empty; none with a line break or another control character.
pub(crate) fn device_name(name: &str, default: &str) -> Result<String, String> {
    let name = name.trim();
    if name.chars().any(char::is_control) {
        return Err("O nome do dispositivo não pode ter quebras de linha.".into());
    }
    if name.chars().count() > 64 {
        return Err("O nome do dispositivo passa de 64 letras.".into());
    }
    Ok(if name.is_empty() { default.to_string() } else { name.to_string() })
}

/// The env file's text: the two lines the unit reads.
pub(crate) fn env_text(key: &Secret, name: &str) -> String {
    format!("SOLOIST_API_KEY={}\nSOLOIST_DEVICE_NAME={}\n", shell_quote(&key.0), shell_quote(name))
}

/// Writes `bytes` to `path` all at once: a sibling only its owner reads (600 from the first byte), synced and renamed
/// over it. On an error the old file stays as it was and the sibling goes. The error names the file, never what it holds.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_file_name(format!(".{}.tmp-{}", named(path), std::process::id()));
    let _ = fs::remove_file(&temp);
    let result = create_private(&temp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        set_mode(path, 0o600)
    });
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|error| format!("não foi possível gravar {}: {}", path.display(), error.kind()))
}

/// Saves the key and the device's name in `paths.env_file` (its folder 700, the file 600, written all at once).
pub(crate) fn write_env(paths: &Paths, key: &Secret, name: &str, default_name: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("Cole a chave do Soloist antes de salvar.".into());
    }
    // a key is letters, digits and a few signs: anything else is a copy gone wrong (and is never echoed)
    if !key.0.chars().all(|letter| letter.is_ascii_graphic()) || key.0.chars().count() > 512 {
        return Err("Isso não parece uma chave do Soloist: copie de novo do painel do Spotify.".into());
    }
    let name = device_name(name, default_name)?;
    let folder = paths.env_file.parent().ok_or("caminho do soloist.env sem pasta")?;
    private_folder(folder).map_err(|error| format!("não foi possível criar {}: {}", folder.display(), error.kind()))?;
    write_private(&paths.env_file, env_text(key, &name).as_bytes())
}

/// Writes the user's unit when there is none, and never over one (the person's own, or ours from before). True when it
/// was written.
pub(crate) fn write_unit(paths: &Paths) -> Result<bool, String> {
    if fs::symlink_metadata(&paths.unit).is_ok() {
        return Ok(false);
    }
    let failed = |error: std::io::Error| format!("não foi possível gravar {}: {error}", paths.unit.display());
    let folder = paths.unit.parent().ok_or("caminho do serviço sem pasta")?;
    fs::create_dir_all(folder).map_err(failed)?;
    let mut file = match fs::OpenOptions::new().write(true).create_new(true).open(&paths.unit) {
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        other => other.map_err(failed)?,
    };
    if let Err(error) = file.write_all(UNIT.as_bytes()).and_then(|()| file.sync_all()) {
        // half a unit is worse than none
        let _ = fs::remove_file(&paths.unit);
        return Err(failed(error));
    }
    Ok(true)
}

/// Step 4: the key saved (when one was pasted; else the one saved before stays), the unit written when missing and
/// systemd told to start Soloist now and at each login. A service already running restarts to read a new key.
pub(crate) fn turn_on(paths: &Paths, runner: &dyn Runner, key: Option<&Secret>, name: &str, default_name: &str) -> Result<String, String> {
    if !paths.program.is_file() {
        return Err("Baixe o Soloist antes (passo 1).".into());
    }
    let was_running = runner.run(Path::new("systemctl"), &["--user", "is-active", SERVICE]).is_ok_and(|ran| ran.output.trim() == "active");
    let saved = match key.filter(|key| !key.is_empty()) {
        Some(key) => {
            write_env(paths, key, name, default_name)?;
            true
        }
        None if fs::symlink_metadata(&paths.env_file).is_err() => return Err("Falta a chave: cole a chave do passo 2.".into()),
        None => false,
    };
    if write_unit(paths)? {
        systemctl(runner, &["daemon-reload"])?;
    }
    systemctl(runner, &["enable", "--now", SERVICE])?;
    if saved && was_running {
        systemctl(runner, &["restart", SERVICE])?;
    }
    Ok(match saved {
        true => "Chave salva e Soloist ligado: ele abre sozinho quando você entra no computador.".into(),
        false => "Soloist ligado: ele abre sozinho quando você entra no computador.".into(),
    })
}

/// "start" or "stop" of the user's service.
pub(crate) fn service(runner: &dyn Runner, verb: &str) -> Result<String, String> {
    systemctl(runner, &[verb, SERVICE])?;
    Ok(if verb == "stop" { "Soloist parado." } else { "Soloist ligado." }.into())
}

/// The service in words (its state, since when, its last code) and what Soloist says of itself (`ctl status`): never
/// its command line, which holds the key; masked all the same.
pub(crate) fn details(paths: &Paths, runner: &dyn Runner) -> String {
    let properties = ["ActiveState", "SubState", "UnitFileState", "ActiveEnterTimestamp", "ExecMainStatus", "NRestarts"];
    let mut args = vec!["--user", "show", SERVICE, "--no-pager"];
    for property in &properties {
        args.extend(["-p", property]);
    }
    let shown = runner.run(Path::new("systemctl"), &args).map(|ran| ran.output).unwrap_or_default();
    let value = |name: &str| shown.lines().find_map(|line| line.strip_prefix(name)?.strip_prefix('=')).unwrap_or_default().trim().to_string();
    let mut lines = vec![format!("Serviço {SERVICE}: {} ({}), {}", value("ActiveState"), value("SubState"), value("UnitFileState"))];
    if !value("ActiveEnterTimestamp").is_empty() {
        lines.push(format!("Ligado desde {}", value("ActiveEnterTimestamp")));
    }
    if value("ExecMainStatus") == super::EXPIRED.to_string() {
        lines.push("Última saída: código 10, a versão expirou (atualize no passo 1)".into());
    }
    if !value("NRestarts").is_empty() && value("NRestarts") != "0" {
        lines.push(format!("Reinícios: {}", value("NRestarts")));
    }
    if let Some(data) = paths.data.to_str()
        && paths.program.is_file()
        && let Ok(ran) = runner.run(&paths.program, &["ctl", "status", "-D", data])
    {
        lines.extend(ran.output.lines().map(str::trim).filter(|line| !line.is_empty()).map(String::from));
    }
    mask(&lines.join("\n"))
}
