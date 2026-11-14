//! What the island does when it is asked: open an agent, a new chat in a workspace of its own, the voice.

use super::{NewForm, below_island, paint};
use crate::app::chat::direct::{NewChat, folder, folder_label};
use crate::app::chat::mini::{HEIGHT, WIDTH, screen_work};
use crate::prelude::*;

/// `wanted`, or the first of "wanted 2", "wanted 3"… that no name in `taken` has (any case).
pub(crate) fn unique_name<'a>(taken: impl IntoIterator<Item = &'a str> + Clone, wanted: &str) -> String {
    let free = |name: &str| !taken.clone().into_iter().any(|other| other.to_lowercase() == name.to_lowercase());
    (1..).map(|count| if count == 1 { wanted.to_string() } else { format!("{wanted} {count}") }).find(|name| free(name)).unwrap_or_default()
}

impl DrawingApp {
    pub(super) fn island_apply(&mut self, ctx: &Context, out: paint::Out) {
        if out.toggle_form {
            self.island.form = if self.island.form.is_some() { None } else { Some(self.island_form()) };
            ctx.request_repaint();
        }
        if out.pick_folder
            && let Some(path) = rfd::FileDialog::new().set_title("Pasta do chat").pick_folder()
            && let Some(form) = self.island.form.as_mut()
        {
            form.folder = path.display().to_string();
        }
        if out.create
            && let Some(form) = self.island.form.clone()
        {
            let installed = self.installed_agents();
            match self.island_new_chat(&form, &installed) {
                Ok(message) => {
                    self.island.form = None;
                    self.toast(message);
                    if let Some(id) = self.chat_agent.clone() {
                        self.island_show(ctx, &id);
                    }
                }
                Err(why) => self.toast(why),
            }
        }
        if let Some(id) = out.open_agent {
            self.island_show(ctx, &id);
        }
        if out.voice {
            self.voice_toggle();
        }
    }

    /// A new chat's form with what Ajustes presets: its folder.
    fn island_form(&mut self) -> NewForm {
        let installed = self.installed_agents();
        NewForm { name: String::new(), folder: NewChat::from_prefs(&self.prefs, &installed).cwd, worktree: false }
    }

    /// A new chat in a workspace of its own, so the work and the diff in progress in this one go on untouched: the
    /// workspace is made (named as the form says, else by the folder), shown, and the chat made in it. With
    /// `worktree` the folder is a git worktree of the repository (`maestro/environments.rs`), so the repository's
    /// own changes stay as they are. The chat is the one shown (`chat_agent`).
    pub(crate) fn island_new_chat(&mut self, form: &NewForm, installed: &[Preset]) -> Result<String, String> {
        if installed.is_empty() {
            return Err("Nenhum agente instalado: instale o claude ou o codex e volte aqui.".into());
        }
        let base = form.folder.trim().to_string();
        let base_path = folder(&base).filter(|path| path.is_dir()).ok_or_else(|| format!("A pasta {base} não existe."))?;
        self.load_spaces();
        let wanted = if form.name.trim().is_empty() { folder_label(&base) } else { form.name.trim().to_string() };
        let name = unique_name(self.spaces.list.iter().map(|space| space.name.as_str()), &wanted);
        let mut cwd = base;
        if form.worktree {
            let (path, _) = crate::app::maestro::environments::make(&base_path, &name, &uuid::Uuid::new_v4().simple().to_string())?;
            cwd = path.to_string_lossy().into_owned();
        }
        let space = self.create_space(&name, &cwd)?;
        if let Err(why) = self.switch_space(&space) {
            let _ = self.remove_space(&space);
            return Err(why);
        }
        let mut chat = NewChat::from_prefs(&self.prefs, installed);
        chat.cwd = cwd;
        self.create_chat(&chat, installed);
        Ok(format!("Chat novo no espaço {name}{}.", if form.worktree { ", num worktree próprio" } else { "" }))
    }

    /// Shows agent `id` from the island: a chat opens in the mini chat below the island (in the app's chat screen,
    /// when that is open); a terminal is brought up on its canvas. The app's window comes to the front when it
    /// is where the agent shows, even from the tray.
    pub(crate) fn island_show(&mut self, ctx: &Context, id: &str) {
        self.terminal_unread.remove(id);
        if self.chat_open || !self.is_chat(id) {
            self.open_agent(id);
            self.raise_window = true;
            return;
        }
        self.chat_agent = Some(id.to_string());
        if !self.mini.open {
            let anchor = self.island.anchor.unwrap_or_default();
            let [_, _, width, height] = self.prefs.mini_chat_window.unwrap_or([0.0, 0.0, WIDTH, HEIGHT]);
            let place = below_island(anchor, Vec2::new(width, height), screen_work(ctx, Pos2::new(anchor[0], anchor[1])));
            self.mini.opened_at = Some([place.left(), place.top(), place.width(), place.height()]);
        }
        self.mini_set(true);
    }
}
