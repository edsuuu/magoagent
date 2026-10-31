//! The call in the chat: the Ligar button turns the voice on for the agent shown, what is heard
//! goes to it as a message, what it answers is said; Mudo keeps the microphone out of it.

use crate::app::avatar::AvatarState;
use crate::app::chat_list::Look;
use crate::prelude::*;
use std::sync::atomic::Ordering;

impl DrawingApp {
    /// Ligar / Desligar.
    pub(crate) fn voice_toggle(&mut self) {
        if self.voice.on {
            self.voice.shut();
            self.voice.status.clear();
            return;
        }
        self.voice = Voice::default();
        self.voice.on = true;
        self.voice.state = VoiceState::Preparing;
        self.voice_follow();
        let language = match self.prefs.language {
            1 => "pt",
            2 => "en",
            _ => "auto",
        };
        let setup = VoiceSetup { voice: self.prefs.voice.clone(), language: language.into(), gpu: self.prefs.voice_gpu };
        let (rx, _port, server, piper) = crate::voice::start(self.voice.shared.clone(), setup);
        self.voice.events = Some(rx);
        self.voice_server = server;
        self.voice_piper = piper;
    }

    pub(crate) fn voice_mute(&mut self) {
        let muted = !self.voice.shared.muted.load(Ordering::Relaxed);
        self.voice.shared.muted.store(muted, Ordering::Relaxed);
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
        if !self.voice.on {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        self.voice_follow();
        let events: Vec<VoiceEvent> = self.voice.events.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
        for event in events {
            match event {
                VoiceEvent::Ready => {
                    self.voice.status.clear();
                    self.voice.state = VoiceState::Listening;
                    self.voice.server = self.voice_server.lock().unwrap().take();
                }
                VoiceEvent::Status(text) => self.voice.status = text,
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
                VoiceEvent::Failed(why) => {
                    self.voice.shut();
                    self.voice.status = why;
                    return;
                }
            }
        }
        // the agent's new words, a sentence at a time
        if let Some(t) = self.voice.agent.as_ref().and_then(|id| self.transcripts.get(id)) {
            let fresh: Vec<String> = t.items[self.voice.spoken.min(t.items.len())..]
                .iter()
                .filter_map(|i| if let crate::transcript::ChatItem::Agent(text) = i { Some(text.clone()) } else { None })
                .flat_map(|text| sentences(&text))
                .collect();
            self.voice.spoken = t.items.len();
            if self.voice.standby {
                // on standby the answer waits: a tone says it came
                if !fresh.is_empty() {
                    self.voice.held.extend(fresh);
                    self.voice.tone = crate::voice::tone().map(|s| (s, self.now));
                }
            } else {
                self.voice.queue.extend(fresh);
            }
        }
        if self.voice.tone.as_ref().is_some_and(|(_, at)| self.now - at > 0.5) {
            self.voice.tone = None;
        }
        if self.voice.player.as_mut().is_some_and(|p| p.done()) {
            self.voice.player = None;
        }
        if self.voice.player.is_none()
            && let Some(sentence) = self.voice.queue.pop_front()
        {
            let piper = self.voice_piper.lock().unwrap().clone();
            // the system's voice in the chosen voice's language when Piper isn't there
            let lang = self.prefs.voice.get(..2).unwrap_or("pt").to_string();
            self.voice.player = Player::say(&sentence, piper.as_ref(), &lang);
        }
        let speaking = self.voice.player.is_some() || self.voice.tone.is_some();
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
        let answering = self.chat_agent.as_ref().is_some_and(|id| self.chat_runs.contains_key(id));
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
        let name = self.voice_name();
        match crate::voice::voice_command(text, &name) {
            Some(VoiceCmd::Mute) => {
                self.voice.standby = true;
                self.voice.hush();
                self.voice.queue.push_back("Certo, fico quieto até você me chamar.".into());
            }
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

    /// The round buttons at the box's end, before the send button: the call (bars that move
    /// with the microphone), and Mudo while it is on. Gives (ligar, mudo) clicked.
    pub(crate) fn voice_buttons(&self, ui: &mut egui::Ui, right: Pos2, look: &Look, t: f64) -> (bool, bool) {
        let voice = &self.voice;
        let call = Rect::from_center_size(right, Vec2::splat(32.0));
        let call_r = ui.interact(call, ui.scope_id().with("chat_voice"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let tip = if voice.on { "Desligar a voz" } else { "Conversar por voz" };
        let call_r = call_r.on_hover_text(tip);
        let p = ui.painter();
        let (fill, ink) = match (voice.on, call_r.hovered()) {
            (true, _) => (crate::app::chat_list::ACCENT, Color32::WHITE),
            (false, true) => (look.raised, look.text),
            (false, false) => (Color32::TRANSPARENT, look.soft),
        };
        p.circle_filled(call.center(), 16.0, fill);
        // five bars: still when off, with the microphone's level when on
        let level = f32::from_bits(voice.shared.level.load(Ordering::Relaxed)).min(1.0);
        for k in 0..5 {
            let x = call.center().x + (k as f32 - 2.0) * 4.0;
            let base = [5.0, 9.0, 12.0, 9.0, 5.0][k];
            let wave = if voice.on && matches!(voice.state, VoiceState::Hearing | VoiceState::Listening) {
                (level * 10.0 * (1.0 + (t * 9.0 + k as f64).sin() as f32 * 0.5)).min(8.0)
            } else {
                0.0
            };
            let h = (base + wave).min(16.0);
            p.line_segment([Pos2::new(x, call.center().y - h / 2.0), Pos2::new(x, call.center().y + h / 2.0)], Stroke::new(2.0, ink));
        }
        if !voice.on {
            return (call_r.clicked(), false);
        }
        let mute = Rect::from_center_size(right - Vec2::new(36.0, 0.0), Vec2::splat(32.0));
        let muted = voice.shared.muted.load(Ordering::Relaxed);
        let mute_r = ui.interact(mute, ui.scope_id().with("chat_mute"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(if muted {
            "Ligar o microfone"
        } else {
            "Mudo"
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
        (call_r.clicked(), mute_r.clicked())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        app.voice_heard(&ctx, "Claude, se muta");
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
