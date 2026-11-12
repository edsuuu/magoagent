//! What other agents ask a chat over the bridge (`magoagent ask`, the MCP's ask_agent): each
//! question waits in line until the chat is free, runs as a message of the chat that says whose it
//! is, and the agent's last words of that turn go back to who asked. `check` reads the end of the
//! conversation.

use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use crate::transcript::ChatItem;
use std::sync::mpsc::Sender;

/// A question for a chat, waiting its turn or being answered.
pub(crate) struct ChatAsk {
    pub(crate) chat: String,
    /// Who asked: its id on the canvas ("" outside it) and its name.
    from: String,
    asker: String,
    text: String,
    reply: Sender<Result<String, String>>,
    /// The chat's agent runs on it now.
    running: bool,
    /// What that run said last (its answer), or how it failed.
    answer: Option<Result<String, String>>,
}

/// Between the asker's name and the question, in the message the chat gets.
const RETURNS: &str = "\" (a sua resposta volta para quem perguntou):\n\n";

impl ChatAsk {
    /// The message as the chat gets it: whose question it is, then the question.
    pub(crate) fn prompt(&self) -> String {
        format!("Pergunta de \"{}{RETURNS}{}", self.asker, self.text)
    }
}

/// `prompt` read back: who asked and the question; None for a message of the person's.
pub(crate) fn asked_by(message: &str) -> Option<(&str, &str)> {
    message.strip_prefix("Pergunta de \"")?.split_once(RETURNS)
}

impl DrawingApp {
    /// The bridge's `ask` on a chat: the question joins the chat's line.
    pub(crate) fn ask_chat(&mut self, id: &str, from: &str, asker: &str, text: &str, reply: Sender<Result<String, String>>) {
        if !self.is_chat(id) {
            let _ = reply.send(Err("não existe mais".into()));
            return;
        }
        let ask = ChatAsk { chat: id.into(), from: from.into(), asker: asker.into(), text: text.into(), reply, running: false, answer: None };
        self.chat_asks.push(ask);
    }

    /// Each frame: an answered question goes back to who asked; the first in line of a chat that
    /// answers nothing now is sent to it; the questions of a chat removed are given up.
    pub(crate) fn chat_asks_tick(&mut self, ctx: &Context) {
        let mut i = 0;
        while i < self.chat_asks.len() {
            let ask = &self.chat_asks[i];
            let id = ask.chat.clone();
            if ask.running {
                // its turn ended (or was stopped) once the chat runs nothing
                if !self.chat_runs.contains_key(&id) {
                    let ask = self.chat_asks.remove(i);
                    let _ = ask.reply.send(ask.answer.unwrap_or_else(|| Err("parou sem responder".into())));
                    continue;
                }
            } else if !self.is_chat(&id) {
                let _ = self.chat_asks.remove(i).reply.send(Err("foi removido antes de responder".into()));
                continue;
            } else if matches!(self.chat_state(&id), Some(AgentState::Idle(_))) && !self.chat_asks[..i].iter().any(|a| a.chat == id) {
                let prompt = ask.prompt();
                if !self.chat_send(ctx, &id, &prompt) {
                    // why is what `chat_send` just said (the agent is not installed, the folder is gone)
                    let why = format!("não pôde receber a pergunta: {}", self.status);
                    let _ = self.chat_asks.remove(i).reply.send(Err(why));
                    continue;
                }
                self.ask_sent(i);
            }
            i += 1;
        }
    }

    /// Question `i` went to its chat: its answer is what that run says. Kept as the asker's, so the
    /// chat shows it as that agent's and not the person's, and an event says who asked whom.
    fn ask_sent(&mut self, i: usize) {
        let ask = &mut self.chat_asks[i];
        ask.running = true;
        if let Some(bridge) = &self.bridge {
            let mut shared = bridge.shared.lock().unwrap();
            shared.asked.push((ask.chat.clone(), ask.prompt(), ask.asker.clone()));
            let extra = shared.asked.len().saturating_sub(50);
            shared.asked.drain(..extra);
            shared.events.push((ask.from.clone(), ask.chat.clone(), tool_summary("pergunta", &json!({ "prompt": ask.text }))));
        }
    }

    /// What the chat's agent printed (`poll_chats`): while it answers a question, its last words so
    /// far are the answer (Claude Code's `result`, Codex's last message), or why it failed.
    pub(crate) fn chat_heard(&mut self, id: &str, event: &Value, codex: bool) {
        let Some(ask) = self.chat_asks.iter_mut().find(|a| a.running && a.chat == id) else { return };
        let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
        let said = if codex {
            match (event["type"].as_str(), event["item"]["type"].as_str()) {
                (Some("item.completed"), Some("agent_message")) => Some(Ok(text(&event["item"]["text"]))),
                (Some("turn.failed"), _) => Some(Err(format!("falhou: {}", text(&event["error"]["message"])))),
                _ => None,
            }
        } else if event["type"] == "result" {
            let words = text(&event["result"]);
            Some(if event["is_error"] == true { Err(format!("falhou: {words}")) } else { Ok(words) })
        } else {
            None
        };
        if said.is_some() {
            ask.answer = said;
        }
    }

    /// The bridge's `check` on a chat: whether it answers now, and the end of its conversation.
    pub(crate) fn chat_check(&mut self, id: &str) -> Result<String, String> {
        let chat = self.chats.iter().find(|c| c.id == id).cloned().ok_or("esse chat não existe mais")?;
        self.chat_refresh(id);
        let state = if matches!(self.chat_state(id), Some(AgentState::Working(_))) { "respondendo" } else { "parado" };
        let said = |item: &ChatItem| match item {
            ChatItem::User(text) => Some(format!("Pessoa: {text}")),
            ChatItem::Agent(text) => Some(format!("Maestro: {text}")),
            ChatItem::Error(text) => Some(format!("Erro: {text}")),
            _ => None,
        };
        // ponytail: the last 6 messages whole; a `lines` argument if agents ask for more or less
        let mut last: Vec<String> = self.transcripts.get(id).map(|t| t.items.iter().rev().filter_map(said).take(6).collect()).unwrap_or_default();
        last.reverse();
        let talk = if last.is_empty() { "(sem mensagens ainda)".to_string() } else { last.join("\n\n") };
        Ok(format!("Maestro (chat \"{}\", {state}):\n\n{talk}", chat.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat::direct::NewChat;
    use std::sync::mpsc::channel;

    /// An app with the bridge up and one chat of an agent this machine does not have (a test never
    /// runs the real `claude`).
    fn app_with_chat() -> (DrawingApp, String) {
        let mut app = DrawingApp { bridge: Bridge::start(None).ok(), ..Default::default() };
        let installed = [Preset { name: "Fantasma".into(), command: "mago-agente-que-nao-existe".into(), on: true }];
        app.create_chat(&NewChat { agent: 0, cwd: String::new(), mode: 0 }, &installed);
        let id = app.chat_agent.clone().unwrap();
        (app, id)
    }

    #[test]
    fn a_question_waits_in_the_chats_line_goes_as_the_askers_and_its_answer_comes_back() {
        let (mut app, id) = app_with_chat();
        let (reply, answer) = channel();
        app.ask_chat(&id, "t1", "Sábia", "quanto é 2+2?", reply);
        // in line, said whose it is
        let prompt = app.chat_asks[0].prompt();
        assert_eq!(prompt, "Pergunta de \"Sábia\" (a sua resposta volta para quem perguntou):\n\nquanto é 2+2?");
        assert!(answer.try_recv().is_err() && !app.chat_asks[0].running);
        assert_eq!((asked_by(&prompt), asked_by("oi")), (Some(("Sábia", "quanto é 2+2?")), None));
        // sent: the chat shows it as that agent's question, and the event goes from the asker to the chat
        app.ask_sent(0);
        app.chat_agent = Some(id.clone());
        assert_eq!(app.asked_of(), [(prompt, "Sábia".to_string())]);
        let shared = app.bridge.as_ref().unwrap().shared.clone();
        assert_eq!(shared.lock().unwrap().events, [("t1".to_string(), id.clone(), "pergunta: quanto é 2+2?".to_string())]);
        // what its agent says last on that turn is the answer (Claude Code's `result`)
        app.chat_heard(&id, &json!({ "type": "assistant", "message": {} }), false);
        app.chat_heard(&id, &json!({ "type": "result", "is_error": false, "result": "4" }), false);
        // the turn over (the chat runs nothing): it goes back, and the line is free
        app.chat_asks_tick(&Context::default());
        assert_eq!(answer.try_recv(), Ok(Ok("4".to_string())));
        assert!(app.chat_asks.is_empty());
    }

    #[test]
    fn a_question_the_chat_cannot_take_or_never_answers_says_why() {
        let (mut app, id) = app_with_chat();
        let ctx = Context::default();
        // its agent is not on this machine: the question never starts, and the asker hears why
        let (reply, answer) = channel();
        app.ask_chat(&id, "", "um agente fora do canvas", "oi", reply);
        app.chat_asks_tick(&ctx);
        let why = answer.try_recv().unwrap().unwrap_err();
        assert!(why.starts_with("não pôde receber a pergunta: ") && why.contains("não foi encontrado"), "{why}");
        assert!(app.chat_asks.is_empty());
        assert!(app.bridge.as_ref().unwrap().shared.lock().unwrap().asked.is_empty(), "never sent: no question of the chat's");
        // a turn that failed, one stopped before a word (Esc), a Codex one: each says how it ended
        for (codex, event, heard) in [
            (false, json!({ "type": "result", "is_error": true, "result": "sem crédito" }), Err("falhou: sem crédito".to_string())),
            (false, json!({ "type": "system", "subtype": "init" }), Err("parou sem responder".to_string())),
            (true, json!({ "type": "item.completed", "item": { "type": "agent_message", "text": "pronto" } }), Ok("pronto".to_string())),
            (true, json!({ "type": "turn.failed", "error": { "message": "limite" } }), Err("falhou: limite".to_string())),
        ] {
            let (reply, answer) = channel();
            app.ask_chat(&id, "t1", "Sábia", "oi", reply);
            app.ask_sent(0);
            app.chat_heard(&id, &event, codex);
            app.chat_asks_tick(&ctx);
            assert_eq!(answer.try_recv(), Ok(heard));
        }
        // the chat removed with a question in line: given up, not left hanging; one for no chat is refused
        let (reply, answer) = channel();
        app.ask_chat(&id, "t1", "Sábia", "oi", reply);
        app.remove_chat(&id);
        app.chat_asks_tick(&ctx);
        assert_eq!(answer.try_recv(), Ok(Err("foi removido antes de responder".to_string())));
        let (reply, answer) = channel();
        app.ask_chat(&id, "t1", "Sábia", "oi", reply);
        assert_eq!(answer.try_recv(), Ok(Err("não existe mais".to_string())));
        assert!(app.chat_check(&id).is_err());
    }

    #[test]
    fn check_reads_the_end_of_the_chats_conversation() {
        let (mut app, id) = app_with_chat();
        app.chat_transcript(&id).items.extend([ChatItem::User("oi".into()), ChatItem::Agent("olá".into())]);
        let name = app.chats[0].name.clone();
        assert_eq!(app.chat_check(&id).unwrap(), format!("Maestro (chat \"{name}\", parado):\n\nPessoa: oi\n\nMaestro: olá"));
    }
}
