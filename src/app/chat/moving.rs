//! `set_folder` on a conversation already going: it moves to the new folder with its context. Claude Code files a
//! session by the folder it runs in, so its file is copied to the new folder's place (the old one stays) and the next
//! message resumes it there; Codex keeps its sessions by id, in one place. The move waits for the answer running now
//! (the agent asked for it mid-turn, and that turn still writes to the old file). The chat's card goes to the
//! workspace of the new folder.

use crate::app::chat::direct::chat_name;
use crate::prelude::*;
use crate::transcript::project_folder;
use std::path::Path;

/// Where Claude Code files session `session` once it runs in `cwd`, beside `old` (its file now): the same `projects`
/// folder, on the side that has it (Windows, or a WSL distro, which names the folder as Linux does).
fn claude_copy(old: &Path, cwd: &str, session: &str) -> Option<PathBuf> {
    let projects = old.parent()?.parent()?;
    let native = crate::transcript::claude_dir().is_some_and(|dir| old.starts_with(dir));
    let name = if native || !cfg!(windows) { project_folder(cwd) } else { project_folder(&crate::app::system::wsl::linux_path(cwd)) };

    Some(projects.join(name).join(format!("{session}.jsonl")))
}

impl DrawingApp {
    /// Chat `id` moves to folder `path` once it answers nothing (`apply_chat_moves`); what the agent hears now.
    pub(crate) fn move_chat(&mut self, id: &str, path: &str) -> String {
        self.chat_moves.retain(|(chat, _)| chat != id);
        self.chat_moves.push((id.to_string(), path.to_string()));

        if self.chat_runs.contains_key(id) {
            return format!("Esta conversa passa para {path} assim que esta resposta terminar, com todo o contexto: a próxima mensagem já roda lá.");
        }

        self.apply_chat_moves();
        format!("Esta conversa agora trabalha em {path}, com todo o contexto.")
    }

    /// Each frame: the moves of chats that answer nothing now.
    pub(crate) fn apply_chat_moves(&mut self) {
        let runs = &self.chat_runs;
        let ready: Vec<(String, String)> = self.chat_moves.extract_if(.., |(id, _)| !runs.contains_key(id)).collect();

        for (id, path) in ready {
            if let Err(why) = self.move_chat_now(&id, &path) {
                self.toast(why);
            }
        }
    }

    fn move_chat_now(&mut self, id: &str, path: &str) -> Result<(), String> {
        let chat = self.chats.iter().find(|chat| chat.id == id).cloned().ok_or("Esse chat não existe mais.")?;

        // its conversation where Claude Code looks for it from the new folder
        if let (Some(session), "claude") = (chat.session.as_deref(), chat.agent.as_str())
            && let Some(old) = Self::claude_transcript(&chat.cwd, session).filter(|old| old.is_file())
            && let Some(new) = claude_copy(&old, path, session).filter(|new| *new != old)
        {
            if let Some(parent) = new.parent() {
                fs::create_dir_all(parent).map_err(|error| format!("A conversa não pôde ir para {path}: {error}"))?;
            }

            fs::copy(&old, &new).map_err(|error| format!("A conversa não pôde ir para {path}: {error}"))?;
        }

        let Some(chat) = self.chats.iter_mut().find(|chat| chat.id == id) else { return Ok(()) };

        chat.cwd = path.to_string();

        // a name the person gave stays
        if !chat.named {
            let agent = chat.name.split(" · ").next().unwrap_or(&chat.agent).to_string();

            chat.name = chat_name(&agent, &chat.cwd);
        }

        self.prefs.folders.remember(path);
        self.save_chats();

        // its card, to the workspace of its new folder
        if let Some(space) = self.folder_space(path)
            && self.chat_node(id).is_some()
        {
            self.move_chat_card(id, &space.name)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claude_session_is_copied_beside_its_old_file_under_the_new_folders_name() {
        let old = Path::new("/home/eu/.claude/projects/-home-eu-a/s1.jsonl");

        assert_eq!(claude_copy(old, "/home/eu/b", "s1"), Some(PathBuf::from("/home/eu/.claude/projects/-home-eu-b/s1.jsonl")));
    }
}
