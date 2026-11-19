//! O changelog num cartão no meio da tela: com uma versão nova achada pelo app (system/updates.rs, ao abrir e a cada
//! hora), num momento calmo (nenhum agente trabalhando, a janela na frente, nada sendo digitado), "Nova versão X" com as
//! notas da release e Atualizar agora / Atualizar depois / Pular esta versão; na primeira abertura de uma versão que o
//! app instalou, uma vez, "O que há de novo na X" só com Fechar. As notas em markdown simples (`render::markdown`).

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
    /// Pular esta versão.
    Skip,
    Now,
    /// Atualizar e reiniciar, after the warning that the app closes and reopens.
    Restart,
    /// Cancelar on that warning.
    Cancel,
}

impl DrawingApp {
    /// The new version found by the app's own check opens its card in a quiet moment: no agent working, the window in
    /// front and nothing being typed. Held for the agents, it says so once.
    fn open_offer(&mut self, ctx: &Context) {
        let Some(release) = self.updates.offer.clone() else { return };
        if self.changelog.is_some() || !matches!(self.updates.state, UpdateState::Available(_)) {
            self.updates.offer = None;
            return;
        }
        if self.agents_working() {
            if self.updates.held_told.as_deref() != Some(release.version.as_str()) {
                self.updates.held_told = Some(release.version.clone());
                self.toast(format!("Versão {} do MagoAgent disponível: o aviso aparece quando os agentes terminarem.", release.version));
            }
            return;
        }
        // typing (a box of egui, a terminal): 2 s with no key first
        let (time, typed) = ctx.input(|input| (input.time, input.events.iter().any(|event| matches!(event, egui::Event::Text(_) | egui::Event::Key { .. }))));
        if typed {
            self.updates.typed_at = Some(time);
        }
        let busy = ctx.egui_wants_keyboard_input() || self.updates.typed_at.is_some_and(|at| time - at < 2.0);
        // a roteiro's window opens without the focus (a test of the card, MAGOAGENT_CHANGELOG)
        let away = ctx.input(|input| input.viewport().focused == Some(false)) && self.script.is_none();
        if away || busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
            return;
        }
        self.updates.offer = None;
        self.changelog = Some(Changelog { version: release.version, notes: release.notes, installed: false });
    }

    /// Whether an agent is in the middle of a turn: a terminal's, a chat's or a session outside the app.
    pub(crate) fn agents_working(&self) -> bool {
        let working = |state: &crate::app::ui::agents::AgentState| matches!(state, crate::app::ui::agents::AgentState::Working(_));
        self.agent_rows().iter().any(|row| working(&row.state)) || self.outside_pills().iter().any(|pill| working(&pill.state))
    }

    /// The card waits for the welcome and the tour.
    pub(crate) fn changelog_ui(&mut self, ctx: &Context) {
        if self.welcome.is_some() || self.tour.is_some() {
            return;
        }
        self.open_offer(ctx);
        let id = egui::Id::unique("changelog");
        // where the card's top was put when it opened: it grows down from there, not jumping (the warning, the download)
        let top_id = id.with("top");
        let Some(changelog) = self.changelog.clone() else {
            ctx.data_mut(|data| data.remove::<f32>(top_id));
            return;
        };
        let k = K::new(self.dark());
        let screen = ctx.content_rect();
        let confirming = self.updates.confirming && !changelog.installed;
        // after Atualizar: the download on the card, then what went wrong
        let (busy, failed) = match &self.updates.state {
            UpdateState::Downloading(_) | UpdateState::Started(_) if !changelog.installed => (Some(self.updates.state_line()), None),
            UpdateState::Failed(why) if !changelog.installed => (None, Some(format!("Não deu para atualizar: {why}."))),
            _ => (None, None),
        };
        let size = match &self.updates.state {
            UpdateState::Available(release) => release.size,
            _ => None,
        };
        let mut choice = None;
        let veil = egui::Area::new(id.with("veil")).order(egui::Order::Foreground).fixed_pos(screen.min).movable(false).show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0, Color32::from_black_alpha(120));
        });
        let shadow = egui::epaint::Shadow { offset: [0, 16], blur: 48, spread: 0, color: Color32::from_black_alpha(if k.dark { 160 } else { 70 }) };
        let frame = egui::Frame::new().fill(k.bg).corner_radius(14).stroke(Stroke::new(1.0, k.border)).shadow(shadow).inner_margin(INSIDE as i8);
        let top = ctx.data(|data| data.get_temp::<f32>(top_id));
        let area = egui::Area::new(id).order(egui::Order::Foreground);
        let area = match top {
            Some(top) => area.pivot(egui::Align2::CENTER_TOP).fixed_pos(Pos2::new(screen.center().x, top)),
            None => area.anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO),
        };
        let shown = area.show(ctx, |ui| {
            frame.show(ui, |ui| {
                let width = WIDTH - 2.0 * INSIDE;
                ui.set_width(width);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                let title =
                    if changelog.installed { format!("O que há de novo na {}", changelog.version) } else { format!("Nova versão {}", changelog.version) };
                text_line(ui, &title, kit::semibold(18.0), k.fg, width);
                if !changelog.installed {
                    let download = size.map_or(String::new(), |bytes| format!(" · download de {} MB", (bytes + 500_000) / 1_000_000));
                    ui.add_space(4.0);
                    text_line(ui, &format!("Você está na {}{download}", env!("CARGO_PKG_VERSION")), FontId::proportional(12.0), k.muted_fg, width);
                }
                ui.add_space(14.0);
                ui.spacing_mut().scroll = crate::app::ui::scroll_style();
                egui::ScrollArea::vertical().max_height((screen.height() * 0.5).max(120.0)).auto_shrink([false, true]).show(ui, |ui| {
                    ui.set_width(width - 8.0);
                    paint_notes(ui, &k, &changelog.notes);
                });
                ui.add_space(16.0);
                if let Some(busy) = &busy {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(14.0).color(k.muted_fg));
                        text_line(ui, busy, FontId::proportional(12.0), k.muted_fg, width - 24.0);
                    });
                    return;
                }
                if let Some(failed) = &failed {
                    text_line(ui, failed, FontId::proportional(12.0), k.destructive, width);
                    ui.add_space(12.0);
                } else if confirming {
                    text_line(ui, CLOSES_AND_REOPENS, FontId::proportional(12.0), k.muted_fg, width);
                    ui.add_space(12.0);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let buttons: &[(&str, ChangelogChoice, bool)] = match (changelog.installed || failed.is_some(), confirming) {
                        (true, _) => &[("Fechar", ChangelogChoice::Later, true)],
                        (false, true) => &[("Atualizar e reiniciar", ChangelogChoice::Restart, true), ("Cancelar", ChangelogChoice::Cancel, false)],
                        (false, false) => &[("Atualizar agora", ChangelogChoice::Now, true), ("Atualizar depois", ChangelogChoice::Later, false)],
                    };
                    for (label, what, primary) in buttons {
                        let clicked = card_button(ui, &k, label, *primary).clicked();
                        if clicked {
                            choice = Some(*what);
                        }
                    }
                    // the version let go: its card won't open by itself again
                    if !changelog.installed && failed.is_none() && !confirming {
                        ui.add_space(ui.available_width() - link_width(ui, SKIP));
                        if skip_link(ui, &k).clicked() {
                            choice = Some(ChangelogChoice::Skip);
                        }
                    }
                });
            });
        });
        if top.is_none() {
            let height = shown.response.rect.height();
            ctx.data_mut(|data| data.insert_temp(top_id, (screen.center().y - height / 2.0).max(screen.top() + 16.0)));
        }
        ctx.move_to_top(veil.response.layer_id);
        ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, id));
        super::titlebar::raise_window_buttons(ctx, egui::LayerId::new(egui::Order::Foreground, id));
        if busy.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            choice = Some(if confirming { ChangelogChoice::Cancel } else { ChangelogChoice::Later });
        }
        if let Some(choice) = choice {
            self.changelog_choose(choice);
        }
    }

    /// Atualizar depois closes the card for 6 h or until the computer restarts (`update_put_off`); Pular esta versão
    /// for good; the version still waits in Ajustes › Sobre. Atualizar agora goes by the same way as Sobre's button,
    /// the warning first when the app closes and reopens, and the card stays to show the download.
    pub(crate) fn changelog_choose(&mut self, choice: ChangelogChoice) {
        let installed = self.changelog.as_ref().is_some_and(|changelog| changelog.installed);
        match choice {
            ChangelogChoice::Later | ChangelogChoice::Skip if !installed && matches!(self.updates.state, UpdateState::Available(_)) => {
                let _ = self.update_put_off(choice == ChangelogChoice::Skip);
            }
            ChangelogChoice::Later | ChangelogChoice::Skip => {
                self.changelog = None;
                self.updates.confirming = false;
            }
            ChangelogChoice::Cancel => self.updates.confirming = false,
            ChangelogChoice::Now if matches!(&self.updates.state, UpdateState::Available(release) if installs_in_place(release)) => {
                self.updates.confirming = true;
            }
            ChangelogChoice::Now | ChangelogChoice::Restart => {
                let in_place = matches!(&self.updates.state, UpdateState::Available(release) if installs_in_place(release));
                if !in_place {
                    self.changelog = None;
                }
                if let Err(why) = self.update_install() {
                    self.changelog = None;
                    self.toast(why);
                }
            }
        }
    }
}

const SKIP: &str = "Pular esta versão";

fn link_width(ui: &egui::Ui, text: &str) -> f32 {
    ui.painter().layout_no_wrap(text.into(), FontId::proportional(12.0), Color32::WHITE).size().x
}

/// "Pular esta versão": a quiet link at the card's left, underlined under the pointer.
fn skip_link(ui: &mut egui::Ui, k: &K) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(SKIP.into(), FontId::proportional(12.0), k.muted_fg);
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, SKIP));
    let ink = if response.hovered() { k.fg } else { k.muted_fg };
    if response.hovered() {
        ui.painter().hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, ink));
    }
    ui.painter().galley(rect.min, galley, ink);
    response
}

/// One text, wrapped at `width`.
fn text_line(ui: &mut egui::Ui, text: &str, font: FontId, color: Color32, width: f32) {
    let galley = ui.painter().layout(text.into(), font, color, width);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, galley.size().y), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
}

/// The card's buttons, one size and one line: the accent's, or `kit::button`'s colours.
fn card_button(ui: &mut egui::Ui, k: &K, text: &str, primary: bool) -> egui::Response {
    let ink = if primary { Color32::WHITE } else { k.fg };
    let galley = ui.painter().layout_no_wrap(text.into(), kit::semibold(13.0), ink);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(galley.size().x + 28.0, 32.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));
    let fill = if primary { k.primary } else { k.button };
    ui.painter().rect_filled(rect, 8, if response.hovered() { fill.gamma_multiply(0.85) } else { fill });
    if !primary {
        ui.painter().rect_stroke(rect, 8, Stroke::new(1.0, k.button_line), StrokeKind::Inside);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, ink);
    response
}

/// The notes, a line of the source a line on the card: headings bigger, items with their mark, quotes and code in.
fn paint_notes(ui: &mut egui::Ui, k: &K, notes: &str) {
    let width = ui.available_width();
    if notes.trim().is_empty() {
        return text_line(ui, "Essa release não traz notas.", FontId::proportional(14.0), k.muted_fg, width);
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
                (0.0, String::new(), markdown::inline(text, true), if level <= 2 { 16.0 } else { 15.0 }, k.fg)
            }
            Line::Item { depth, mark, text } => {
                let mark = match mark {
                    Mark::Bullet => "•".to_string(),
                    Mark::Number(number) => number,
                    Mark::Task(done) => if done { "☑" } else { "☐" }.to_string(),
                };
                (14.0 + depth as f32 * 14.0, mark, markdown::inline(text, false), 14.0, k.fg)
            }
            Line::Quote(text) => (14.0, "│".into(), markdown::inline(text, false), 14.0, k.muted_fg),
            Line::Code(text) => (14.0, String::new(), vec![Span { text: text.into(), code: true, ..Default::default() }], 13.0, k.fg),
            Line::Para(text) => (0.0, String::new(), markdown::inline(text, false), 14.0, k.fg),
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
