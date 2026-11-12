use super::*;

impl DrawingApp {
    /// Ligar / Desligar.
    pub(crate) fn voice_toggle(&mut self) {
        if !CALL_ON && !self.voice.on {
            return;
        }
        if self.voice.on {
            self.voice.shut();
            self.voice.status.clear();
            // the bar goes back to rest: the call's last state (listening, muted) ended with it
            self.avatar_set(AvatarState::Idle);
            self.ring(crate::voice::CALL_END);
            return;
        }
        let codex = self.prefs.voice_codex && cfg!(not(target_os = "linux"));
        let openai = crate::voice::openai::key(&self.prefs);
        let engine = call_engine(codex, codex && crate::voice::codex::codex_program().is_some(), self.prefs.voice_openai, openai.is_some());
        match engine {
            CallEngine::Codex => return self.codex_call(),
            CallEngine::NoKey => {
                self.voice.status = "Ponha a chave da OpenAI em Ajustes › Agentes › Chaves de API, ou use a voz local em Ajustes › Voz.".into();
                return;
            }
            CallEngine::OpenAi | CallEngine::Local => {}
        }
        self.voice = Voice::default();
        self.voice.on = true;
        self.voice.state = VoiceState::Preparing;
        // Codex's voice was the choice and Codex isn't here: the call goes on without it
        if codex {
            let other = if engine == CallEngine::OpenAi { "da OpenAI" } else { "local" };
            self.voice.status = format!("O Codex não foi encontrado: a chamada usa a voz {other}.");
        }
        // with no chat shown what is heard would reach no one
        self.call_chat(false);
        self.voice_follow();
        // one microphone at a time
        self.dictation = None;
        let setup = VoiceSetup {
            voice: self.prefs.voice.clone(),
            language: self.voice_language().into(),
            gpu: self.prefs.voice_gpu,
            pause: self.prefs.voice_pause,
            openai: openai.filter(|_| engine == CallEngine::OpenAi),
        };
        let (rx, _port, server, piper) = crate::voice::start(self.voice.shared.clone(), setup);
        self.voice.events = Some(rx);
        self.voice_server = server;
        self.voice_piper = piper;
    }

    /// The language whisper hears: the Maestro's; "follow the person" hears the voice's (guessing the
    /// language of a short phrase gets it wrong).
    pub(crate) fn voice_language(&self) -> &str {
        match self.prefs.language {
            1 => "pt",
            2 => "en",
            _ => self.prefs.voice.get(..2).unwrap_or("pt"),
        }
    }

    /// The call on Codex's voice: `codex app-server` in the chat's folder, going on with the chat's
    /// Codex thread, with the Maestro's MCP and instructions; the page (voice/page.rs) brings the sound.
    pub(super) fn codex_call(&mut self) {
        self.voice = Voice::default();
        self.voice.on = true;
        self.voice.state = VoiceState::Preparing;
        let chat = self.call_chat(true);
        // a Claude chat (or a terminal) stays its agent's: the voice only speaks for it
        self.voice.relay = chat.as_ref().is_none_or(|c| c.agent != "codex");
        self.voice_follow();
        let cwd = chat.as_ref().and_then(|c| crate::app::chat::direct::folder(&c.cwd)).or_else(projects_dir).unwrap_or_else(std::env::temp_dir);
        let _ = fs::create_dir_all(&cwd);
        let setup = crate::voice::codex::CodexSetup {
            cwd,
            resume: chat.as_ref().filter(|c| c.agent == "codex").and_then(|c| c.session.clone()),
            instructions: chat.as_ref().map(|c| crate::app::chat::tools::system_prompt(&self.prefs, c)).unwrap_or_default(),
            args: chat.as_ref().map(|c| crate::app::chat::tools::maestro_args("codex", &self.prefs, c)).unwrap_or_default(),
            full_access: chat.as_ref().is_some_and(|c| c.mode == 1),
            relay: self.voice.relay,
        };
        match crate::voice::codex::CodexVoice::start(&setup) {
            Ok(codex) => self.voice.codex = Some(codex),
            Err(why) => self.voice_fail(why),
        }
    }

    /// The chat the call goes on in: the one shown (none for a terminal), else a new one as
    /// Ajustes presets it (its agent, folder and leave), which is then shown. With no agent on the
    /// PATH, Codex's voice (`codex`) still gets a Codex chat: the Codex inside its app runs it.
    pub(super) fn call_chat(&mut self, codex: bool) -> Option<crate::app::chat::direct::Chat> {
        if self.chat_agent.is_none() {
            self.start_chat(true);
        }
        if let Some(id) = &self.chat_agent {
            return self.chats.iter().find(|c| &c.id == id).cloned();
        }
        if !codex {
            return None;
        }
        let cwd = projects_dir().map(|p| p.display().to_string()).unwrap_or_default();
        let chat = crate::app::chat::direct::Chat {
            id: uuid::Uuid::new_v4().simple().to_string(),
            agent: "codex".into(),
            name: format!("Codex · {}", crate::app::chat::direct::folder_label(&cwd)),
            cwd,
            session: None,
            mode: 0,
            model: String::new(),
            effort: String::new(),
            recruit: false,
        };
        self.chats.push(chat.clone());
        self.chat_agent = Some(chat.id.clone());
        self.save_chats();
        Some(chat)
    }

    /// One of the call's sounds; it is kept (and the microphone held) while it is heard.
    pub(super) fn ring(&mut self, sound: crate::voice::Sound) {
        // the tests stay quiet
        if cfg!(test) {
            return;
        }
        let end = self.now + crate::voice::sound_length(sound) as f64;
        self.voice.tone = crate::voice::chime(sound).map(|s| (s, end));
    }

    /// The call ends on an error: its sound, and why under the box.
    /// One that couldn't start shows why over the conversation (voice/orb.rs).
    pub(crate) fn voice_fail(&mut self, why: String) {
        let launching = self.voice.on && self.voice.state == VoiceState::Preparing;
        self.voice.shut();
        self.avatar_set(AvatarState::Idle);
        if launching {
            self.voice.launch_failed = Some(if why == crate::voice::MIC_DENIED {
                ("Permissão do microfone negada".into(), "Permita o acesso ao microfone nas configurações do sistema para iniciar um chat por voz".into(), true)
            } else {
                ("Não foi possível iniciar o chat por voz".into(), if why.is_empty() { "Tente novamente ou volte".into() } else { why.clone() }, false)
            });
        }
        self.voice.status = why;
        self.ring(crate::voice::CALL_ERROR);
    }

    /// Every frame on Codex's voice: its events, and the page's offer once the thread is there.
    /// False when the call ended.
    pub(super) fn codex_tick(&mut self, ctx: &Context) -> bool {
        use crate::transcript::ChatItem;
        use crate::voice::codex::CodexEvent;
        let events: Vec<CodexEvent> = self.voice.codex.as_ref().map(|c| c.events.try_iter().collect()).unwrap_or_default();
        for event in events {
            match event {
                CodexEvent::Thread(id) => {
                    // a Codex chat without a thread goes on with the call's
                    if let Some(chat) = self.voice.agent.as_ref().and_then(|a| self.chats.iter_mut().find(|c| &c.id == a))
                        && chat.agent == "codex"
                        && chat.session.is_none()
                    {
                        chat.session = Some(id.clone());
                    }
                    if let Some(codex) = self.voice.codex.as_mut() {
                        codex.thread = Some(id);
                    }
                }
                CodexEvent::Answer(sdp) => self.voice.answer = Some(sdp),
                CodexEvent::Started => {
                    self.voice.status.clear();
                    self.ring(crate::voice::CALL_START);
                    self.voice.state = VoiceState::Listening;
                    self.voice.connected_at = Some(self.now);
                }
                // the call shows in the chat: what was asked and what Codex answered
                CodexEvent::Heard(text) | CodexEvent::Answered(text) if text.trim().is_empty() => {}
                // for another agent: what was heard is its message (and the app's commands)
                CodexEvent::Heard(text) if self.voice.relay => self.voice_heard(ctx, &text),
                CodexEvent::Answered(_) | CodexEvent::Working(_) | CodexEvent::Doing(_) if self.voice.relay => {}
                CodexEvent::Heard(text) => {
                    // the voice itself asks before muting; the app shows where the call stands
                    self.mute_step(&text);
                    if self.voice.standby && matches!(crate::voice::voice_command(&text, &self.voice_name()), Some(VoiceCmd::Unmute | VoiceCmd::Ask(_))) {
                        self.voice.standby = false;
                    }
                    self.voice_note(ChatItem::User(text));
                }
                CodexEvent::Answered(text) => self.voice_note(ChatItem::Agent(text)),
                // what Codex is doing shows under the box while it works
                CodexEvent::Working(on) => {
                    self.voice.working = on;
                    self.voice.status = if on { "Pensando".into() } else { String::new() };
                }
                CodexEvent::Doing(what) => self.voice.status = what,
                // once it is on, an error on the way is said and the call goes on (it ends on Closed)
                CodexEvent::Failed(why) if self.voice.connected_at.is_some() => self.voice.status = format!("Aviso da voz: {why}"),
                CodexEvent::Failed(why) => {
                    self.voice_fail(format!("Erro no chat por voz: {why}"));
                    return false;
                }
                CodexEvent::Closed(reason) => {
                    // closed on "error": the error said before is the why
                    let said = self.voice.status.strip_prefix("Aviso da voz: ").map(str::to_string);
                    let reason = if reason.as_deref() == Some("error") { said.or(reason) } else { reason };
                    self.voice_fail(match reason {
                        Some(reason) => format!("Chat por voz encerrado: {reason}"),
                        None => "Chat por voz encerrado inesperadamente".into(),
                    });
                    return false;
                }
            }
        }
        if let Some(codex) = self.voice.codex.as_mut()
            && codex.thread.is_some()
            && let Some(offer) = self.voice.offer.take()
        {
            codex.call(&offer, &self.prefs.codex_voice);
        }
        true
    }

    /// A line of the call in the chat it is for, counted as already said.
    pub(super) fn voice_note(&mut self, item: crate::transcript::ChatItem) {
        let Some(id) = self.voice.agent.clone() else { return };
        let transcript = self.transcripts.entry(id.clone()).or_insert_with(|| crate::transcript::Transcript::new(PathBuf::new()));
        transcript.items.push(item);
        self.voice.spoken = transcript.items.len();
        self.save_chat_items(&id);
    }

    pub(crate) fn voice_mute(&mut self) {
        let muted = !self.voice.shared.muted.load(Ordering::Relaxed);
        self.voice.shared.muted.store(muted, Ordering::Relaxed);
        self.ring(if muted { crate::voice::CALL_MUTE } else { crate::voice::CALL_UNMUTE });
    }

    /// Silenciar / Ativar som dos alto-falantes: what is being said stops.
    pub(crate) fn voice_speakers(&mut self) {
        self.voice.speakers_off = !self.voice.speakers_off;
        if self.voice.speakers_off {
            self.voice.hush();
        }
        self.ring(if self.voice.speakers_off { crate::voice::CALL_MUTE } else { crate::voice::CALL_UNMUTE });
    }

    /// The call follows the chat shown: what it already said isn't read out.
    pub(super) fn voice_follow(&mut self) {
        if self.voice.agent != self.chat_agent {
            self.voice.agent = self.chat_agent.clone();
            self.voice.spoken = self.chat_agent.as_ref().and_then(|id| self.transcripts.get(id)).map_or(0, |t| t.items.len());
            self.voice.hush();
        }
    }
}
