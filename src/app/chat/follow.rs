//! A chat's scroll (the chat screen's, the mini chat's, a card's on the canvas) follows the newest, as egui's stick to
//! the bottom does. A run or a step clicked never moves it: held still while it eases open or shut, the row stays under
//! the pointer; stopped at the end that way, it follows the newest again as soon as something new comes in.

use crate::app::chat::accordion::UNFOLD_SECONDS;
use crate::prelude::*;
use crate::transcript::{ChatItem, Transcript};

/// What a view's scroll kept from its last frame.
#[derive(Clone, Copy, Default)]
struct Follow {
    /// How far down it was, and how far down it could go.
    offset: f32,
    end: f32,
    /// At the end: following the newest.
    at_end: bool,
    /// Taken off the end by a run or a step clicked there: what the conversation held then (`talk_mark`).
    paused: Option<(usize, usize)>,
}

/// A run or a step of chat `id` opened or shut this frame: its scrolls stay where they are while it eases (`scroll_held`).
pub(crate) fn hold_scroll(ctx: &Context, id: &str) {
    // read before: the context is locked inside `data_mut`
    let (frame, time) = (ctx.cumulative_frame_nr(), ctx.input(|i| i.time));

    ctx.data_mut(|d| d.insert_temp(held_key(id), (frame, time)));
}

/// In the frame after a run or a step of chat `id` opened or shut (`hold_scroll`), and while it eases (`UNFOLD_SECONDS`
/// and a frame or two).
pub(crate) fn scroll_held(ctx: &Context, id: &str) -> bool {
    let (now, frame_now) = (ctx.input(|i| i.time), ctx.cumulative_frame_nr());
    let easing = f64::from(UNFOLD_SECONDS) + 0.1;

    ctx.data(|d| d.get_temp::<(u64, f64)>(held_key(id))).is_some_and(|(frame, time)| frame + 1 >= frame_now || now - time <= easing)
}

fn held_key(id: &str) -> egui::Id {
    egui::Id::unique(("chat_card_held", id))
}

/// What a conversation holds, as its scroll sees it: how many items, and how long the last one is (the agent's words
/// growing, a command's output, its answer).
pub(crate) fn talk_mark(transcript: Option<&Transcript>) -> (usize, usize) {
    let Some(transcript) = transcript else { return (0, 0) };

    let last = match transcript.items.last() {
        Some(ChatItem::User(text) | ChatItem::Agent(text) | ChatItem::Error(text) | ChatItem::System(text)) => text.len(),
        Some(ChatItem::Tool(step)) => step.log.len() + step.output.as_ref().map_or(0, String::len) + usize::from(step.ended.is_some()),
        _ => 0,
    };

    (transcript.items.len(), last)
}

/// `scroll` showing `add`, chat `chat`'s conversation in `view` (the chat screen, the mini chat, a card): stuck to the
/// newest, but held still while a run or a step of it eases (`scroll_held`). Taken off the end that way, it stays where it
/// is until `mark` (`talk_mark`) changes, then goes back to the end and follows again; scrolled by hand meanwhile, it is
/// egui's again (back at the end, it follows).
// ponytail: shutting a step with nothing under it to scroll to moves the view (egui never scrolls past the end); a blank
// filler under the conversation, given back as it is scrolled up, would keep it
pub(crate) fn follow_scroll<R>(
    ui: &mut egui::Ui,
    scroll: egui::ScrollArea,
    chat: &str,
    view: &str,
    mark: (usize, usize),
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let ctx = ui.ctx().clone();
    let key = egui::Id::unique(("chat_follow", view, chat));
    let held = scroll_held(&ctx, chat);
    let mut follow = ctx.data(|d| d.get_temp::<Follow>(key)).unwrap_or_default();

    // at the end when it was clicked: to bring back
    if held && follow.at_end && follow.paused.is_none() {
        follow.paused = Some(mark);
    }

    let back = !held && follow.paused.is_some_and(|then| then != mark);
    let mut scroll = scroll.stick_to_bottom(!held);

    if back {
        // where the end was last frame: still there at this frame's end, egui sticks to it again (else again next frame)
        scroll = scroll.vertical_scroll_offset(follow.end);
        ctx.request_repaint();
    }

    let output = scroll.show(ui, add);
    let offset = output.state.clamped_offset().y;
    let end = output.content_size.y - output.inner_rect.height();
    let at_end = end <= 0.0 || offset >= end;
    // back at the end (egui follows), or scrolled by hand: nothing to bring back
    let scrolled = !held && !back && (offset - follow.offset).abs() > 0.5;

    if !held && (at_end || scrolled) {
        follow.paused = None;
    }

    ctx.data_mut(|d| d.insert_temp(key, Follow { offset, end, at_end, paused: follow.paused }));
    output.inner
}

#[cfg(test)]
mod tests {
    use crate::transcript::{ChatItem, Step, Transcript};

    #[test]
    fn the_mark_changes_with_a_new_item_and_with_more_of_the_last() {
        let mut transcript = Transcript::new(std::path::PathBuf::new());

        assert_eq!(super::talk_mark(None), (0, 0));
        transcript.items.push(ChatItem::Agent("Abrindo".into()));
        let first = super::talk_mark(Some(&transcript));

        transcript.items[0] = ChatItem::Agent("Abrindo o Google".into());
        let longer = super::talk_mark(Some(&transcript));

        transcript.items.push(ChatItem::Tool(Step { id: "a".into(), name: "Bash".into(), ..Default::default() }));
        let step = super::talk_mark(Some(&transcript));

        if let Some(ChatItem::Tool(running)) = transcript.items.last_mut() {
            running.log = "1 teste ok".into();
        }

        let printed = super::talk_mark(Some(&transcript));

        assert!(first != longer && longer != step && step != printed, "{first:?} {longer:?} {step:?} {printed:?}");
    }
}
