//! File operations from the project tree: name checks, and sending to the system's trash.

use super::*;
use std::io;

/// Why `name` can't name a file or folder on `os` (as `std::env::consts::OS` says it), in the
/// words the tree shows; None when it can. An empty name is the caller's to refuse.
pub(crate) fn name_problem(name: &str, os: &str) -> Option<&'static str> {
    if os == "linux" {
        if name.contains('/') {
            return Some("Um nome não pode conter /");
        }
    } else if name.contains(['\\', '/', ':', '<', '>', '"', '|', '?', '*']) {
        return Some("Um nome não pode conter \\ / : < > \" | ? *");
    }
    if !name.is_empty() && name.chars().all(|c| c == '.') {
        return Some("Um nome não pode ter só pontos.");
    }
    // CON, NUL, COM1… are devices on Windows, with any extension
    let stem = name.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
    let device = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    (os == "windows" && device).then_some("O Windows reserva esse nome para um dispositivo.")
}

/// Sends `path` (a file or a whole folder) to the system's trash. Nothing is deleted for good
/// when the trash refuses it: the error comes back instead.
pub(crate) fn trash(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct FileOp {
            hwnd: isize,
            func: u32,
            from: *const u16,
            to: *const u16,
            flags: u16,
            aborted: i32,
            mappings: *mut std::ffi::c_void,
            title: *const u16,
        }
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn SHFileOperationW(op: *mut FileOp) -> i32;
        }
        use std::os::windows::ffi::OsStrExt;
        // a list of paths: each one ends in a NUL, the list in another
        let from: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
        const FO_DELETE: u32 = 3;
        // FOF_SILENT | FOF_NOCONFIRMATION | FOF_ALLOWUNDO (the Recycle Bin) | FOF_NOERRORUI |
        // FOF_WANTNUKEWARNING: what the bin can't take is asked about, never deleted unasked
        const FLAGS: u16 = 0x4 | 0x10 | 0x40 | 0x400 | 0x4000;
        let mut op = FileOp {
            hwnd: 0,
            func: FO_DELETE,
            from: from.as_ptr(),
            to: std::ptr::null(),
            flags: FLAGS,
            aborted: 0,
            mappings: std::ptr::null_mut(),
            title: std::ptr::null(),
        };
        // SAFETY: `from` is double-NUL-terminated and outlives the call; the rest is null or plain
        match unsafe { SHFileOperationW(&mut op) } {
            0 if op.aborted == 0 => Ok(()),
            0 => Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado")),
            code => Err(io::Error::other(format!("a Lixeira recusou (código {code:#x})"))),
        }
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSFileManager, NSString, NSURL};
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&url, None).map_err(|e| io::Error::other(e.localizedDescription().to_string()))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        freedesktop_trash(path)
    }
}

/// The freedesktop.org trash in the home folder: the item moves to `Trash/files`, with a
/// `.trashinfo` beside it in `Trash/info` saying where it came from. Something on another disk
/// can't be moved there and is refused.
// ponytail: only the home trash (no `$topdir/.Trash-$uid` per disk), and the date is UTC where the
// spec asks for local time; add both if a desktop ever misplaces what it restores
#[cfg(not(any(windows, target_os = "macos")))]
fn freedesktop_trash(path: &Path) -> io::Result<()> {
    use std::io::Write;
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    let data = var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local/share"))).ok_or_else(|| io::Error::other("sem pasta pessoal"))?;
    let (files, info) = (data.join("Trash/files"), data.join("Trash/info"));
    std::fs::create_dir_all(&files)?;
    std::fs::create_dir_all(&info)?;
    let path = std::path::absolute(path)?;
    let name = path.file_name().ok_or_else(|| io::Error::other("sem nome"))?.to_string_lossy().into_owned();
    // the first free name: a.txt, a.txt.2, a.txt.3…; the info file is claimed first, as the spec says
    let (kept, mut file) = (1..)
        .map(|n| if n == 1 { name.clone() } else { format!("{name}.{n}") })
        .find_map(|n| std::fs::File::create_new(info.join(format!("{n}.trashinfo"))).ok().map(|f| (n, f)))
        .expect("an endless range");
    let encoded: String = path
        .to_string_lossy()
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect();
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // days since 1970 to a calendar date (Howard Hinnant's civil_from_days)
    let z = days + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (day, month) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let year = yoe + era * 400 + i64::from(month <= 2);
    let date = format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}", rest / 3600, rest / 60 % 60, rest % 60);
    let written = write!(file, "[Trash Info]\nPath={encoded}\nDeletionDate={date}\n");
    let moved = written.and_then(|_| std::fs::rename(&path, files.join(&kept)));
    if moved.is_err() {
        let _ = std::fs::remove_file(info.join(format!("{kept}.trashinfo")));
    }
    moved
}

/// A pick from a tree row's right-click menu.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum FileAction {
    NewFile,
    NewFolder,
    Rename,
    Trash,
    Delete,
    CopyPath,
    CopyRelative,
    Reveal,
    Refresh,
}

impl FileAction {
    /// Whether the project's own folder offers it: the root is not renamed or removed from here.
    pub(crate) fn on_root(self) -> bool {
        !matches!(self, FileAction::Rename | FileAction::Trash | FileAction::Delete)
    }
}

/// Opening a folder in the system's file manager, as each system words it.
pub(crate) const REVEAL: &str = if cfg!(windows) {
    "Mostrar no Explorador"
} else if cfg!(target_os = "macos") {
    "Abrir no Finder"
} else {
    "Mostrar no Gerenciador de Arquivos"
};

/// A tree row's right-click menu, in groups, as the card's menu and the IDE's tree word it.
pub(crate) const FILE_MENU: [&[(FileAction, &str)]; 4] = {
    use FileAction::*;
    [
        &[(NewFile, "Novo Arquivo"), (NewFolder, "Nova Pasta")],
        &[(Rename, "Renomear"), (Trash, "Mover para a Lixeira"), (Delete, "Excluir Permanentemente")],
        &[(CopyPath, "Copiar Caminho"), (CopyRelative, "Copiar Caminho Relativo"), (Reveal, REVEAL)],
        &[(Refresh, "Atualizar")],
    ]
};

/// The right-click menu of a row in the enlarged IDE's tree (`root`: the project's own folder);
/// the pick, once made.
pub(crate) fn file_menu(ui: &mut Ui, root: bool) -> Option<FileAction> {
    let mut pick = None;
    for (n, group) in FILE_MENU.into_iter().enumerate() {
        if n > 0 {
            ui.separator();
        }
        for &(action, label) in group {
            if ui.add_enabled(!root || action.on_root(), egui::Button::new(label)).clicked() {
                pick = Some(action);
                ui.close();
            }
        }
    }
    pick
}

/// What the tree asks before a file operation, in a small window over the IDE or its card.
pub(crate) enum FileAsk {
    /// A name: for a new file (`Some(false)`) or folder (`Some(true)`) in the folder `at`, or the
    /// new name of `at` (None).
    Name { at: PathBuf, folder: Option<bool>, text: String },
    /// Send `at` to the trash, or delete it for good, once confirmed; `dont_ask`: the trash no
    /// longer asks.
    Remove { at: PathBuf, forever: bool, dont_ask: bool },
}

impl Ide {
    /// Acts on a pick from the menu of the row `path`: asks first for what changes the disk.
    pub(crate) fn file_action(&mut self, path: &Path, action: FileAction) {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        // what's made from a file's menu goes beside it
        let folder = if path.is_dir() { path } else { path.parent().unwrap_or(path) }.to_path_buf();
        match action {
            FileAction::NewFile => self.file_ask = Some(FileAsk::Name { at: folder, folder: Some(false), text: "Sem título".into() }),
            FileAction::NewFolder => self.file_ask = Some(FileAsk::Name { at: folder, folder: Some(true), text: "Nova Pasta".into() }),
            FileAction::Rename => self.file_ask = Some(FileAsk::Name { at: path.to_path_buf(), folder: None, text: name }),
            FileAction::Trash if !self.confirm_trash => self.file_answer(FileAsk::Remove { at: path.to_path_buf(), forever: false, dont_ask: false }),
            FileAction::Trash => self.file_ask = Some(FileAsk::Remove { at: path.to_path_buf(), forever: false, dont_ask: false }),
            FileAction::Delete => self.file_ask = Some(FileAsk::Remove { at: path.to_path_buf(), forever: true, dont_ask: false }),
            FileAction::CopyPath => self.ctx.copy_text(path.display().to_string()),
            FileAction::CopyRelative => {
                let relative = self.root.as_deref().and_then(|root| path.strip_prefix(root).ok()).unwrap_or(path);
                self.ctx.copy_text(relative.display().to_string());
            }
            // ponytail: opens the folder, without picking the file in it; `explorer /select,` or
            // `open -R` would, per system
            FileAction::Reveal => crate::app::portal::open_external(&folder.to_string_lossy()),
            FileAction::Refresh => {
                self.sync_tree();
                self.status = "Atualizado.".into();
            }
        }
    }

    /// Makes a file or folder `name` in `dir`, shows it in the tree and opens a file.
    pub(crate) fn create(&mut self, dir: &Path, name: &str, folder: bool) -> Result<PathBuf, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Nada foi criado.".into());
        }
        if let Some(why) = name_problem(name, std::env::consts::OS) {
            return Err(why.into());
        }
        let path = dir.join(name);
        let made = if folder { std::fs::create_dir(&path) } else { std::fs::File::create_new(&path).map(drop) };
        made.map_err(|e| format!("Não foi possível criar isso: {e}"))?;
        self.sync_tree();
        self.card_reveal(&path);
        if !folder {
            self.open_file(path.clone(), None);
        }
        self.status = (if folder { "Pasta criada." } else { "Arquivo criado." }).into();
        Ok(path)
    }

    /// Renames `path` to `name` in its folder; its open tabs follow. Another file of that name is
    /// never replaced (a change of case only is the same file on Windows and macOS).
    pub(crate) fn rename_path(&mut self, path: &Path, name: &str) -> Result<PathBuf, String> {
        let name = name.trim();
        if name.is_empty() || path.file_name() == Some(std::ffi::OsStr::new(name)) {
            return Err("Nada foi renomeado.".into());
        }
        if let Some(why) = name_problem(name, std::env::consts::OS) {
            return Err(why.into());
        }
        let to = path.with_file_name(name);
        if to.exists() && std::fs::canonicalize(&to).ok() != std::fs::canonicalize(path).ok() {
            return Err(format!("Já existe {name} nesta pasta."));
        }
        std::fs::rename(path, &to).map_err(|e| format!("Não foi possível renomear isso: {e}"))?;
        for tab in &mut self.tabs {
            if let Ok(rest) = tab.path.strip_prefix(path) {
                tab.path = if rest.as_os_str().is_empty() { to.clone() } else { to.join(rest) };
            }
        }
        self.sync_tree();
        self.card_reveal(&to);
        self.status = "Renomeado.".into();
        Ok(to)
    }

    /// Sends `path` to the trash (`forever` deletes it instead). Clean tabs of what went close; a
    /// tab with unsaved edits stays, marked as gone, and Ctrl+S writes the file back.
    pub(crate) fn remove_path(&mut self, path: &Path, forever: bool) -> Result<(), String> {
        let done = match forever {
            true if path.is_dir() => std::fs::remove_dir_all(path),
            true => std::fs::remove_file(path),
            false => trash(path),
        };
        done.map_err(|e| match forever {
            true => format!("Não foi possível excluir isso: {e}"),
            false => format!("Nada foi excluído: {e}. O MagoAgent não apaga um arquivo de vez quando a lixeira o recusa."),
        })?;
        for i in (0..self.tabs.len()).rev() {
            if !self.tabs[i].dirty && self.tabs[i].path.starts_with(path) {
                self.close_tab(i);
            }
        }
        self.sync_tree();
        self.status = (if forever { "1 item excluído." } else { "1 item movido para a lixeira." }).into();
        Ok(())
    }

    /// The open question's window; Enter answers it, Esc or a click outside drops it.
    pub(crate) fn file_ask_ui(&mut self, ctx: &egui::Context) {
        let Some(ask) = &mut self.file_ask else { return };
        let mut answer = None;
        let modal = egui::Modal::new(self.id.with("file_ask")).show(ctx, |ui| {
            t::style(ui.style_mut());
            ui.set_width(340.0);
            match ask {
                FileAsk::Name { at, folder, text } => {
                    let what = match folder {
                        Some(true) => "Nome da nova pasta".to_string(),
                        Some(false) => "Nome do novo arquivo".to_string(),
                        None => format!("Novo nome para {}", at.file_name().unwrap_or_default().to_string_lossy()),
                    };
                    ui.label(what);
                    let field_id = self.id.with("file_ask_name");
                    let field = egui::TextEdit::singleline(text).id(field_id).desired_width(f32::INFINITY).show(ui);
                    if !field.response.has_focus() {
                        field.response.request_focus();
                        // the name ready to type over; a file's extension stays out of the selection
                        let dot = text.rfind('.').filter(|&i| i > 0 && folder.is_none() && !at.is_dir());
                        let end = dot.map_or(text.chars().count(), |i| text[..i].chars().count());
                        let mut state = field.state;
                        let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(end));
                        state.cursor.set_char_range(Some(range));
                        state.store(ui.ctx(), field_id);
                    }
                    let problem = name_problem(text.trim(), std::env::consts::OS);
                    if let Some(why) = problem {
                        ui.colored_label(t::ERROR, why);
                    }
                    let ok = problem.is_none() && !text.trim().is_empty();
                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let label = if folder.is_some() { "Criar" } else { "Renomear" };
                        if ui.add_enabled(ok, egui::Button::new(label)).clicked() || (ok && ui.input(|i| i.key_pressed(Key::Enter))) {
                            answer = Some(true);
                        }
                        if ui.button("Cancelar").clicked() {
                            answer = Some(false);
                        }
                    });
                }
                FileAsk::Remove { at, forever, dont_ask } => {
                    let name = at.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    let (title, body, label) = if *forever {
                        ("Excluir permanentemente?", format!("{name} será excluído de vez e não poderá ser recuperado."), "Excluir 1 Item")
                    } else {
                        ("Mover para a Lixeira?", format!("{name} será movido para a lixeira. Você pode restaurá-lo de lá."), "Mover para a Lixeira")
                    };
                    ui.label(egui::RichText::new(title).strong());
                    ui.label(body);
                    if !*forever {
                        ui.checkbox(dont_ask, "Não perguntar novamente");
                    }
                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(label).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                            answer = Some(true);
                        }
                        if ui.button("Cancelar").clicked() {
                            answer = Some(false);
                        }
                    });
                }
            }
        });
        if modal.should_close() {
            answer.get_or_insert(false);
        }
        let Some(go) = answer else { return };
        let Some(ask) = self.file_ask.take() else { return };
        if go {
            self.file_answer(ask);
        }
    }

    /// Does what a question was answered yes to, and says how it went.
    fn file_answer(&mut self, ask: FileAsk) {
        let done = match ask {
            FileAsk::Name { at, folder: Some(folder), text } => self.create(&at, &text, folder).map(drop),
            FileAsk::Name { at, folder: None, text } => self.rename_path(&at, &text).map(drop),
            FileAsk::Remove { at, forever, dont_ask } => {
                if dont_ask {
                    (self.confirm_trash, self.dont_ask_trash) = (false, true);
                }
                self.remove_path(&at, forever)
            }
        };
        let message = done.map_or_else(|why| why, |()| self.status.clone());
        self.notify(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_each_systems_rules() {
        assert_eq!(name_problem("main.rs", "windows"), None);
        assert_eq!(name_problem("a:b", "macos"), Some("Um nome não pode conter \\ / : < > \" | ? *"));
        assert_eq!(name_problem("a:b", "linux"), None);
        assert_eq!(name_problem("a/b", "linux"), Some("Um nome não pode conter /"));
        assert_eq!(name_problem("..", "linux"), Some("Um nome não pode ter só pontos."));
        for device in ["con", "NUL.txt", "com1", "Lpt9.log"] {
            assert_eq!(name_problem(device, "windows"), Some("O Windows reserva esse nome para um dispositivo."), "{device}");
            assert_eq!(name_problem(device, "linux"), None);
        }
        assert_eq!(name_problem("com10", "windows"), None);
        assert_eq!(name_problem("console.txt", "windows"), None);
    }

    #[test]
    fn not_asking_again_is_kept_when_the_answer_is_yes() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-dontask-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "").unwrap();
        let mut ide = Ide::new(egui::Context::default(), "n", &dir.join("nada").to_string_lossy());
        // the trash itself stays out of tests: the same answer, deleting for good
        ide.file_answer(FileAsk::Remove { at: dir.join("a.txt"), forever: true, dont_ask: true });
        assert!(!dir.join("a.txt").exists());
        assert!(!ide.confirm_trash && ide.dont_ask_trash);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_tree_makes_renames_and_deletes_files() {
        let dir = std::env::temp_dir().join(format!("magoagent-ide-fileops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut ide = Ide::new(egui::Context::default(), "f", &dir.to_string_lossy());
        // a folder, then a file in it: opened in a tab and shown in the tree
        let src = ide.create(&dir, "src", true).unwrap();
        let file = ide.create(&src, " main.rs ", false).unwrap();
        assert_eq!(file, src.join("main.rs"));
        assert_eq!(ide.tabs[ide.active].path, file);
        assert!(ide.card_rows().iter().any(|r| r.1 == "main.rs"));
        assert!(ide.create(&src, "main.rs", false).is_err(), "never over a file");
        assert!(ide.create(&src, "a/b", false).is_err());
        // renamed: the tab of what was inside follows; never over another file
        std::fs::write(dir.join("other.rs"), "").unwrap();
        let lib = ide.rename_path(&src, "lib").unwrap();
        assert_eq!(ide.tabs[ide.active].path, lib.join("main.rs"));
        assert!(ide.rename_path(&dir.join("other.rs"), "lib").is_err());
        assert!(dir.join("other.rs").exists());
        // deleted for good (the trash is the user's, never a test's): its clean tab closes
        ide.remove_path(&lib, true).unwrap();
        assert!(!lib.exists());
        assert!(ide.tabs.iter().all(|t| !t.path.starts_with(&lib)));
        assert!(!ide.card_rows().iter().any(|r| r.1 == "lib"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
