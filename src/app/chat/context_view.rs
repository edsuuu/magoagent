//! O desenho do painel Contexto: o cabeçalho com Atualizar e Fechar, a porcentagem grande, a barra
//! empilhada por categoria com a legenda, a faixa de quando compacta com Compactar, o valor de
//! "Compactar automaticamente em", os conectores (os com problema primeiro, os demais recolhidos em
//! "+ N") e, no rodapé, os arquivos de memória. Os dados são os de chat/context_ui.rs.

use crate::app::chat::context::*;
use crate::app::chat::context_ui::{Info, connectors_note, summary_note};
use crate::app::chat::header::READY;
use crate::app::chat::list::{ACCENT, AMBER, CROSS, DOWN, Look, RED, bold, clipped, glyph};
use crate::app::chat::thinking::CLAUDE;
use crate::prelude::*;

/// O nome na legenda, a cor e se conta no uso (as adiadas não entram na janela).
pub(crate) fn style(name: &str) -> (&'static str, Color32, bool) {
    match name {
        "Messages" => ("Mensagens", CLAUDE, true),
        "System tools" => ("Ferramentas do sistema", rgb_hex(0x5e5ce6), true),
        "MCP tools" => ("Ferramentas MCP", ACCENT, true),
        "Skills" => ("Skills", rgb_hex(0xbf5af2), true),
        "System prompt" => ("Prompt do sistema", rgb_hex(0x8e8e93), true),
        "MCP server instructions" => ("Instruções dos servidores MCP", rgb_hex(0x64d2ff), true),
        "Memory files" => ("Memória (CLAUDE.md)", READY, true),
        "Autocompact buffer" => ("Reserva da auto-compactação", rgb_hex(0x48484a), true),
        "Free space" => ("Livre", rgb_hex(0x2c2c2e), false),
        "MCP tools (deferred)" => ("Ferramentas MCP (adiadas)", rgb_hex(0x2c2c2e), false),
        "System tools (deferred)" => ("Ferramentas do sistema (adiadas)", rgb_hex(0x2c2c2e), false),
        _ => ("", rgb_hex(0x636366), true),
    }
}

/// `style` com a linha do que cresceu desde o último `/context` ("Since"), nas Mensagens esmaecidas.
pub(crate) fn key_style(key: &str) -> (&'static str, Color32, bool) {
    match key {
        "Since" => ("Desde a última atualização", style("Messages").1.gamma_multiply(0.6), true),
        _ => style(key),
    }
}

/// A ordem da legenda, como no painel do Claude Code.
pub(crate) const ORDER: [&str; 12] = [
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

/// A cor da letra do conector, tirada do nome: sempre a mesma para ele.
pub(crate) fn initial_ink(name: &str) -> Color32 {
    const INKS: [u32; 6] = [0xd97757, 0x64a8ff, 0xffb340, 0xff6961, 0x4cd47c, 0xbf8cff];
    let k = name.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) as usize % INKS.len();

    rgb_hex(INKS[k])
}

/// A letra do conector: o fundo (o do chat puxado para a cor) e a cor.
fn initial_colors(name: &str, look: &Look) -> (Color32, Color32) {
    let ink = initial_ink(name);

    (look.bg.lerp_to_gamma(ink, if look.dark { 0.14 } else { 0.18 }), ink)
}

/// A primeira letra ou algarismo do nome, em maiúscula.
pub(crate) fn initial(name: &str) -> String {
    name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default()
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
    ui.label(egui::RichText::new(note).size(13.0).color(look.muted));
}

/// A porcentagem, a barra empilhada e a legenda, a faixa de quando compacta e o seletor.
fn summary(ui: &mut egui::Ui, info: Option<&Info>, look: &Look, current: &str, asked: &mut Asked) {
    if let Some(note) = summary_note(info) {
        return empty(ui, look, &note);
    }

    let Some((info, Some(report))) = info.map(|info| (info, info.report.as_ref())) else { return };

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
    let small = p.layout_no_wrap(format!("{} de {} tokens", tokens(used), tokens(report.window)), FontId::proportional(13.0), look.muted);

    p.galley(Pos2::new(row.left() + big_w + 8.0, row.bottom() - small.size().y - 4.0), small, look.muted);
    // a barra: cada categoria que conta, em ordem, e o que sobra
    let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());

    ui.painter().rect_filled(bar, 4, look.surface);
    let clip = ui.painter().with_clip_rect(bar);
    let mut x = bar.left();

    for key in ORDER.iter().filter(|k| style(k).2) {
        let w = bar.width() * (value(key) as f32 / window as f32);

        if w >= 0.5 {
            clip.rect_filled(Rect::from_min_size(Pos2::new(x, bar.top()), Vec2::new(w, bar.height())), 0, key_style(key).1);
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

            let (name, color, counts) = key_style(key);
            let (row, hover) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());

            if counts {
                hover.tip(format!("{} da janela", percent(value(key) as f32 / window as f32 * 100.0)));
            }

            let p = ui.painter();
            let square = Rect::from_center_size(Pos2::new(row.left() + 4.0, row.center().y), Vec2::splat(8.0));

            if counts {
                p.rect_filled(square, 2, color);
            } else {
                p.rect_stroke(square, 2, Stroke::new(1.0, look.raised), StrokeKind::Inside);
            }

            let font = FontId::proportional(13.0);
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
        let font = FontId::proportional(13.0);
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

    p.text(Pos2::new(row.left(), row.center().y), egui::Align2::LEFT_CENTER, "Compactar automaticamente em", FontId::proportional(13.0), look.muted);
    let shown = if current.is_empty() { "Automático".to_string() } else { current.to_string() };
    let g = p.layout_no_wrap(shown, FontId::proportional(13.0), look.text);
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

    crate::app::ui::menu_look::click_menu(&r, |ui| {
        use crate::app::ui::menu_look::menu_choice;

        if menu_choice(ui, current.is_empty(), "Automático", true) {
            asked.choice = Some(String::new());
            ui.close();
        }

        for value in COMPACT_CHOICES {
            if menu_choice(ui, current == value, value, true) {
                asked.choice = Some(value.to_string());
                ui.close();
            }
        }
    });

    // as ferramentas MCP, uma a uma com os tokens
    if !report.tools.is_empty() {
        let title = egui::RichText::new(format!("Ferramentas MCP  {}", report.tools.len())).size(13.0).color(look.soft);

        egui::CollapsingHeader::new(title).id_salt("ctx_tools").show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;

            for t in &report.tools {
                let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                let p = ui.painter();
                let font = FontId::proportional(12.0);
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
    let sorted: Vec<&Server> = info.map(Info::ranked).unwrap_or_default();
    let width = ui.available_width();
    let (head, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::hover());
    let p = ui.painter();
    let mut job = egui::text::LayoutJob::default();
    let format = egui::TextFormat { font_id: FontId::proportional(12.0), color: look.faint, extra_letter_spacing: 0.66, ..Default::default() };

    job.append("CONECTORES", 0.0, format);
    let g = p.layout_job(job);
    let y = head.bottom() - 12.0;

    p.galley(Pos2::new(head.left() + 18.0, y - g.size().y / 2.0), g, look.faint);

    if !sorted.is_empty() {
        p.text(Pos2::new(head.right() - 18.0, y), egui::Align2::RIGHT_CENTER, connected_words(&sorted), FontId::proportional(12.0), look.faint);
    }

    egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 12 }).show(ui, |ui| {
        ui.set_width(width - 16.0);
        ui.spacing_mut().item_spacing.y = 2.0;

        if sorted.is_empty() {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| empty(ui, look, connectors_note(info)));

            return;
        }

        let tools: &[String] = info.map_or(&[], Info::tools);
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
            Some(more_label(rest))
        } else {
            None
        };

        if let Some(label) = label {
            let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            let color = if response.hovered() { look.text } else { look.muted };

            ui.painter().text(Pos2::new(row.left() + 10.0, row.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(13.0), color);

            if response.clicked() {
                ui.data_mut(|d| d.insert_temp(more_id, !open));
            }
        }
    });
}

/// Uma linha de conector; um clique mostra as ferramentas dele. Diz se Tentar foi clicado.
fn connector_row(ui: &mut egui::Ui, server: &Server, tools: &[String], look: &Look) -> bool {
    let mine = server_tools(tools, &server.name);
    let id = ui.make_persistent_id(("ctx_server", &server.name));
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
    let note = connector_note(server, tools);

    let note_color = match server.status {
        Status::NeedsAuth => AMBER,
        Status::Failed => RED,
        _ => look.muted,
    };

    // Tentar: só nos que falharam, o que o `claude mcp list` faz de verdade
    let button = (server.status == Status::Failed).then(|| {
        let w = ui.painter().layout_no_wrap("Tentar".into(), FontId::proportional(12.0), look.text).size().x + 18.0;

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
    let letter = initial(&server.name);
    let g = p.layout_no_wrap(letter.clone(), FontId::proportional(13.0), ink);

    bold(p, square.center() - g.size() / 2.0, &letter, 12.0, ink);
    let right = button.map_or(rect.right() - 24.0, |b| b.left() - 8.0);
    let room = (right - square.right() - 10.0).max(20.0);
    let name = clipped(p, server.name.clone(), FontId::proportional(14.0), look.text, room);

    p.galley(Pos2::new(square.right() + 10.0, rect.top() + 5.0), name, look.text);
    let g = clipped(p, note, FontId::proportional(12.0), note_color, room);

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
        p.text(b.center(), egui::Align2::CENTER_CENTER, "Tentar", FontId::proportional(12.0), look.text);
        clicked = r.clicked();
    }

    let response = if server.status == Status::NeedsAuth { response.tip("Entre pelo /mcp numa sessão do Claude Code") } else { response };

    if !mine.is_empty() && response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        state.toggle(ui);
    }

    state.show_body_unindented(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin { left: 46, right: 10, top: 2, bottom: 6 }).show(ui, |ui| {
            for name in &mine {
                ui.label(egui::RichText::new(*name).size(12.0).color(look.soft));
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
    let font = FontId::proportional(13.0);
    let v = p.text(Pos2::new(rect.right() - 18.0, y), egui::Align2::RIGHT_CENTER, tokens(total), font.clone(), look.muted);
    let g = clipped(p, report.memory_words(), font, look.muted, (v.left() - rect.left() - 46.0).max(20.0));
    let text = Rect::from_min_size(Pos2::new(rect.left() + 36.0, y - g.size().y / 2.0), g.size());

    p.galley(text.min, g, look.muted);
    ui.interact(text, ui.scope_id().with("ctx_memory"), Sense::hover()).tip(report.memory_tip());
}
