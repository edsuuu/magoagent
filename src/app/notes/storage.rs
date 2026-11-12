//! A note's Desfocar (the text behind a veil) and Mover Arquivo para… (its `.md` goes to a folder of
//! the user's choosing, or back to `notas/` beside the scene). Both are rows of the note's menu and
//! actions of the MCP's `note_stack`.

use crate::prelude::*;
use std::path::{Path, PathBuf};

/// A file name in `dir` for the note `text`: its name, then `-2`, `-3`… while the name is taken.
fn free_name(dir: &Path, text: &str, id: &str) -> PathBuf {
    let file = crate::app::maestro::binder::note_file(&note_name(text)).unwrap_or_else(|| format!("{id}.md"));
    let stem = file.trim_end_matches(".md").to_string();
    (1..).map(|n| dir.join(if n == 1 { file.clone() } else { format!("{stem}-{n}.md") })).find(|p| !p.exists()).unwrap_or_default()
}

impl DrawingApp {
    /// Desfocar / Remover Desfoque.
    pub(crate) fn set_note_blur(&mut self, note: usize, on: bool) {
        if self.scene.elements[note].note.blurred != on {
            self.snapshot();
            self.scene.elements[note].note.blurred = on;
        }
    }

    /// Mover Arquivo para…: the note's `.md` goes to `folder`; `None` brings it back beside the scene.
    pub(crate) fn move_note_file(&mut self, note: usize, folder: Option<&Path>) -> Result<String, String> {
        let scene = self.autosave.clone().ok_or("Sem pasta de dados: a nota não tem arquivo para mover.")?;
        let element = &self.scene.elements[note];
        let (old, text) = (crate::file::note_file(&scene, element), self.note_text(note));
        let new = match folder {
            Some(dir) => free_name(dir, &text, &element.id),
            None => scene.with_file_name("notas").join(format!("{}.md", element.id)),
        };
        let fail = |e: std::io::Error| format!("Não foi possível mover o arquivo da nota: {e}");
        if old != new {
            std::fs::create_dir_all(new.parent().unwrap_or(Path::new("."))).map_err(fail)?;
            crate::file::write_atomic(&new, &text).map_err(fail)?;
            let _ = std::fs::remove_file(&old);
        }
        self.snapshot();
        self.scene.elements[note].note.file = folder.map(|_| new.to_string_lossy().into_owned());
        Ok(match folder {
            Some(_) => format!("Arquivo da nota em {}.", new.display()),
            None => "Arquivo da nota de volta na pasta do app.".into(),
        })
    }

    /// The menu's Mover Arquivo para…: asks for the folder (a note already moved goes back instead).
    pub(crate) fn move_note_file_ui(&mut self, note: usize) {
        let result = if self.scene.elements[note].note.file.is_some() {
            self.move_note_file(note, None)
        } else {
            match rfd::FileDialog::new().set_title("Mover arquivo da nota para").pick_folder() {
                Some(dir) => self.move_note_file(note, Some(&dir)),
                None => return,
            }
        };
        if let Err(e) = result {
            self.toast(e);
        }
    }

    /// The MCP's `note_stack` actions `blur` (`value`: on/off, default toggles) and `move_file` (`folder`,
    /// empty brings it back).
    pub(crate) fn act_note_file(&mut self, action: &str, name: &str, args: &Value) -> Result<String, String> {
        let note = self.node_named(name)?;
        if self.scene.elements[note].created.is_none() {
            return Err(format!("{name} não é uma nota"));
        }
        if action == "blur" {
            let on = match args["value"].as_str().map(|s| s.to_lowercase()) {
                Some(v) if ["off", "desligar", "false"].contains(&v.as_str()) => false,
                Some(v) if ["on", "ligar", "true"].contains(&v.as_str()) => true,
                _ => args["value"].as_bool().unwrap_or(!self.scene.elements[note].note.blurred),
            };
            self.set_note_blur(note, on);
            return Ok(if on { "Nota desfocada." } else { "Nota sem desfoque." }.into());
        }
        match args["folder"].as_str().map(str::trim).filter(|f| !f.is_empty()) {
            Some(folder) if !Path::new(folder).is_dir() => Err(format!("{folder} não é uma pasta")),
            folder => self.move_note_file(note, folder.map(Path::new)),
        }
    }
}
