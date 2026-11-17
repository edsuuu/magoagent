//! A chat's title written by Haiku from its first message, as Claude Code titles its conversations. The
//! first line of the message is its name until the title comes back (a few seconds).

use super::*;
use std::sync::{Arc, Mutex};

/// What Haiku is told: a short title in the message's own language, nothing else.
const TITLE_PROMPT: &str = "Você dá títulos a conversas. Responda só com um título curto (de 2 a 6 palavras) para a conversa que começa \
     com a mensagem recebida, no idioma dela, sem aspas e sem ponto final. Não responda a mensagem.";

/// The titles that came back: (chat, the name it had when asked, the title). Filled by the threads, read each frame.
pub(crate) type ChatTitles = Arc<Mutex<Vec<(String, String, String)>>>;

impl DrawingApp {
    /// Asks Haiku, in the background, for chat `id`'s title from its first message. Nothing changes when
    /// Claude Code isn't here or the call fails: the chat keeps the name it has.
    pub(crate) fn ask_chat_title(&self, ctx: Option<&Context>, id: &str, message: &str) {
        // the tests make chats without calling the model
        if cfg!(test) {
            return;
        }
        let Some(cli) = agent_cli("claude") else { return };
        let Some(asked) = self.chats.iter().find(|chat| chat.id == id).map(|chat| chat.name.clone()) else { return };
        let (titles, ctx, id) = (self.chat_titles.clone(), ctx.cloned(), id.to_string());
        let message: String = message.chars().take(2000).collect();
        std::thread::spawn(move || {
            let mut command = cli.command();
            // no tools, no MCP, no settings (their hooks), no session left in the person's list
            command.args(["-p", "--model", "haiku", "--no-session-persistence", "--tools", "", "--strict-mcp-config", "--setting-sources", ""]);
            command.args(["--system-prompt", TITLE_PROMPT]);
            command.current_dir(std::env::temp_dir());
            let mut command = cli.finish(command);
            command.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
            let Ok(mut child) = command.spawn() else { return };
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(message.as_bytes());
            }
            let Ok(output) = child.wait_with_output() else { return };
            if !output.status.success() {
                return;
            }
            let text = String::from_utf8_lossy(&output.stdout);
            let Some(title) = crate::transcript::prompt_title(text.trim().trim_matches(['"', '\u{201c}', '\u{201d}', '.'])) else { return };
            if let Ok(mut arrived) = titles.lock() {
                arrived.push((id, asked, title));
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// The titles that came back, each on its chat while it still has the name it had when asked (one
    /// renamed in between keeps the person's).
    pub(crate) fn take_chat_titles(&mut self) {
        let arrived = self.chat_titles.lock().map(|mut arrived| std::mem::take(&mut *arrived)).unwrap_or_default();
        if arrived.is_empty() {
            return;
        }
        for (id, asked, title) in arrived {
            if let Some(chat) = self.chats.iter_mut().find(|chat| chat.id == id && chat.name == asked) {
                (chat.name, chat.named) = (title, true);
            }
        }
        self.save_chats();
    }
}
