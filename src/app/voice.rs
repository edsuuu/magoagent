//! The call in the chat: the Ligar button turns the voice on for the agent shown, what is heard
//! goes to it as a message, what it answers is said; Mudo keeps the microphone out of it.

use crate::app::avatar::AvatarState;
use crate::app::chat_list::Look;
use crate::app::voice_orb::ORB_BLUE;
use crate::prelude::*;
use std::sync::atomic::Ordering;

impl DrawingApp {
    /// Ligar / Desligar.
    pub(crate) fn voice_toggle(&mut self) {
        if self.voice.on {
            self.voice.shut();
            self.voice.status.clear();
            self.ring(crate::voice::CALL_END);
            return;
        }
        if self.prefs.voice_codex && cfg!(not(target_os = "linux")) {
            self.codex_call();
            return;
        }
        let openai = crate::voice_openai::key(&self.prefs);
        if self.prefs.voice_openai && openai.is_none() {
            self.voice.status = "Ponha a chave da OpenAI em Ajustes › API, ou use a voz local em Ajustes › Voz.".into();
            return;
        }
        self.voice = Voice::default();
        self.voice.on = true;
        self.voice.state = VoiceState::Preparing;
        self.voice_follow();
        // whisper hears the Maestro's language; "follow the person" hears the voice's (guessing the
        // language of a short phrase gets it wrong)
        let language = match self.prefs.language {
            1 => "pt",
            2 => "en",
            _ => self.prefs.voice.get(..2).unwrap_or("pt"),
        };
        let setup = VoiceSetup {
            voice: self.prefs.voice.clone(),
            language: language.into(),
            gpu: self.prefs.voice_gpu,
            pause: self.prefs.voice_pause,
            openai: openai.filter(|_| self.prefs.voice_openai),
        };
        let (rx, _port, server, piper) = crate::voice::start(self.voice.shared.clone(), setup);
        self.voice.events = Some(rx);
        self.voice_server = server;
        self.voice_piper = piper;
    }

    /// The call on Codex's voice: `codex app-server` in the chat's folder, going on with the chat's
    /// Codex thread, with the Maestro's MCP and instructions; the page (voice_page.rs) brings the sound.
    fn codex_call(&mut self) {
        self.voice = Voice::default();
        self.voice.on = true;
        self.voice.state = VoiceState::Preparing;
        let chat = Some(self.call_chat());
        self.voice_follow();
        let cwd = chat.as_ref().and_then(|c| crate::app::chat_direct::folder(&c.cwd)).or_else(projects_dir).unwrap_or_else(std::env::temp_dir);
        let _ = fs::create_dir_all(&cwd);
        let setup = crate::voice_codex::CodexSetup {
            cwd,
            resume: chat.as_ref().filter(|c| c.agent == "codex").and_then(|c| c.session.clone()),
            instructions: chat.as_ref().map(|c| crate::app::chat_tools::system_prompt(&self.prefs, c)).unwrap_or_default(),
            args: chat.as_ref().map(|c| crate::app::chat_tools::maestro_args("codex", &self.prefs, c)).unwrap_or_default(),
            full_access: chat.as_ref().is_some_and(|c| c.mode == 1),
        };
        match crate::voice_codex::CodexVoice::start(&setup) {
            Ok(codex) => self.voice.codex = Some(codex),
            Err(why) => self.voice_fail(why),
        }
    }

    /// The Codex chat the call goes on in, kept like any other: the one shown, else a new one in
    /// its folder (or the chats' folder), which is then shown.
    fn call_chat(&mut self) -> crate::app::chat_direct::Chat {
        let shown = self.chat_agent.as_ref().and_then(|id| self.chats.iter().find(|c| &c.id == id)).cloned();
        if let Some(chat) = shown.as_ref().filter(|c| c.agent == "codex") {
            return chat.clone();
        }
        let cwd = shown.as_ref().map(|c| c.cwd.clone()).or_else(|| projects_dir().map(|p| p.display().to_string())).unwrap_or_default();
        let chat = crate::app::chat_direct::Chat {
            id: uuid::Uuid::new_v4().simple().to_string(),
            agent: "codex".into(),
            name: format!("Codex · {}", crate::app::chat_direct::folder_label(&cwd)),
            cwd,
            session: None,
            mode: shown.map_or(0, |c| c.mode),
            model: String::new(),
            effort: String::new(),
        };
        self.chats.push(chat.clone());
        self.chat_agent = Some(chat.id.clone());
        self.save_chats();
        chat
    }

    /// One of the call's sounds; it is kept (and the microphone held) while it is heard.
    fn ring(&mut self, sound: crate::voice::Sound) {
        // the tests stay quiet
        if cfg!(test) {
            return;
        }
        let end = self.now + crate::voice::sound_length(sound) as f64;
        self.voice.tone = crate::voice::chime(sound).map(|s| (s, end));
    }

    /// The call ends on an error: its sound, and why under the box.
    /// One that couldn't start shows why over the conversation (voice_orb.rs).
    pub(crate) fn voice_fail(&mut self, why: String) {
        let launching = self.voice.on && self.voice.state == VoiceState::Preparing;
        self.voice.shut();
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
    fn codex_tick(&mut self) -> bool {
        use crate::transcript::ChatItem;
        use crate::voice_codex::CodexEvent;
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
                CodexEvent::Failed(why) => {
                    self.voice_fail(format!("Erro no chat por voz: {why}"));
                    return false;
                }
                CodexEvent::Closed(reason) => {
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
    fn voice_note(&mut self, item: crate::transcript::ChatItem) {
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
    fn voice_follow(&mut self) {
        if self.voice.agent != self.chat_agent {
            self.voice.agent = self.chat_agent.clone();
            self.voice.spoken = self.chat_agent.as_ref().and_then(|id| self.transcripts.get(id)).map_or(0, |t| t.items.len());
            self.voice.hush();
        }
    }

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
        if self.voice.codex.is_some() && !self.codex_tick() {
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
        if self.voice.codex.is_none()
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
        if self.voice.player.is_none()
            && let Some(sentence) = self.voice.queue.pop_front()
        {
            if let Some(key) = crate::voice_openai::key(&self.prefs).filter(|_| self.prefs.voice_openai) {
                // one request for all that waits: no gap between sentences
                let rest: Vec<String> = self.voice.queue.drain(..).collect();
                let text = std::iter::once(sentence).chain(rest).collect::<Vec<_>>().join(" ");
                self.voice.player = crate::voice_openai::say(&key, &self.prefs.openai_voice, &text);
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
    fn mute_step(&mut self, text: &str) -> Option<&'static str> {
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
    fn voice_send(&mut self, ctx: &Context, text: &str) {
        let Some(id) = self.chat_agent.clone() else { return };
        self.voice_follow();
        if self.is_chat(&id) {
            self.chat_send(ctx, &id, text);
        } else {
            self.send_prompt(&id, text);
        }
    }

    /// The round buttons at the box's end, before the send button: the call (bars when off; on,
    /// an X to end it, white while it connects and blue once on, as on Codex's call), and while it
    /// is on the microphone and the speakers. Gives (ligar, mudo, alto-falantes) clicked.
    pub(crate) fn voice_buttons(&self, ui: &mut egui::Ui, right: Pos2, look: &Look, _t: f64) -> (bool, bool, bool) {
        let voice = &self.voice;
        let call = Rect::from_center_size(right, Vec2::splat(32.0));
        let call_r = ui.interact(call, ui.scope_id().with("chat_voice"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let tip = match voice.state {
            VoiceState::Off => "Iniciar chat por voz",
            VoiceState::Preparing => "Cancelar chat por voz",
            _ => "Encerrar chat por voz",
        };
        let call_r = call_r.on_hover_text(tip);
        let p = ui.painter();
        let c = call.center();
        if voice.on {
            let (fill, ink) = match voice.state {
                VoiceState::Preparing => (look.text, look.surface),
                _ => (ORB_BLUE.gamma_multiply(if call_r.hovered() { 0.9 } else { 1.0 }), Color32::WHITE),
            };
            p.circle_filled(c, 16.0, fill);
            for d in [Vec2::new(4.5, 4.5), Vec2::new(4.5, -4.5)] {
                p.line_segment([c - d, c + d], Stroke::new(1.8, ink));
            }
        } else {
            let (fill, ink) = if call_r.hovered() { (look.raised, look.text) } else { (Color32::TRANSPARENT, look.soft) };
            p.circle_filled(c, 16.0, fill);
            // five bars: the call
            for k in 0..5 {
                let x = c.x + (k as f32 - 2.0) * 4.0;
                let h = [5.0, 9.0, 12.0, 9.0, 5.0][k];
                p.line_segment([Pos2::new(x, c.y - h / 2.0), Pos2::new(x, c.y + h / 2.0)], Stroke::new(2.0, ink));
            }
            return (call_r.clicked(), false, false);
        }
        // the speakers: a speaker with its waves, crossed out when silenced
        let speakers = Rect::from_center_size(right - Vec2::new(72.0, 0.0), Vec2::splat(32.0));
        let off = voice.speakers_off;
        let speakers_r = ui
            .interact(speakers, ui.scope_id().with("chat_speakers"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(if off { "Ativar som dos alto-falantes" } else { "Silenciar alto-falantes" });
        let p = ui.painter();
        if off || speakers_r.hovered() {
            p.circle_filled(speakers.center(), 16.0, look.raised);
        }
        let ink = if off { look.text } else { look.soft };
        let s = speakers.center() - Vec2::new(2.0, 0.0);
        p.add(Shape::convex_polygon(
            vec![
                s + Vec2::new(-5.0, -2.5),
                s + Vec2::new(-2.0, -2.5),
                s + Vec2::new(2.0, -6.0),
                s + Vec2::new(2.0, 6.0),
                s + Vec2::new(-2.0, 2.5),
                s + Vec2::new(-5.0, 2.5),
            ],
            ink,
            Stroke::NONE,
        ));
        if off {
            p.line_segment([s + Vec2::new(4.5, -3.0), s + Vec2::new(10.5, 3.0)], Stroke::new(1.5, ink));
            p.line_segment([s + Vec2::new(4.5, 3.0), s + Vec2::new(10.5, -3.0)], Stroke::new(1.5, ink));
        } else {
            for r in [5.0f32, 8.5] {
                let arc: Vec<Pos2> = (0..=8).map(|k| s + Vec2::angled(-0.75 + 1.5 * k as f32 / 8.0) * r).collect();
                p.add(Shape::line(arc, Stroke::new(1.5, ink)));
            }
        }
        let mute = Rect::from_center_size(right - Vec2::new(36.0, 0.0), Vec2::splat(32.0));
        let muted = voice.shared.muted.load(Ordering::Relaxed);
        let mute_r = ui.interact(mute, ui.scope_id().with("chat_mute"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(if muted {
            "Ativar microfone"
        } else {
            "Silenciar microfone"
        });
        let p = ui.painter();
        if muted || mute_r.hovered() {
            p.circle_filled(mute.center(), 16.0, if muted { crate::app::chat_list::RED.gamma_multiply(0.85) } else { look.raised });
        }
        let ink = if muted { Color32::WHITE } else { look.soft };
        // a microphone: the capsule, its stand; crossed when muted
        let c = mute.center();
        p.rect_filled(Rect::from_center_size(c - Vec2::new(0.0, 3.0), Vec2::new(5.0, 10.0)), 2.5, ink);
        p.add(Shape::line(
            vec![c + Vec2::new(-4.5, 0.0), c + Vec2::new(-4.5, 2.0), c + Vec2::new(0.0, 6.0), c + Vec2::new(4.5, 2.0), c + Vec2::new(4.5, 0.0)],
            Stroke::new(1.5, ink),
        ));
        p.line_segment([c + Vec2::new(0.0, 6.0), c + Vec2::new(0.0, 9.0)], Stroke::new(1.5, ink));
        if muted {
            p.line_segment([c + Vec2::new(-7.0, -8.0), c + Vec2::new(7.0, 8.0)], Stroke::new(2.0, ink));
        }
        (call_r.clicked(), mute_r.clicked(), speakers_r.clicked())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_call_goes_on_in_a_codex_chat_whose_words_are_kept() {
        let mut app = DrawingApp::default();
        let installed = vec![Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
        app.create_chat(&crate::app::chat_direct::NewChat { agent: 0, cwd: "/tmp/projeto".into(), mode: 1 }, &installed);
        // a Claude chat shown: the call opens a Codex chat in its folder and shows it
        let chat = app.call_chat();
        assert_eq!((chat.agent.as_str(), chat.cwd.as_str(), chat.mode), ("codex", "/tmp/projeto", 1));
        assert_eq!(app.chat_agent.as_deref(), Some(chat.id.as_str()));
        assert_eq!(app.call_chat().id, chat.id, "the Codex chat shown is the call's");
        // what is said goes to its file as it was
        use crate::transcript::{ChatItem, Step};
        let items = vec![
            ChatItem::User("Oi".into()),
            ChatItem::Agent("Olá".into()),
            ChatItem::Tool(Step { name: "Bash".into(), what: "ls".into(), diff: Some((1, 2)), ..Default::default() }),
        ];
        let json = serde_json::to_string(&items).unwrap();
        assert_eq!(serde_json::from_str::<Vec<ChatItem>>(&json).unwrap(), items);
    }

    #[test]
    fn on_standby_the_answer_waits_for_the_name_and_a_tone_says_it_came() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        let installed = vec![Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
        app.create_chat(&crate::app::chat_direct::NewChat { agent: 0, cwd: String::new(), mode: 0 }, &installed);
        let id = app.chat_agent.clone().unwrap();
        assert_eq!(app.voice_name(), "claude");
        app.voice.on = true;
        app.voice.state = VoiceState::Listening;
        app.voice.agent = Some(id.clone());
        // it asks first: "não" keeps it listening, "sim" mutes it
        app.voice_heard(&ctx, "Claude, se muta");
        assert!(!app.voice.standby);
        assert_eq!(app.voice.queue.front().map(String::as_str), Some("Quer que eu fique mudo?"));
        app.voice_heard(&ctx, "Não, deixa.");
        assert!(!app.voice.standby);
        assert_eq!(app.voice.queue.back().map(String::as_str), Some("Tá bom, continuo ouvindo."));
        app.voice_heard(&ctx, "Claude, se muta");
        app.voice_heard(&ctx, "Sim, pode.");
        assert!(app.voice.standby);
        assert_eq!(app.voice.queue.front().map(String::as_str), Some("Certo, fico quieto até você me chamar."));
        app.voice.queue.clear();
        // the conversation around isn't for it
        app.voice_heard(&ctx, "então a gente vai no mercado");
        assert!(app.voice.queue.is_empty() && app.chat_runs.is_empty());
        // an answer comes: held, not said
        app.transcripts
            .entry(id.clone())
            .or_insert_with(|| crate::transcript::Transcript::new(PathBuf::new()))
            .items
            .push(crate::transcript::ChatItem::Agent("Terminei os testes. Tudo passou.".into()));
        app.voice_tick(&ctx);
        assert_eq!(app.voice.held.len(), 2, "{:?}", app.voice.held);
        assert!(app.voice.queue.is_empty());
        app.voice_heard(&ctx, "Claude, pode falar");
        assert_eq!(app.voice.held.len(), 0);
        assert_eq!(app.voice.queue.len(), 2);
        assert!(app.voice.standby, "still on standby until desmuta");
        app.voice.queue.clear();
        app.voice_heard(&ctx, "Claude, desmuta");
        assert!(!app.voice.standby);
    }
}
