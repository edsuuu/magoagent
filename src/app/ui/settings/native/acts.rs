//! The verbs Swift sends (`ui.settings.*`): a row's new value, a button, the shortcut recorder and the two questions.

use super::*;

impl DrawingApp {
    /// `ui.settings.<verb>` from Swift.
    pub(crate) fn native_settings_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        forget();
        let id = args["id"].as_str().unwrap_or_default();
        match verb {
            "tab" => {
                let index = args["index"].as_u64().ok_or("tab pede o index")? as usize;
                self.settings_tab = index.min(super::super::SETTINGS_TABS.len() - 1);
                self.settings_edit = None;
                Ok(String::new())
            }
            "close" => {
                self.settings_open = false;
                self.settings_edit = None;
                self.hotkeys.recording = None;
                self.claude_hooks_preview = None;
                self.wipe_confirm = false;
                Ok(String::new())
            }
            "set" => self.settings_swift_set(id, &args["value"]),
            "press" => self.settings_swift_press(id, args),
            "hotkey" => self.settings_swift_hotkey(args),
            "preview_yes" => {
                self.claude_hooks_confirm();
                Ok(String::new())
            }
            "preview_no" => {
                self.claude_hooks_preview = None;
                Ok(String::new())
            }
            "wipe_yes" => {
                self.wipe_confirm = false;
                self.wipe_all_data().map(|()| String::new()).map_err(|why| format!("Nada foi apagado: {why}."))
            }
            "wipe_no" => {
                self.wipe_confirm = false;
                Ok(String::new())
            }
            other => Err(format!("ui.settings.{other}: verbo desconhecido")),
        }
    }

    /// A row's new value (`set`): the egui page's assignment for that row.
    fn settings_swift_set(&mut self, id: &str, value: &Value) -> Result<String, String> {
        let (on, index, number, words) = (
            value.as_bool().unwrap_or(false),
            value.as_u64().unwrap_or(0) as usize,
            value.as_i64().unwrap_or(0),
            value.as_str().unwrap_or_default().to_string(),
        );
        let prefs = &mut self.prefs;
        if let Some(rest) = id.strip_prefix("preset.") {
            let (at, field) = rest.split_once('.').ok_or("preset.N.campo")?;
            let preset = prefs.presets.get_mut(at.parse::<usize>().map_err(|error| error.to_string())?).ok_or("preset desconhecido")?;
            match field {
                "on" => preset.on = on,
                "name" => preset.name = words,
                "command" => preset.command = words,
                "icon" => preset.icon = crate::app::ui::TERMINAL_ICONS.get(index).copied().unwrap_or_default().into(),
                _ => return Err(format!("campo do preset desconhecido: {field}")),
            }
            return Ok(String::new());
        }
        match id {
            "theme" => prefs.theme = [Theme::System, Theme::Light, Theme::Dark][index.min(2)],
            "language" => prefs.language = index.min(super::super::LANGUAGES.len() - 1),
            "frame_rate" => prefs.frame_rate = FRAME_RATES[index.min(FRAME_RATES.len() - 1)],
            "start_in_chat" => prefs.start_in_chat = index == 1,
            "run_at_login" => {
                crate::app::system::run_at_login::set_run_at_login(on)?;
                prefs.run_at_login = on;
            }
            "close_to_background" => prefs.close_to_background = on,
            "island" => prefs.island = on,
            "island_place" => self.put_island(*Place::ALL.get(index).ok_or("lugar desconhecido")?),
            "island_close_after" => prefs.island_close_after = super::super::general::CLOSE_AFTER[index.min(3)],
            "island_compact_after" => prefs.island_compact_after = super::super::general::COMPACT_AFTER[index.min(4)],
            "paper" => prefs.paper = index == 1,
            "box_overlap" => prefs.box_overlap = index == 1,
            "window_outline" => prefs.window_outline = WindowOutline::ALL[index.min(WindowOutline::ALL.len() - 1)],
            "show_hints" => prefs.show_hints = on,
            "wheel" => (prefs.wheel_auto, prefs.mouse_wheel_zooms) = (index == 0, index == 2),
            "zoom_step" => prefs.zoom_step = number.clamp(2, 50) as u8,
            "ligatures" => prefs.ligatures = on,
            "hidden_shown" => prefs.hidden_shown = words,
            "terminal_theme" => prefs.terminal.theme = crate::terminal::theme_choices().get(index).ok_or("tema desconhecido")?.0.to_string(),
            "terminal_font" => {
                prefs.terminal.font_size = number.clamp(*crate::terminal::FONT_SIZES.start() as i64, *crate::terminal::FONT_SIZES.end() as i64) as f32
            }
            "shell" => {
                let shells = crate::app::terminal::prefs::detected_shells();
                prefs.terminal.shell = if index == 0 { String::new() } else { shells.get(index - 1).ok_or("shell desconhecido")?.1.clone() };
            }
            "copy_on_select" => prefs.terminal.copy_on_select = on,
            "terminal_history" => prefs.terminal_history = [1_000, 5_000, 10_000][index.min(2)],
            "agent_mode" => prefs.agent_mode = [AgentMode::Terminal, AgentMode::Chat][index.min(1)],
            "usage_stop" => prefs.usage_stop = [70, 80, 90, 0][index.min(3)],
            "chat_agent" => prefs.chat_agent = ["claude", "codex"][index.min(1)].into(),
            "chat_folder" => prefs.chat_folder = words,
            "chat_mode" => prefs.chat_mode = index.min(crate::app::chat::direct::CHAT_MODES.len() - 1),
            "style_skill" => prefs.style_skill = words,
            "chat_autocompact" => {
                prefs.chat_autocompact = if index == 0 {
                    String::new()
                } else {
                    crate::app::chat::context::COMPACT_CHOICES.get(index - 1).ok_or("tamanho desconhecido")?.to_string()
                }
            }
            "chat_panel" => prefs.chat_panel = on,
            "agent_hooks" => prefs.agent_hooks = on,
            "voice_engine" => (prefs.voice_codex, prefs.voice_openai) = (index == 0, index == 2),
            "codex_voice" => prefs.codex_voice = crate::voice::codex::CODEX_VOICES.get(index).ok_or("voz desconhecida")?.0.into(),
            "openai_voice" => prefs.openai_voice = crate::voice::openai::OPENAI_VOICES.get(index).ok_or("voz desconhecida")?.0.into(),
            "openai_key" => prefs.openai_key = words,
            "studio_url" | "studio_key" => {
                if id == "studio_url" {
                    prefs.voice_studio_url = words
                } else {
                    prefs.voice_studio_key = words
                }
                // another address or key: the list is asked again
                *self.studio_voices.lock().unwrap() = crate::voice::studio::StudioVoices::Idle;
            }
            "studio_voice" => {
                let found = match &*self.studio_voices.lock().unwrap() {
                    crate::voice::studio::StudioVoices::Done(found) => found.get(index).map(|(id, _)| id.clone()),
                    _ => None,
                };
                prefs.voice_studio_voice = found.ok_or("voz desconhecida")?;
            }
            "voice_mic" => {
                let mics = crate::voice::input_names();
                prefs.voice_mic = if index == 0 { String::new() } else { mics.get(index - 1).ok_or("microfone desconhecido")?.clone() };
            }
            "voice_pause" => prefs.voice_pause = number.clamp(4, 16) as f32 / 4.0,
            "ok_mago" => prefs.ok_mago = on,
            "speak_answers" => prefs.speak_answers = on,
            "github" => prefs.integrations.github = on,
            "spotify" => prefs.integrations.spotify = on,
            "hermes" => {
                prefs.integrations.hermes = on;
                self.integrations.hermes_confirm = false;
            }
            "whatsapp" => prefs.integrations.whatsapp = on,
            "luna_dances" => prefs.integrations.luna_dances = on,
            "spotify_client_id" => prefs.integrations.spotify_client_id = words,
            "whatsapp_base" => {
                prefs.integrations.whatsapp_base = words;
                self.integrations.whatsapp.forget();
            }
            "update_check" => prefs.update_check = index.min(2) as u8,
            other => return Err(format!("ajuste desconhecido: {other}")),
        }
        Ok(String::new())
    }

    /// A button (`press`): what the egui page's button does.
    fn settings_swift_press(&mut self, id: &str, args: &Value) -> Result<String, String> {
        let arg = args["arg"].as_str().unwrap_or_default().to_string();
        let typed = args["value"].as_str().unwrap_or_default().to_string();
        let toast = |app: &mut Self, said: Result<String, String>| app.toast(said.unwrap_or_else(|why| why));
        if let Some(rest) = id.strip_prefix("preset.") {
            let (at, op) = rest.split_once('.').ok_or("preset.N.ação")?;
            let at: usize = at.parse().map_err(|error: std::num::ParseIntError| error.to_string())?;
            let presets = &mut self.prefs.presets;
            match op {
                "up" if at > 0 && at < presets.len() => presets.swap(at, at - 1),
                "down" if at + 1 < presets.len() => presets.swap(at, at + 1),
                "remove" if at < presets.len() => drop(presets.remove(at)),
                _ => {}
            }
            return Ok(String::new());
        }
        match id {
            "tour" => self.start_tour(),
            "island_reset" => {
                self.reset_island();
                self.toast("A ilha voltou ao topo da tela.");
            }
            "presets_add" => self.prefs.presets.push(Preset::new("", "")),
            "presets_reset" => self.prefs.presets = default_presets(),
            "mcp_register" => {
                let agent =
                    crate::app::system::mcp_register::MCP_AGENTS.iter().find(|(name, _)| *name == arg).map(|(name, _)| *name).ok_or("agente desconhecido")?;
                crate::app::system::mcp_register::mcp_register_soon(agent, &Context::default());
            }
            "style_skill_open" => {
                if let Err(why) = self.show_in_tree(&self.prefs.style_skill.clone()) {
                    self.toast(why);
                }
            }
            "voice_play" => {
                use crate::voice::studio::{StudioServer, StudioVoices};
                let failed = match &*self.studio_voices.lock().unwrap() {
                    StudioVoices::Failed(why) => Some(why.clone()),
                    _ => None,
                };
                let sample = "Olá! Eu sou a Luna, a assistente do MagoAgent.";
                match (self.prefs.voice_openai, crate::voice::openai::key(&self.prefs), failed) {
                    (true, Some(key), _) => self.voice_sample = crate::voice::openai::say(&key, &self.prefs.openai_voice, sample),
                    (true, None, _) => self.toast("Ponha a chave da OpenAI em Ajustes › Voz."),
                    (false, _, Some(why)) => self.toast(why),
                    (false, _, None) => self.voice_sample = StudioServer::from_prefs(&self.prefs).say(sample),
                }
            }
            "studio_refresh" => *self.studio_voices.lock().unwrap() = crate::voice::studio::StudioVoices::Idle,
            "call_toggle" => self.voice_toggle(),
            "hooks_install" if arg == "claude" => self.claude_hooks_open_preview(),
            "hooks_install" => {
                let said = self.act_codex_hooks("install");
                toast(self, said);
            }
            "hooks_remove" => {
                let said = if arg == "claude" { self.act_claude_hooks("remove") } else { self.act_codex_hooks("remove") };
                toast(self, said);
            }
            "github_save" => {
                let said = self.github_save_token(&typed);
                toast(self, said);
            }
            "github_test" => {
                self.github_test();
            }
            "github_remove" => {
                let said = self.github_remove_token().map_err(|why| format!("O token não foi removido: {why}"));
                toast(self, said);
            }
            "spotify_connect" => {
                let said = self.spotify_connect();
                toast(self, said);
            }
            "spotify_cancel" => self.spotify_cancel_login(),
            "spotify_disconnect" => {
                let said = self.spotify_disconnect().map_err(|why| format!("A conta não foi desconectada: {why}"));
                toast(self, said);
            }
            "spotify_previous" | "spotify_play_pause" | "spotify_next" => {
                use crate::app::integrations::spotify::Command;
                self.spotify_control(match id {
                    "spotify_previous" => Command::Previous,
                    "spotify_next" => Command::Next,
                    _ => Command::PlayPause,
                });
            }
            "hermes_confirm" => self.integrations.hermes_confirm = true,
            "hermes_cancel" => self.integrations.hermes_confirm = false,
            "hermes_install" | "hermes_remove" => {
                self.integrations.hermes_confirm = false;
                let said = self.act_hermes(id.trim_start_matches("hermes_"));
                toast(self, said);
            }
            "whatsapp_save" => {
                let said = self.whatsapp_save_credential(&typed);
                toast(self, said);
                self.integrations.whatsapp.forget();
            }
            "whatsapp_test" => {
                if !self.whatsapp_test() {
                    self.toast("URL inválida: precisa ser http://127.0.0.1:porta ou http://localhost:porta");
                }
            }
            "whatsapp_remove" => {
                let said = self.whatsapp_remove_credential().map_err(|why| format!("A credencial não foi removida: {why}"));
                toast(self, said);
                self.integrations.whatsapp.forget();
            }
            "open" => crate::app::browser::open_external(&arg),
            "open_path" => super::super::data::open_path(std::path::Path::new(&arg)),
            "wipe" => self.wipe_confirm = true,
            "update_check" => self.update_check(false),
            "update_install" => {
                // an update that closes and reopens the app: a second press, after the warning
                if matches!(&self.updates.state, UpdateState::Available(release) if installs_in_place(release)) {
                    self.updates.confirming = true;
                } else if let Err(why) = self.update_install() {
                    self.toast(why);
                }
            }
            "update_install_now" => {
                if let Err(why) = self.update_install() {
                    self.toast(why);
                }
            }
            "update_cancel" => self.updates.confirming = false,
            other => return Err(format!("botão desconhecido: {other}")),
        }
        Ok(String::new())
    }

    /// The shortcut recorder (`hotkey`): `op` record, cancel, save, clear, default, switch ({on}) or key ({key, cmd, alt,
    /// shift}: a key pressed while recording, named as egui names it).
    fn settings_swift_hotkey(&mut self, args: &Value) -> Result<String, String> {
        use super::super::hotkeys::Ask;
        let id = args["id"].as_str().unwrap_or_default();
        let def = hotkeys::find(id).ok_or_else(|| format!("atalho desconhecido: {id}"))?;
        let flag = |name: &str| args[name].as_bool().unwrap_or(false);
        match args["op"].as_str().unwrap_or_default() {
            "record" => self.hotkey_ask(def, Ask::Record),
            "cancel" => self.hotkey_ask(def, Ask::Cancel),
            "save" => self.hotkey_ask(def, Ask::Save),
            "clear" => self.hotkey_ask(def, Ask::Clear),
            "default" => self.hotkey_ask(def, Ask::Default),
            "switch" => self.hotkey_ask(def, Ask::Switch(flag("on"))),
            "key" => {
                let name = args["key"].as_str().unwrap_or_default();
                let key = egui::Key::from_name(name)
                    .or_else(|| egui::Key::ALL.iter().copied().find(|key| key.name().eq_ignore_ascii_case(name)))
                    .ok_or_else(|| format!("tecla desconhecida: {name}"))?;
                let modifiers = egui::Modifiers { alt: flag("alt"), ctrl: false, shift: flag("shift"), mac_cmd: flag("cmd"), command: flag("cmd") };
                // only the shortcut being recorded takes the key
                if self.hotkeys.recording.as_ref().is_some_and(|editing| editing.id == def.id)
                    && let Some(mut editing) = self.hotkeys.recording.take()
                    && !editing.press(keys::record(key, modifiers), &mut self.prefs.shortcuts)
                {
                    self.hotkeys.recording = Some(editing);
                }
            }
            other => return Err(format!("hotkey {other}: operação desconhecida")),
        }
        Ok(String::new())
    }
}
