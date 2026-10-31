//! The installer: the same program, run as `MagoAgent-Instalador` (or `magoagent instalar`). The
//! person picks the folder (C:\Program Files\MagoAgent by default, like Steam; Windows asks for
//! the administrator there, and the folder is then given to the person so the app can write its
//! data in it). It closes a MagoAgent that is open, copies itself into the folder, moves the data
//! of an earlier install or build into `dados`, downloads everything the voice needs (whisper.cpp,
//! its models, Piper and all the voices of Ajustes › Voz), lets the person pick the voice by
//! hearing it, records the folder (`record_install_dir`) and adds MagoAgent to the system's menu.
//! Nothing is downloaded later, when a feature is used.
// ponytail: no uninstaller (deleting the folder and the menu shortcut is all it takes); the Mac
// gets a plain folder, not an .app bundle (build-macos-app.sh makes that one)

use crate::prelude::*;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};

/// Whether this run is the installer: by the program's name or by its first argument.
pub(crate) fn wanted(args: &[String]) -> bool {
    let name = std::env::current_exe().ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase())).unwrap_or_default();
    args.first().is_some_and(|a| a == "instalar") || name.contains("instalador")
}

/// The folder given with `--pasta`: the run Windows started as administrator, which installs
/// there right away.
fn folder_arg(args: &[String]) -> Option<String> {
    args.iter().position(|a| a == "--pasta").and_then(|i| args.get(i + 1)).cloned()
}

pub(crate) fn run(args: &[String]) -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Instalar o MagoAgent")
        .with_inner_size([640.0, 600.0])
        .with_resizable(false)
        .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).expect("assets/icon.png is a png"));
    let given = folder_arg(args);
    eframe::run_native(
        "Instalar o MagoAgent",
        eframe::NativeOptions { viewport, ..Default::default() },
        Box::new(move |cc| {
            crate::app::ui::install_fonts(&cc.egui_ctx, false);
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let mut installer = Installer::default();
            if let Some(dir) = given {
                installer.dir = dir;
                installer.start(&cc.egui_ctx);
            }
            Ok(Box::new(installer))
        }),
    )
}

enum Step {
    Start,
    Working(Receiver<Msg>),
    Voice,
    Done,
    Failed(String),
}

enum Msg {
    Line(String),
    Done,
    Failed(String),
}

struct Installer {
    step: Step,
    /// Where it installs, as typed or picked.
    dir: String,
    lines: Vec<String>,
    voice: usize,
    sample: Option<Player>,
    /// The download under way: its .part file, for the bytes so far.
    part_check: f64,
    part: String,
}

impl Default for Installer {
    fn default() -> Self {
        let dir = recorded_install_dir().or_else(default_install_dir).map(|d| d.display().to_string()).unwrap_or_default();
        Installer { step: Step::Start, dir, lines: Vec::new(), voice: 0, sample: None, part_check: 0.0, part: String::new() }
    }
}

impl Installer {
    fn dir(&self) -> PathBuf {
        // a trailing \ would escape the quote around it when Windows hands it to the administrator run
        PathBuf::from(self.dir.trim().trim_end_matches(['\\', '/']))
    }

    /// Installs on a thread, saying each step.
    fn start(&mut self, ctx: &Context) {
        let (tx, rx) = channel();
        let (repaint, dir) = (ctx.clone(), self.dir());
        std::thread::spawn(move || {
            let say = |line: String| {
                let _ = tx.send(Msg::Line(line));
                repaint.request_repaint();
            };
            let end = match install(&dir, &say) {
                Ok(()) => Msg::Done,
                Err(e) => Msg::Failed(e),
            };
            let _ = tx.send(end);
            repaint.request_repaint();
        });
        self.step = Step::Working(rx);
    }
}

impl eframe::App for Installer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let accent = Color32::from_rgb(0x4f, 0x8c, 0xff);
        let big = |ui: &mut egui::Ui, text: &str| {
            ui.add_sized([200.0, 36.0], egui::Button::new(egui::RichText::new(text).size(15.0).color(Color32::WHITE)).fill(accent))
        };
        egui::Frame::new().inner_margin(28).show(ui, |ui| {
            ui.label(egui::RichText::new("MagoAgent").size(26.0).strong());
            ui.add_space(6.0);
            let dir = self.dir();
            match &mut self.step {
                Step::Start => {
                    ui.label(egui::RichText::new("Pasta de instalação").size(14.0).strong());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.dir).desired_width(440.0).font(FontId::proportional(14.0)));
                        if ui.button("Escolher…").clicked()
                            && let Some(picked) = rfd::FileDialog::new().set_title("Pasta de instalação do MagoAgent").pick_folder()
                        {
                            // inside the folder picked, in its own, as Steam does
                            let own = picked.file_name().is_some_and(|n| n.eq_ignore_ascii_case("MagoAgent"));
                            self.dir = if own { picked } else { picked.join("MagoAgent") }.display().to_string();
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Tudo do app fica nessa pasta: o programa, os desenhos, os chats e os modelos de voz.")
                            .size(13.0)
                            .color(Color32::GRAY),
                    );
                    ui.add_space(12.0);
                    let (gpu, cuda) = crate::voice::gpu();
                    let size = if cuda { "cerca de 2 GB" } else { "cerca de 1 GB" };
                    ui.label(
                        egui::RichText::new(format!(
                            "Baixa agora, uma vez só ({size}): o reconhecimento de voz (whisper.cpp e o modelo{}), o Piper e as {} vozes. GPU: {gpu}.",
                            if cuda { " grande para a GPU" } else { "" },
                            crate::voice::VOICES.len()
                        ))
                        .size(14.0)
                        .color(Color32::GRAY),
                    );
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Se o MagoAgent estiver aberto, ele é fechado (o desenho já está salvo).").size(13.0).color(Color32::GRAY));
                    ui.add_space(24.0);
                    if big(ui, "Instalar").clicked() && !self.dir.trim().is_empty() {
                        if !writable(&dir) && cfg!(windows) {
                            // Program Files: this run can't write there; Windows asks for the
                            // administrator and that run installs
                            match elevate(&dir) {
                                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                                Err(e) => self.step = Step::Failed(e),
                            }
                        } else {
                            self.start(&ctx);
                        }
                    }
                }
                Step::Working(rx) => {
                    let mut next = None;
                    for msg in rx.try_iter() {
                        match msg {
                            Msg::Line(line) => self.lines.push(line),
                            Msg::Done => next = Some(Step::Voice),
                            Msg::Failed(e) => next = Some(Step::Failed(e)),
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(egui::RichText::new(format!("Instalando em {}…", dir.display())).size(15.0));
                    });
                    // how far the download is
                    let now = ctx.input(|i| i.time);
                    if now - self.part_check > 0.5 {
                        self.part_check = now;
                        self.part = downloading(&dir).unwrap_or_default();
                    }
                    ui.add_space(8.0);
                    lines(ui, &self.lines, &self.part);
                    ctx.request_repaint_after(std::time::Duration::from_millis(500));
                    if let Some(step) = next {
                        self.step = step;
                    }
                }
                Step::Voice => {
                    ui.label(egui::RichText::new("Escolha a voz do Maestro").size(17.0).strong());
                    ui.label(egui::RichText::new("Ouça cada uma; dá para trocar depois em Ajustes › Voz.").size(13.0).color(Color32::GRAY));
                    ui.add_space(10.0);
                    let voice_dir = dir.join("dados").join("voice");
                    egui::ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
                        for (k, name) in crate::voice::VOICES.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let language = if name.starts_with("pt") { "português" } else { "inglês" };
                                ui.radio_value(&mut self.voice, k, egui::RichText::new(format!("{} · {language}", voice_name(name))).size(14.0));
                                ui.add_space(8.0);
                                if ui.small_button("▶ Ouvir").clicked() {
                                    let piper = crate::voice::piper(&voice_dir, name);
                                    let text = if name.starts_with("pt") {
                                        "Olá! Eu sou o Maestro do MagoAgent. Vamos trabalhar?"
                                    } else {
                                        "Hi! I'm the MagoAgent Maestro. Shall we get to work?"
                                    };
                                    self.voice = k;
                                    self.sample = Player::say(text, piper.as_ref(), &name[..2]);
                                }
                            });
                        }
                    });
                    ui.add_space(14.0);
                    if big(ui, "Continuar").clicked() {
                        self.sample = None;
                        match save_voice(&dir, crate::voice::VOICES[self.voice]) {
                            Ok(()) => self.step = Step::Done,
                            Err(e) => self.step = Step::Failed(e),
                        }
                    }
                }
                Step::Done => {
                    ui.label(egui::RichText::new("Pronto: o MagoAgent está instalado.").size(17.0).strong());
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("Ele está no menu Iniciar e em {}.", dir.display())).size(14.0).color(Color32::GRAY));
                    ui.add_space(22.0);
                    if big(ui, "Abrir o MagoAgent").clicked() {
                        open_app(&dir);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Step::Failed(why) => {
                    ui.label(egui::RichText::new("A instalação parou.").size(17.0).strong().color(Color32::from_rgb(0xff, 0x6b, 0x6b)));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(why.as_str()).size(14.0));
                    ui.add_space(8.0);
                    lines(ui, &self.lines, "");
                    ui.add_space(12.0);
                    if big(ui, "Tentar de novo").clicked() {
                        // what came down stays; the rest is fetched again
                        self.lines.clear();
                        self.step = Step::Start;
                    }
                }
            }
        });
    }
}

fn lines(ui: &mut egui::Ui, lines: &[String], part: &str) {
    egui::ScrollArea::vertical().max_height(330.0).stick_to_bottom(true).show(ui, |ui| {
        for line in lines {
            ui.label(egui::RichText::new(line).size(13.0).monospace());
        }
        if !part.is_empty() {
            ui.label(egui::RichText::new(part).size(13.0).monospace().color(Color32::GRAY));
        }
    });
}

/// Whether this run can write in `dir` (making it when it isn't there).
fn writable(dir: &Path) -> bool {
    let probe = dir.join(".magoagent-teste");
    let ok = fs::create_dir_all(dir).is_ok() && fs::write(&probe, b"").is_ok();
    let _ = fs::remove_file(probe);
    ok
}

/// The installer again, as administrator (Windows asks), installing into `dir`.
fn elevate(dir: &Path) -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let quote = |p: &Path| p.display().to_string().replace('\'', "''");
    let script = format!("Start-Process -FilePath '{}' -ArgumentList 'instalar --pasta \"{}\"' -Verb RunAs", quote(&me), quote(dir));
    let ok = crate::ide::command("powershell").args(["-NoProfile", "-Command", &script]).status().is_ok_and(|s| s.success());
    if ok { Ok(()) } else { Err("Para instalar nessa pasta o Windows precisa da permissão de administrador. Permita, ou escolha outra pasta.".into()) }
}

/// The installed app, as the person (not as the administrator the installer may run as).
fn open_app(dir: &Path) {
    let program = dir.join(crate::voice::exe("magoagent"));
    if cfg!(windows) {
        let _ = std::process::Command::new("explorer.exe").arg(&program).spawn();
    } else {
        let _ = std::process::Command::new(&program).current_dir(dir).spawn();
    }
}

/// The .part file being downloaded and its size so far.
fn downloading(dir: &Path) -> Option<String> {
    let mut stack = vec![dir.join("dados").join("voice")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "part") {
                let mb = entry.metadata().map_or(0, |m| m.len()) / 1_000_000;
                return Some(format!("  {mb} MB baixados"));
            }
        }
    }
    None
}

/// The whole install into `dir`, saying each step.
fn install(dir: &Path, say: &dyn Fn(String)) -> Result<(), String> {
    close_running(say);
    // the data of an earlier install (wherever it was) or of a build from before the installer
    let earlier: Vec<PathBuf> =
        [recorded_install_dir(), old_install_dir()].into_iter().flatten().filter(|d| d != dir).map(|d| d.join("dados")).chain(legacy_data_dir()).collect();
    let program = install_into(dir, &earlier, say)?;
    give_to_person(dir, say);
    record_install_dir(dir);
    shortcut(&program, say);
    say("Tudo instalado.".into());
    Ok(())
}

/// Where the first installer put the app, before the folder could be picked.
fn old_install_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").filter(|_| cfg!(windows)).map(|d| PathBuf::from(d).join("Programs").join("MagoAgent"))
}

/// The program, the data (the first of `earlier` that exists) and the voice into `dir`; gives
/// where the program went.
fn install_into(dir: &Path, earlier: &[PathBuf], say: &dyn Fn(String)) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    say("Copiando o programa…".into());
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let target = dir.join(crate::voice::exe("magoagent"));
    if me != target {
        let tmp = target.with_extension("novo");
        fs::copy(&me, &tmp).map_err(|e| format!("Não foi possível copiar o programa: {e}"))?;
        fs::rename(&tmp, &target).map_err(|e| format!("Não foi possível trocar o programa (ele está aberto?): {e}"))?;
    }
    let data = dir.join("dados");
    if !data.is_dir() {
        match earlier.iter().find(|d| d.is_dir()) {
            Some(old) => {
                say(format!("Trazendo os dados de {}…", old.display()));
                if fs::rename(old, &data).is_err() {
                    copy_dir(old, &data).map_err(|e| format!("Não foi possível trazer os dados: {e}"))?;
                    let _ = fs::remove_dir_all(old);
                }
                // an earlier install's folder keeps only its program: it goes, so one copy is left
                if old.file_name().is_some_and(|n| n == "dados")
                    && let Some(folder) = old.parent()
                {
                    let _ = fs::remove_file(folder.join(crate::voice::exe("magoagent")));
                    let _ = fs::remove_dir(folder);
                }
            }
            None => fs::create_dir_all(&data).map_err(|e| e.to_string())?,
        }
    }
    let _ = fs::create_dir_all(data.join("projetos"));
    fetch_voice(&data.join("voice"), say)?;
    Ok(target)
}

/// On Windows the folder becomes the person's to change (as Steam does with its own in Program
/// Files): the app writes its data there without being administrator.
fn give_to_person(dir: &Path, say: &dyn Fn(String)) {
    if !cfg!(windows) {
        return;
    }
    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(domain), Ok(name)) => format!("{domain}\\{name}"),
        (_, Ok(name)) => name,
        _ => return,
    };
    let ok = crate::ide::command("icacls").arg(dir).args(["/grant", &format!("{user}:(OI)(CI)M"), "/Q"]).status().is_ok_and(|s| s.success());
    if !ok {
        say("Não foi possível liberar a pasta para o seu usuário.".into());
    }
}

/// A MagoAgent that is open is asked to close (it saves), then made to.
fn close_running(say: &dyn Fn(String)) {
    let running = || {
        if cfg!(windows) {
            crate::ide::command("tasklist")
                .args(["/FI", "IMAGENAME eq magoagent.exe", "/NH"])
                .output()
                .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("magoagent.exe"))
        } else {
            crate::ide::command("pgrep").args(["-x", "magoagent"]).output().is_ok_and(|o| o.status.success())
        }
    };
    if !running() {
        return;
    }
    say("Fechando o MagoAgent aberto…".into());
    if cfg!(windows) {
        let _ = crate::ide::command("taskkill").args(["/IM", "magoagent.exe"]).output();
    } else {
        let _ = crate::ide::command("pkill").args(["-x", "magoagent"]).output();
    }
    for _ in 0..50 {
        if !running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    if cfg!(windows) {
        let _ = crate::ide::command("taskkill").args(["/F", "/IM", "magoagent.exe"]).output();
    } else {
        let _ = crate::ide::command("pkill").args(["-9", "-x", "magoagent"]).output();
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            copy_dir(&path, &to.join(entry.file_name()))?;
        } else {
            fs::copy(&path, to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Everything the call uses, into `dir`: whisper.cpp (and its CUDA build with NVIDIA on Windows),
/// the small model (and large-v3-turbo with a GPU), Piper and every voice of Ajustes › Voz.
fn fetch_voice(dir: &Path, say: &dyn Fn(String)) -> Result<(), String> {
    use crate::voice::*;
    let (_, gpu) = gpu();
    if !cfg!(target_os = "macos") {
        let asset = if cfg!(windows) { "whisper-bin-x64.zip" } else { "whisper-bin-ubuntu-x64.tar.gz" };
        if whisper_server(dir, false).is_none() {
            let archive = dir.join(asset);
            download(say, "o whisper.cpp (10 MB)", &format!("{WHISPER_RELEASE}/{asset}"), &archive)?;
            unpack(&archive, &dir.join("whisper"))?;
        }
        if gpu && cfg!(windows) && whisper_server(dir, true).is_none() {
            let asset = "whisper-bin-win-cuda-12.4.0-x64.zip";
            let archive = dir.join(asset);
            download(say, "o whisper.cpp com CUDA (684 MB)", &format!("{WHISPER_RELEASE}/{asset}"), &archive)?;
            unpack(&archive, &dir.join("whisper-cuda"))?;
        }
    } else if whisper_server(dir, false).is_none() {
        say("No Mac o whisper.cpp vem do Homebrew: brew install whisper-cpp".into());
    }
    download(say, "o modelo de voz (190 MB)", &format!("{WHISPER_MODELS_URL}/{WHISPER_MODEL}"), &dir.join(WHISPER_MODEL))?;
    if gpu {
        download(say, "o modelo de voz grande, para a GPU (550 MB)", &format!("{WHISPER_MODELS_URL}/{WHISPER_TURBO}"), &dir.join(WHISPER_TURBO))?;
    }
    if find_file(&dir.join("piper"), &exe("piper")).is_none() {
        let asset = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", _) => "piper_windows_amd64.zip",
            ("macos", "aarch64") => "piper_macos_aarch64.tar.gz",
            ("macos", _) => "piper_macos_x64.tar.gz",
            (_, "aarch64") => "piper_linux_aarch64.tar.gz",
            _ => "piper_linux_x86_64.tar.gz",
        };
        let archive = dir.join(asset);
        download(say, "o Piper (22 MB)", &format!("{PIPER_RELEASE}/{asset}"), &archive)?;
        unpack(&archive, dir)?;
    }
    for name in VOICES {
        let url = voice_url(name).ok_or("voz sem endereço")?;
        let voice = dir.join("voices").join(format!("{name}.onnx"));
        download(say, &format!("a voz {} (até 110 MB)", voice_name(name)), &url, &voice)?;
        download(say, &format!("a voz {}", voice_name(name)), &format!("{url}.json"), &voice.with_extension("onnx.json"))?;
    }
    Ok(())
}

/// Fetches `url` to `to` with curl (there on the three systems), unless it is already there.
fn download(say: &dyn Fn(String), what: &str, url: &str, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Ok(());
    }
    say(format!("Baixando {what}…"));
    if let Some(dir) = to.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let part = to.with_extension("part");
    let status = crate::ide::command("curl").args(["-L", "--fail", "-sS", "-o"]).arg(&part).arg(url).status().map_err(|e| format!("curl: {e}"))?;
    if !status.success() {
        let _ = fs::remove_file(&part);
        return Err(format!("Não foi possível baixar {what}. Confira a internet e tente de novo."));
    }
    fs::rename(&part, to).map_err(|e| e.to_string())
}

/// Unpacks `archive` (zip or tar.gz, tar reads both) into `dir`.
fn unpack(archive: &Path, dir: &Path) -> Result<(), String> {
    let _ = fs::create_dir_all(dir);
    // Windows' own tar (bsdtar, which opens zips), not one a shell on the PATH may bring
    let tar = match std::env::var_os("SystemRoot").filter(|_| cfg!(windows)) {
        Some(root) => PathBuf::from(root).join("System32").join("tar.exe"),
        None => PathBuf::from("tar"),
    };
    let status = crate::ide::command(tar).arg("-xf").arg(archive).arg("-C").arg(dir).status().map_err(|e| format!("tar: {e}"))?;
    let _ = fs::remove_file(archive);
    if status.success() { Ok(()) } else { Err(format!("Não foi possível abrir {}.", archive.display())) }
}

/// MagoAgent in the system's menu: the Start menu on Windows, the applications menu on Linux.
fn shortcut(program: &Path, say: &dyn Fn(String)) {
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
    if cfg!(windows) {
        let Some(link) = var("APPDATA").map(|d| d.join(r"Microsoft\Windows\Start Menu\Programs\MagoAgent.lnk")) else { return };
        let dir = program.parent().unwrap_or(program);
        let script = format!(
            "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath = '{}'; $s.WorkingDirectory = '{}'; $s.Save()",
            link.display(),
            program.display(),
            dir.display()
        );
        let made = crate::ide::command("powershell").args(["-NoProfile", "-Command", &script]).status().is_ok_and(|s| s.success());
        say(if made { "Atalho no menu Iniciar.".into() } else { "Não foi possível pôr o atalho no menu Iniciar.".into() });
    } else if !cfg!(target_os = "macos") {
        let Some(file) = var("HOME").map(|h| h.join(".local/share/applications/magoagent.desktop")) else { return };
        let _ = fs::create_dir_all(file.parent().unwrap_or(&file));
        let entry = format!("[Desktop Entry]\nType=Application\nName=MagoAgent\nExec={}\nTerminal=false\n", program.display());
        if fs::write(&file, entry).is_ok() {
            say("Atalho no menu de aplicativos.".into());
        }
    }
}

/// The voice picked goes into the installed app's preferences.
fn save_voice(dir: &Path, voice: &str) -> Result<(), String> {
    let path = dir.join("dados").join("preferences.json");
    let mut prefs = read_prefs(&path).0.unwrap_or_default();
    prefs.voice = voice.to_string();
    let json = serde_json::to_string_pretty(&prefs).map_err(|e| e.to_string())?;
    write_atomic(&path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_installer_is_known_by_its_name_or_argument_and_names_the_voices() {
        assert!(wanted(&["instalar".into()]));
        assert!(!wanted(&["mcp".into()]));
        assert_eq!(folder_arg(&["instalar".into(), "--pasta".into(), "C:\\Program Files\\MagoAgent".into()]).as_deref(), Some("C:\\Program Files\\MagoAgent"));
        assert_eq!(folder_arg(&["instalar".into()]), None);
    }

    #[test]
    fn installing_moves_the_earlier_data_in_and_keeps_the_voice_picked() {
        use crate::voice::*;
        let root = std::env::temp_dir().join(format!("magoagent-instalar-{}", uuid::Uuid::new_v4().simple()));
        let (dir, old) = (root.join("MagoAgent"), root.join("antigo"));
        // an earlier build's data, with the voice already downloaded (empty files stand in)
        let voice = old.join("voice");
        let fake = |p: PathBuf| {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"").unwrap();
        };
        fake(old.join("autosave.mago"));
        fake(voice.join("whisper").join(exe("whisper-server")));
        fake(voice.join("whisper-cuda").join(exe("whisper-server")));
        fake(voice.join(WHISPER_MODEL));
        fake(voice.join(WHISPER_TURBO));
        fake(voice.join("piper").join(exe("piper")));
        for name in VOICES {
            fake(voice.join("voices").join(format!("{name}.onnx")));
            fake(voice.join("voices").join(format!("{name}.onnx.json")));
        }
        let said = std::cell::RefCell::new(Vec::new());
        // an earlier install's folder: its data comes along and its program goes
        let earlier_install = root.join("instalado-antes");
        fs::create_dir_all(&earlier_install).unwrap();
        fs::rename(&old, earlier_install.join("dados")).unwrap();
        fake(earlier_install.join(exe("magoagent")));
        let old = earlier_install.join("dados");
        let program = install_into(&dir, &[root.join("nao-existe"), old.clone()], &|line| said.borrow_mut().push(line)).unwrap();
        assert!(!earlier_install.exists(), "the earlier install's folder went");
        assert!(program.is_file() && dir.join("dados").join("autosave.mago").is_file() && dir.join("dados").join("projetos").is_dir());
        assert!(!old.exists(), "moved, not copied");
        assert!(said.borrow().iter().all(|l| !l.starts_with("Baixando")), "nothing to download: {:?}", said.borrow());
        save_voice(&dir, "en_US-amy-medium").unwrap();
        assert_eq!(read_prefs(&dir.join("dados").join("preferences.json")).0.unwrap().voice, "en_US-amy-medium");
        let _ = fs::remove_dir_all(&root);
    }
}
