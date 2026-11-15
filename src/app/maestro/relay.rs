//! O relay: `magoagent hook`, o processo que o Claude Code roda a cada evento de hook. Lê o evento, diz de onde ele vem
//! e o entrega ao app pela ponte. Nunca segura o agente: sem app ouvindo, sai na hora sem imprimir nada; só espera onde
//! a pessoa pode responder (um pedido de permissão, a pergunta do hook `wait`), e mesmo aí desiste assim que o processo
//! que o rodou se vai, porque responderam no terminal.

use super::outside::{ASK_PREFIX, WAIT};
use crate::mcp::bridge::hook_endpoint_path;
use crate::prelude::*;
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc;
use std::time::Duration;

/// The app must answer the bridge within this: no app, no hook.
const CONNECT: Duration = Duration::from_millis(300);
/// De quanto em quanto tempo o relay que espera confere se o processo que o rodou ainda existe.
const PARENT_POLL: Duration = Duration::from_millis(100);

/// Cuts every string of `value` to `MAX_FIELD` bytes: the hook sends one line, not a file.
fn shorten(value: &mut Value) {
    const MAX_FIELD: usize = 2000;
    match value {
        Value::String(text) if text.len() > MAX_FIELD => {
            let mut cut = MAX_FIELD;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
        }
        Value::Array(items) => items.iter_mut().for_each(shorten),
        Value::Object(fields) => fields.values_mut().for_each(shorten),
        _ => {}
    }
}

/// What the hook sends of the CLI's event: the origin added, what weighs left out; an AskUserQuestion's input whole,
/// as it is what goes back with the answers.
pub(crate) fn tag(payload: &mut Value, env: &dyn Fn(&str) -> Option<String>) {
    let origin = if env("CLAUDE_CODE_ENTRYPOINT").as_deref() == Some("claude-desktop") {
        "claude-desktop"
    } else if env("CURSOR_TRACE_ID").is_some() {
        "cursor"
    } else {
        ""
    };
    let input = (payload["tool_name"] == "AskUserQuestion").then(|| payload["tool_input"].clone());
    if let Some(fields) = payload.as_object_mut() {
        for weighty in ["tool_response", "tool_output", "transcript_path"] {
            fields.remove(weighty);
        }
        fields.insert("mago_origin".into(), origin.into());
        fields.insert("mago_pid".into(), std::process::id().into());
        for (field, variable) in
            [("term_program", "TERM_PROGRAM"), ("wt_session", "WT_SESSION"), ("vscode_pid", "VSCODE_PID"), ("wsl_distro", "WSL_DISTRO_NAME")]
        {
            if !fields.contains_key(field) {
                fields.insert(field.into(), env(variable).unwrap_or_default().into());
            }
        }
        if fields.get("cwd").and_then(Value::as_str).is_none_or(str::is_empty)
            && let Ok(here) = std::env::current_dir()
        {
            fields.insert("cwd".into(), here.to_string_lossy().into_owned().into());
        }
    }
    shorten(payload);
    if let Some(input) = input {
        payload["tool_input"] = input;
    }
}

/// Diz se o processo que rodou o relay (o Claude Code, ou o shell que ele usou) já saiu.
fn parent_watch() -> impl Fn() -> bool {
    #[cfg(unix)]
    {
        // órfão, o processo ganha outro pai
        let parent = std::os::unix::process::parent_id();
        move || std::os::unix::process::parent_id() != parent
    }
    #[cfg(not(unix))]
    {
        let parent = crate::transcript::parents().get(&std::process::id()).copied();
        move || parent.is_some_and(|parent| !crate::transcript::process::alive(parent))
    }
}

/// A resposta que a thread do pedido trouxe; None quando ela acabou sem resposta ou `gone` diz que o processo que rodou
/// o relay se foi.
fn wait_answer(answers: &mpsc::Receiver<String>, gone: impl Fn() -> bool) -> Option<String> {
    loop {
        match answers.recv_timeout(PARENT_POLL) {
            Ok(answer) => return Some(answer),
            Err(mpsc::RecvTimeoutError::Timeout) if gone() => return None,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return None,
        }
    }
}

/// Manda `request` ao app como `from` e devolve o que imprimir (vazio: nada). Quando o hook espera a pessoa (`wait`, em
/// segundos), o pedido vai numa thread e esta vigia o processo que rodou o relay: o Claude Code encerra o hook quando a
/// pergunta é respondida no terminal, mas um shell entre os dois (Git Bash, cmd) pode ser o encerrado e deixar o relay
/// órfão. A saída do pai encerra a espera; o processo termina e o app vê a conexão cair.
pub(crate) fn ask_app(address: String, token: String, from: String, request: Value, wait: u64) -> String {
    if wait == 0 {
        return request_to(&address, &token, &from, request, Duration::ZERO).unwrap_or_default();
    }
    let (answered, answers) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = answered.send(request_to(&address, &token, &from, request, Duration::from_secs(wait)).unwrap_or_default());
    });
    wait_answer(&answers, parent_watch()).unwrap_or_default()
}

/// `magoagent hook external` with the app at `endpoint` (address, token): what to print, empty for nothing. It waits
/// only where the person can answer: a permission request, and the AskUserQuestion the `wait` hook is for.
pub(crate) fn send(endpoint: (String, String), mut payload: Value, may_wait: bool, env: &dyn Fn(&str) -> Option<String>) -> String {
    let (address, token) = endpoint;
    // no app listening: end at once, the CLI goes on as always
    let Ok(socket) = address.parse::<SocketAddr>() else { return String::new() };
    if TcpStream::connect_timeout(&socket, CONNECT).is_err() {
        return String::new();
    }
    tag(&mut payload, env);
    let question = payload["tool_name"] == "AskUserQuestion";
    let wait = match hook_name(&payload) {
        "PermissionRequest" if !question => WAIT,
        "PreToolUse" if may_wait && question => WAIT,
        // a pergunta é do hook que espera (Claude Code 2.1.85+): o PermissionRequest dela e o PreToolUse geral voltam na hora, sem cartão
        "PermissionRequest" | "PreToolUse" if question => return String::new(),
        _ => 0,
    };
    let from = format!("{ASK_PREFIX}{}", payload["session_id"].as_str().unwrap_or_default());
    ask_app(address, token, from, json!({ "cmd": "hook", "payload": payload, "wait": wait }), wait)
}

/// The app's address and token, from the fixed `bridge.json` it keeps for the hooks.
pub(crate) fn endpoint_from(path: &std::path::Path) -> Option<(String, String)> {
    let saved: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    Some((saved["address"].as_str()?.to_string(), saved["token"].as_str()?.to_string()))
}

/// `magoagent hook external`: o evento vai ao app que o `bridge.json` fixo aponta; sem app, nada.
pub(crate) fn external(payload: Value, may_wait: bool, env: &dyn Fn(&str) -> Option<String>) -> String {
    match hook_endpoint_path().and_then(|path| endpoint_from(&path)) {
        Some(endpoint) => send(endpoint, payload, may_wait, env),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn a_waiting_relay_gives_up_as_soon_as_its_parent_is_gone() {
        let (answered, answers) = mpsc::channel::<String>();
        let start = Instant::now();
        let checks = std::cell::Cell::new(0);
        let gone = || {
            checks.set(checks.get() + 1);
            checks.get() >= 2
        };
        assert_eq!(wait_answer(&answers, gone), None);
        assert!(start.elapsed() < Duration::from_secs(2), "{:?}", start.elapsed());
        // com o pai vivo, a resposta vence
        answered.send("allow".into()).unwrap();
        assert_eq!(wait_answer(&answers, || false).as_deref(), Some("allow"));
        // e a thread do pedido acabar sem resposta encerra a espera
        drop(answered);
        assert_eq!(wait_answer(&answers, || false), None);
        // o pai deste teste (o cargo) está vivo
        assert!(!parent_watch()());
    }
}
