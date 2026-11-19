//! Parar tudo: every agent stops where it is and nothing makes one go on by itself afterwards. Only a new message of
//! the person's starts one again. The chat's /parar (and "para tudo" written alone), the MCP's `stop_all` and the
//! question when the window closes with agents at work (system/close_ask.rs) all come here.

use crate::app::ui::agents::AgentState;
use crate::prelude::*;

/// What written alone in a chat's box means Parar tudo.
const STOP_WORDS: [&str; 8] = ["para tudo", "pare tudo", "parar tudo", "parem tudo", "para tudo agora", "pare tudo agora", "para todos", "parem todos"];

/// Whether `text` is only a request to stop everything ("Para tudo!", "pare tudo").
pub(crate) fn stop_words(text: &str) -> bool {
    let words: Vec<String> = text.split_whitespace().map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).collect();

    STOP_WORDS.contains(&words.join(" ").as_str())
}

impl DrawingApp {
    /// The agents at work now (chats answering, agents in terminals on a turn): their names.
    pub(crate) fn agents_at_work(&self) -> Vec<String> {
        let rows = self.agent_rows().into_iter().filter(|row| matches!(row.state, AgentState::Working(_)) || self.chat_runs.contains_key(&row.id));

        rows.map(|row| self.agent_label(&row.id)).collect()
    }

    /// Parar tudo: the questions between agents end unanswered (a stopped recruit's "parou sem responder" would wake
    /// the chat that recruited it), the queue and what waits to be sent go, every chat stops and every agent at work in a
    /// terminal gets Esc. What it did, said.
    pub(crate) fn stop_all(&mut self) -> String {
        let asks = self.end_asks("parado pela pessoa");
        let queued = self.chat_queue.len() + self.chat_outbox.len() + self.chat_cmd_outbox.len();

        self.chat_queue.clear();
        self.chat_outbox.clear();
        self.chat_cmd_outbox.clear();
        let chats: Vec<String> = self.chat_runs.keys().cloned().collect();

        for id in &chats {
            self.chat_stop(id);
        }

        let terminals: Vec<String> = self
            .terminals
            .iter()
            .filter(|(id, session)| session.is_ok() && !self.is_chat(id) && matches!(self.agent_state(id), AgentState::Working(_)))
            .map(|(id, _)| id.clone())
            .collect();

        for id in &terminals {
            if let Some(Ok(session)) = self.terminals.get(id) {
                session.handle.write(vec![0x1b]);
            }
        }

        let mut said = vec![format!("Tudo parado: {} chat(s) e {} agente(s) em terminal.", chats.len(), terminals.len())];

        if queued > 0 {
            said.push(format!("{queued} mensagem(ns) da fila descartada(s)."));
        }

        if asks > 0 {
            said.push(format!("{asks} pergunta(s) entre agentes encerrada(s) sem resposta."));
        }

        said.push("Nada continua sem uma mensagem sua.".into());
        said.join(" ")
    }
}
