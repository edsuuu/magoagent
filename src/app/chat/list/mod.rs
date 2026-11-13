//! The chat's frame: its colors, the conversations down the left (grouped by date, with a search;
//! the agent's mark, title, folder and state) or, narrow or folded, a column of their initials. Also
//! the code card with its header, a message's Copiar / Citar menu and ⌘F's search.

use crate::app::chat::tint;
use crate::app::ui::agents::{AgentRow, AgentState, ago};
use crate::app::ui::menu_look::{click_menu, context_menu, menu_row};
use crate::prelude::*;
use crate::transcript::ChatItem;
mod view;

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
    /// Mostrar no canvas on a row (chat/node.rs).
    pub(crate) on_canvas: Option<String>,
}

/// The list's search, and when each chat last changed (`chat_when`) with when that was looked up.
#[derive(Default)]
pub(crate) struct ListState {
    pub(crate) filter: String,
    dates: std::cell::RefCell<std::collections::HashMap<String, (f64, Option<i64>)>>,
    /// The Maestros whose agents are folded away under their accordion (list/view.rs).
    pub(crate) folded: std::cell::RefCell<std::collections::HashSet<String>>,
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

/// The Maestro's subchats: for each row that is not a Maestro (`maestro[i]` false) but is roped to
/// one (`links`, by id), the index of that Maestro. A Maestro's agents are listed under it.
pub(crate) fn subchat_parents(ids: &[&str], maestro: &[bool], links: &std::collections::HashMap<String, Vec<String>>) -> Vec<Option<usize>> {
    (0..ids.len())
        .map(|i| {
            if maestro[i] {
                return None;
            }
            let roped = links.get(ids[i])?;
            (0..ids.len()).find(|&j| maestro[j] && roped.iter().any(|r| r == ids[j]))
        })
        .collect()
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
    fn a_maestros_agents_are_its_subchats() {
        let links = std::collections::HashMap::from([("r1".to_string(), vec!["m".to_string()]), ("t".to_string(), vec!["x".to_string()])]);
        let parents = super::subchat_parents(&["m", "r1", "t", "x"], &[true, false, false, false], &links);
        // r1 is roped to the Maestro m; t is roped only to another agent: both stay apart otherwise
        assert_eq!(parents, [None, Some(0), None, None]);
    }

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
