//! Ajustes › Integrações › Soloist › Configurar (Linux): Soloist set up from the app, step by step, with no other
//! program: (1) its download from Spotify into `~/.local/bin`, (2) the developer key pasted, (3) the device's name,
//! (4) both saved in `~/.config/soloist/soloist.env` and the user's `soloist.service` turned on, (5) Soloist chosen on
//! the phone. Each step shows done, now or later by what is found (`detect`, only looked at); what is done is never
//! redone, and what is changed (`install`) changes only when the person presses its button. The work (curl, tar,
//! systemctl) runs on a thread of its own; the commands go through a `Runner`, so the tests use fakes in a temporary
//! folder. The key never reaches `Prefs`, a log, a toast or the MCP: only the env file.

#![cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]

use crate::prelude::*;
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

pub(crate) mod detect;
pub(crate) mod install;
#[cfg(all(test, unix))]
mod tests;

pub(crate) use detect::{KeyFile, Login, Program, Status};
pub(crate) use install::Secret;

/// The user's service the setup writes and asks systemd about.
pub(crate) const SERVICE: &str = "soloist.service";
/// `soloist --version`'s code for a build past its 90 days (the unit does not restart it for that one).
pub(crate) const EXPIRED: i32 = 10;
/// Spotify's page where the key is made (it needs Premium).
pub(crate) const DASHBOARD: &str = "https://developer.spotify.com/dashboard/soloist";
/// How often the open steps look again (step 5 waits for the phone).
const EVERY: Duration = Duration::from_secs(3);
/// The longest a command may take before it is stopped.
const COMMAND_TIME: Duration = Duration::from_secs(60);

/// Where the setup puts things, all under one home: the places the unit names with `%h`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Paths {
    pub(crate) program: std::path::PathBuf,
    pub(crate) env_file: std::path::PathBuf,
    pub(crate) unit: std::path::PathBuf,
    pub(crate) data: std::path::PathBuf,
}

impl Paths {
    pub(crate) fn under(home: &Path) -> Paths {
        Paths {
            program: home.join(".local/bin/soloist"),
            env_file: home.join(".config/soloist/soloist.env"),
            unit: home.join(".config/systemd/user").join(SERVICE),
            data: home.join(".local/share/soloist"),
        }
    }

    fn of_this_user() -> Result<Paths, String> {
        std::env::var_os("HOME").filter(|home| !home.is_empty()).map(|home| Paths::under(Path::new(&home))).ok_or_else(|| "sem HOME".to_string())
    }
}

/// How a command ended.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Ran {
    /// None when a signal ended it.
    pub(crate) code: Option<i32>,
    /// What it printed: the standard output, then the errors.
    pub(crate) output: String,
}

/// Runs the setup's commands: the system's (`System`) or, in the tests, a fake.
pub(crate) trait Runner {
    /// `program` with `args`, nothing on its input; Err when it could not start or took too long.
    fn run(&self, program: &Path, args: &[&str]) -> Result<Ran, String>;
}

/// The real commands, stopped after `COMMAND_TIME`. Errors name the program, never its arguments.
pub(crate) struct System;

impl Runner for System {
    fn run(&self, program: &Path, args: &[&str]) -> Result<Ran, String> {
        let name = program.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
        let mut child = crate::editor::command(program)
            .args(args)
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("{name} não pôde ser iniciado: {error}"))?;
        let deadline = Instant::now() + COMMAND_TIME;
        // ponytail: waits by polling and reads the output after the end (the setup's commands print a few lines; one
        // printing past the pipe's 64 KB would wait until the deadline); read on threads if one ever does
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(30)),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{name} demorou demais"));
                }
                Err(error) => return Err(format!("{name}: {error}")),
            }
        }
        let output = child.wait_with_output().map_err(|error| format!("{name}: {error}"))?;
        let text = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
        Ok(Ran { code: output.status.code(), output: text })
    }
}

/// A step's mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Done,
    Now,
    Todo,
}

impl Mark {
    pub(crate) fn words(self) -> &'static str {
        match self {
            Mark::Done => "feito",
            Mark::Now => "agora",
            Mark::Todo => "a fazer",
        }
    }
}

/// The five steps' marks: each done by what was found (`typed`: a key waits in the field, so steps 2 and 3 are filled
/// in); the first not done is the one to do now.
pub(crate) fn marks(status: &Status, typed: bool) -> [Mark; 5] {
    let saved = status.key != KeyFile::Missing;
    let done = [
        matches!(status.program, Program::Ready(_)),
        saved || typed,
        saved || typed,
        saved && status.unit && status.enabled && status.running(),
        status.login == Login::In,
    ];
    let first = done.iter().position(|done| !done);
    std::array::from_fn(|index| match (done[index], Some(index) == first) {
        (true, _) => Mark::Done,
        (false, true) => Mark::Now,
        (false, false) => Mark::Todo,
    })
}

/// The steps' titles.
pub(crate) const TITLES: [&str; 5] =
    ["Baixar o Soloist", "Gerar a chave de desenvolvedor", "Dar um nome ao dispositivo", "Salvar e ligar com o computador", "Escolher o Soloist no celular"];

/// What each step says under its title, by what was found: `typed` a key waits in the field, `name` the one typed.
pub(crate) fn hints(status: &Status, typed: bool, name: &str) -> [String; 5] {
    let program = match (&status.foreign, &status.program) {
        (Some(arch), _) => format!("O Soloist só existe para computadores x86_64 (este é {arch})."),
        (None, Program::Ready(version)) => format!("{version}, em ~/.local/bin/soloist. Cada versão vale 90 dias."),
        (None, Program::Expired) => "Esta versão expirou (cada uma vale 90 dias): atualize para baixar a nova.".into(),
        (None, Program::Broken(why)) => format!("O ~/.local/bin/soloist não rodou ({why}): baixe de novo."),
        (None, Program::Missing) => "O programa oficial do Spotify vai para ~/.local/bin/soloist, sem sudo.".into(),
    };
    let key = match (status.key, typed) {
        (_, true) => "Chave colada: ela é salva no passo 4.".to_string(),
        (KeyFile::Private, false) => "Já está salva no ~/.config/soloist/soloist.env, que só o seu usuário lê. Para trocar, cole outra.".into(),
        (KeyFile::Open(mode), false) => format!("Já está salva, mas o soloist.env tem a permissão {mode:o}: outros usuários podem ler (o certo é 600)."),
        (KeyFile::Missing, false) => "Abra o painel (precisa do Spotify Premium), gere a chave do Soloist e cole abaixo. Ela não aparece na tela.".into(),
    };
    let device = match (status.key, typed) {
        (KeyFile::Missing, _) | (_, true) => "O nome que aparece no Spotify do celular.".to_string(),
        _ => "Já está salvo no soloist.env. Para trocar, cole a chave de novo e salve.".into(),
    };
    let service = match (status.unit, status.enabled, status.running()) {
        (true, true, true) => "O soloist.service do seu usuário está habilitado e rodando: ele abre sozinho quando você entra.".to_string(),
        (true, _, false) if status.active == "failed" => "O soloist.service falhou. Veja o Status, ou ligue de novo.".into(),
        (true, false, _) => "O soloist.service existe, mas não abre sozinho: ligar o habilita.".into(),
        (true, true, false) => "O soloist.service está parado.".into(),
        (false, ..) => "Grava a chave e o nome (só o seu usuário lê) e o serviço soloist.service do seu usuário, e liga. Sem sudo.".into(),
    };
    let shown =
        if name.trim().is_empty() || (status.key != KeyFile::Missing && !typed) { "o nome que você deu".to_string() } else { format!("\"{}\"", name.trim()) };
    let phone = match status.login {
        Login::In => "O Soloist entrou na sua conta pelo Spotify Connect.".to_string(),
        _ => format!("No Spotify do celular, na mesma rede Wi-Fi, toque no ícone de dispositivos e escolha {shown}."),
    };
    [program, key, device, service, phone]
}

/// What a press of the setup asks for, done on a thread.
pub(crate) enum Job {
    /// Step 1, and Atualizar: downloads and installs the program (a running service restarts on the new one).
    Download,
    /// Step 4: the key (when pasted) and the name saved, the unit written when missing, systemd told.
    TurnOn {
        key: Option<Secret>,
        name: String,
    },
    Start,
    Stop,
    /// What the service and Soloist say of themselves.
    Details,
}

impl Job {
    /// What shows while it runs.
    fn doing(&self) -> &'static str {
        match self {
            Job::Download => "Baixando…",
            Job::TurnOn { .. } => "Ligando…",
            Job::Start => "Ligando…",
            Job::Stop => "Parando…",
            Job::Details => "Conferindo…",
        }
    }

    /// Its name, for the tests.
    #[cfg(test)]
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Job::Download => "download",
            Job::TurnOn { key: Some(_), .. } => "turn_on_with_key",
            Job::TurnOn { key: None, .. } => "turn_on",
            Job::Start => "start",
            Job::Stop => "stop",
            Job::Details => "details",
        }
    }

    /// The job done, by `runner` on the files of `paths`.
    pub(crate) fn run(self, paths: &Paths, runner: &dyn Runner) -> Result<String, String> {
        let default_name = detect::default_name(&detect::this_host());
        match self {
            Job::Download => {
                let version = install::install_program(paths, runner, std::env::consts::ARCH, &std::env::temp_dir(), install::fetch)?;
                // a service that runs (or stopped when its version expired) goes on with the new one
                let enabled = runner.run(Path::new("systemctl"), &["--user", "is-enabled", SERVICE]).is_ok_and(|ran| ran.output.trim().starts_with("enabled"));
                if paths.unit.is_file() && enabled {
                    install::systemctl(runner, &["restart", SERVICE])?;
                    return Ok(format!("Soloist atualizado e religado: {version}."));
                }
                Ok(format!("Soloist baixado: {version}."))
            }
            Job::TurnOn { key, name } => install::turn_on(paths, runner, key.as_ref(), &name, &default_name),
            Job::Start => install::service(runner, "start"),
            Job::Stop => install::service(runner, "stop"),
            Job::Details => Ok(install::details(paths, runner)),
        }
    }
}

enum Message {
    Checked(Status),
    Finished(Result<String, String>),
    Details(String),
}

/// The setup's side in the app: the steps open or not, the two fields, what was found and the work running.
#[derive(Default)]
pub(crate) struct Setup {
    /// Configurar was pressed: the steps show.
    pub(crate) open: bool,
    /// The key as pasted, until step 4 saves it: never in `Prefs`, a toast or the MCP.
    pub(crate) key_field: String,
    pub(crate) name_field: String,
    /// What the last look found, and when.
    pub(crate) status: Option<Status>,
    checked: Option<Instant>,
    checking: bool,
    /// What runs now ("Baixando…").
    pub(crate) busy: Option<&'static str>,
    /// What the last press ended with.
    pub(crate) said: Option<Result<String, String>>,
    /// What Status showed.
    pub(crate) details: Option<String>,
    channel: Option<(Sender<Message>, Receiver<Message>)>,
    /// The jobs asked, for the tests (they start no thread).
    #[cfg(test)]
    pub(crate) asked: Vec<&'static str>,
}

impl Setup {
    fn sender(&mut self) -> Sender<Message> {
        self.channel.get_or_insert_with(channel).0.clone()
    }

    /// The steps opened: the name's field starts with the default.
    pub(crate) fn open(&mut self) {
        self.open = true;
        if self.name_field.trim().is_empty() {
            self.name_field = detect::default_name(&detect::this_host());
        }
    }

    /// The steps closed: a key not saved goes with them.
    pub(crate) fn close(&mut self) {
        self.open = false;
        self.key_field.clear();
        self.details = None;
    }

    /// The steps in words, for the MCP: their marks and what each says. The key's field says only whether it holds one.
    pub(crate) fn report(&self) -> String {
        let Some(status) = &self.status else { return "Conferindo o Soloist agora: peça `setup` de novo em um instante.".into() };
        let typed = !self.key_field.trim().is_empty();
        let marks = marks(status, typed);
        let hints = hints(status, typed, &self.name_field);
        let mut lines = vec!["Configurar o Soloist (Ajustes › Integrações › Soloist):".to_string()];
        for index in 0..5 {
            lines.push(format!("{}. {} — {}. {}", index + 1, TITLES[index], marks[index].words(), hints[index]));
        }
        if !status.vpn.is_empty() {
            lines.push(vpn_warning(&status.vpn));
        }
        if let Some(busy) = self.busy {
            lines.push(format!("Agora: {busy}"));
        } else if let Some(said) = &self.said {
            lines.push(format!("Último pedido: {}", said.as_ref().unwrap_or_else(|why| why)));
        }
        lines.join("\n")
    }
}

/// The warning when a tunnel is up.
pub(crate) fn vpn_warning(tunnels: &[String]) -> String {
    format!("VPN ou túnel ativo ({}): pode impedir que o celular encontre o Soloist na rede.", tunnels.join(", "))
}

impl DrawingApp {
    /// Each frame: what the setup's threads said. While something runs, the frames keep coming to take it.
    pub(crate) fn soloist_setup_tick(&mut self, ctx: &Context) {
        let setup = &mut self.integrations.soloist_setup;
        let mut said = None;
        while let Some(message) = setup.channel.as_ref().and_then(|(_, inbox)| inbox.try_recv().ok()) {
            match message {
                Message::Checked(status) => {
                    (setup.status, setup.checked, setup.checking) = (Some(status), Some(Instant::now()), false);
                }
                Message::Details(text) => (setup.details, setup.busy) = (Some(text), None),
                Message::Finished(result) => {
                    said = Some(result.clone().unwrap_or_else(|why| format!("Soloist: {why}")));
                    (setup.said, setup.busy, setup.checked) = (Some(result), None, None);
                }
            }
        }
        if setup.busy.is_some() || setup.checking {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if let Some(said) = said {
            self.toast(said);
        }
    }

    /// While the steps show: a look every few seconds (step 5 waits for the phone). Only on Linux, never in a test.
    pub(crate) fn soloist_setup_watch(&mut self, ctx: &Context) {
        let setup = &self.integrations.soloist_setup;
        if setup.checked.is_none_or(|checked| checked.elapsed() >= EVERY) {
            self.soloist_setup_check();
        }
        ctx.request_repaint_after(EVERY);
    }

    /// A look at the setup, on a thread (only one at a time).
    pub(crate) fn soloist_setup_check(&mut self) {
        let setup = &mut self.integrations.soloist_setup;
        if setup.checking || cfg!(any(windows, target_os = "macos", test)) {
            return;
        }
        let Ok(paths) = Paths::of_this_user() else { return };
        setup.checking = true;
        let outbox = setup.sender();
        std::thread::spawn(move || {
            let status = detect::detect(&paths, &System, std::env::consts::ARCH, Path::new("/sys/class/net"));
            let _ = outbox.send(Message::Checked(status));
        });
    }

    /// Starts `job` on a thread: what it ends with comes back in a toast and on the card. Err when something already
    /// runs or this is not Linux.
    pub(crate) fn soloist_setup_run(&mut self, job: Job) -> Result<String, String> {
        if cfg!(any(windows, target_os = "macos")) {
            return Err("o Soloist do Spotify só existe no Linux".into());
        }
        let setup = &mut self.integrations.soloist_setup;
        if let Some(busy) = setup.busy {
            return Err(format!("o Soloist já está ocupado ({busy}): espere terminar"));
        }
        let doing = job.doing();
        let started = format!("{doing} A resposta vem num aviso e no `setup`.");
        (setup.busy, setup.said) = (Some(doing), None);
        #[cfg(test)]
        setup.asked.push(job.name());
        // never the real commands in a test
        if cfg!(test) {
            return Ok(started);
        }
        let paths = Paths::of_this_user()?;
        let outbox = setup.sender();
        std::thread::spawn(move || {
            let message = match job {
                Job::Details => Message::Details(Job::Details.run(&paths, &System).unwrap_or_default()),
                job => Message::Finished(job.run(&paths, &System)),
            };
            let _ = outbox.send(message);
        });
        Ok(started)
    }

    /// The MCP's `integrations soloist setup`: the steps as last seen (and a new look, when that one is old).
    pub(crate) fn soloist_setup_report(&mut self) -> String {
        let setup = &self.integrations.soloist_setup;
        if setup.checked.is_none_or(|checked| checked.elapsed() >= EVERY) {
            self.soloist_setup_check();
        }
        self.integrations.soloist_setup.report()
    }
}
