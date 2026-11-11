//! O painel "Contexto" à direita do chat do Claude Code: quanto da janela está em uso, a barra por
//! categoria, o valor de "Compactar automaticamente em", os conectores (servidores MCP) e o botão
//! Compactar sessão. Os números vêm do stream (init e usage, ao vivo) e do `/context` em modo
//! print, que roda numa thread só com o painel aberto, ao abrir e depois de cada resposta; o
//! `claude mcp list` só roda se ainda não houve resposta para dizer os conectores.

use crate::app::chat_context::*;
use crate::app::chat_list::{CROSS, Look, bold, clipped, glyph};
use crate::prelude::*;
use std::sync::mpsc::{Receiver, Sender, channel};

/// A largura do painel.
pub(crate) const PANEL: f32 = 290.0;

enum Job {
    Report(String, Result<Report, String>),
    Servers(String, Vec<Server>),
}

/// O que se sabe de um chat para o painel.
#[derive(Default)]
pub(crate) struct Info {
    init: Option<Init>,
    listed: Vec<Server>,
    listing: bool,
    /// Os tokens em uso pela última resposta e quanto era quando o `/context` foi lido.
    live: u64,
    live_at_report: u64,
    report: Option<Report>,
    loading: bool,
    dirty: bool,
    was_running: bool,
    error: Option<String>,
}

pub(crate) struct CtxState {
    /// Onde o painel foi pintado no último quadro (para o Esc).
    pub(crate) rect: Option<Rect>,
    info: std::collections::HashMap<String, Info>,
    tx: Sender<Job>,
    rx: Receiver<Job>,
}

impl Default for CtxState {
    fn default() -> Self {
        let (tx, rx) = channel();
        CtxState { rect: None, info: Default::default(), tx, rx }
    }
}

/// O nome na lista, a cor e se conta no uso (as adiadas não entram na janela).
fn style(name: &str) -> (&'static str, Color32, bool) {
    let grey = |v: u8| Color32::from_rgb(v, v, v);
    match name {
        "Messages" => ("Mensagens", Color32::from_rgb(0x4c, 0x8d, 0xf6), true),
        "System tools" => ("Ferramentas do sistema", Color32::from_rgb(0xe8, 0x8a, 0x3a), true),
        "MCP tools" => ("Ferramentas MCP", Color32::from_rgb(0x2f, 0xb8, 0xa6), true),
        "Skills" => ("Skills", grey(0xb4), true),
        "System prompt" => ("Prompt do sistema", grey(0x96), true),
        "MCP server instructions" => ("Instruções dos servidores MCP", grey(0x7e), true),
        "Memory files" => ("Arquivos de memória", grey(0x68), true),
        "Autocompact buffer" => ("Buffer de auto-compactação", grey(0x4a), true),
        "Free space" => ("Espaço livre", grey(0x2a), false),
        "MCP tools (deferred)" => ("Ferramentas MCP (adiadas)", grey(0x2a), false),
        "System tools (deferred)" => ("Ferramentas do sistema (adiadas)", grey(0x2a), false),
        _ => ("", grey(0x60), true),
    }
}

/// A ordem da lista, como no painel do Claude Code.
const ORDER: [&str; 12] = [
    "Messages",
    "System tools",
    "MCP tools",
    "Skills",
    "System prompt",
    "MCP server instructions",
    "Memory files",
    "Since",
    "Autocompact buffer",
    "Free space",
    "MCP tools (deferred)",
    "System tools (deferred)",
];

impl Info {
    /// O que cresceu desde o último `/context`.
    fn since(&self) -> u64 {
        if self.report.is_some() { self.live.saturating_sub(self.live_at_report) } else { 0 }
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
        let servers: Vec<&Server> = info.map(|i| i.init.as_ref().map_or(i.listed.iter().collect(), |init| init.servers.iter().collect())).unwrap_or_default();
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

    /// Onde o painel fica: a coluna à direita da área, ou por cima quando a janela é estreita.
    pub(crate) fn context_rect(&self, area: Rect, pad: f32, narrow: bool) -> Rect {
        // no máximo 30% da área do chat, mas nunca tão estreito que não caiba a lista
        let width = PANEL.min(area.width() * 0.3).max(240.0).min(area.width() - 2.0 * pad);
        let top = area.top() + if narrow { 52.0 } else { 58.0 };
        Rect::from_min_max(Pos2::new(area.right() - pad - width, top), Pos2::new(area.right() - pad, area.bottom() - pad))
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
        command.args(crate::app::chat_footer::tuning_args(&chat.agent, &chat.model, &chat.effort));
        command.args(crate::app::chat_tools::maestro_args(&chat.agent, &self.prefs, &chat));
        command.args(autocompact_args(&self.prefs));
        if let Some((name, key)) = crate::app::ui::settings::api_env(&self.prefs, &chat.agent) {
            command.env(name, key);
        }
        command.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        if let Some(dir) = crate::app::chat_direct::folder(&chat.cwd).filter(|d| d.is_dir()) {
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

    fn spawn_servers(&self, ctx: &Context, id: &str) {
        let Some(program) = crate::ide::lsp::which("claude") else { return };
        let mut command = crate::ide::command(program);
        command.args(["mcp", "list"]).env("NO_COLOR", "1");
        command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        if let Some(dir) = self.chats.iter().find(|c| c.id == id).and_then(|c| crate::app::chat_direct::folder(&c.cwd)).filter(|d| d.is_dir()) {
            command.current_dir(dir);
        }
        let (tx, ctx, id) = (self.chat_ctx.tx.clone(), ctx.clone(), id.to_string());
        std::thread::spawn(move || {
            let list = command.output().map(|o| parse_mcp_list(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default();
            let _ = tx.send(Job::Servers(id, list));
            ctx.request_repaint();
        });
    }

    /// Pinta o painel; devolve nada, aplica na hora o que se clicou.
    pub(crate) fn context_panel(&mut self, ctx: &Context, rect: Rect, look: &Look, id: &str) {
        let mut refresh = false;
        let mut compact = false;
        let mut close = false;
        let mut choice: Option<String> = None;
        let info = self.chat_ctx.info.get(id);
        // sempre acima: a área do chat sobe sozinha quando se clica nela e cobriria o painel
        let current = self.prefs.chat_autocompact.clone();
        egui::Area::new(egui::Id::unique("chat_context")).fixed_pos(rect.min).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            ui.painter().rect_filled(rect, 12, look.panel);
            ui.painter().rect_stroke(rect, 12, Stroke::new(1.0, look.line), StrokeKind::Inside);
            let inner = rect.shrink2(Vec2::new(16.0, 14.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                ui.set_width(inner.width());
                ui.spacing_mut().item_spacing.y = 6.0;
                // título e Atualizar
                let (head, _) = ui.allocate_exact_size(Vec2::new(inner.width(), 22.0), Sense::hover());
                bold(ui.painter(), head.left_top(), "Contexto", 16.0, look.text);
                let x = Rect::from_center_size(Pos2::new(head.right() - 11.0, head.center().y), Vec2::splat(24.0));
                let xr = ui.interact(x, ui.scope_id().with("ctx_close"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Fechar (Esc)");
                if xr.hovered() {
                    ui.painter().circle_filled(x.center(), 12.0, look.raised);
                }
                glyph(ui.painter(), x.center(), 12.0, 2.0, if xr.hovered() { look.text } else { look.soft }, CROSS);
                close = xr.clicked();
                let label = if info.is_some_and(|i| i.loading) { "Atualizando…" } else { "Atualizar" };
                let g = ui.painter().layout_no_wrap(label.into(), FontId::proportional(12.0), look.soft);
                let button = Rect::from_min_size(Pos2::new(x.left() - g.size().x - 10.0, head.top()), Vec2::new(g.size().x + 8.0, 22.0));
                let r = ui.interact(button, ui.scope_id().with("ctx_refresh"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                ui.painter().galley(button.center() - g.size() / 2.0, g, if r.hovered() { look.text } else { look.soft });
                refresh = r.clicked();
                let footer = 84.0;
                let body = (inner.height() - 28.0 - footer).max(80.0);
                egui::ScrollArea::vertical().id_salt("chat_context_body").max_height(body).auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_width(inner.width() - 10.0);
                    summary(ui, info, look, &current, &mut choice);
                    ui.add_space(6.0);
                    servers(ui, info, look);
                    details(ui, info, look);
                });
                ui.add_space(4.0);
                footer_ui(ui, info, look, &mut compact);
            });
        });
        self.chat_ctx.rect = Some(rect);
        if close {
            self.prefs.chat_panel = false;
            self.chat_ctx.rect = None;
        }
        if refresh && let Some(info) = self.chat_ctx.info.get_mut(id) {
            (info.dirty, info.error) = (true, None);
        }
        if let Some(value) = choice {
            self.prefs.chat_autocompact = value;
            if let Some(info) = self.chat_ctx.info.get_mut(id) {
                info.dirty = true;
            }
        }
        if compact {
            self.chat_submit(ctx, id, "/compact", "/compact");
        }
    }
}

/// Uma linha da lista: quadradinho de cor, nome, e à direita os tokens e a porcentagem.
/// O quadradinho de uma linha: cheio, só o contorno (o que não conta na janela) ou nenhum.
enum Mark {
    Fill(Color32),
    Hollow,
    Bare,
}

fn line(ui: &mut egui::Ui, look: &Look, mark: Mark, name: &str, tokens: &str, pct: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    let p = ui.painter();
    let square = Rect::from_center_size(Pos2::new(rect.left() + 5.0, rect.center().y), Vec2::splat(10.0));
    match mark {
        Mark::Fill(color) => drop(p.rect_filled(square, 2, color)),
        Mark::Hollow => drop(p.rect_stroke(square.shrink(0.5), 2, Stroke::new(1.0, look.muted), StrokeKind::Inside)),
        Mark::Bare => {}
    }
    let font = FontId::proportional(12.5);
    let pct_w = 44.0;
    p.text(Pos2::new(rect.right(), rect.center().y), egui::Align2::RIGHT_CENTER, pct, font.clone(), look.muted);
    let t = p.text(Pos2::new(rect.right() - pct_w, rect.center().y), egui::Align2::RIGHT_CENTER, tokens, font.clone(), look.soft);
    let g = clipped(p, name.to_string(), font, look.text, (t.left() - rect.left() - 28.0).max(20.0));
    p.galley(Pos2::new(rect.left() + 18.0, rect.center().y - g.size().y / 2.0), g, look.text);
}

fn summary(ui: &mut egui::Ui, info: Option<&Info>, look: &Look, current: &str, choice: &mut Option<String>) {
    let Some(info) = info else { return empty(ui, look, "Mande uma mensagem para ver o contexto.") };
    let Some(report) = &info.report else {
        let note = match (&info.error, info.loading, info.live) {
            (Some(error), ..) => format!("Não foi possível ler o contexto: {error}"),
            (_, true, _) => "Lendo o contexto…".to_string(),
            (_, _, live) if live > 0 => format!("{} tokens em uso.", short(live)),
            _ => "Mande uma mensagem para ver o contexto.".to_string(),
        };
        return empty(ui, look, &note);
    };
    let since = info.since();
    let used = report.used + since;
    let window = report.window.max(1);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{}% usado", (used as f64 / window as f64 * 100.0).round())).size(20.0).strong().color(look.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{} / {}", short(used), short(report.window))).size(12.5).color(look.muted));
        });
    });
    // a barra: cada categoria que conta, em ordem, e o que sobra
    let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 10.0), Sense::hover());
    ui.painter().rect_filled(bar, 5, style("Free space").1);
    let mut x = bar.left();
    let value = |key: &str| if key == "Since" { since } else { report.cat(key) };
    let ui_painter = ui.painter().with_clip_rect(bar);
    for key in ORDER.iter().filter(|k| style(k).2) {
        let w = bar.width() * (value(key) as f32 / window as f32);
        if w >= 0.5 {
            let color = if *key == "Since" { style("Messages").1.gamma_multiply(0.6) } else { style(key).1 };
            ui_painter.rect_filled(Rect::from_min_size(Pos2::new(x, bar.top()), Vec2::new(w, bar.height())), 0, color);
            x += w;
        }
    }
    ui.add_space(4.0);
    // Compactar automaticamente em [400k ▾]
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Compactar automaticamente em").size(12.5).color(look.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let shown = if current.is_empty() { short(report.window) } else { current.to_string() };
            let button = egui::Button::new(egui::RichText::new(format!("{shown} ▾")).size(12.5).color(look.text)).fill(look.surface).corner_radius(6);
            let response = ui.add(button).on_hover_cursor(CursorIcon::PointingHand);
            egui::Popup::menu(&response).show(|ui| {
                ui.set_min_width(110.0);
                if ui.selectable_label(current.is_empty(), "Automático").clicked() {
                    *choice = Some(String::new());
                }
                for value in COMPACT_CHOICES {
                    if ui.selectable_label(current == value, value).clicked() {
                        *choice = Some(value.to_string());
                    }
                }
            });
        });
    });
    ui.label(
        egui::RichText::new("Um valor menor consome menos do seu limite. O Claude resume as mensagens mais antigas mais cedo.").size(11.5).color(look.muted),
    );
    ui.add_space(4.0);
    for key in ORDER {
        let (name, color, counts) = style(key);
        let tokens = if key == "Since" { since } else { report.cats.iter().find(|c| c.name == key).map_or(0, |c| c.tokens) };
        let known = key == "Since" && since > 0 || report.cats.iter().any(|c| c.name == key);
        if !known {
            continue;
        }
        let name = if key == "Since" { "Desde a última atualização" } else { name };
        let color = if key == "Since" { style("Messages").1.gamma_multiply(0.6) } else { color };
        let pct = if counts { percent(tokens as f32 / window as f32 * 100.0) } else { "—".into() };
        line(ui, look, if counts { Mark::Fill(color) } else { Mark::Hollow }, name, &short(tokens), &pct);
    }
}

/// As seções que abrem: as ferramentas MCP e os arquivos de memória.
fn details(ui: &mut egui::Ui, info: Option<&Info>, look: &Look) {
    let Some(report) = info.and_then(|i| i.report.as_ref()) else { return };
    ui.add_space(4.0);
    let tools =
        egui::CollapsingHeader::new(egui::RichText::new(format!("Ferramentas MCP  {}", report.tools.len())).size(13.0).color(look.text)).id_salt("ctx_tools");
    tools.show(ui, |ui| {
        for t in &report.tools {
            line(ui, look, Mark::Bare, &format!("{} · {}", t.server, t.name), &short(t.tokens), "");
        }
    });
    let memory = egui::CollapsingHeader::new(egui::RichText::new(format!("Arquivos de memória  {}", report.memory.len())).size(13.0).color(look.text))
        .id_salt("ctx_memory");
    memory.show(ui, |ui| {
        for (path, tokens) in &report.memory {
            let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
            line(ui, look, Mark::Bare, name, &short(*tokens), "");
        }
    });
}

fn empty(ui: &mut egui::Ui, look: &Look, note: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(note).size(12.5).color(look.muted));
}

/// Os conectores: o estado, quantas ferramentas e, aberto, quais.
fn servers(ui: &mut egui::Ui, info: Option<&Info>, look: &Look) {
    let list: &[Server] = match info {
        Some(i) => i.init.as_ref().map_or(i.listed.as_slice(), |init| init.servers.as_slice()),
        None => &[],
    };
    ui.add_space(4.0);
    let title = egui::RichText::new(format!("Conectores  {}", list.len())).size(13.0).color(look.text);
    egui::CollapsingHeader::new(title).id_salt("ctx_servers").default_open(true).show(ui, |ui| connectors(ui, info, look, list));
}

fn connectors(ui: &mut egui::Ui, info: Option<&Info>, look: &Look, list: &[Server]) {
    if list.is_empty() {
        let reading = info.is_none_or(|i| i.listing);
        return empty(ui, look, if reading { "Lendo os conectores…" } else { "Nenhum conector configurado." });
    }
    let tools: Vec<String> = match info {
        Some(i) => i.init.as_ref().map(|init| init.mcp_tools.clone()).unwrap_or_default(),
        None => Vec::new(),
    };
    for server in list {
        let mine: Vec<&str> = tools.iter().filter(|t| t.starts_with(&format!("mcp__{}__", norm(&server.name)))).map(|t| tool_name(t, &server.name)).collect();
        let count = if tools.is_empty() { None } else { Some(mine.len()) };
        let color = match server.status {
            Status::Connected => crate::app::chat_list::GREEN,
            Status::Failed => crate::app::chat_list::RED,
            Status::NeedsAuth => crate::app::chat_list::AMBER,
            Status::Pending => look.muted,
        };
        let id = ui.make_persistent_id(("ctx_server", &server.name));
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
                let p = ui.painter();
                p.circle_filled(Pos2::new(rect.left() + 5.0, rect.center().y), 4.0, color);
                let right = match count {
                    Some(n) => format!("{n} ferramentas"),
                    None => String::new(),
                };
                let g = p.layout_no_wrap(right, FontId::proportional(11.5), look.muted);
                p.galley(Pos2::new(rect.right() - g.size().x, rect.center().y - g.size().y / 2.0), g.clone(), look.muted);
                let name = clipped(p, server.name.clone(), FontId::proportional(12.5), look.text, (rect.width() - g.size().x - 28.0).max(20.0));
                p.galley(Pos2::new(rect.left() + 16.0, rect.center().y - name.size().y / 2.0), name, look.text);
                response.on_hover_text(server.status.label());
            })
            .body(|ui| {
                ui.label(egui::RichText::new(server.status.label()).size(11.5).color(color));
                for name in &mine {
                    ui.label(egui::RichText::new(*name).size(11.5).color(look.soft));
                }
            });
    }
}

/// O rodapé: quanto falta para o auto-compact e Compactar sessão.
fn footer_ui(ui: &mut egui::Ui, info: Option<&Info>, look: &Look, compact: &mut bool) {
    if let Some(report) = info.and_then(|i| i.report.as_ref()) {
        let left = report.until_compact().saturating_sub(info.map_or(0, Info::since));
        ui.label(egui::RichText::new(format!("{} até o auto-compact", short(left))).size(12.0).color(look.muted));
    }
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Resume a conversa para liberar contexto (/compact)");
    let fill = if response.hovered() { look.raised } else { look.surface };
    ui.painter().rect_filled(rect, 8, fill);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Compactar sessão", FontId::proportional(13.5), look.text);
    // ícone de setas para dentro, à esquerda do texto
    glyph(
        ui.painter(),
        Pos2::new(rect.center().x - 78.0, rect.center().y),
        14.0,
        2.0,
        look.text,
        &[&[(5.0, 9.0), (12.0, 12.0), (19.0, 9.0)], &[(5.0, 15.0), (12.0, 12.0), (19.0, 15.0)]],
    );
    *compact = response.clicked();
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
        app.create_chat(&crate::app::chat_direct::NewChat { agent: 0, cwd: String::new(), mode: 0 }, &installed);
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
