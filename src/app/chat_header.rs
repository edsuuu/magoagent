//! The bar over the conversation: the agent's mark in a rounded square (Claude's asterisk, still or
//! turning while it works), its name and the chat's whole folder, its state and model; at the right
//! the plan's 5-hour window, the context ring that opens the Contexto panel, the search, the live
//! terminal and Abrir em miniatura.

use crate::app::chat_list::{AMBER, CHAT_SEARCH, CROSS, ListAction, Look, PROMPT, RED, clipped, elapsed, glyph};
use crate::app::chat_thinking::{CLAUDE, spinner};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;

/// Ready's green.
pub(crate) const READY: Color32 = Color32::from_rgb(0x34, 0xc7, 0x59);

/// The state's dot: green ready, Claude's orange working, amber when it needs you.
pub(crate) fn state_color(state: &AgentState, look: &Look) -> Color32 {
    match state {
        AgentState::Working(_) => CLAUDE,
        AgentState::Waiting => AMBER,
        AgentState::Ended => look.faint,
        AgentState::Idle(_) | AgentState::Active(_) => READY,
    }
}

/// The program a terminal runs, for the header's tag.
pub(crate) fn program(spec: &TerminalSpec, claude: bool) -> Option<&'static str> {
    match spec.command.split_whitespace().next() {
        _ if claude || spec.command.contains("claude") => Some("Claude Code"),
        Some("codex") => Some("Codex"),
        _ => None,
    }
}

/// The agent's mark `size` across: Claude's asterisk in its orange (whole and still unless
/// `working`), else the icon of its terminal.
pub(crate) fn agent_mark(p: &Painter, spec: &TerminalSpec, claude: bool, at: Pos2, size: f32, color: Color32, working: bool, t: f64) {
    if claude {
        // 0.65 s in: the spinner's whole star
        spinner(p, at, size, CLAUDE, if working { t } else { 0.65 });
    } else {
        terminal_icon(p, spec, at, color);
    }
}

/// `path` with the home folder as "~" ("" is home), cut in the middle with "…" to `room` characters
/// (the end, where the folder's own name is, keeps the larger part).
pub(crate) fn short_path(path: &str, home: &str, room: usize) -> String {
    let path = path.trim();
    let home = home.trim_end_matches(['/', '\\']);
    let mut out = if path.is_empty() { "~".to_string() } else { path.to_string() };
    if !home.is_empty()
        && let Some(rest) = out.get(home.len()..).filter(|_| out.get(..home.len()).is_some_and(|h| h.eq_ignore_ascii_case(home)))
        && (rest.is_empty() || rest.starts_with(['/', '\\']))
    {
        out = format!("~{rest}");
    }
    let chars: Vec<char> = out.chars().collect();
    if chars.len() <= room {
        return out;
    }
    if room < 2 {
        return "…".into();
    }
    let tail = (room - 1) * 3 / 5;
    let head = room - 1 - tail;
    chars[..head].iter().chain(std::iter::once(&'…')).chain(&chars[chars.len() - tail..]).collect()
}

/// A rounded button of the bar, `on` while what it opens shows; `border` draws its hairline.
fn bar_button(ui: &mut egui::Ui, rect: Rect, salt: &str, look: &Look, on: bool, border: bool) -> egui::Response {
    let response = ui.interact(rect, ui.scope_id().with(salt), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if on || response.hovered() {
        p.rect_filled(rect, 8, look.hover);
    }
    if border {
        p.rect_stroke(rect, 8, Stroke::new(1.0, look.line), StrokeKind::Inside);
    }
    response
}

/// The context ring: the used share of the window in blue over a grey track.
fn ring(p: &Painter, at: Pos2, share: f32, look: &Look) {
    let (r, width) = (5.6, 2.4);
    p.circle_stroke(at, r, Stroke::new(width, look.raised));
    let share = share.clamp(0.0, 1.0);
    if share > 0.0 {
        let n = (share * 40.0).ceil() as usize;
        let points = (0..=n).map(|k| at + Vec2::angled(-std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * share * k as f32 / n as f32) * r).collect();
        p.add(Shape::line(points, Stroke::new(width, crate::app::chat_list::ACCENT)));
    }
}

impl DrawingApp {
    /// The agent's name (a chat's before its " · folder"), its folder and model, as the bar shows them.
    fn header_facts(&self, row: &AgentRow) -> (String, Option<String>, Option<String>) {
        match self.chats.iter().find(|c| c.id == row.id) {
            Some(chat) => {
                let name = chat.name.split(" · ").next().unwrap_or(&chat.name).to_string();
                let model = crate::app::chat_footer::tuning_names(chat).0;
                (name, Some(chat.cwd.clone()), Some(model).filter(|m| !m.is_empty()))
            }
            None => {
                let model = self.usage.agents.get(&row.id).map(|u| u.model.clone()).filter(|m| !m.is_empty());
                (row.spec.label().to_string(), Some(row.spec.cwd.clone()).filter(|c| !c.trim().is_empty()), model)
            }
        }
    }

    /// The bar: the agent's mark, name and folder, state and model; the buttons at the right
    /// (`search` is ⌘F's field). Gives its height.
    #[allow(clippy::too_many_arguments)]
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
        let (left, top) = (main.left() + if narrow { 4.0 } else { 8.0 }, main.top() + if narrow { 2.0 } else { 0.0 });
        let height = if narrow { 50.0 } else { 56.0 };
        let middle = top + height / 2.0 - 1.0;
        let button = |right: f32, width: f32| Rect::from_min_max(Pos2::new(right - width, middle - 16.0), Pos2::new(right, middle + 16.0));
        let font = FontId::proportional(12.0);
        // the buttons first, from the right: the name and folder are cut before them
        let mut right = main.right();
        // Abrir em miniatura: the chat leaves for its panel over the canvas
        let rect = button(right, 32.0);
        let mini = bar_button(ui, rect, "chat_mini", look, false, false).tip("Abrir em miniatura");
        let ink = if mini.hovered() { look.text } else { look.soft };
        glyph(ui.painter(), rect.center(), 17.0, 2.0, ink, &[&[(3.0, 5.0), (21.0, 5.0), (21.0, 19.0), (3.0, 19.0), (3.0, 5.0)]]);
        ui.painter().rect_filled(Rect::from_min_max(rect.center() + Vec2::new(0.5, 0.5), rect.center() + Vec2::new(6.0, 4.5)), 1, ink);
        act.mini |= mini.clicked();
        right = rect.left() - 6.0;
        if talks && !self.is_chat(&row.id) && !row.spec.chat {
            let label = if narrow { "" } else { "Terminal" };
            let width = if narrow { 32.0 } else { 34.0 + ui.painter().layout_no_wrap(label.into(), font.clone(), look.soft).size().x + 10.0 };
            let rect = button(right, width);
            let r = bar_button(ui, rect, "chat_live", look, self.chat_terminal, true).tip("A tela do terminal, ao vivo");
            let p = ui.painter();
            let ink = if self.chat_terminal || r.hovered() { look.text } else { look.soft };
            glyph(p, Pos2::new(if narrow { rect.center().x } else { rect.left() + 18.0 }, middle), 16.0, 2.0, ink, PROMPT);
            p.text(Pos2::new(rect.left() + 32.0, middle), egui::Align2::LEFT_CENTER, label, font.clone(), ink);
            act.terminal |= r.clicked();
            if row.state == AgentState::Waiting {
                // it waits on that terminal: an amber badge on its corner
                p.circle(rect.right_top() + Vec2::new(-2.0, 2.0), 6.0, AMBER, Stroke::new(2.0, look.bg));
            }
            right = rect.left() - 6.0;
        }
        match search.as_mut() {
            Some(words) => {
                let field = button(right, if narrow { 130.0 } else { 200.0 });
                ui.painter().rect_filled(field, 8, look.surface);
                let edit = egui::TextEdit::singleline(words)
                    .id(egui::Id::unique(CHAT_SEARCH))
                    .hint_text(egui::RichText::new("Buscar na conversa").color(look.muted))
                    .font(FontId::proportional(13.0))
                    .text_color(look.text)
                    .margin(egui::Margin::ZERO)
                    .frame(egui::Frame::NONE);
                // a frameless field keeps no margin: its room is left out of the rect
                ui.put(Rect::from_min_max(field.min + Vec2::new(10.0, 7.0), field.max - Vec2::new(28.0, 7.0)), edit);
                let close = Rect::from_center_size(Pos2::new(field.right() - 15.0, middle), Vec2::splat(22.0));
                let x = ui
                    .interact(close, ui.scope_id().with("chat_search_close"), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .tip("Fechar a busca (Esc)");
                glyph(ui.painter(), close.center(), 13.0, 2.0, if x.hovered() { look.text } else { look.soft }, CROSS);
                if x.clicked() {
                    *search = None;
                }
                right = field.left() - 6.0;
            }
            None => {
                let rect = button(right, 32.0);
                let r = bar_button(ui, rect, "chat_search_open", look, false, false).tip(keys_label("Buscar na conversa (⌘F)"));
                paint_icon(ui.painter(), Icon::Search, rect.center(), if r.hovered() { look.text } else { look.soft });
                if r.clicked() {
                    *search = Some(String::new());
                    ui.memory_mut(|m| m.request_focus(egui::Id::unique(CHAT_SEARCH)));
                }
                right = rect.left() - 6.0;
            }
        }
        // the context ring: how much of the window is in use; it opens and closes the Contexto panel
        if self.context_wanted(&row.id) {
            let share = self.context_percent(&row.id);
            let words = share.filter(|_| !narrow).map(|s| format!("{s:.0}%"));
            let text = words.as_ref().map(|w| ui.painter().layout_no_wrap(w.clone(), font.clone(), look.soft));
            let rect = button(right, 36.0 + text.as_ref().map_or(0.0, |g| g.size().x + 8.0));
            let tip = match share {
                Some(s) => format!("Contexto: {s:.0}% da janela em uso · clique para ver o painel"),
                None => "Contexto e conectores".into(),
            };
            let r = bar_button(ui, rect, "chat_context", look, self.prefs.chat_panel, true).tip(tip);
            let p = ui.painter();
            ring(p, Pos2::new(rect.left() + 18.0, middle), share.unwrap_or(0.0) as f32 / 100.0, look);
            if let Some(g) = text {
                p.galley(Pos2::new(rect.left() + 34.0, middle - g.size().y / 2.0), g, look.soft);
            }
            act.context |= r.clicked();
            right = rect.left() - 6.0;
        }
        // the plan's 5-hour window, when a session told it
        let now = now_ms() / 1000;
        if let Some(window) = self.usage.plan.five_hour.filter(|w| w.resets_at > now && !narrow) {
            let clock = crate::app::usage::local_clock(window.resets_at);
            let label = ui.painter().layout_no_wrap("5 h".into(), font.clone(), look.soft);
            let words = ui.painter().layout_no_wrap(format!("{:.0}% · renova {clock}", window.used), font.clone(), look.soft);
            let rect = button(right, 10.0 + label.size().x + 8.0 + 48.0 + 8.0 + words.size().x + 10.0);
            let r = ui.interact(rect, ui.scope_id().with("chat_plan"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let p = ui.painter();
            if r.hovered() {
                p.rect_filled(rect, 8, look.hover);
            }
            // a click shows both windows, the session's and the week's
            let week = self.usage.plan.seven_day.filter(|w| w.resets_at > now);
            egui::Popup::menu(&r).show(|ui| {
                ui.set_width(250.0);
                plan_meter(ui, "Sessão (5 h)", window.used, format!("Renova às {clock}"), look);
                if let Some(week) = week {
                    ui.add_space(10.0);
                    let when = format!("Renova {} às {}", crate::app::usage::local_day(week.resets_at), crate::app::usage::local_clock(week.resets_at));
                    plan_meter(ui, "Semanal", week.used, when, look);
                }
            });
            p.rect_stroke(rect, 8, Stroke::new(1.0, look.line), StrokeKind::Inside);
            let mut x = rect.left() + 10.0;
            p.galley(Pos2::new(x, middle - label.size().y / 2.0), label.clone(), look.soft);
            x += label.size().x + 8.0;
            let bar = Rect::from_min_size(Pos2::new(x, middle - 2.0), Vec2::new(48.0, 4.0));
            p.rect_filled(bar, 2, look.surface);
            p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(48.0 * (window.used as f32 / 100.0).clamp(0.0, 1.0), 4.0)), 2, used_color(window.used));
            p.galley(Pos2::new(bar.right() + 8.0, middle - words.size().y / 2.0), words, look.soft);
            right = rect.left() - 6.0;
        }
        // the mark, in its rounded square
        let (name, folder, model) = self.header_facts(row);
        let claude = program(&row.spec, self.agent_sessions.contains_key(&row.id)) == Some("Claude Code");
        let square = Rect::from_center_size(Pos2::new(left + 14.0, middle), Vec2::splat(28.0));
        let p = ui.painter();
        p.rect_filled(square, 8, if claude { look.bg.lerp_to_gamma(CLAUDE, 0.12) } else { look.hover });
        let working = matches!(row.state, AgentState::Working(_));
        agent_mark(p, &row.spec, claude, square.center(), 16.0, look.text, working, t);
        // name · folder
        let x = square.right() + 12.0;
        let room = (right - 8.0 - x).max(0.0);
        let line1 = middle - 9.0;
        let size = if narrow { 14.0 } else { 15.0 };
        let named = clipped(p, name, FontId::proportional(size), look.strong, room);
        let name_at = Pos2::new(x, line1 - named.size().y / 2.0);
        // painted twice a pixel apart: the UI font has no bold weight
        let name_rect = Rect::from_min_size(name_at, named.size());
        p.galley(name_at + Vec2::new(1.0 / p.pixels_per_point(), 0.0), named.clone(), look.strong);
        p.galley(name_at, named, look.strong);
        if let Some(folder) = folder {
            let home = std::env::home_dir().map(|h| h.display().to_string()).unwrap_or_default();
            let dot = p.text(Pos2::new(name_rect.right() + 8.0, line1), egui::Align2::LEFT_CENTER, "·", FontId::proportional(13.0), look.muted);
            let mono = FontId::monospace(12.0);
            let char_w = p.layout_no_wrap("M".into(), mono.clone(), look.soft).size().x.max(1.0);
            let at = dot.right() + 8.0;
            let fits = ((x + room - at) / char_w).floor().max(0.0) as usize;
            let shown = short_path(&folder, &home, fits);
            if fits > 1 {
                let g = p.layout_no_wrap(shown.clone(), mono.clone(), look.soft);
                let rect = Rect::from_min_size(Pos2::new(at, line1 - g.size().y / 2.0), g.size());
                p.galley(rect.min, g, look.soft);
                let whole = crate::app::chat_direct::folder(&folder).map_or(folder.clone(), |f| f.display().to_string());
                ui.interact(rect, ui.scope_id().with("chat_folder_path"), Sense::hover()).tip(whole);
            }
        }
        // the state's dot and words, and the model
        let words = match row.state {
            AgentState::Working(since) => format!("Trabalhando · há {}", elapsed(self.now - since)),
            AgentState::Waiting => match self.waiting_secs(&row.id) {
                Some(s) => format!("Precisa de você · esperando no terminal há {}", elapsed(s)),
                None => "Precisa de você · esperando no terminal".into(),
            },
            AgentState::Idle(_) => "Pronto".into(),
            _ => self.state_note(row).map_or(row.state.label().to_string(), |n| format!("{} · {n}", row.state.label())),
        };
        let line = match &model {
            Some(model) => format!("{words} · {model}"),
            None => words,
        };
        let line2 = middle + 10.0;
        let dot = Pos2::new(x + 3.0, line2);
        let color = state_color(&row.state, look);
        let p = ui.painter();
        if matches!(row.state, AgentState::Working(_) | AgentState::Waiting) {
            crate::app::chat_list::pulse_dot(p, dot, 3.0, color, if working { 1.6 } else { 2.0 }, t);
        } else {
            p.circle_filled(dot, 3.0, color);
        }
        let g = clipped(p, line.clone(), font, look.muted, (room - 12.0).max(0.0));
        let text = Rect::from_min_size(Pos2::new(x + 12.0, line2 - g.size().y / 2.0), g.size());
        p.galley(text.min, g, look.muted);
        if matches!(row.state, AgentState::Active(_)) {
            ui.interact(text, ui.scope_id().with("chat_state"), Sense::hover())
                .tip(format!("{line}\n\nPela saída do terminal: este programa não diz se está trabalhando"));
        }
        ui.painter().hline(main.x_range(), top + height - 0.5, Stroke::new(1.0, look.hover));
        top + height - main.top()
    }
}

#[cfg(test)]
mod tests {
    use super::short_path;

    #[test]
    fn the_folder_shows_home_as_a_tilde_and_is_cut_in_the_middle() {
        assert_eq!(short_path("/home/ana/Desktop/projetos/magoagent", "/home/ana", 60), "~/Desktop/projetos/magoagent");
        assert_eq!(short_path("C:\\Users\\Ana\\Desktop\\loja", "c:\\users\\ana\\", 60), "~\\Desktop\\loja");
        assert_eq!(short_path("", "/home/ana", 60), "~");
        assert_eq!(short_path("~/x", "/home/ana", 60), "~/x");
        // another user's folder starting the same is not home
        assert_eq!(short_path("/home/anabela/x", "/home/ana", 60), "/home/anabela/x");
        let cut = short_path("/home/ana/Desktop/projetos/clientes/magoagent", "/home/ana", 20);
        assert_eq!(cut.chars().count(), 20);
        assert!(cut.starts_with("~/Desk") && cut.ends_with("magoagent") && cut.contains('…'), "{cut}");
        assert_eq!(short_path("/muito/longo", "", 1), "…");
    }
}

/// Green, amber past 70%, red past 90%.
fn used_color(used: f64) -> Color32 {
    match used {
        u if u >= 90.0 => RED,
        u if u >= 70.0 => AMBER,
        _ => READY,
    }
}

/// A window of the plan in the chip's popup: its name and share, the bar, when it renews.
fn plan_meter(ui: &mut egui::Ui, name: &str, used: f64, when: String, look: &Look) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::hover());
    let p = ui.painter();
    let (left, right) = (rect.left() + 4.0, rect.right() - 4.0);
    p.text(Pos2::new(left, rect.top() + 2.0), egui::Align2::LEFT_TOP, name, FontId::proportional(13.0), look.text);
    p.text(Pos2::new(right, rect.top() + 2.0), egui::Align2::RIGHT_TOP, format!("{used:.0}%"), FontId::proportional(13.0), look.text);
    let bar = Rect::from_min_max(Pos2::new(left, rect.top() + 22.0), Pos2::new(right, rect.top() + 27.0));
    p.rect_filled(bar, 3, look.surface);
    p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * (used as f32 / 100.0).clamp(0.0, 1.0), bar.height())), 3, used_color(used));
    p.text(Pos2::new(left, rect.top() + 32.0), egui::Align2::LEFT_TOP, when, FontId::proportional(11.5), look.soft);
}
