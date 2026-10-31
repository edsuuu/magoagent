//! The chat's frame: its colors, the agents down the left (name, what it does, a shape per state)
//! or, narrow or folded, a column of their initials, and the header over the conversation. Also
//! the Anexar clip, the code card with its header, a message's Copiar / Citar menu and ⌘F's search.

use crate::app::chat::tint;
use crate::app::ui::agents::{AgentRow, AgentState, ago};
use crate::prelude::*;
use crate::transcript::ChatItem;

/// The whole list's width and the folded column's.
pub(crate) const LIST: f32 = 248.0;
pub(crate) const COLUMN: f32 = 52.0;
/// Narrower than this the list is a column of initials and opens over the conversation.
pub(crate) const NARROW: f32 = 760.0;

pub(crate) const GREEN: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x50);
pub(crate) const AMBER: Color32 = Color32::from_rgb(0xf0, 0xa7, 0x3a);
pub(crate) const RED: Color32 = Color32::from_rgb(0xf8, 0x51, 0x49);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x8b, 0x87, 0xff);

/// The chat's colors: the design's on the dark look; the light one wasn't drawn, so the app's own.
#[derive(Clone, Copy)]
pub(crate) struct Look {
    pub(crate) dark: bool,
    pub(crate) bg: Color32,
    /// The agents' list.
    pub(crate) panel: Color32,
    /// Buttons and the box.
    pub(crate) surface: Color32,
    /// Chips and buttons on the band above the box.
    pub(crate) raised: Color32,
    pub(crate) band: Color32,
    pub(crate) selected: Color32,
    pub(crate) line: Color32,
    pub(crate) text: Color32,
    pub(crate) soft: Color32,
    pub(crate) muted: Color32,
}

impl Look {
    pub(crate) fn new(dark: bool, c: &UiColors) -> Look {
        let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        if dark {
            Look {
                dark,
                bg: hex(0x121212),
                panel: hex(0x222229),
                surface: hex(0x31303c),
                raised: hex(0x2a2a33),
                band: hex(0x1a1a20),
                selected: hex(0x3d3c4b),
                line: hex(0x2c2c36),
                text: hex(0xececf1),
                soft: hex(0xa3a3ad),
                muted: hex(0x8e8e96),
            }
        } else {
            let line = Color32::from_black_alpha(24);
            Look {
                dark,
                bg: Color32::WHITE,
                panel: c.surface_low,
                surface: c.surface_low,
                raised: c.selected,
                band: c.surface_low,
                selected: c.selected,
                line,
                text: c.text,
                soft: c.text.gamma_multiply(0.72),
                muted: c.muted,
            }
        }
    }
}

/// A state as a shape `size` across: a pulsing green dot working, a pulsing amber one when it
/// needs you, an empty ring idle (green when only its screen moves), a grey square ended.
pub(crate) fn state_mark(p: &Painter, at: Pos2, size: f32, state: &AgentState, look: &Look, t: f64) {
    let r = size / 2.0;
    let grey = |v: u8| if look.dark { Color32::from_gray(v) } else { look.muted };
    match state {
        AgentState::Working(_) => pulse_dot(p, at, r, GREEN, 1.6, t),
        AgentState::Waiting => pulse_dot(p, at, r, AMBER, 2.0, t),
        AgentState::Active(_) => {
            p.circle_stroke(at, r - 0.75, Stroke::new(1.5, GREEN));
        }
        AgentState::Idle(_) => {
            p.circle_stroke(at, r - 0.75, Stroke::new(1.5, grey(0x7c)));
        }
        AgentState::Ended => {
            p.rect_filled(Rect::from_center_size(at, Vec2::splat(size)), 2, grey(0x6a));
        }
    }
}

/// A dot with a ring growing out of it, 1 to 2.6 times its size and fading from 0.6, every
/// `period` seconds.
pub(crate) fn pulse_dot(p: &Painter, at: Pos2, r: f32, color: Color32, period: f64, t: f64) {
    let eased = 1.0 - (1.0 - (t.rem_euclid(period) / period) as f32).powi(2);
    p.circle_filled(at, r * (1.0 + 1.6 * eased), color.gamma_multiply(0.6 * (1.0 - eased)));
    p.circle_filled(at, r, color);
}

/// A line icon from the design's 24-unit drawings, `size` across centered at `at`.
pub(crate) fn glyph(p: &Painter, at: Pos2, size: f32, width: f32, color: Color32, lines: &[&[(f32, f32)]]) {
    let k = size / 24.0;
    for points in lines {
        p.add(Shape::line(points.iter().map(|&(x, y)| at + Vec2::new(x - 12.0, y - 12.0) * k).collect(), Stroke::new(width * k, color)));
    }
}

/// The terminal prompt: the Terminal button, Bash's chip, "Abrir terminal".
pub(crate) const PROMPT: &[&[(f32, f32)]] = &[&[(4.5, 7.0), (9.5, 12.0), (4.5, 17.0)], &[(12.0, 17.5), (19.5, 17.5)]];
pub(crate) const CHECK: &[&[(f32, f32)]] = &[&[(5.0, 12.5), (9.5, 17.0), (19.0, 7.5)]];
pub(crate) const CROSS: &[&[(f32, f32)]] = &[&[(7.0, 7.0), (17.0, 17.0)], &[(17.0, 7.0), (7.0, 17.0)]];
pub(crate) const RIGHT: &[&[(f32, f32)]] = &[&[(9.5, 6.0), (15.5, 12.0), (9.5, 18.0)]];
pub(crate) const DOWN: &[&[(f32, f32)]] = &[&[(6.0, 9.5), (12.0, 15.5), (18.0, 9.5)]];
const FOLD: &[&[(f32, f32)]] = &[&[(11.0, 17.0), (6.0, 12.0), (11.0, 7.0)], &[(18.0, 17.0), (13.0, 12.0), (18.0, 7.0)]];
const UNFOLD: &[&[(f32, f32)]] = &[&[(13.0, 17.0), (18.0, 12.0), (13.0, 7.0)], &[(6.0, 17.0), (11.0, 12.0), (6.0, 7.0)]];

/// "Abrir canvas": two nodes and the rope between them.
fn canvas_icon(p: &Painter, at: Pos2, color: Color32) {
    let k = 16.0 / 24.0;
    let box_at = |x: f32, y: f32| Rect::from_min_size(at + Vec2::new(x - 12.0, y - 12.0) * k, Vec2::new(8.0, 6.5) * k);
    for r in [box_at(3.0, 3.5), box_at(13.0, 14.0)] {
        p.rect_stroke(r, 1, Stroke::new(1.35, color), StrokeKind::Middle);
    }
    glyph(p, at, 16.0, 2.0, color, &[&[(11.0, 6.75), (14.5, 6.75), (16.3, 7.5), (17.0, 9.25), (17.0, 14.0)]]);
}

/// "Trabalhando · há 1m 12s": 40 s, 1m 12s, 2 h 5 min.
pub(crate) fn elapsed(seconds: f64) -> String {
    match seconds.max(0.0) as u64 {
        s @ 0..=59 => format!("{s} s"),
        s @ 60..=3599 => format!("{}m {}s", s / 60, s % 60),
        s => format!("{} h {} min", s / 3600, s % 3600 / 60),
    }
}

/// The program a terminal runs, for the header's tag.
fn program(spec: &TerminalSpec, claude: bool) -> Option<&'static str> {
    match spec.command.split_whitespace().next() {
        _ if claude || spec.command.contains("claude") => Some("Claude Code"),
        Some("codex") => Some("Codex"),
        _ => None,
    }
}

/// A rounded button with an icon and maybe words; `on`: pressed (the live terminal shows).
fn header_button(ui: &mut egui::Ui, rect: Rect, salt: &str, look: &Look, on: bool, label: &str, icon: impl FnOnce(&Painter, Pos2, Color32)) -> egui::Response {
    let response = ui.interact(rect, ui.scope_id().with(salt), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let fill = if on {
        look.selected
    } else if response.hovered() {
        look.surface.gamma_multiply(1.25)
    } else {
        look.surface
    };
    let p = ui.painter();
    p.rect_filled(rect, 8, fill);
    let ink = look.text;
    if label.is_empty() {
        icon(p, rect.center(), ink);
    } else {
        icon(p, Pos2::new(rect.left() + 18.0, rect.center().y), ink);
        p.text(Pos2::new(rect.left() + 33.0, rect.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(13.5), ink);
    }
    response
}

/// A name painted twice a pixel apart: the UI font has no bold weight.
pub(crate) fn bold(p: &Painter, at: Pos2, text: &str, size: f32, color: Color32) -> Rect {
    let galley = p.layout_no_wrap(text.into(), FontId::proportional(size), color);
    let rect = Rect::from_min_size(at, galley.size());
    p.galley(at + Vec2::new(1.0 / p.pixels_per_point(), 0.0), galley.clone(), color);
    p.galley(at, galley, color);
    rect
}

/// What the list and the header ask for.
#[derive(Default)]
pub(crate) struct ListAction {
    pub(crate) pick: Option<String>,
    pub(crate) toggle: bool,
    pub(crate) terminal: bool,
    pub(crate) canvas: bool,
    /// Novo chat, and Remover chat on a row.
    pub(crate) new_chat: bool,
    pub(crate) remove: Option<String>,
    /// Chats (0) or Agentes (1) picked at the top of the list.
    pub(crate) tab: Option<usize>,
}

pub(crate) const LIST_TABS: [&str; 2] = ["Chats", "Agentes"];

impl DrawingApp {
    /// How long its agent has been waiting on its terminal, when its Claude Code session says.
    pub(crate) fn waiting_secs(&self, id: &str) -> Option<f64> {
        self.agent_sessions.get(id).filter(|s| s.status == "waiting" && s.status_at > 0).map(|s| (now_ms() - s.status_at).max(0) as f64 / 1000.0)
    }

    /// Under a name in the list: its state and a word more ("Precisa de você · 2 min").
    fn list_note(&self, row: &AgentRow, tool: Option<&str>) -> String {
        let more = match row.state {
            AgentState::Working(_) => tool.map(str::to_string).or_else(|| self.state_note(row)),
            AgentState::Waiting => self.waiting_secs(&row.id).map(|s| ago(s).trim_start_matches("há ").to_string()),
            AgentState::Ended => self.agent_events.iter().rev().find(|e| e.id == row.id).map(|e| ago(self.now - e.at)),
            _ => None,
        };
        let space = row.space.as_ref().map(|s| format!("{s} · ")).unwrap_or_default();
        match more {
            Some(more) => format!("{space}{} · {more}", row.state.label()),
            None => format!("{space}{}", row.state.label()),
        }
    }

    /// The agents, each a row: its mark, name, what it does and its state's shape; `tool` is what
    /// the chosen one runs now.
    pub(crate) fn chat_list(&self, ui: &mut egui::Ui, side: Rect, rows: &[AgentRow], look: &Look, tool: Option<&str>, t: f64, act: &mut ListAction) {
        ui.painter().rect_filled(side, 12, look.panel);
        let inner = side.shrink2(Vec2::new(8.0, 10.0));
        let head = inner.top() + 14.0;
        // Chats | Agentes: the direct chats, or the agents in terminals on the canvas
        let mut x = inner.left() + 4.0;
        for (i, name) in LIST_TABS.iter().enumerate() {
            let on = self.chat_list_tab == i;
            let text = format!("{name} {}", if on { rows.len().to_string() } else { String::new() });
            let label = ui.painter().layout_no_wrap(text.trim_end().to_string(), FontId::proportional(12.5), if on { look.text } else { look.muted });
            let tab = Rect::from_min_size(Pos2::new(x, head - 12.0), Vec2::new(label.size().x + 16.0, 24.0));
            let response = ui.interact(tab, ui.scope_id().with(("chat_tab", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if on || response.hovered() {
                ui.painter().rect_filled(tab, 8, if on { look.raised } else { look.surface });
            }
            ui.painter().galley(tab.center() - label.size() / 2.0, label, if on { look.text } else { look.muted });
            if response.clicked() && !on {
                act.tab = Some(i);
            }
            x = tab.right() + 4.0;
        }
        let fold = Rect::from_center_size(Pos2::new(inner.right() - 14.0, head), Vec2::splat(28.0));
        let response = ui
            .interact(fold, ui.scope_id().with("chat_fold"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(keys_label("Recolher lista (⌘B)"));
        if response.hovered() {
            ui.painter().rect_filled(fold, 7, look.surface);
        }
        glyph(ui.painter(), fold.center(), 16.0, 2.0, look.soft, FOLD);
        act.toggle |= response.clicked();
        // Novo chat: a conversation with an agent installed here, in a folder
        let add = fold.translate(Vec2::new(-32.0, 0.0));
        let response = ui.interact(add, ui.scope_id().with("chat_new"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Novo chat");
        if response.hovered() {
            ui.painter().rect_filled(add, 7, look.surface);
        }
        glyph(ui.painter(), add.center(), 16.0, 2.0, look.soft, &[&[(12.0, 5.0), (12.0, 19.0)], &[(5.0, 12.0), (19.0, 12.0)]]);
        act.new_chat |= response.clicked();
        let list = Rect::from_min_max(Pos2::new(inner.left(), inner.top() + 36.0), inner.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical().id_salt("chat_agents").auto_shrink(false).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for row in rows {
                    let on = self.chat_agent.as_ref() == Some(&row.id);
                    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
                    if self.is_chat(&row.id) {
                        response.context_menu(|ui| {
                            if ui.button("Remover chat").clicked() {
                                act.remove = Some(row.id.clone());
                                ui.close();
                            }
                        });
                    }
                    let p = ui.painter();
                    if on || response.hovered() {
                        p.rect_filled(rect, 10, if on { look.selected } else { look.surface.gamma_multiply(0.6) });
                    }
                    terminal_icon(p, &row.spec, Pos2::new(rect.left() + 20.0, rect.center().y), if on { look.text } else { look.soft });
                    let ended = row.state == AgentState::Ended;
                    let room = rect.width() - 40.0 - 28.0;
                    let name = clipped(p, row.spec.label().into(), FontId::proportional(14.0), if ended { look.soft } else { look.text }, room);
                    p.galley(Pos2::new(rect.left() + 40.0, rect.top() + 8.0), name, look.text);
                    let color = match row.state {
                        AgentState::Waiting => AMBER,
                        AgentState::Ended => look.muted,
                        _ if on => look.text.gamma_multiply(0.8),
                        _ => look.soft,
                    };
                    let note = self.list_note(row, if on { tool } else { None });
                    p.galley(Pos2::new(rect.left() + 40.0, rect.top() + 27.0), clipped(p, note, FontId::proportional(12.0), color, room), color);
                    state_mark(p, Pos2::new(rect.right() - 16.0, rect.center().y), 8.0, &row.state, look, t);
                    if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        act.pick = Some(row.id.clone());
                    }
                }
            });
        });
    }

    /// Folded or narrow: a column of initials, each with its state's badge, under the » button.
    pub(crate) fn chat_column(&self, ui: &mut egui::Ui, side: Rect, rows: &[AgentRow], look: &Look, t: f64, act: &mut ListAction) {
        ui.painter().rect_filled(side, 12, look.panel);
        let unfold = Rect::from_center_size(Pos2::new(side.center().x, side.top() + 24.0), Vec2::new(36.0, 32.0));
        let response = ui
            .interact(unfold, ui.scope_id().with("chat_unfold"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(keys_label("Expandir lista (⌘B)"));
        if response.hovered() {
            ui.painter().rect_filled(unfold, 8, look.surface);
        }
        glyph(ui.painter(), unfold.center(), 16.0, 2.0, look.soft, UNFOLD);
        act.toggle |= response.clicked();
        ui.painter().hline(Rect::from_center_size(unfold.center(), Vec2::new(28.0, 0.0)).x_range(), unfold.bottom() + 8.0, Stroke::new(1.0, look.line));
        let list = Rect::from_min_max(Pos2::new(side.left(), unfold.bottom() + 15.0), side.max - Vec2::new(0.0, 8.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical().id_salt("chat_initials").auto_shrink(false).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for row in rows {
                    let on = self.chat_agent.as_ref() == Some(&row.id);
                    let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::hover());
                    let rect = Rect::from_center_size(strip.center(), Vec2::splat(38.0));
                    let tip = format!("{} · {}", row.spec.label(), row.state.label());
                    let response = ui
                        .interact(rect, ui.scope_id().with(("chat_initial", &row.id)), Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(tip);
                    let ground = if on { look.selected } else { look.panel };
                    let p = ui.painter();
                    if on || response.hovered() {
                        p.rect_filled(rect, 10, if on { look.selected } else { look.surface.gamma_multiply(0.6) });
                    }
                    let initial: String = row.spec.label().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                    let ink = if row.state == AgentState::Ended {
                        look.muted
                    } else if on {
                        look.text
                    } else {
                        look.soft
                    };
                    let g = p.layout_no_wrap(initial.clone(), FontId::proportional(14.0), ink);
                    bold(p, rect.center() - g.size() / 2.0, &initial, 14.0, ink);
                    // the badge, ringed in the ground it sits on
                    let badge = rect.right_bottom() - Vec2::splat(6.5);
                    p.circle_filled(badge, 5.5, ground);
                    state_mark(p, badge, 7.0, &row.state, look, t);
                    if response.clicked() {
                        act.pick = Some(row.id.clone());
                    }
                }
            });
        });
    }

    /// Name, conversation title, program; the state and since when; ⌘F's field (`search`), the
    /// Terminal and canvas buttons (narrow: an icon and "Canvas"). Gives the header's height.
    pub(crate) fn chat_header(
        &self,
        ui: &mut egui::Ui,
        main: Rect,
        row: &AgentRow,
        look: &Look,
        narrow: bool,
        talks: bool,
        t: f64,
        act: &mut ListAction,
        search: &mut Option<String>,
    ) -> f32 {
        let transcript = self.transcripts.get(&row.id);
        let (left, top) = (main.left() + if narrow { 4.0 } else { 8.0 }, main.top() + if narrow { 2.0 } else { 0.0 });
        let height = if narrow { 50.0 } else { 56.0 };
        let middle = top + if narrow { 19.0 } else { 22.0 };
        // the buttons first, at the right: the title is cut before them
        let mut right = main.right();
        let canvas_label = if narrow { "Canvas" } else { "Abrir canvas" };
        let canvas_w = 33.0 + ui.painter().layout_no_wrap(canvas_label.into(), FontId::proportional(13.5), look.text).size().x + 12.0;
        let canvas = Rect::from_min_max(Pos2::new(right - canvas_w, middle - 16.0), Pos2::new(right, middle + 16.0));
        act.canvas |= header_button(ui, canvas, "chat_canvas", look, false, canvas_label, canvas_icon).clicked();
        right = canvas.left() - 8.0;
        if talks && !self.is_chat(&row.id) && !row.spec.chat {
            let width =
                if narrow { 32.0 } else { 33.0 + ui.painter().layout_no_wrap("Terminal".into(), FontId::proportional(13.5), look.text).size().x + 12.0 };
            let rect = Rect::from_min_max(Pos2::new(right - width, middle - 16.0), Pos2::new(right, middle + 16.0));
            let label = if narrow { "" } else { "Terminal" };
            let button = header_button(ui, rect, "chat_live", look, self.chat_terminal, label, |p, at, ink| glyph(p, at, 16.0, 2.0, ink, PROMPT));
            act.terminal |= button.on_hover_text("A tela do terminal, ao vivo").clicked();
            if row.state == AgentState::Waiting {
                // it waits on that terminal: an amber badge on its corner
                ui.painter().circle(rect.right_top() + Vec2::new(-2.0, 2.0), 6.0, AMBER, Stroke::new(2.0, look.bg));
            }
            right = rect.left();
        }
        if let Some(words) = search.as_mut() {
            let field = Rect::from_min_max(Pos2::new(right - 8.0 - if narrow { 130.0 } else { 200.0 }, middle - 16.0), Pos2::new(right - 8.0, middle + 16.0));
            ui.painter().rect_filled(field, 8, look.surface);
            let edit = egui::TextEdit::singleline(words)
                .id(egui::Id::unique(CHAT_SEARCH))
                .hint_text(egui::RichText::new("Buscar na conversa").color(look.muted))
                .font(FontId::proportional(13.0))
                .text_color(look.text)
                .margin(egui::Margin { left: 10, right: 28, top: 8, bottom: 8 })
                .frame(egui::Frame::NONE);
            ui.put(field, edit);
            let close = Rect::from_center_size(Pos2::new(field.right() - 15.0, middle), Vec2::splat(22.0));
            let x = ui
                .interact(close, ui.scope_id().with("chat_search_close"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("Fechar a busca (Esc)");
            glyph(ui.painter(), close.center(), 13.0, 2.0, if x.hovered() { look.text } else { look.soft }, CROSS);
            if x.clicked() {
                *search = None;
            }
            right = field.left();
        }
        let room = right - left - 16.0;
        let p = ui.painter();
        let size = if narrow { 16.0 } else { 18.0 };
        let name = bold(p, Pos2::new(left, top), row.spec.label(), size, look.text);
        let mut x = name.right() + if narrow { 8.0 } else { 10.0 };
        let tag = program(&row.spec, self.agent_sessions.contains_key(&row.id)).filter(|_| !narrow);
        let tag_w = tag.map_or(0.0, |tag| p.layout_no_wrap(tag.into(), FontId::proportional(11.0), look.soft).size().x + 22.0);
        if let Some(title) = transcript.and_then(|t| t.title.clone()) {
            let title_size = if narrow { 12.5 } else { 13.5 };
            let g = clipped(p, title, FontId::proportional(title_size), look.muted, (room - (x - left) - tag_w).max(0.0));
            // on the name's baseline
            let at = Pos2::new(x, name.bottom() - g.size().y - (size - title_size) * 0.12);
            x += g.size().x + 10.0;
            p.galley(at, g, look.muted);
        }
        if let Some(tag) = tag {
            let g = p.layout_no_wrap(tag.into(), FontId::proportional(11.0), look.soft);
            let pill = Rect::from_min_size(Pos2::new(x, name.center().y - 8.5), Vec2::new(g.size().x + 12.0, 17.0));
            p.rect_stroke(pill, 5, Stroke::new(1.0, look.line), StrokeKind::Inside);
            p.galley(pill.center() - g.size() / 2.0, g, look.soft);
        }
        // what it is doing and since when
        let line = match row.state {
            AgentState::Working(since) => format!("Trabalhando · há {}", elapsed(self.now - since)),
            AgentState::Waiting => match self.waiting_secs(&row.id) {
                Some(s) => format!("Precisa de você · esperando no terminal há {}", elapsed(s)),
                None => "Precisa de você · esperando no terminal".into(),
            },
            _ => self.state_note(row).map_or(row.state.label().to_string(), |n| format!("{} · {n}", row.state.label())),
        };
        let color = match row.state {
            AgentState::Working(_) | AgentState::Active(_) => GREEN,
            AgentState::Waiting => AMBER,
            _ => look.muted,
        };
        let state_y = name.bottom() + if narrow { 2.0 } else { 3.0 } + 8.5;
        state_mark(p, Pos2::new(left + 3.5, state_y), 7.0, &row.state, look, t);
        let g = clipped(p, line.clone(), FontId::proportional(if narrow { 12.0 } else { 12.5 }), color, room - 15.0);
        let text = Rect::from_min_size(Pos2::new(left + 15.0, state_y - g.size().y / 2.0), g.size());
        p.galley(text.min, g, color);
        if matches!(row.state, AgentState::Active(_)) {
            ui.interact(text, ui.scope_id().with("chat_state"), Sense::hover())
                .on_hover_text(format!("{line}\n\nPela saída do terminal: este programa não diz se está trabalhando"));
        }
        ui.painter().hline(main.x_range(), top + height - 0.5, Stroke::new(1.0, look.panel));
        top + height - main.top()
    }
}

/// One line of text cut with "…" to fit `width`.
pub(crate) fn clipped(painter: &Painter, text: String, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    painter.layout_job(job)
}

/// "Anexar": the design's paper clip, its arcs in short segments.
pub(crate) fn clip_icon(p: &Painter, at: Pos2, color: Color32) {
    let arc =
        |cx: f32, cy: f32, r: f32, from: f32| (0..=8).map(move |k| (from + 22.5 * k as f32).to_radians()).map(move |a| (cx + r * a.cos(), cy + r * a.sin()));
    let mut points = vec![(20.5, 11.5)];
    points.extend(arc(8.75, 16.15, 5.0, 45.0));
    points.extend(arc(16.2, 6.4, 3.4, 225.0));
    points.extend(arc(8.8, 16.2, 1.7, 45.0));
    points.push((15.5, 7.1));
    glyph(p, at, 17.0, 2.0, color, &[&points]);
}

/// A block of code under "linguagem · arquivo" and Copiar. The fence's line names them: "php",
/// "php lib/login.php", "rust:src/main.rs".
pub(crate) fn code_card(ui: &mut egui::Ui, info: &str, code: &str, c: &UiColors, dark: bool) {
    let (lang, file) = info.split_once([' ', ':']).map_or((info, ""), |(lang, file)| (lang, file.trim().trim_start_matches("title=").trim_matches('"')));
    let title = if file.is_empty() { lang.to_string() } else { format!("{lang} · {file}") };
    let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    let (fill, edge, quiet, ink) =
        if dark { (hex(0x181818), hex(0x2a2a2e), hex(0x8a8a94), hex(0xd4d4d8)) } else { (c.surface_low, tint(false, 20), c.muted, c.text) };
    egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, edge)).corner_radius(10).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let (head, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
        let p = ui.painter();
        p.text(Pos2::new(head.left() + 12.0, head.center().y), egui::Align2::LEFT_CENTER, title, FontId::monospace(12.0), quiet);
        p.hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0, edge));
        let label = p.layout_no_wrap("Copiar".into(), FontId::proportional(12.0), quiet);
        let button = Rect::from_min_max(
            Pos2::new(head.right() - 6.0 - (8.0 + 13.0 + 5.0 + label.size().x + 8.0), head.center().y - 12.0),
            Pos2::new(head.right() - 6.0, head.center().y + 12.0),
        );
        let response = ui
            .interact(button, ui.scope_id().with(("copy", code.len(), code.get(..code.len().min(32)))), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let p = ui.painter();
        if response.hovered() {
            p.rect_filled(button, 6, tint(dark, 18));
        }
        let copy: &[&[(f32, f32)]] = &[
            &[(8.5, 10.5), (8.5, 8.5), (20.5, 8.5), (20.5, 20.5), (8.5, 20.5), (8.5, 10.5)],
            &[(15.5, 5.5), (15.5, 5.0), (14.0, 3.5), (5.0, 3.5), (3.5, 5.0), (3.5, 14.0), (5.0, 15.5), (5.5, 15.5)],
        ];
        glyph(p, Pos2::new(button.left() + 14.5, button.center().y), 13.0, 2.0, quiet, copy);
        p.galley(Pos2::new(button.left() + 26.0, button.center().y - label.size().y / 2.0), label, quiet);
        if response.clicked() {
            ui.ctx().copy_text(code.into());
        }
        egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.label(egui::RichText::new(code).monospace().size(12.5).color(ink));
        });
    });
}

/// Copiar and Citar on a right click anywhere on a message, its whole text.
// ponytail: egui keeps a label selection's text private, so a selection is copied with ⌘C, not
// here; no Excluir mensagem, the history is Claude Code's file. Upgrade: egui exposing the selection
pub(crate) fn message_menu(response: &egui::Response, text: &str, cite: &mut Option<String>) {
    response.container_context_menu(|ui| {
        if ui.button("Copiar").clicked() {
            ui.ctx().copy_text(text.into());
        }
        if ui.button("Citar").clicked() {
            *cite = Some(text.into());
        }
    });
}

/// Citar: `text` at the end of the box, each line after "> ", and a blank line to write under it.
pub(crate) fn quote_into(draft: &mut String, text: &str) {
    if !draft.is_empty() && !draft.ends_with('\n') {
        draft.push('\n');
    }
    for line in text.trim_end().lines() {
        draft.push_str(&format!("> {line}\n"));
    }
    draft.push('\n');
}

/// The id of ⌘F's field.
pub(crate) const CHAT_SEARCH: &str = "chat_search";

/// Which of the person's and the agent's messages say `query` (already lowercase), in order.
pub(crate) fn found(items: &[ChatItem], query: &str) -> Vec<usize> {
    let says = |item: &ChatItem| matches!(item, ChatItem::User(t) | ChatItem::Agent(t) if t.to_lowercase().contains(query));
    items.iter().enumerate().filter(|(_, item)| says(item)).map(|(i, _)| i).collect()
}
