//! The installer: the same program, run as `MagoAgent-Instalador` (or `magoagent instalar`). The
//! person picks the folder (C:\Program Files\MagoAgent by default, like Steam; Windows asks for
//! the administrator there, and the folder is then given to the person so the app can write its
//! data in it). It closes a MagoAgent that is open, copies itself into the folder, moves the data
//! of an earlier install or build into `data`, records the folder (`record_install_dir`) and the
//! version (`versao`) and adds MagoAgent to the system's menu. Found installed, it offers to update
//! it. Nothing is downloaded, neither now nor later when a feature is used.
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

/// This installer's version.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The version installed in `dir`: its `versao` file; "anterior" for an install from before it.
fn installed_version(dir: &Path) -> Option<String> {
    dir.join(crate::voice::exe("magoagent")).is_file().then(|| fs::read_to_string(dir.join("versao")).map_or("anterior".into(), |v| v.trim().to_string()))
}

/// "0.0.2" → (0, 0, 2), to tell an update from a downgrade; "anterior" is older than any.
pub(crate) fn version_key(version: &str) -> (u32, u32, u32) {
    let mut parts = version.split('.').map(|p| p.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

pub(crate) fn run(args: &[String]) -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title(format!("Instalar o {} {VERSION}", crate::model::APP_TITLE))
        .with_inner_size([640.0, 600.0])
        .with_resizable(false)
        .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icon.png")).expect("assets/icon.png is a png"));
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
}

impl Default for Installer {
    fn default() -> Self {
        let dir = recorded_install_dir().or_else(default_install_dir).map(|d| d.display().to_string()).unwrap_or_default();
        Installer { step: Step::Start, dir, lines: Vec::new() }
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
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::model::APP_TITLE).size(26.0).strong());
                ui.label(egui::RichText::new(VERSION).size(15.0).color(Color32::GRAY));
            });
            ui.add_space(6.0);
            let dir = self.dir();
            let installed = installed_version(&dir);
            match &mut self.step {
                Step::Start => {
                    if let Some(old) = &installed {
                        let text = match version_key(old).cmp(&version_key(VERSION)) {
                            _ if old == "anterior" => format!("Já há um MagoAgent instalado nessa pasta. Atualizar para a versão {VERSION}?"),
                            std::cmp::Ordering::Less => format!("A versão {old} está instalada nessa pasta. Atualizar para a {VERSION}?"),
                            std::cmp::Ordering::Equal => format!("A versão {VERSION} já está instalada nessa pasta."),
                            std::cmp::Ordering::Greater => format!("Uma versão mais nova ({old}) está instalada nessa pasta; esta é a {VERSION}."),
                        };
                        ui.label(egui::RichText::new(text).size(15.0).strong());
                        ui.label(egui::RichText::new("Os desenhos, os chats e os ajustes ficam como estão.").size(14.0).color(Color32::GRAY));
                        ui.add_space(12.0);
                    }
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
                    ui.label(egui::RichText::new("Tudo do app fica nessa pasta: o programa, os desenhos e os chats.").size(14.0).color(Color32::GRAY));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Se o MagoAgent estiver aberto, ele é fechado (o desenho já está salvo).").size(14.0).color(Color32::GRAY));
                    ui.add_space(24.0);
                    let label = if installed.is_some() { "Atualizar" } else { "Instalar" };
                    if big(ui, label).clicked() && !self.dir.trim().is_empty() {
                        if cfg!(windows) {
                            // always as administrator (the owner's choice): Windows asks, and that
                            // run installs and gives the folder to the person
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
                            Msg::Done => next = Some(Step::Done),
                            Msg::Failed(e) => next = Some(Step::Failed(e)),
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(egui::RichText::new(format!("Instalando em {}…", dir.display())).size(15.0));
                    });
                    ui.add_space(8.0);
                    lines(ui, &self.lines);
                    ctx.request_repaint_after(std::time::Duration::from_millis(500));
                    if let Some(step) = next {
                        self.step = step;
                    }
                }
                Step::Done => {
                    ui.label(egui::RichText::new(format!("Pronto: o MagoAgent {VERSION} está instalado.")).size(17.0).strong());
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
                    lines(ui, &self.lines);
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

fn lines(ui: &mut egui::Ui, lines: &[String]) {
    egui::ScrollArea::vertical().max_height(330.0).stick_to_bottom(true).show(ui, |ui| {
        for line in lines {
            ui.label(egui::RichText::new(line).size(14.0).monospace());
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

/// A path inside a single-quoted PowerShell string.
fn ps_quote(path: &Path) -> String {
    path.display().to_string().replace('\'', "''")
}

/// The installer again, as administrator (Windows asks), installing into `dir`.
fn elevate(dir: &Path) -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let script = format!("Start-Process -FilePath '{}' -ArgumentList 'instalar --pasta \"{}\"' -Verb RunAs", ps_quote(&me), ps_quote(dir));
    let ok = crate::editor::command("powershell").args(["-NoProfile", "-Command", &script]).status().is_ok_and(|s| s.success());
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

/// The whole install into `dir`, saying each step.
fn install(dir: &Path, say: &dyn Fn(String)) -> Result<(), String> {
    close_running(say);
    // the data of an earlier install (wherever it was) or of a build from before the installer
    let earlier: Vec<PathBuf> = [recorded_install_dir(), old_install_dir()]
        .into_iter()
        .flatten()
        .filter(|d| d != dir)
        .map(|d| {
            crate::file::migrate_data(&d);
            d.join("data")
        })
        .chain(legacy_data_dir())
        .collect();
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

/// The program and the data (the first of `earlier` that exists) into `dir`; gives
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
    crate::file::migrate_data(dir);
    let data = dir.join("data");
    if !data.is_dir() {
        match earlier.iter().find(|d| d.is_dir()) {
            Some(old) => {
                say(format!("Trazendo os dados de {}…", old.display()));
                if fs::rename(old, &data).is_err() {
                    copy_dir(old, &data).map_err(|e| format!("Não foi possível trazer os dados: {e}"))?;
                    let _ = fs::remove_dir_all(old);
                }
                // an earlier install's folder keeps only its program: it goes, so one copy is left
                if old.file_name().is_some_and(|n| n == "data" || n == "dados")
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
    fs::write(dir.join("versao"), VERSION).map_err(|e| e.to_string())?;
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
    let ok = crate::editor::command("icacls").arg(dir).args(["/grant", &format!("{user}:(OI)(CI)M"), "/Q"]).status().is_ok_and(|s| s.success());
    // an update runs as the person, in a folder already theirs: changing its permissions takes the
    // administrator, but there is nothing to give
    if !ok && !writable(dir) {
        say("Não foi possível liberar a pasta para o seu usuário.".into());
    }
}

/// A MagoAgent that is open is asked to close (it saves), then made to. Never this very process (it may
/// be `magoagent instalar`); the agents' `magoagent mcp` go too, as Windows won't replace a running binary.
fn close_running(say: &dyn Fn(String)) {
    let me = std::process::id().to_string();
    let others = || -> Vec<String> {
        crate::editor::command("pgrep")
            .args(["-x", "magoagent"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().filter(|pid| *pid != me).map(String::from).collect())
            .unwrap_or_default()
    };
    let running = || {
        if cfg!(windows) {
            crate::editor::command("tasklist")
                .args(["/FI", "IMAGENAME eq magoagent.exe", "/FI", &format!("PID ne {me}"), "/NH"])
                .output()
                .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("magoagent.exe"))
        } else {
            !others().is_empty()
        }
    };
    let kill = |force: bool| {
        if cfg!(windows) {
            let mut command = crate::editor::command("taskkill");
            command.args(["/IM", "magoagent.exe", "/FI", &format!("PID ne {me}")]).args(force.then_some("/F"));
            let _ = command.output();
        } else {
            for pid in others() {
                let _ = crate::editor::command("kill").args(force.then_some("-9")).arg(&pid).output();
            }
        }
    };
    if !running() {
        return;
    }
    say("Fechando o MagoAgent aberto…".into());
    kill(false);
    for _ in 0..50 {
        if !running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    kill(true);
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

/// MagoAgent in the system's menu: the Start menu on Windows, the applications menu on Linux.
fn shortcut(program: &Path, say: &dyn Fn(String)) {
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
    if cfg!(windows) {
        let Some(link) = var("APPDATA").map(|d| d.join(r"Microsoft\Windows\Start Menu\Programs\MagoAgent.lnk")) else { return };
        let dir = program.parent().unwrap_or(program);
        // the program carries no icon of its own: the shortcut takes the logo from a file beside it
        let icon = dir.join("magoagent.ico");
        let _ = fs::write(&icon, include_bytes!("../../assets/icon.ico"));
        let script = format!(
            "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath = '{}'; $s.WorkingDirectory = '{}'; $s.IconLocation = '{},0'; $s.Save()",
            ps_quote(&link),
            ps_quote(program),
            ps_quote(dir),
            ps_quote(&icon)
        );
        let made = crate::editor::command("powershell").args(["-NoProfile", "-Command", &script]).status().is_ok_and(|s| s.success());
        say(if made { "Atalho no menu Iniciar.".into() } else { "Não foi possível pôr o atalho no menu Iniciar.".into() });
    } else if !cfg!(target_os = "macos") {
        let Some(file) = var("HOME").map(|h| h.join(".local/share/applications/magoagent.desktop")) else { return };
        let _ = fs::create_dir_all(file.parent().unwrap_or(&file));
        let icon = program.with_file_name("magoagent.png");
        let _ = fs::write(&icon, include_bytes!("../../assets/icon.png"));
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName={}\nExec={}\nIcon={}\nTerminal=false\n",
            crate::model::APP_TITLE,
            program.display(),
            icon.display()
        );
        if fs::write(&file, entry).is_ok() {
            say("Atalho no menu de aplicativos.".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_installer_is_known_by_its_name_or_argument_and_() {
        assert!(wanted(&["instalar".into()]));
        assert!(!wanted(&["mcp".into()]));
        assert_eq!(folder_arg(&["instalar".into(), "--pasta".into(), "C:\\Program Files\\MagoAgent".into()]).as_deref(), Some("C:\\Program Files\\MagoAgent"));
        assert_eq!(folder_arg(&["instalar".into()]), None);
        assert!(version_key("anterior") < version_key("0.0.1") && version_key("0.0.9") < version_key("0.1.0"));
        assert_eq!(ps_quote(Path::new("C:/Users/O'Neil")), "C:/Users/O''Neil");
    }

    #[test]
    fn installing_moves_the_earlier_data_in() {
        use crate::voice::exe;
        let root = std::env::temp_dir().join(format!("magoagent-instalar-{}", uuid::Uuid::new_v4().simple()));
        let (dir, old) = (root.join("MagoAgent"), root.join("antigo"));
        // an earlier build's data (empty files stand in)
        let fake = |p: PathBuf| {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"").unwrap();
        };
        fake(old.join("autosave.mago"));
        let said = std::cell::RefCell::new(Vec::new());
        // an earlier install's folder: its data comes along and its program goes
        let earlier_install = root.join("instalado-antes");
        fs::create_dir_all(&earlier_install).unwrap();
        fs::rename(&old, earlier_install.join("data")).unwrap();
        fake(earlier_install.join(exe("magoagent")));
        let old = earlier_install.join("data");
        let program = install_into(&dir, &[root.join("nao-existe"), old.clone()], &|line| said.borrow_mut().push(line)).unwrap();
        assert_eq!(installed_version(&dir).as_deref(), Some(VERSION));
        assert!(!earlier_install.exists(), "the earlier install's folder went");
        assert!(program.is_file() && dir.join("data").join("autosave.mago").is_file() && dir.join("data").join("projetos").is_dir());
        assert!(!old.exists(), "moved, not copied");
        assert!(said.borrow().iter().all(|l| !l.starts_with("Baixando")), "nothing to download: {:?}", said.borrow());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_old_dados_folder_becomes_data() {
        let root = std::env::temp_dir().join(format!("magoagent-migrar-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(root.join("dados")).unwrap();
        fs::write(root.join("dados").join("autosave.mago"), b"x").unwrap();
        crate::file::migrate_data(&root);
        assert!(root.join("data").join("autosave.mago").is_file() && !root.join("dados").exists());
        // with both there, nothing is touched
        fs::create_dir_all(root.join("dados")).unwrap();
        crate::file::migrate_data(&root);
        assert!(root.join("dados").is_dir());
        let _ = fs::remove_dir_all(&root);
    }
}
