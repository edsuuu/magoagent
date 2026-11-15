//! The voice call: the microphone is listened to by its level (speech starts when it rises,
//! ends after the pause set in Ajustes, 2 s by default), each stretch of speech goes to the
//! speech-to-text of a VoiceStudio server or of OpenAI's API and its words come back as a message;
//! the summary of what the agent answers is spoken by the same engine's voice through the speaker.
//! Nothing is downloaded: no model and no program of voice comes with the app or through it.

pub(crate) mod codex;
pub(crate) mod dictionary;
pub(crate) mod openai;
pub(crate) mod studio;

// ponytail: speech is found by level, not by a VAD model (Silero when it misfires); the words
// go over an HTTP call through curl

use crate::prelude::*;
use std::collections::VecDeque;
use std::io::Read;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
mod audio;
mod command;
mod microphone;
mod player;
mod setup;
mod sounds;
mod state;
#[cfg(test)]
mod tests;
mod text;
pub(crate) use audio::*;
pub(crate) use command::*;
pub(crate) use microphone::*;
pub(crate) use player::*;
pub(crate) use setup::*;
pub(crate) use sounds::*;
pub(crate) use state::*;
pub(crate) use text::*;
