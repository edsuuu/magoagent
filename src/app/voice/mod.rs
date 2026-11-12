//! The call in the chat: the Ligar button turns the voice on for the agent shown, what is heard
//! goes to it as a message, what it answers is said; Mudo keeps the microphone out of it.

pub(crate) mod dictation;
pub(crate) mod orb;
#[cfg(not(target_os = "linux"))]
pub(crate) mod page;
pub(crate) mod popup;

use crate::app::chat::list::Look;
use crate::app::maestro::avatar::AvatarState;
use crate::app::voice::orb::ORB_BLUE;
use crate::prelude::*;
use std::sync::atomic::Ordering;
mod buttons;
mod call;
mod paint;
mod tick;
pub(crate) use paint::*;

/// What a call hears and speaks with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CallEngine {
    Codex,
    OpenAi,
    Local,
    /// The API's voice was the choice and has no key.
    NoKey,
}

/// The engine a call starts on: Codex's voice when it is the choice and Codex is installed; else
/// the API's when Ajustes says so (its missing key only stops the call when it was the choice
/// itself, not when Codex's was and Codex isn't here); else this computer's.
pub(crate) fn call_engine(codex: bool, codex_here: bool, openai: bool, key: bool) -> CallEngine {
    match (codex && codex_here, openai, key) {
        (true, ..) => CallEngine::Codex,
        (_, true, true) => CallEngine::OpenAi,
        (_, true, false) if !codex => CallEngine::NoKey,
        _ => CallEngine::Local,
    }
}

/// The voice chat (the call) is on hold: its buttons are hidden and nothing starts it. Codex's
/// realtime voice answers 404 from ChatGPT; the way on (OpenAI's API, or Whisper and a local voice)
/// is to be decided. Tests keep running it.
// ponytail: a switch in the code, not in Ajustes; true brings the call back everywhere
pub(crate) const CALL_ON: bool = cfg!(test);

impl DrawingApp {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_call_bars_move_with_what_the_call_does() {
        let at = |state, mic, t| call_bars(state, mic, t);
        assert_ne!(at(VoiceState::Speaking, 0.0, 0.1), at(VoiceState::Speaking, 0.0, 0.3), "speaking, they wave");
        let quiet: f32 = at(VoiceState::Hearing, 0.0, 0.2).iter().sum();
        let loud: f32 = at(VoiceState::Hearing, 1.0, 0.2).iter().sum();
        assert!(loud > quiet + 5.0, "hearing, they follow the voice");
        assert_eq!(at(VoiceState::Muted, 1.0, 0.2), [4.0; 5]);
        assert!(at(VoiceState::Speaking, 0.0, 0.7).iter().all(|h| (4.0..=14.0).contains(h)));
    }

    #[test]
    fn the_call_stays_in_the_chat_shown_and_a_codex_chats_words_are_kept() {
        let mut app = DrawingApp::default();
        let installed = vec![Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
        app.create_chat(&crate::app::chat::direct::NewChat { agent: 0, cwd: "/tmp/projeto".into(), mode: 1 }, &installed);
        // a Claude chat shown: the call stays in it
        let claude = app.chat_agent.clone();
        assert_eq!(app.call_chat(true).map(|c| c.agent), Some("claude".into()));
        assert_eq!((app.chats.len(), app.chat_agent.clone()), (1, claude));
        // nothing shown: a chat of the agent Ajustes presets is made and shown, on any voice
        app.chat_agent = None;
        app.chat_installed = Some((app.now, installed.clone()));
        let chat = app.call_chat(false).unwrap();
        assert_eq!(chat.agent, "claude");
        assert_eq!(app.chat_agent.as_deref(), Some(chat.id.as_str()));
        // no agent on the PATH: only Codex's voice gets a chat, the one its app's Codex runs
        app.chat_agent = None;
        app.chat_installed = Some((app.now, Vec::new()));
        assert_eq!(app.call_chat(false), None);
        let chat = app.call_chat(true).unwrap();
        assert_eq!(chat.agent, "codex");
        assert_eq!(app.chat_agent.as_deref(), Some(chat.id.as_str()));
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
    fn a_call_starts_without_codex_when_codex_is_not_installed() {
        // (Codex's voice chosen, Codex installed, the API's voice chosen, its key there)
        assert_eq!(call_engine(true, true, true, false), CallEngine::Codex);
        assert_eq!(call_engine(true, false, true, true), CallEngine::OpenAi, "Codex missing: the API's voice, with its key");
        assert_eq!(call_engine(true, false, true, false), CallEngine::Local, "and without one, this computer's");
        assert_eq!(call_engine(false, true, true, false), CallEngine::NoKey, "the API's voice itself chosen asks for its key");
        assert_eq!(call_engine(false, true, true, true), CallEngine::OpenAi);
        assert_eq!(call_engine(false, false, false, true), CallEngine::Local);
    }

    #[test]
    fn on_standby_the_answer_waits_for_the_name_and_a_tone_says_it_came() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        let installed = vec![Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
        app.create_chat(&crate::app::chat::direct::NewChat { agent: 0, cwd: String::new(), mode: 0 }, &installed);
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
