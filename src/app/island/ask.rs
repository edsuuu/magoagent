//! The card of a question or a permission on the open island: one card
//! with its wash (cyan asking, amber for a permission), the wizard inside it at the left, and at 116 the agent and the
//! permission's command with a row of round buttons, or the question whole with its options as rows (each label and what
//! it means, wrapped: asked "fica com os ... e eu tenho que passar o mouse em cima de cada opção") and "Outro", a box for
//! an answer of one's own; the card grows with them, and past the window's room they scroll.

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
/// The most a question's card grows to: the island's window always has 420 for it (`window::ROOM`).
const QUESTION_MOST: f32 = 420.0;
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
    /// The words of "Outro" sent.
    Other,
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
        if !approval {
            let who = (name, color, doing);
            let pressed = self.island_question(ui, &painter, page, ask, (left, width), who, outside.is_some(), out);
            let send = match pressed {
                None => return,
                Some(Press::Option(option)) => ask.pick(option),
                Some(Press::Other) => ask.answer_other(),
                Some(Press::Terminal) => {
                    if let Some((_, session)) = outside {
                        self.outside.declined.borrow_mut().push((session, now_ms()));
                    }
                    asks.remove(index).send_terminal();
                    return;
                }
                Some(Press::Done | Press::Allow(_)) => ask.next(),
            };
            if send {
                asks.remove(index).send(true);
            }
            return;
        }
        // the row's buttons, as `actions options`: wrapped, 6 apart (8 on a permission)
        let mut buttons: Vec<(String, Kind, Press, Option<String>, Option<&str>, bool)> = Vec::new();
        buttons.push(("Negar".into(), Kind::Secondary, Press::Allow(false), None, Some("N"), false));
        buttons.push(("Permitir".into(), Kind::Primary, Press::Allow(true), None, Some("Y"), false));
        let title = ask.summary();
        if outside.is_some() {
            buttons.push(("Responder no terminal".into(), Kind::Link, Press::Terminal, None, None, false));
        }
        let gap = 8.0;
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
        // the command in a one-line `.code` with "ver tudo" at its end, or opened: the whole request in a well that
        // scrolls, as tall as the card leaves
        let more = ui.painter().layout_no_wrap(if open { "recolher" } else { "ver tudo" }.to_string(), font(11.0), LINK);
        let text = clipped(ui.painter(), title, FontId::monospace(12.0), Color32::from_rgb(0xe8, 0xe9, 0xec), width - 20.0 - more.size().x - 10.0);
        let text_height = if open { (page.height() - 28.0 - 20.0 - 5.0 - rows_height).max(60.0) } else { text.size().y + 10.0 };
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
        {
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
        }
        y += text_height + 5.0;
        // the row; an approval's clicks in its first 600 ms don't count (`CLICK_GUARD_MS`): it came up under a pointer
        // busy elsewhere
        let guarded = ask.guarded(time);
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
        match pressed {
            Some(Press::Allow(allow)) => asks.remove(index).send(allow),
            Some(Press::Terminal) => {
                if let Some((_, session)) = outside {
                    self.outside.declined.borrow_mut().push((session, now_ms()));
                }
                asks.remove(index).send_terminal();
            }
            _ => {}
        }
    }
}

impl DrawingApp {
    /// A question's card from `top`: who asks, the question whole, its options as rows (the label and what it means,
    /// wrapped, the number key at the left) and "Outro"; under them OK (a multiple choice) and "Responder no terminal"
    /// (a session outside the app). Its height goes to `out.body`; what is taller than the window leaves scrolls.
    #[allow(clippy::too_many_arguments)]
    fn island_question(
        &self,
        ui: &mut egui::Ui,
        painter: &Painter,
        page: Rect,
        ask: &mut crate::app::chat::decide::PersonAsk,
        (left, width): (f32, f32),
        (name, color, doing): (Option<String>, Color32, String),
        outside: bool,
        out: &mut Out,
    ) -> Option<Press> {
        let step = ask.step.min(ask.questions.len() - 1);
        let question = ask.questions[step].clone();
        // `agentWho`: its dot, its name, what it does
        let top = page.top() + 14.0;
        let mut x = left;
        if let Some(name) = name {
            painter.circle_filled(Pos2::new(x + 4.0, top + 7.5), 4.0, color);
            x += 8.0 + 7.0;
            let galley = painter.layout_no_wrap(name, semi(12.0), INK);
            let right = x + galley.size().x;
            painter.galley(Pos2::new(x, top + 7.5 - galley.size().y / 2.0), galley, INK);
            x = right + 7.0;
        }
        let doing = clipped(painter, doing, font(12.0), DIM, (page.right() - 40.0 - x).max(0.0));
        painter.galley(Pos2::new(x, top + 7.5 - doing.size().y / 2.0), doing, DIM);
        // what the card holds, measured first: its height is the island's
        let text = ui.painter().layout(question.text.clone(), semi(15.0), INK, width);
        let hint = question.multiple.then(|| ui.painter().layout_no_wrap("Escolha uma ou mais".to_string(), font(11.0), DIM));
        let words = width - 12.0 - 26.0 - 12.0;
        let options: Vec<_> = question
            .options
            .iter()
            .map(|(label, meaning)| {
                let label = ui.painter().layout(label.clone(), semi(13.0), INK, words);
                let meaning = (!meaning.is_empty()).then(|| ui.painter().layout(meaning.clone(), font(11.5), DIM, words));
                let height = (9.0 + label.size().y + meaning.as_ref().map_or(0.0, |meaning| 2.0 + meaning.size().y) + 9.0).max(34.0);
                (label, meaning, height)
            })
            .collect();
        let other_id = egui::Id::unique(("island_ask_other", ask.chat.as_str(), step));
        let other_h = super::prompt::bar_height(ui, other_id);
        let gap = 6.0;
        let content = text.size().y
            + hint.as_ref().map_or(0.0, |hint| 4.0 + hint.size().y)
            + 10.0
            + options.iter().map(|(_, _, height)| height + gap).sum::<f32>()
            + other_h;
        let footer = question.multiple || outside;
        let footer_h = if footer { 30.0 + 8.0 } else { 0.0 };
        // the page's height (the island's less its header and margin), between the card it was and the window's room
        let needed = 14.0 + 15.0 + 10.0 + content + footer_h + 14.0;
        let margins = super::paint::PAGE_TOP + 10.0;
        out.body = needed.clamp(QUESTION_H - margins, QUESTION_MOST - margins);
        // the list, under who asks and over the footer, scrolling when it is taller than that
        let bottom = page.bottom() - 14.0 - footer_h;
        let area = Rect::from_min_max(Pos2::new(left, top + 15.0 + 10.0), Pos2::new(page.right() - 12.0, bottom.max(top + 40.0)));
        let mut pressed = None;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(area.intersect(ui.clip_rect()));
        child.spacing_mut().scroll = crate::app::ui::scroll_style();
        let scroll = egui::ScrollArea::vertical().id_salt(("island_ask_list", ask.chat.as_str(), step)).max_height(area.height()).auto_shrink([false, true]);
        scroll.show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, text.size().y), Sense::hover());
            ui.painter().galley(rect.min, text, INK);
            if let Some(hint) = hint {
                ui.add_space(4.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, hint.size().y), Sense::hover());
                ui.painter().galley(rect.min, hint, DIM);
            }
            ui.add_space(10.0);
            for (option, (label, meaning, height)) in options.into_iter().enumerate() {
                let on = ask.picked[step].contains(&option);
                let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                let painter = ui.painter();
                let ground = if on { CYAN.gamma_multiply(0.2) } else { Color32::from_white_alpha(if response.hovered() { 34 } else { 18 }) };
                let edge = if on { CYAN.gamma_multiply(0.8) } else { Color32::from_white_alpha(14) };
                painter.rect(rect, 10, ground, Stroke::new(1.0, edge), StrokeKind::Inside);
                // the key that picks it (1 to 9), ticked when picked in a multiple choice
                let key = Rect::from_center_size(Pos2::new(rect.left() + 12.0 + 9.0, rect.top() + 9.0 + label.rows[0].height() / 2.0), Vec2::splat(18.0));
                let mark = if question.multiple && on { "✓".to_string() } else { (option + 1).to_string() };
                painter.rect(key, 5, Color32::TRANSPARENT, Stroke::new(1.0, Color32::from_white_alpha(if on { 140 } else { 70 })), StrokeKind::Inside);
                painter.text(key.center(), egui::Align2::CENTER_CENTER, mark, font(10.5), INK.gamma_multiply(if on { 1.0 } else { 0.7 }));
                let x = rect.left() + 12.0 + 26.0;
                let below = rect.top() + 9.0 + label.size().y;
                painter.galley(Pos2::new(x, rect.top() + 9.0), label, INK);
                if let Some(meaning) = meaning {
                    painter.galley(Pos2::new(x, below + 2.0), meaning, DIM);
                }
                if response.clicked() {
                    pressed = Some(Press::Option(option));
                }
                ui.add_space(gap);
            }
            // "Outro": the answer in one's own words, sent by Enter or its round button
            let (bar, _) = ui.allocate_exact_size(Vec2::new(width, other_h), Sense::hover());
            let painter = ui.painter().clone();
            painter.rect(bar, 12, Color32::from_white_alpha(18), Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
            let send = Rect::from_center_size(Pos2::new(bar.right() - 8.0 - 12.0, bar.bottom() - 20.0), Vec2::splat(24.0));
            let field = Rect::from_min_max(Pos2::new(bar.left() + 12.0, bar.top() + 6.0), Pos2::new(send.left() - 8.0, bar.bottom() - 6.0));
            let hint = "Outro: escreva a sua resposta…";
            let (_, sent) = super::prompt::send_field(ui, &painter, (field, send), &mut ask.other[step], other_id, hint, "island_ask_other_send");
            if sent {
                pressed = Some(Press::Other);
            }
        });
        // the footer: OK for a multiple choice, the way to the terminal for a session outside the app
        if footer {
            let mut x = left;
            let y = page.bottom() - 14.0 - 30.0;
            if question.multiple {
                let off = ask.picked[step].is_empty() && ask.other[step].trim().is_empty();
                let size = button_size(ui, "OK", Kind::Primary, None);
                let rect = Rect::from_min_size(Pos2::new(x, y + (30.0 - size.y) / 2.0), size);
                let response = ui.interact(rect, ui.scope_id().with("island_ask_ok"), Sense::click());
                paint_button(painter, rect, "OK", Kind::Primary, None, off, &response);
                if response.clicked() && !off {
                    pressed = Some(Press::Done);
                }
                x = rect.right() + 8.0;
            }
            if outside {
                let label = "Responder no terminal";
                let size = button_size(ui, label, Kind::Link, None);
                let rect = Rect::from_min_size(Pos2::new(x, y + (30.0 - size.y) / 2.0), size);
                let response = ui.interact(rect, ui.scope_id().with("island_ask_terminal"), Sense::click());
                paint_button(painter, rect, label, Kind::Link, None, false, &response);
                if response.clicked() {
                    pressed = Some(Press::Terminal);
                }
            }
        }
        pressed
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
