//! O desenho do painel Contexto: o cabeçalho com Atualizar e Fechar, a porcentagem grande, a barra
//! empilhada por categoria com a legenda, a faixa de quando compacta com Compactar, o valor de
//! "Compactar automaticamente em", os conectores (os com problema primeiro, os demais recolhidos em
//! "+ N") e, no rodapé, os arquivos de memória. Os dados são os de chat_context_ui.rs.

use crate::app::chat_context::*;
use crate::app::chat_context_ui::Info;
use crate::app::chat_header::READY;
use crate::app::chat_list::{ACCENT, AMBER, CROSS, DOWN, Look, RED, bold, clipped, glyph};
use crate::app::chat_thinking::CLAUDE;
use crate::prelude::*;

/// O nome na legenda, a cor e se conta no uso (as adiadas não entram na janela).
pub(crate) fn style(name: &str) -> (&'static str, Color32, bool) {
    let rgb = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    match name {
        "Messages" => ("Mensagens", CLAUDE, true),
        "System tools" => ("Ferramentas do sistema", rgb(0x5e5ce6), true),
        "MCP tools" => ("Ferramentas MCP", ACCENT, true),
        "Skills" => ("Skills", rgb(0xbf5af2), true),
        "System prompt" => ("Prompt do sistema", rgb(0x8e8e93), true),
        "MCP server instructions" => ("Instruções dos servidores MCP", rgb(0x64d2ff), true),
        "Memory files" => ("Memória (CLAUDE.md)", READY, true),
        "Autocompact buffer" => ("Reserva da auto-compactação", rgb(0x48484a), true),
        "Free space" => ("Livre", rgb(0x2c2c2e), false),
        "MCP tools (deferred)" => ("Ferramentas MCP (adiadas)", rgb(0x2c2c2e), false),
        "System tools (deferred)" => ("Ferramentas do sistema (adiadas)", rgb(0x2c2c2e), false),
        _ => ("", rgb(0x636366), true),
    }
}

/// A ordem da legenda, como no painel do Claude Code.
const ORDER: [&str; 12] = [
    "System prompt",
    "System tools",
    "MCP tools",
    "MCP server instructions",
    "Skills",
    "Memory files",
    "Messages",
    "Since",
    "Autocompact buffer",
    "Free space",
    "MCP tools (deferred)",
    "System tools (deferred)",
];

/// "301,8k": os tokens com a vírgula daqui.
fn tokens(n: u64) -> String {
    short(n).replace('.', ",")
}

/// A letra do conector em cores tiradas do nome, sempre as mesmas para ele.
fn initial_colors(name: &str, look: &Look) -> (Color32, Color32) {
    const INKS: [u32; 6] = [0xd97757, 0x64a8ff, 0xffb340, 0xff6961, 0x4cd47c, 0xbf8cff];
    let k = name.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) as usize % INKS.len();
    let v = INKS[k];
    let ink = Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    (look.bg.lerp_to_gamma(ink, if look.dark { 0.14 } else { 0.18 }), ink)
}

/// O que o painel pede, aplicado depois do quadro.
#[derive(Default)]
struct Asked {
    refresh: bool,
    compact: bool,
    close: bool,
    retry: bool,
    choice: Option<String>,
}

impl DrawingApp {
    /// Pinta o painel e aplica na hora o que se clicou.
    pub(crate) fn context_panel(&mut self, ctx: &Context, rect: Rect, look: &Look, id: &str) {
        let mut asked = Asked::default();
        let info = self.chat_ctx.info.get(id);
        let current = self.prefs.chat_autocompact.clone();
        let ground = look.bg.lerp_to_gamma(look.panel, 0.4);
        // sempre acima: a área do chat sobe sozinha quando se clica nela e cobriria o painel
        egui::Area::new(egui::Id::unique("chat_context")).fixed_pos(rect.min).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            let p = ui.painter();
            p.rect_filled(rect, 0, ground);
            p.vline(rect.left() + 0.5, rect.y_range(), Stroke::new(1.0, look.hover));
            // o cabeçalho: Contexto, Atualizar e Fechar
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 56.0));
            let title = p.layout_no_wrap("Contexto".into(), FontId::proportional(14.0), look.strong);
            bold(p, Pos2::new(head.left() + 18.0, head.center().y - title.size().y / 2.0), "Contexto", 14.0, look.strong);
            p.hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0, look.hover));
            let x = Rect::from_center_size(Pos2::new(head.right() - 26.0, head.center().y), Vec2::splat(28.0));
            let xr = ui.interact(x, ui.scope_id().with("ctx_close"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Fechar (Esc)");
            if xr.hovered() {
                ui.painter().rect_filled(x, 7, look.hover);
            }
            glyph(ui.painter(), x.center(), 14.0, 2.2, if xr.hovered() { look.text } else { look.soft }, CROSS);
            asked.close = xr.clicked();
            let r = x.translate(Vec2::new(-32.0, 0.0));
            let loading = info.is_some_and(|i| i.loading);
            let rr = ui.interact(r, ui.scope_id().with("ctx_refresh"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(if loading {
                "Atualizando…"
            } else {
                "Atualizar"
            });
            if rr.hovered() {
                ui.painter().rect_filled(r, 7, look.hover);
            }
            refresh_icon(
                ui.painter(),
                r.center(),
                if rr.hovered() || loading { look.text } else { look.soft },
                if loading { ui.input(|i| i.time) } else { 0.0 },
            );
            asked.refresh = rr.clicked();
            // o rodapé: a memória
            let memory = info.and_then(|i| i.report.as_ref()).filter(|r| !r.memory.is_empty());
            let foot = if memory.is_some() { 44.0 } else { 0.0 };
            if let Some(report) = memory {
                let foot_rect = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - foot), rect.max);
                memory_footer(ui, foot_rect, report, look);
            }
            let body = Rect::from_min_max(Pos2::new(rect.left() + 1.0, head.bottom()), Pos2::new(rect.right(), rect.bottom() - foot));
            ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                egui::ScrollArea::vertical().id_salt("chat_context_body").auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_width(body.width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    egui::Frame::new().inner_margin(egui::Margin { left: 18, right: 18, top: 16, bottom: 16 }).show(ui, |ui| {
                        ui.set_width(body.width() - 36.0);
                        ui.spacing_mut().item_spacing.y = 12.0;
                        summary(ui, info, look, &current, &mut asked);
                    });
                    let (line, _) = ui.allocate_exact_size(Vec2::new(body.width(), 1.0), Sense::hover());
                    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, look.hover));
                    connectors(ui, info, look, &mut asked.retry);
                });
            });
        });
        self.chat_ctx.rect = Some(rect);
        if asked.close {
            self.prefs.chat_panel = false;
            self.chat_ctx.rect = None;
        }
        if asked.refresh
            && let Some(info) = self.chat_ctx.info.get_mut(id)
        {
            (info.dirty, info.error) = (true, None);
        }
        if let Some(value) = asked.choice {
            self.prefs.chat_autocompact = value;
            if let Some(info) = self.chat_ctx.info.get_mut(id) {
                info.dirty = true;
            }
        }
        // Tentar: o `claude mcp list` testa os conectores de novo
        if asked.retry
            && let Some(info) = self.chat_ctx.info.get_mut(id)
        {
            (info.retried, info.listing) = (true, true);
            self.spawn_servers(ctx, id);
        }
        if asked.compact {
            self.chat_submit(ctx, id, "/compact", "/compact");
        }
    }
}

/// ↻: um arco quase inteiro com a ponta da seta; girando por `t` enquanto atualiza.
fn refresh_icon(p: &Painter, at: Pos2, color: Color32, t: f64) {
    let turn = (t * 4.0) as f32;
    let r = 6.0;
    let points: Vec<Pos2> = (0..=24).map(|k| at + Vec2::angled(turn + 0.3 + k as f32 / 24.0 * 5.3) * r).collect();
    let end = *points.last().unwrap_or(&at);
    p.add(Shape::line(points, Stroke::new(1.8, color)));
    // the head points back along the arc's way
    let back = turn + 5.6 - std::f32::consts::FRAC_PI_2;
    for wing in [-0.65, 0.65] {
        p.line_segment([end, end + Vec2::angled(back + wing) * 4.0], Stroke::new(1.8, color));
    }
}

fn empty(ui: &mut egui::Ui, look: &Look, note: &str) {
    ui.label(egui::RichText::new(note).size(12.5).color(look.muted));
}

/// A porcentagem, a barra empilhada e a legenda, a faixa de quando compacta e o seletor.
fn summary(ui: &mut egui::Ui, info: Option<&Info>, look: &Look, current: &str, asked: &mut Asked) {
    let Some(info) = info else { return empty(ui, look, "Mande uma mensagem para ver o contexto.") };
    let Some(report) = &info.report else {
        let note = match (&info.error, info.loading, info.live) {
            (Some(error), ..) => format!("Não foi possível ler o contexto: {error}"),
            (_, true, _) => "Lendo o contexto…".to_string(),
            (_, _, live) if live > 0 => format!("{} tokens em uso.", tokens(live)),
            _ => "Mande uma mensagem para ver o contexto.".to_string(),
        };
        return empty(ui, look, &note);
    };
    let since = info.since();
    let used = report.used + since;
    let window = report.window.max(1);
    let value = |key: &str| if key == "Since" { since } else { report.cat(key) };
    // 75%  301,8k de 400k tokens, na mesma linha de base
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
    let p = ui.painter();
    let big = p.layout_no_wrap(format!("{:.0}%", used as f64 / window as f64 * 100.0), FontId::proportional(26.0), look.strong);
    let at = Pos2::new(row.left(), row.bottom() - big.size().y);
    p.galley(at + Vec2::new(1.0 / p.pixels_per_point(), 0.0), big.clone(), look.strong);
    let big_w = big.size().x;
    p.galley(at, big, look.strong);
    let small = p.layout_no_wrap(format!("{} de {} tokens", tokens(used), tokens(report.window)), FontId::proportional(12.0), look.muted);
    p.galley(Pos2::new(row.left() + big_w + 8.0, row.bottom() - small.size().y - 4.0), small, look.muted);
    // a barra: cada categoria que conta, em ordem, e o que sobra
    let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());
    ui.painter().rect_filled(bar, 4, look.surface);
    let clip = ui.painter().with_clip_rect(bar);
    let mut x = bar.left();
    for key in ORDER.iter().filter(|k| style(k).2) {
        let w = bar.width() * (value(key) as f32 / window as f32);
        if w >= 0.5 {
            let color = if *key == "Since" { style("Messages").1.gamma_multiply(0.6) } else { style(key).1 };
            clip.rect_filled(Rect::from_min_size(Pos2::new(x, bar.top()), Vec2::new(w, bar.height())), 0, color);
            x += w;
        }
    }
    // a legenda: quadradinho, nome e os tokens à direita
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 7.0;
        for key in ORDER {
            let known = if key == "Since" { since > 0 } else { report.cats.iter().any(|c| c.name == key) };
            if !known {
                continue;
            }
            let (name, color, counts) = style(key);
            let name = if key == "Since" { "Desde a última atualização" } else { name };
            let color = if key == "Since" { style("Messages").1.gamma_multiply(0.6) } else { color };
            let (row, hover) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
            if counts {
                hover.on_hover_text(format!("{} da janela", percent(value(key) as f32 / window as f32 * 100.0)));
            }
            let p = ui.painter();
            let square = Rect::from_center_size(Pos2::new(row.left() + 4.0, row.center().y), Vec2::splat(8.0));
            if counts {
                p.rect_filled(square, 2, color);
            } else {
                p.rect_stroke(square, 2, Stroke::new(1.0, look.raised), StrokeKind::Inside);
            }
            let font = FontId::proportional(12.0);
            let v = p.text(Pos2::new(row.right(), row.center().y), egui::Align2::RIGHT_CENTER, tokens(value(key)), font.clone(), look.muted);
            let g = clipped(p, name.to_string(), font, look.soft, (v.left() - row.left() - 26.0).max(20.0));
            p.galley(Pos2::new(row.left() + 18.0, row.center().y - g.size().y / 2.0), g, look.soft);
        }
    });
    // a faixa: quando compacta sozinho, e Compactar agora
    let left = report.until_compact().saturating_sub(since);
    egui::Frame::new().fill(look.hover).corner_radius(9).inner_margin(egui::Margin::symmetric(10, 9)).show(ui, |ui| {
        let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
        let p = ui.painter();
        let clock = Pos2::new(row.left() + 7.0, row.center().y);
        p.circle_stroke(clock, 6.0, Stroke::new(1.6, AMBER));
        p.line_segment([clock + Vec2::new(0.0, -3.5), clock], Stroke::new(1.6, AMBER));
        p.line_segment([clock, clock + Vec2::new(2.5, 1.7)], Stroke::new(1.6, AMBER));
        let font = FontId::proportional(12.0);
        let words = p.layout_no_wrap("Compacta sozinho em ".into(), font.clone(), look.soft);
        let words_w = words.size().x;
        p.galley(Pos2::new(row.left() + 22.0, row.center().y - words.size().y / 2.0), words, look.soft);
        bold(p, Pos2::new(row.left() + 22.0 + words_w, row.center().y - 7.5), &tokens(left), 12.0, look.text);
        let label = p.layout_no_wrap("Compactar".into(), font, look.bg);
        let button = Rect::from_min_max(Pos2::new(row.right() - label.size().x - 20.0, row.top()), row.max);
        let r = ui
            .interact(button, ui.scope_id().with("ctx_compact"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .tip("Resume a conversa para liberar contexto (/compact)");
        let p = ui.painter();
        p.rect_filled(button, 7, if r.hovered() { look.strong } else { look.text });
        bold(p, button.center() - label.size() / 2.0, "Compactar", 12.0, look.bg);
        asked.compact = r.clicked();
    });
    // Compactar automaticamente em [Automático ▾]
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    let p = ui.painter();
    p.text(Pos2::new(row.left(), row.center().y), egui::Align2::LEFT_CENTER, "Compactar automaticamente em", FontId::proportional(12.0), look.muted);
    let shown = if current.is_empty() { "Automático".to_string() } else { current.to_string() };
    let g = p.layout_no_wrap(shown, FontId::proportional(12.0), look.text);
    let pick = Rect::from_min_max(Pos2::new(row.right() - g.size().x - 30.0, row.top()), row.max);
    let r = ui
        .interact(pick, ui.scope_id().with("ctx_autocompact"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .tip("Um valor menor consome menos do seu limite. O Claude resume as mensagens mais antigas mais cedo.");
    let p = ui.painter();
    if r.hovered() {
        p.rect_filled(pick, 7, look.hover);
    }
    p.rect_stroke(pick, 7, Stroke::new(1.0, look.surface), StrokeKind::Inside);
    p.galley(Pos2::new(pick.left() + 8.0, pick.center().y - g.size().y / 2.0), g, look.text);
    glyph(p, Pos2::new(pick.right() - 12.0, pick.center().y), 10.0, 2.6, look.text, DOWN);
    egui::Popup::menu(&r).show(|ui| {
        ui.set_min_width(110.0);
        if ui.selectable_label(current.is_empty(), "Automático").clicked() {
            asked.choice = Some(String::new());
        }
        for value in COMPACT_CHOICES {
            if ui.selectable_label(current == value, value).clicked() {
                asked.choice = Some(value.to_string());
            }
        }
    });
    // as ferramentas MCP, uma a uma com os tokens
    if !report.tools.is_empty() {
        let title = egui::RichText::new(format!("Ferramentas MCP  {}", report.tools.len())).size(12.0).color(look.soft);
        egui::CollapsingHeader::new(title).id_salt("ctx_tools").show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for t in &report.tools {
                let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                let p = ui.painter();
                let font = FontId::proportional(11.5);
                let v = p.text(Pos2::new(row.right(), row.center().y), egui::Align2::RIGHT_CENTER, tokens(t.tokens), font.clone(), look.muted);
                let g = clipped(p, format!("{} · {}", t.server, t.name), font, look.soft, (v.left() - row.left() - 8.0).max(20.0));
                p.galley(Pos2::new(row.left(), row.center().y - g.size().y / 2.0), g, look.soft);
            }
        });
    }
}

/// Os conectores: o rótulo com quantos estão conectados; os com problema primeiro; cada um com a
/// inicial, o nome, as ferramentas ou o estado e a bolinha verde; Tentar nos que falharam.
fn connectors(ui: &mut egui::Ui, info: Option<&Info>, look: &Look, retry: &mut bool) {
    let list: Vec<&Server> = info.map(|i| i.servers().iter().collect()).unwrap_or_default();
    let width = ui.available_width();
    let (head, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::hover());
    let p = ui.painter();
    let mut job = egui::text::LayoutJob::default();
    let format = egui::TextFormat { font_id: FontId::proportional(11.0), color: look.faint, extra_letter_spacing: 0.66, ..Default::default() };
    job.append("CONECTORES", 0.0, format);
    let g = p.layout_job(job);
    let y = head.bottom() - 12.0;
    p.galley(Pos2::new(head.left() + 18.0, y - g.size().y / 2.0), g, look.faint);
    if !list.is_empty() {
        let on = list.iter().filter(|s| s.status == Status::Connected).count();
        let words = format!("{on} de {} conectados", list.len());
        p.text(Pos2::new(head.right() - 18.0, y), egui::Align2::RIGHT_CENTER, words, FontId::proportional(11.0), look.faint);
    }
    egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 12 }).show(ui, |ui| {
        ui.set_width(width - 16.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        if list.is_empty() {
            let reading = info.is_none_or(|i| i.listing);
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(10, 4))
                .show(ui, |ui| empty(ui, look, if reading { "Lendo os conectores…" } else { "Nenhum conector configurado." }));
            return;
        }
        let tools: &[String] = info.and_then(|i| i.init.as_ref()).map_or(&[], |init| init.mcp_tools.as_slice());
        // os que pedem atenção primeiro
        let rank = |s: &Server| match s.status {
            Status::Failed => 0,
            Status::NeedsAuth => 1,
            Status::Pending => 2,
            Status::Connected => 3,
        };
        let mut sorted = list.clone();
        sorted.sort_by_key(|s| rank(s));
        let more_id = ui.make_persistent_id("ctx_more");
        let open = ui.data(|d| d.get_temp::<bool>(more_id).unwrap_or(false));
        let shown = if open || sorted.len() <= 6 { sorted.len() } else { 5 };
        for server in &sorted[..shown] {
            *retry |= connector_row(ui, server, tools, look);
        }
        let rest = &sorted[shown..];
        let label = if open && sorted.len() > 6 {
            Some("Mostrar menos".to_string())
        } else if !rest.is_empty() {
            let all_on = rest.iter().all(|s| s.status == Status::Connected);
            Some(format!("+ {} {}", rest.len(), if all_on { "conectados" } else { "conectores" }))
        } else {
            None
        };
        if let Some(label) = label {
            let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            let color = if response.hovered() { look.text } else { look.muted };
            ui.painter().text(Pos2::new(row.left() + 10.0, row.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(12.0), color);
            if response.clicked() {
                ui.data_mut(|d| d.insert_temp(more_id, !open));
            }
        }
    });
}

/// Uma linha de conector; um clique mostra as ferramentas dele. Diz se Tentar foi clicado.
fn connector_row(ui: &mut egui::Ui, server: &Server, tools: &[String], look: &Look) -> bool {
    let mine: Vec<&str> = tools.iter().filter(|t| t.starts_with(&format!("mcp__{}__", norm(&server.name)))).map(|t| tool_name(t, &server.name)).collect();
    let id = ui.make_persistent_id(("ctx_server", &server.name));
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
    let (note, note_color) = match server.status {
        Status::Connected if !tools.is_empty() => (format!("{} ferramentas", mine.len()), look.muted),
        Status::Connected => ("Conectado".to_string(), look.muted),
        Status::NeedsAuth => ("Precisa entrar".to_string(), AMBER),
        Status::Failed => ("Não respondeu".to_string(), RED),
        Status::Pending => (server.status.label().to_string(), look.muted),
    };
    // Tentar: só nos que falharam, o que o `claude mcp list` faz de verdade
    let button = (server.status == Status::Failed).then(|| {
        let w = ui.painter().layout_no_wrap("Tentar".into(), FontId::proportional(11.0), look.text).size().x + 18.0;
        Rect::from_center_size(Pos2::new(rect.right() - 10.0 - w / 2.0, rect.center().y), Vec2::new(w, 24.0))
    });
    let tried = button.map(|b| {
        ui.interact(b, ui.scope_id().with(("ctx_retry", &server.name)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Testa a conexão de novo")
    });
    let p = ui.painter();
    if response.hovered() || state.is_open() {
        p.rect_filled(rect, 9, look.hover);
    }
    let square = Rect::from_center_size(Pos2::new(rect.left() + 23.0, rect.center().y), Vec2::splat(26.0));
    let (fill, ink) = initial_colors(&server.name, look);
    p.rect_filled(square, 7, fill);
    let letter: String = server.name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let g = p.layout_no_wrap(letter.clone(), FontId::proportional(12.0), ink);
    bold(p, square.center() - g.size() / 2.0, &letter, 12.0, ink);
    let right = button.map_or(rect.right() - 24.0, |b| b.left() - 8.0);
    let room = (right - square.right() - 10.0).max(20.0);
    let name = clipped(p, server.name.clone(), FontId::proportional(13.0), look.text, room);
    p.galley(Pos2::new(square.right() + 10.0, rect.top() + 5.0), name, look.text);
    let g = clipped(p, note, FontId::proportional(11.0), note_color, room);
    p.galley(Pos2::new(square.right() + 10.0, rect.top() + 22.0), g, note_color);
    if server.status == Status::Connected {
        p.circle_filled(Pos2::new(rect.right() - 14.0, rect.center().y), 3.5, READY);
    }
    let mut clicked = false;
    if let (Some(b), Some(r)) = (button, &tried) {
        if r.hovered() {
            p.rect_filled(b, 6, look.raised);
        }
        p.rect_stroke(b, 6, Stroke::new(1.0, look.raised), StrokeKind::Inside);
        p.text(b.center(), egui::Align2::CENTER_CENTER, "Tentar", FontId::proportional(11.0), look.text);
        clicked = r.clicked();
    }
    let response = if server.status == Status::NeedsAuth { response.on_hover_text("Entre pelo /mcp numa sessão do Claude Code") } else { response };
    if !mine.is_empty() && response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        state.toggle(ui);
    }
    state.show_body_unindented(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin { left: 46, right: 10, top: 2, bottom: 6 }).show(ui, |ui| {
            for name in &mine {
                ui.label(egui::RichText::new(*name).size(11.5).color(look.soft));
            }
        });
    });
    clicked
}

/// O rodapé: o arquivo de memória (e quantos mais) e os tokens deles.
fn memory_footer(ui: &mut egui::Ui, rect: Rect, report: &Report, look: &Look) {
    let p = ui.painter();
    p.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, look.hover));
    let y = rect.center().y;
    let doc: &[&[(f32, f32)]] = &[&[(6.0, 3.0), (15.0, 3.0), (19.0, 7.0), (19.0, 21.0), (6.0, 21.0), (6.0, 3.0)], &[(14.0, 3.0), (14.0, 8.0), (19.0, 8.0)]];
    glyph(p, Pos2::new(rect.left() + 24.5, y), 13.0, 2.0, look.muted, doc);
    let total: u64 = report.memory.iter().map(|(_, t)| t).sum();
    let font = FontId::proportional(12.0);
    let v = p.text(Pos2::new(rect.right() - 18.0, y), egui::Align2::RIGHT_CENTER, tokens(total), font.clone(), look.muted);
    let first = report.memory[0].0.rsplit(['/', '\\']).next().unwrap_or(&report.memory[0].0);
    let words = match report.memory.len() {
        1 => format!("Memória: {first}"),
        n => format!("Memória: {first} e mais {}", n - 1),
    };
    let g = clipped(p, words, font, look.muted, (v.left() - rect.left() - 46.0).max(20.0));
    let text = Rect::from_min_size(Pos2::new(rect.left() + 36.0, y - g.size().y / 2.0), g.size());
    p.galley(text.min, g, look.muted);
    let all: Vec<String> = report.memory.iter().map(|(path, t)| format!("{path}: {}", tokens(*t))).collect();
    ui.interact(text, ui.scope_id().with("ctx_memory"), Sense::hover()).tip(all.join("\n"));
}
