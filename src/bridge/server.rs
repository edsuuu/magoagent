use super::*;

pub(super) fn serve(stream: TcpStream, shared: &Mutex<Shared>, token: &str, sender: &mpsc::Sender<UiCommand>, ctx: Option<&Context>) {
    let mut line = String::new();
    // a client that connects and never sends its line would hold this thread forever
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(mut writer) = stream.try_clone() else { return };
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return;
    }
    let reply = match serde_json::from_str::<Value>(&line) {
        Ok(request) => authorize(&request, shared, token).and_then(|request| handle(&request, shared, sender)),
        Err(e) => Err(format!("pedido inválido: {e}")),
    };
    if let Some(ctx) = ctx {
        ctx.request_repaint();
    }
    let reply = match reply {
        Ok(text) => json!({ "ok": true, "text": text }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    let _ = writeln!(writer, "{reply}");
}

impl Shared {
    /// The token of a terminal's new shell: the one that terminal had before stops working.
    pub(crate) fn terminal_token(&mut self, id: &str) -> String {
        let token = new_id();
        self.terminal_tokens.retain(|_, owner| owner != id);
        self.terminal_tokens.insert(token.clone(), id.to_string());
        token
    }

    /// Keeps only the tokens of the terminals whose shell still runs.
    pub(crate) fn keep_tokens(&mut self, running: impl Fn(&String) -> bool) {
        self.terminal_tokens.retain(|_, owner| running(owner));
    }
}

/// Compares every byte, whatever the first difference: the time it takes says nothing about the token.
pub(super) fn same_token(a: &str, b: &str) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.as_bytes().get(i).copied().unwrap_or(0) ^ b.as_bytes().get(i).copied().unwrap_or(0));
    }
    diff == 0
}

/// Who is asking, by the token: the app's own (`bridge.json`: the chat's MCP, the CLI outside a terminal)
/// speaks as the `from` it says; a terminal's speaks only as that terminal.
pub(super) fn authorize(request: &Value, shared: &Mutex<Shared>, general: &str) -> Result<Value, String> {
    let token = request["token"].as_str().unwrap_or_default();
    if same_token(token, general) {
        return Ok(request.clone());
    }
    let owner = shared.lock().unwrap().terminal_tokens.iter().find_map(|(known, id)| same_token(token, known).then(|| id.clone()));
    let Some(owner) = owner else { return Err("token inválido".into()) };
    let from = request["from"].as_str().unwrap_or_default();
    if !from.is_empty() && from != owner {
        return Err("este terminal não pode falar por outro".into());
    }
    let mut request = request.clone();
    request["from"] = owner.into();
    Ok(request)
}
