//! The agent bridge: terminals and chats on the canvas talk to each other through the app.
//!
//! The app listens on 127.0.0.1 with a random token. Each terminal gets the address, the token
//! and its own id in its environment (also written to `bridge.json` next to the scene, for MCP
//! clients outside the canvas); `magoagent list|ask|check|note` and the MCP agent tools send one
//! JSON line per request and read one back. A terminal only reaches what a line connects it to;
//! so does a chat whose card is roped (bridge/chat.rs).

use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use crate::transcript::Transcript;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

mod chat;
mod client;
mod describe;
mod handle;
mod lookup;
mod portal;
mod server;
#[cfg(test)]
mod tests;
mod tree;
mod types;
pub use client::*;
pub use describe::*;
pub use handle::*;
pub use lookup::*;
pub use portal::*;
pub use server::*;
pub use types::*;
