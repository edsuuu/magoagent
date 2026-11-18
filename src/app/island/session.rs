//! A session outside the app (Claude Code in a terminal, `maestro/outside.rs`) on the island's conversation page, where
//! its Abrir leads: what was said in it, read from the file Claude Code writes (`outside_transcript`) and followed while
//! it works, drawn as the compact conversation of a chat of the app (`prompt.rs`). The session lives in its terminal (a
//! second Claude Code resuming it from the app would write the same history from two places at once), so the box sends
//! there (`maestro/message.rs`): typed in its tmux pane at once, or queued for the end of its turn. Claude Desktop,
//! Cursor and Hermes are answered in their own windows: their box is "Responder no terminal". The way to the terminal's
//! window is at the right of the box.

use super::forms;
use super::log::said;
use super::paint::{CARD, DIM, DIM_3, INK, Out, View, aim, font};
use super::prompt::{bar_height, chat_body, send_field};
use crate::app::chat::list::clipped;
use crate::app::mascot::mago::MagoState;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use crate::transcript::{ChatItem, Transcript};

/// How long a word about the terminal's window, or about the message sent, stays in the box.
const NOTICE_FOR: f64 = 5.0;
/// The messages waiting for the session's turn to end: the palette's orange.
const QUEUED: Color32 = Color32::from_rgb(0xf2, 0x9b, 0x38);

/// The session shown: its pill, its conversation as read so far (none where Claude Code keeps no file, as Hermes), when it
/// was last read, what the way to its terminal or the last message answered, and the message being written.
pub(crate) struct SessionView {
    pub(crate) id: String,
    transcript: Option<Transcript>,
    polled: Option<f64>,
    notice: Option<(String, f64)>,
    draft: String,
}

impl SessionView {
    pub(crate) fn new(id: &str) -> SessionView {
        SessionView { id: id.to_string(), transcript: None, polled: None, notice: None, draft: String::new() }
    }

    /// The words said in it, for the page's height.
    pub(crate) fn said(&self) -> usize {
        self.transcript.as_ref().map_or(0, |transcript| said(&transcript.items).count())
    }

    /// A word for the box (the terminal's window could not be brought, how the message went): said there for a while.
    pub(crate) fn notice(&mut self, text: &str, time: f64) {
        self.notice = Some((text.to_string(), time + NOTICE_FOR));
    }
}

impl DrawingApp {
    /// Reads what the session shown said since the last time, every half second: the file is found once it is there.
    /// Meanwhile its pill stays (`outside_shown`), though its turn ended: the person may still write to it.
    pub(crate) fn follow_session(&mut self, time: f64) {
        let Some(id) = self.island.session.as_ref().filter(|session| session.polled.is_none_or(|at| time - at >= 0.5)).map(|session| session.id.clone()) else {
            return;
        };
        self.outside_shown(&id);
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
        let Some(id) = self.island.session.as_ref().map(|session| session.id.clone()) else { return };
        let (writable, pending, unread) = (self.outside_takes_messages(&id), self.outside_pending(&id), self.outside_unread(&id));
        let Some(session) = self.island.session.as_ref() else { return };
        let line = view.lines.iter().find(|line| line.id == session.id);
        let state = line.map_or(AgentState::Ended, |line| line.state);
        let painter = ui.painter().with_clip_rect(page);
        painter.rect(page, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        // the wizard where the app's conversation has it (`island_prompt`)
        let origin = view.origin.get();
        let height = page.bottom() + 10.0 - origin.y;
        aim(view, Pos2::new(origin.x + 52.0, origin.y + 42.0 + (height - 42.0 - 84.0) / 2.0 + 42.0), 44.0, MagoState::from_agent(state, time));
        let (log, chip_row, bar) = chat_body(page, bar_height(ui, session_draft_id(&session.id)));
        match &session.transcript {
            Some(transcript) => {
                let messages: Vec<&ChatItem> = said(&transcript.items).collect();
                out.picture = self.island_log(ui, log, &messages, matches!(state, AgentState::Working(_)), time).or(out.picture.take());
            }
            None => {
                painter.text(log.center(), egui::Align2::CENTER_CENTER, unread, font(12.0), DIM);
            }
        }
        // whose it is, at the right over the box, as the app's chat shows its agent and model
        let name = line.map_or("Sessão encerrada".to_string(), |line| format!("{} · {}", line.name, state.label()));
        let galley = clipped(ui.painter(), name, font(10.5), Color32::from_rgb(0x7b, 0x80, 0x89), chip_row.width() - 40.0);
        let chip = Rect::from_min_size(Pos2::new(chip_row.right() - galley.size().x - 27.0, chip_row.top() + 2.0), Vec2::new(galley.size().x + 27.0, 20.0));
        painter.rect_filled(chip, 10, Color32::from_white_alpha(15));
        painter.circle_filled(Pos2::new(chip.left() + 11.0, chip.center().y), 3.0, line.map_or(DIM_3, |line| line.color));
        painter.galley(Pos2::new(chip.left() + 19.0, chip.center().y - galley.size().y / 2.0), galley, Color32::from_rgb(0x7b, 0x80, 0x89));
        // what was sent and waits for the end of its turn, beside its name
        let mut free = chip.left() - 8.0;
        if pending > 0 {
            let galley = ui.painter().layout_no_wrap(format!("{pending} na fila"), font(10.5), QUEUED);
            let tag = Rect::from_min_size(Pos2::new(chip.left() - 6.0 - galley.size().x - 16.0, chip.top()), Vec2::new(galley.size().x + 16.0, 20.0));
            painter.rect_filled(tag, 10, QUEUED.gamma_multiply(0.16));
            painter.galley(Pos2::new(tag.left() + 8.0, tag.center().y - galley.size().y / 2.0), galley, QUEUED);
            free = tag.left() - 8.0;
        }
        // the box, and the way to the terminal at its right
        painter.rect_filled(bar, 12, Color32::from_white_alpha(18));
        let width = ui.painter().layout_no_wrap("Ir ao terminal".to_string(), font(11.5), INK).size().x + 22.0;
        let go = forms::chip(ui, Pos2::new(bar.right() - 7.0 - width, bar.bottom() - 20.0 - 13.0), "Ir ao terminal", false);
        if go.clicked() {
            out.terminal = Some(session.id.clone());
        }
        let notice = session.notice.as_ref().filter(|(_, until)| time < *until).map(|(text, _)| text.clone());
        if notice.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
        if writable {
            // with words in the box (a message that did not go), the notice goes over it, at the left of the name
            if !session.draft.is_empty()
                && let Some(text) = &notice
            {
                let galley = clipped(ui.painter(), text.clone(), font(11.0), INK, (free - chip_row.left()).max(10.0));
                painter.galley(Pos2::new(chip_row.left() + 4.0, chip.center().y - galley.size().y / 2.0), galley, INK);
            }
            self.island_session_box(ui, &painter, bar, go.rect.left(), &id, notice, time);
            return;
        }
        let (words, color) = notice.as_deref().map_or(("Responder no terminal", DIM_3), |text| (text, INK));
        let galley = clipped(ui.painter(), words.to_string(), font(12.5), color, (go.rect.left() - bar.left() - 20.0).max(0.0));
        painter.galley(Pos2::new(bar.left() + 12.0, bar.center().y - galley.size().y / 2.0), galley, color);
    }

    /// The box of a Claude Code session, up to `right`: the field and the round send button (`island_prompt`'s). Enter or
    /// the button sends; the box says how it went (`Delivery::notice`) while it is empty.
    fn island_session_box(&mut self, ui: &mut egui::Ui, painter: &Painter, bar: Rect, right: f32, id: &str, notice: Option<String>, time: f64) {
        // the round button stays at the box's last line as the words grow
        let send_rect = Rect::from_center_size(Pos2::new(right - 8.0 - 14.0, bar.bottom() - 20.0), Vec2::splat(28.0));
        let field = Rect::from_min_max(Pos2::new(bar.left() + 10.0, bar.top() + 6.0), Pos2::new(send_rect.left() - 8.0, bar.bottom() - 6.0));
        // the island still opening: no room for the box yet
        if field.width() < 24.0 {
            return;
        }
        let Some(session) = self.island.session.as_mut() else { return };
        let mut draft = std::mem::take(&mut session.draft);
        let notice = notice.filter(|_| draft.is_empty());
        // in two lines at most: the longest says when the message goes
        if let Some(text) = &notice {
            let mut job = egui::text::LayoutJob::simple(text.clone(), font(12.0), INK, (field.width() - 6.0).max(10.0));
            (job.wrap.max_rows, job.wrap.overflow_character) = (2, Some('…'));
            let galley = ui.painter().layout_job(job);
            painter.galley(Pos2::new(field.left() + 4.0, field.center().y - galley.size().y / 2.0), galley, INK);
        }
        let hint = if notice.is_some() { "" } else { "Mensagem para esta sessão…" };
        let (edit, sent) = send_field(ui, painter, (field, send_rect), &mut draft, session_draft_id(id), hint, "island_session_send");
        if sent {
            // a message that did not go stays in the box, with the reason over it
            let words = match self.outside_message(id, &draft) {
                Ok(delivery) => {
                    draft.clear();
                    delivery.notice().to_string()
                }
                Err(why) => why,
            };
            if let Some(session) = self.island.session.as_mut() {
                session.notice(&words, time);
            }
            edit.request_focus();
        }
        if let Some(session) = self.island.session.as_mut() {
            session.draft = draft;
        }
    }
}

/// The id of the box of session `id` (its field, and the height its words keep for `bar_height`).
fn session_draft_id(id: &str) -> egui::Id {
    egui::Id::unique(("island_session_draft", id))
}
