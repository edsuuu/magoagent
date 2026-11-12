//! What an agent can build on the canvas through the bridge, like the reference app's Maestro:
//! recruit another agent (a new terminal below it), open a portal (to its right), connect two
//! things, and notify the person. What it creates is roped to it.

pub(crate) mod avatar;
pub(crate) mod binder;
pub(crate) mod demands;
pub(crate) mod environments;
pub(crate) mod skills;
pub(crate) mod timeline;

use crate::prelude::*;
use std::sync::mpsc::Sender;

impl DrawingApp {
    /// A box the size of `want` at its place, or further down until it overlaps no window or note.
    pub(crate) fn free_spot(&self, want: Rect) -> Rect {
        let taken: Vec<Rect> = self.scene.elements.iter().filter(|e| e.is_node() || e.created.is_some()).map(element_bounds).collect();
        let mut at = want;
        // ponytail: walks straight down, 30 steps at most; a packed canvas may still overlap
        for _ in 0..30 {
            if !taken.iter().any(|t| t.expand(20.0).intersects(at)) {
                break;
            }
            at = at.translate(Vec2::new(0.0, want.height() + 40.0));
        }
        at
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
        // recruit in chat mode too, so its card is big enough to show the conversation, not a cut preview (node_ui.rs)
        let (mut cwd, size) = match (&caller.terminal, &caller.chat) {
            (Some(t), _) => (t.cwd.clone(), b.size()),
            (_, Some(chat)) => (chat.cwd.clone(), self.new_terminal_size()),
            _ => (String::new(), b.size()),
        };
        let at = self.free_spot(Rect::from_min_size(Pos2::new(b.min.x, b.max.y + 60.0), size));
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
            terminal.terminal = Some(TerminalSpec { name: name.clone(), cwd, command, maestro: false, ..Default::default() });
        }
        self.scene.elements.push(terminal);
        // one Cmd+Z takes back the recruit and its rope together
        self.rope_between(from, &id, false);
        self.publish_nodes();
        let _ = reply.send(Ok(format!("Agente \"{name}\" criado abaixo de você e conectado; fale com ele com `magoagent ask \"{name}\" …`.{own}")));
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
        };
        self.chats.push(chat);
        self.save_chats();
        if !prompt.is_empty() {
            self.chat_starts.push((id.into(), prompt.into()));
        }
        ChatSpec { name: name.into(), agent, cwd: cwd.into() }
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
        let at = self.free_spot(Rect::from_min_size(Pos2::new(b.max.x + 60.0, b.min.y), Vec2::new(720.0, 520.0)));
        let mut portal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        portal.style = node_style(portal.style);
        portal.portal = Some(PortalSpec { name: name.clone(), url, ..Default::default() });
        let id = portal.id.clone();
        self.snapshot();
        self.scene.elements.push(portal);
        self.rope_between(from, &id, false);
        self.publish_nodes();
        let _ = reply.send(Ok(format!("Navegador \"{name}\" aberto à sua direita e conectado.")));
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

    /// `notify`: a toast in the app and a system notification (macOS, Linux); on Windows the
    /// taskbar asks for a look.
    pub(crate) fn notify(&mut self, from: &str, text: &str) {
        // a workspace off view has no node on screen: its terminal's own name
        let who = self.agent_nodes().into_iter().find(|n| n.id == from).map(|n| n.name).or_else(|| self.away.get(from).map(|(_, s)| s.label().to_string()));
        // a chat with no card on this canvas
        let who = who.or_else(|| self.chats.iter().find(|c| c.id == from).map(|c| self.maestro_name(&c.name)));
        let who = who.unwrap_or_else(|| "Agente".into());
        self.toast(format!("{who}: {text}"));
        #[cfg(target_os = "macos")]
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
            #[cfg(target_os = "linux")]
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
