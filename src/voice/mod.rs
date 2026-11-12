//! The voice call: the microphone is listened to by its level (speech starts when it rises,
//! ends after the pause set in Ajustes, 2 s by default), each stretch of speech goes to
//! whisper.cpp's server (run here as a program, its model loaded once) and its words come back as
//! a message; the summary of what the agent answers is spoken a sentence at a time by Piper
//! through the speaker, or by the system's voice while Piper isn't there. whisper.cpp, Piper and
//! their models are programs and files the installer downloads (install.rs), never the call.

pub(crate) mod codex;
pub(crate) mod openai;

// ponytail: speech is found by level, not by a VAD model (Silero when it misfires); the words
// go over a local HTTP call through curl; Windows and Linux download whisper.cpp's binaries, the
// Mac takes it from Homebrew (brew install whisper-cpp)

use crate::prelude::*;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
mod audio;
mod command;
mod player;
mod setup;
mod sounds;
mod state;
#[cfg(test)]
mod tests;
mod text;
mod whisper;
pub(crate) use audio::*;
pub(crate) use command::*;
pub(crate) use player::*;
pub(crate) use setup::*;
pub(crate) use sounds::*;
pub(crate) use state::*;
pub(crate) use text::*;
pub(crate) use whisper::*;
