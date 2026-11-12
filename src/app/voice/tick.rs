use super::*;

impl DrawingApp {
    /// Every frame while the voice is on: what the threads say, the message heard, the answer
    /// to say, the sentence being said.
    pub(crate) fn voice_tick(&mut self, ctx: &Context) {
        // the hang-up tone plays out after the call
        if self.voice.tone.as_ref().is_some_and(|(_, end)| self.now > *end) {
            self.voice.tone = None;
        }
        if !self.voice.on {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        self.voice_follow();
        if self.voice.codex.is_some() && !self.codex_tick(ctx) {
            return;
        }
        let events: Vec<VoiceEvent> = self.voice.events.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
        for event in events {
            match event {
                VoiceEvent::Ready => {
                    self.voice.status.clear();
                    self.ring(crate::voice::CALL_START);
                    self.voice.state = VoiceState::Listening;
                    self.voice.connected_at = Some(self.now);
                    self.voice.server = self.voice_server.lock().unwrap().take();
                }
                VoiceEvent::Status(text) => self.voice.status = text,
                // talked over: it stops and listens
                VoiceEvent::Interrupt => {
                    self.voice.hush();
                    self.voice.tone = None;
                    self.voice.state = VoiceState::Hearing;
                }
                VoiceEvent::Hearing(on) => {
                    if on {
                        self.voice.hush();
                    }
                    self.voice.state = if on { VoiceState::Hearing } else { VoiceState::Listening };
                }
                VoiceEvent::Heard(text) => {
                    self.voice.status.clear();
                    self.voice.state = VoiceState::Listening;
                    self.voice_heard(ctx, &text);
                }
                VoiceEvent::Failed(why) => return self.voice_fail(why),
            }
        }
        // the agent's new words, a sentence at a time
        if (self.voice.codex.is_none() || self.voice.relay)
            && let Some(t) = self.voice.agent.as_ref().and_then(|id| self.transcripts.get(id))
        {
            let fresh: Vec<String> = t.items[self.voice.spoken.min(t.items.len())..]
                .iter()
                .filter_map(|i| if let crate::transcript::ChatItem::Agent(text) = i { Some(text.clone()) } else { None })
                .flat_map(|text| spoken(&text))
                .collect();
            self.voice.spoken = t.items.len();
            if self.voice.standby {
                // on standby the answer waits: a tone says it came
                if !fresh.is_empty() {
                    self.voice.held.extend(fresh);
                    self.ring(crate::voice::CALL_WAITING);
                }
            } else {
                self.voice.queue.extend(fresh);
            }
        }
        if self.voice.speakers_off {
            self.voice.queue.clear();
        }
        if self.voice.player.as_mut().is_some_and(|p| p.done()) {
            self.voice.player = None;
        }
        // on Codex's voice the sentences go to it, all that waits at once
        if let Some(codex) = self.voice.codex.as_mut() {
            if !self.voice.queue.is_empty() {
                codex.speak(&self.voice.queue.drain(..).collect::<Vec<_>>().join(" "));
            }
        } else if self.voice.player.is_none()
            && let Some(sentence) = self.voice.queue.pop_front()
        {
            if let Some(key) = crate::voice::openai::key(&self.prefs).filter(|_| self.prefs.voice_openai) {
                // one request for all that waits: no gap between sentences
                let rest: Vec<String> = self.voice.queue.drain(..).collect();
                let text = std::iter::once(sentence).chain(rest).collect::<Vec<_>>().join(" ");
                self.voice.player = crate::voice::openai::say(&key, &self.prefs.openai_voice, &text);
            } else {
                let piper = self.voice_piper.lock().unwrap().clone();
                // the system's voice in the chosen voice's language when Piper isn't there
                let lang = self.prefs.voice.get(..2).unwrap_or("pt").to_string();
                self.voice.player = Player::say(&sentence, piper.as_ref(), &lang);
            }
        }
        let speaking = self.voice.player.is_some() || self.voice.tone.is_some() || self.voice.talking;
        self.voice.shared.hold.store(speaking, Ordering::Relaxed);
        let muted = self.voice.shared.muted.load(Ordering::Relaxed) || self.voice.standby;
        self.voice.state = match self.voice.state {
            VoiceState::Preparing | VoiceState::Off => self.voice.state,
            _ if speaking => VoiceState::Speaking,
            _ if muted => VoiceState::Muted,
            VoiceState::Speaking | VoiceState::Muted => VoiceState::Listening,
            other => other,
        };
        // the avatar shows the call
        let answering = self.voice.working || self.chat_agent.as_ref().is_some_and(|id| self.chat_runs.contains_key(id));
        self.avatar_set(match self.voice.state {
            VoiceState::Speaking => AvatarState::Speaking,
            VoiceState::Muted => AvatarState::Muted,
            VoiceState::Preparing => AvatarState::Thinking,
            _ if answering => AvatarState::Thinking,
            VoiceState::Off => AvatarState::Idle,
            _ => AvatarState::Listening,
        });
    }

    /// The agent's name on the call, to be called by: claude, codex, or the terminal's program.
    pub(crate) fn voice_name(&self) -> String {
        let id = self.chat_agent.as_deref().unwrap_or_default();
        if let Some(chat) = self.chats.iter().find(|c| c.id == id) {
            return chat.agent.clone();
        }
        let command = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.terminal.as_ref()).map(|t| t.command.clone()).unwrap_or_default();
        if command.contains("codex") { "codex".into() } else { "claude".into() }
    }

    /// What was heard: a command by name (se muta, desmuta, pode falar, or a request on standby),
    /// else a message to the agent; on standby, anything not addressed by name is not for it.
    pub(crate) fn voice_heard(&mut self, ctx: &Context, text: &str) {
        if let Some(line) = self.mute_step(text) {
            self.voice.queue.push_back(line.into());
            return;
        }
        let name = self.voice_name();
        match crate::voice::voice_command(text, &name) {
            // asked in mute_step
            Some(VoiceCmd::Mute) => {}
            Some(VoiceCmd::Unmute) => {
                self.voice.standby = false;
                let held = std::mem::take(&mut self.voice.held);
                self.voice.queue.extend(held);
                if self.voice.queue.is_empty() {
                    self.voice.queue.push_back("Estou ouvindo.".into());
                }
            }
            Some(VoiceCmd::Speak) => {
                let held = std::mem::take(&mut self.voice.held);
                if held.is_empty() {
                    self.voice.queue.push_back("Não tenho nada pendente.".into());
                } else {
                    self.voice.queue.extend(held);
                }
            }
            Some(VoiceCmd::Ask(request)) => self.voice_send(ctx, &request),
            None if self.voice.standby => {}
            None => self.voice_send(ctx, text),
        }
    }

    /// "se muta" asks first, and a yes puts the call on standby: what to say back, when `text` was a
    /// step of it.
    pub(super) fn mute_step(&mut self, text: &str) -> Option<&'static str> {
        let asked = std::mem::take(&mut self.voice.asked_mute);
        match (asked, crate::voice::yes_or_no(text), crate::voice::voice_command(text, &self.voice_name())) {
            (true, Some(true), _) => {
                self.voice.standby = true;
                self.voice.hush();
                Some("Certo, fico quieto até você me chamar.")
            }
            (true, Some(false), _) => Some("Tá bom, continuo ouvindo."),
            (_, _, Some(VoiceCmd::Mute)) => {
                self.voice.asked_mute = true;
                self.voice.hush();
                Some("Quer que eu fique mudo?")
            }
            _ => None,
        }
    }

    /// What was heard goes to the chat's agent as a message.
    pub(super) fn voice_send(&mut self, ctx: &Context, text: &str) {
        let Some(id) = self.chat_agent.clone() else { return };
        self.voice_follow();
        if self.is_chat(&id) {
            self.chat_send(ctx, &id, text);
        } else {
            self.send_prompt(&id, text);
        }
    }
}
