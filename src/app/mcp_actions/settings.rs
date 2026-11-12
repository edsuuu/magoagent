use super::*;

impl DrawingApp {
    pub(super) fn act_setting(&mut self, name: &str, value: &str) -> Result<String, String> {
        match (name, value) {
            ("theme", "light" | "claro") => self.prefs.theme = Theme::Light,
            ("theme", "dark" | "escuro") => self.prefs.theme = Theme::Dark,
            ("theme", "system" | "sistema") => self.prefs.theme = Theme::System,
            ("terminal_history", "1000" | "1.000") => self.prefs.terminal_history = 1_000,
            ("terminal_history", "5000" | "5.000") => self.prefs.terminal_history = 5_000,
            ("terminal_history", "10000" | "10.000") => self.prefs.terminal_history = 10_000,
            ("voice", "codex") => self.prefs.voice_codex = true,
            ("voice", "openai") => (self.prefs.voice_codex, self.prefs.voice_openai) = (false, true),
            ("voice", "local") => (self.prefs.voice_codex, self.prefs.voice_openai) = (false, false),
            ("avatar", "on" | "ligado") => self.prefs.avatar = true,
            ("avatar", "off" | "desligado") => self.prefs.avatar = false,
            ("avatar_follow", "on" | "ligado") => self.avatar_track_window(true),
            ("avatar_follow", "off" | "desligado") => self.avatar_track_window(false),
            ("max_agents", n) if n.parse::<usize>().is_ok_and(|n| n <= 50) => self.prefs.max_agents = n.parse().unwrap_or(0),
            ("usage_stop", n) if n.parse::<u8>().is_ok_and(|n| n <= 100) => self.prefs.usage_stop = n.parse().unwrap_or(0),
            ("agent_mode", mode) if AgentMode::parse(mode).is_some() => self.prefs.agent_mode = AgentMode::parse(mode).unwrap_or_default(),
            ("recruit_model", "" | "cli" | "padrão") => self.prefs.recruit_model.clear(),
            ("recruit_model", model) if model.chars().all(|c| c.is_ascii_alphanumeric() || "-._[]".contains(c)) => self.prefs.recruit_model = model.to_string(),
            ("zoom_step", n) if n.parse::<u8>().is_ok_and(|n| (2..=50).contains(&n)) => self.prefs.zoom_step = n.parse().unwrap_or(10),
            ("favorite_folder", path) if !path.trim().is_empty() => {
                let on = self.prefs.folders.toggle_favorite(path);
                return Ok(format!("{} {} pastas favoritas.", path.trim(), if on { "entrou nas" } else { "saiu das" }));
            }
            _ => return Err(format!("ajuste desconhecido: {name} = {value}")),
        }
        Ok(format!("{name} agora é {value}."))
    }

    pub(super) fn act_portal_tab(&mut self, portal: &str, action: &str, url: &str, index: Option<usize>) -> Result<String, String> {
        let at = self.node_named(portal)?;
        let spec = self.scene.elements[at].portal.as_ref().ok_or_else(|| format!("{portal} não é um navegador"))?;
        let (mut tabs, active) = (spec.tab_urls(), spec.active_tab());
        let (tabs, active) = match action {
            "new" => {
                tabs.push(crate::app::browser::normalize_url(url));
                let last = tabs.len() - 1;
                (tabs, last)
            }
            "close" if tabs.len() == 1 => return Err("é a única aba; feche o navegador com close_window".into()),
            "close" => {
                let i = index.unwrap_or(active).min(tabs.len() - 1);
                tabs.remove(i);
                // the one after the closed tab moves into its place
                let next = if active > i { active - 1 } else { active };
                let last = tabs.len() - 1;
                (tabs, next.min(last))
            }
            "select" => {
                let i = index.ok_or("diga o index da aba")?;
                if i >= tabs.len() {
                    return Err(format!("o navegador tem {} aba(s)", tabs.len()));
                }
                (tabs, i)
            }
            other => return Err(format!("ação desconhecida: {other}")),
        };
        self.snapshot();
        if let Some(spec) = self.scene.elements[at].portal.as_mut() {
            spec.set_tabs(tabs, active);
        }
        Ok(format!("Abas de {portal} ajustadas."))
    }

    /// Each frame: the window's own commands, the
    /// messages for chats.
    pub(crate) fn actions_tick(&mut self, ctx: &Context) {
        for (id, text) in std::mem::take(&mut self.chat_outbox) {
            self.chat_send(ctx, &id, &text);
        }
        for (id, text) in std::mem::take(&mut self.chat_cmd_outbox) {
            self.chat_submit(ctx, &id, &text, &text);
        }
        for command in std::mem::take(&mut self.viewport_commands) {
            ctx.send_viewport_cmd(command);
        }
    }
}
