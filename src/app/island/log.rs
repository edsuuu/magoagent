//! The island's conversation log, the app's chat and a session outside alike: the person's words in bubbles at the
//! right with the pictures they pasted, the answers drawn as the chat on the canvas draws them (`agent_words_in`: code
//! cards with Copiar, pictures, files, tables), and the dots of an agent at work. It sticks to the bottom as it grows.

use super::paint::{INK, font};
use crate::app::chat::image_key;
use crate::app::chat::images::thumb;
use crate::prelude::*;
use crate::transcript::ChatItem;
use egui::text::{LayoutJob, TextFormat};

pub(super) const REPLY: Color32 = Color32::from_rgb(0xb0, 0xb5, 0xbe);
/// The type, the room between two messages and between the lines of one.
const SIZE: f32 = 12.5;
const MESSAGE_GAP: f32 = 14.0;
const LINE_HEIGHT: f32 = 18.0;
/// A pasted picture is at most this big in the log.
const PICTURE: Vec2 = Vec2::new(220.0, 140.0);

/// What the log shows of a conversation: the words said and the pictures the person sent.
pub(super) fn said(items: &[ChatItem]) -> impl Iterator<Item = &ChatItem> {
    items.iter().filter(|item| matches!(item, ChatItem::User(_) | ChatItem::Agent(_) | ChatItem::Image { user: true, .. }))
}

impl DrawingApp {
    /// The log of `items` (from `said`) in `log`; the picture clicked in it (its base64), to show on the whole screen.
    pub(super) fn island_log(&self, ui: &mut egui::Ui, log: Rect, items: &[&ChatItem], working: bool, time: f64) -> Option<String> {
        let mut clicked = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(log), |ui| {
            ui.spacing_mut().scroll = crate::app::ui::scroll_style();
            egui::ScrollArea::vertical().id_salt("island_prompt_log").stick_to_bottom(true).auto_shrink(false).show(ui, |ui| {
                // the room the scroll bar leaves (`ui::scroll_style`)
                let width = ui.available_width();
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = MESSAGE_GAP;
                // the answers as the chat on the canvas draws them (code cards with Copiar, pictures, files), always dark
                ui.visuals_mut().dark_mode = true;
                let colors = UiColors::new(true);
                for item in items {
                    match item {
                        // another agent's question: whose it is over it, as the chat on the canvas says (chat_item)
                        ChatItem::User(text) => match crate::app::chat::ask::asked_by(text) {
                            Some((who, question)) => {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 4.0;
                                    asker(ui, who, width);
                                    bubble(ui, question, width);
                                });
                            }
                            None => bubble(ui, text, width),
                        },
                        ChatItem::Agent(text) => self.agent_words_in(ui, text, width - 8.0, &colors, false),
                        ChatItem::Image { data, .. } if self.island_picture(ui, data, width) => clicked = Some(data.clone()),
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
        clicked
    }

    /// A picture the person pasted, at the right as their words, fitted in `PICTURE` (the chat's thumbnails, decoded once).
    fn island_picture(&self, ui: &mut egui::Ui, data: &str, width: f32) -> bool {
        let pic = self.chat_images.borrow_mut().entry(image_key(data)).or_insert_with(|| thumb(ui.ctx(), data)).clone();
        let size = match pic.get() {
            Some(None) => return false,
            Some(Some(pic)) => fit(pic.size, PICTURE),
            None => PICTURE,
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, size.y), Sense::hover());
        let frame = Rect::from_min_size(Pos2::new(rect.right() - size.x, rect.top()), size);
        let response = ui.interact(frame, ui.scope_id().with(("island_picture", image_key(data))), Sense::click());
        let response = response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Ver em tela cheia");
        match pic.get().and_then(Option::as_ref) {
            Some(pic) => {
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                ui.painter().add(egui::epaint::RectShape::filled(frame, 10, Color32::WHITE).with_texture(pic.texture.id(), uv));
            }
            None => {
                ui.painter().rect_filled(frame, 10, Color32::from_white_alpha(14));
            }
        }
        let edge = if response.hovered() { 60 } else { 22 };
        ui.painter().rect_stroke(frame, 10, Stroke::new(1.0, Color32::from_white_alpha(edge)), StrokeKind::Inside);
        response.clicked()
    }
}

/// `size` (pixels) shrunk to fit `most`, its shape kept.
fn fit(size: [u32; 2], most: Vec2) -> Vec2 {
    let size = Vec2::new(size[0].max(1) as f32, size[1].max(1) as f32);
    size * (most.x / size.x).min(most.y / size.y).min(1.0)
}

/// The person's words: `.chat-row.user`, at the right, 32 in from the left; `.bubble`: 13% white, radius 12; selectable.
fn bubble(ui: &mut egui::Ui, text: &str, width: f32) {
    let text = unmarked(text);
    if text.is_empty() {
        return;
    }
    let format = TextFormat { font_id: font(SIZE), color: INK, line_height: Some(LINE_HEIGHT), ..Default::default() };
    let mut job = LayoutJob::single_section(text, format);
    job.wrap.max_width = width - 32.0 - 20.0;
    let galley = ui.painter().layout_job(job.clone());
    let size = galley.size() + Vec2::new(20.0, 14.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, size.y), Sense::hover());
    let bubble = Rect::from_min_size(Pos2::new(rect.right() - size.x, rect.top()), size);
    ui.painter().rect_filled(bubble, 12, Color32::from_white_alpha(33));
    selectable(ui, bubble.min + Vec2::new(10.0, 7.0), job);
}

/// Over an agent's question in the log: "Pergunta de <who>", small and at the right, where the bubble goes.
fn asker(ui: &mut egui::Ui, who: &str, width: f32) {
    let galley = crate::app::chat::list::clipped(ui.painter(), format!("Pergunta de {who}"), font(11.0), REPLY, width - 32.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, galley.size().y), Sense::hover());
    ui.painter().galley(Pos2::new(rect.right() - galley.size().x, rect.top()), galley, REPLY);
}

/// `job` at `at` as text the person can select and copy (asked: "o texto que o Claude retornar eu preciso poder
/// selecionar, até as minhas mensagens"): the same job that was measured, wrapped at its own width, from the left.
fn selectable(ui: &mut egui::Ui, at: Pos2, job: LayoutJob) {
    let room = Rect::from_min_size(at, Vec2::new(job.wrap.max_width, f32::INFINITY));
    ui.scope_builder(egui::UiBuilder::new().max_rect(room).layout(egui::Layout::top_down(egui::Align::LEFT)), |ui| {
        ui.add(egui::Label::new(job).wrap().selectable(true));
    });
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
    fn an_answer_in_the_island_is_drawn_as_the_chat_on_the_canvas() {
        let (ctx, app) = (Context::default(), DrawingApp::default());
        let path = std::env::temp_dir().join(format!("mago-island-log-{}.png", std::process::id()));
        let answer = ChatItem::Agent(format!("Abra e digite `wsl -d`:\n\n```bash\nwsl -d Ubuntu\n```\n\n![Print das duas telas]({})", path.display()));
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 900.0))), ..Default::default() };
        let output = ctx.run_ui(raw, |ui| {
            app.island_log(ui, Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 880.0)), &[&answer], false, 0.0);
        });
        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|text| text == "Copiar"), "the code card and its Copiar: {texts:?}");
        assert!(texts.iter().any(|text| text.contains("digite wsl -d")), "the inline code keeps its space: {texts:?}");
        assert!(!texts.iter().any(|text| text.contains("Print das duas telas")), "the picture is not its words: {texts:?}");
        // the picture went to the chat's own picture (no file there: its card)
        assert!(texts.iter().any(|text| text == "Arquivo não encontrado"), "{texts:?}");
    }

    #[test]
    fn a_picture_fits_its_box_and_keeps_its_shape() {
        assert_eq!(fit([1100, 700], PICTURE), Vec2::new(220.0, 140.0));
        let wide = fit([686, 316], PICTURE);
        assert!((wide.x - 220.0).abs() < 0.01 && (wide.y - 101.34).abs() < 0.01, "{wide:?}");
        assert_eq!(fit([100, 50], PICTURE), Vec2::new(100.0, 50.0), "a small one stays its size");
    }
}
