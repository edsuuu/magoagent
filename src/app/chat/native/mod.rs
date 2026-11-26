//! The chat on the Mac in SwiftUI (macos/MagoUI/Sources/MagoUI/Chat): each frame the conversations list goes out as the
//! topic "chats" and every chat on screen (the one open, the ones a SwiftUI view watches) as "chat:<id>"; what SwiftUI
//! asks comes back as `ui.chat.<verb>` and runs the same functions the egui chat runs. The egui chat steps aside
//! (`chat_screen`, `mini_chat_ui`).

mod context;
mod list;
mod snapshot;
mod words;

pub(crate) use snapshot::Chips;

use crate::app::chat::footer::{FooterAct, agent_default};
use crate::app::chat::list::running::chip;
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::transcript::ChatItem;
use snapshot::pick;
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
    /// Files picked with + (Anexar imagem), for the box in the next frame (`chat_add_files` needs its context).
    attach: Vec<PathBuf>,
    /// Buscar na conversa: the chat searched and the words.
    search: Option<(String, String)>,
    /// The Contexto panel's Compactar and Tentar, for the next frame (they need its context): the chat and which.
    context: Vec<(String, &'static str)>,
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

fn state_key(state: &AgentState) -> &'static str {
    match chip(state).0 {
        "Trabalhando" => "working",
        "Esperando" => "waiting",
        _ => "ready",
    }
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
        crate::app::native::publish("chat_context", &self.context_snapshot());

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

        let picked = NATIVE.with_borrow_mut(|native| std::mem::take(&mut native.attach));

        self.chat_add_files(ctx, picked, false);

        for (id, action) in NATIVE.with_borrow_mut(|native| std::mem::take(&mut native.context)) {
            match action {
                "compact" => _ = self.chat_submit(ctx, &id, "/compact", "/compact"),
                _ => self.spawn_servers(ctx, &id),
            }
        }

        // the Contexto panel's reads, as `chat_screen` asks them each frame
        if let Some(id) = self.chat_agent.clone().filter(|id| self.chat_open && self.context_wanted(id)) {
            self.context_tick(ctx, &id, self.prefs.chat_panel);
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

                // a session outside the app: its conversation, read from its file (as the list's `act.outside`)
                if id.starts_with(crate::app::maestro::outside::PILL_PREFIX) {
                    self.island_show(&id, self.now);
                    self.chat_agent = Some(id);
                    self.chat_polled = f64::MIN;
                } else {
                    self.open_agent(&id);
                }

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

                // a session outside the app runs in a window of its own: that one comes
                if id.starts_with(crate::app::maestro::outside::PILL_PREFIX) {
                    return match self.outside_terminal(&id) {
                        true => Ok("Terminal na frente.".into()),
                        false => Err("Não deu para trazer a janela desse terminal: responda nele.".into()),
                    };
                }

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
            // a Maestro's fold in the list ("3 agentes")
            "team" => {
                let id = chat()?;

                self.set_team_open(&id, args["open"].as_bool().unwrap_or(true));
                Ok(String::new())
            }
            // a button on a row that answers it there (`row_answers`), by its place
            "answer" => {
                let id = chat()?;
                let (answers, guarded) = self.row_answers(&id, self.now);
                let at = args["index"].as_u64().unwrap_or(u64::MAX) as usize;
                let Some((_, _, answer)) = answers.into_iter().nth(at).filter(|_| !guarded) else { return Ok(String::new()) };

                self.answer_row(&id, answer);
                Ok("Respondido.".into())
            }
            "ask" => self.native_chat_decide(&chat()?, &text("action"), args),
            // + (Anexar imagem): the files picked in SwiftUI's panel; × on one of them
            "attach" => {
                let paths = args["paths"].as_array().into_iter().flatten().filter_map(Value::as_str).map(PathBuf::from);

                NATIVE.with_borrow_mut(|native| native.attach.extend(paths));
                Ok(String::new())
            }
            "unattach" => {
                self.unattach(args["index"].as_u64().unwrap_or(u64::MAX) as usize);
                Ok(String::new())
            }
            // Buscar na conversa: only the messages that say it ("" ends the search)
            "search" => {
                let (id, query) = (chat()?, text("query"));

                NATIVE.with_borrow_mut(|native| native.search = (!query.trim().is_empty()).then_some((id, query)));
                Ok(String::new())
            }
            // the Contexto panel (context_view.rs): Compactar, Tentar on a connector that failed and "Compactar
            // automaticamente em"; opening, closing and Atualizar are the MCP's `chat_context`
            "context" => {
                let id = chat()?;
                let info = self.chat_ctx.info.get_mut(&id);

                match text("action").as_str() {
                    "autocompact" => {
                        let value = text("value");

                        if !value.is_empty() && !crate::app::chat::context::COMPACT_CHOICES.contains(&value.as_str()) {
                            return Err(format!("valor desconhecido: {value}"));
                        }

                        self.prefs.chat_autocompact = value;

                        if let Some(info) = info {
                            info.dirty = true;
                        }
                    }
                    "compact" => NATIVE.with_borrow_mut(|native| native.context.push((id, "compact"))),
                    // Tentar: the `claude mcp list` tries the connectors again
                    "retry" => {
                        if let Some(info) = info {
                            (info.retried, info.listing) = (true, true);
                            NATIVE.with_borrow_mut(|native| native.context.push((id, "retry")));
                        }
                    }
                    other => return Err(format!("action desconhecida: {other}. Use compact, retry ou autocompact.")),
                }

                Ok(String::new())
            }
            // Copiar on a step's card: the whole of what it printed (the card shows its end)
            "step_log" => {
                let (id, step) = (chat()?, text("step"));

                let found = self.transcripts.get(&id).and_then(|transcript| {
                    transcript.items.iter().find_map(|item| match item {
                        ChatItem::Tool(found) if found.id == step => Some(found.log.clone()),
                        _ => None,
                    })
                });

                found.ok_or_else(|| "Passo não encontrado.".into())
            }
            other => Err(format!("ui.chat.{other} não existe")),
        }
    }

    /// The amber card over the box, as decide.rs's `decide_card` answers it: a question's tab, an option, another
    /// answer typed, Permitir/Enviar (allow), Negar/Recusar (deny) or Responder no terminal; clicks in its first moments
    /// are ignored as there.
    fn native_chat_decide(&mut self, id: &str, action: &str, args: &Value) -> Result<String, String> {
        let mut asks = self.person_asks.borrow_mut();
        let Some(index) = asks.iter().position(|ask| ask.chat == id) else { return Err("Nada esperando uma decisão.".into()) };
        let ask = &mut asks[index];
        let last = ask.questions.len().saturating_sub(1);
        let step = ask.step.min(last);

        if action != "other" && ask.guarded(self.now) {
            return Ok(String::new());
        }

        let decided = match action {
            "tab" => {
                ask.step = (args["index"].as_u64().unwrap_or(0) as usize).min(last);
                None
            }
            "pick" => ask.pick(args["index"].as_u64().unwrap_or(u64::MAX) as usize).then_some(true),
            "other" => {
                if let Some(other) = ask.other.get_mut(step) {
                    *other = args["text"].as_str().unwrap_or_default().to_string();
                }

                None
            }
            "allow" if ask.questions.is_empty() || ask.complete() => Some(true),
            "allow" => return Err("Responda todas as perguntas.".into()),
            "deny" => Some(false),
            "terminal" => {
                if let Some(source) = &ask.source {
                    self.outside.declined.borrow_mut().push((source.session.clone(), now_ms()));
                }

                asks.remove(index).send_terminal();

                return Ok("Respondido no terminal.".into());
            }
            other => return Err(format!("action desconhecida: {other}")),
        };

        if let Some(allow) = decided {
            asks.remove(index).send(allow);

            return Ok(if allow { "Respondido.".into() } else { "Recusado.".into() });
        }

        Ok(String::new())
    }

    /// The row of agent `id`: a terminal, a chat (a draft too) or a session outside.
    fn row_of(&self, id: &str) -> Option<AgentRow> {
        self.agent_rows().into_iter().chain(self.draft_rows()).find(|row| row.id == id).or_else(|| self.outside_row(id))
    }
}

#[cfg(test)]
mod tests;
