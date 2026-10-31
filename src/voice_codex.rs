//! The call on Codex's own voice, the one its app has: `codex app-server` (the official program, on
//! the person's ChatGPT login) holds a realtime session by WebRTC. The sound comes and goes in a
//! page kept out of sight (app/voice_page.rs); what the person asks, the voice delegates to Codex in
//! the chat's folder, with the app's MCP in its hands, and says what it answers.
// ponytail: `thread/realtime/*` is experimental in the Codex CLI (0.163); if a release renames it,
// the call fails with Codex's error and the OpenAI and Local engines still work

use crate::prelude::*;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// The voices of Codex's realtime session (v3) that ChatGPT has too, with how they sound.
pub(crate) const CODEX_VOICES: [(&str, &str); 9] = [
    ("juniper", "Juniper (feminina)"),
    ("maple", "Maple (feminina)"),
    ("sol", "Sol (feminina)"),
    ("vale", "Vale (feminina)"),
    ("arbor", "Arbor (masculina)"),
    ("breeze", "Breeze (masculina)"),
    ("cove", "Cove (masculina)"),
    ("ember", "Ember (masculina)"),
    ("spruce", "Spruce (masculina)"),
];

const SPEAKING: &str = "Fale sempre em português do Brasil, com naturalidade e frases curtas. Nunca leia caminhos, nomes de arquivo, código, comandos ou \
                        links em voz alta: diga que os detalhes estão na tela. Se pedirem para você se mutar, pergunte antes: \"Quer que eu fique mudo?\". Só com \
                        um sim, diga \"Certo, fico quieto até você me chamar.\" e fique em silêncio, sem responder nem delegar nada, até te chamarem \
                        pelo nome ou dizerem \"desmuta\".";

/// What `codex app-server` says, as the call needs it.
#[derive(Debug, PartialEq)]
pub(crate) enum CodexEvent {
    /// The thread the call delegates to is ready.
    Thread(String),
    /// The WebRTC answer to the page's offer.
    Answer(String),
    Started,
    /// What the person said, and what Codex answered a delegation with.
    Heard(String),
    Answered(String),
    /// Codex is working on a delegation, or done.
    Working(bool),
    Failed(String),
    Closed,
}

/// One line from `codex app-server` (JSON-RPC) as an event of the call.
pub(crate) fn codex_event(line: &Value) -> Option<CodexEvent> {
    let p = &line["params"];
    let text = |v: &Value| v.as_str().map(str::to_string);
    match line["method"].as_str() {
        None => {
            if let Some(message) = text(&line["error"]["message"]) {
                return Some(CodexEvent::Failed(message));
            }
            text(&line["result"]["thread"]["id"]).map(CodexEvent::Thread)
        }
        Some("thread/realtime/sdp") => text(&p["sdp"]).map(CodexEvent::Answer),
        Some("thread/realtime/started") => Some(CodexEvent::Started),
        Some("thread/realtime/transcript/done") if p["role"] == "user" => text(&p["text"]).map(CodexEvent::Heard),
        Some("item/completed") if p["item"]["type"] == "agentMessage" => text(&p["item"]["text"]).filter(|t| !t.is_empty()).map(CodexEvent::Answered),
        Some("turn/started") => Some(CodexEvent::Working(true)),
        Some("turn/completed") => Some(CodexEvent::Working(false)),
        Some("thread/realtime/error") => Some(CodexEvent::Failed(text(&p["message"]).unwrap_or_default())),
        Some("thread/realtime/closed") => Some(CodexEvent::Closed),
        _ => None,
    }
}

/// Where the call delegates: the chat's folder, its Codex thread to go on with, the Maestro's
/// instructions and options (chat_tools.rs) and whether Codex may leave the sandbox.
pub(crate) struct CodexSetup {
    pub(crate) cwd: PathBuf,
    pub(crate) resume: Option<String>,
    pub(crate) instructions: String,
    pub(crate) args: Vec<String>,
    pub(crate) full_access: bool,
}

/// The codex inside Codex's own app, when the CLI isn't on the PATH: on Windows the newest under
/// %LOCALAPPDATA%\OpenAI\Codex\bin\<build>\.
// ponytail: the macOS path is a guess at the app bundle, not checked on a Mac
fn codex_app_cli() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return Some(PathBuf::from("/Applications/Codex.app/Contents/Resources/codex")).filter(|p| p.is_file());
    }
    let bin = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("OpenAI").join("Codex").join("bin");
    fs::read_dir(bin)
        .ok()?
        .flatten()
        .map(|e| e.path().join(crate::voice::exe("codex")))
        .filter(|p| p.is_file())
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

/// `codex app-server` with the call's thread.
pub(crate) struct CodexVoice {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    next: u64,
    pub(crate) thread: Option<String>,
    pub(crate) events: Receiver<CodexEvent>,
}

impl CodexVoice {
    /// Starts `codex app-server` and opens (or resumes) the thread; the session waits for `call`.
    pub(crate) fn start(setup: &CodexSetup) -> Result<CodexVoice, String> {
        let program = crate::ide::lsp::which("codex")
            .or_else(codex_app_cli)
            .ok_or("O Codex não foi encontrado: instale o app do Codex (ou o Codex CLI) e entre com a sua conta do ChatGPT.")?;
        let mut child = crate::ide::command(program)
            .arg("app-server")
            .args(&setup.args)
            .current_dir(&setup.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Não foi possível rodar o codex: {e}"))?;
        let stdin = Arc::new(Mutex::new(child.stdin.take().ok_or("codex sem entrada")?));
        let stdout = child.stdout.take().ok_or("codex sem saída")?;
        let (tx, events) = channel();
        let answer = stdin.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
                // a request from Codex (an approval, a question): the call has no one to ask
                if value["method"].is_string() && !value["id"].is_null() {
                    let reply = json!({ "id": value["id"], "error": { "code": -32601, "message": "não disponível na chamada de voz" } });
                    let _ = writeln!(answer.lock().unwrap(), "{reply}");
                    continue;
                }
                if let Some(event) = codex_event(&value)
                    && tx.send(event).is_err()
                {
                    break;
                }
            }
            let _ = tx.send(CodexEvent::Closed);
        });
        let mut voice = CodexVoice { child, stdin, next: 0, thread: None, events };
        voice.request(
            "initialize",
            json!({ "clientInfo": { "name": "magoagent", "version": env!("CARGO_PKG_VERSION") }, "capabilities": { "experimentalApi": true } }),
        );
        voice.send(json!({ "method": "initialized" }));
        let sandbox = if setup.full_access { "danger-full-access" } else { "workspace-write" };
        let thread = json!({ "cwd": setup.cwd, "approvalPolicy": "never", "sandbox": sandbox, "developerInstructions": setup.instructions });
        match &setup.resume {
            Some(id) => voice.request("thread/resume", json!({ "threadId": id, "cwd": setup.cwd, "approvalPolicy": "never", "sandbox": sandbox })),
            None => voice.request("thread/start", thread),
        }
        Ok(voice)
    }

    fn send(&mut self, message: Value) {
        let _ = writeln!(self.stdin.lock().unwrap(), "{message}");
    }

    fn request(&mut self, method: &str, params: Value) {
        self.next += 1;
        self.send(json!({ "id": self.next, "method": method, "params": params }));
    }

    /// The realtime session, on the page's WebRTC `offer`, in `voice`; the answer comes as `Answer`.
    pub(crate) fn call(&mut self, offer: &str, voice: &str) {
        let Some(thread) = self.thread.clone() else { return };
        let params = json!({ "threadId": thread, "outputModality": "audio", "version": "v3", "voice": voice, "transport": { "type": "webrtc", "sdp": offer }, "prompt": SPEAKING });
        self.request("thread/realtime/start", params);
    }
}

impl Drop for CodexVoice {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.clone() {
            self.request("thread/realtime/stop", json!({ "threadId": thread }));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_app_server_lines_become_the_calls_events() {
        let ev = |s: &str| codex_event(&serde_json::from_str(s).unwrap());
        assert_eq!(ev(r#"{"id":2,"result":{"thread":{"id":"t1"}}}"#), Some(CodexEvent::Thread("t1".into())));
        assert_eq!(ev(r#"{"id":3,"error":{"message":"sem login"}}"#), Some(CodexEvent::Failed("sem login".into())));
        assert_eq!(ev(r#"{"method":"thread/realtime/sdp","params":{"threadId":"t1","sdp":"v=0"}}"#), Some(CodexEvent::Answer("v=0".into())));
        assert_eq!(ev(r#"{"method":"thread/realtime/transcript/done","params":{"role":"user","text":"Oi"}}"#), Some(CodexEvent::Heard("Oi".into())));
        assert_eq!(ev(r#"{"method":"thread/realtime/transcript/done","params":{"role":"assistant","text":"Olá"}}"#), None);
        assert_eq!(
            ev(r#"{"method":"item/completed","params":{"item":{"type":"agentMessage","text":"Três arquivos."}}}"#),
            Some(CodexEvent::Answered("Três arquivos.".into()))
        );
        assert_eq!(ev(r#"{"method":"turn/started","params":{}}"#), Some(CodexEvent::Working(true)));
        assert_eq!(ev(r#"{"method":"thread/realtime/error","params":{"message":"AVAS"}}"#), Some(CodexEvent::Failed("AVAS".into())));
        assert_eq!(ev(r#"{"method":"mcpServer/startupStatus/updated","params":{}}"#), None);
    }
}
