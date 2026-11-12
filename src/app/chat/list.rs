//! The chat's frame: its colors, the conversations down the left (grouped by date, with a search;
//! the agent's mark, title, folder and state) or, narrow or folded, a column of their initials. Also
//! the code card with its header, a message's Copiar / Citar menu and ⌘F's search.

use crate::app::chat::tint;
use crate::app::ui::agents::{AgentRow, AgentState, ago};
use crate::app::ui::menu_look::{click_menu, context_menu, menu_row};
use crate::prelude::*;
use crate::transcript::ChatItem;

/// The whole list's width and the folded column's.
pub(crate) const LIST: f32 = 248.0;
pub(crate) const COLUMN: f32 = 52.0;
/// Narrower than this the list is a column of initials and opens over the conversation.
pub(crate) const NARROW: f32 = 760.0;
/// Where the window buttons end, which macOS puts over the list (`place_window_buttons`); 0 elsewhere.
const LIGHTS_BOTTOM: f32 = if cfg!(target_os = "macos") { LIGHTS_Y + 8.0 } else { 0.0 };

pub(crate) const GREEN: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x50);
pub(crate) const AMBER: Color32 = Color32::from_rgb(0xf0, 0xa7, 0x3a);
pub(crate) const RED: Color32 = Color32::from_rgb(0xf8, 0x51, 0x49);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x0a, 0x84, 0xff);

/// The chat's colors, neutral greys (no purple cast) on both looks: the label ramp of the design is
/// white at 85% (text), 55% (secondary) and 25% (tertiary) over the ground, black at 85/50/26 on light.
pub(crate) mod ink {
    pub(crate) const DARK_BG: u32 = 0x121212;
    pub(crate) const DARK_PANEL: u32 = 0x1c1c1e;
    pub(crate) const DARK_SURFACE: u32 = 0x2c2c2e;
    pub(crate) const DARK_RAISED: u32 = 0x3a3a3c;
    pub(crate) const DARK_BAND: u32 = 0x1a1a1a;
    pub(crate) const DARK_SELECTED: u32 = 0x3a3a3c;
    pub(crate) const DARK_LINE: u32 = 0x2c2c2e;
    /// Label 85%, secondary 55% and tertiary on 0x121212.
    pub(crate) const DARK_TEXT: u32 = 0xdbdbdb;
    pub(crate) const DARK_SOFT: u32 = 0xa8a8a8;
    pub(crate) const DARK_MUTED: u32 = 0x8e8e8e;
    /// The hairline around pictures and the boxes of the chat.
    pub(crate) const DARK_EDGE: u32 = 0x48484a;
    /// A row under the pointer, the header's hairline; the names; the hints under the box.
    pub(crate) const DARK_HOVER: u32 = 0x232325;
    pub(crate) const DARK_STRONG: u32 = 0xf2f2f2;
    pub(crate) const DARK_FAINT: u32 = 0x6e6e73;
    pub(crate) const LIGHT_PANEL: u32 = 0xf5f5f5;
    pub(crate) const LIGHT_SURFACE: u32 = 0xf0f0f0;
    pub(crate) const LIGHT_RAISED: u32 = 0xe4e4e4;
    pub(crate) const LIGHT_TEXT: u32 = 0x262626;
    pub(crate) const LIGHT_SOFT: u32 = 0x5c5c5c;
    pub(crate) const LIGHT_MUTED: u32 = 0x808080;
    pub(crate) const LIGHT_HOVER: u32 = 0xebebeb;
    pub(crate) const LIGHT_STRONG: u32 = 0x111111;
    pub(crate) const LIGHT_FAINT: u32 = 0x9a9a9e;
}

/// The chat's colors (`ink`).
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
    pub(crate) hover: Color32,
    pub(crate) strong: Color32,
    pub(crate) faint: Color32,
}

impl Look {
    pub(crate) fn new(dark: bool, _c: &UiColors) -> Look {
        let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        if dark {
            Look {
                dark,
                bg: hex(ink::DARK_BG),
                panel: hex(ink::DARK_PANEL),
                surface: hex(ink::DARK_SURFACE),
                raised: hex(ink::DARK_RAISED),
                band: hex(ink::DARK_BAND),
                selected: hex(ink::DARK_SELECTED),
                line: hex(ink::DARK_LINE),
                text: hex(ink::DARK_TEXT),
                soft: hex(ink::DARK_SOFT),
                muted: hex(ink::DARK_MUTED),
                hover: hex(ink::DARK_HOVER),
                strong: hex(ink::DARK_STRONG),
                faint: hex(ink::DARK_FAINT),
            }
        } else {
            let line = Color32::from_black_alpha(24);
            Look {
                dark,
                bg: Color32::WHITE,
                panel: hex(ink::LIGHT_PANEL),
                surface: hex(ink::LIGHT_SURFACE),
                raised: hex(ink::LIGHT_RAISED),
                band: hex(ink::LIGHT_PANEL),
                selected: hex(ink::LIGHT_RAISED),
                line,
                text: hex(ink::LIGHT_TEXT),
                soft: hex(ink::LIGHT_SOFT),
                muted: hex(ink::LIGHT_MUTED),
                hover: hex(ink::LIGHT_HOVER),
                strong: hex(ink::LIGHT_STRONG),
                faint: hex(ink::LIGHT_FAINT),
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

/// "Trabalhando · há 1m 12s": 40 s, 1m 12s, 2 h 5 min.
pub(crate) fn elapsed(seconds: f64) -> String {
    match seconds.max(0.0) as u64 {
        s @ 0..=59 => format!("{s} s"),
        s @ 60..=3599 => format!("{}m {}s", s / 60, s % 60),
        s => format!("{} h {} min", s / 3600, s % 3600 / 60),
    }
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
    /// O botão do painel Contexto.
    pub(crate) context: bool,
    pub(crate) canvas: bool,
    /// Novo chat, and Remover chat on a row.
    pub(crate) new_chat: bool,
    pub(crate) remove: Option<String>,
    /// Abrir em miniatura: the chat leaves for its panel over the canvas.
    pub(crate) mini: bool,
    /// Mostrar no canvas on a row (chat_node.rs).
    pub(crate) on_canvas: Option<String>,
}

/// The list's search, and when each chat last changed (`chat_when`) with when that was looked up.
#[derive(Default)]
pub(crate) struct ListState {
    pub(crate) filter: String,
    dates: std::cell::RefCell<std::collections::HashMap<String, (f64, Option<i64>)>>,
}

/// The group of a chat that last changed at `when`, `now` being now (Unix seconds) and `offset`
/// the local zone's seconds from UTC: by the local calendar day.
pub(crate) fn date_group(when: i64, now: i64, offset: i64) -> &'static str {
    let day = |s: i64| (s + offset).div_euclid(86_400);
    match day(now) - day(when) {
        ..=0 => "Hoje",
        1 => "Ontem",
        2..=7 => "Últimos 7 dias",
        _ => "Mais antigas",
    }
}

/// The list's groups over items (when each changed, whether it is a chat), each with its items'
/// indexes, newest first: by date, or all under Conversas when no chat has one (a chat without one
/// is new: Hoje); terminals under Terminais, at the end.
pub(crate) fn date_groups(items: &[(Option<i64>, bool)], now: i64, offset: i64) -> Vec<(&'static str, Vec<usize>)> {
    let dated = items.iter().any(|(when, chat)| *chat && when.is_some());
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(items[i].0.unwrap_or(i64::MAX)));
    let mut groups: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for i in order {
        let name = match items[i] {
            (_, false) => "Terminais",
            _ if !dated => "Conversas",
            (None, _) => "Hoje",
            (Some(when), _) => date_group(when, now, offset),
        };
        match groups.iter_mut().find(|(n, _)| *n == name) {
            Some((_, members)) => members.push(i),
            None => groups.push((name, vec![i])),
        }
    }
    groups.sort_by_key(|(name, _)| *name == "Terminais");
    groups
}

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
            AgentState::Ended => self.agent_events.iter().rev().find(|e| e.id == row.id).map(|e| ago(e.age())),
            _ => None,
        };
        let label = if matches!(row.state, AgentState::Idle(_)) { "Pronto" } else { row.state.label() };
        match more {
            Some(more) => format!("{label} · {more}"),
            None => label.to_string(),
        }
    }

    /// When chat `id` last changed: its session file's time (Claude Code's, or what the app keeps of a
    /// Codex chat), in Unix seconds; None before its first message. Looked up again every 5 s.
    fn chat_when(&self, id: &str) -> Option<i64> {
        let mut dates = self.chat_listing.dates.borrow_mut();
        if let Some((_, when)) = dates.get(id).filter(|(at, _)| self.now - at < 5.0) {
            return *when;
        }
        let chat = self.chats.iter().find(|c| c.id == id)?;
        let file = match (&chat.session, chat.agent.as_str()) {
            (None, _) => None,
            (Some(_), "codex") => chat_items_path(id),
            (Some(session), _) => DrawingApp::claude_transcript(&chat.cwd, session),
        };
        let when = file.and_then(|f| fs::metadata(f).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64);
        dates.insert(id.to_string(), (self.now, when));
        when
    }

    /// The chats under Conversas, the newest first in groups by date, with the search field over them
    /// (`filter`); a terminal whose conversation is open goes under Terminais. `tool` is what the
    /// chosen one runs now.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn chat_list(
        &self,
        ui: &mut egui::Ui,
        side: Rect,
        rows: &[AgentRow],
        look: &Look,
        tool: Option<&str>,
        t: f64,
        act: &mut ListAction,
        filter: &mut String,
    ) {
        ui.painter().rect_filled(side, 12, look.panel);
        // the title under the window buttons, and on macOS + and « beside them, as in the reference app
        let inner = Rect::from_min_max(Pos2::new(side.left() + 8.0, (side.top() + 10.0).max(LIGHTS_BOTTOM + 10.0)), side.max - Vec2::new(8.0, 10.0));
        let head = inner.top() + 14.0;
        let tools = if cfg!(target_os = "macos") { LIGHTS_Y } else { head };
        bold(ui.painter(), Pos2::new(inner.left() + 8.0, head - 9.0), "Conversas", 14.0, look.strong);
        let fold = Rect::from_center_size(Pos2::new(inner.right() - 14.0, tools), Vec2::splat(28.0));
        let response =
            ui.interact(fold, ui.scope_id().with("chat_fold"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(keys_label("Recolher lista (⌘B)"));
        if response.hovered() {
            ui.painter().rect_filled(fold, 7, look.hover);
        }
        glyph(ui.painter(), fold.center(), 16.0, 2.0, look.soft, FOLD);
        act.toggle |= response.clicked();
        // Nova conversa: a chat with an agent installed here, in a folder
        let add = fold.translate(Vec2::new(-32.0, 0.0));
        let response = ui.interact(add, ui.scope_id().with("chat_new"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Nova conversa");
        if response.hovered() {
            ui.painter().rect_filled(add, 7, look.hover);
        }
        glyph(ui.painter(), add.center(), 16.0, 2.0, look.soft, &[&[(12.0, 5.0), (12.0, 19.0)], &[(5.0, 12.0), (19.0, 12.0)]]);
        act.new_chat |= response.clicked();
        // the search over the titles and names
        let field = Rect::from_min_max(Pos2::new(inner.left() + 2.0, head + 20.0), Pos2::new(inner.right() - 2.0, head + 50.0));
        ui.painter().rect_filled(field, 8, look.hover);
        paint_icon(ui.painter(), Icon::Search, Pos2::new(field.left() + 15.0, field.center().y), look.muted);
        let edit = egui::TextEdit::singleline(filter)
            .id(egui::Id::unique("chat_list_filter"))
            .hint_text(egui::RichText::new("Buscar conversas").color(look.muted))
            .font(FontId::proportional(13.0))
            .text_color(look.text)
            .margin(egui::Margin::ZERO)
            .frame(egui::Frame::NONE);
        // the frameless field keeps no margin: the room for the magnifier is left out of its rect
        ui.put(Rect::from_min_max(field.min + Vec2::new(30.0, 6.0), field.max - Vec2::new(8.0, 6.0)), edit);
        let words = filter.trim().to_lowercase();
        let title = |row: &AgentRow| self.transcripts.get(&row.id).and_then(|t| t.title.clone()).unwrap_or_else(|| row.spec.label().to_string());
        let says = |row: &AgentRow| words.is_empty() || title(row).to_lowercase().contains(&words) || row.spec.label().to_lowercase().contains(&words);
        let shown: Vec<&AgentRow> = rows.iter().filter(|r| says(r)).collect();
        let now = now_ms();
        let dates: Vec<(Option<i64>, bool)> = shown.iter().map(|r| (self.chat_when(&r.id), self.is_chat(&r.id))).collect();
        let groups = date_groups(&dates, now / 1000, crate::app::chat::viewer::local_offset(now));
        let list = Rect::from_min_max(Pos2::new(inner.left(), field.bottom() + 4.0), inner.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical().id_salt("chat_agents").auto_shrink(false).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (group, members) in groups {
                    let (label, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
                    let mut job = egui::text::LayoutJob::default();
                    let format = egui::TextFormat { font_id: FontId::proportional(11.0), color: look.faint, extra_letter_spacing: 0.6, ..Default::default() };
                    job.append(&group.to_uppercase(), 0.0, format);
                    let g = ui.painter().layout_job(job);
                    ui.painter().galley(Pos2::new(label.left() + 10.0, label.bottom() - g.size().y - 4.0), g, look.faint);
                    for i in members {
                        self.list_row(ui, shown[i], &title(shown[i]), look, tool, t, act);
                    }
                }
            });
        });
    }

    /// A conversation of the list: the agent's mark, the title, the folder and state under it after
    /// the state's dot, and ⋯ (Remover chat) under the pointer.
    #[allow(clippy::too_many_arguments)]
    fn list_row(&self, ui: &mut egui::Ui, row: &AgentRow, title: &str, look: &Look, tool: Option<&str>, t: f64, act: &mut ListAction) {
        let on = self.chat_agent.as_ref() == Some(&row.id);
        let chat = self.is_chat(&row.id);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
        let remove = |ui: &mut egui::Ui, act: &mut ListAction| {
            if menu_row(ui, Some(Icon::Map), "Mostrar no canvas", "", None) {
                act.on_canvas = Some(row.id.clone());
                ui.close();
            }
            if chat && menu_row(ui, Some(Icon::Trash), "Remover chat", "", Some(crate::app::ui::UiColors::new(ui.visuals().dark_mode).danger)) {
                act.remove = Some(row.id.clone());
                ui.close();
            }
        };
        context_menu(&response, |ui| remove(ui, act));
        let dots = Rect::from_center_size(Pos2::new(rect.right() - 18.0, rect.center().y), Vec2::splat(24.0));
        let dots_r = ui.interact(dots, ui.scope_id().with(("chat_row_more", &row.id)), Sense::click());
        let menu_open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&dots_r));
        let hovered = response.hovered() || dots_r.hovered() || menu_open;
        let p = ui.painter();
        if on || hovered {
            p.rect_filled(rect, 10, if on { look.surface } else { look.hover });
        }
        let claude = crate::app::chat::header::program(&row.spec, self.agent_sessions.contains_key(&row.id)) == Some("Claude Code");
        let working = matches!(row.state, AgentState::Working(_));
        crate::app::chat::header::agent_mark(p, &row.spec, claude, Pos2::new(rect.left() + 20.0, rect.center().y), 14.0, look.soft, working, t);
        let room = rect.width() - 40.0 - if chat && hovered { 34.0 } else { 10.0 };
        let ink = if row.state == AgentState::Ended { look.soft } else { look.text };
        p.galley(Pos2::new(rect.left() + 40.0, rect.top() + 8.0), clipped(p, title.to_string(), FontId::proportional(13.5), ink, room), ink);
        let note = self.list_note(row, if on { tool } else { None });
        let line = format!("{} · {note}", crate::app::chat::direct::folder_label(&row.spec.cwd));
        let y = rect.top() + 35.0;
        p.circle_filled(Pos2::new(rect.left() + 43.0, y), 3.0, crate::app::chat::header::state_color(&row.state, look));
        let g = clipped(p, line, FontId::proportional(11.5), look.muted, room - 12.0);
        p.galley(Pos2::new(rect.left() + 51.0, y - g.size().y / 2.0), g, look.muted);
        if chat && hovered {
            if dots_r.hovered() {
                p.rect_filled(dots, 6, look.raised);
            }
            for dx in [-5.0, 0.0, 5.0] {
                p.circle_filled(dots.center() + Vec2::new(dx, 0.0), 1.6, look.soft);
            }
        }
        if chat {
            click_menu(&dots_r, |ui| remove(ui, act));
        }
        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            act.pick = Some(row.id.clone());
        }
    }

    /// Folded or narrow: a column of initials, each with its state's badge, under the » button.
    pub(crate) fn chat_column(&self, ui: &mut egui::Ui, side: Rect, rows: &[AgentRow], look: &Look, t: f64, act: &mut ListAction) {
        // folded, the column starts under the window buttons, which stay on the bare ground
        let side = side.with_min_y(side.top().max(LIGHTS_BOTTOM + 4.0));
        ui.painter().rect_filled(side, 12, look.panel);
        let unfold = Rect::from_center_size(Pos2::new(side.center().x, side.top() + 24.0), Vec2::new(36.0, 32.0));
        let response = ui
            .interact(unfold, ui.scope_id().with("chat_unfold"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .tip(keys_label("Expandir lista (⌘B)"));
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
                    let response =
                        ui.interact(rect, ui.scope_id().with(("chat_initial", &row.id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(tip);
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
}

/// One line of text cut with "…" to fit `width`.
pub(crate) fn clipped(painter: &Painter, text: String, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    painter.layout_job(job)
}

/// A block of code under "linguagem · arquivo" and Copiar. The fence's line names them: "php",
/// "php lib/login.php", "rust:src/main.rs".
pub(crate) fn code_card(ui: &mut egui::Ui, info: &str, code: &str, c: &UiColors, dark: bool) {
    code_box(ui, info, code, code, c, dark);
}

/// `code_card` showing `shown` while Copiar takes `code` (a long output shows its end only).
pub(crate) fn code_box(ui: &mut egui::Ui, info: &str, shown: &str, code: &str, c: &UiColors, dark: bool) {
    let (lang, file) = info.split_once([' ', ':']).map_or((info, ""), |(lang, file)| (lang, file.trim().trim_start_matches("title=").trim_matches('"')));
    let title = if file.is_empty() { lang.to_string() } else { format!("{lang} · {file}") };
    let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    let (fill, edge, quiet, ink) = if dark {
        (hex(ink::DARK_BAND), hex(ink::DARK_LINE), hex(ink::DARK_MUTED), hex(ink::DARK_TEXT))
    } else {
        (c.surface_low, tint(false, 20), c.muted, c.text)
    };
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
            ui.label(egui::RichText::new(shown).monospace().size(12.5).color(ink));
        });
    });
}

/// Copiar and Citar on a right click anywhere on a message, its whole text.
// ponytail: egui keeps a label selection's text private, so a selection is copied with ⌘C, not
// here; no Excluir mensagem, the history is Claude Code's file. Upgrade: egui exposing the selection
pub(crate) fn message_menu(response: &egui::Response, text: &str, cite: &mut Option<String>) {
    // egui's container menu (a right click where no widget took it), in the app's menu box
    let open = if response.container_secondary_clicked() {
        Some(egui::SetOpenCommand::Bool(true))
    } else if response.container_clicked() {
        Some(egui::SetOpenCommand::Bool(false))
    } else {
        None
    };
    let dark = response.ctx.global_style().visuals.dark_mode;
    egui::Popup::menu(response).open_memory(open).at_pointer_fixed().frame(crate::app::ui::menu_look::menu_frame(dark)).show(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.set_min_width(crate::app::ui::menu_look::MENU_MIN);
        if menu_row(ui, Some(Icon::Duplicate), "Copiar", "", None) {
            ui.ctx().copy_text(text.into());
            ui.close();
        }
        if menu_row(ui, Some(Icon::Edit), "Citar", "", None) {
            *cite = Some(text.into());
            ui.close();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chats_group_by_their_local_day_and_the_terminals_go_last() {
        // 2026-11-11 12:00 UTC, in a zone three hours behind (09:00 there)
        let (now, zone, hour) = (1_794_398_400, -3 * 3600, 3600);
        assert_eq!(date_group(now - hour, now, zone), "Hoje");
        // 20:00 local the evening before
        assert_eq!(date_group(now - 13 * hour, now, zone), "Ontem");
        assert_eq!(date_group(now - 3 * 24 * hour, now, zone), "Últimos 7 dias");
        assert_eq!(date_group(now - 30 * 24 * hour, now, zone), "Mais antigas");
        let items = [(Some(now - 30 * 24 * hour), true), (None, false), (Some(now - hour), true), (None, true)];
        assert_eq!(date_groups(&items, now, zone), [("Hoje", vec![3, 2]), ("Mais antigas", vec![0]), ("Terminais", vec![1])]);
        // no dates at all: one group, in the list's order
        assert_eq!(date_groups(&[(None, true), (None, true)], now, zone), [("Conversas", vec![0, 1])]);
    }
}
