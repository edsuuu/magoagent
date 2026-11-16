//! A session outside the app (Claude Code in a terminal, `maestro/outside.rs`) on the island's conversation page, where
//! its Abrir leads: what was said in it, read from the file Claude Code writes (`outside_transcript`) and followed while
//! it works, drawn as the compact conversation of a chat of the app (`prompt.rs`). It is only read here: the session
//! lives in its terminal, and a second Claude Code resuming it from the app would write the same history from two
//! places at once. So the box is "Responder no terminal", beside the way to the terminal's window.

use super::forms;
use super::paint::{CARD, DIM, DIM_3, INK, Out, View, aim, font, wash_with};
use super::prompt::{paint_log, said};
use crate::app::chat::list::clipped;
use crate::app::mascot::mago::MagoState;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use crate::transcript::Transcript;

/// How long a word about the terminal's window stays in the box.
const NOTICE_FOR: f64 = 5.0;

/// The session shown: its pill, its conversation as read so far (none where Claude Code keeps no file, as Hermes), when it
/// was last read, and what the way to its terminal answered.
pub(crate) struct SessionView {
    pub(crate) id: String,
    transcript: Option<Transcript>,
    polled: Option<f64>,
    notice: Option<(String, f64)>,
}

impl SessionView {
    pub(crate) fn new(id: &str) -> SessionView {
        SessionView { id: id.to_string(), transcript: None, polled: None, notice: None }
    }

    /// The words said in it, for the page's height.
    pub(crate) fn said(&self) -> usize {
        self.transcript.as_ref().map_or(0, |transcript| said(&transcript.items).count())
    }

    /// The terminal's window could not be brought: said in the box for a while.
    pub(crate) fn notice(&mut self, text: &str, time: f64) {
        self.notice = Some((text.to_string(), time + NOTICE_FOR));
    }
}

impl DrawingApp {
    /// Reads what the session shown said since the last time, every half second: the file is found once it is there.
    pub(crate) fn follow_session(&mut self, time: f64) {
        let Some(id) = self.island.session.as_ref().filter(|session| session.polled.is_none_or(|at| time - at >= 0.5)).map(|session| session.id.clone()) else {
            return;
        };
        let path = self.outside_transcript(&id);
        let Some(session) = self.island.session.as_mut() else { return };
        session.polled = Some(time);
        // the pill's session changed (a sibling in the same folder spoke last): its file instead
        if path.is_some() && session.transcript.as_ref().map(|transcript| transcript.path().to_path_buf()) != path {
            session.transcript = path.map(Transcript::new);
        }
        if let Some(transcript) = session.transcript.as_mut() {
            transcript.poll();
        }
    }

    /// The conversation page of a session outside: its log, its name and the box that sends to its terminal.
    pub(super) fn island_session(&mut self, ui: &mut egui::Ui, page: Rect, view: &View, time: f64, out: &mut Out) {
        self.follow_session(time);
        let Some(session) = self.island.session.as_ref() else { return };
        let line = view.lines.iter().find(|line| line.id == session.id);
        let state = line.map_or(AgentState::Ended, |line| line.state);
        let painter = ui.painter().with_clip_rect(page);
        painter.rect(page, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        wash_with(&painter, page, Color32::from_rgb(0x63, 0x66, 0xf1), 0.5, 0.0);
        // the wizard where the app's conversation has it (`island_prompt`)
        let origin = view.origin.get();
        let height = page.bottom() + 10.0 - origin.y;
        aim(view, Pos2::new(origin.x + 52.0, origin.y + 42.0 + (height - 42.0 - 84.0) / 2.0 + 42.0), 44.0, MagoState::from_agent(state, time), 40.0);
        let body = Rect::from_min_max(Pos2::new(page.left() + 84.0, page.top() + 12.0), Pos2::new(page.right() - 16.0, page.bottom() - 14.0));
        let bar = Rect::from_min_max(Pos2::new(body.left(), body.bottom() - 40.0), body.max);
        let chip_row = Rect::from_min_max(Pos2::new(body.left(), bar.top() - 6.0 - 22.0), Pos2::new(body.right() - 10.0, bar.top() - 6.0));
        let log = Rect::from_min_max(Pos2::new(body.left(), body.top()), Pos2::new(body.right() - 22.0, chip_row.top() - 6.0));
        match &session.transcript {
            Some(transcript) => {
                let messages: Vec<(String, bool)> = said(&transcript.items).map(|(text, user)| (text.to_string(), user)).collect();
                paint_log(ui, log, &messages, matches!(state, AgentState::Working(_)), time);
            }
            None => {
                let words = "A conversa desta sessão não pode ser lida daqui.";
                painter.text(log.center(), egui::Align2::CENTER_CENTER, words, font(12.0), DIM);
            }
        }
        // whose it is, at the right over the box, as the app's chat shows its agent and model
        let name = line.map_or("Sessão encerrada".to_string(), |line| format!("{} · {}", line.name, state.label()));
        let galley = clipped(ui.painter(), name, font(10.5), Color32::from_rgb(0x7b, 0x80, 0x89), chip_row.width() - 40.0);
        let chip = Rect::from_min_size(Pos2::new(chip_row.right() - galley.size().x - 27.0, chip_row.top() + 2.0), Vec2::new(galley.size().x + 27.0, 20.0));
        painter.rect_filled(chip, 10, Color32::from_white_alpha(15));
        painter.circle_filled(Pos2::new(chip.left() + 11.0, chip.center().y), 3.0, line.map_or(DIM_3, |line| line.color));
        painter.galley(Pos2::new(chip.left() + 19.0, chip.center().y - galley.size().y / 2.0), galley, Color32::from_rgb(0x7b, 0x80, 0x89));
        // the box: the answer goes in the terminal, and the way there
        painter.rect_filled(bar, 12, Color32::from_white_alpha(18));
        let width = ui.painter().layout_no_wrap("Ir ao terminal".to_string(), font(11.5), INK).size().x + 22.0;
        let go = forms::chip(ui, Pos2::new(bar.right() - 7.0 - width, bar.center().y - 13.0), "Ir ao terminal", false);
        if go.clicked() {
            out.terminal = Some(session.id.clone());
        }
        let notice = session.notice.as_ref().filter(|(_, until)| time < *until).map(|(text, _)| text.as_str());
        let (words, color) = notice.map_or(("Responder no terminal", DIM_3), |text| (text, INK));
        let galley = clipped(ui.painter(), words.to_string(), font(12.5), color, (go.rect.left() - bar.left() - 20.0).max(0.0));
        painter.galley(Pos2::new(bar.left() + 12.0, bar.center().y - galley.size().y / 2.0), galley, color);
        if notice.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}
