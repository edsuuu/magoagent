//! The chats on the bridge. A chat's card on the canvas is an agent like a terminal's: roped, it
//! reaches what its ropes reach and what is roped to it asks it. With no rope yet, or no card on the
//! canvas shown, the chat's agent reaches the whole canvas, as the Maestro always did. A question
//! to a chat is the app's to run (app/chat_ask.rs): it waits in the chat's line, and its answer is
//! what the chat's agent said last on that turn.

use super::*;

/// Whether `from` sees the whole canvas: a caller outside it, or a chat's card nothing is roped to.
pub(super) fn outside(nodes: &[Node], from: &str) -> bool {
    from.is_empty() || nodes.iter().any(|n| n.id == from && n.kind == NodeKind::Chat && n.links.is_empty())
}

/// A chat in `list`: said to be one, and whether its agent answers now.
pub(super) fn state(shared: &Mutex<Shared>, id: &str) -> &'static str {
    match shared.lock().unwrap().states.get(id) {
        Some(AgentState::Working(_)) => "chat, respondendo",
        _ => "chat, parado",
    }
}

/// `ask` on a chat: the question goes to the app and this waits for the chat's answer. Past
/// `timeout` the question stays with the chat, whose conversation `check` reads.
pub(super) fn ask(target: &Node, from: &str, asker: &str, text: &str, timeout: Duration, sender: &mpsc::Sender<UiCommand>) -> Result<(Asked, String), String> {
    if text.trim().is_empty() {
        return Err("diga a pergunta".into());
    }
    let (reply, answer) = mpsc::channel();
    let command = UiCommand::AskChat { id: target.id.clone(), from: from.into(), asker: asker.into(), text: text.trim().into(), reply };
    sender.send(command).map_err(|_| "o app fechou".to_string())?;
    match answer.recv_timeout(timeout) {
        Ok(Ok(text)) => Ok((Asked::Answered, text)),
        Ok(Err(why)) => Err(format!("\"{}\" {why}", target.name)),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Ok((Asked::StillWorking, format!("(\"{}\" ainda está respondendo; leia a conversa dele depois com check/read_terminal)", target.name)))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!("\"{}\" ficou sem responder: o app fechou", target.name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, kind: NodeKind, links: &[&str]) -> Node {
        Node { id: id.into(), kind, name: id.into(), text: String::new(), links: links.iter().map(|l| l.to_string()).collect(), maestro: true }
    }

    /// The app's side of an `ask` on a chat, answered by `answer` from what was asked.
    fn app(commands: mpsc::Receiver<UiCommand>, answer: impl Fn(&str, &str) -> Result<String, String> + Send + 'static) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            while let Ok(command) = commands.recv() {
                match command {
                    UiCommand::AskChat { id, asker, text, reply, .. } => {
                        assert_eq!(id, "Papo");
                        let _ = reply.send(answer(&asker, &text));
                    }
                    UiCommand::CheckChat { reply, .. } => {
                        let _ = reply.send(Ok("Chat \"Papo\" (parado)".into()));
                    }
                    other => panic!("{other:?}"),
                }
            }
        })
    }

    #[test]
    fn a_rope_between_a_chat_and_a_terminal_shows_in_list_both_ways() {
        let nodes = vec![node("Dev", NodeKind::Terminal, &["Papo"]), node("Papo", NodeKind::Chat, &["Dev"]), node("Solto", NodeKind::Terminal, &[])];
        let shared = Mutex::new(Shared { nodes, chats: [("Papo".to_string(), "Papo".to_string())].into(), ..Default::default() });
        shared.lock().unwrap().states.insert("Papo".into(), AgentState::Working(0.0));
        let (sender, _commands) = mpsc::channel();
        let list = |from: &str| handle(&json!({ "cmd": "list", "from": from }), &shared, &sender).unwrap();
        assert_eq!(list("Dev"), "Você: \"Dev\"\nAgentes conectados:\n  - \"Papo\" (chat, respondendo)");
        // the chat reaches what its card is roped to, like a terminal
        let chat = list("Papo");
        assert!(chat.starts_with("Você: \"Papo\"\nAgentes conectados:\n  - \"Dev\"") && !chat.contains("Solto"), "{chat}");
        assert!(list("Solto").contains("Nada conectado"));
        // no rope yet: the whole canvas, as before the chat had a card (and never itself)
        shared.lock().unwrap().nodes[1].links.clear();
        let alone = list("Papo");
        assert!(alone.contains("Agentes:\n  - \"Dev\"") && alone.contains("\"Solto\"") && !alone.contains("- \"Papo\""), "{alone}");
        // no card on this canvas: outside it, though it speaks by its own token
        shared.lock().unwrap().nodes.remove(1);
        let gone = list("Papo");
        assert!(!gone.contains("Você") && gone.contains("\"Dev\"") && gone.contains("\"Solto\""), "{gone}");
        // a terminal the canvas lost still reaches nothing
        assert!(list("sumiu").contains("Nada conectado"));
    }

    #[test]
    fn what_a_chat_builds_hangs_from_its_card_even_when_the_canvas_has_none_yet() {
        let nodes = vec![node("Dev", NodeKind::Terminal, &[])];
        let shared = Mutex::new(Shared { nodes, chats: [("Papo".to_string(), "Maestro".to_string())].into(), ..Default::default() });
        let (sender, commands) = mpsc::channel();
        let ui = std::thread::spawn(move || {
            let mut built = Vec::new();
            while let Ok(command) = commands.recv() {
                let (what, reply) = match command {
                    UiCommand::Recruit { from, command, reply, .. } => (format!("recruit {from} {command}"), reply),
                    UiCommand::CreatePortal { from, url, reply, .. } => (format!("portal {from} {url}"), reply),
                    UiCommand::Connect { a, b, reply } => (format!("connect {a} {b}"), reply),
                    other => panic!("{other:?}"),
                };
                built.push(what);
                let _ = reply.send(Ok("feito".into()));
            }
            built
        });
        let ask = |from: &str, request: Value| {
            let mut request = request;
            request["from"] = from.into();
            handle(&request, &shared, &sender)
        };
        // the chat's agent is the Maestro: the app is told which chat builds, and puts its card there
        assert_eq!(ask("Papo", json!({ "cmd": "recruit", "text": "codex" })), Ok("feito".into()));
        assert_eq!(ask("Papo", json!({ "cmd": "portal_create", "text": "exemplo.com" })), Ok("feito".into()));
        assert_eq!(ask("Papo", json!({ "cmd": "connect", "name": "Dev" })), Ok("feito".into()));
        // who is on no canvas and is no chat still builds nothing of its own
        assert!(ask("", json!({ "cmd": "recruit" })).unwrap_err().contains("de dentro de um terminal"));
        assert!(ask("", json!({ "cmd": "connect", "name": "Dev" })).unwrap_err().contains("as duas pontas"));
        drop(sender);
        assert_eq!(ui.join().unwrap(), ["recruit Papo codex", "portal Papo exemplo.com", "connect Papo Dev"]);
    }

    #[test]
    fn asking_a_chat_carries_the_askers_name_and_brings_its_answer_back_over_tcp() {
        let Bridge { address, token, shared, commands } = Bridge::start(None).unwrap();
        let nodes = vec![node("Dev", NodeKind::Terminal, &["Papo"]), node("Papo", NodeKind::Chat, &["Dev"])];
        *shared.lock().unwrap() = Shared { nodes, chats: [("Papo".to_string(), "Papo".to_string())].into(), ..Default::default() };
        // the app here is a thread that answers with what it was asked, and by whom
        let _ui = app(commands, |asker, text| Ok(format!("{asker} perguntou: {text}")));
        let request = |cmd: Value| request_to(&address, &token, "Dev", cmd, Duration::from_secs(20));
        let answer = request(json!({ "cmd": "ask", "name": "Papo", "text": " quanto é 40+2? ", "timeout": 20 })).unwrap();
        assert_eq!(answer, "Dev perguntou: quanto é 40+2?");
        // the rope between them was lit only while the ask ran; the event says who answered whom
        assert!(shared.lock().unwrap().asking.is_empty());
        assert_eq!(shared.lock().unwrap().events.last().unwrap(), &("Papo".to_string(), "Dev".to_string(), "respondeu".to_string()));
        assert_eq!(request(json!({ "cmd": "check", "name": "Papo" })).unwrap(), "Chat \"Papo\" (parado)");
        // keys are a terminal's
        let raw = request(json!({ "cmd": "ask", "name": "Papo", "text": "\\e", "raw": true })).unwrap_err();
        assert!(raw.contains("é um chat"), "{raw}");
    }

    #[test]
    fn a_busy_chat_takes_the_question_and_a_second_asker_waits_for_the_first() {
        let nodes = vec![node("Dev", NodeKind::Terminal, &["Papo"]), node("Outro", NodeKind::Chat, &["Papo"]), node("Papo", NodeKind::Chat, &["Dev", "Outro"])];
        let shared = Mutex::new(Shared {
            nodes,
            chats: [("Papo".to_string(), "Papo".to_string()), ("Outro".to_string(), "Outro".to_string())].into(),
            ..Default::default()
        });
        // working: a terminal would refuse the question, a chat lines it up
        shared.lock().unwrap().states.insert("Papo".into(), AgentState::Working(0.0));
        let (sender, commands) = mpsc::channel();
        let ui = app(commands, |asker, _| if asker == "Outro" { Ok("da fila".into()) } else { Err("não pôde receber a pergunta: sem agente".into()) });
        // a chat asks a chat, by its own name
        let ask = |from: &str| handle(&json!({ "cmd": "ask", "name": "Papo", "text": "oi", "from": from, "timeout": 5 }), &shared, &sender);
        assert_eq!(ask("Outro"), Ok("da fila".into()));
        assert_eq!(ask("Dev"), Err("\"Papo\" não pôde receber a pergunta: sem agente".into()));
        assert_eq!(shared.lock().unwrap().events.last().unwrap().2, "não recebeu a pergunta");
        // one question at a time, as with a terminal
        shared.lock().unwrap().asking.push(("Outro".into(), "Papo".into()));
        assert_eq!(ask("Dev"), Err("\"Papo\" já está respondendo a Outro: espere essa resposta ou leia a tela com check.".into()));
        drop(sender);
        ui.join().unwrap();
    }
}
