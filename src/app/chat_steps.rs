//! What the agent does, in the chat: its tool calls as steps in the conversation (a run of them
//! folds into one line), the band over the box with the one running now, the amber card when it
//! waits on its terminal, and the messages queued meanwhile.

use crate::app::chat_list::{ACCENT, AMBER, CHECK, CROSS, DOWN, GREEN, Look, PROMPT, RED, RIGHT, bold, clipped, elapsed, glyph, pulse_dot};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::transcript::{ChatItem, Step, Transcript};

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
        match names.iter_mut().find(|(n, _)| *n == step.name) {
            Some(seen) => seen.1 += 1,
            None => names.push((&step.name, 1)),
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
                ui.add_space(8.0);
                return;
            }
        }
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for step in steps {
                let live = match running.iter().any(|(id, _)| *id == step.id) {
                    true if waiting => Live::Waiting,
                    true => Live::Running,
                    false => Live::Done,
                };
                step_row(ui, step, live, look, narrow, t);
            }
        });
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
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
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
                    .on_hover_text("Interromper (Esc)");
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
                // from the left: the dots, the tool's chip, its command
                busy_dots(p, Pos2::new(rect.left(), cy), GREEN, t);
                let mut x = rect.left() + 23.0 + gap;
                match &running {
                    Some((step, more)) => {
                        let name = p.layout_no_wrap(step.name.clone(), small.clone(), look.text);
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
                        p.text(Pos2::new(x, cy), egui::Align2::LEFT_CENTER, "Pensando", FontId::proportional(13.0), look.soft);
                    }
                }
                response.clicked()
            })
            .inner
    }

    /// Waiting on its terminal: the amber card takes the band's place, with the terminal's last
    /// lines and "Abrir terminal". True: that button clicked.
    pub(crate) fn waiting_card(&self, ui: &mut egui::Ui, row: &AgentRow, look: &Look, narrow: bool, t: f64) -> bool {
        let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        let (fill, edge, title, note) =
            if look.dark { (hex(0x2a2213), hex(0x7a5a22), hex(0xf6c46f), hex(0xd8c7a6)) } else { (hex(0xfdf3e1), hex(0xe6b866), hex(0x8a5a00), hex(0x6b5a3a)) };
        let why = match self.agent_sessions.get(&row.id).and_then(|s| s.waiting_for.as_deref()) {
            Some(w) if w.contains("permission") => "O Claude Code pediu permissão".to_string(),
            Some(w) if w.contains("input") => "O Claude Code espera a sua resposta".into(),
            Some(w) => format!("O Claude Code espera: {w}"),
            None => "Uma pergunta ou um menu na tela do terminal".into(),
        };
        let why = match self.waiting_secs(&row.id) {
            Some(s) => format!("{why} · há {}", elapsed(s)),
            None => why,
        };
        let size = if narrow { 13.5 } else { 14.0 };
        egui::Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0, edge))
            .corner_radius(12)
            .inner_margin(egui::Margin { left: 14, right: 12, top: 12, bottom: 12 })
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::hover());
                let ink = hex(0x1d1508);
                let label = ui.painter().layout_no_wrap("Abrir terminal".into(), FontId::proportional(13.5), ink);
                let button = Rect::from_min_max(
                    Pos2::new(rect.right() - (12.0 + 16.0 + 8.0 + label.size().x + 14.0), rect.center().y - 16.0),
                    Pos2::new(rect.right(), rect.center().y + 16.0),
                );
                let response = ui.interact(button, ui.scope_id().with("chat_open_terminal"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                let p = ui.painter();
                pulse_dot(p, Pos2::new(rect.left() + 4.5, rect.center().y), 4.5, AMBER, 2.0, t);
                let room = button.left() - rect.left() - 21.0 - 12.0;
                let head = format!("{} precisa de você no terminal", row.spec.label());
                let head = clipped(p, head, FontId::proportional(size), title, room).text().to_string();
                bold(p, Pos2::new(rect.left() + 21.0, rect.top()), &head, size, title);
                p.galley(Pos2::new(rect.left() + 21.0, rect.top() + 20.0), clipped(p, why, FontId::proportional(12.5), note, room), note);
                p.rect_filled(button, 8, if response.hovered() { AMBER.gamma_multiply(0.85) } else { AMBER });
                glyph(p, Pos2::new(button.left() + 20.0, button.center().y), 16.0, 2.2, ink, PROMPT);
                bold(p, Pos2::new(button.left() + 36.0, button.center().y - label.size().y / 2.0), "Abrir terminal", 13.5, ink);
                ui.add_space(4.0);
                // the menu it waits on, as its screen ends
                let screen = self.terminals.get(&row.id).and_then(|s| s.as_ref().ok()).map(|s| s.handle.screen_text()).unwrap_or_default();
                let lines: Vec<&str> = screen.lines().collect();
                let end = lines.iter().rposition(|l| !l.trim().is_empty()).map_or(0, |i| i + 1);
                let tail = lines[end.saturating_sub(8)..end].join("\n");
                egui::Frame::new()
                    .fill(hex(0x17130b))
                    .stroke(Stroke::new(1.0, hex(0x3d2f14)))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::Label::new(egui::RichText::new(tail).monospace().size(12.5).color(hex(0xe8dcc4))).extend());
                    });
                response.clicked()
            })
            .inner
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

/// One step: ✓ (✗ failed) or a pulsing dot while it runs, its name in a 6ch column, what it was
/// given and what its answer says; a running command's last output line under it.
fn step_row(ui: &mut egui::Ui, step: &Step, live: Live, look: &Look, narrow: bool, t: f64) {
    let size = if narrow { 12.0 } else { 12.5 };
    let font = FontId::monospace(size);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    let p = ui.painter();
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
    let name = p.layout_no_wrap(step.name.clone(), font.clone(), look.text);
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
}

#[cfg(test)]
mod tests {
    use crate::transcript::Step;

    #[test]
    fn a_count_in_the_output_is_read_and_paths_or_dates_are_not() {
        assert_eq!(super::counter("..............        14 / 23 ( 60%)"), Some((14, 23)));
        assert_eq!(super::counter("[3/10] compilando"), Some((3, 10)));
        assert_eq!(super::counter("src/1/2 2026/10/28 v1.2/3"), None);
        assert_eq!(super::counter("30 / 4"), None, "more done than the total: no count");
        assert_eq!(super::counter("sem contador"), None);
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
