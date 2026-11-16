//! The island's conversation log, the app's chat and a session outside alike: the person's words in bubbles at the
//! right with the pictures they pasted, the answers as the light Markdown the agents write (headings, lists, quotes,
//! code, **bold**, `code`), and the dots of an agent at work. It sticks to the bottom as it grows.

use super::paint::{INK, font, semi};
use crate::app::chat::image_key;
use crate::app::chat::images::thumb;
use crate::prelude::*;
use crate::render::markdown::{Line, Mark, Span, inline, lines};
use crate::transcript::ChatItem;
use egui::text::{LayoutJob, TextFormat};

pub(super) const REPLY: Color32 = Color32::from_rgb(0xb0, 0xb5, 0xbe);
const QUOTE: Color32 = Color32::from_rgb(0x8b, 0x90, 0x99);
const LINK: Color32 = Color32::from_rgb(0x7d, 0xd3, 0xfc);
/// The type, the room between two messages and between the lines of one.
const SIZE: f32 = 12.5;
const MESSAGE_GAP: f32 = 14.0;
const LINE_HEIGHT: f32 = 18.0;
/// Room kept at the right for the scroll bar, so it never runs over the words.
const SCROLL_GUTTER: f32 = 14.0;
/// A pasted picture is at most this big in the log.
const PICTURE: Vec2 = Vec2::new(220.0, 140.0);

/// What the log shows of a conversation: the words said and the pictures the person sent.
pub(super) fn said(items: &[ChatItem]) -> impl Iterator<Item = &ChatItem> {
    items.iter().filter(|item| matches!(item, ChatItem::User(_) | ChatItem::Agent(_) | ChatItem::Image { user: true, .. }))
}

impl DrawingApp {
    /// The log of `items` (from `said`) in `log`.
    pub(super) fn island_log(&self, ui: &mut egui::Ui, log: Rect, items: &[&ChatItem], working: bool, time: f64) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(log), |ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
            egui::ScrollArea::vertical().id_salt("island_prompt_log").stick_to_bottom(true).auto_shrink(false).show(ui, |ui| {
                let width = log.width() - SCROLL_GUTTER;
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = MESSAGE_GAP;
                for item in items {
                    match item {
                        ChatItem::User(text) => bubble(ui, text, width),
                        ChatItem::Agent(text) => reply(ui, text, width),
                        ChatItem::Image { data, .. } => self.island_picture(ui, data, width),
                        _ => {}
                    }
                }
                // the agent at work: its dots where the answer will come
                if working {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 16.0), Sense::hover());
                    for dot in 0..3 {
                        let lift = ((time * 6.0 - dot as f64).sin() * 0.5 + 0.5) as f32;
                        let at = Pos2::new(rect.left() + 4.0 + dot as f32 * 9.0, rect.center().y - lift * 2.0);
                        ui.painter().circle_filled(at, 2.5, REPLY.gamma_multiply(0.5 + lift * 0.5));
                    }
                }
            });
        });
    }

    /// A picture the person pasted, at the right as their words, fitted in `PICTURE` (the chat's thumbnails, decoded once).
    fn island_picture(&self, ui: &mut egui::Ui, data: &str, width: f32) {
        let pic = self.chat_images.borrow_mut().entry(image_key(data)).or_insert_with(|| thumb(ui.ctx(), data)).clone();
        let size = match pic.get() {
            Some(None) => return,
            Some(Some(pic)) => fit(pic.size, PICTURE),
            None => PICTURE,
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, size.y), Sense::hover());
        let frame = Rect::from_min_size(Pos2::new(rect.right() - size.x, rect.top()), size);
        match pic.get().and_then(Option::as_ref) {
            Some(pic) => {
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                ui.painter().add(egui::epaint::RectShape::filled(frame, 10, Color32::WHITE).with_texture(pic.texture.id(), uv));
            }
            None => {
                ui.painter().rect_filled(frame, 10, Color32::from_white_alpha(14));
            }
        }
        ui.painter().rect_stroke(frame, 10, Stroke::new(1.0, Color32::from_white_alpha(22)), StrokeKind::Inside);
    }
}

/// `size` (pixels) shrunk to fit `most`, its shape kept.
fn fit(size: [u32; 2], most: Vec2) -> Vec2 {
    let size = Vec2::new(size[0].max(1) as f32, size[1].max(1) as f32);
    size * (most.x / size.x).min(most.y / size.y).min(1.0)
}

/// The person's words: `.chat-row.user`, at the right, 32 in from the left; `.bubble`: 13% white, radius 12.
fn bubble(ui: &mut egui::Ui, text: &str, width: f32) {
    let text = unmarked(text);
    if text.is_empty() {
        return;
    }
    let format = TextFormat { font_id: font(SIZE), color: INK, line_height: Some(LINE_HEIGHT), ..Default::default() };
    let mut job = LayoutJob::single_section(text, format);
    job.wrap.max_width = width - 32.0 - 20.0;
    let galley = ui.painter().layout_job(job);
    let size = galley.size() + Vec2::new(20.0, 14.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, size.y), Sense::hover());
    let bubble = Rect::from_min_size(Pos2::new(rect.right() - size.x, rect.top()), size);
    ui.painter().rect_filled(bubble, 12, Color32::from_white_alpha(33));
    ui.painter().galley(bubble.min + Vec2::new(10.0, 7.0), galley, INK);
}

/// The person's words without Claude Code's "[Image #3]" marks: the picture shows right after them.
fn unmarked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("[Image #") {
        let after = &rest[start + "[Image #".len()..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && after[digits..].starts_with(']') {
            out.push_str(&rest[..start]);
            rest = after[digits + 1..].strip_prefix(' ').unwrap_or(&after[digits + 1..]);
        } else {
            out.push_str(&rest[..start + 1]);
            rest = &rest[start + 1..];
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// One block of an answer, as `reply` lays them out.
#[derive(Debug, PartialEq)]
enum Block<'a> {
    Words(Line<'a>),
    /// The lines between two fences, together.
    Code(Vec<&'a str>),
    /// Blank lines between two blocks: a little more room.
    Gap,
}

/// The answer's lines as blocks: a fence's lines in one card, a run of blank lines one gap (none at the edges).
fn blocks(text: &str) -> Vec<Block<'_>> {
    let mut out: Vec<Block> = Vec::new();
    for line in lines(text) {
        match (line, out.last_mut()) {
            (Line::Code(code), Some(Block::Code(run))) => run.push(code),
            (Line::Code(code), _) => out.push(Block::Code(vec![code])),
            (Line::Blank, None | Some(Block::Gap)) => {}
            (Line::Blank, _) => out.push(Block::Gap),
            (line, _) => out.push(Block::Words(line)),
        }
    }
    if matches!(out.last(), Some(Block::Gap)) {
        out.pop();
    }
    out
}

/// An answer: `.reply`, #b0b5be, 8 from the right, its Markdown drawn.
fn reply(ui: &mut egui::Ui, text: &str, width: f32) {
    let width = width - 8.0;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for block in blocks(text) {
            match block {
                Block::Gap => ui.add_space(2.0),
                Block::Code(code) => {
                    let format = TextFormat { font_id: FontId::monospace(SIZE - 1.5), color: INK, line_height: Some(LINE_HEIGHT - 2.0), ..Default::default() };
                    let mut job = LayoutJob::single_section(code.join("\n"), format);
                    job.wrap.max_width = width - 20.0;
                    let galley = ui.painter().layout_job(job);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, galley.size().y + 14.0), Sense::hover());
                    ui.painter().rect_filled(rect, 8, Color32::from_white_alpha(12));
                    ui.painter().galley(rect.min + Vec2::new(10.0, 7.0), galley, INK);
                }
                Block::Words(Line::Rule) => {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 9.0), Sense::hover());
                    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, Color32::from_white_alpha(28)));
                }
                Block::Words(Line::Heading(level, words)) => {
                    let size = SIZE + [3.0, 2.0, 1.0].get(level - 1).copied().unwrap_or(0.0);
                    paragraph(ui, &inline(words, true), size, INK, 0.0, width);
                }
                Block::Words(Line::Quote(words)) => {
                    let rect = paragraph(ui, &inline(words, false), SIZE, QUOTE, 12.0, width);
                    ui.painter().vline(rect.left() + 2.0, rect.y_range(), Stroke::new(2.0, Color32::from_white_alpha(40)));
                }
                Block::Words(Line::Item { depth, mark, text }) => {
                    let indent = 4.0 + depth as f32 * 14.0;
                    let rect = paragraph(ui, &inline(text, false), SIZE, REPLY, indent + 16.0, width);
                    let middle = Pos2::new(rect.left() + indent + 4.0, rect.top() + LINE_HEIGHT / 2.0);
                    let painter = ui.painter();
                    match mark {
                        Mark::Bullet => {
                            painter.circle_filled(middle, 2.0, REPLY);
                        }
                        Mark::Number(number) => {
                            painter.text(Pos2::new(rect.left() + indent, middle.y), egui::Align2::LEFT_CENTER, number, font(SIZE), REPLY);
                        }
                        Mark::Task(done) => {
                            let square = Rect::from_center_size(middle + Vec2::new(1.0, 0.0), Vec2::splat(10.0));
                            painter.rect_stroke(square, 3, Stroke::new(1.0, REPLY), StrokeKind::Inside);
                            if done {
                                let points = [
                                    square.left_center() + Vec2::new(2.0, 0.0),
                                    square.center_bottom() - Vec2::new(0.0, 2.5),
                                    square.right_top() + Vec2::new(-2.0, 2.0),
                                ];
                                painter.line(points.to_vec(), Stroke::new(1.4, INK));
                            }
                        }
                    }
                }
                Block::Words(Line::Para(words)) => {
                    paragraph(ui, &inline(words, false), SIZE, REPLY, 0.0, width);
                }
                Block::Words(Line::Blank | Line::Code(_)) => {}
            }
        }
    });
}

/// A run of styled words `indent` in from the left, wrapped at `width`; its whole row.
fn paragraph(ui: &mut egui::Ui, spans: &[Span], size: f32, color: Color32, indent: f32, width: f32) -> Rect {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width - indent;
    for span in spans {
        let font_id = if span.code {
            FontId::monospace(size - 1.5)
        } else if span.bold {
            semi(size)
        } else {
            font(size)
        };
        let ink = if span.link {
            LINK
        } else if span.bold || span.code {
            INK
        } else {
            color
        };
        let format = TextFormat {
            font_id,
            color: ink,
            italics: span.italic,
            line_height: Some(LINE_HEIGHT * size / SIZE),
            background: if span.code { Color32::from_white_alpha(18) } else { Color32::TRANSPARENT },
            strikethrough: if span.strike { Stroke::new(1.0, ink) } else { Stroke::NONE },
            underline: if span.link { Stroke::new(1.0, LINK.gamma_multiply(0.5)) } else { Stroke::NONE },
            ..Default::default()
        };
        job.append(&span.text, 0.0, format);
    }
    let galley = ui.painter().layout_job(job);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, galley.size().y), Sense::hover());
    ui.painter().galley(rect.min + Vec2::new(indent, 0.0), galley, color);
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_has_the_words_and_the_persons_pictures_only() {
        let picture = |user| ChatItem::Image { data: "x".into(), media: "image/png".into(), user, at: 0 };
        let items = [ChatItem::User("oi".into()), ChatItem::System("x".into()), picture(true), picture(false), ChatItem::Agent("olá".into())];
        assert_eq!(said(&items).collect::<Vec<_>>(), [&items[0], &items[2], &items[4]]);
    }

    #[test]
    fn the_persons_words_lose_claude_codes_picture_marks() {
        assert_eq!(unmarked("[Image #15] as mensagens"), "as mensagens");
        assert_eq!(unmarked("a barra [Image #16] está colada"), "a barra está colada");
        assert_eq!(unmarked("[Image #3]"), "");
        assert_eq!(unmarked("veja [Image #x] e [1]"), "veja [Image #x] e [1]");
    }

    #[test]
    fn an_answer_is_drawn_in_blocks_with_its_code_in_one_card() {
        let text = "Rode:\n\n```\n! sudo apt install ./mago.deb\nmagoagent\n```\n\n- **Tema**: claro\n\n\n> nota\n";
        assert_eq!(
            blocks(text),
            [
                Block::Words(Line::Para("Rode:")),
                Block::Gap,
                Block::Code(vec!["! sudo apt install ./mago.deb", "magoagent"]),
                Block::Gap,
                Block::Words(Line::Item { depth: 0, mark: Mark::Bullet, text: "**Tema**: claro" }),
                Block::Gap,
                Block::Words(Line::Quote("nota")),
            ]
        );
        let spans = inline("**Tema**: claro", false);
        assert!(spans[0].bold && spans[0].text == "Tema" && !spans[1].bold);
    }

    #[test]
    fn a_picture_fits_its_box_and_keeps_its_shape() {
        assert_eq!(fit([1100, 700], PICTURE), Vec2::new(220.0, 140.0));
        let wide = fit([686, 316], PICTURE);
        assert!((wide.x - 220.0).abs() < 0.01 && (wide.y - 101.34).abs() < 0.01, "{wide:?}");
        assert_eq!(fit([100, 50], PICTURE), Vec2::new(100.0, 50.0), "a small one stays its size");
    }
}
