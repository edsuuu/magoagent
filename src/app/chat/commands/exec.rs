use super::*;

impl DrawingApp {
    /// A note in the chat's conversation (a block when it has several lines).
    pub(crate) fn chat_note(&mut self, id: &str, text: impl Into<String>) {
        self.chat_transcript(id).items.push(ChatItem::System(text.into()));
        self.save_chat_items(id);
    }

    pub(super) fn chat_of(&self, id: &str) -> Option<Chat> {
        self.chats.iter().find(|c| c.id == id).cloned()
    }

    /// The box's text sent: a local command runs here; anything else goes to the agent.
    /// False when it wasn't taken (the words stay in the box).
    pub(crate) fn chat_submit(&mut self, ctx: &Context, id: &str, text: &str, prompt: &str) -> bool {
        // while the login waits, the words in the box are the code the browser showed
        let code = self.chat_cmds.get_mut(id).filter(|r| matches!(r.kind, Kind::Login) && !text.trim_start().starts_with('/')).and_then(|r| {
            let stdin = r.stdin.as_mut()?;
            writeln!(stdin, "{}", text.trim()).and_then(|_| stdin.flush()).ok()
        });
        if code.is_some() {
            self.chat_note(id, "Código enviado ao login.");
            return true;
        }
        let parsed = self.chat_of(id).filter(|_| text == prompt).and_then(|c| parse(text, &c.agent));
        let Some(parsed) = parsed else { return self.chat_send(ctx, id, prompt) };
        match parsed {
            Err(error) => self.chat_note(id, error),
            Ok(local) => {
                if self.chat_cmds.contains_key(id) && local != Local::Clear {
                    self.toast("Um comando ainda está rodando neste chat: espere ou pare com Esc.");
                    return false;
                }
                self.run_local(ctx, id, local);
            }
        }
        true
    }

    pub(super) fn run_local(&mut self, ctx: &Context, id: &str, local: Local) {
        let Some(chat) = self.chat_of(id) else { return };
        match local {
            Local::Clear => self.chat_clear(id),
            Local::Help => {
                let mut lines = vec!["Comandos do chat".to_string()];
                let custom = self.slash_custom(&chat);
                for e in catalog(&chat.agent).iter().chain(&custom) {
                    lines.push(if e.args.is_empty() { format!("/{} · {}", e.name, e.desc) } else { format!("/{} {} · {}", e.name, e.args, e.desc) });
                }
                lines.push("Qualquer outro /comando vai direto ao agente.".into());
                self.chat_note(id, lines.join("\n"));
            }
            Local::Model(arg) => self.chat_set_model(id, &arg),
            Local::Effort(arg) => self.chat_set_effort(id, &arg),
            other => {
                match cli_args(&chat.agent, &other) {
                    Err(error) => self.chat_note(id, error),
                    Ok(args) => {
                        let kind = match other {
                            Local::Login(_) => {
                                self.chat_note(id, "Abrindo o navegador para entrar… Termine o login lá. Se ele mostrar um código, cole aqui na caixa e tecle Enter. Esc cancela.");
                                Kind::Login
                            }
                            Local::Status => Kind::Status(self.chat_summary(&chat)),
                            Local::Logout => Kind::Plain("/logout"),
                            Local::Mcp => Kind::Plain("/mcp"),
                            Local::Doctor => Kind::Plain("/doctor"),
                            _ => Kind::Plain("/update"),
                        };
                        self.start_cmd(ctx, id, args, kind);
                    }
                }
            }
        }
    }

    pub(super) fn chat_summary(&self, chat: &Chat) -> String {
        let (model, effort) = (agent_default(&chat.agent), (&chat.model, &chat.effort));
        let or_default = |own: &str, default: &str| {
            if own.is_empty() { format!("{} (padrão)", if default.is_empty() { "do agente" } else { default }) } else { own.into() }
        };
        format!(
            "Pasta: {}\nModelo: {}\nEsforço: {}\nPermissão: {}\n",
            chat.cwd,
            or_default(effort.0, &model.0),
            or_default(effort.1, &model.1),
            crate::app::chat::direct::CHAT_MODES[chat.mode.min(1)]
        )
    }

    /// Runs the agent's CLI hidden in the chat's folder; what it prints comes in `poll_cmds`.
    pub(super) fn start_cmd(&mut self, ctx: &Context, id: &str, args: Vec<String>, kind: Kind) {
        let Some(chat) = self.chat_of(id) else { return };
        let Some(program) = crate::ide::lsp::which(&chat.agent) else {
            self.chat_note(id, format!("{} não foi encontrado no PATH.", chat.agent));
            return;
        };
        let mut command = crate::ide::command(program);
        // a login may ask for the code the browser shows: its stdin stays open for the chat's box
        let stdin = if matches!(kind, Kind::Login) { std::process::Stdio::piped() } else { std::process::Stdio::null() };
        command.args(args).env("NO_COLOR", "1").stdin(stdin).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        if let Some(dir) = folder(&chat.cwd).filter(|d| d.is_dir()) {
            command.current_dir(dir);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return self.chat_note(id, format!("Não foi possível rodar {}: {error}", chat.agent)),
        };
        let (tx, rx) = channel();
        let (stdout, stderr, stdin) = (child.stdout.take(), child.stderr.take(), child.stdin.take());
        let live = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let (live_out, live_err) = (live.clone(), live.clone());
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let errors = std::thread::spawn(move || stderr.map(|e| pump(e, &live_err)).unwrap_or_default());
            let out = stdout.map(|o| pump(o, &live_out)).unwrap_or_default();
            let _ = tx.send((out, errors.join().unwrap_or_default()));
            repaint.request_repaint();
        });
        self.chat_cmds.insert(id.to_string(), CmdRun { child, rx, since: self.now, kind, ctx: ctx.clone(), live, stdin, shown_url: false });
    }

    /// Esc or Interromper: the command running stops.
    pub(crate) fn cmd_cancel(&mut self, id: &str) {
        if let Some(mut run) = self.chat_cmds.remove(id) {
            let _ = run.child.kill();
            let _ = run.child.wait();
            self.chat_note(id, "Comando cancelado.");
        }
    }

    /// Each frame: the commands that ended put their output in the conversation.
    pub(crate) fn poll_cmds(&mut self) {
        let ids: Vec<String> = self.chat_cmds.keys().cloned().collect();
        for id in ids {
            // the login's address, in case the browser didn't open by itself
            let url = self.chat_cmds.get_mut(&id).filter(|r| matches!(r.kind, Kind::Login) && !r.shown_url).and_then(|r| {
                let url = first_url(&clean(&r.live.lock().unwrap()))?;
                r.shown_url = true;
                Some(url)
            });
            if let Some(url) = url {
                self.chat_note(&id, format!("Se o navegador não abriu, abra este endereço: {url}"));
            }
            let Some((out, err)) = self.chat_cmds.get(&id).and_then(|r| r.rx.try_recv().ok()) else { continue };
            let Some(mut run) = self.chat_cmds.remove(&id) else { continue };
            let ok = run.child.wait().is_ok_and(|s| s.success());
            let (out, err) = (clean(&out), clean(&err));
            let said = if out.is_empty() { err.clone() } else { out.clone() };
            let agent = self.chat_of(&id).map(|c| c.agent).unwrap_or_default();
            let status = |said: &str| if agent == "codex" { (said.contains("Logged in"), said.to_string()) } else { describe_status(said) };
            let note = match run.kind {
                _ if !ok && !matches!(run.kind, Kind::Status(_) | Kind::Check) => {
                    format!("O comando falhou.{}", if said.is_empty() { String::new() } else { format!("\n{said}") })
                }
                Kind::Login => {
                    // the browser is back: who is in now
                    if let Ok(args) = cli_args(&agent, &Local::Status) {
                        self.start_cmd(&run.ctx, &id, args, Kind::Check);
                    }
                    continue;
                }
                Kind::Check => match status(&said) {
                    (true, who) if agent == "codex" => format!("Pronto, login feito. {who}"),
                    (true, who) if self.slash.sent.contains_key(&id) => {
                        self.slash.retry.insert(id.clone());
                        format!(
                            "Pronto. {who}.
Clique em Mandar de novo para reenviar a última mensagem."
                        )
                    }
                    (true, who) => format!("Pronto. {who}."),
                    (false, _) => "O login terminou, mas o agente ainda não mostra uma conta. Tente /login de novo.".into(),
                },
                Kind::Status(summary) => format!("{summary}{}", status(&said).1),
                Kind::Plain(title) => format!("{title}\n{}", if said.is_empty() { "(sem saída)" } else { &said }),
            };
            self.chat_note(&id, note);
            run.ctx.request_repaint();
        }
    }

    /// /clear: a new session in the same chat, the conversation shown emptied.
    pub(crate) fn chat_clear(&mut self, id: &str) {
        self.chat_stop(id);
        if let Some(chat) = self.chats.iter_mut().find(|c| c.id == id) {
            chat.session = None;
        }
        self.transcripts.insert(id.to_string(), Transcript::new(PathBuf::new()));
        self.chat_images.borrow_mut().clear();
        self.save_chats();
        self.chat_note(id, "Conversa nova: a próxima mensagem começa uma sessão do zero.");
    }
}
