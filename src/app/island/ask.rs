//! The card of a question or a permission on the open island: one card
//! with its wash (cyan asking, amber for a permission), the wizard inside it at the left, and at 116 the agent, the
//! question (or the command) and a row of round buttons.

use super::paint::{AMBER, BLACK, CARD, DIM, INK, Out, View, aim, font, semi, wash_with};
use crate::app::chat::list::clipped;
use crate::app::maestro::outside::ASK_PREFIX;
use crate::app::mascot::mago::MagoState;
use crate::prelude::*;

/// The island's height on a question with options to pick (`QUESTION_PICKER_H`) and on a permission (`approval`).
pub(super) const QUESTION_H: f32 = 200.0;
pub(super) const APPROVAL_H: f32 = 160.0;
/// A permission opened to read the whole request (`PersonAsk::details`).
pub(super) const APPROVAL_OPEN_H: f32 = 380.0;
/// "Responder no terminal" and "ver tudo": light enough to read on the amber wash.
const LINK: Color32 = Color32::from_rgb(0xd4, 0xd7, 0xdd);
const CYAN: Color32 = Color32::from_rgb(0x22, 0xd3, 0xee);

/// What a button of the row does.
#[derive(Clone, Copy)]
enum Press {
    Option(usize),
    Done,
    Allow(bool),
    Terminal,
}

/// A button of the row: `.btn.primary` (ink on white), `.btn.secondary` (9% white) or the gray `.link-btn`.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Primary,
    Secondary,
    Link,
}

impl DrawingApp {
    /// The card of the first ask waiting (`view.asks`); its height goes to `out.body`.
    pub(super) fn island_ask(&self, ui: &mut egui::Ui, page: Rect, view: &View, time: f64, out: &mut Out) {
        let Some(chat) = view.asks.first() else { return };
        // who asks, before the asks are borrowed: its colour
        let color = self.ask_line(view).map(|line| line.color);
        let mut asks = self.person_asks.borrow_mut();
        // a chat that no longer runs gave its ask up
        if !self.chat_runs.contains_key(chat) && !chat.starts_with(ASK_PREFIX) {
            asks.retain(|ask| ask.chat != *chat);
            return;
        }
        let Some(index) = asks.iter().position(|ask| ask.chat == *chat) else { return };
        let ask = &mut asks[index];
        let approval = ask.questions.is_empty();
        let open = approval && ask.open;
        out.body = (if open {
            APPROVAL_OPEN_H
        } else if approval {
            APPROVAL_H
        } else {
            QUESTION_H
        }) - super::paint::PAGE_TOP
            - 10.0;
        let (wash, strength, state) = if approval { (AMBER, 0.42, MagoState::NeedsYou) } else { (CYAN, 0.38, MagoState::Asking) };

        // `.card.wash`: radius 20, the hairline, the wash rising from under its bottom
        let painter = ui.painter().with_clip_rect(page);
        painter.rect(page, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        wash_with(&painter, page, wash, strength, 0.0);
        // botPosition with no botY: the middle of an 84 pt card under the header, at 62 from the island's left
        let origin = view.origin.get();
        let height = page.bottom() + 10.0 - origin.y;
        aim(view, Pos2::new(origin.x + 62.0, origin.y + 42.0 + (height - 42.0 - 84.0) / 2.0 + 42.0), 56.0, state);

        // `foldButton`: "Depois — deixar esperando", the island folds and the ask keeps waiting
        let fold = Rect::from_center_size(Pos2::new(page.right() - 18.0, page.top() + 16.0), Vec2::splat(16.0));
        let response = ui.interact(fold, ui.scope_id().with("island_ask_fold"), Sense::click()).on_hover_text("Depois — deixar esperando");
        painter.circle_filled(fold.center(), 8.0, Color32::from_white_alpha(18));
        let chevron = [fold.center() + Vec2::new(-2.6, 1.3), fold.center() + Vec2::new(0.0, -1.3), fold.center() + Vec2::new(2.6, 1.3)];
        painter.add(egui::Shape::line(chevron.to_vec(), Stroke::new(1.6, Color32::from_rgb(0x5f, 0x64, 0x6d))));
        out.close |= response.clicked();
        // the asks waiting behind this one
        if view.queued > 0 {
            painter.text(Pos2::new(fold.left() - 8.0, fold.center().y), egui::Align2::RIGHT_CENTER, format!("+{} na fila", view.queued), font(11.0), DIM);
        }

        // `agentWho`: its dot, its name, what it does
        let outside = ask.source.as_ref().map(|source| (source.label.clone(), source.session.clone()));
        let name = outside.as_ref().map(|(label, _)| label.clone()).or_else(|| self.chats.iter().find(|item| item.id == *chat).map(|item| item.name.clone()));
        let color = color.unwrap_or(wash);
        let doing = match ask.questions.len() {
            0 => format!("precisa de permissão · {}", ask.tool_label()),
            1 => "está perguntando".to_string(),
            total => format!("está perguntando ({} de {total})", ask.step.min(total - 1) + 1),
        };
        let left = page.left() + 116.0;
        let width = page.right() - 16.0 - left;
        // the row's buttons, as `actions options`: wrapped, 6 apart (8 on a permission)
        let mut buttons: Vec<(String, Kind, Press, Option<String>, Option<&str>, bool)> = Vec::new();
        let title = if approval {
            buttons.push(("Negar".into(), Kind::Secondary, Press::Allow(false), None, Some("N"), false));
            buttons.push(("Permitir".into(), Kind::Primary, Press::Allow(true), None, Some("Y"), false));
            ask.summary()
        } else {
            let step = ask.step.min(ask.questions.len() - 1);
            let question = &ask.questions[step];
            for (option, (label, meaning)) in question.options.iter().enumerate() {
                let on = ask.picked[step].contains(&option);
                let kind = if on { Kind::Primary } else { Kind::Secondary };
                buttons.push((label.clone(), kind, Press::Option(option), (!meaning.is_empty()).then(|| meaning.clone()), None, false));
            }
            if question.multiple {
                buttons.push(("OK".into(), Kind::Primary, Press::Done, None, None, ask.picked[step].is_empty()));
            }
            question.text.clone()
        };
        if outside.is_some() {
            buttons.push(("Responder no terminal".into(), Kind::Link, Press::Terminal, None, None, false));
        }
        let gap = if approval { 8.0 } else { 6.0 };
        let sizes: Vec<Vec2> = buttons.iter().map(|(label, kind, _, _, key, _)| button_size(ui, label, *kind, *key)).collect();
        let mut rows: Vec<Vec<usize>> = vec![Vec::new()];
        let mut used = 0.0;
        for (index, size) in sizes.iter().enumerate() {
            let row = rows.last_mut().expect("one row at least");
            if !row.is_empty() && used + gap + size.x > width {
                rows.push(vec![index]);
                used = size.x;
            } else {
                used += if row.is_empty() { size.x } else { gap + size.x };
                row.push(index);
            }
        }
        let row_height = |row: &Vec<usize>| row.iter().map(|index| sizes[*index].y).fold(0.0, f32::max);
        let rows_height = rows.iter().map(row_height).sum::<f32>() + gap * (rows.len() - 1) as f32;
        // the question in 600 15px, two lines at most (`.question-text`); the command in a one-line `.code` with "ver tudo"
        // at its end, or opened: the whole request in a well that scrolls, as tall as the card leaves
        let more = ui.painter().layout_no_wrap(if open { "recolher" } else { "ver tudo" }.to_string(), font(11.0), LINK);
        let text = if approval {
            clipped(ui.painter(), title.clone(), FontId::monospace(12.0), Color32::from_rgb(0xe8, 0xe9, 0xec), width - 20.0 - more.size().x - 10.0)
        } else {
            let mut job = egui::text::LayoutJob::simple(title.clone(), semi(15.0), INK, width);
            job.wrap.max_rows = 2;
            job.wrap.overflow_character = Some('…');
            ui.painter().layout_job(job)
        };
        let text_height = if open {
            (page.height() - 28.0 - 20.0 - 5.0 - rows_height).max(60.0)
        } else if approval {
            text.size().y + 10.0
        } else {
            text.size().y
        };
        // `.stack`: the three centred down the card, 5 apart
        let total = 15.0 + 5.0 + text_height + 5.0 + rows_height;
        let mut y = page.center().y - total / 2.0;
        let mut x = left;
        if name.is_some() {
            painter.circle_filled(Pos2::new(x + 4.0, y + 7.5), 4.0, color);
            x += 8.0 + 7.0;
            let galley = painter.layout_no_wrap(name.clone().unwrap_or_default(), semi(12.0), INK);
            let right = x + galley.size().x;
            painter.galley(Pos2::new(x, y + 7.5 - galley.size().y / 2.0), galley, INK);
            x = right + 7.0;
        }
        let doing = clipped(&painter, doing, font(12.0), DIM, (page.right() - 40.0 - x).max(0.0));
        painter.galley(Pos2::new(x, y + 7.5 - doing.size().y / 2.0), doing, DIM);
        y += 15.0 + 5.0;
        if approval {
            let well_width = if open { width } else { text.size().x + 20.0 + more.size().x + 10.0 };
            let well = Rect::from_min_size(Pos2::new(left, y), Vec2::new(well_width, text_height));
            painter.rect(well, 10, Color32::from_white_alpha(18), Stroke::new(1.0, Color32::from_white_alpha(15)), StrokeKind::Inside);
            // "ver tudo" / "recolher", at the right of the line (the top of the opened well)
            let toggle = Rect::from_min_size(Pos2::new(well.right() - 10.0 - more.size().x, y + 5.0), more.size());
            let hit = if open { toggle.expand(4.0) } else { well };
            let response = ui.interact(hit, ui.scope_id().with("island_ask_more"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let ink = if response.hovered() { INK } else { LINK };
            if response.hovered() {
                painter.hline(toggle.x_range(), toggle.bottom(), Stroke::new(1.0, ink));
            }
            painter.galley(toggle.min, more, ink);
            if open {
                // the request, field by field, selectable to copy a piece of it
                let inner = Rect::from_min_max(Pos2::new(well.left() + 10.0, toggle.bottom() + 6.0), well.max - Vec2::new(4.0, 6.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
                egui::ScrollArea::vertical().id_salt(("island_ask_details", index)).auto_shrink([false, false]).show(&mut child, |ui| {
                    let details = egui::RichText::new(ask.details()).font(FontId::monospace(12.0)).color(Color32::from_rgb(0xe8, 0xe9, 0xec));
                    ui.add(egui::Label::new(details).wrap());
                });
            } else {
                painter.galley(Pos2::new(left + 10.0, y + 5.0), text, Color32::from_rgb(0xe8, 0xe9, 0xec));
            }
            if response.clicked() {
                ask.open = !open;
            }
        } else {
            let area = Rect::from_min_size(Pos2::new(left, y), text.size());
            painter.galley(area.min, text, INK);
            ui.interact(area, ui.scope_id().with("island_ask_title"), Sense::hover()).on_hover_text(&title);
        }
        y += text_height + 5.0;
        // the row; an approval's clicks in its first 600 ms don't count (`CLICK_GUARD_MS`): it came up under a pointer
        // busy elsewhere
        let guarded = approval && ask.guarded(time);
        let mut pressed = None;
        for row in &rows {
            let height = row_height(row);
            let mut x = left;
            for index in row {
                let (label, kind, press, tip, key, off) = &buttons[*index];
                let rect = Rect::from_min_size(Pos2::new(x, y + (height - sizes[*index].y) / 2.0), sizes[*index]);
                x = rect.right() + gap;
                let mut response = ui.interact(rect, ui.scope_id().with(("island_ask_button", *index)), Sense::click());
                if let Some(tip) = tip {
                    response = response.on_hover_text(tip);
                }
                paint_button(&painter, rect, label, *kind, *key, *off, &response);
                if response.clicked() && !off {
                    pressed = Some(*press);
                }
            }
            y += height + gap;
        }
        if guarded {
            return;
        }
        let send = match pressed {
            None => return,
            Some(Press::Option(option)) => ask.pick(option).then_some(true),
            Some(Press::Done) => ask.next().then_some(true),
            Some(Press::Allow(allow)) => Some(allow),
            Some(Press::Terminal) => {
                if let Some((_, session)) = outside {
                    self.outside.declined.borrow_mut().push((session, now_ms()));
                }
                asks.remove(index).send_terminal();
                return;
            }
        };
        if let Some(allow) = send {
            asks.remove(index).send(allow);
        }
    }
}

/// `.btn`: 500 12.5px, padding 7 13, a `.kbd` 7 after the label; `.link-btn`: 500 11px.
fn button_size(ui: &egui::Ui, label: &str, kind: Kind, key: Option<&str>) -> Vec2 {
    if kind == Kind::Link {
        let galley = ui.painter().layout_no_wrap(label.to_string(), font(11.0), DIM);
        return Vec2::new(galley.size().x + 12.0, galley.size().y + 2.0);
    }
    let galley = ui.painter().layout_no_wrap(label.to_string(), font(12.5), INK);
    let key = key.map_or(0.0, |key| ui.painter().layout_no_wrap(key.to_string(), font(10.5), INK).size().x + 8.0 + 2.0 + 7.0);
    Vec2::new(galley.size().x + key + 26.0, galley.size().y + 14.0)
}

fn paint_button(painter: &Painter, rect: Rect, label: &str, kind: Kind, key: Option<&str>, off: bool, response: &egui::Response) {
    let response = response.clone().on_hover_cursor(CursorIcon::PointingHand);
    let fade = if off { 0.4 } else { 1.0 };
    if kind == Kind::Link {
        let ink = if response.hovered() { INK } else { LINK };
        let text = painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font(11.0), ink.gamma_multiply(fade));
        if response.hovered() {
            painter.hline(text.x_range(), text.bottom(), Stroke::new(1.0, ink));
        }
        return;
    }
    // `:active` scale .94
    let rect = if response.is_pointer_button_down_on() { Rect::from_center_size(rect.center(), rect.size() * 0.94) } else { rect };
    let (ground, ink) = match kind {
        Kind::Primary => (INK, BLACK),
        _ => (Color32::from_white_alpha(if response.hovered() { 38 } else { 23 }), Color32::from_rgb(0xf1, 0xf2, 0xf4)),
    };
    painter.rect_filled(rect, rect.height() / 2.0, ground.gamma_multiply(fade));
    let galley = painter.layout_no_wrap(label.to_string(), font(12.5), ink);
    let key = key.map(|key| painter.layout_no_wrap(key.to_string(), font(10.5), ink));
    let width = galley.size().x + key.as_ref().map_or(0.0, |key| key.size().x + 8.0 + 2.0 + 7.0);
    let mut x = rect.center().x - width / 2.0;
    let right = x + galley.size().x;
    painter.galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, ink.gamma_multiply(fade));
    if let Some(key) = key {
        x = right + 7.0;
        // `.kbd`: 10.5px, padding 0 4, radius 4, at 55%, its border black on the white button and white on the gray one
        let boxed = Rect::from_min_size(Pos2::new(x, rect.center().y - key.size().y / 2.0), key.size() + Vec2::new(10.0, 0.0));
        let edge = if kind == Kind::Primary { Color32::from_black_alpha(102) } else { Color32::from_white_alpha(102) };
        painter.rect(boxed, 4, Color32::TRANSPARENT, Stroke::new(1.0, edge.gamma_multiply(0.55 * fade)), StrokeKind::Inside);
        painter.galley(Pos2::new(x + 5.0, boxed.top()), key, ink.gamma_multiply(0.55 * fade));
    }
}
