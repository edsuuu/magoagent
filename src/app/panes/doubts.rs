//! Dúvidas: a section per agent (its name and its git branch, folds), each of its questions a card that folds too: the
//! question, the agent's recommendation apart (information that folds, not a button), the answers it suggests as
//! buttons and a box for the person's; then Enviar respondidas, Enviar todas and Descartar. On the canvas it is one of
//! the right panels; over the chat it opens on top of the conversation, the same panel.

use crate::prelude::*;
use std::sync::Arc;

/// What the panel's buttons asked for: (agent, what).
enum DoubtAction {
    Send(String, bool),
    Discard(String),
}

/// `parts` (text, size, color) laid out to wrap at the width a folding header gives its text in `ui`: the header's own
/// text does not wrap.
fn header_text(ui: &egui::Ui, parts: &[(&str, f32, Color32)]) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::default();

    for (text, size, color) in parts {
        job.append(text, 0.0, egui::TextFormat::simple(FontId::proportional(*size), *color));
    }

    job.wrap.max_width = ui.available_width() - ui.spacing().indent;
    ui.ctx().fonts_mut(|fonts| fonts.layout_job(job))
}

impl DrawingApp {
    /// The panel's body.
    pub(crate) fn doubts_pane(&mut self, ui: &mut egui::Ui) {
        let (colors, dark) = (self.colors(), self.dark());
        let agents = self.doubts.agents();

        if agents.is_empty() {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "Nenhuma dúvida agora. Quando um agente tiver perguntas para você, elas aparecem aqui: responda e envie as respondidas ou \
                         todas de uma vez.",
                    )
                    .size(13.0)
                    .color(colors.muted),
                )
                .wrap(),
            );

            return;
        }

        let names: Vec<String> = agents.iter().map(|agent| self.agent_label(agent)).collect();
        let mut action = None;
        let mut changed = false;

        for (agent, name) in agents.iter().zip(&names) {
            let (answered, total) = (self.doubts.of(agent).filter(|doubt| doubt.answered()).count(), self.doubts.of(agent).count());
            let branch = self.doubts.branch(agent);
            let branch = if branch.is_empty() { String::new() } else { format!("   ramo {branch}") };
            let count = format!("   {answered} de {total} respondidas");
            let title = header_text(ui, &[(name, 13.5, colors.text), (&branch, 12.0, colors.code), (&count, 12.0, colors.muted)]);

            egui::CollapsingHeader::new(title).id_salt(("doubts_agent", agent)).default_open(true).show(ui, |ui| {
                for doubt in self.doubts.list.iter_mut().filter(|doubt| &doubt.agent == agent) {
                    super::card(ui, dark, |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        let (question, recommendation) = doubt.parts();
                        let answer = if doubt.answered() { format!("\n→ {}", doubt.answer.trim()) } else { String::new() };
                        let header = header_text(ui, &[(&question, 13.0, colors.text), (&answer, 12.0, colors.muted)]);

                        egui::CollapsingHeader::new(header).id_salt(("doubt", doubt.id)).default_open(!doubt.answered()).show(ui, |ui| {
                            // what the agent would pick: to read, so it folds and has no button look
                            if !recommendation.is_empty() {
                                let label = egui::RichText::new("Recomendação do agente").size(12.0).italics().color(colors.muted);

                                egui::CollapsingHeader::new(label).id_salt(("doubt_recommendation", doubt.id)).show(ui, |ui| {
                                    egui::Frame::new().fill(colors.surface_low).corner_radius(6).inner_margin(6).show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        let said = egui::RichText::new(&recommendation).size(12.5).italics().color(colors.muted);

                                        ui.add(egui::Label::new(said).wrap());
                                    });
                                });
                            }

                            if !doubt.options.is_empty() {
                                ui.label(egui::RichText::new("Escolha uma resposta").size(11.5).color(colors.muted));

                                ui.horizontal_wrapped(|ui| {
                                    for option in &doubt.options {
                                        let button = egui::Button::new(egui::RichText::new(option).size(12.5)).selected(doubt.answer.trim() == option);

                                        if ui.add(button).clicked() {
                                            doubt.answer = option.clone();
                                            changed = true;
                                        }
                                    }
                                });
                            }

                            let hint = if doubt.options.is_empty() { "Sua resposta…" } else { "Ou escreva a sua resposta…" };

                            let edit = egui::TextEdit::multiline(&mut doubt.answer)
                                .id(egui::Id::unique(("doubt_answer", doubt.id)))
                                .hint_text(hint)
                                .desired_rows(1)
                                .desired_width(f32::INFINITY);

                            changed |= ui.add(edit).changed();
                        });
                    });
                }

                ui.horizontal_wrapped(|ui| {
                    let send_answered = ui
                        .add_enabled(answered > 0, egui::Button::new(format!("Enviar respondidas ({answered})")))
                        .on_hover_text("Manda ao agente só as que têm resposta; as outras continuam aqui");

                    if send_answered.clicked() {
                        action = Some(DoubtAction::Send(agent.clone(), false));
                    }

                    if ui.button("Enviar todas").on_hover_text("Manda todas de uma vez; as sem resposta vão como decisão do agente").clicked() {
                        action = Some(DoubtAction::Send(agent.clone(), true));
                    }

                    if ui.button("Descartar").on_hover_text("Tira estas dúvidas da tela sem mandar nada").clicked() {
                        action = Some(DoubtAction::Discard(agent.clone()));
                    }
                });
            });

            ui.add_space(8.0);
        }

        if changed {
            self.doubts.changed_at = Some(ui.input(|input| input.time));
        }

        match action.filter(|_| super::live(ui)) {
            Some(DoubtAction::Send(agent, all)) => match self.doubts_send(&agent, all) {
                Ok(said) => self.toast(said),
                Err(why) => self.toast(why),
            },
            Some(DoubtAction::Discard(agent)) => self.doubts_discard(&agent),
            None => {}
        }
    }
}
