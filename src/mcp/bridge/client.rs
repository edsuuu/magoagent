use super::*;

/// Sends a request to the running app: from inside a terminal through its environment,
/// otherwise through `bridge.json`.
pub(crate) fn request(request: Value, wait: Duration) -> Result<String, String> {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    let (address, token, from) = match (env("MAGOAGENT_BRIDGE"), env("MAGOAGENT_TOKEN")) {
        // a chat's agent speaks as its chat (app/chat/node.rs `chat_env`)
        (Some(address), Some(token)) => (address, token, env("MAGOAGENT_TERMINAL_ID").or_else(|| env("MAGOAGENT_CHAT_ID")).unwrap_or_default()),
        _ => {
            let saved = endpoint_path().and_then(|p| fs::read_to_string(p).ok()).and_then(|d| serde_json::from_str::<Value>(&d).ok());
            let saved = saved.ok_or("O MagoAgent não está aberto.")?;
            (saved["address"].as_str().unwrap_or_default().into(), saved["token"].as_str().unwrap_or_default().into(), String::new())
        }
    };
    request_to(&address, &token, &from, request, wait)
}

pub(crate) fn request_to(address: &str, token: &str, from: &str, mut request: Value, wait: Duration) -> Result<String, String> {
    request["token"] = token.into();
    request["from"] = from.into();
    let mut stream = TcpStream::connect(address).map_err(|_| "O MagoAgent não está aberto (a ponte de agentes não respondeu).")?;
    stream.set_read_timeout(Some(wait + Duration::from_secs(30))).map_err(|e| e.to_string())?;
    writeln!(stream, "{request}").map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).map_err(|e| e.to_string())?;
    let reply: Value = serde_json::from_str(&line).map_err(|_| "resposta inválida do MagoAgent")?;
    match reply["ok"].as_bool() {
        Some(true) => Ok(reply["text"].as_str().unwrap_or_default().into()),
        _ => Err(reply["error"].as_str().unwrap_or("erro desconhecido").into()),
    }
}

pub(super) const HELP: &str = "Uso: magoagent <comando>
  list                         o que está conectado a este terminal (ou chat)
  ask <nome> <prompt>          manda o prompt ao agente (terminal ou chat) e espera ele terminar
      --raw                    envia teclas cruas (\\n Enter, \\t Tab, \\e Esc, \\x03 Ctrl+C); só a terminais
      --timeout <segundos>     padrão 600
  check <nome>                 lê a tela do agente (de um chat, o fim da conversa) sem mandar nada
  note read <nome>             lê uma nota conectada
  note write <nome> <texto>    troca o texto da nota
  note create <texto>          cria uma nota ligada a este terminal
  portal navigate <nome> <url> abre um endereço num navegador conectado
  portal text <nome>           lê o texto da página
  portal eval <nome> <js>      roda JavaScript na página e mostra o resultado
  portal click <nome> <css>    seu cursor vai até o elemento e clica
  portal fill <nome> <css> <texto>  seu cursor vai até o campo e digita
  recruit [--worktree] [comando]  cria um agente abaixo de você, ligado a você (padrão: claude); ele ganha um nome;
                               com --worktree, num git worktree próprio, num ramo dele
  portal create <url> [nome]   abre um navegador à sua direita, ligado a você
  connect <a> [b]              liga você a <a>, ou <a> a <b> (qualquer coisa do canvas)
  notify <texto>               avisa a pessoa (no app e numa notificação do sistema; no Windows, a barra de tarefas pisca)";

/// `magoagent list|ask|check|note …`; returns the exit code.
pub(crate) fn cli(args: &[String]) -> i32 {
    let (mut words, mut raw, mut timeout) = (Vec::new(), false, 600.0);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--raw" => raw = true,
            "--timeout" => timeout = rest.next().and_then(|t| t.parse().ok()).unwrap_or(timeout),
            word => words.push(word),
        }
    }
    let request = match words.as_slice() {
        ["list"] => json!({ "cmd": "list" }),
        ["ask", name, prompt @ ..] if !prompt.is_empty() => {
            json!({ "cmd": "ask", "name": name, "text": prompt.join(" "), "raw": raw, "timeout": timeout })
        }
        ["check", name] => json!({ "cmd": "check", "name": name }),
        ["note", "read", name] => json!({ "cmd": "note_read", "name": name }),
        ["note", "write", name, text @ ..] => json!({ "cmd": "note_write", "name": name, "text": text.join(" ") }),
        ["note", "create", text @ ..] => json!({ "cmd": "note_create", "text": text.join(" ") }),
        ["portal", "navigate", name, url] => json!({ "cmd": "portal_navigate", "name": name, "text": url }),
        ["portal", "text", name] => json!({ "cmd": "portal_text", "name": name }),
        ["portal", "eval", name, script @ ..] => json!({ "cmd": "portal_eval", "name": name, "text": script.join(" ") }),
        ["portal", "click", name, selector] => json!({ "cmd": "portal_click", "name": name, "selector": selector }),
        ["portal", "fill", name, selector, text @ ..] => json!({ "cmd": "portal_fill", "name": name, "selector": selector, "text": text.join(" ") }),
        ["recruit", "--worktree", command @ ..] => json!({ "cmd": "recruit", "text": command.join(" "), "worktree": true }),
        ["recruit", command @ ..] => json!({ "cmd": "recruit", "text": command.join(" ") }),
        ["portal", "create", url, name @ ..] => json!({ "cmd": "portal_create", "name": name.join(" "), "text": url }),
        ["connect", a] => json!({ "cmd": "connect", "name": a }),
        ["connect", a, b] => json!({ "cmd": "connect", "name": a, "to": b }),
        ["notify", text @ ..] if !text.is_empty() => json!({ "cmd": "notify", "text": text.join(" ") }),
        _ => {
            println!("{HELP}");
            return if matches!(words.as_slice(), ["help"] | []) { 0 } else { 2 };
        }
    };
    match self::request(request, Duration::from_secs_f64(timeout)) {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
