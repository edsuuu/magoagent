//! Dúvidas: per agent its questions, each with the answers it suggests and a box for the person's, then Enviar
//! respondidas, Enviar todas and Descartar. On the canvas it is one of the right panels; over the chat it opens on top
//! of the conversation, the same panel.

use crate::prelude::*;

/// What the panel's buttons asked for: (agent, what).
enum DoubtAction {
    Send(String, bool),
    Discard(String),
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

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(name).size(13.5).strong().color(colors.text));
                ui.label(egui::RichText::new(format!("{answered} de {total} respondidas")).size(12.0).color(colors.muted));
            });

            for doubt in self.doubts.list.iter_mut().filter(|doubt| &doubt.agent == agent) {
                super::card(ui, dark, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.add(egui::Label::new(egui::RichText::new(&doubt.question).size(13.0).color(colors.text)).wrap());

                    if !doubt.options.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for option in &doubt.options {
                                if ui.selectable_label(doubt.answer.trim() == option, option).clicked() {
                                    doubt.answer = option.clone();
                                    changed = true;
                                }
                            }
                        });
                    }

                    let edit = egui::TextEdit::multiline(&mut doubt.answer)
                        .id(egui::Id::unique(("doubt_answer", doubt.id)))
                        .hint_text("Sua resposta…")
                        .desired_rows(1)
                        .desired_width(f32::INFINITY);

                    changed |= ui.add(edit).changed();
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
