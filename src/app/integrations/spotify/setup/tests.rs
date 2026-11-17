//! Soloist's setup on a home of its own in the temporary folder, with fake commands: the real home, systemd and
//! Spotify are never touched.

use super::detect::{arch_ok, default_name, detect, tunnels};
use super::install::{DOWNLOAD, UNIT, download_allowed, install_program, mask, shell_quote, turn_on, write_env, write_unit};
use super::*;
use std::cell::{Cell, RefCell};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const KEY: &str = "spak_SEGREDO0123456789abcdef";

/// The fake commands: each call written down ("program args"), each answered by `answer`.
struct Fake<F: Fn(&str, &[&str]) -> Result<Ran, String>> {
    answer: F,
    calls: RefCell<Vec<String>>,
}

impl<F: Fn(&str, &[&str]) -> Result<Ran, String>> Fake<F> {
    fn new(answer: F) -> Self {
        Fake { answer, calls: RefCell::new(Vec::new()) }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl<F: Fn(&str, &[&str]) -> Result<Ran, String>> Runner for Fake<F> {
    fn run(&self, program: &Path, args: &[&str]) -> Result<Ran, String> {
        let name = program.file_name().unwrap().to_string_lossy().to_string();
        self.calls.borrow_mut().push(format!("{name} {}", args.join(" ")));
        (self.answer)(&name, args)
    }
}

fn ran(code: i32, output: &str) -> Result<Ran, String> {
    Ok(Ran { code: Some(code), output: output.into() })
}

/// A home in the temporary folder, gone at the end.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Home {
        let folder = std::env::temp_dir().join(format!("magoagent-soloist-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        Home(folder)
    }

    fn paths(&self) -> Paths {
        Paths::under(&self.0)
    }

    /// A program at `~/.local/bin/soloist` (bytes only: the fakes answer for it).
    fn with_program(self) -> Self {
        let program = self.paths().program;
        fs::create_dir_all(program.parent().unwrap()).unwrap();
        fs::write(&program, "programa").unwrap();
        self
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.0.join(".config/soloist"), fs::Permissions::from_mode(0o700));
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder).unwrap().flatten().map(|entry| entry.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    names
}

#[test]
fn the_env_file_is_private_quoted_and_written_at_once() {
    let home = Home::new("env");
    let paths = home.paths();
    let name = r#"Sala $HOME "da" `tv` \ok"#;
    write_env(&paths, &Secret::new(&format!("  {KEY}\n")), name, "Soloist casa").unwrap();
    let text = fs::read_to_string(&paths.env_file).unwrap();
    assert_eq!(text, format!("SOLOIST_API_KEY=\"{KEY}\"\nSOLOIST_DEVICE_NAME=\"Sala \\$HOME \\\"da\\\" \\`tv\\` \\\\ok\"\n"), "the spaces of the copy go");
    assert_eq!((mode(paths.env_file.parent().unwrap()), mode(&paths.env_file)), (0o700, 0o600));
    // the shell reads back exactly what was typed
    let script = format!(". '{}'; printf %s \"$SOLOIST_DEVICE_NAME\"", paths.env_file.display());
    let read = std::process::Command::new("sh").args(["-c", &script]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&read.stdout), name);
    // saved again: replaced whole, still private, nothing left beside it
    write_env(&paths, &Secret::new("spak_outra"), "  ", "Soloist casa").unwrap();
    assert_eq!(
        fs::read_to_string(&paths.env_file).unwrap(),
        "SOLOIST_API_KEY=\"spak_outra\"\nSOLOIST_DEVICE_NAME=\"Soloist casa\"\n",
        "an empty name is the default"
    );
    assert_eq!(mode(&paths.env_file), 0o600);
    assert_eq!(names_in(paths.env_file.parent().unwrap()), ["soloist.env"]);
    assert_eq!(shell_quote("a'b"), "\"a'b\"");
    assert!(write_env(&paths, &Secret::new(KEY), "linha\nnova", "x").is_err(), "no line break in the name");
}

#[test]
fn the_key_is_never_in_an_error_or_a_debug() {
    let home = Home::new("secret");
    let paths = home.paths();
    let secret = |text: &str| assert!(!text.contains("SEGREDO"), "the key showed: {text}");
    secret(&format!("{:?}", Secret::new(KEY)));
    secret(&write_env(&paths, &Secret::new(&format!("{KEY}\u{7}")), "x", "x").unwrap_err());
    secret(&write_env(&paths, &Secret::new(&format!("{KEY}é")), "x", "x").unwrap_err());
    assert!(write_env(&paths, &Secret::new(" \n"), "x", "x").unwrap_err().contains("Cole a chave"));
    // the new text cannot be written (a folder where its copy goes): the old file stays whole, the error names the file
    write_env(&paths, &Secret::new("spak_velha"), "x", "x").unwrap();
    let folder = paths.env_file.parent().unwrap().to_path_buf();
    let blocker = folder.join(format!(".soloist.env.tmp-{}", std::process::id()));
    fs::create_dir_all(blocker.join("dentro")).unwrap();
    let error = write_env(&paths, &Secret::new(KEY), "x", "x").unwrap_err();
    secret(&error);
    assert!(error.contains("soloist.env"), "{error}");
    assert!(fs::read_to_string(&paths.env_file).unwrap().contains("spak_velha"));
    fs::remove_dir_all(&blocker).unwrap();
    assert_eq!(names_in(&folder), ["soloist.env"]);
    // where a folder should be there is a file
    let blocked = Paths { env_file: home.0.join("arquivo/soloist.env"), ..home.paths() };
    fs::write(home.0.join("arquivo"), "").unwrap();
    secret(&write_env(&blocked, &Secret::new(KEY), "x", "x").unwrap_err());
    // systemctl failing, saying the command line with the key: masked
    let home = home.with_program();
    let fake = Fake::new(|_, args: &[&str]| if args.contains(&"enable") { ran(1, &format!("Failed: soloist -n x -k {KEY} -D y")) } else { ran(0, "") });
    let error = turn_on(&paths, &fake, Some(&Secret::new(KEY)), "x", "x").unwrap_err();
    secret(&error);
    assert!(error.contains("-k ***"), "{error}");
    assert!(fake.calls().iter().all(|call| !call.contains("SEGREDO") && !call.contains("sudo")), "{:?}", fake.calls());
    drop(home);
    assert_eq!(mask(&format!("soloist --api-key={KEY} x --api-key {KEY} {KEY}")), "soloist --api-key=*** x --api-key *** spak_***");
}

#[test]
fn the_unit_is_written_once_and_never_over_another() {
    let home = Home::new("unit");
    let paths = home.paths();
    assert!(write_unit(&paths).unwrap());
    assert_eq!(fs::read_to_string(&paths.unit).unwrap(), UNIT);
    for line in ["EnvironmentFile=%h/.config/soloist/soloist.env", "-w 127.0.0.1:0", "RestartPreventExitStatus=10", "WantedBy=default.target"] {
        assert!(UNIT.contains(line), "{line}");
    }
    fs::write(&paths.unit, "a minha").unwrap();
    assert!(!write_unit(&paths).unwrap());
    assert_eq!(fs::read_to_string(&paths.unit).unwrap(), "a minha", "the person's own stays");
    // a link (even one going nowhere) is someone's too
    fs::remove_file(&paths.unit).unwrap();
    std::os::unix::fs::symlink(home.0.join("nada"), &paths.unit).unwrap();
    assert!(!write_unit(&paths).unwrap() && !home.0.join("nada").exists());
}

#[test]
fn turning_on_saves_writes_and_tells_systemd_in_order() {
    let home = Home::new("on");
    let paths = home.paths();
    let systemd = |active: &'static str| move |_: &str, args: &[&str]| ran(if args.contains(&"is-active") && active != "active" { 3 } else { 0 }, active);
    assert!(turn_on(&paths, &Fake::new(systemd("inactive")), Some(&Secret::new(KEY)), "x", "x").unwrap_err().contains("passo 1"));
    let home = home.with_program();
    assert!(turn_on(&paths, &Fake::new(systemd("inactive")), None, "x", "x").unwrap_err().contains("Falta a chave"), "no key saved, none pasted");
    let fake = Fake::new(systemd("inactive"));
    turn_on(&paths, &fake, Some(&Secret::new(KEY)), "Sala", "x").unwrap();
    let user = |rest: &str| format!("systemctl --user {rest}");
    assert_eq!(fake.calls(), [user("is-active soloist.service"), user("daemon-reload"), user("enable --now soloist.service")]);
    assert!(paths.env_file.is_file() && fs::read_to_string(&paths.unit).unwrap() == UNIT);
    // all there and running: nothing written again, the key saved before stays
    let before = fs::read_to_string(&paths.env_file).unwrap();
    let fake = Fake::new(systemd("active"));
    turn_on(&paths, &fake, None, "Outro", "x").unwrap();
    assert_eq!(fake.calls(), [user("is-active soloist.service"), user("enable --now soloist.service")]);
    assert_eq!(fs::read_to_string(&paths.env_file).unwrap(), before);
    // a new key for a service that runs: it restarts to read it
    let fake = Fake::new(systemd("active"));
    turn_on(&paths, &fake, Some(&Secret::new("spak_nova")), "Sala", "x").unwrap();
    assert_eq!(fake.calls().last().unwrap(), &user("restart soloist.service"));
    assert!(fake.calls().iter().all(|call| !call.contains("spak_") && !call.contains("sudo")));
    drop(home);
}

#[test]
fn the_status_is_found_without_changing_anything() {
    let home = Home::new("status");
    let paths = home.paths();
    let net = home.0.join("net");
    fs::create_dir_all(&net).unwrap();
    // nothing there: nothing is run, step 1 is the one to do
    let fake = Fake::new(|_, _| ran(0, ""));
    let status = detect(&paths, &fake, "x86_64", &net);
    assert_eq!(status, Status::default());
    assert!(fake.calls().is_empty(), "{:?}", fake.calls());
    assert_eq!(marks(&status, false), [Mark::Now, Mark::Todo, Mark::Todo, Mark::Todo, Mark::Todo]);
    // all there: the program answers, the key's file is only looked at (no one could read it), systemd says it runs
    let home = home.with_program();
    fs::create_dir_all(paths.env_file.parent().unwrap()).unwrap();
    fs::write(&paths.env_file, "não é lido").unwrap();
    fs::set_permissions(&paths.env_file, fs::Permissions::from_mode(0o000)).unwrap();
    fs::create_dir_all(paths.unit.parent().unwrap()).unwrap();
    fs::write(&paths.unit, UNIT).unwrap();
    let answers = |now: (i32, &'static str), status: &'static str| {
        move |program: &str, args: &[&str]| match (program, args) {
            ("soloist", ["--version"]) => ran(0, "soloist 1.3.10.74 build 1\n"),
            ("soloist", ["ctl", "now", ..]) => ran(now.0, now.1),
            ("soloist", ["ctl", "status", ..]) => ran(0, status),
            ("systemctl", [_, "is-enabled", _]) => ran(0, "enabled\n"),
            ("systemctl", [_, "is-active", _]) => ran(0, "active\n"),
            _ => ran(1, ""),
        }
    };
    let fake = Fake::new(answers((0, "Tocando: Canção"), ""));
    let status = detect(&paths, &fake, "x86_64", &net);
    assert_eq!(status.program, Program::Ready("soloist 1.3.10.74 build 1".into()));
    assert_eq!((status.key, status.unit, status.enabled, status.active.as_str(), status.login), (KeyFile::Private, true, true, "active", Login::In));
    assert_eq!(marks(&status, false), [Mark::Done; 5]);
    assert!(fake.calls().iter().all(|call| ["--version", "is-enabled", "is-active", "ctl now"].iter().any(|read| call.contains(read))), "{:?}", fake.calls());
    // running but no one chose it yet: step 5 is the one to do
    let status = detect(&paths, &Fake::new(answers((3, "Error: not logged in"), "")), "x86_64", &net);
    assert_eq!((status.login, marks(&status, false)[4]), (Login::Waiting, Mark::Now));
    // `now` with nothing to say: `status` tells; nothing answering: off
    assert_eq!(detect(&paths, &Fake::new(answers((1, "nada"), "logged in: yes")), "x86_64", &net).login, Login::In);
    assert_eq!(detect(&paths, &Fake::new(answers((2, "error: cannot determine websocket port"), "")), "x86_64", &net).login, Login::Off);
    // a key's file others read: said, with its mode
    fs::set_permissions(&paths.env_file, fs::Permissions::from_mode(0o644)).unwrap();
    let status = detect(&paths, &Fake::new(answers((0, ""), "")), "x86_64", &net);
    assert_eq!(status.key, KeyFile::Open(0o644));
    assert!(hints(&status, false, "")[1].contains("644"));
    // a key pasted and not saved yet fills steps 2 and 3
    let fresh = Status { program: Program::Ready("v".into()), ..Default::default() };
    assert_eq!(marks(&fresh, true), [Mark::Done, Mark::Done, Mark::Done, Mark::Now, Mark::Todo]);
    assert!(hints(&fresh, true, "Sala")[4].contains("\"Sala\""), "the phone step names the device");
    drop(home);
}

#[test]
fn an_expired_version_says_so_and_a_new_one_is_downloaded() {
    let home = Home::new("expired").with_program();
    let paths = home.paths();
    let scratch = home.0.join("tmp");
    fs::create_dir_all(&scratch).unwrap();
    let expired = Fake::new(|_, _| ran(EXPIRED, "expired"));
    let status = detect(&paths, &expired, "x86_64", &home.0);
    assert_eq!((status.program.clone(), marks(&status, false)[0]), (Program::Expired, Mark::Now));
    assert!(hints(&status, false, "")[0].contains("Esta versão expirou"));
    assert_eq!(status.login, Login::Off, "an expired one is not asked to log in");
    // the download: the archive comes, tar takes the program out, it runs, it is installed 755 over the old one
    let fetched = |url: &str, archive: &Path| {
        assert_eq!(url, DOWNLOAD);
        fs::write(archive, "tar.gz").map_err(|error| error.to_string())
    };
    let new_version = |version: (i32, &'static str)| {
        move |program: &str, args: &[&str]| match (program, args) {
            ("tar", ["-xzf", _, "-C", folder, "soloist"]) => {
                fs::write(Path::new(folder).join("soloist"), "novo").unwrap();
                ran(0, "")
            }
            ("soloist", ["--version"]) => ran(version.0, version.1),
            _ => ran(1, ""),
        }
    };
    let error = install_program(&paths, &Fake::new(new_version((EXPIRED, ""))), "x86_64", &scratch, fetched).unwrap_err();
    assert!(error.contains("expirou"), "{error}");
    assert_eq!(fs::read_to_string(&paths.program).unwrap(), "programa", "nothing installed");
    let version = install_program(&paths, &Fake::new(new_version((0, "soloist 2.0\n"))), "x86_64", &scratch, fetched).unwrap();
    assert_eq!(version, "soloist 2.0");
    assert_eq!((fs::read_to_string(&paths.program).unwrap().as_str(), mode(&paths.program)), ("novo", 0o755));
    assert!(names_in(&scratch).is_empty() && names_in(paths.program.parent().unwrap()) == ["soloist"], "nothing left behind");
    // an archive whose `soloist` is a link: not installed
    let linked = Fake::new(|program: &str, args: &[&str]| match (program, args) {
        ("tar", [.., "-C", folder, _]) => {
            std::os::unix::fs::symlink("/etc/hostname", Path::new(folder).join("soloist")).unwrap();
            ran(0, "")
        }
        _ => ran(0, "v"),
    });
    assert!(install_program(&paths, &linked, "x86_64", &scratch, fetched).is_err());
    assert_eq!(fs::read_to_string(&paths.program).unwrap(), "novo");
}

#[test]
fn only_an_x86_64_computer_downloads_soloist() {
    assert!(arch_ok("x86_64").is_ok());
    let error = arch_ok("aarch64").unwrap_err();
    assert!(error.contains("x86_64") && error.contains("aarch64"), "{error}");
    let home = Home::new("arch");
    let asked = Cell::new(false);
    let fetch = |_: &str, _: &Path| {
        asked.set(true);
        Ok(())
    };
    assert!(install_program(&home.paths(), &Fake::new(|_, _| ran(0, "")), "aarch64", &home.0, fetch).is_err());
    assert!(!asked.get(), "nothing downloaded");
    let status = detect(&home.paths(), &Fake::new(|_, _| ran(0, "")), "aarch64", &home.0);
    assert!(hints(&status, false, "")[0].contains("só existe para computadores x86_64 (este é aarch64)"));
    // the download comes from Spotify's CDN only
    assert!(download_allowed(DOWNLOAD) && download_allowed("https://outro.spotifycdn.com/a"));
    for bad in
        ["http://soloist-builds.spotifycdn.com/a", "https://evil.com/a", "https://soloist-builds.spotifycdn.com@evil.com/", "https://spotifycdn.com.evil.io/"]
    {
        assert!(!download_allowed(bad), "{bad}");
    }
}

#[test]
fn a_tunnel_that_is_up_warns_and_the_name_has_a_default() {
    let home = Home::new("net");
    for (name, flags) in [("eth0", "0x1003"), ("wg0", "0x1091"), ("tun0", "0x1090"), ("tailscale0", "0x11"), ("lo", "0x9")] {
        fs::create_dir_all(home.0.join(name)).unwrap();
        fs::write(home.0.join(name).join("flags"), format!("{flags}\n")).unwrap();
    }
    assert_eq!(tunnels(&home.0), ["tailscale0", "wg0"], "tun0 is down");
    assert!(vpn_warning(&tunnels(&home.0)).contains("tailscale0, wg0"));
    assert_eq!(default_name("edsu-linux.casa\n"), "Soloist edsu-linux");
    assert_eq!(default_name(""), "Soloist");
}

#[test]
fn the_details_never_show_the_key() {
    let home = Home::new("details").with_program();
    let fake = Fake::new(|program: &str, _: &[&str]| match program {
        "systemctl" => {
            ran(0, "ActiveState=active\nSubState=running\nUnitFileState=enabled\nActiveEnterTimestamp=seg 2026-11-16\nExecMainStatus=0\nNRestarts=0\n")
        }
        _ => ran(0, &format!("soloist: running (pid 1)\n  cmd: soloist -k {KEY}\n  logged in: yes\n")),
    });
    let text = install::details(&home.paths(), &fake);
    assert!(text.starts_with("Serviço soloist.service: active (running), enabled\nLigado desde seg 2026-11-16"), "{text}");
    assert!(text.contains("logged in: yes") && !text.contains("SEGREDO"), "{text}");
}
