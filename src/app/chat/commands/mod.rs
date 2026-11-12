//! Slash commands of the chat's box: `/` at its start opens a menu (name and what it does, filtered
//! as you type); the local ones run here (login, logout, status, mcp, doctor, update on the agent's
//! own CLI, hidden; clear, model, effort, help on the chat itself) and their output lands in the
//! conversation as a note, never going to the model. Any other `/word` goes to the agent as the
//! message, and the menu also lists the commands and skills found in `.claude/` of the chat's
//! folder and of the home.

use crate::app::chat::direct::{Chat, folder};
use crate::app::chat::footer::{Choice, agent_default, efforts, label, models, more_models};
use crate::app::chat::list::Look;
use crate::prelude::*;
use crate::transcript::{ChatItem, Transcript};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};

/// The menu's state: the picked row, the text it was last for, the text Esc closed it on, and the
/// custom commands found for a folder (looked up again every 5 s).
#[derive(Default)]
pub(crate) struct Slash {
    sel: usize,
    last: String,
    off: Option<String>,
    custom: Option<(String, f64, Vec<Entry>)>,
    /// The last message sent per chat, and the chats whose login came back since it failed.
    pub(crate) sent: std::collections::HashMap<String, String>,
    pub(crate) retry: std::collections::HashSet<String>,
}

enum Kind {
    Login,
    /// After a login: who is in now.
    Check,
    /// `/status`: the chat's own settings, then the login.
    Status(String),
    /// The output as it comes, under the command's name.
    Plain(&'static str),
}

/// A command's process running hidden: what it prints comes when it ends.
pub(crate) struct CmdRun {
    child: std::process::Child,
    rx: Receiver<(Vec<u8>, Vec<u8>)>,
    pub(crate) since: f64,
    kind: Kind,
    ctx: Context,
    /// Everything printed so far, both streams; the login's address is read from it.
    live: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    stdin: Option<std::process::ChildStdin>,
    shown_url: bool,
}

/// Reads `from` to its end, handing each chunk to `live` as it comes.
fn pump(mut from: impl Read, live: &std::sync::Mutex<Vec<u8>>) -> Vec<u8> {
    let (mut all, mut buf) = (Vec::new(), [0u8; 2048]);
    while let Ok(n) = from.read(&mut buf) {
        if n == 0 {
            break;
        }
        all.extend_from_slice(&buf[..n]);
        live.lock().unwrap().extend_from_slice(&buf[..n]);
    }
    all
}

/// The first address in what a login printed.
pub(crate) fn first_url(text: &str) -> Option<String> {
    text.split_whitespace().find(|w| w.starts_with("https://")).map(|w| w.trim_end_matches(['.', ',', ')']).to_string())
}

/// A login error's button asks for the login through the context: the item is drawn by `&self`.
const LOGIN_ASK: &str = "slash_login_ask";

pub(crate) fn take_login_ask(ctx: &Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::unique(LOGIN_ASK))).is_some()
}

impl DrawingApp {}

/// The box takes "/name " and its cursor goes to the end.
pub(crate) fn complete_into(ctx: &Context, draft: &mut String, name: &str, id: egui::Id) {
    *draft = format!("/{name} ");
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(draft.chars().count()))));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// The model `arg` names: an id or a name from the lists, a word one holds ("opus"), "padrão" for
/// the agent's own; else what was typed, which the agent reads as its own alias.
pub(crate) fn resolve_model(arg: &str, all: &[Choice]) -> String {
    let a = arg.trim().to_lowercase();
    if ["padrão", "padrao", "default"].contains(&a.as_str()) {
        return String::new();
    }
    let exact = all.iter().find(|c| c.id.to_lowercase() == a || c.name.to_lowercase() == a);
    let holds = || all.iter().find(|c| c.id.to_lowercase().contains(&a) || c.name.to_lowercase().contains(&a));
    exact.or_else(holds).map_or_else(|| arg.trim().to_string(), |c| c.id.clone())
}

/// The effort `arg` names, by id or by name; Some("") is the default; None, not one of them.
pub(crate) fn resolve_effort(arg: &str, list: &[Choice]) -> Option<String> {
    let a = arg.trim().to_lowercase();
    if ["padrão", "padrao", "default"].contains(&a.as_str()) {
        return Some(String::new());
    }
    list.iter().find(|e| e.id == a || e.name.to_lowercase() == a || e.name.to_lowercase().replace('á', "a") == a).map(|e| e.id.clone())
}

/// The two buttons under a login error: Entrar, and once the login is done Mandar de novo. They
/// ask through the context (`take_login_ask`, `take_retry_ask`): the conversation is drawn by `&self`.
const RETRY_ASK: &str = "slash_retry_ask";

pub(crate) fn take_retry_ask(ctx: &Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::unique(RETRY_ASK))).is_some()
}

impl DrawingApp {
    /// Under the conversation, when its last word is an expired-login error: the primary button.
    pub(crate) fn login_buttons(&self, ui: &mut egui::Ui, id: &str, items: &[ChatItem], look: &Look) {
        let expired = items
            .iter()
            .rev()
            .find(|i| !matches!(i, ChatItem::System(_)))
            .is_some_and(|i| matches!(i, ChatItem::Error(e) if crate::app::chat::error_hint(e).is_some()));
        if !expired || !self.is_chat(id) || self.chat_cmds.contains_key(id) {
            return;
        }
        let retry = self.slash.retry.contains(id);
        let (fill, ink) = if look.dark { (Color32::WHITE, Color32::BLACK) } else { (Color32::BLACK, Color32::WHITE) };
        ui.add_space(6.0);
        let label = egui::RichText::new(if retry { "Mandar de novo" } else { "Entrar" }).size(14.0).strong().color(ink);
        // no keyboard focus: an Enter meant for the box never presses it
        let sense = Sense::CLICK;
        let button = egui::Button::new(label).sense(sense).fill(fill).corner_radius(18).min_size(Vec2::new(120.0, 34.0));
        if ui.add(button).on_hover_cursor(CursorIcon::PointingHand).clicked() {
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique(if retry { RETRY_ASK } else { LOGIN_ASK }), true));
        }
        ui.add_space(8.0);
    }

    /// Mandar de novo: the message that failed goes again, now that the login is back.
    pub(crate) fn chat_retry(&mut self, ctx: &Context, id: &str) {
        self.slash.retry.remove(id);
        if let Some(prompt) = self.slash.sent.get(id).cloned() {
            self.chat_send(ctx, id, &prompt);
        }
    }
}

mod catalog;
mod exec;
mod slash;
#[cfg(test)]
mod tests;
pub(crate) use catalog::*;
