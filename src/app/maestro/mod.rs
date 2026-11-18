//! What an agent can build on the canvas through the bridge, like the reference app's Maestro:
//! recruit another agent (a new terminal below it), open a portal (to its right), connect two
//! things, and notify the person. What it creates is roped to it.

pub(crate) mod avatar;
pub(crate) mod binder;
pub(crate) mod claude_hooks;
pub(crate) mod codex_hooks;
pub(crate) mod demands;
pub(crate) mod environments;
pub(crate) mod events;
pub(crate) mod message;
pub(crate) mod normalize;
pub(crate) mod outside;
pub(crate) mod relay;
pub(crate) mod session_window;
pub(crate) mod skills;
pub(crate) mod timeline;

use crate::prelude::*;
use std::sync::mpsc::Sender;

/// A recruit in chat mode starts this size: its task, a screenful of its work and the band, under a terminal's size (at
/// 340 x 220 the band went past the card's edge).
pub(crate) const RECRUIT_CARD: Vec2 = Vec2::new(380.0, 400.0);

/// What an agent says, signed with its name so whoever reads it (the person, Luna, another agent) knows who speaks:
/// "Ana: estou parado" (asked: "cada agente no chat precisa ter um nome ... precisa avisar 'Agente tal aqui: estou
/// parado'"). A text that already starts with the name ("Ana aqui: …", "Ana: …", "Agente Ana: …") stays as it is.
pub(crate) fn signed(who: &str, text: &str) -> String {
    let (who, text) = (who.trim(), text.trim());
    match () {
        _ if who.is_empty() => text.to_string(),
        _ if text.is_empty() => who.to_string(),
        _ if says_who(who, text) => text.to_string(),
        _ => format!("{who}: {text}"),
    }
}

/// Whether `text` starts with `who`'s name, as a whole word, in any case and with "Agente" before it or not; a Maestro
/// told apart by its chat ("Luna (loja)") also by its first name.
fn says_who(who: &str, text: &str) -> bool {
    let text = text.to_lowercase();
    let text = text.strip_prefix("agente ").unwrap_or(&text);
    let who = who.to_lowercase();
    let short = who.split(" (").next().unwrap_or(&who);
    [who.as_str(), short].iter().any(|name| text.strip_prefix(name).is_some_and(|rest| rest.chars().next().is_none_or(|next| !next.is_alphanumeric())))
}

impl DrawingApp {
    /// A box the size of `want` at its place, or the free one nearest to it, overlapping no window or note.
    pub(crate) fn free_spot(&self, want: Rect) -> Rect {
        free_spot_among(self.scene.elements.iter(), want, want)
    }

    /// The same, around `around` (the window that opens it) when `want` is taken: a team of recruits, the terminals and
    /// the pages an agent opens stay around it instead of piling up on one side.
    pub(crate) fn free_spot_near(&self, want: Rect, around: Rect) -> Rect {
        free_spot_among(self.scene.elements.iter(), want, around)
    }

    /// `recruit`: a terminal the caller's size and folder, below it, running `command`, with an agent's name.
    #[cfg(test)]
    pub(crate) fn recruit(&mut self, from: &str, command: &str, reply: Sender<Result<String, String>>) {
        self.recruit_in(from, command, false, None, reply);
    }

    /// The recruit, in the caller's folder or, with `worktree`, in an environment of its own: a git
    /// worktree of the caller's repository on a branch of the recruit's (environments.rs). `mode` (else
    /// Ajustes' `agent_mode`) says whether it is a terminal or a chat of the app (a card, no Maestro's hands).
    pub(crate) fn recruit_in(&mut self, from: &str, command: &str, worktree: bool, mode: Option<AgentMode>, reply: Sender<Result<String, String>>) {
        let Some(caller) = self.scene.elements.iter().find(|e| e.id == from) else {
            let _ = reply.send(Err("recrute de dentro de um terminal do MagoAgent".into()));
            return;
        };
        let b = local_bounds(caller);
        // the ceiling of agents and the plan's limit hold the Maestro back (usage.rs)
        if let Some(why) = self.recruit_refusal() {
            let _ = reply.send(Err(why));
            return;
        }
        let chat_mode = mode.unwrap_or(self.prefs.agent_mode) == AgentMode::Chat;
        let raw = command;
        let command = crate::app::usage::recruit_command(command, &self.prefs.recruit_model);
        let Some(caller) = self.scene.elements.iter().find(|e| e.id == from) else { return };
        // a chat's card is smaller than any terminal: its recruit is a new terminal's size, in the chat's folder; a
        // recruit in chat mode is a card (its task and what it does, chat/recruit_card.rs), pulled bigger by hand
        let (mut cwd, size) = match (&caller.terminal, &caller.chat) {
            (Some(t), _) => (t.cwd.clone(), b.size()),
            (_, Some(chat)) => (chat.cwd.clone(), self.new_terminal_size()),
            _ => (String::new(), b.size()),
        };
        let size = if chat_mode { RECRUIT_CARD } else { size };
        let at = self.free_spot_near(Rect::from_min_size(Pos2::new(b.min.x, b.max.y + 60.0), size), b);
        let side = side_of(at, b);
        let name = self.agent_name();
        let mut terminal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        // its own environment first: without it there is no recruit
        let mut own = String::new();
        if worktree {
            let folder = crate::app::chat::direct::folder(&cwd).unwrap_or_default();
            match crate::app::maestro::environments::make(&folder, &name, &terminal.id) {
                Ok((path, branch)) => {
                    own = format!(
                        " Ele trabalha num ambiente próprio, {} (ramo {branch}): o que fizer fica nesse ramo até alguém juntar, e a pasta fica no disco até alguém tirar.",
                        path.display()
                    );
                    cwd = path.to_string_lossy().into_owned();
                }
                Err(why) => {
                    let _ = reply.send(Err(why));
                    return;
                }
            }
        }
        terminal.style = node_style(terminal.style);
        let id = terminal.id.clone();
        self.snapshot();
        if chat_mode {
            let spec = self.new_recruit_chat(&id, &name, &cwd, raw);
            terminal.chat = Some(spec);
        } else {
            let mut spec = TerminalSpec { name: name.clone(), cwd, command, maestro: false, ..Default::default() };
            // its agent only in WSL (system/wsl.rs): the terminal opens in that distro
            if let Some(distro) = spec.command.split_whitespace().next().and_then(agent_cli).as_ref().and_then(|cli| cli.wsl()) {
                spec.look.shell = wsl_shell(&distro.name);
            }
            terminal.terminal = Some(spec);
        }
        self.scene.elements.push(terminal);
        // one Cmd+Z takes back the recruit and its rope together
        self.rope_between(from, &id, false);
        self.publish_nodes();
        let _ = reply.send(Ok(format!("Agente \"{name}\" criado {side} e conectado; fale com ele com `magoagent ask \"{name}\" …`.{own}")));
    }

    /// The chat of a recruit (same id as its card), its first message queued: `raw` is the recruit's command, its
    /// program (claude, codex; else Ajustes' agent) dropped; the flags it starts with (`recruit_flags`) fill the chat's
    /// model, effort and permission, and what is left is the prompt.
    fn new_recruit_chat(&mut self, id: &str, name: &str, cwd: &str, raw: &str) -> ChatSpec {
        let raw = raw.trim();
        let (first, rest) = raw.split_once(char::is_whitespace).unwrap_or((raw, ""));
        let (agent, rest) = if matches!(first, "claude" | "codex") { (first.to_string(), rest) } else { (self.prefs.chat_agent.clone(), raw) };
        let (flags, prompt) = recruit_flags(rest);
        let prompt = prompt.trim().trim_matches('"').trim();
        let modes = crate::app::chat::direct::CHAT_MODES.len() - 1;
        let mode = flags.mode.or_else(|| crate::app::chat::footer::agent_mode(&agent)).unwrap_or(self.prefs.chat_mode).min(modes);
        let model = flags.model.unwrap_or_else(|| if agent == "claude" { self.prefs.recruit_model.clone() } else { String::new() });
        let chat = crate::app::chat::direct::Chat {
            id: id.into(),
            agent: agent.clone(),
            name: name.into(),
            cwd: cwd.into(),
            session: None,
            mode,
            model,
            effort: flags.effort.unwrap_or_default(),
            recruit: true,
            named: false,
            draft: false,
        };
        self.chats.push(chat);
        self.save_chats();
        if !prompt.is_empty() {
            self.chat_starts.push((id.into(), prompt.into()));
        }
        ChatSpec { name: name.into(), agent, cwd: cwd.into(), folded: None }
    }

    /// `portal create`: a page to the caller's right, roped to it.
    pub(crate) fn create_portal(&mut self, from: &str, url: &str, name: &str, reply: Sender<Result<String, String>>) {
        let Some(caller) = self.scene.elements.iter().find(|e| e.id == from) else {
            let _ = reply.send(Err("abra navegadores de dentro de um terminal do MagoAgent".into()));
            return;
        };
        let b = local_bounds(caller);
        let url = normalize_url(url);
        let name = match name.trim() {
            "" if host_of(&url).is_empty() => "Navegador".to_string(),
            "" => host_of(&url).to_string(),
            name => name.to_string(),
        };
        let at = self.free_spot_near(Rect::from_min_size(Pos2::new(b.max.x + 60.0, b.min.y), Vec2::new(720.0, 520.0)), b);
        let mut portal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        portal.style = node_style(portal.style);
        portal.portal = Some(PortalSpec { name: name.clone(), url, ..Default::default() });
        let id = portal.id.clone();
        self.snapshot();
        self.scene.elements.push(portal);
        self.rope_between(from, &id, false);
        self.publish_nodes();
        let _ = reply.send(Ok(format!("Navegador \"{name}\" aberto {} e conectado.", side_of(at, b))));
    }

    /// The MCP's `create_terminal`: a terminal an agent opens to run what it watches (tests, an API), to its right and
    /// roped to it (`asker`, the terminal or chat that asked: the bridge says which), in its folder unless `cwd` says
    /// otherwise, running `command` as if typed. No agent to hang it from, no terminal: it would be loose on the canvas.
    pub(crate) fn act_create_terminal(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let asker = text("asker");
        // a chat opens it from its card, put on the canvas shown when it is not there
        if !asker.is_empty() {
            self.add_chat_node(&asker, None);
        }
        let Some(caller) = self.scene.elements.iter().find(|e| !asker.is_empty() && e.id == asker) else {
            return Err("Abra terminais de dentro de um terminal ou chat do MagoAgent: o terminal fica ligado por corda a quem o abriu.".into());
        };
        let b = local_bounds(caller);
        let folder = caller.terminal.as_ref().map(|t| t.cwd.clone()).or_else(|| caller.chat.as_ref().map(|c| c.cwd.clone())).unwrap_or_default();
        let command = text("command");
        let cwd = Some(text("cwd")).filter(|cwd| !cwd.is_empty()).unwrap_or(folder);
        let name = Some(text("name")).filter(|name| !name.is_empty()).unwrap_or_else(|| self.command_name(&command));
        let name = self.unique_terminal_name(&name, None);
        let at = self.free_spot_near(Rect::from_min_size(Pos2::new(b.max.x + 60.0, b.min.y), self.new_terminal_size()), b);
        let side = side_of(at, b);
        let mut terminal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        terminal.style = node_style(terminal.style);
        terminal.terminal = Some(TerminalSpec { name: name.clone(), cwd, command: command.clone(), maestro: false, ..Default::default() });
        let id = terminal.id.clone();
        // one Cmd+Z takes back the terminal and its rope together
        self.snapshot();
        self.scene.elements.push(terminal);
        self.hang_from(&asker, std::slice::from_ref(&id));
        self.publish_nodes();
        let running = if command.is_empty() { String::new() } else { format!(", rodando `{command}`") };
        Ok(format!(
            "Terminal \"{name}\" aberto {side}{running} e ligado a você por corda: leia a tela com read_terminal \"{name}\" (ou `magoagent check \"{name}\"`), \
             mande teclas com ask_agent raw (\\x03 para parar) e veja os processos com terminal_processes."
        ))
    }

    /// `connect`: a rope between two things on the canvas, by id (the bridge resolves names).
    pub(crate) fn connect_for_agent(&mut self, a: &str, b: &str, reply: Sender<Result<String, String>>) {
        let name = |id: &str| self.agent_nodes().into_iter().find(|n| n.id == id).map(|n| n.name);
        // the bridge resolved the ids a moment ago; an end deleted since would make no rope
        let (Some(first), Some(second)) = (name(a), name(b)) else {
            let _ = reply.send(Err("uma das pontas já não está no canvas".into()));
            return;
        };
        if self.links().get(a).is_some_and(|l| l.iter().any(|x| x == b)) {
            let _ = reply.send(Ok(format!("\"{first}\" e \"{second}\" já estavam conectados.")));
            return;
        }
        self.connect(a, b);
        self.publish_nodes();
        let _ = reply.send(Ok(format!("\"{first}\" e \"{second}\" conectados.")));
    }

    /// The name an agent goes by when it speaks (`from`, its id): its card's on the canvas, its terminal's in a workspace
    /// off view, a chat's (the Maestro's as Luna, a recruit's own); "Agente" when it is none of them.
    pub(crate) fn speaker(&self, from: &str) -> String {
        // a workspace off view has no node on screen: its terminal's own name
        let who = self.agent_nodes().into_iter().find(|n| n.id == from).map(|n| n.name).or_else(|| self.away.get(from).map(|(_, s)| s.label().to_string()));
        // a chat with no card on this canvas
        let who = who.or_else(|| self.chats.iter().find(|c| c.id == from).map(|c| self.maestro_name(&c.name)));
        who.unwrap_or_else(|| "Agente".into())
    }

    /// `notify`: a toast in the app and a system notification (macOS, Linux); on Windows the
    /// taskbar asks for a look.
    pub(crate) fn notify(&mut self, from: &str, text: &str) {
        let who = self.speaker(from);
        self.toast(signed(&who, text));
        // the tests notify too: they never reach the system's notifications (each `cargo test` left one on the desktop)
        #[cfg(all(target_os = "macos", not(test)))]
        {
            // the text goes as an argument, never inside the script; waited on so no zombie stays
            let (title, text) = (format!("MagoAgent · {who}"), text.to_string());
            std::thread::spawn(move || {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", "on run argv", "-e", "display notification (item 2 of argv) with title (item 1 of argv)", "-e", "end run"])
                    .args([title, text])
                    .status();
            });
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.attention = true;
            // `--`: a text that starts with a dash is not an option
            #[cfg(all(target_os = "linux", not(test)))]
            {
                let (title, text) = (format!("MagoAgent · {who}"), text.to_string());
                std::thread::spawn(move || std::process::Command::new("notify-send").arg("--").arg(title).arg(text).status());
            }
        }
    }
}

/// What a recruit's command said about its chat before the prompt.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct RecruitFlags {
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<String>,
    /// An index of `CHAT_MODES`.
    pub(crate) mode: Option<usize>,
}

/// Reads the flags the command starts with (`--model x`, `--effort x`, `--permission-mode x`, also `--flag=x`, and
/// `--dangerously-skip-permissions`) and gives the rest, the prompt. A flag it does not know ends the reading, so it
/// stays in the prompt; a value it cannot use (an effort that does not exist) is dropped, the flag read all the same.
// ponytail: only flags before the prompt; `claude "x" --model y` keeps the flag in the prompt
pub(crate) fn recruit_flags(rest: &str) -> (RecruitFlags, &str) {
    let mut flags = RecruitFlags::default();
    let mut rest = rest.trim_start();
    loop {
        let (word, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let (name, inline) = match word.split_once('=') {
            Some((n, v)) => (n, Some(v)),
            None => (word, None),
        };
        if name == "--dangerously-skip-permissions" {
            flags.mode = Some(1);
            rest = after.trim_start();
            continue;
        }
        if !matches!(name, "--model" | "--effort" | "--permission-mode") {
            return (flags, rest);
        }
        let (value, after) = match inline {
            Some(v) => (v, after),
            None => after.trim_start().split_once(char::is_whitespace).unwrap_or((after.trim_start(), "")),
        };
        let value = value.trim_matches('"');
        match name {
            "--model" if !value.is_empty() => flags.model = Some(value.into()),
            "--effort" if ["low", "medium", "high", "xhigh", "max", "ultra"].contains(&value) => flags.effort = Some(value.into()),
            "--permission-mode" => flags.mode = Some(usize::from(value == "bypassPermissions")),
            _ => {}
        }
        rest = after.trim_start();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_an_agent_says_is_signed_with_its_name_once() {
        assert_eq!(signed("Ana", "estou parado"), "Ana: estou parado");
        assert_eq!(signed("Ana", "  terminei os testes "), "Ana: terminei os testes");
        // already said who it is: as it is
        for said in ["Ana aqui: estou parado", "Ana: terminei", "ana, preciso de você", "Agente Ana aqui: estou parado", "Ana"] {
            assert_eq!(signed("Ana", said), said, "{said}");
        }
        // a longer name that starts the same is someone else
        assert_eq!(signed("Ana", "Anabela pediu os testes"), "Ana: Anabela pediu os testes");
        // a Maestro told apart by its chat, by its first name too
        assert_eq!(signed("Luna (loja)", "Luna aqui: pronto"), "Luna aqui: pronto");
        assert_eq!(signed("Luna (loja)", "pronto"), "Luna (loja): pronto");
        // no name, no words
        assert_eq!(signed("", "oi"), "oi");
        assert_eq!(signed("Ana", ""), "Ana");
    }
}
