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

    /// The Chats area (`chats_view`): open on the chip `filter` and the search `query` (None keeps it), with what it lists,
    /// each Maestro's agents under it; `team` opens or folds a Maestro's agents (chat/list/team.rs) first. Or the canvas back.
    pub(super) fn act_chats_view(&mut self, state: &str, filter: &str, query: Option<&str>, team: Option<(&str, bool)>) -> Result<String, String> {
        use crate::app::chat::list::running::{Show, chip, kept, running_counts, space_groups};
        use crate::app::chat::list::team::{team_summary, under_maestros};

        let folded = match team {
            Some((maestro, open)) => format!("{}\n", self.act_team(maestro, open)?),
            None => String::new(),
        };

        if state == "close" {
            self.show_canvas();

            return Ok(format!("{folded}De volta ao canvas."));
        }

        if !filter.is_empty() {
            self.chat_listing.show = Show::named(filter).ok_or_else(|| format!("Filtro desconhecido: {filter}. Use all, working, waiting ou ready."))?;
        }

        if let Some(query) = query {
            self.chat_listing.filter = query.trim().to_string();
        }

        self.show_chat();
        let entries = self.running_entries();
        let (running, waiting) = running_counts(entries.iter().map(|entry| entry.state));
        let show = self.chat_listing.show;
        let shown = kept(&entries, show, &self.chat_listing.filter.to_lowercase());
        let mut report = format!("{folded}Chats · {running} rodando · {waiting} esperando (filtro: {})", show.label());

        if !self.chat_listing.filter.is_empty() {
            report += &format!(", busca: {}", self.chat_listing.filter);
        }

        let order: Vec<String> = self.spaces.list.iter().map(|space| space.id.clone()).collect();
        let keyed: Vec<(&str, Option<&crate::app::ui::agents::AgentRow>)> = shown.iter().map(|entry| (entry.id.as_str(), entry.row.as_ref())).collect();
        let parents = self.team_parents(&keyed);

        let line = |entry: &crate::app::chat::list::running::Entry, indent: &str| {
            let when = entry.ago.map(|seconds| format!(" ({})", crate::app::ui::agents::ago(seconds))).unwrap_or_default();

            format!("\n{indent}- {} — {} — {}{when}", entry.name, chip(&entry.state).0, entry.last)
        };

        for (space, members) in space_groups(&shown, &order) {
            report += &format!("\n\n{}:", self.group_look(&space).0);

            for (top, team) in under_maestros(&members, &parents) {
                report += &line(shown[top], "");

                if !team.is_empty() {
                    let states: Vec<_> = team.iter().map(|&index| shown[index].state).collect();
                    let open = if self.team_open(&shown[top].id) { "abertos" } else { "recolhidos" };

                    report += &format!("\n  agentes dele: {} ({open} na lista)", team_summary(&states));
                }

                for index in team {
                    report += &line(shown[index], "  ");
                }
            }
        }

        if shown.is_empty() {
            report += "\n\nNada com esse filtro.";
        }

        Ok(report)
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

    /// `chat_queue`: what waits in the queues (`list`), or one agent's queue sent now (`send_now`) or back in the box
    /// (`edit`), as the buttons under it do.
    pub(super) fn act_chat_queue(&mut self, agent: &str, action: &str) -> Result<String, String> {
        if matches!(action, "" | "list") && agent.is_empty() {
            if self.chat_queue.is_empty() {
                return Ok("Nada na fila.".into());
            }

            return Ok(self.chat_queue.iter().map(|(id, text)| format!("{}: {text}", self.agent_label(id))).collect::<Vec<_>>().join("\n\n"));
        }

        let id = match agent {
            "" => self.chat_agent.clone().ok_or("Nenhuma conversa aberta: diga o agente.")?,
            name => self.agent_named(name)?.0,
        };

        match action {
            "" | "list" => {
                let queued: Vec<&str> = self.chat_queue.iter().filter(|(to, _)| *to == id).map(|(_, text)| text.as_str()).collect();

                Ok(if queued.is_empty() {
                    format!("Nada na fila de {}.", self.agent_label(&id))
                } else {
                    format!("Na fila de {}: {}", self.agent_label(&id), queued.join("\n\n"))
                })
            }
            "send_now" => self.send_queued_now(&id),
            "edit" => {
                if !self.has_queued(&id) {
                    return Err(format!("Nada na fila de {}.", self.agent_label(&id)));
                }

                self.chat_agent = Some(id.clone());
                self.unqueue(&id);
                Ok("A mensagem da fila voltou para a caixa da conversa.".into())
            }
            other => Err(format!("action desconhecida: {other}. Use list, send_now ou edit.")),
        }
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
            "dictate_on" if !crate::app::voice::DICTATION_ON => return Err("O ditado está desligado por enquanto.".into()),
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
