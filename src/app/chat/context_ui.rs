//! O painel "Contexto" à direita do chat do Claude Code (o desenho fica em chat/context_view.rs): quanto
//! da janela está em uso, a barra por categoria, o valor de "Compactar automaticamente em", os
//! conectores (servidores MCP) e Compactar. Os números vêm do stream (init e usage, ao vivo) e do `/context` em modo
//! print, que roda numa thread só com o painel aberto, ao abrir e depois de cada resposta; o
//! `claude mcp list` só roda se ainda não houve resposta para dizer os conectores.

use crate::app::chat::context::*;
use crate::prelude::*;
use std::sync::mpsc::{Receiver, Sender, channel};

/// A largura do painel.
pub(crate) const PANEL: f32 = 312.0;

enum Job {
    Report(String, Result<Report, String>),
    Servers(String, Vec<Server>),
}

/// O que se sabe de um chat para o painel.
#[derive(Default)]
pub(crate) struct Info {
    pub(crate) init: Option<Init>,
    pub(crate) listed: Vec<Server>,
    pub(crate) listing: bool,
    /// Tentar: o `claude mcp list` testou de novo e vale mais que o init da última resposta.
    pub(crate) retried: bool,
    /// Os tokens em uso pela última resposta e quanto era quando o `/context` foi lido.
    pub(crate) live: u64,
    live_at_report: u64,
    pub(crate) report: Option<Report>,
    pub(crate) loading: bool,
    pub(crate) dirty: bool,
    was_running: bool,
    pub(crate) error: Option<String>,
}

pub(crate) struct CtxState {
    /// Onde o painel foi pintado no último quadro (para o Esc).
    pub(crate) rect: Option<Rect>,
    pub(crate) info: std::collections::HashMap<String, Info>,
    tx: Sender<Job>,
    rx: Receiver<Job>,
}

impl Default for CtxState {
    fn default() -> Self {
        let (tx, rx) = channel();
        CtxState { rect: None, info: Default::default(), tx, rx }
    }
}

impl Info {
    /// O que cresceu desde o último `/context`.
    pub(crate) fn since(&self) -> u64 {
        if self.report.is_some() { self.live.saturating_sub(self.live_at_report) } else { 0 }
    }

    /// Os conectores: os do init da última resposta, ou os do `claude mcp list` (antes dela ou depois de Tentar).
    pub(crate) fn servers(&self) -> &[Server] {
        match &self.init {
            Some(init) if !(self.retried && !self.listed.is_empty()) => &init.servers,
            _ => &self.listed,
        }
    }
}

impl DrawingApp {
    /// A ação `chat_context` do MCP: abre, fecha ou relê o painel e diz o que ele mostra.
    pub(crate) fn act_chat_context(&mut self, chat: &str, state: &str) -> Result<String, String> {
        let id = self.chat_named(chat)?;
        if !self.context_wanted(&id) {
            return Err("o painel Contexto é dos chats do Claude Code".into());
        }
        match state {
            "open" => self.prefs.chat_panel = true,
            "close" => self.prefs.chat_panel = false,
            "refresh" => self.chat_ctx.info.entry(id.clone()).or_default().dirty = true,
            _ => {}
        }
        let info = self.chat_ctx.info.get(&id);
        let mut out = format!("Painel {}.", if self.prefs.chat_panel { "aberto" } else { "fechado" });
        match info.and_then(|i| i.report.as_ref()) {
            Some(r) => {
                out += &format!(
                    "
Contexto: {} / {} em uso ({} até o auto-compact).",
                    short(r.used),
                    short(r.window),
                    short(r.until_compact())
                );
                for c in &r.cats {
                    out += &format!(
                        "
{}: {}",
                        c.name,
                        short(c.tokens)
                    );
                }
            }
            None => {
                out += "
Contexto ainda não lido: mande uma mensagem ao chat e abra o painel."
            }
        }
        let servers: Vec<&Server> = info.map(|i| i.servers().iter().collect()).unwrap_or_default();
        for s in servers {
            let n =
                info.and_then(|i| i.init.as_ref()).map(|init| init.mcp_tools.iter().filter(|t| t.starts_with(&format!("mcp__{}__", norm(&s.name)))).count());
            out += &format!(
                "
Conector {}: {}{}",
                s.name,
                s.status.label(),
                n.map_or(String::new(), |n| format!(", {n} ferramentas"))
            );
        }
        Ok(out)
    }

    /// O painel é do Claude Code em conversa direta (o Codex e os terminais não dizem o contexto).
    pub(crate) fn context_wanted(&self, id: &str) -> bool {
        self.chats.iter().any(|c| c.id == id && c.agent != "codex")
    }

    /// Quanto da janela de contexto está em uso (0 a 100): o último `/context` mais o que cresceu
    /// depois, ou o que a linha de status disse; None antes de qualquer leitura.
    pub(crate) fn context_percent(&self, id: &str) -> Option<f64> {
        let info = self.chat_ctx.info.get(id);
        match info.and_then(|i| i.report.as_ref().map(|r| (i, r))) {
            Some((info, report)) => Some((report.used + info.since()) as f64 / report.window.max(1) as f64 * 100.0),
            None => self.usage.agents.get(id).and_then(|u| u.context),
        }
    }

    /// Onde o painel fica: a coluna à direita da área, ou por cima quando a janela é estreita.
    pub(crate) fn context_rect(&self, area: Rect, pad: f32, narrow: bool) -> Rect {
        // no máximo 30% da área do chat, mas nunca tão estreito que não caiba a lista
        // colado à direita, da barra até embaixo, com a linha à esquerda
        let _ = (pad, narrow);
        let width = PANEL.min(area.width() * 0.3).max(240.0).min(area.width());
        Rect::from_min_max(Pos2::new(area.right() - width, area.top()), area.max)
    }

    /// Esc com o ponteiro sobre o painel fecha o painel (antes de o chat tratar o Esc: ele voltaria ao canvas).
    pub(crate) fn context_esc(&mut self, ctx: &Context) -> bool {
        let over = self.chat_ctx.rect.zip(ctx.input(|i| i.pointer.hover_pos())).is_some_and(|(r, p)| r.contains(p));
        if !self.prefs.chat_panel || !over || egui::Popup::is_any_open(ctx) || !ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            return false;
        }
        self.prefs.chat_panel = false;
        self.chat_ctx.rect = None;
        true
    }

    /// Cada quadro: lê o que as threads trouxeram e, com o painel aberto, pede o que falta.
    pub(crate) fn context_tick(&mut self, ctx: &Context, id: &str, open: bool) {
        while let Ok(job) = self.chat_ctx.rx.try_recv() {
            match job {
                Job::Report(chat, result) => {
                    let info = self.chat_ctx.info.entry(chat).or_default();
                    info.loading = false;
                    match result {
                        Ok(report) => (info.report, info.live_at_report, info.error) = (Some(report), info.live, None),
                        Err(error) => info.error = Some(error),
                    }
                }
                Job::Servers(chat, list) => {
                    let info = self.chat_ctx.info.entry(chat).or_default();
                    (info.listing, info.listed) = (false, list);
                }
            }
        }
        let running = self.chat_runs.contains_key(id);
        let info = self.chat_ctx.info.entry(id.to_string()).or_default();
        // uma resposta acabou: o que o `/context` disse ficou velho
        info.dirty |= info.was_running && !running;
        info.was_running = running;
        let session = self.chats.iter().any(|c| c.id == id && c.session.is_some());
        if !open || running {
            return;
        }
        if info.init.is_none() && info.listed.is_empty() && !info.listing {
            info.listing = true;
            self.spawn_servers(ctx, id);
        } else if session && !info.loading && (info.report.is_none() && info.error.is_none() || info.dirty) {
            info.dirty = false;
            info.loading = true;
            self.spawn_report(ctx, id);
        }
    }

    /// Os eventos do stream de uma resposta que o painel usa: init, tokens em uso.
    pub(crate) fn context_event(&mut self, id: &str, event: &Value) {
        let info = self.chat_ctx.info.entry(id.to_string()).or_default();
        if let Some(init) = parse_init(event) {
            info.init = Some(init);
        }
        if let Some(used) = context_used(event) {
            info.live = used;
        }
    }

    fn spawn_report(&self, ctx: &Context, id: &str) {
        let Some(chat) = self.chats.iter().find(|c| c.id == id).cloned() else { return };
        let (Some(session), Some(program)) = (chat.session.clone(), crate::ide::lsp::which("claude")) else { return };
        let mut command = crate::ide::command(program);
        // sem gravar nada na sessão: o `/context` não vira mensagem na conversa
        command.args(["-p", "--output-format", "json", "--no-session-persistence", "--resume", &session]);
        command.args(crate::app::chat::footer::tuning_args(&chat.agent, &chat.model, &chat.effort));
        command.args(crate::app::chat::tools::maestro_args(&chat.agent, &self.prefs, &chat));
        command.args(autocompact_args(&self.prefs));
        command.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        if let Some(dir) = crate::app::chat::direct::folder(&chat.cwd).filter(|d| d.is_dir()) {
            command.current_dir(dir);
        }
        let (tx, ctx, id) = (self.chat_ctx.tx.clone(), ctx.clone(), id.to_string());
        std::thread::spawn(move || {
            let result = command
                .spawn()
                .map_err(|e| format!("Não foi possível rodar o claude: {e}"))
                .and_then(|mut child| {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = std::io::Write::write_all(&mut stdin, b"/context");
                    }
                    child.wait_with_output().map_err(|e| e.to_string())
                })
                .and_then(|out| print_result(&String::from_utf8_lossy(&out.stdout)))
                .and_then(|text| parse_context(&text));
            let _ = tx.send(Job::Report(id, result));
            ctx.request_repaint();
        });
    }

    pub(crate) fn spawn_servers(&self, ctx: &Context, id: &str) {
        let Some(program) = crate::ide::lsp::which("claude") else { return };
        let mut command = crate::ide::command(program);
        command.args(["mcp", "list"]).env("NO_COLOR", "1");
        command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        if let Some(dir) = self.chats.iter().find(|c| c.id == id).and_then(|c| crate::app::chat::direct::folder(&c.cwd)).filter(|d| d.is_dir()) {
            command.current_dir(dir);
        }
        let (tx, ctx, id) = (self.chat_ctx.tx.clone(), ctx.clone(), id.to_string());
        std::thread::spawn(move || {
            let list = command.output().map(|o| parse_mcp_list(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default();
            let _ = tx.send(Job::Servers(id, list));
            ctx.request_repaint();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um quadro com o ponteiro em `at` e o Esc apertado; devolve o que `context_esc` disse.
    fn esc_at(app: &mut DrawingApp, at: Pos2) -> bool {
        let ctx = Context::default();
        let key = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
        let input = egui::RawInput { events: vec![egui::Event::PointerMoved(at), key], ..Default::default() };
        let mut closed = false;
        let mut out = ctx.run_ui(input, |ui| closed = app.context_esc(ui.ctx()));
        out.textures_delta.clear();
        closed
    }

    #[test]
    fn the_panel_opens_and_closes_by_the_button_the_x_and_esc() {
        let mut app = DrawingApp::default();
        let installed = [Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
        app.create_chat(&crate::app::chat::direct::NewChat { agent: 0, cwd: String::new(), mode: 0 }, &installed);
        // começa fechado; o botão do cabeçalho (act.context) e a ação do MCP alternam
        assert!(!app.prefs.chat_panel);
        app.act_chat_context("", "open").unwrap();
        assert!(app.prefs.chat_panel);
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        let rect = app.context_rect(area, 16.0, false);
        // nunca mais que 30% da área do chat
        assert!(rect.width() <= 300.0 && rect.width() <= area.width() * 0.3 + 0.5, "{}", rect.width());
        app.chat_ctx.rect = Some(rect);
        // Esc com o ponteiro fora do painel não o fecha; sobre ele, sim
        assert!(!esc_at(&mut app, Pos2::new(10.0, 10.0)) && app.prefs.chat_panel);
        assert!(esc_at(&mut app, rect.center()) && !app.prefs.chat_panel);
        // o × grava o mesmo estado
        app.act_chat_context("", "open").unwrap();
        app.act_chat_context("", "close").unwrap();
        assert!(!app.prefs.chat_panel);
    }
}
