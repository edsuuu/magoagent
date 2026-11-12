use super::*;

impl DrawingApp {
    pub(super) fn act_chat_list(&self) -> String {
        if self.chats.is_empty() {
            return "Não há chats.".into();
        }
        let line = |c: &crate::app::chat::direct::Chat| {
            let open = if self.chat_agent.as_deref() == Some(c.id.as_str()) { " (aberto)" } else { "" };
            let busy = if self.chat_runs.contains_key(&c.id) { ", respondendo" } else { "" };
            format!("{}{open}: {} em {}{busy}", c.name, c.agent, c.cwd)
        };
        self.chats.iter().map(line).collect::<Vec<_>>().join("\n")
    }

    pub(super) fn act_chat_search(&mut self, query: &str, chat: &str, limit: usize) -> Result<String, String> {
        if query.trim().is_empty() {
            return Err("Diga o texto a procurar.".into());
        }
        let selected: Vec<_> = if chat.is_empty() { self.chats.iter().map(|c| c.id.clone()).collect() } else { vec![self.chat_named(chat)?] };
        for id in &selected {
            self.chat_refresh(id);
        }
        let mut lines = Vec::new();
        for id in selected {
            let Some(info) = self.chats.iter().find(|c| c.id == id) else { continue };
            let Some(transcript) = self.transcripts.get(&id) else { continue };
            let messages: Vec<_> = transcript
                .items
                .iter()
                .filter_map(|item| match item {
                    crate::transcript::ChatItem::User(s) => Some(("Pessoa", s.as_str())),
                    crate::transcript::ChatItem::Agent(s) => Some((info.agent.as_str(), s.as_str())),
                    crate::transcript::ChatItem::Error(s) => Some(("Erro", s.as_str())),
                    crate::transcript::ChatItem::System(s) => Some(("Sistema", s.as_str())),
                    _ => None,
                })
                .collect();
            let input: Vec<_> = messages.iter().map(|(who, text)| crate::app::chat::search::SearchMessage { who, text }).collect();
            for hit in crate::app::chat::search::search(&input, query) {
                lines.push(format!("{} · {}: {}", info.name, hit.who, hit.excerpt));
                if lines.len() >= limit {
                    break;
                }
            }
            if lines.len() >= limit {
                break;
            }
        }
        Ok(if lines.is_empty() { format!("Nada encontrado para \"{query}\".") } else { lines.join("\n") })
    }

    pub(super) fn act_chat_new(&mut self, agent: &str, folder_path: &str) -> Result<String, String> {
        let installed = self.installed_agents();
        if installed.is_empty() {
            return Err("Nenhum agente instalado (claude ou codex).".into());
        }
        let mut form = crate::app::chat::direct::NewChat::from_prefs(&self.prefs, &installed);
        if !agent.is_empty() {
            form.agent =
                installed.iter().position(|p| p.command.split_whitespace().next() == Some(agent)).ok_or_else(|| format!("{agent} não está instalado."))?;
        }
        if !folder_path.is_empty() {
            folder(folder_path).filter(|p| p.is_dir()).ok_or_else(|| format!("A pasta {folder_path} não existe."))?;
            form.cwd = folder_path.to_string();
        }
        self.create_chat(&form, &installed);
        self.show_chat();
        Ok(format!("Chat novo aberto: {}.", self.chats.last().map_or("", |c| c.name.as_str())))
    }

    pub(super) fn act_voice(&mut self, state: &str) -> Result<String, String> {
        use std::sync::atomic::Ordering;
        let muted = self.voice.shared.muted.load(Ordering::Relaxed);
        match state {
            "dictate_on" | "dictate_off" if self.dictation.is_some() == (state == "dictate_on") => {}
            "dictate_on" | "dictate_off" => self.dictate_toggle(),
            "on" if !crate::app::voice::CALL_ON => return Err("O chat por voz está desligado por enquanto.".into()),
            "on" | "off" if self.voice.on == (state == "on") => {}
            "on" | "off" => self.voice_toggle(),
            "mute" | "unmute" if !self.voice.on => return Err("A chamada não está ligada.".into()),
            "mute" | "unmute" if muted != (state == "mute") => self.voice_mute(),
            "speakers_off" | "speakers_on" if self.voice.speakers_off != (state == "speakers_off") => self.voice_speakers(),
            "mute" | "unmute" | "speakers_off" | "speakers_on" => {}
            other => return Err(format!("estado desconhecido: {other}")),
        }
        Ok("Chamada ajustada.".into())
    }
}
