use super::*;

fn parse_island_color(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    (hex.len() == 6).then(|| u32::from_str_radix(hex, 16).ok()).flatten().map(|rgb| [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
}

/// The GNOME extension that tells the island's eyes where the pointer is (island/gnome.rs): put in, taken out or how it is.
fn gnome_cursor(action: &str) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    if crate::app::island::gnome::wanted() {
        use crate::app::island::gnome::{Status, install, remove, status};
        return match action {
            "install" | "instalar" => install(),
            "remove" | "remover" => remove(),
            _ => Ok(match status() {
                Status::On => "A extensão do GNOME está ativa: os olhos da ilha seguem o mouse na tela toda.",
                Status::Installed => "A extensão do GNOME está instalada; o GNOME a carrega no próximo login.",
                Status::Missing => "A extensão do GNOME não está instalada.",
            }
            .into()),
        };
    }
    let _ = action;
    Err("A extensão é do GNOME com Wayland; nos outros sistemas os olhos já veem o mouse.".into())
}

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
            ("voice", "voicestudio" | "studio") => (self.prefs.voice_codex, self.prefs.voice_openai) = (false, false),
            ("voice", "openai") => (self.prefs.voice_codex, self.prefs.voice_openai) = (false, true),
            ("voice_studio_url", url) if !url.trim().is_empty() => self.prefs.voice_studio_url = url.trim().to_string(),
            ("voice_studio_voice", voice) if !voice.trim().is_empty() => self.prefs.voice_studio_voice = voice.trim().to_string(),
            ("voice_studio_key", key) => self.prefs.voice_studio_key = key.trim().to_string(),
            ("close_to_background", "on" | "ligado") => self.prefs.close_to_background = true,
            ("close_to_background", "off" | "desligado") => self.prefs.close_to_background = false,
            ("avatar", "on" | "ligado") => self.prefs.avatar = true,
            ("avatar", "off" | "desligado") => self.prefs.avatar = false,
            ("claude_hooks", action @ ("install" | "instalar" | "remove" | "remover" | "status")) => return self.act_claude_hooks(action),
            ("codex_hooks", action @ ("install" | "instalar" | "remove" | "remover" | "status")) => return self.act_codex_hooks(action),
            ("island", "on" | "ligado") => self.prefs.island = true,
            ("island", "off" | "desligado") => self.prefs.island = false,
            ("island_sound", "on" | "ligado") => self.prefs.island_sound = true,
            ("island_sound", "off" | "desligado") => self.prefs.island_sound = false,
            ("island_volume", volume) if volume.parse::<f32>().is_ok_and(|v| (0.0..=0.2).contains(&v)) => {
                self.prefs.island_volume = volume.parse().unwrap_or(0.1)
            }
            ("frame_rate", rate) if rate.parse::<u16>().is_ok_and(|rate| FRAME_RATES.contains(&rate)) => self.prefs.frame_rate = rate.parse().unwrap_or(60),
            ("gnome_cursor", action @ ("install" | "instalar" | "remove" | "remover" | "status")) => return gnome_cursor(action),
            ("island_close_after", seconds) if ["5", "10", "15", "30"].contains(&seconds) => self.prefs.island_close_after = seconds.parse().unwrap_or(15),
            ("island_color", color) if parse_island_color(color).is_some() => self.prefs.island_color = parse_island_color(color).unwrap_or([0, 0, 0]),
            (name, value) if name.starts_with("shortcut_") => return self.act_shortcut(&name["shortcut_".len()..], value),
            ("avatar_follow", "on" | "ligado") => self.avatar_track_window(true),
            ("avatar_follow", "off" | "desligado") => self.avatar_track_window(false),
            ("max_agents", n) if n.parse::<usize>().is_ok_and(|n| n <= 50) => self.prefs.max_agents = n.parse().unwrap_or(0),
            ("usage_stop", n) if n.parse::<u8>().is_ok_and(|n| n <= 100) => self.prefs.usage_stop = n.parse().unwrap_or(0),
            ("agent_mode", mode) if AgentMode::parse(mode).is_some() => self.prefs.agent_mode = AgentMode::parse(mode).unwrap_or_default(),
            ("recruit_model", "" | "cli" | "padrão") => self.prefs.recruit_model.clear(),
            ("recruit_model", model) if model.chars().all(|c| c.is_ascii_alphanumeric() || "-._[]".contains(c)) => self.prefs.recruit_model = model.to_string(),
            ("window_outline", name) if WindowOutline::parse(name).is_some() => self.prefs.window_outline = WindowOutline::parse(name).unwrap_or_default(),
            ("style_skill", path) => self.prefs.style_skill = path.trim().to_string(),
            ("hidden_shown", names) => self.prefs.hidden_shown = names.trim().to_string(),
            ("zoom_step", n) if n.parse::<u8>().is_ok_and(|n| (2..=50).contains(&n)) => self.prefs.zoom_step = n.parse().unwrap_or(10),
            ("voice_mic", "auto" | "automático" | "") => self.prefs.voice_mic.clear(),
            ("voice_mic", mic) => {
                let mics = crate::voice::input_names();
                let found = mics
                    .iter()
                    .find(|m| m.to_lowercase().contains(&mic.to_lowercase()))
                    .ok_or_else(|| format!("Microfone \"{mic}\" não encontrado. Há: {}.", mics.join("; ")))?;
                self.prefs.voice_mic = found.clone();
                return Ok(format!("O microfone agora é {found}."));
            }
            ("chat_agent", agent @ ("claude" | "codex")) => self.prefs.chat_agent = agent.into(),
            ("chat_folder", folder) => self.prefs.chat_folder = folder.trim().to_string(),
            ("chat_mode", "auto") => self.prefs.chat_mode = 0,
            ("chat_mode", "bypass") => self.prefs.chat_mode = 1,
            ("chat_autocompact", "padrão" | "padrao" | "") => self.prefs.chat_autocompact.clear(),
            ("chat_autocompact", size) if crate::app::chat::context::COMPACT_CHOICES.iter().any(|c| c.eq_ignore_ascii_case(size)) => {
                self.prefs.chat_autocompact = size.to_uppercase().replace('K', "k")
            }
            ("chat_panel", "on" | "ligado") => self.prefs.chat_panel = true,
            ("chat_panel", "off" | "desligado") => self.prefs.chat_panel = false,
            ("agent_hooks", "on" | "ligado") => self.prefs.agent_hooks = true,
            ("agent_hooks", "off" | "desligado") => self.prefs.agent_hooks = false,
            ("ok_mago", "on" | "ligado") => self.prefs.ok_mago = true,
            ("ok_mago", "off" | "desligado") => self.prefs.ok_mago = false,
            ("speak_answers", "on" | "ligado") => self.prefs.speak_answers = true,
            ("speak_answers", "off" | "desligado") => self.prefs.speak_answers = false,
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
        // the app's own window, whichever window draws now
        for command in std::mem::take(&mut self.viewport_commands) {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, command);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_skill_sets_the_path_as_given() {
        let mut app = DrawingApp::default();
        assert!(app.act_setting("style_skill", " C:/Skill/SKILL.md ").is_ok());
        assert_eq!(app.prefs.style_skill, "C:/Skill/SKILL.md");
    }
}

#[cfg(test)]
mod chat_tests {
    use super::*;

    #[test]
    fn the_chat_settings_answer_to_the_mcp() {
        let mut app = DrawingApp::default();
        app.act_setting("chat_agent", "codex").unwrap();
        app.act_setting("chat_mode", "bypass").unwrap();
        app.act_setting("chat_folder", " C:/Projetos ").unwrap();
        app.act_setting("chat_autocompact", "200k").unwrap();
        app.act_setting("chat_panel", "on").unwrap();
        app.act_setting("agent_hooks", "on").unwrap();
        let prefs = &app.prefs;
        assert_eq!((prefs.chat_agent.as_str(), prefs.chat_mode, prefs.chat_folder.as_str()), ("codex", 1, "C:/Projetos"));
        assert_eq!((prefs.chat_autocompact.as_str(), prefs.chat_panel, prefs.agent_hooks), ("200k", true, true));
        app.act_setting("chat_autocompact", "1m").unwrap();
        assert_eq!(app.prefs.chat_autocompact, "1M");
        app.act_setting("chat_autocompact", "padrão").unwrap();
        assert!(app.prefs.chat_autocompact.is_empty());
        app.act_setting("ok_mago", "off").unwrap();
        app.act_setting("speak_answers", "off").unwrap();
        assert!(!app.prefs.ok_mago && !app.prefs.speak_answers);
        assert!(app.act_setting("chat_agent", "gemini").is_err());
        assert!(app.act_setting("chat_autocompact", "3k").is_err());
    }
}
