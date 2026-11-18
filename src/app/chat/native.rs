//! The chat on the Mac in SwiftUI (macos/MagoUI/Sources/MagoUI/Chat): each frame the conversations list goes out as the
//! topic "chats" and every chat on screen (the one open, the ones a SwiftUI view watches) as "chat:<id>"; what SwiftUI
//! asks comes back as `ui.chat.<verb>` and runs the same functions the egui chat runs. The egui chat steps aside
//! (`chat_screen`, `mini_chat_ui`).

use crate::app::chat::direct::{CHAT_MODES, folder_label};
use crate::app::chat::footer::{Choice, FooterAct, agent_default, chip_folder, efforts, label, models, more_models, pretty, tuning_names};
use crate::app::chat::list::running::{chip, counts, kept, running_counts, space_groups};
use crate::app::chat::segments::{Seg, segments};
use crate::app::chat::steps::{counter, run_summary, running_step, short_tool};
use crate::app::chat::{EARLIER, window_start};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::transcript::{ChatItem, Step};
use std::cell::RefCell;
use std::collections::BTreeSet;

/// How often the snapshots are built: a long conversation laid out as JSON each frame would cost more than it shows.
const PUBLISH_EVERY: f64 = 1.0 / 15.0;

/// What the bridge between SwiftUI and the chat keeps from one frame to the next.
#[derive(Default)]
struct Native {
    /// The chats a SwiftUI view shows besides the open one (`ui.chat.watch`).
    watched: BTreeSet<String>,
    /// Messages sent from SwiftUI, waiting for a frame with its context (`native_chat_frame`).
    sends: Vec<(String, String)>,
    /// The box's words as SwiftUI last had them; when Rust changes them (Editar, a refused send) `draft_seq` grows and
    /// SwiftUI takes them.
    draft_seen: String,
    draft_seq: u64,
    published: f64,
    polled: f64,
}

thread_local! {
    // ponytail: one bridge state for the UI thread, a field of DrawingApp if the chat ever runs off it
    static NATIVE: RefCell<Native> = RefCell::default();
}

/// The conversations list ("Chats").
#[derive(Serialize, PartialEq, Debug)]
pub(crate) struct ChatsSnapshot {
    /// The chat screen is up (not the canvas).
    open: bool,
    current: Option<String>,
    show: &'static str,
    filter: String,
    /// Todos, Trabalhando, Esperando, Prontos.
    counts: [usize; 4],
    running: usize,
    waiting: usize,
    groups: Vec<GroupSnapshot>,
}

#[derive(Serialize, PartialEq, Debug)]
struct GroupSnapshot {
    name: String,
    entries: Vec<EntrySnapshot>,
}

#[derive(Serialize, PartialEq, Debug)]
struct EntrySnapshot {
    id: String,
    name: String,
    /// working, waiting or ready.
    state: &'static str,
    last: String,
    ago: String,
    color: [u8; 3],
    /// A chat of the app (renamed, removed); else a terminal or a session outside.
    chat: bool,
}

/// One conversation.
#[derive(Serialize, PartialEq, Debug)]
pub(crate) struct ChatSnapshot {
    id: String,
    title: String,
    agent: String,
    folder: String,
    /// working, waiting or ready.
    state: &'static str,
    status: String,
    is_chat: bool,
    /// No message went yet: the provider may still change.
    fresh: bool,
    providers: Vec<Provider>,
    /// How many messages are above the ones sent ("Mostrar mensagens anteriores").
    earlier: usize,
    items: Vec<ItemSnapshot>,
    band: Option<Band>,
    ask: Option<AskSnapshot>,
    queue: Vec<String>,
    draft: String,
    draft_seq: u64,
    hint: String,
    slash: Vec<SlashEntry>,
    chips: Option<Chips>,
}

#[derive(Serialize, PartialEq, Debug)]
struct Provider {
    name: String,
    program: String,
    on: bool,
}

/// A message, or a run of tool steps; `id` is its place in the conversation (stable while it streams).
#[derive(Serialize, PartialEq, Debug, Default)]
struct ItemSnapshot {
    id: String,
    /// user, agent, system, divider, error, tools, image.
    kind: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    parts: Vec<Part>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    steps: Vec<StepSnapshot>,
    /// A run of tools: "4 ferramentas · Edit, Bash ×2 · 46 s", and whether it starts folded.
    #[serde(skip_serializing_if = "String::is_empty")]
    summary: String,
    folded: bool,
    /// Another agent's question: whose.
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'static str>,
}

/// A piece of the agent's words: text (markdown), code, image or file.
#[derive(Serialize, PartialEq, Debug)]
struct Part {
    kind: &'static str,
    text: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    info: String,
}

#[derive(Serialize, PartialEq, Debug)]
struct StepSnapshot {
    id: String,
    name: String,
    what: String,
    note: String,
    failed: bool,
    running: bool,
    diff: Option<(usize, usize)>,
    input: String,
    /// The end of what it printed.
    output: String,
    error: String,
}

/// The band over the box while the agent works.
#[derive(Serialize, PartialEq, Debug)]
struct Band {
    tool: String,
    what: String,
    counter: Option<(u32, u32)>,
    /// The thinking line when no tool runs ("Fulano está trabalhando…").
    thinking: String,
    seconds: u64,
}

#[derive(Serialize, PartialEq, Debug)]
struct AskSnapshot {
    tool: String,
    questions: Vec<QuestionSnapshot>,
}

#[derive(Serialize, PartialEq, Debug)]
struct QuestionSnapshot {
    text: String,
    header: String,
    multiple: bool,
    options: Vec<(String, String)>,
}

#[derive(Serialize, PartialEq, Debug)]
struct SlashEntry {
    name: String,
    args: String,
    desc: String,
}

/// The chips under the box: folder, permission, model and effort.
#[derive(Serialize, PartialEq, Debug)]
struct Chips {
    folder: String,
    folder_path: String,
    /// The conversation started in its folder: no other.
    folder_locked: bool,
    /// "Nenhuma pasta": the app's own folder.
    no_folder: String,
    recent: Vec<(String, String)>,
    mode: usize,
    modes: Vec<&'static str>,
    tuning: String,
    model: String,
    models: Vec<ChoiceSnapshot>,
    more: Vec<ChoiceSnapshot>,
    effort: String,
    efforts: Vec<ChoiceSnapshot>,
}

#[derive(Serialize, PartialEq, Debug)]
struct ChoiceSnapshot {
    id: String,
    name: String,
    note: String,
}

fn choices(list: &[Choice]) -> Vec<ChoiceSnapshot> {
    list.iter().map(|choice| ChoiceSnapshot { id: choice.id.clone(), name: choice.name.clone(), note: choice.note.clone() }).collect()
}

fn state_key(state: &AgentState) -> &'static str {
    match chip(state).0 {
        "Trabalhando" => "working",
        "Esperando" => "waiting",
        _ => "ready",
    }
}

/// The agent's words cut as the egui chat cuts them (chat/segments.rs).
fn parts(text: &str) -> Vec<Part> {
    segments(text)
        .into_iter()
        .map(|seg| match seg {
            Seg::Text(text) => Part { kind: "text", text, info: String::new() },
            Seg::Code { info, code } => Part { kind: "code", text: code, info },
            Seg::Image { alt, src } => Part { kind: "image", text: src, info: alt },
            Seg::File { label, path, .. } => Part { kind: "file", text: path, info: label },
        })
        .collect()
}

fn step_snapshot(step: &Step, running: &[(String, String)]) -> StepSnapshot {
    let lines: Vec<&str> = step.log.lines().collect();
    StepSnapshot {
        id: step.id.clone(),
        name: short_tool(&step.name).to_string(),
        what: step.what.clone(),
        note: step.note.clone().unwrap_or_default(),
        failed: step.failed,
        running: running.iter().any(|(id, _)| *id == step.id),
        diff: step.diff,
        input: step.input.clone(),
        output: lines[lines.len().saturating_sub(20)..].join("\n"),
        error: step.error.clone().unwrap_or_default(),
    }
}

/// What picking `id` stores: "" when it is the agent's default (as footer.rs's `pick`).
fn pick(id: &str, default: &str) -> String {
    if id == default { String::new() } else { id.to_string() }
}

impl DrawingApp {
    /// Called every frame by the coordinator while the chat runs in SwiftUI: the transcripts read, the snapshots out.
    pub(crate) fn native_chat(&mut self) {
        self.installed_agents();
        let shown = self.chat_agent.clone();
        self.chats.retain(|chat| !chat.draft || shown.as_deref() == Some(chat.id.as_str()));
        // the chat screen with nothing picked: Claude's chat first, else a draft (as `chat_screen`)
        if self.chat_open && self.chat_agent.as_ref().is_none_or(|id| self.row_of(id).is_none()) {
            let chats = self.chat_rows();
            self.chat_agent = chats.iter().find(|row| row.spec.command.contains("claude")).or(chats.first()).map(|row| row.id.clone());
            if self.chat_agent.is_none() {
                self.start_draft_chat(true);
            }
        }
        self.refresh_transcript();
        let now = self.now;
        let (due, poll, watched) = NATIVE.with_borrow_mut(|native| {
            if self.chat_draft != native.draft_seen {
                native.draft_seen = self.chat_draft.clone();
                native.draft_seq += 1;
            }
            let poll = now - native.polled >= 0.5;
            if poll {
                native.polled = now;
            }
            let due = now - native.published >= PUBLISH_EVERY;
            if due {
                native.published = now;
            }
            (due, poll, native.watched.clone())
        });
        let mut ids: Vec<String> = watched.into_iter().collect();
        ids.extend(self.chat_agent.clone().filter(|id| !ids.contains(id)));
        if poll {
            let open = self.chat_agent.clone();
            for id in ids.iter().filter(|id| open.as_ref() != Some(*id)) {
                self.refresh_transcript_of(id);
            }
        }
        if !due {
            return;
        }
        crate::app::native::publish("chats", &self.native_chats_snapshot());
        for id in ids {
            if let Some(snapshot) = self.native_chat_snapshot(&id) {
                crate::app::native::publish(&format!("chat:{id}"), &snapshot);
            }
        }
    }

    /// What SwiftUI sent that needs the frame's context: its messages go as the box's Enter sends them.
    pub(crate) fn native_chat_frame(&mut self, ctx: &Context) {
        for (id, text) in NATIVE.with_borrow_mut(|native| std::mem::take(&mut native.sends)) {
            self.send_from_box(ctx, &id, text);
        }
    }

    /// `ui.chat.<verb>`: what has no action of its own in the MCP layer.
    pub(crate) fn native_chat_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().to_string();
        let chat = || {
            let id = text("chat");
            if id.is_empty() { self.chat_agent.clone().ok_or_else(|| "Nenhuma conversa aberta.".to_string()) } else { Ok(id) }
        };
        match verb {
            "watch" | "unwatch" => {
                let id = chat()?;
                NATIVE.with_borrow_mut(|native| if verb == "watch" { native.watched.insert(id) } else { native.watched.remove(&id) });
                Ok(String::new())
            }
            // a row of the list: its conversation on the chat screen
            "select" => {
                let id = chat()?;
                self.show_chat();
                self.open_agent(&id);
                Ok("Conversa aberta.".into())
            }
            "draft" => {
                let words = text("text");
                NATIVE.with_borrow_mut(|native| native.draft_seen = words.clone());
                self.chat_draft = words;
                Ok(String::new())
            }
            // Citar: the words as a quote in the box, which SwiftUI takes (`draft_seq`)
            "cite" => {
                crate::app::chat::list::quote_into(&mut self.chat_draft, &text("text"));
                Ok(String::new())
            }
            "send" => {
                let (id, words) = (chat()?, text("text"));
                if words.trim().is_empty() && !self.has_attached() {
                    return Err("Nada para enviar.".into());
                }
                self.chat_draft.clear();
                NATIVE.with_borrow_mut(|native| {
                    native.draft_seen.clear();
                    native.sends.push((id, words.trim_end().to_string()));
                });
                Ok("Enviado.".into())
            }
            // Interromper (Esc)
            "stop" => {
                let id = chat()?;
                if self.is_chat(&id) {
                    self.chat_stop(&id);
                } else if let Some(Ok(session)) = self.terminals.get(&id) {
                    session.handle.write(vec![0x1b]);
                }
                Ok("Interrompido.".into())
            }
            // the queue's Enviar agora and Editar
            "queue" => {
                let id = chat()?;
                match text("action").as_str() {
                    "send_now" => self.send_queued_now(&id),
                    "edit" => {
                        self.chat_agent = Some(id.clone());
                        self.unqueue(&id);
                        Ok("A mensagem voltou para a caixa.".into())
                    }
                    other => Err(format!("action desconhecida: {other}. Use send_now ou edit.")),
                }
            }
            // a chip of the box: model, effort, mode or folder (Abrir pasta… is SwiftUI's own panel)
            "footer" => {
                let id = chat()?;
                let Some(current) = self.chats.iter().find(|c| c.id == id).cloned() else { return Err("Não é um chat.".into()) };
                self.chat_agent = Some(id.clone());
                let (default_model, default_effort) = agent_default(&current.agent);
                let ran = self.ran_model(&id);
                let default_model = if ran.is_empty() { default_model } else { ran };
                let act = if let Some(model) = args["model"].as_str() {
                    FooterAct::Model(pick(model, &default_model))
                } else if let Some(effort) = args["effort"].as_str() {
                    FooterAct::Effort(pick(effort, &default_effort))
                } else if let Some(mode) = args["mode"].as_u64() {
                    FooterAct::Mode(mode as usize)
                } else if let Some(folder) = args["folder"].as_str() {
                    FooterAct::FolderPick(folder.to_string())
                } else {
                    return Err("diga model, effort, mode ou folder".into());
                };
                self.chat_footer_act(act);
                Ok(String::new())
            }
            "earlier" => {
                let id = chat()?;
                let pages = if self.chat_earlier.0 == id { self.chat_earlier.1 + 1 } else { 1 };
                self.chat_earlier = (id, pages);
                Ok(String::new())
            }
            // Nova conversa: a draft, made a chat by its first message
            "new" => {
                self.show_chat();
                if !self.start_draft_chat(false) {
                    return Err("Nenhum agente instalado (claude ou codex).".into());
                }
                Ok("Conversa nova.".into())
            }
            "remove" => {
                let id = chat()?;
                self.remove_chat(&id);
                Ok("Conversa removida.".into())
            }
            // Ver no canvas: a chat's card, a terminal's terminal
            "on_canvas" => {
                let id = chat()?;
                if self.is_chat(&id) {
                    return self.show_chat_on_canvas(&id);
                }
                self.show_canvas();
                self.open_agent(&id);
                Ok("No canvas.".into())
            }
            "settings" => {
                self.settings_open = !self.settings_open;
                Ok(String::new())
            }
            other => Err(format!("ui.chat.{other} não existe")),
        }
    }

    /// The row of agent `id`: a terminal, a chat (a draft too) or a session outside.
    fn row_of(&self, id: &str) -> Option<AgentRow> {
        self.agent_rows().into_iter().chain(self.draft_rows()).find(|row| row.id == id).or_else(|| self.outside_row(id))
    }

    pub(crate) fn native_chats_snapshot(&self) -> ChatsSnapshot {
        let entries = self.running_entries();
        let (running, waiting) = running_counts(entries.iter().map(|entry| entry.state));
        let show = self.chat_listing.show;
        let shown = kept(&entries, show, &self.chat_listing.filter.to_lowercase());
        let order: Vec<String> = self.spaces.list.iter().map(|space| space.id.clone()).collect();
        let groups = space_groups(&shown, &order)
            .into_iter()
            .map(|(space, members)| GroupSnapshot {
                name: self.group_look(&space).0,
                entries: members
                    .into_iter()
                    .map(|index| {
                        let entry = shown[index];
                        EntrySnapshot {
                            id: entry.id.clone(),
                            name: entry.name.clone(),
                            state: state_key(&entry.state),
                            last: entry.last.clone(),
                            ago: entry.ago.map(crate::app::ui::agents::ago).unwrap_or_default(),
                            color: [entry.color.r(), entry.color.g(), entry.color.b()],
                            chat: self.is_chat(&entry.id),
                        }
                    })
                    .collect(),
            })
            .collect();
        ChatsSnapshot {
            open: self.chat_open,
            current: self.chat_agent.clone(),
            show: show.key(),
            filter: self.chat_listing.filter.clone(),
            counts: counts(&entries),
            running,
            waiting,
            groups,
        }
    }

    pub(crate) fn native_chat_snapshot(&mut self, id: &str) -> Option<ChatSnapshot> {
        let row = self.row_of(id)?;
        let chat = self.chats.iter().find(|c| c.id == id).cloned();
        let transcript = self.transcripts.get(id);
        let asked = if self.chat_agent.as_deref() == Some(id) { self.asked_of() } else { Vec::new() };
        let mut items = Vec::new();
        let mut earlier = 0;
        if let Some(t) = transcript {
            // each question marks one message, the latest that says it (as `conversation`)
            let mut from = std::collections::HashMap::new();
            for (prompt, who) in asked.iter().rev() {
                let says = |(index, item): &(usize, &ChatItem)| {
                    !from.contains_key(index) && matches!(item, ChatItem::User(text) if prompt.split_whitespace().eq(text.split_whitespace()))
                };
                if let Some((index, _)) = t.items.iter().enumerate().rev().find(says) {
                    from.insert(index, who.clone());
                }
            }
            let pages = if self.chat_earlier.0 == id { self.chat_earlier.1 } else { 0 };
            earlier = window_start(t.items.len(), pages);
            let mut index = earlier;
            while index < t.items.len() {
                let steps: Vec<&Step> = t.items[index..].iter().map_while(|item| if let ChatItem::Tool(step) = item { Some(step) } else { None }).collect();
                let next = index + steps.len().max(1);
                let mut item = ItemSnapshot { id: index.to_string(), ..Default::default() };
                if !steps.is_empty() {
                    item.kind = "tools";
                    item.summary = run_summary(&steps);
                    item.folded = steps.len() >= 2 && next < t.items.len();
                    item.steps = steps.iter().map(|step| step_snapshot(step, &t.running)).collect();
                } else {
                    match &t.items[index] {
                        ChatItem::User(text) => (item.kind, item.text, item.from) = ("user", text.clone(), from.get(&index).cloned()),
                        ChatItem::Agent(text) => (item.kind, item.parts) = ("agent", parts(text)),
                        ChatItem::System(text) if text == EARLIER => (item.kind, item.text) = ("divider", text.clone()),
                        ChatItem::System(text) => (item.kind, item.text) = ("system", text.clone()),
                        ChatItem::Error(text) => (item.kind, item.text, item.hint) = ("error", text.clone(), crate::app::chat::error_hint(text)),
                        // ponytail: a picture says only its type, its bytes would go in every snapshot; a file path for it later
                        ChatItem::Image { media, user, .. } => (item.kind, item.text, item.from) = ("image", media.clone(), user.then(String::new)),
                        ChatItem::Tool(_) => {}
                    }
                }
                items.push(item);
                index = next;
            }
        }
        let band = match row.state {
            AgentState::Working(since) => Some(match transcript.and_then(running_step) {
                Some((step, more)) => Band {
                    tool: short_tool(&step.name).to_string(),
                    what: if more > 0 { format!("{} +{more}", step.what) } else { step.what.clone() },
                    counter: step.output.as_deref().and_then(counter),
                    thinking: String::new(),
                    seconds: step.started.map_or(self.now - since, |started| now_ms() as f64 / 1000.0 - started).max(0.0) as u64,
                },
                None => {
                    let name = row.spec.name.split(" · ").next().filter(|name| !name.is_empty()).unwrap_or("O agente");
                    let thinking = crate::app::chat::thinking::line(name, since, self.now - since);
                    Band { tool: String::new(), what: String::new(), counter: None, thinking, seconds: (self.now - since).max(0.0) as u64 }
                }
            }),
            _ => None,
        };
        let ask = self.person_asks.borrow().iter().find(|ask| ask.chat == id).map(|ask| AskSnapshot {
            tool: ask.tool.clone(),
            questions: ask
                .questions
                .iter()
                .map(|question| QuestionSnapshot {
                    text: question.text.clone(),
                    header: question.header.clone(),
                    multiple: question.multiple,
                    options: question.options.clone(),
                })
                .collect(),
        });
        let waiting = row.state == AgentState::Waiting && ask.is_none();
        let status = match row.state {
            AgentState::Working(since) => format!("Trabalhando · há {}", crate::app::chat::list::elapsed(self.now - since)),
            AgentState::Waiting if ask.is_some() => "Precisa de você · responda acima da caixa".into(),
            AgentState::Waiting => "Precisa de você · esperando no terminal".into(),
            AgentState::Idle(_) => "Pronto".into(),
            other => other.label().to_string(),
        };
        let (agent, folder, model) = match &chat {
            Some(chat) => {
                let model = tuning_names(chat, &self.ran_model(&chat.id)).0;
                (chat.name.split(" · ").next().unwrap_or(&chat.name).to_string(), chat.cwd.clone(), model)
            }
            None => (row.spec.label().to_string(), row.spec.cwd.clone(), self.usage.agents.get(id).map(|usage| usage.model.clone()).unwrap_or_default()),
        };
        let status = [status, model].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ");
        let fresh = chat.as_ref().is_some_and(|chat| chat.session.is_none());
        let providers = if fresh {
            self.chat_providers().into_iter().map(|(name, program)| Provider { on: program == row.spec.command, name, program }).collect()
        } else {
            Vec::new()
        };
        let hint = if waiting { "Escreva uma mensagem (fica na fila até você responder no terminal)" } else { "Escreva uma mensagem…" };
        // the slash menu reads the open chat's box
        let slash = if self.chat_agent.as_deref() == Some(id) { self.slash_entries().unwrap_or_default() } else { Vec::new() };
        let chips = chat.as_ref().map(|chat| {
            let (default_model, default_effort) = agent_default(&chat.agent);
            let ran = self.ran_model(&chat.id);
            let default_model = if ran.is_empty() { default_model } else { ran };
            let model_id = if chat.model.is_empty() { default_model } else { chat.model.clone() };
            let effort_id = if chat.effort.is_empty() { default_effort } else { chat.effort.clone() };
            let (mut list, more) = (models(&chat.agent), more_models(&chat.agent));
            if !model_id.is_empty() && !list.iter().chain(&more).any(|choice| choice.id == model_id) {
                list.insert(0, Choice { id: model_id.clone(), name: pretty(&model_id), note: "O que esta conversa está usando".into() });
            }
            let all: Vec<Choice> = list.iter().chain(&more).cloned().collect();
            let effort_list = efforts(&chat.agent, &model_id);
            let model = if model_id.is_empty() { "Modelo padrão".to_string() } else { label(&all, &model_id) };
            let effort = effort_list.iter().find(|choice| choice.id == effort_id).map(|choice| choice.name.clone());
            Chips {
                folder: chip_folder(&chat.cwd),
                folder_path: chat.cwd.clone(),
                folder_locked: chat.session.is_some(),
                no_folder: crate::file::projects_dir().map(|dir| dir.display().to_string()).unwrap_or_default(),
                recent: self.prefs.folders.choices().into_iter().map(|(path, _)| (folder_label(&path), path)).collect(),
                mode: chat.mode.min(CHAT_MODES.len() - 1),
                modes: CHAT_MODES.to_vec(),
                tuning: effort.map_or(model.clone(), |effort| format!("{model} · {effort}")),
                model: model_id,
                models: choices(&list),
                more: choices(&more),
                effort: effort_id,
                efforts: choices(&effort_list),
            }
        });
        let (draft, draft_seq) = NATIVE.with_borrow(|native| (self.chat_draft.clone(), native.draft_seq));
        Some(ChatSnapshot {
            id: id.to_string(),
            title: self.chat_title(&row),
            agent,
            folder: if crate::app::chat::direct::no_folder(&folder) { "Nenhuma pasta".into() } else { folder },
            state: state_key(&row.state),
            status,
            is_chat: chat.is_some(),
            fresh,
            providers,
            earlier,
            items,
            band,
            ask,
            queue: self.chat_queue.iter().filter(|(to, _)| to == id).map(|(_, text)| text.clone()).collect(),
            draft,
            draft_seq,
            hint: hint.into(),
            slash: slash.into_iter().map(|entry| SlashEntry { name: entry.name, args: entry.args, desc: entry.desc }).collect(),
            chips,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat::direct::Chat;
    use crate::transcript::Transcript;

    fn app_with_chat() -> DrawingApp {
        let mut app = DrawingApp::default();
        app.chats.push(Chat {
            id: "c1".into(),
            agent: "claude".into(),
            name: "Claude Code · loja".into(),
            cwd: "/tmp/loja".into(),
            session: None,
            mode: 0,
            model: String::new(),
            effort: String::new(),
            recruit: false,
            named: false,
            draft: false,
        });
        let mut transcript = Transcript::new("/nada.jsonl".into());
        let step = |id: &str, name: &str| ChatItem::Tool(Step { id: id.into(), name: name.into(), what: "x".into(), ..Default::default() });
        transcript.items = vec![
            ChatItem::User("oi".into()),
            step("a", "Read"),
            step("b", "Edit"),
            ChatItem::Agent("Feito:\n\n```rust\nfn a() {}\n```".into()),
            step("c", "Bash"),
        ];
        transcript.running = vec![("c".into(), "Bash x".into())];
        app.transcripts.insert("c1".into(), transcript);
        app.chat_agent = Some("c1".into());
        app
    }

    #[test]
    fn the_snapshot_has_the_messages_the_tool_runs_folded_and_the_chips() {
        let mut app = app_with_chat();
        let snapshot = app.native_chat_snapshot("c1").expect("the chat");
        let kinds: Vec<&str> = snapshot.items.iter().map(|item| item.kind).collect();
        assert_eq!(kinds, ["user", "tools", "agent", "tools"]);
        assert_eq!(snapshot.items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), ["0", "1", "3", "4"]);
        // two tools with words after them fold; the last one runs open
        assert!(snapshot.items[1].folded && snapshot.items[1].summary.starts_with("2 ferramentas"));
        assert!(!snapshot.items[3].folded && snapshot.items[3].steps[0].running);
        let parts: Vec<(&str, &str)> = snapshot.items[2].parts.iter().map(|part| (part.kind, part.text.as_str())).collect();
        assert_eq!(parts, [("text", "Feito:"), ("code", "fn a() {}")]);
        assert_eq!(snapshot.items[2].parts[1].info, "rust");
        assert!(serde_json::to_value(&snapshot).is_ok_and(|json| json["items"][0]["text"] == "oi"));
        let chips = snapshot.chips.expect("a chat has chips");
        assert_eq!((chips.folder.as_str(), chips.folder_locked, chips.modes.len()), ("loja", false, CHAT_MODES.len()));
        assert!(snapshot.fresh && snapshot.is_chat && snapshot.state == "ready");
    }

    #[test]
    fn the_verbs_change_the_draft_queue_and_chips_as_the_egui_chat_does() {
        let mut app = app_with_chat();
        app.native_chat_act("draft", &json!({ "text": "/st" })).expect("draft");
        assert_eq!(app.chat_draft, "/st");
        let snapshot = app.native_chat_snapshot("c1").expect("the chat");
        assert!(snapshot.slash.iter().any(|entry| entry.name == "status"));
        // Editar: the queue goes back to the box, and SwiftUI sees a new draft
        app.queue_message("c1", "depois".into());
        app.native_chat_act("queue", &json!({ "chat": "c1", "action": "edit" })).expect("edit");
        let seq = NATIVE.with_borrow(|native| native.draft_seq);
        app.native_chat();
        assert!(NATIVE.with_borrow(|native| native.draft_seq) > seq && app.chat_draft.starts_with("depois"));
        app.native_chat_act("footer", &json!({ "chat": "c1", "mode": 1 })).expect("mode");
        app.native_chat_act("footer", &json!({ "chat": "c1", "effort": "medium" })).expect("effort");
        // stored "" when it is the agent's own default (this machine's settings): the chip says what runs
        let chips = app.native_chat_snapshot("c1").and_then(|snapshot| snapshot.chips).expect("chips");
        assert_eq!((app.chats[0].mode, chips.effort.as_str()), (1, "medium"));
        app.native_chat_act("send", &json!({ "chat": "c1", "text": "vai\n" })).expect("send");
        assert!(app.chat_draft.is_empty() && NATIVE.with_borrow(|native| native.sends == [("c1".to_string(), "vai".to_string())]));
        assert!(app.native_chat_act("nada", &json!({})).is_err());
        let list = app.native_chats_snapshot();
        assert_eq!(list.current.as_deref(), Some("c1"));
        assert!(list.groups.iter().flat_map(|group| &group.entries).any(|entry| entry.id == "c1" && entry.chat));
    }
}
