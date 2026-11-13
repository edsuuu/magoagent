//! What the agent does, in the chat: its tool calls as steps in the conversation (a run of them
//! folds into one line), the band over the box with the one running now, the amber card when it
//! waits on its terminal, and the messages queued meanwhile.

use crate::app::chat::list::{ACCENT, AMBER, CHECK, CROSS, DOWN, GREEN, Look, PROMPT, RED, RIGHT, bold, clipped, code_card, elapsed, glyph, pulse_dot};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::transcript::{ChatItem, Step, Transcript};

/// What the waiting card's buttons ask: its terminal, or a menu's choice (from the marked one, to).
pub(crate) enum CardClick {
    Terminal,
    Pick(usize, usize),
}

/// How a step stands.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Live {
    Done,
    Running,
    /// Running, while its agent waits on its terminal (a permission it asked for).
    Waiting,
}

/// Three dots lighting up in turn (1.2 s, 0.2 s apart): something runs. 23 pt wide.
pub(crate) fn busy_dots(p: &Painter, left: Pos2, color: Color32, t: f64) {
    let smooth = |x: f32| x * x * (3.0 - 2.0 * x);
    for k in 0..3 {
        let x = ((t - 0.2 * k as f64).rem_euclid(1.2) / 1.2) as f32;
        let on = if x < 0.4 {
            smooth(x / 0.4)
        } else if x < 0.8 {
            1.0 - smooth((x - 0.4) / 0.4)
        } else {
            0.0
        };
        p.circle_filled(left + Vec2::new(2.5 + 9.0 * k as f32, 0.0), 2.5, color.gamma_multiply(0.25 + 0.75 * on));
    }
}

/// A count the output shows ("14 / 23", "3/10"): the last one, done ≤ total. Pieces of paths and
/// dates ("a/1/2", "2026/10/28") are no count.
pub(crate) fn counter(line: &str) -> Option<(u32, u32)> {
    let mut found = None;
    for (i, _) in line.match_indices('/') {
        let (left, right) = (line[..i].trim_end(), line[i + 1..].trim_start());
        let done = &left[left.trim_end_matches(|c: char| c.is_ascii_digit()).len()..];
        let total = &right[..right.len() - right.trim_start_matches(|c: char| c.is_ascii_digit()).len()];
        let before = left[..left.len() - done.len()].chars().next_back();
        let after = right[total.len()..].chars().next();
        let joined = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '/' || c == '.');
        if done.is_empty() || total.is_empty() || joined(before) || joined(after) {
            continue;
        }
        if let (Ok(done), Ok(total)) = (done.parse(), total.parse())
            && done <= total
            && total > 0
        {
            found = Some((done, total));
        }
    }
    found
}

/// A folded run of steps: "4 ferramentas · Edit, Write, Bash ×2 · 46 s".
pub(crate) fn run_summary(steps: &[&Step]) -> String {
    let mut names: Vec<(&str, usize)> = Vec::new();
    for step in steps {
        let name = short_tool(&step.name);
        match names.iter_mut().find(|(n, _)| *n == name) {
            Some(seen) => seen.1 += 1,
            None => names.push((name, 1)),
        }
    }
    let list: Vec<String> = names.iter().map(|(n, k)| if *k > 1 { format!("{n} ×{k}") } else { n.to_string() }).collect();
    let start = steps.iter().filter_map(|s| s.started).reduce(f64::min);
    let end = steps.iter().filter_map(|s| s.ended).reduce(f64::max);
    let time = match (start, end) {
        (Some(start), Some(end)) if end >= start => format!(" · {}", elapsed(end - start)),
        _ => String::new(),
    };
    format!("{} ferramentas · {}{time}", steps.len(), list.join(", "))
}

/// The tool running now (the latest called and not answered) and how many more run beside it.
pub(crate) fn running_step(transcript: &Transcript) -> Option<(Step, usize)> {
    let (id, what) = transcript.running.last()?;
    let step = transcript.items.iter().rev().find_map(|i| match i {
        ChatItem::Tool(s) if &s.id == id => Some(s.clone()),
        _ => None,
    });
    // called before the part of the file read: its name and input are all there is
    let (name, what) = what.split_once(' ').unwrap_or((what, ""));
    let step = step.unwrap_or_else(|| Step { id: id.clone(), name: name.into(), what: what.into(), ..Default::default() });
    Some((step, transcript.running.len() - 1))
}

/// The tool's mark on the band's chip.
fn tool_icon(p: &Painter, name: &str, at: Pos2, color: Color32) {
    let icon = match name {
        "Bash" => return glyph(p, at, 12.0, 2.4, color, PROMPT),
        "Read" => Icon::Eye,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => Icon::Edit,
        "Grep" | "Glob" | "WebSearch" => Icon::Search,
        _ => Icon::More,
    };
    paint_icon_scaled(p, icon, at, color, 0.75, 0.0);
}

/// "0:08".
fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl DrawingApp {
    /// A run of tool steps one after another. Two or more with something said after them fold
    /// into a line that opens on a click (`fold` gets the run's key); the latest stay open.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn tool_run(
        &self,
        ui: &mut egui::Ui,
        steps: &[&Step],
        trailing: bool,
        running: &[(String, String)],
        waiting: bool,
        look: &Look,
        narrow: bool,
        t: f64,
        fold: &mut Option<String>,
    ) {
        let key = steps[0].id.clone();
        if steps.len() >= 2 && !trailing {
            let open = self.chat_unfolded.contains(&key);
            let font = FontId::monospace(if narrow { 11.5 } else { 12.0 });
            let text = ui.painter().layout_no_wrap(run_summary(steps), font, look.muted);
            let (rect, response) = ui.allocate_exact_size(Vec2::new(text.size().x + 30.0, if narrow { 22.0 } else { 24.0 }), Sense::click());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            let p = ui.painter();
            if response.hovered() {
                p.rect_filled(rect, 6, look.surface.gamma_multiply(0.6));
            }
            glyph(p, Pos2::new(rect.left() + 10.0, rect.center().y), 12.0, 2.4, look.muted, if open { DOWN } else { RIGHT });
            p.galley(Pos2::new(rect.left() + 22.0, rect.center().y - text.size().y / 2.0), text, look.muted);
            if response.clicked() {
                *fold = Some(key);
            }
            if !open {
                self.step_files(ui, steps, &self.colors());
                ui.add_space(8.0);
                return;
            }
        }
        let c = self.colors();
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for step in steps {
                let live = match running.iter().any(|(id, _)| *id == step.id) {
                    true if waiting => Live::Waiting,
                    true => Live::Running,
                    false => Live::Done,
                };
                // a click on its row opens its card, kept open as the runs are (`chat_unfolded`)
                let card = format!("card:{}", step.id);
                if step_row(ui, step, live, look, narrow, t) {
                    *fold = Some(card.clone());
                }
                if self.chat_unfolded.contains(&card) && step_card(ui, step, live, look, &c) {
                    *fold = Some(card.clone());
                }
            }
        });
        self.step_files(ui, steps, &c);
        ui.add_space(8.0);
    }

    /// The band over the box while the agent works: dots, the tool and its command, a bar when its
    /// output counts ("14 / 23"), how long it runs and Interromper. True: Interromper clicked.
    pub(crate) fn work_band(&self, ui: &mut egui::Ui, row: &AgentRow, transcript: Option<&Transcript>, look: &Look, narrow: bool, t: f64) -> bool {
        let AgentState::Working(since) = row.state else { return false };
        let running = transcript.and_then(running_step);
        let margin = if narrow { egui::Margin { left: 12, right: 5, top: 5, bottom: 5 } } else { egui::Margin { left: 14, right: 6, top: 6, bottom: 6 } };
        let frame = egui::Frame::new().fill(look.band).stroke(Stroke::new(1.0, look.line)).corner_radius(12).inner_margin(margin);
        frame
            .show(ui, |ui| {
                // Claude's band is taller: its mascot works in it, in the pose of what it does
                let claude = crate::app::mascot::poses::claude(&row.spec);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), if claude { 44.0 } else { 28.0 }), Sense::hover());
                let (cy, gap) = (rect.center().y, if narrow { 8.0 } else { 12.0 });
                let small = FontId::monospace(if narrow { 11.5 } else { 12.0 });
                // from the right: Interromper, the time, the count and its bar
                let label = ui.painter().layout_no_wrap("Interromper".into(), FontId::proportional(13.0), look.text);
                let kbd = ui.painter().layout_no_wrap("Esc".into(), FontId::monospace(10.5), look.soft);
                let width = if narrow { 28.0 } else { 10.0 + 12.0 + 7.0 + label.size().x + 7.0 + kbd.size().x + 10.0 + 6.0 };
                let button = Rect::from_min_max(Pos2::new(rect.right() - width, cy - 14.0), Pos2::new(rect.right(), cy + 14.0));
                let response = ui
                    .interact(button, ui.scope_id().with("chat_interrupt"), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .tip("Interromper (Esc)");
                let p = ui.painter();
                p.rect_filled(button, 8, if response.hovered() { look.selected } else { look.raised });
                let stop = |at: Pos2| p.rect_filled(Rect::from_center_size(at, Vec2::splat(if narrow { 6.5 } else { 7.0 })), 1.5, look.text);
                if narrow {
                    stop(button.center());
                } else {
                    stop(Pos2::new(button.left() + 16.0, cy));
                    let x = button.left() + 29.0;
                    p.galley(Pos2::new(x, cy - label.size().y / 2.0), label.clone(), look.text);
                    let key = Rect::from_min_size(Pos2::new(x + label.size().x + 7.0, cy - 8.0), Vec2::new(kbd.size().x + 10.0, 16.0));
                    p.rect_stroke(key, 4, Stroke::new(1.0, look.line), StrokeKind::Inside);
                    p.galley(key.center() - kbd.size() / 2.0, kbd, look.soft);
                }
                let mut right = button.left() - gap;
                let started = running.as_ref().and_then(|(s, _)| s.started);
                let seconds = started.map_or(self.now - since, |s| now_ms() as f64 / 1000.0 - s);
                if !narrow {
                    let g = p.layout_no_wrap(clock(seconds), small.clone(), look.muted);
                    right -= g.size().x;
                    p.galley(Pos2::new(right, cy - g.size().y / 2.0), g, look.muted);
                    right -= gap;
                }
                if let Some((done, total)) = running.as_ref().and_then(|(s, _)| s.output.as_deref()).and_then(counter) {
                    let g = p.layout_no_wrap(format!("{done}/{total}"), FontId::proportional(if narrow { 12.0 } else { 12.5 }), look.soft);
                    right -= g.size().x;
                    p.galley(Pos2::new(right, cy - g.size().y / 2.0), g, look.soft);
                    let bar = Rect::from_min_max(Pos2::new(right - gap - if narrow { 44.0 } else { 120.0 }, cy - 2.0), Pos2::new(right - gap, cy + 2.0));
                    p.rect_filled(bar, 2, look.line);
                    p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * done as f32 / total as f32, 4.0)), 2, ACCENT);
                    right = bar.left() - gap;
                }
                // from the left: Claude Code's mascot at work (the dots for other agents), the tool's chip, its command
                let mut x = if claude {
                    let tool = running.as_ref().map(|(s, _)| (s.name.as_str(), s.what.as_str()));
                    // the message it answers: the subject of its thinking
                    let topic =
                        transcript.and_then(|t| t.items.iter().rev().find_map(|i| if let ChatItem::User(text) = i { Some(text.as_str()) } else { None }));
                    let pose = crate::app::mascot::poses::working(tool, topic.unwrap_or(""), seconds, since.to_bits());
                    crate::app::mascot::poses::paint_pose(p, pose, Pos2::new(rect.left() + 26.0, cy), 60.0, t, true, 1.0);
                    rect.left() + 54.0 + gap
                } else {
                    busy_dots(p, Pos2::new(rect.left(), cy), GREEN, t);
                    rect.left() + 23.0 + gap
                };
                match &running {
                    Some((step, more)) => {
                        let name = p.layout_no_wrap(short_tool(&step.name).to_string(), small.clone(), look.text);
                        let chip = Rect::from_min_size(Pos2::new(x, cy - 11.0), Vec2::new(8.0 + 12.0 + 6.0 + name.size().x + 8.0, 22.0));
                        p.rect_filled(chip, 6, look.raised);
                        tool_icon(p, &step.name, Pos2::new(chip.left() + 14.0, cy), look.text);
                        p.galley(Pos2::new(chip.left() + 26.0, cy - name.size().y / 2.0), name, look.text);
                        x = chip.right() + gap;
                        let what = if *more > 0 { format!("{} +{more}", step.what) } else { step.what.clone() };
                        let g = clipped(p, what, small, look.soft, (right - x).max(0.0));
                        p.galley(Pos2::new(x, cy - g.size().y / 2.0), g, look.soft);
                    }
                    None => {
                        let name = row.spec.name.split(" · ").next().filter(|n| !n.is_empty()).unwrap_or("O agente");
                        let words = crate::app::chat::thinking::line(name, since, self.now - since);
                        let bright = if claude { crate::app::chat::thinking::CLAUDE } else { look.text };
                        crate::app::chat::thinking::shimmer(p, Pos2::new(x, cy), &words, FontId::proportional(13.0), look.soft, bright, t);
                    }
                }
                response.clicked()
            })
            .inner
    }

    /// Waiting on its terminal: the amber card takes the band's place, with the menu it waits on as
    /// buttons (its question over them) or, when the screen shows none, its last lines; and
    /// "Abrir terminal".
    pub(crate) fn waiting_card(&self, ui: &mut egui::Ui, row: &AgentRow, look: &Look, narrow: bool, t: f64) -> Option<CardClick> {
        let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        let (fill, edge, title, note) =
            if look.dark { (hex(0x2a2213), hex(0x7a5a22), hex(0xf6c46f), hex(0xd8c7a6)) } else { (hex(0xfdf3e1), hex(0xe6b866), hex(0x8a5a00), hex(0x6b5a3a)) };
        let (well, well_edge) = if look.dark { (hex(0x17130b), hex(0x3d2f14)) } else { (hex(0xfffaf0), hex(0xefd3a0)) };
        let screen = self.terminals.get(&row.id).and_then(|s| s.as_ref().ok()).map(|s| s.handle.screen_text()).unwrap_or_default();
        let menu = menu_choices(&screen);
        let why = match (self.agent_sessions.get(&row.id).and_then(|s| s.waiting_for.as_deref()), &menu) {
            (Some(w), _) if w.contains("permission") => "Pediu permissão".to_string(),
            (_, Some(_)) => "Escolha uma opção".into(),
            (Some(w), None) if w.contains("input") => "Espera a sua resposta no terminal".into(),
            (Some(w), None) => format!("Espera: {w}"),
            (None, None) => "Uma pergunta ou um menu na tela do terminal".into(),
        };
        let why = match self.waiting_secs(&row.id) {
            Some(s) => format!("{why} · há {}", elapsed(s)),
            None => why,
        };
        let size = if narrow { 13.5 } else { 14.0 };
        let mut click = None;
        egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, edge)).corner_radius(12).inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            // the head: the dot, who and why, and the terminal a click away
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::hover());
            let label = ui.painter().layout_no_wrap("Abrir terminal".into(), FontId::proportional(13.0), title);
            let button =
                Rect::from_min_max(Pos2::new(rect.right() - (12.0 + 16.0 + 6.0 + label.size().x + 12.0), rect.top() + 2.0), rect.max - Vec2::new(0.0, 2.0));
            let response = ui.interact(button, ui.scope_id().with("chat_open_terminal"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let p = ui.painter();
            pulse_dot(p, Pos2::new(rect.left() + 5.0, rect.center().y), 4.5, AMBER, 2.0, t);
            let room = button.left() - rect.left() - 20.0 - 12.0;
            let head = clipped(p, format!("{} precisa de você", row.spec.label()), FontId::proportional(size), title, room).text().to_string();
            bold(p, Pos2::new(rect.left() + 20.0, rect.top()), &head, size, title);
            p.galley(Pos2::new(rect.left() + 20.0, rect.top() + 19.0), clipped(p, why, FontId::proportional(12.5), note, room), note);
            p.rect(button, 8, if response.hovered() { AMBER.gamma_multiply(0.18) } else { Color32::TRANSPARENT }, Stroke::new(1.0, edge), StrokeKind::Inside);
            glyph(p, Pos2::new(button.left() + 20.0, button.center().y), 16.0, 2.2, title, PROMPT);
            p.galley(Pos2::new(button.left() + 34.0, button.center().y - label.size().y / 2.0), label, title);
            if response.clicked() {
                click = Some(CardClick::Terminal);
            }
            let Some(menu) = menu else {
                // no menu to read: its last lines
                let lines: Vec<&str> = screen.lines().collect();
                let end = lines.iter().rposition(|l| !l.trim().is_empty()).map_or(0, |i| i + 1);
                egui::Frame::new().fill(well).stroke(Stroke::new(1.0, well_edge)).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 8)).show(
                    ui,
                    |ui| {
                        ui.set_width(ui.available_width());
                        let tail = lines[end.saturating_sub(8)..end].join("\n");
                        ui.add(egui::Label::new(egui::RichText::new(tail).monospace().size(12.5).color(look.text)).extend());
                    },
                );
                return;
            };
            if !menu.question.is_empty() {
                ui.add(egui::Label::new(egui::RichText::new(menu.question.join("\n")).size(13.5).color(look.text)).wrap());
            }
            for (k, choice) in menu.choices.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                let ground = if response.hovered() { AMBER.gamma_multiply(0.22) } else { well };
                let p = ui.painter();
                p.rect(rect, 8, ground, Stroke::new(1.0, if k == menu.marked { AMBER } else { well_edge }), StrokeKind::Inside);
                let text = clipped(p, choice.clone(), FontId::proportional(13.5), look.text, rect.width() - 24.0);
                p.galley(Pos2::new(rect.left() + 12.0, rect.center().y - text.size().y / 2.0), text, look.text);
                if response.clicked() {
                    click = Some(CardClick::Pick(menu.marked, k));
                }
            }
        });
        click
    }

    /// What was written to an agent while it waited on its terminal: it goes when it doesn't.
    pub(crate) fn queue_message(&mut self, id: &str, text: String) {
        match self.chat_queue.iter_mut().find(|(to, _)| to == id) {
            Some((_, queued)) => *queued = format!("{queued}\n\n{text}"),
            None => self.chat_queue.push((id.to_string(), text)),
        }
    }

    /// Sends what was queued to the agents no longer waiting on their terminal; an agent gone
    /// gives its words back to the box.
    pub(crate) fn flush_chat_queue(&mut self) {
        for (id, text) in std::mem::take(&mut self.chat_queue) {
            let ready = self.terminals.get(&id).and_then(|s| s.as_ref().ok()).is_some_and(|s| s.handle.ready().is_ok());
            match self.agent_state(&id) {
                AgentState::Waiting => self.chat_queue.push((id, text)),
                AgentState::Ended => {
                    self.toast(format!("{} fechou: a mensagem da fila voltou para a caixa.", self.terminal_name(&id)));
                    let gap = if self.chat_draft.is_empty() { "" } else { "\n" };
                    self.chat_draft = format!("{}{gap}{text}", self.chat_draft);
                }
                _ if ready => {
                    self.send_prompt(&id, &text);
                }
                _ => self.chat_queue.push((id, text)),
            }
        }
    }

    /// The person's messages queued for `id`, at the conversation's end; "Editar" takes them back
    /// into the box (`edit`).
    pub(crate) fn queued_bubbles(&self, ui: &mut egui::Ui, id: &str, width: f32, look: &Look, edit: &mut bool) {
        for (_, text) in self.chat_queue.iter().filter(|(to, _)| to == id) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                *edit |= ui
                    .add(egui::Button::new(egui::RichText::new("Editar").size(11.5).color(look.soft)).frame(false))
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .clicked();
                ui.label(egui::RichText::new("Na fila · vai quando você responder no terminal ·").size(11.5).color(look.muted));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                egui::Frame::new()
                    .fill(crate::app::chat::bubble_fill(look.dark, true).gamma_multiply(0.55))
                    .corner_radius(18)
                    .inner_margin(egui::Margin::symmetric(14, 7))
                    .show(ui, |ui| {
                        ui.set_max_width(width * 0.7);
                        ui.label(egui::RichText::new(text).size(14.0).color(Color32::WHITE));
                    });
            });
            ui.add_space(8.0);
        }
    }
}

/// A tool's name as the line shows it: an MCP tool (`mcp__server__tool`) by its own name.
pub(crate) fn short_tool(name: &str) -> &str {
    name.strip_prefix("mcp__").and_then(|rest| rest.split_once("__")).map_or(name, |(_, tool)| tool)
}

/// One step: ✓ (✗ failed) or a pulsing dot while it runs, its name in a 6ch column, what it was
/// given and what its answer says; a running command's last output line under it. True: clicked.
pub(crate) fn step_row(ui: &mut egui::Ui, step: &Step, live: Live, look: &Look, narrow: bool, t: f64) -> bool {
    let size = if narrow { 12.0 } else { 12.5 };
    let font = FontId::monospace(size);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if response.hovered() {
        p.rect_filled(rect.expand2(Vec2::new(0.0, 2.0)), 4, look.surface.gamma_multiply(0.6));
    }
    let (x0, cy) = (rect.left() + if narrow { 4.0 } else { 6.0 }, rect.center().y);
    let mark = Pos2::new(x0 + 6.5, cy);
    match live {
        Live::Running => pulse_dot(p, mark, 3.5, GREEN, 1.6, t),
        Live::Waiting => pulse_dot(p, mark, 3.5, AMBER, 2.0, t),
        Live::Done if step.failed => glyph(p, mark, 12.0, 2.6, RED, CROSS),
        // no answer seen (cut off, or before what was read): a quieter check
        Live::Done => glyph(p, mark, 13.0, 2.6, if step.ended.is_some() { GREEN } else { look.muted }, CHECK),
    }
    let live_now = live != Live::Done;
    let name_x = x0 + 13.0 + if narrow { 7.0 } else { 8.0 };
    let column = p.layout_no_wrap("000000".into(), font.clone(), look.text).size().x;
    let name = p.layout_no_wrap(short_tool(&step.name).to_string(), font.clone(), look.text);
    // a name longer than the column pushes what it was given along, never under it
    let column = column.max(name.size().x);
    p.galley(Pos2::new(name_x, cy - name.size().y / 2.0), name, look.text);
    // what its answer says, at the line's end: measured first, the input is cut before it
    let mut tail: Vec<(String, Color32)> = Vec::new();
    match live {
        Live::Running => tail.push(("· rodando".into(), GREEN)),
        Live::Waiting => tail.push(("· aguardando você".into(), AMBER)),
        Live::Done => {
            if let Some(note) = &step.note {
                tail.push((format!("· {note}"), look.muted));
            }
            if let Some((added, removed)) = step.diff.filter(|_| step.note.is_none()) {
                tail.push(("·".into(), look.muted));
                tail.push((format!("+{added}"), GREEN));
                tail.push((format!("−{removed}"), RED));
            }
        }
    }
    let tail: Vec<_> = tail.into_iter().map(|(text, color)| p.layout_no_wrap(text, font.clone(), color)).collect();
    let tail_width: f32 = tail.iter().map(|g| g.size().x + 8.0).sum();
    let what_x = name_x + column + 8.0;
    let what_color = if live_now { look.text.gamma_multiply(0.9) } else { look.soft };
    let what = clipped(p, step.what.clone(), font.clone(), what_color, (rect.right() - what_x - tail_width).max(0.0));
    let mut x = what_x + what.size().x + 8.0;
    p.galley(Pos2::new(what_x, cy - what.size().y / 2.0), what, what_color);
    for g in tail {
        let w = g.size().x;
        p.galley(Pos2::new(x, cy - g.size().y / 2.0), g, look.muted);
        x += w + 8.0;
    }
    if let Some(out) = step.output.as_ref().filter(|_| live_now) {
        let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 17.0), Sense::hover());
        let g = clipped(ui.painter(), out.clone(), FontId::monospace(size - 0.5), look.muted, line.right() - name_x);
        ui.painter().galley(Pos2::new(name_x, line.center().y - g.size().y / 2.0), g, look.muted);
    }
    response.clicked()
}

/// How many lines of what a step printed its card shows.
const CARD_LINES: usize = 40;

/// The end of a long output, for the card (hundreds of lines made a block several screens tall):
/// its last `CARD_LINES` lines under a line saying how many are above. `None` when it all fits.
fn card_output(log: &str) -> Option<String> {
    let lines: Vec<&str> = log.lines().collect();
    let above = lines.len().checked_sub(CARD_LINES).filter(|n| *n > 0)?;
    Some(format!("… ({above} linhas acima; Copiar leva todas)\n{}", lines[above..].join("\n")))
}

/// A step opened: how it stands and how long it took, its folder and files, its whole input
/// and what it printed (each with Copiar), and why it failed. True: clicked where nothing else
/// takes the click (its text, Copiar), which closes it as its row does.
fn step_card(ui: &mut egui::Ui, step: &Step, live: Live, look: &Look, c: &UiColors) -> bool {
    // under what it holds, so Copiar and the selectable text keep their clicks: last frame's place
    let key = ui.scope_id().with(("step_card", &step.id));
    let last = ui.ctx().data_mut(|d| d.get_temp::<Rect>(key));
    let clicked = last.is_some_and(|rect| ui.interact(rect, key, Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked());
    let state = match live {
        Live::Running => "rodando".to_string(),
        Live::Waiting => "aguardando você".into(),
        Live::Done if step.failed => "falhou".into(),
        Live::Done if step.ended.is_none() => "sem resposta".into(),
        Live::Done => "terminou".into(),
    };
    let took = match (step.started, step.ended) {
        (Some(start), Some(end)) if end >= start => format!(" em {}", elapsed(end - start)),
        _ => String::new(),
    };
    let mut lines = vec![format!("{} · {state}{took}", step.name)];
    lines.extend(step.cwd.as_ref().map(|cwd| format!("Pasta: {cwd}")));
    if !step.files.is_empty() {
        lines.push(format!("Arquivos: {}", step.files.join(", ")));
    }
    let frame = egui::Frame::new().inner_margin(egui::Margin { left: 27, right: 0, top: 0, bottom: 4 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(lines.join("\n")).monospace().size(11.5).color(look.muted));
        let input = if step.input.is_empty() { &step.what } else { &step.input };
        if !input.is_empty() {
            code_card(ui, if step.name == "Bash" { "comando" } else { "entrada" }, input, c, look.dark);
        }
        if !step.log.is_empty() {
            match card_output(&step.log) {
                Some(end) => crate::app::chat::list::code_box(ui, "saída", &end, &step.log, c, look.dark),
                None => code_card(ui, "saída", &step.log, c, look.dark),
            }
        }
        if let Some(error) = &step.error {
            ui.label(egui::RichText::new(error).monospace().size(12.0).color(RED));
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(key, frame.response.rect));
    clicked
}

#[cfg(test)]
mod tests {
    use crate::transcript::Step;

    #[test]
    fn an_mcp_tool_goes_by_its_own_name() {
        assert_eq!(super::short_tool("mcp__magoagent__open_path"), "open_path");
        assert_eq!(super::short_tool("ToolSearch"), "ToolSearch");
    }

    #[test]
    fn a_count_in_the_output_is_read_and_paths_or_dates_are_not() {
        assert_eq!(super::counter("..............        14 / 23 ( 60%)"), Some((14, 23)));
        assert_eq!(super::counter("[3/10] compilando"), Some((3, 10)));
        assert_eq!(super::counter("src/1/2 2026/10/28 v1.2/3"), None);
        assert_eq!(super::counter("30 / 4"), None, "more done than the total: no count");
        assert_eq!(super::counter("sem contador"), None);
    }

    #[test]
    fn a_long_output_shows_its_end_in_the_card() {
        assert_eq!(super::card_output("uma\nduas"), None, "what fits shows whole");
        let long: Vec<String> = (1..=100).map(|n| format!("linha {n}")).collect();
        let end = super::card_output(&long.join("\n")).unwrap();
        assert!(end.starts_with("… (60 linhas acima; Copiar leva todas)\nlinha 61\n") && end.ends_with("linha 100"), "{end}");
        assert_eq!(end.lines().count(), super::CARD_LINES + 1);
    }

    #[test]
    fn the_band_names_the_latest_tool_running_and_how_many_more() {
        use crate::transcript::{ChatItem, Transcript};
        let mut t = Transcript::new(std::path::PathBuf::new());
        assert!(super::running_step(&t).is_none(), "answered: it thinks");
        let read = Step { id: "b".into(), name: "Read".into(), what: "b.rs".into(), started: Some(5.0), ..Default::default() };
        t.items.push(ChatItem::Tool(read.clone()));
        t.running = vec![("a".into(), "Read a.rs".into()), ("b".into(), "Read b.rs".into())];
        assert_eq!(super::running_step(&t), Some((read, 1)));
        // called before what was read of the file: from its one line
        t.running = vec![("c".into(), "Bash cargo test --all".into())];
        let (step, more) = super::running_step(&t).unwrap();
        assert_eq!((step.name.as_str(), step.what.as_str(), more), ("Bash", "cargo test --all", 0));
    }

    #[test]
    fn a_folded_run_says_its_tools_and_how_long_they_took() {
        let step = |name: &str, started: f64, ended: Option<f64>| Step { name: name.into(), started: Some(started), ended, ..Default::default() };
        let run = [step("Edit", 100.0, Some(101.0)), step("Write", 102.0, Some(103.0)), step("Bash", 104.0, Some(120.0)), step("Bash", 121.0, Some(146.0))];
        let refs: Vec<&Step> = run.iter().collect();
        assert_eq!(super::run_summary(&refs), "4 ferramentas · Edit, Write, Bash ×2 · 46 s");
        let unanswered = [step("Read", 1.0, None), step("Read", 2.0, None)];
        assert_eq!(super::run_summary(&unanswered.iter().collect::<Vec<_>>()), "2 ferramentas · Read ×2");
    }
}
