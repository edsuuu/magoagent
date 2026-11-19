//! The topic "chat:<id>": one conversation as the egui chat lays it out (chat/mod.rs `chat_item`, conversation.rs,
//! steps.rs, decide.rs, composer.rs, footer.rs): the messages with the agent's words cut as markdown.rs cuts them and the
//! files they name found (filetree/link.rs), the runs of steps with their cards, the band, the amber cards, the queue,
//! the team at work and the chips.

use super::words::{Part, file_card, parts, picture};
use super::{NATIVE, state_key};
use crate::app::chat::ask::{asked_by, is_answer};
use crate::app::chat::direct::{CHAT_MODES, folder, folder_label, is_new_session_mark};
use crate::app::chat::footer::{Choice, agent_default, chip_folder, efforts, label, models, more_models, pretty, tuning_names};
use crate::app::chat::list::elapsed;
use crate::app::chat::segments::{is_picture, resolve};
use crate::app::chat::steps::{card_output, counter, run_summary, running_step, short_tool};
use crate::app::chat::{EARLIER, window_start};
use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use crate::transcript::{ChatItem, Step};
use std::path::Path;

/// One conversation.
#[derive(Serialize, PartialEq, Debug)]
pub(crate) struct ChatSnapshot {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) agent: String,
    pub(super) folder: String,
    /// Claude Code: its asterisk in the header's square and its orange in the band; else the terminal's mark.
    pub(super) claude: bool,
    /// working, waiting or ready.
    pub(super) state: &'static str,
    pub(super) status: String,
    pub(super) is_chat: bool,
    /// No message went yet: the provider may still change.
    pub(super) fresh: bool,
    pub(super) providers: Vec<Provider>,
    /// How many messages are above the ones sent ("Mostrar mensagens anteriores").
    pub(super) earlier: usize,
    pub(super) items: Vec<ItemSnapshot>,
    pub(super) band: Option<Band>,
    pub(super) ask: Option<AskSnapshot>,
    /// A terminal stopped on a question only it answers (steps.rs `waiting_card`).
    pub(super) waiting: Option<WaitingSnapshot>,
    pub(super) queue: Vec<String>,
    /// The agents it recruited at work now, a line each (conversation.rs `team_lines`).
    pub(super) team: Vec<String>,
    pub(super) draft: String,
    pub(super) draft_seq: u64,
    pub(super) hint: String,
    pub(super) slash: Vec<SlashEntry>,
    pub(super) chips: Option<Chips>,
    /// Buscar na conversa: the words searched ("" not searching).
    pub(super) query: String,
    /// The pictures that go with the next message: where each came from and its path.
    pub(super) attached: Vec<(String, String)>,
    /// "2 imagens · 648 KB".
    pub(super) attached_total: String,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct Provider {
    pub(super) name: String,
    pub(super) program: String,
    pub(super) on: bool,
}

/// A message, or a run of tool steps; `id` is its place in the conversation (stable while it streams).
#[derive(Serialize, PartialEq, Debug, Default)]
pub(super) struct ItemSnapshot {
    pub(super) id: String,
    /// user, agent, system, divider, error, tools, image.
    pub(super) kind: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) parts: Vec<Part>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) steps: Vec<StepSnapshot>,
    /// A run of tools: "4 ferramentas · Edit, Bash ×2 · 46 s", and whether it starts folded.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) summary: String,
    pub(super) folded: bool,
    /// What a run of steps wrote (a picture, a file's card), under it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) media: Vec<Part>,
    /// Another agent's message: "Resposta de Fulano", "Pergunta de…", "Mensagem de…"; on an image, "" when the person
    /// sent it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) hint: Option<&'static str>,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct StepSnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) what: String,
    pub(super) note: String,
    pub(super) failed: bool,
    /// done, running or waiting (running while its agent waits on the person).
    pub(super) live: &'static str,
    /// Answered: a check in green, else a quieter one.
    pub(super) ended: bool,
    pub(super) diff: Option<(usize, usize)>,
    /// A running command's last line, under its row.
    pub(super) output: String,
    /// Its card: "Bash · terminou em 3 s" and its folder; its files; its input (under "comando" or "entrada"), the end
    /// of what it printed and why it failed.
    pub(super) head: String,
    pub(super) files: Vec<String>,
    pub(super) input_label: &'static str,
    pub(super) input: String,
    pub(super) log: String,
    /// The log was cut: Copiar asks for the whole of it (`ui.chat.step_log`).
    pub(super) log_cut: bool,
    pub(super) error: String,
}

/// The band over the box while the agent works.
#[derive(Serialize, PartialEq, Debug)]
pub(super) struct Band {
    pub(super) tool: String,
    pub(super) what: String,
    pub(super) counter: Option<(u32, u32)>,
    /// The thinking line when no tool runs ("Fulano está trabalhando…").
    pub(super) thinking: String,
    pub(super) seconds: u64,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct AskSnapshot {
    /// "Fulano precisa de você", "Fulano pede permissão: Bash".
    pub(super) head: String,
    pub(super) tool: String,
    /// A permission: what it is about, in one line.
    pub(super) summary: String,
    /// A session outside the app: it can be answered in its terminal.
    pub(super) outside: bool,
    pub(super) questions: Vec<QuestionSnapshot>,
    /// The question shown, and per question the options picked and the words typed in "Outra resposta…".
    pub(super) step: usize,
    pub(super) picked: Vec<Vec<usize>>,
    pub(super) other: Vec<String>,
    /// Every question answered: Enviar shows.
    pub(super) complete: bool,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct QuestionSnapshot {
    pub(super) text: String,
    pub(super) header: String,
    pub(super) multiple: bool,
    pub(super) options: Vec<(String, String)>,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct WaitingSnapshot {
    pub(super) head: String,
    pub(super) why: String,
    /// The menu on its screen: its question, its choices and the one marked; else the screen's last lines.
    pub(super) question: String,
    pub(super) choices: Vec<String>,
    pub(super) marked: usize,
    pub(super) screen: String,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct SlashEntry {
    pub(super) name: String,
    pub(super) args: String,
    pub(super) desc: String,
}

/// The chips under the box: folder, permission, model and effort.
#[derive(Serialize, PartialEq, Debug)]
pub(super) struct Chips {
    pub(super) folder: String,
    pub(super) folder_path: String,
    /// The conversation started in its folder: no other.
    pub(super) folder_locked: bool,
    /// "Nenhuma pasta": the app's own folder.
    pub(super) no_folder: String,
    pub(super) recent: Vec<(String, String)>,
    pub(super) mode: usize,
    pub(super) modes: Vec<&'static str>,
    pub(super) tuning: String,
    pub(super) model: String,
    pub(super) models: Vec<ChoiceSnapshot>,
    pub(super) more: Vec<ChoiceSnapshot>,
    pub(super) effort: String,
    pub(super) efforts: Vec<ChoiceSnapshot>,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct ChoiceSnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) note: String,
}

fn choices(list: &[Choice]) -> Vec<ChoiceSnapshot> {
    list.iter().map(|choice| ChoiceSnapshot { id: choice.id.clone(), name: choice.name.clone(), note: choice.note.clone() }).collect()
}

fn step_snapshot(step: &Step, live: &'static str) -> StepSnapshot {
    let state = match live {
        "running" => "rodando",
        "waiting" => "aguardando você",
        _ if step.failed => "falhou",
        _ if step.ended.is_none() => "sem resposta",
        _ => "terminou",
    };

    let took = match (step.started, step.ended) {
        (Some(start), Some(end)) if end >= start => format!(" em {}", elapsed(end - start)),
        _ => String::new(),
    };

    let head = [Some(format!("{} · {state}{took}", step.name)), step.cwd.as_ref().map(|cwd| format!("Pasta: {cwd}"))].into_iter().flatten();
    let cut = card_output(&step.log);

    StepSnapshot {
        id: step.id.clone(),
        name: short_tool(&step.name).to_string(),
        what: step.what.clone(),
        note: step.note.clone().unwrap_or_default(),
        failed: step.failed,
        live,
        ended: step.ended.is_some(),
        diff: step.diff,
        output: step.output.clone().filter(|_| live != "done").unwrap_or_default(),
        head: head.collect::<Vec<_>>().join("\n"),
        files: step.files.clone(),
        input_label: if step.name == "Bash" { "comando" } else { "entrada" },
        input: if step.input.is_empty() { step.what.clone() } else { step.input.clone() },
        log_cut: cut.is_some(),
        log: cut.unwrap_or_else(|| step.log.clone()),
        error: step.error.clone().unwrap_or_default(),
    }
}

/// What picking `id` stores: "" when it is the agent's default (as footer.rs's `pick`).
pub(super) fn pick(id: &str, default: &str) -> String {
    if id == default { String::new() } else { id.to_string() }
}

impl DrawingApp {
    pub(crate) fn native_chat_snapshot(&mut self, id: &str) -> Option<ChatSnapshot> {
        let row = self.row_of(id)?;
        let chat = self.chats.iter().find(|c| c.id == id).cloned();
        // where a relative path in its words starts: a chat's folder (media.rs `chat_folder`)
        let base = chat.as_ref().and_then(|chat| folder(&chat.cwd));
        let transcript = self.transcripts.get(id);
        let asked = if self.chat_agent.as_deref() == Some(id) { self.asked_of() } else { Vec::new() };
        let waiting_now = row.state == AgentState::Waiting;
        let query = NATIVE.with_borrow(|native| native.search.clone()).filter(|(chat, _)| chat == id).map(|(_, query)| query);
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

                    item.steps = steps
                        .iter()
                        .map(|step| {
                            let live = match t.running.iter().any(|(running, _)| *running == step.id) {
                                true if waiting_now => "waiting",
                                true => "running",
                                false => "done",
                            };

                            step_snapshot(step, live)
                        })
                        .collect();

                    // what a Write made shows under the run (media.rs `step_files`)
                    item.media = steps
                        .iter()
                        .filter(|step| step.name == "Write" && step.ended.is_some() && !step.failed)
                        .filter_map(|step| step.files.first())
                        .filter(|file| is_picture(file) || resolve(file, base.as_deref()).is_file())
                        .map(|file| if is_picture(file) { picture(file, "", base.as_deref()) } else { file_card(&resolve(file, base.as_deref()), "") })
                        .collect();
                } else {
                    item = message(&t.items[index], index, from.get(&index), base.as_deref(), self.now);
                }

                items.push(item);
                index = next;
            }

            // Buscar na conversa: only the messages that say it, as `conversation` lists them
            if let Some(query) = &query {
                items = crate::app::chat::list::found(&t.items, &query.to_lowercase())
                    .into_iter()
                    .map(|index| message(&t.items[index], index, from.get(&index), base.as_deref(), self.now))
                    .collect();

                earlier = 0;
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

        let chat_name = chat.as_ref().map(|chat| chat.name.clone()).unwrap_or_default();

        let ask = self.person_asks.borrow().iter().find(|ask| ask.chat == id).map(|ask| {
            let outside = ask.source.as_ref().map(|source| source.label.clone());
            let name = outside.clone().unwrap_or_else(|| chat_name.clone());

            // as decide.rs `decide_card` heads it
            let head = match (outside.is_some(), ask.questions.is_empty()) {
                (true, true) => format!("{name} precisa de permissão: {}", ask.tool),
                (true, false) => format!("{name} está perguntando"),
                (false, true) => format!("{name} pede permissão: {}", ask.tool),
                (false, false) => format!("{name} precisa de você"),
            };

            AskSnapshot {
                head,
                tool: ask.tool.clone(),
                summary: ask.summary(),
                outside: outside.is_some(),
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
                step: ask.step.min(ask.questions.len().saturating_sub(1)),
                picked: ask.picked.clone(),
                other: ask.other.clone(),
                complete: ask.complete(),
            }
        });

        let waiting = waiting_now && ask.is_none();

        // a terminal that waits on its screen: the amber card with its menu (a chat's agent asks on the card above)
        let waiting_card = (waiting && chat.is_none()).then(|| {
            let (screen, menu, why) = self.waiting_screen(id);
            let lines: Vec<&str> = screen.lines().collect();
            let end = lines.iter().rposition(|line| !line.trim().is_empty()).map_or(0, |index| index + 1);

            WaitingSnapshot {
                head: format!("{} precisa de você", row.spec.label()),
                why,
                question: menu.as_ref().map(|menu| menu.question.join("\n")).unwrap_or_default(),
                choices: menu.as_ref().map(|menu| menu.choices.clone()).unwrap_or_default(),
                marked: menu.as_ref().map_or(0, |menu| menu.marked),
                screen: if menu.is_some() { String::new() } else { lines[end.saturating_sub(8)..end].join("\n") },
            }
        });

        let status = match row.state {
            AgentState::Working(since) => format!("Trabalhando · há {}", elapsed(self.now - since)),
            AgentState::Waiting if ask.is_some() => "Precisa de você · responda acima da caixa".into(),
            AgentState::Waiting => match self.waiting_secs(id) {
                Some(seconds) => format!("Precisa de você · esperando no terminal há {}", elapsed(seconds)),
                None => "Precisa de você · esperando no terminal".into(),
            },
            AgentState::Idle(_) => "Pronto".into(),
            other => other.label().to_string(),
        };

        let (agent, folder_path, model) = match &chat {
            Some(chat) => {
                let model = tuning_names(chat, &self.ran_model(&chat.id)).0;

                (chat.name.split(" · ").next().unwrap_or(&chat.name).to_string(), chat.cwd.clone(), model)
            }
            None => (row.spec.label().to_string(), row.spec.cwd.clone(), self.usage.agents.get(id).map(|usage| usage.model.clone()).unwrap_or_default()),
        };

        // · model · space, as the header says it
        let space = Some(self.row_space(&row)).filter(|space| !space.is_empty()).map(|space| self.space_name(&space)).unwrap_or_default();
        let status = [status, model, space].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ");
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
        let claude = crate::app::chat::header::program(&row.spec, self.agent_sessions.contains_key(id)) == Some("Claude Code");

        Some(ChatSnapshot {
            id: id.to_string(),
            title: self.chat_title(&row),
            agent,
            folder: if crate::app::chat::direct::no_folder(&folder_path) { "Nenhuma pasta".into() } else { short_home(&folder_path) },
            claude,
            state: state_key(&row.state),
            status,
            is_chat: chat.is_some(),
            fresh,
            providers,
            earlier,
            items,
            band,
            ask,
            waiting: waiting_card,
            queue: self.chat_queue.iter().filter(|(to, _)| to == id).map(|(_, text)| text.clone()).collect(),
            team: self.team_lines(id),
            draft,
            draft_seq,
            hint: hint.into(),
            slash: slash.into_iter().map(|entry| SlashEntry { name: entry.name, args: entry.args, desc: entry.desc }).collect(),
            chips,
            query: query.unwrap_or_default(),
            attached: self.attached_list().into_iter().map(|(origin, path)| (origin, path.display().to_string())).collect(),
            attached_total: self.attached_summary().unwrap_or_default(),
        })
    }
}

/// A message of the conversation (not a step), as chat/mod.rs `chat_item` draws it; `from`: the agent that asked it.
fn message(item: &ChatItem, index: usize, from: Option<&String>, base: Option<&Path>, now: f64) -> ItemSnapshot {
    let mut out = ItemSnapshot { id: index.to_string(), ..Default::default() };

    match item {
        ChatItem::User(text) => {
            let kind = match asked_by(text) {
                Some(_) if is_answer(text) => "Resposta de",
                Some(_) => "Pergunta de",
                None => "Mensagem de",
            };

            (out.kind, out.text, out.from) = ("user", text.clone(), from.map(|who| format!("{kind} {who}")));
        }
        ChatItem::Agent(text) => (out.kind, out.parts) = ("agent", parts(text, base, now)),
        ChatItem::System(text) if text == EARLIER || is_new_session_mark(text) => (out.kind, out.text) = ("divider", text.clone()),
        ChatItem::System(text) => (out.kind, out.text) = ("system", text.clone()),
        ChatItem::Error(text) => (out.kind, out.text, out.hint) = ("error", text.clone(), crate::app::chat::error_hint(text)),
        // ponytail: a picture says only its type, its bytes would go in every snapshot; a file path for it later
        ChatItem::Image { media, user, .. } => (out.kind, out.text, out.from) = ("image", media.clone(), user.then(String::new)),
        ChatItem::Tool(_) => {}
    }

    out
}

/// The folder with the home folder as "~" (header.rs `short_path`, whole: SwiftUI cuts it in the middle to fit).
fn short_home(path: &str) -> String {
    let home = std::env::home_dir().map(|home| home.display().to_string()).unwrap_or_default();

    crate::app::chat::header::short_path(path, &home, usize::MAX)
}
