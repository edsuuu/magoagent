//! O app no fluxo da atualização: salvar o trabalho (e conferir no disco), parar os terminais e os
//! agentes, passar a vez ao atualizador (updater.rs) e fechar; e, ao abrir de novo, dizer como acabou.

use super::updater::{self, Prepare};
use crate::prelude::*;

struct Stages<'a> {
    app: &'a mut DrawingApp,
    installer: &'a std::path::Path,
    version: &'a str,
    copy: Option<PathBuf>,
}

impl Prepare for Stages<'_> {
    fn check(&mut self) -> Result<(), String> {
        self.copy = Some(updater::copy_self()?);
        Ok(())
    }

    fn save(&mut self) -> Result<(), String> {
        self.app.save_everything().map_err(|why| format!("não foi possível salvar o trabalho ({why}); nada foi fechado e a atualização não foi feita"))
    }

    fn stop(&mut self) {
        self.app.stop_everything();
    }

    fn hand_off(&mut self) -> Result<(), String> {
        let install_dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(PathBuf::from)).ok_or("não sei onde o MagoAgent está instalado")?;
        let copy = self.copy.as_deref().ok_or("sem o atualizador")?;
        updater::launch(copy, self.installer, std::process::id(), &install_dir, self.version)
    }
}

impl DrawingApp {
    /// Salvar → parar → passar a vez ao atualizador; then the window closes (the updater waits for it).
    pub(crate) fn begin_update(&mut self, installer: &std::path::Path, release: &super::updates::Release) -> Result<(), String> {
        let mut stages = Stages { app: self, installer, version: &release.version, copy: None };
        updater::prepare(&mut stages)?;
        self.quitting = true;
        self.viewport_commands.push(egui::ViewportCommand::Close);
        Ok(())
    }

    /// Everything the app reopens from, written and read back; the first thing that can't be is the reason.
    pub(crate) fn save_everything(&mut self) -> Result<(), String> {
        let path = self.autosave.clone().ok_or("o app está sem arquivo de salvamento")?;
        self.finish_text();
        for editor in self.filetrees.editors.values_mut() {
            editor.save_all();
            if let Some(tab) = editor.tabs.iter().find(|tab| tab.dirty()) {
                return Err(format!("o editor não salvou {}", tab.path.display()));
            }
        }
        // what an AI wrote meanwhile is merged first; a drawing that can't be read is never written over
        self.sync_from_file();
        if modified(&path) != self.synced_stamp {
            return Err("o desenho salvo mudou ou não pôde ser lido".into());
        }
        let stamp = write_scene_file(&path, &self.scene).map_err(|error| format!("desenho: {error}"))?;
        self.synced = self.scene.clone();
        self.synced_stamp = stamp;
        self.dirty = false;
        let (written, _) = read_scene_file(&path).map_err(|error| format!("o desenho salvo não pôde ser relido: {error}"))?;
        if written.elements.len() != self.scene.elements.len() {
            return Err("o desenho salvo não confere com o da tela".into());
        }
        // beside the drawing, like chats.json: a test's drawing never writes in the person's data folder
        let json = serde_json::to_string_pretty(&prefs_for_file(&self.prefs, &self.saved_prefs)).map_err(|error| error.to_string())?;
        write_atomic(&path.with_file_name("preferences.json"), json).map_err(|error| format!("preferências: {error}"))?;
        self.saved_prefs = self.prefs.clone();
        self.try_save_chats().map_err(|error| format!("chats: {error}"))?;
        for chat in self.chats.iter().filter(|chat| chat.agent == "codex").map(|chat| chat.id.clone()).collect::<Vec<_>>() {
            self.save_chat_items(&chat);
        }
        self.try_save_spaces().map_err(|error| format!("espaços: {error}"))
    }

    /// The terminals' shells with everything they started (the agents in them) and the chats' agents, ended.
    pub(crate) fn stop_everything(&mut self) {
        if cfg!(windows) {
            for session in self.terminals.values().filter_map(|session| session.as_ref().ok()).filter(|session| !session.exited() && session.shell_pid != 0) {
                let _ = crate::editor::command("taskkill").args(["/T", "/F", "/PID", &session.shell_pid.to_string()]).output();
            }
        }
        // dropping a run ends its process (and what it started)
        self.chat_runs.clear();
    }

    /// When the app opens after an update: how it ended, once; and the leftovers of the update's work folder go.
    pub(crate) fn show_update_result(&mut self) {
        let Some(path) = updater::result_path().filter(|path| path.is_file()) else { return };
        let answer = fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
        let _ = fs::remove_file(&path);
        if let Some(message) = answer.and_then(|answer| updater::result_message(&answer, env!("CARGO_PKG_VERSION"))) {
            self.toast(message);
        }
        // best effort: the updater's own copy may still be closing
        std::thread::spawn(|| _ = fs::remove_dir_all(updater::work_dir()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("magoagent-flow-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn the_work_is_written_and_read_back_before_anything_stops() {
        let folder = folder("saved");
        let mut app = DrawingApp { autosave: Some(folder.join("autosave.mago")), ..Default::default() };
        let element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 40.0, height: 30.0 });
        app.scene.elements.push(element);
        app.dirty = true;
        assert_eq!(app.save_everything(), Ok(()));
        assert!(!app.dirty);
        assert_eq!(read_scene_file(&folder.join("autosave.mago")).unwrap().0.elements.len(), 1);
        assert!(folder.join("chats.json").is_file() && folder.join("espacos.json").is_file() && folder.join("preferences.json").is_file());
        let _ = fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_save_that_fails_says_why_and_a_drawing_it_cannot_read_is_left_alone() {
        let folder = folder("failed");
        // the folder of the drawing is a file: nothing can be written
        let blocker = folder.join("arquivo");
        fs::write(&blocker, "x").unwrap();
        let mut app = DrawingApp { autosave: Some(blocker.join("autosave.mago")), ..Default::default() };
        assert!(app.save_everything().is_err());
        // a drawing on disk that is not one: not written over
        let spoiled = folder.join("estragado.mago");
        fs::write(&spoiled, "isto não é um desenho").unwrap();
        let mut app = DrawingApp { autosave: Some(spoiled.clone()), ..Default::default() };
        assert!(app.save_everything().is_err());
        assert_eq!(fs::read_to_string(&spoiled).unwrap(), "isto não é um desenho");
        assert!(DrawingApp::default().save_everything().is_err(), "no file to save to");
        let _ = fs::remove_dir_all(&folder);
    }
}
