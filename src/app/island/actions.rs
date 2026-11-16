//! What the island does when it is asked: open an agent, a new chat in a workspace of its own, the voice.

use super::{NewForm, Tab, forms, paint};
use crate::app::chat::direct::{NewChat, folder, folder_label};
use crate::prelude::*;

/// `wanted`, or the first of "wanted 2", "wanted 3"… that no name in `taken` has (any case).
pub(crate) fn unique_name<'a>(taken: impl IntoIterator<Item = &'a str> + Clone, wanted: &str) -> String {
    let free = |name: &str| !taken.clone().into_iter().any(|other| other.to_lowercase() == name.to_lowercase());
    (1..).map(|count| if count == 1 { wanted.to_string() } else { format!("{wanted} {count}") }).find(|name| free(name)).unwrap_or_default()
}

impl DrawingApp {
    pub(super) fn island_apply(&mut self, ctx: &Context, out: paint::Out, time: f64) {
        // the focus moves only when a pill is clicked (or the keyboard picks one) and stays, as Coucou's `focusId`
        if let Some(id) = out.focus.clone() {
            self.outside_focus(&id);
            self.island.focus = Some(id);
        }
        if out.wake {
            self.island.machine.click(time);
        }
        if let Some(size) = out.resize {
            self.island.pulled = Some([size.x.round(), size.y.round()]);
            self.island.chat_big = false;
        } else if let Some(size) = self.island.pulled.take() {
            *if self.island.chat_full { &mut self.prefs.island_chat } else { &mut self.prefs.island_talk } = Some(size);
        }
        // an edge double-clicked: back to the size it had before it was pulled
        if out.unpull {
            if self.island.chat_full {
                (self.prefs.island_chat, self.island.chat_big) = (None, false);
            } else {
                self.prefs.island_talk = None;
            }
        }
        if let Some(data) = out.picture.clone() {
            self.island_picture_open(ctx, data);
        }
        self.island.resizing = out.resize.is_some();
        if out.sound {
            self.prefs.island_sound = !self.prefs.island_sound;
        }
        if out.full {
            self.island.chat_full = true;
        }
        if out.big {
            self.island.chat_big = !self.island.chat_big;
        }
        if out.close {
            self.island_fold();
            self.island.machine.force_petit(time);
            (self.island.tab, self.island.form) = (Tab::Home, None);
            self.island.settings = false;
        }
        if let Some(tab) = out.tab {
            self.island.settings = false;
            self.island.tab = tab;
            // a session outside shows on the conversation page only until another page is taken
            if tab != Tab::Chat {
                self.island.session = None;
            }
            self.island.machine.interacted(time);
            self.island.form = if tab == Tab::New { Some(self.island.form.take().unwrap_or_else(|| self.island_form())) } else { None };
            if tab == Tab::Chat && self.chat_agent.is_none() {
                self.chat_agent = self.chats.first().map(|chat| chat.id.clone());
            }
        }
        if out.toggle_settings {
            self.island.settings = !self.island.settings;
            self.island.machine.interacted(time);
        }
        if out.open_app_settings {
            self.open_settings_at("Geral");
            self.raise_window = true;
        }
        if let Some(volume) = out.volume {
            self.prefs.island_volume = volume.clamp(0.0, 0.2);
        }
        if let Some(close_after) = out.close_after {
            self.prefs.island_close_after = close_after;
        }
        if let Some(color) = out.color {
            self.prefs.island_color = [color.r(), color.g(), color.b()];
        }
        if out.pick_folder
            && let Some(path) = rfd::FileDialog::new().set_title("Pasta do projeto").pick_folder()
            && let Some(form) = self.island.form.as_mut()
        {
            form.folder = path.display().to_string();
            form.projects = forms::remember_project(&form.folder, &form.projects);
            form.failed = None;
        }
        if out.create
            && let Some(form) = self.island.form.clone()
        {
            let installed = self.installed_agents();
            match self.island_new_chat(&form, &installed) {
                Ok(message) => {
                    self.prefs.island_projects = forms::remember_project(&form.folder, &self.prefs.island_projects);
                    if let Some(id) = self.chat_agent.clone().filter(|_| !form.prompt.trim().is_empty()) {
                        self.chat_send(ctx, &id, form.prompt.trim());
                    }
                    self.island.form = None;
                    self.island.tab = Tab::Chat;
                    self.toast(message);
                }
                Err(why) => {
                    if let Some(form) = self.island.form.as_mut() {
                        form.failed = Some(why);
                    }
                }
            }
        }
        if let Some(id) = out.talk {
            self.island.session = None;
            self.chat_agent = Some(id);
            self.island.tab = Tab::Chat;
            self.island.machine.force_home(time);
        }
        if let Some(id) = out.open_agent {
            self.island_show(&id, time);
        }
        if let Some(id) = out.canvas {
            self.island_on_canvas(&id);
        }
        if let Some(id) = out.terminal
            && !self.outside_terminal(&id)
            && let Some(session) = self.island.session.as_mut()
        {
            session.notice("Não deu para trazer a janela desse terminal: responda nele.", time);
        }
        if out.voice {
            self.voice_toggle();
        }
    }

    /// The app in front on the canvas, on the card of agent `id` (the island's "No canvas"; `focus_canvas` for the MCP).
    pub(crate) fn island_on_canvas(&mut self, id: &str) {
        self.terminal_unread.remove(id);
        self.show_canvas();
        self.open_agent(id);
        self.raise_window = true;
    }

    /// A new chat's form with what Ajustes presets: its folder.
    /// The "+" form: the recent projects (the ones chosen here, then the folders of the chats, newest first), the first
    /// of them chosen; none, and the person picks one.
    pub(super) fn island_form(&mut self) -> NewForm {
        let folders = self.chats.iter().rev().map(|chat| chat.cwd.clone()).filter(|cwd| std::path::Path::new(cwd).is_dir()).collect::<Vec<_>>();
        let mut projects = folders.iter().rev().fold(Vec::new(), |list, cwd| forms::remember_project(cwd, &list));
        for kept in self.prefs.island_projects.iter().rev() {
            projects = forms::remember_project(kept, &projects);
        }
        NewForm { folder: projects.first().cloned().unwrap_or_default(), projects, ..Default::default() }
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

    /// Shows agent `id` from the island: a chat opens in the island's own conversation; a terminal is brought up on its
    /// canvas, and with the app's chat screen open the agent shows there. The app's window comes to the front when it is
    /// where the agent shows, even from the tray. A session outside the app opens its conversation in the island, read
    /// from its terminal's (`session.rs`).
    pub(crate) fn island_show(&mut self, id: &str, time: f64) {
        if self.outside_focus(id) {
            self.island.focus = Some(id.to_string());
            self.island.session = Some(super::session::SessionView::new(id));
            (self.island.tab, self.island.chat_full, self.island.settings, self.island.form) = (Tab::Chat, false, false, None);
            self.island.machine.force_home(time);
            return;
        }
        if self.integration_open(id) {
            return;
        }
        self.terminal_unread.remove(id);
        if self.chat_open || !self.is_chat(id) {
            self.open_agent(id);
            self.raise_window = true;
            return;
        }
        self.chat_agent = Some(id.to_string());
        self.island.tab = Tab::Chat;
        self.island.machine.force_home(time);
    }
}

impl DrawingApp {
    /// Closing the island with an ask on it only folds it (Coucou's `foldApproval`): the ask keeps waiting and shows again
    /// when the island opens.
    pub(super) fn island_fold(&mut self) {
        self.island.folded = self.person_asks.borrow().iter().map(|ask| ask.chat.clone()).collect();
    }
}
