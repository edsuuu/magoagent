//! O changelog num cartão no meio da tela: na primeira abertura do dia com uma versão nova (system/updates.rs), "Nova
//! versão X" com as notas da release e Atualizar agora / Atualizar depois; na primeira abertura de uma versão que o app
//! instalou, uma vez, "O que há de novo na X" só com Fechar. As notas em markdown simples (`render::markdown`).

use super::settings::kit::{self, K};
use crate::app::system::updates::{CLOSES_AND_REOPENS, UpdateState, installs_in_place};
use crate::prelude::*;
use crate::render::markdown::{self, Line, Mark, Span};
use egui::text::{LayoutJob, TextFormat};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Changelog {
    pub(crate) version: String,
    pub(crate) notes: String,
    /// "O que há de novo": the version is the one running, there is nothing to install.
    pub(crate) installed: bool,
}

const WIDTH: f32 = 520.0;
const INSIDE: f32 = 22.0;

/// What the card's buttons asked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ChangelogChoice {
    /// Atualizar depois, Fechar or Esc.
    Later,
    Now,
    /// Atualizar e reiniciar, after the warning that the app closes and reopens.
    Restart,
    /// Cancelar on that warning.
    Cancel,
}

impl DrawingApp {
    /// The card waits for the welcome and the tour.
    pub(crate) fn changelog_ui(&mut self, ctx: &Context) {
        if self.welcome.is_some() || self.tour.is_some() {
            return;
        }
        let Some(changelog) = self.changelog.clone() else { return };
        let k = K::new(self.dark());
        let screen = ctx.content_rect();
        let confirming = self.updates.confirming && !changelog.installed;
        let mut choice = None;
        let id = egui::Id::unique("changelog");
        let veil = egui::Area::new(id.with("veil")).order(egui::Order::Foreground).fixed_pos(screen.min).movable(false).show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0, Color32::from_black_alpha(120));
        });
        let shadow = egui::epaint::Shadow { offset: [0, 16], blur: 48, spread: 0, color: Color32::from_black_alpha(if k.dark { 160 } else { 70 }) };
        let frame = egui::Frame::new().fill(k.bg).corner_radius(14).stroke(Stroke::new(1.0, k.border)).shadow(shadow).inner_margin(INSIDE as i8);
        egui::Area::new(id).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            frame.show(ui, |ui| {
                let width = WIDTH - 2.0 * INSIDE;
                ui.set_width(width);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                let (title, line) = if changelog.installed {
                    (format!("O que há de novo na {}", changelog.version), format!("Você já está na versão {}.", changelog.version))
                } else {
                    (format!("Nova versão {}", changelog.version), format!("Você usa a {}. Veja o que muda:", env!("CARGO_PKG_VERSION")))
                };
                text_line(ui, &title, kit::semibold(19.0), k.fg, width);
                ui.add_space(4.0);
                text_line(ui, &line, FontId::proportional(13.0), k.muted_fg, width);
                ui.add_space(14.0);
                ui.spacing_mut().scroll = crate::app::ui::scroll_style();
                egui::ScrollArea::vertical().max_height((screen.height() * 0.5).max(120.0)).auto_shrink([false, true]).show(ui, |ui| {
                    ui.set_width(width - 8.0);
                    paint_notes(ui, &k, &changelog.notes);
                });
                ui.add_space(16.0);
                if confirming {
                    text_line(ui, CLOSES_AND_REOPENS, FontId::proportional(12.5), k.muted_fg, width);
                    ui.add_space(12.0);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let buttons: &[(&str, ChangelogChoice, bool)] = match (changelog.installed, confirming) {
                        (true, _) => &[("Fechar", ChangelogChoice::Later, true)],
                        (false, true) => &[("Atualizar e reiniciar", ChangelogChoice::Restart, true), ("Cancelar", ChangelogChoice::Cancel, false)],
                        (false, false) => &[("Atualizar agora", ChangelogChoice::Now, true), ("Atualizar depois", ChangelogChoice::Later, false)],
                    };
                    for (label, what, primary) in buttons {
                        let clicked = if *primary { primary_button(ui, &k, label) } else { kit::button(ui, &k, label) }.clicked();
                        if clicked {
                            choice = Some(*what);
                        }
                    }
                });
            });
        });
        ctx.move_to_top(veil.response.layer_id);
        ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, id));
        super::titlebar::raise_window_buttons(ctx, egui::LayerId::new(egui::Order::Foreground, id));
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            choice = Some(if confirming { ChangelogChoice::Cancel } else { ChangelogChoice::Later });
        }
        if let Some(choice) = choice {
            self.changelog_choose(choice);
        }
    }

    /// Atualizar depois closes the card: the next day's first opening (or Ajustes › Sobre) shows the version again.
    /// Atualizar agora goes by the same way as Sobre's button, the warning first when the app closes and reopens.
    pub(crate) fn changelog_choose(&mut self, choice: ChangelogChoice) {
        match choice {
            ChangelogChoice::Later => {
                self.changelog = None;
                self.updates.confirming = false;
            }
            ChangelogChoice::Cancel => self.updates.confirming = false,
            ChangelogChoice::Now if matches!(&self.updates.state, UpdateState::Available(release) if installs_in_place(release)) => {
                self.updates.confirming = true;
            }
            ChangelogChoice::Now | ChangelogChoice::Restart => {
                self.changelog = None;
                let in_place = matches!(&self.updates.state, UpdateState::Available(release) if installs_in_place(release));
                match self.update_install() {
                    Err(why) => self.toast(why),
                    Ok(()) if in_place => self.toast("Baixando e conferindo a versão nova; depois o MagoAgent salva o trabalho, fecha e reabre sozinho."),
                    Ok(()) => {}
                }
            }
        }
    }
}

/// One text, wrapped at `width`.
fn text_line(ui: &mut egui::Ui, text: &str, font: FontId, color: Color32, width: f32) {
    let galley = ui.painter().layout(text.into(), font, color, width);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, galley.size().y), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
}

/// Like `kit::button`, in the accent.
fn primary_button(ui: &mut egui::Ui, k: &K, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.into(), kit::semibold(12.5), Color32::WHITE);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(galley.size().x + 28.0, 30.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));
    ui.painter().rect_filled(rect, 8, if response.hovered() { k.primary.gamma_multiply(0.85) } else { k.primary });
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    response
}

/// The notes, a line of the source a line on the card: headings bigger, items with their mark, quotes and code in.
fn paint_notes(ui: &mut egui::Ui, k: &K, notes: &str) {
    let width = ui.available_width();
    if notes.trim().is_empty() {
        return text_line(ui, "Essa release não traz notas.", FontId::proportional(13.5), k.muted_fg, width);
    }
    for line in markdown::lines(notes) {
        let (indent, mark, spans, size, ink) = match line {
            Line::Blank => {
                ui.add_space(6.0);
                continue;
            }
            Line::Rule => {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 13.0), Sense::hover());
                ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, k.border));
                continue;
            }
            Line::Heading(level, text) => {
                ui.add_space(6.0);
                (0.0, String::new(), markdown::inline(text, true), if level <= 2 { 16.0 } else { 14.5 }, k.fg)
            }
            Line::Item { depth, mark, text } => {
                let mark = match mark {
                    Mark::Bullet => "•".to_string(),
                    Mark::Number(number) => number,
                    Mark::Task(done) => if done { "☑" } else { "☐" }.to_string(),
                };
                (14.0 + depth as f32 * 14.0, mark, markdown::inline(text, false), 13.5, k.fg)
            }
            Line::Quote(text) => (14.0, "│".into(), markdown::inline(text, false), 13.5, k.muted_fg),
            Line::Code(text) => (14.0, String::new(), vec![Span { text: text.into(), code: true, ..Default::default() }], 12.5, k.fg),
            Line::Para(text) => (0.0, String::new(), markdown::inline(text, false), 13.5, k.fg),
        };
        let galley = ui.painter().layout_job(notes_job(&spans, size, ink, width - indent));
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, galley.size().y + 3.0), Sense::hover());
        if !mark.is_empty() {
            ui.painter().text(rect.min + Vec2::new(indent - 12.0, 0.0), egui::Align2::LEFT_TOP, mark, FontId::proportional(size), k.muted_fg);
        }
        ui.painter().galley(rect.min + Vec2::new(indent, 0.0), galley, ink);
    }
}

/// The spans as one layout in the interface's face: bold in its semibold, code in the monospace on a tint.
pub(crate) fn notes_job(spans: &[Span], size: f32, ink: Color32, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    for span in spans {
        let font_id = if span.code {
            FontId::monospace(size * 0.92)
        } else if span.bold {
            kit::semibold(size)
        } else {
            FontId::proportional(size)
        };
        let format = TextFormat {
            font_id,
            color: ink,
            italics: span.italic,
            line_height: Some(size * 1.45),
            strikethrough: if span.strike { Stroke::new(1.0, ink) } else { Stroke::NONE },
            underline: if span.link { Stroke::new(1.0, ink.gamma_multiply(0.4)) } else { Stroke::NONE },
            background: if span.code { ink.gamma_multiply(0.1) } else { Color32::TRANSPARENT },
            ..Default::default()
        };
        job.append(&span.text, 0.0, format);
    }
    job
}
