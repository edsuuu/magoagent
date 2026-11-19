//! Início: the summary of every tab. At the left a card with the counts and the Luna under them; at the right one list of
//! what is on and what changed last, what needs the person first. A line opens its tab; its × hides it until it changes
//! again, and "Mostrar ocultos" brings them all back.

use super::music::GREEN;
use super::paint::{AMBER, CARD, DIM, DIM_3, HOVER, INK, Out, View, aim, font, semi, shoulder_mark};
use super::{Line, Tab, main_state};
use crate::app::chat::list::clipped;
use crate::app::maestro::outside::{Badge, PILL_PREFIX};
use crate::app::mascot::mago::MagoState;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// The summary card's width, and a line of the list.
const SUMMARY_W: f32 = 236.0;
const ROW_H: f32 = 38.0;
const BLUE: Color32 = Color32::from_rgb(0x3b, 0x9e, 0xff);
const RED: Color32 = Color32::from_rgb(0xf8, 0x51, 0x49);

/// What a line is at, as one value: it shows again once this changes after it was hidden.
pub(super) fn signature(line: &Line) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    (&line.detail, line.state.label(), format!("{:?}", line.badge), line.asleep).hash(&mut hasher);
    hasher.finish()
}

/// Who needs the person first, then what failed, what works, what just ended, the rest; the latest first in each.
fn rank(line: &Line) -> u8 {
    match (line.badge, line.state, line.mago) {
        (Some(Badge::Approval), ..) | (_, AgentState::Waiting, _) => 0,
        (Some(Badge::Error), ..) | (_, _, MagoState::Error) => 1,
        (_, AgentState::Working(_), _) => 2,
        (Some(Badge::Finished), ..) | (_, _, MagoState::Done) => 3,
        _ => 4,
    }
}

/// Whether a click on `line` opens its conversation right away: a chat of the app and a session outside have one (the
/// island's, or the app's own chat screen); a terminal opens its card on the canvas instead, and an integration its tab.
fn opens(line: &Line) -> bool {
    line.chat || line.id.starts_with(PILL_PREFIX)
}

/// The lines Início lists: the sessions outside and the integrations always, the app's own agents only while something
/// goes on with them (an idle chat is Chats' business).
pub(super) fn feed_lines<'a>(lines: &'a [Line], hidden: &HashMap<String, u64>) -> (Vec<&'a Line>, usize) {
    let mut shown: Vec<&Line> =
        lines.iter().filter(|line| !matches!(Tab::of(&line.id), Tab::Chats) || rank(line) < 4).filter(|line| line.state != AgentState::Ended).collect();
    let before = shown.len();
    shown.retain(|line| hidden.get(&line.id) != Some(&signature(line)));
    let hidden_count = before - shown.len();
    shown.sort_by(|a, b| rank(a).cmp(&rank(b)).then(b.at.cmp(&a.at)));
    (shown, hidden_count)
}

/// How long ago `at` (ms) was, at `now` (ms): "agora", "há 4 min", "há 2 h", "ontem", "há 3 dias"; unknown: nothing.
pub(super) fn ago(at: i64, now: i64) -> String {
    if at <= 0 {
        return String::new();
    }
    let minutes = (now - at).max(0) / 60_000;
    match minutes {
        0 => "agora".into(),
        1..=59 => format!("há {minutes} min"),
        60..=1439 => format!("há {} h", minutes / 60),
        1440..=2879 => "ontem".into(),
        _ => format!("há {} dias", minutes / 1440),
    }
}

/// The summary's lines: (colour, the count in bold or none, the words).
fn summary(lines: &[Line], failing_prs: usize, playing: bool) -> Vec<(Color32, Option<usize>, String)> {
    let needs = lines.iter().filter(|line| rank(line) == 0).count();
    let working = lines.iter().filter(|line| matches!(line.state, AgentState::Working(_))).count();
    let plural = |count: usize, one: &str, many: &str| if count == 1 { one.to_string() } else { many.to_string() };
    let mut rows = Vec::new();
    if needs > 0 {
        rows.push((AMBER, Some(needs), plural(needs, "precisa de você", "precisam de você")));
    }
    if working > 0 {
        rows.push((BLUE, Some(working), plural(working, "agente trabalhando", "agentes trabalhando")));
    }
    if failing_prs > 0 {
        rows.push((RED, Some(failing_prs), plural(failing_prs, "PR com CI falhando", "PRs com CI falhando")));
    }
    if playing {
        rows.push((GREEN, None, "tocando no Spotify".into()));
    }
    if rows.is_empty() {
        rows.push((DIM_3, None, "Tudo calmo por aqui".into()));
    }
    rows
}

impl DrawingApp {
    /// Início inside `page`: the summary card and the list.
    pub(super) fn island_feed(&self, ui: &mut egui::Ui, page: Rect, view: &View, time: f64, out: &mut Out) {
        let left = Rect::from_min_size(page.min, Vec2::new(SUMMARY_W, page.height()));
        let right = Rect::from_min_max(Pos2::new(left.right() + 10.0, page.top()), page.max);
        for card in [left, right] {
            ui.painter().rect(card, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        }
        // the summary, the Luna under it
        let failing = self
            .integrations
            .github
            .pulse
            .as_ref()
            .map_or(0, |pulse| pulse.mine.iter().filter(|pr| pr.ci == crate::app::integrations::github::CiState::Failure).count());
        let playing = self.integrations.spotify.now.as_ref().is_some_and(|now| now.playing);
        let (x, mut y) = (left.left() + 18.0, left.top() + 14.0);
        for shift in [0.0, 0.4] {
            ui.painter().text(Pos2::new(x + shift, y), egui::Align2::LEFT_TOP, "Resumo", font(13.0), INK);
        }
        y += 26.0;
        for (color, count, words) in summary(&view.lines, failing, playing) {
            ui.painter().circle_filled(Pos2::new(x + 3.5, y + 10.0), 3.5, color);
            let mut text_x = x + 14.0;
            if let Some(count) = count {
                let bold = ui.painter().text(Pos2::new(text_x, y + 10.0), egui::Align2::LEFT_CENTER, count.to_string(), semi(12.0), INK);
                text_x = bold.right() + 4.0;
            }
            ui.painter().text(Pos2::new(text_x, y + 10.0), egui::Align2::LEFT_CENTER, words, font(12.0), DIM);
            y += 20.0;
        }
        let state = main_state(view.mood, view.celebrating);
        let room = (left.bottom() - 12.0 - y).max(30.0);
        let size = room.min(64.0);
        aim(view, Pos2::new(left.center().x, left.bottom() - 10.0 - size / 2.0), size, state);
        self.feed_list(ui, right, view, time, out);
    }

    /// The list at the right: a line per thing, the × on the one under the pointer, the footer.
    fn feed_list(&self, ui: &mut egui::Ui, card: Rect, view: &View, time: f64, out: &mut Out) {
        let (rows, hidden) = feed_lines(&view.lines, &self.island.hidden);
        let inner = card.shrink2(Vec2::new(8.0, 7.0));
        let foot_h = 16.0;
        let list = Rect::from_min_max(inner.min, Pos2::new(inner.right(), inner.bottom() - foot_h));
        if rows.is_empty() {
            let words = if hidden > 0 { "Tudo o que havia está oculto" } else { "Nada acontecendo agora" };
            ui.painter().text(list.center(), egui::Align2::CENTER_CENTER, words, font(12.0), DIM_3);
        }
        let now = now_ms();
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 1.0);
            ui.spacing_mut().scroll = crate::app::ui::scroll_style();
            egui::ScrollArea::vertical().id_salt("island_feed").auto_shrink([false, false]).show(ui, |ui| {
                for line in &rows {
                    self.feed_row(ui, line, now, time, out);
                }
            });
        });
        // the footer: the hidden ones back at the left, all hidden at the right
        let foot_y = inner.bottom() - foot_h / 2.0;
        if hidden > 0 {
            let count = format!("{hidden} {} · ", if hidden == 1 { "oculto" } else { "ocultos" });
            let counted = ui.painter().text(Pos2::new(inner.left() + 6.0, foot_y), egui::Align2::LEFT_CENTER, count, font(10.5), DIM_3);
            let show = link(ui, Pos2::new(counted.right(), foot_y), "Mostrar ocultos", egui::Align2::LEFT_CENTER, "feed_show");
            out.show_hidden |= show.clicked();
        }
        if !rows.is_empty() {
            let all = link(ui, Pos2::new(inner.right() - 6.0, foot_y), "Ocultar tudo", egui::Align2::RIGHT_CENTER, "feed_hide_all");
            if all.clicked() {
                out.hide.extend(rows.iter().map(|line| (line.id.clone(), signature(line))));
            }
        }
    }

    /// One line: the mini (with its mark), the name over what it does, how long ago (the × in its place under the pointer).
    fn feed_row(&self, ui: &mut egui::Ui, line: &Line, now: i64, time: f64, out: &mut Out) {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
        let hide_at = Pos2::new(rect.right() - 6.0 - 10.0, rect.center().y);
        let hide_area = Rect::from_center_size(hide_at, Vec2::splat(20.0));
        let hide = ui.interact(hide_area, ui.scope_id().with(("feed_hide", &line.id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let hovered = response.hovered() || hide.hovered();
        if hovered {
            ui.painter().rect_filled(rect, 10, HOVER);
        }
        let mini = Pos2::new(rect.left() + 6.0 + 11.0, rect.center().y + 1.0);
        crate::app::mascot::mago::paint_mini(ui.painter(), &format!("island-feed-{}", line.id), mini, 20.0, line.color, line.mago, time);
        if line.id.starts_with(PILL_PREFIX) {
            shoulder_mark(ui.painter(), mini, 20.0, line, time);
        }
        // how long ago, or the × under the pointer
        let when = match (line.state, ago(line.at, now)) {
            (AgentState::Working(_), _) => "agora".to_string(),
            (_, when) if when.is_empty() && line.id == format!("{}spotify", crate::app::integrations::ITEM_PREFIX) => {
                let playing = self.integrations.spotify.now.as_ref().is_some_and(|now| now.playing);
                if playing { "tocando" } else { "pausado" }.into()
            }
            (_, when) => when,
        };
        let mut text_right = rect.right() - 6.0 - 20.0 - 10.0;
        if hovered {
            let ground = if hide.hovered() { Color32::from_rgb(0x3a, 0x3e, 0x45) } else { Color32::from_rgb(0x2b, 0x2e, 0x33) };
            ui.painter().circle_filled(hide_at, 10.0, ground);
            super::icons::paint(ui.painter(), super::icons::XMARK, hide_at, 12.0, INK);
        } else if !when.is_empty() {
            let galley = ui.painter().layout_no_wrap(when, font(10.5), DIM_3);
            text_right = rect.right() - 6.0 - galley.size().x - 10.0;
            ui.painter().galley(Pos2::new(text_right + 10.0, rect.center().y - galley.size().y / 2.0), galley, DIM_3);
        }
        let hide = hide.on_hover_text("Ocultar");
        let text_x = rect.left() + 6.0 + 22.0 + 10.0;
        let width = (text_right - text_x).max(10.0);
        let title = clipped(ui.painter(), line.name.clone(), semi(12.0), INK, width);
        ui.painter().galley(Pos2::new(text_x, rect.top() + 5.0), title, INK);
        let tone = match rank(line) {
            0 => AMBER,
            1 => RED,
            _ => DIM_3,
        };
        let detail = clipped(ui.painter(), line.detail.clone(), font(11.0), tone, width);
        ui.painter().galley(Pos2::new(text_x, rect.top() + 20.0), detail, tone);
        if hide.clicked() {
            out.hide.push((line.id.clone(), signature(line)));
        } else if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            // its tab, with it in the card; the integrations with no tab of their own do their first button
            match Tab::of(&line.id) {
                Tab::Home => self.feed_press(&line.id),
                tab => {
                    (out.focus, out.tab, out.picked) = (Some(line.id.clone()), Some(tab), true);
                    // a second click would open it as its Abrir does; here the first does too
                    if opens(line) {
                        out.open_agent = Some(line.id.clone());
                    }
                }
            }
        }
    }

    /// An integration with no tab of its own (WhatsApp): its card's first button, as if pressed there.
    fn feed_press(&self, id: &str) {
        let Some(item) = self.integration_items().into_iter().find(|item| item.id == id) else { return };
        if let Some((_, action)) = item.chips.first() {
            self.integrations.pressed.borrow_mut().push(action.clone());
        }
    }
}

/// A word that is a button, dim and lit under the pointer.
fn link(ui: &mut egui::Ui, at: Pos2, text: &str, align: egui::Align2, id: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), font(10.5), DIM);
    let rect = align.anchor_size(at, galley.size());
    let response = ui.interact(rect.expand(3.0), ui.scope_id().with(id), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let ink = if response.hovered() { INK } else { DIM };
    ui.painter().text(at, align, text, font(10.5), ink);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: &str, state: AgentState, at: i64) -> Line {
        Line {
            id: id.into(),
            name: id.into(),
            detail: "Edit · home.rs".into(),
            state,
            mago: MagoState::Idle,
            color: Color32::WHITE,
            chat: false,
            canvas: false,
            badge: None,
            kind: String::new(),
            at,
            asleep: false,
        }
    }

    #[test]
    fn the_summary_lists_who_needs_you_first_and_hides_until_something_changes() {
        let mut lines = vec![
            line("pill:code:/a", AgentState::Idle(None), 50),
            line("pill:code:/b", AgentState::Waiting, 10),
            line("pill:code:/c", AgentState::Working(0.0), 20),
            line("chat-idle", AgentState::Idle(None), 99),
            line("integration:github", AgentState::Idle(None), 0),
        ];
        let mut hidden = HashMap::new();
        let (shown, count) = feed_lines(&lines, &hidden);
        let ids: Vec<&str> = shown.iter().map(|line| line.id.as_str()).collect();
        assert_eq!(ids, ["pill:code:/b", "pill:code:/c", "pill:code:/a", "integration:github"], "an idle chat of the app is Chats'");
        assert_eq!(count, 0);
        hidden.insert("pill:code:/a".to_string(), signature(&lines[0]));
        let (shown, count) = feed_lines(&lines, &hidden);
        assert!(shown.iter().all(|line| line.id != "pill:code:/a") && count == 1, "hidden");
        lines[0].detail = "Bash · cargo test".into();
        let (shown, count) = feed_lines(&lines, &hidden);
        assert!(shown.iter().any(|line| line.id == "pill:code:/a") && count == 0, "it changed: back");
    }

    #[test]
    fn how_long_ago_in_words() {
        let now = 10_000_000_000;
        assert_eq!(ago(0, now), "");
        assert_eq!(ago(now - 20_000, now), "agora");
        assert_eq!(ago(now - 4 * 60_000, now), "há 4 min");
        assert_eq!(ago(now - 3 * 3_600_000, now), "há 3 h");
        assert_eq!(ago(now - 30 * 3_600_000, now), "ontem");
        assert_eq!(ago(now - 3 * 86_400_000, now), "há 3 dias");
    }

    #[test]
    fn a_chat_or_a_session_outside_opens_its_conversation_on_the_first_click() {
        let mut chat = line("chat-a", AgentState::Working(0.0), 30);
        chat.chat = true;
        let mut terminal = line("terminal-b", AgentState::Working(0.0), 30);
        terminal.canvas = true;
        let mut pill = line("pill:code:/w/loja", AgentState::Working(0.0), 30);
        pill.chat = false;
        let tool = line("integration:github", AgentState::Idle(None), 0);
        assert!(opens(&chat), "a chat: its conversation");
        assert!(opens(&pill), "a session outside: its conversation, followed from its terminal");
        assert!(!opens(&terminal), "a terminal: its card on the canvas");
        assert!(!opens(&tool), "an integration: its tab");
    }

    #[test]
    fn the_summary_counts_what_matters() {
        let lines = [line("a", AgentState::Waiting, 0), line("b", AgentState::Waiting, 0), line("c", AgentState::Working(0.0), 0)];
        let rows = summary(&lines, 1, true);
        let words: Vec<String> = rows.iter().map(|(_, count, words)| format!("{}{words}", count.map(|c| format!("{c} ")).unwrap_or_default())).collect();
        assert_eq!(words, ["2 precisam de você", "1 agente trabalhando", "1 PR com CI falhando", "tocando no Spotify"]);
        assert_eq!(summary(&[], 0, false)[0].2, "Tudo calmo por aqui");
    }
}
