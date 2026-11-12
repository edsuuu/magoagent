use super::*;

pub(crate) fn handle(request: &Value, shared: &Mutex<Shared>, sender: &mpsc::Sender<UiCommand>) -> Result<String, String> {
    // who speaks, as the token says
    let who = request["from"].as_str().unwrap_or_default();
    let name = request["name"].as_str().unwrap_or_default();
    let text = request["text"].as_str().unwrap_or_default();
    // a terminal of a workspace off view sees its own drawing (as it left the screen), and only
    // reads and talks: what changes the drawing or a page waits for its workspace to be open.
    // A chat belongs to no workspace: it always works on the canvas shown
    // ponytail: those nodes are frozen while off view (a note its MCP writes shows on return); upgrade: re-read the space's file on change
    let (nodes, away, chat) = {
        let shared = shared.lock().unwrap();
        let chat = shared.chats.get(who).cloned();
        match shared.away.values().find(|(_, nodes)| chat.is_none() && !who.is_empty() && nodes.iter().any(|n| n.id == who)) {
            Some((space, nodes)) => (nodes.clone(), Some(space.clone()), chat),
            None => (shared.nodes.clone(), None, chat),
        }
    };
    // a chat with no card on the canvas shown speaks from outside it, as it always did
    let from = if chat.is_some() && !nodes.iter().any(|n| n.id == who) { "" } else { who };
    // what it builds (a recruit, a portal, a rope of its own) hangs from its card all the same: the
    // app puts the Maestro's card on the canvas when it is not there (app/terminal.rs)
    let builder = if chat.is_some() { who } else { from };
    let cmd = request["cmd"].as_str().unwrap_or_default();
    if let Some(space) = away
        && !matches!(cmd, "list" | "check" | "ask" | "note_read" | "notify" | "hook")
    {
        // the command as it is typed (`note create`), not the bridge's name for it
        return Err(format!(
            "O espaço \"{space}\" deste terminal não está aberto na tela: abra-o no MagoAgent para usar {}. Com ele fora da tela, list, check, ask e note read continuam funcionando.",
            cmd.replace('_', " ")
        ));
    }
    let handle_of = |node: &Node| shared.lock().unwrap().terminals.get(&node.id).cloned().ok_or("esse terminal não está rodando");
    let send = |command: UiCommand| sender.send(command).map_err(|_| "o app fechou".to_string());
    let answer = |rx: mpsc::Receiver<Result<String, String>>| rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "o app não respondeu".to_string())?;
    // `connect` reaches the whole canvas, not only what is roped to the caller: building the team is the point
    let anywhere = |n: &str| {
        let exact = nodes.iter().find(|x| x.name == n);
        exact
            .or_else(|| nodes.iter().find(|x| x.name.to_lowercase() == n.trim().to_lowercase()))
            .map(|x| x.id.clone())
            .ok_or(format!("Nada chamado \"{n}\" no canvas."))
    };
    // a child agent builds nothing: only a Maestro recruits, opens portals and connects
    if matches!(request["cmd"].as_str(), Some("recruit" | "portal_create" | "connect")) && nodes.iter().any(|n| n.id == from && !n.maestro) {
        return Err("Só um terminal Maestro pode criar terminais, navegadores ou conexões.".into());
    }
    match request["cmd"].as_str().unwrap_or_default() {
        "list" => Ok(describe(&nodes, from, shared)),
        "check" => {
            let target = find(&nodes, from, name, NodeKind::Terminal)?;
            if target.kind == NodeKind::Chat {
                let (reply, rx) = mpsc::channel();
                send(UiCommand::CheckChat { id: target.id, reply })?;
                return answer(rx);
            }
            Ok(handle_of(&target)?.screen_text())
        }
        "ask" => {
            let target = find(&nodes, from, name, NodeKind::Terminal)?;
            if target.id == who {
                return Err("um agente não pode perguntar a si mesmo".into());
            }
            let to_chat = target.kind == NodeKind::Chat;
            if to_chat && request["raw"] == true {
                return Err(format!("\"{}\" é um chat: não recebe teclas, só perguntas.", target.name));
            }
            let timeout = Duration::from_secs_f64(request["timeout"].as_f64().unwrap_or(600.0).clamp(1.0, 3600.0));
            let pair = (from.to_string(), target.id.clone());
            // a chat with no card here still asks by its name
            let asker = nodes.iter().find(|n| n.id == from).map(|n| n.name.as_str()).or(chat.as_deref()).unwrap_or("um agente fora do canvas");
            let transcript = {
                let mut shared = shared.lock().unwrap();
                // two questions at once would mix in its typing box and in its answer
                if let Some((asker, _)) = shared.asking.iter().find(|(_, asked)| *asked == target.id) {
                    let who = nodes.iter().find(|n| n.id == *asker).map_or("outro agente", |n| n.name.as_str());
                    return Err(format!("\"{}\" já está respondendo a {who}: espere essa resposta ou leia a tela com check.", target.name));
                }
                // busy with something else (or an ask that timed out): the words would queue behind
                // it and its answer mix with that work's; keys (`raw`) still go, to interrupt it.
                // A busy chat keeps the question in its line instead (app/chat/ask.rs)
                if !to_chat && request["raw"] != true && matches!(shared.states.get(&target.id), Some(AgentState::Working(_) | AgentState::Active(_))) {
                    return Err(format!("\"{}\" está trabalhando: espere ou leia a tela com check.", target.name));
                }
                shared.asking.push(pair.clone());
                shared.transcripts.get(&target.id).cloned()
            };
            // kept once typed, as the asker's for the chat and an event
            let delivered = || {
                let mut shared = shared.lock().unwrap();
                shared.asked.push((target.id.clone(), text.trim().to_string(), asker.to_string()));
                let extra = shared.asked.len().saturating_sub(50);
                shared.asked.drain(..extra);
                let what = tool_summary(if request["raw"] == true { "teclas" } else { "pergunta" }, &json!({ "prompt": text }));
                shared.events.push((from.to_string(), target.id.clone(), what));
            };
            let answer = if to_chat {
                chat::ask(&target, from, asker, text, timeout, sender)
            } else {
                handle_of(&target)
                    .map_err(String::from)
                    .and_then(|h| ask(&h, text, request["raw"] == true, timeout, transcript, delivered).map_err(|why| format!("\"{}\" {why}", target.name)))
            };
            let mut shared = shared.lock().unwrap();
            shared.asking.retain(|p| *p != pair);
            let what = match &answer {
                Ok((Asked::Answered, _)) => "respondeu",
                Ok((Asked::StillWorking, _)) => "ainda trabalha na pergunta",
                Ok((Asked::Waiting, _)) => "parou numa pergunta no próprio terminal",
                Ok((Asked::Exited, _)) => "encerrou sem responder",
                Ok((Asked::Silent, _)) => "não reagiu à pergunta",
                Err(_) => "não recebeu a pergunta",
            };
            // what went back to the asker: the answer, or why there is none
            let what = if from.is_empty() { format!("{what} (a quem perguntou de fora do canvas)") } else { what.to_string() };
            shared.events.push((target.id.clone(), from.to_string(), what));
            answer.map(|(_, text)| text)
        }
        "note_read" => Ok(find(&nodes, from, name, NodeKind::Note)?.text),
        "note_write" => {
            let note = find(&nodes, from, name, NodeKind::Note)?;
            sender.send(UiCommand::WriteNote { id: note.id, text: text.into() }).map_err(|_| "o app fechou")?;
            Ok(format!("Nota \"{}\" atualizada.", note.name))
        }
        "note_create" => {
            if from.is_empty() {
                return Err("crie notas de dentro de um terminal do MagoAgent".into());
            }
            sender.send(UiCommand::CreateNote { from: from.into(), text: text.into() }).map_err(|_| "o app fechou")?;
            Ok("Nota criada e conectada a este terminal.".into())
        }
        "portal_navigate" => {
            let portal = find(&nodes, from, name, NodeKind::Portal)?;
            let url = normalize_url(text);
            sender.send(UiCommand::PortalNavigate { id: portal.id, url: url.clone() }).map_err(|_| "o app fechou")?;
            Ok(format!("Navegador \"{}\" abrindo {url}.", portal.name))
        }
        "portal_history" => {
            let action = request["action"].as_str().unwrap_or_default();
            let Some(did) = portal_history_action(action) else {
                return Err("ação de histórico desconhecida: use back, forward ou reload".into());
            };
            let portal = find(&nodes, from, name, NodeKind::Portal)?;
            sender.send(UiCommand::PortalHistory { id: portal.id, action: action.into() }).map_err(|_| "o app fechou")?;
            Ok(format!("Navegador \"{}\" {did}.", portal.name))
        }
        cmd @ ("portal_text" | "portal_eval" | "portal_console" | "portal_shot" | "portal_click" | "portal_fill" | "vault_fill" | "portal_wait"
        | "portal_scroll") => {
            // a vault login goes to the portal open on that site; the secret in `text` is typed and never kept
            let portal = if cmd == "vault_fill" {
                portal_of_site(&nodes, from, request["site"].as_str().unwrap_or_default())?
            } else {
                find(&nodes, from, name, NodeKind::Portal)?
            };
            // a picture is the app's to take, not a script's
            if cmd == "portal_shot" {
                let (reply, picture) = mpsc::channel();
                sender.send(UiCommand::PortalShot { id: portal.id, reply }).map_err(|_| "o app fechou")?;
                return picture.recv_timeout(Duration::from_secs(20)).map_err(|_| "o app não respondeu")?;
            }
            let script = match cmd {
                "portal_text" => "document.body ? document.body.innerText : ''".into(),
                "portal_eval" => portal_eval_script(text),
                "portal_console" => crate::app::browser::console::read_script(request["level"].as_str(), request["clear"].as_bool().unwrap_or(false))?,
                "portal_wait" => {
                    let selector = request["selector"].as_str().unwrap_or_default();
                    let timeout = Duration::from_millis(request["timeout_ms"].as_u64().unwrap_or(5000).min(30_000));
                    return portal_wait(selector, timeout, |script| {
                        let (reply, answer) = mpsc::channel();
                        sender.send(UiCommand::PortalEval { id: portal.id.clone(), script: script.into(), reply }).map_err(|_| "o app fechou".to_string())?;
                        answer.recv_timeout(Duration::from_secs(2)).map_err(|_| "a página não respondeu".to_string())?
                    });
                }
                "portal_scroll" => portal_scroll_script(request["selector"].as_str(), request["dy"].as_i64()),
                _ => {
                    let who = nodes.iter().find(|n| n.id == from).map_or("IA", |n| n.name.as_str());
                    cursor_script(who, request["selector"].as_str().unwrap_or_default(), matches!(cmd, "portal_fill" | "vault_fill").then_some(text))
                }
            };
            let (reply, answer) = mpsc::channel();
            sender.send(UiCommand::PortalEval { id: portal.id, script, reply }).map_err(|_| "o app fechou")?;
            let timeout = if cmd == "portal_wait" { Duration::from_secs(35) } else { Duration::from_secs(20) };
            let json = answer.recv_timeout(timeout).map_err(|_| "a página não respondeu")??;
            if cmd == "portal_console" {
                return Ok(crate::app::browser::console::format_response(&json));
            }
            // strings come back JSON-quoted; plain text reads better
            Ok(serde_json::from_str::<Value>(&json).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or(json))
        }
        "recruit" | "portal_create" if builder.is_empty() => Err("isso só funciona de dentro de um terminal do MagoAgent".into()),
        "recruit" => {
            let (reply, rx) = mpsc::channel();
            let command = if text.trim().is_empty() { "claude" } else { text };
            send(UiCommand::Recruit {
                from: builder.into(),
                command: command.into(),
                worktree: request["worktree"] == true,
                mode: request["mode"].as_str().and_then(AgentMode::parse),
                reply,
            })?;
            answer(rx)
        }
        "portal_create" => {
            let (reply, rx) = mpsc::channel();
            send(UiCommand::CreatePortal { from: builder.into(), url: text.into(), name: name.into(), reply })?;
            answer(rx)
        }
        "connect" => {
            let to = request["to"].as_str().unwrap_or_default();
            let (a, b) = match to {
                "" if builder.is_empty() => return Err("diga as duas pontas: connect <a> <b>".into()),
                "" => (builder.to_string(), anywhere(name)?),
                to => (anywhere(name)?, anywhere(to)?),
            };
            if a == b {
                return Err("as duas pontas são a mesma coisa".into());
            }
            let (reply, rx) = mpsc::channel();
            send(UiCommand::Connect { a, b, reply })?;
            answer(rx)
        }
        "raise" => {
            send(UiCommand::Raise)?;
            Ok("Janela de volta.".into())
        }
        "notify" => {
            send(UiCommand::Notify { from: from.into(), text: text.into() })?;
            Ok("Aviso mostrado.".into())
        }
        "show" => {
            send(UiCommand::Show { screen: text.into() })?;
            Ok(format!("Mostrando {}.", if text.is_empty() { "o canvas" } else { text }))
        }
        "set_folder" => {
            let (reply, rx) = mpsc::channel();
            send(UiCommand::SetFolder { path: text.into(), reply })?;
            answer(rx)
        }
        "screenshot" => {
            let (reply, rx) = mpsc::channel();
            send(UiCommand::Screenshot { reply })?;
            answer(rx)
        }
        "review" => {
            let (reply, rx) = mpsc::channel();
            send(UiCommand::Review { path: text.into(), reply })?;
            answer(rx)
        }
        "act" => {
            let (reply, rx) = mpsc::channel();
            // who asked, as the token says: a task an agent makes or moves knows it (a chat's
            // agent is the Maestro, as it was before it had a token: no caller)
            let mut args = request["args"].clone();
            let action = request["action"].as_str().unwrap_or_default();
            if let Some(args) = args.as_object_mut() {
                args.insert("caller".into(), if chat.is_some() { "" } else { from }.into());
                // `file_tree` with no `tree` is the tree roped to who asks, a chat's too
                tree::aim(action, args, &nodes, from);
            }
            send(UiCommand::Act { action: action.into(), args, reply })?;
            answer(rx)
        }
        "hook" => {
            let wait = Duration::from_secs_f64(request["wait"].as_f64().unwrap_or(0.0).clamp(0.0, 600.0));
            let (reply, rx) = mpsc::channel();
            send(UiCommand::Hook { from: who.into(), payload: request["payload"].clone(), reply: (!wait.is_zero()).then_some(reply) })?;
            // no answer in time, or the app let it go: nothing to print
            Ok(if wait.is_zero() { String::new() } else { rx.recv_timeout(wait).ok().and_then(Result::ok).unwrap_or_default() })
        }
        "look" => {
            let (reply, rx) = mpsc::channel();
            send(UiCommand::Look { at: text.into(), zoom: request["zoom"].as_f64().map(|z| z as f32), reply })?;
            answer(rx)
        }
        // the IDE is gone: a file tree roped to the caller opens the file in its own editor pane, or searches the project
        cmd @ ("ide_open" | "ide_search") => {
            let tree = tree::named(&nodes, from, name).ok_or("Nenhuma árvore de arquivos conectada a você: ligue uma por corda (Vincular).")?;
            let args = if cmd == "ide_open" {
                let path = ide_path(Path::new(&tree.text), request["path"].as_str().unwrap_or_default())?;
                json!({ "action": "open", "tree": tree.id, "path": path.to_string_lossy(), "line": request["line"] })
            } else {
                json!({ "action": "find_in_files", "tree": tree.id, "query": text })
            };
            let (reply, rx) = mpsc::channel();
            send(UiCommand::Act { action: "file_tree".into(), args, reply })?;
            answer(rx)
        }
        other => Err(format!("comando desconhecido: {other}")),
    }
}
