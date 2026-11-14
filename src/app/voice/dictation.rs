//! Ditar: the microphone of the chat's box. What is said is written down by the call's own ears
//! (the OpenAI API with its key, else whisper.cpp on this computer, as Ajustes › Voz has it) and
//! lands in the box, not sent; nothing is spoken back. One microphone at a time: the call stops it.

use crate::prelude::*;
use crate::voice::{VoiceEvent, VoiceShared};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// Dictation on: its microphone thread, whisper.cpp when local, and where it stands.
pub(crate) struct Dictation {
    shared: VoiceShared,
    events: std::sync::mpsc::Receiver<VoiceEvent>,
    server: Arc<Mutex<Option<std::process::Child>>>,
    pub(crate) ready: bool,
    pub(crate) hearing: bool,
    pub(crate) status: String,
    /// The tick heard when it starts listening, kept while it plays.
    tone: Option<cpal::Stream>,
}

impl Drop for Dictation {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(mut server) = self.server.lock().unwrap().take() {
            let _ = server.kill();
            let _ = server.wait();
        }
    }
}

impl Dictation {
    /// Under the box: only what went wrong (the box and the button show it starting and listening).
    pub(crate) fn line(&self) -> Option<String> {
        Some(self.status.clone()).filter(|status| !status.is_empty() && status != HEARING_DONE)
    }

    /// What the empty box says while dictating, as Claude Code's /voice: getting the microphone, listening,
    /// writing down what was said.
    pub(crate) fn hint(&self) -> String {
        match (self.ready, self.status.as_str(), self.hearing) {
            (_, HEARING_DONE, _) => "Transcrevendo…".into(),
            (false, "", _) => "Preparando o microfone…".into(),
            (_, status, _) if !status.is_empty() => status.into(),
            (_, _, true) => "Ouvindo…".into(),
            _ => "Ouvindo… pode falar".into(),
        }
    }

    /// The microphone's level now, 0 to 1.
    pub(crate) fn level(&self) -> f32 {
        f32::from_bits(self.shared.level.load(Ordering::Relaxed)).clamp(0.0, 1.0)
    }
}

/// The status the ears send while they write down a stretch just said (voice/whisper.rs).
const HEARING_DONE: &str = "Entendendo…";

/// What was heard at the end of the box, a space after what was there.
pub(crate) fn dictated_into(draft: &mut String, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if !draft.is_empty() && !draft.ends_with([' ', '\n']) {
        draft.push(' ');
    }
    draft.push_str(text);
}

impl DrawingApp {
    /// Ditar / Parar de ditar.
    pub(crate) fn dictate_toggle(&mut self) {
        if self.dictation.take().is_some() {
            // off: a lower tick, as the call's mute
            self.dictation_tone = if cfg!(test) { None } else { crate::voice::chime(crate::voice::CALL_MUTE) };
            return;
        }
        if self.voice.on {
            self.voice_toggle();
        }
        // the API hears unless the person chose this computer's voice (Codex's has no dictation)
        let openai = crate::voice::openai::key(&self.prefs).filter(|_| self.prefs.voice_openai || self.prefs.voice_codex);
        let setup = VoiceSetup {
            voice: self.prefs.voice.clone(),
            language: self.voice_language().into(),
            gpu: self.prefs.voice_gpu,
            pause: self.prefs.voice_pause,
            openai,
            mic: self.prefs.voice_mic.clone(),
        };
        let shared = VoiceShared::default();
        let (events, _port, server, _piper) = crate::voice::start(shared.clone(), setup);
        self.dictation = Some(Dictation { shared, events, server, ready: false, hearing: false, status: String::new(), tone: None });
    }

    /// Every frame while dictating: what was heard goes into the box; an error ends it, said in a toast.
    pub(crate) fn dictation_tick(&mut self, ctx: &Context) {
        let Some(dictation) = self.dictation.as_mut() else { return };
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        let (mut heard, mut failed) = (Vec::new(), None);
        for event in dictation.events.try_iter() {
            match event {
                VoiceEvent::Ready => {
                    (dictation.ready, dictation.status) = (true, String::new());
                    // listening now: a tick, as the call's unmute
                    dictation.tone = if cfg!(test) { None } else { crate::voice::chime(crate::voice::CALL_UNMUTE) };
                }
                VoiceEvent::Status(text) => dictation.status = text,
                VoiceEvent::Hearing(on) => dictation.hearing = on,
                VoiceEvent::Heard(text) => heard.push(text),
                VoiceEvent::Failed(why) => failed = Some(why),
                VoiceEvent::Interrupt => {}
            }
        }
        for text in heard {
            dictated_into(&mut self.chat_draft, &text);
        }
        if let Some(why) = failed {
            self.dictation = None;
            self.toast(format!("Ditado parado: {why}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::dictated_into;

    #[test]
    fn what_is_dictated_goes_at_the_end_of_the_box() {
        let mut draft = String::new();
        dictated_into(&mut draft, " rode os testes ");
        assert_eq!(draft, "rode os testes");
        dictated_into(&mut draft, "e me diga");
        assert_eq!(draft, "rode os testes e me diga");
        dictated_into(&mut draft, "   ");
        draft.push('\n');
        dictated_into(&mut draft, "depois");
        assert_eq!(draft, "rode os testes e me diga\ndepois");
    }
}
