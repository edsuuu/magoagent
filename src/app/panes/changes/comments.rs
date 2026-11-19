//! The review in the panel: while lines of the diff are picked, a bar pinned to the panel's bottom for their comment
//! (over nothing: inline it would push the lines from under the pointer, a box under them would cover the next ones to
//! pick); below the list, the comments made (each with its ×), the general note, the agent it goes to (the ones working
//! in the folder first) and the buttons Enviar revisão and Pedir análise de PR.

use super::super::{card, live};
use super::{ChangeGroup, comment_on};
use crate::prelude::*;

/// The comment's bar at the panel's bottom.
const COMMENT_BAR: f32 = 136.0;
/// The lines of a comment's excerpt its card shows; the rest is counted.
const EXCERPT_LINES: usize = 4;

/// The excerpt's lines as `excerpt_ui` shows them: each line's mark (+, - or a space) and its text, without the
/// indentation they share (tabs as four spaces), at most `EXCERPT_LINES`, and how many more there are.
pub(crate) fn excerpt_lines(excerpt: &str) -> (Vec<(char, String)>, usize) {
    let lines: Vec<(char, String)> = excerpt
        .lines()
        .map(|line| {
            let mut characters = line.chars();
            let mark = characters.next().unwrap_or(' ');

            (mark, characters.as_str().replace('\t', "    "))
        })
        .collect();

    let shared = lines.iter().filter(|(_, text)| !text.trim().is_empty()).map(|(_, text)| text.len() - text.trim_start().len()).min().unwrap_or(0);
    let more = lines.len().saturating_sub(EXCERPT_LINES);
    let shown = lines.into_iter().take(EXCERPT_LINES).map(|(mark, text)| (mark, text.get(shared..).unwrap_or(text.trim_start()).to_string())).collect();

    (shown, more)
}

/// A comment's lines as in the diff: the mark and the code in monospace on the row's green or red, cut at the card's
/// edge, then "+ N linhas" when there are more.
fn excerpt_ui(ui: &mut egui::Ui, excerpt: &str, muted: Color32) {
    let (lines, more) = excerpt_lines(excerpt);

    ui.spacing_mut().item_spacing.y = 0.0;

    for (mark, text) in lines {
        let fill = match mark {
            '+' => super::diff::ADDED_ROW,
            '-' => super::diff::REMOVED_ROW,
            _ => Color32::TRANSPARENT,
        };

        egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(4, 1)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(egui::RichText::new(format!("{mark} {text}")).monospace().size(11.0)).truncate());
        });
    }

    if more > 0 {
        ui.label(egui::RichText::new(format!("+ {more} linhas")).size(11.0).color(muted));
    }

    ui.spacing_mut().item_spacing.y = 6.0;
}

impl DrawingApp {
    /// How much of the panel's bottom the comment's bar takes: all of it while lines are picked, none otherwise.
    pub(crate) fn comment_bar_height(&self) -> f32 {
        let changes = &self.panes.changes;

        if changes.review.picked.is_some() && changes.selected.is_some() { COMMENT_BAR } else { 0.0 }
    }

    /// The comment's bar on the lines picked of the file shown: where, a field (it takes the keys as lines are picked),
    /// Comentar (or Ctrl+Enter) and Cancelar (or Esc, in `canvas/shortcuts.rs`: it lets the lines go before the panel).
    pub(crate) fn review_comment_bar(&mut self, ui: &mut egui::Ui) {
        let colors = self.colors();
        let (Some(rows), Some((_, path))) = (self.panes.changes.review.picked, self.panes.changes.selected.clone()) else { return };
        let root = self.shown_groups().into_iter().find(|g| g.files.iter().any(|(p, _)| *p == path)).map(|g| g.root);
        let Some(preview) = root.and_then(|root| comment_on(&root, &path, &self.panes.changes.diff, rows, "")) else { return };
        let field_id = egui::Id::unique("changes_review_draft");
        let mut add = ui.memory(|m| m.has_focus(field_id)) && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        let mut cancel = false;

        ui.spacing_mut().item_spacing.y = 6.0;
        let lines = if preview.removed { " (removidas)" } else { "" };

        ui.label(egui::RichText::new(format!("Comentar {}{lines}", preview.place())).size(12.0).color(colors.muted));

        let field = egui::TextEdit::multiline(&mut self.panes.changes.review.draft)
            .id(field_id)
            .hint_text("O que está errado aqui?")
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .background_color(colors.surface_low);

        let field = ui.add(field);

        if std::mem::take(&mut self.panes.changes.review.focus_draft) {
            field.request_focus();
        }

        ui.horizontal(|ui| {
            let written = !self.panes.changes.review.draft.trim().is_empty();

            add |= ui.add_enabled(written, egui::Button::new("Comentar")).on_hover_text("Ctrl+Enter").clicked();
            cancel |= ui.button("Cancelar").on_hover_text("Esc").clicked();
        });

        if !live(ui) {
            return;
        }

        let review = &mut self.panes.changes.review;

        if add && !review.draft.trim().is_empty() {
            review.comments.push(super::ReviewComment { note: review.draft.trim().to_string(), ..preview });
            (review.draft, review.picked) = (String::new(), None);
        } else if cancel {
            review.picked = None;
        }
    }

    /// The review: its comments, the general note, the agent and the two ways to send.
    pub(super) fn review_section(&mut self, ui: &mut egui::Ui, groups: &[ChangeGroup]) {
        let (colors, dark) = (self.colors(), self.dark());

        ui.add_space(4.0);
        let count = self.panes.changes.review.comments.len();
        let heading = if count == 0 { "Revisão".to_string() } else { format!("Revisão · {count}") };

        ui.label(egui::RichText::new(heading).size(12.0).color(colors.muted));
        let mut removed = None;

        for (index, comment) in self.panes.changes.review.comments.iter().enumerate() {
            card(ui, dark, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let remove = ui.add(egui::Button::new("×").frame(false)).on_hover_text("Tirar o comentário");

                    if remove.clicked() {
                        removed = Some(index);
                    }

                    ui.vertical(|ui| {
                        ui.add(egui::Label::new(egui::RichText::new(comment.place()).monospace().size(12.0)).truncate());
                        excerpt_ui(ui, &comment.excerpt, colors.muted);
                        ui.add(egui::Label::new(&comment.note).wrap());
                    });
                });
            });
        }

        if let Some(index) = removed.filter(|_| live(ui)) {
            self.panes.changes.review.comments.remove(index);
        }

        if count == 0 {
            ui.label(egui::RichText::new("Clique nas linhas de um diff para comentar, como numa revisão de PR.").size(11.0).color(colors.muted));
        }

        let note = egui::TextEdit::multiline(&mut self.panes.changes.review.note)
            .hint_text("Observação geral (opcional)")
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .background_color(colors.surface_low);

        ui.add(note);
        // the agents working in these folders first, the one of the file open before them
        let mut roots: Vec<PathBuf> = Vec::new();

        if let Some((_, path)) = &self.panes.changes.selected {
            roots.extend(groups.iter().filter(|g| path.starts_with(&g.root)).map(|g| g.root.clone()));
        }

        roots.extend(groups.iter().filter(|g| !g.files.is_empty()).map(|g| g.root.clone()));
        let agents = self.review_agents(&roots);

        if !agents.iter().any(|(row, _)| row.id == self.panes.changes.review.agent)
            && let Some((first, _)) = agents.first()
        {
            self.panes.changes.review.agent = first.id.clone();
        }

        let (mut send, mut ask) = (false, false);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Para").color(colors.muted));

            let shown =
                agents.iter().find(|(row, _)| row.id == self.panes.changes.review.agent).map_or("Nenhum agente".to_string(), |(row, _)| self.review_name(row));

            egui::ComboBox::from_id_salt("changes_review_agent").width(ui.available_width()).selected_text(shown).truncate().show_ui(ui, |ui| {
                for (row, inside) in &agents {
                    let mut text = self.review_name(row);

                    if row.spec.chat {
                        text += " (chat)";
                    }

                    if *inside {
                        text += " · nesta pasta";
                    }

                    ui.selectable_value(&mut self.panes.changes.review.agent, row.id.clone(), text).on_hover_text(&row.spec.cwd);
                }
            });
        });

        ui.horizontal(|ui| {
            let review = &self.panes.changes.review;
            let ready = !agents.is_empty() && (!review.comments.is_empty() || !review.note.trim().is_empty());

            send = ui.add_enabled(ready, egui::Button::new("Enviar revisão")).on_hover_text("Os comentários e a observação, numa mensagem ao agente").clicked();
            let changes = !agents.is_empty() && groups.iter().any(|g| !g.files.is_empty());

            ask = ui
                .add_enabled(changes, egui::Button::new("Pedir análise de PR"))
                .on_hover_text("O agente revisa todo o diff destas pastas, como num PR")
                .clicked();
        });

        if !live(ui) {
            return;
        }

        let agent = self.panes.changes.review.agent.clone();

        if send {
            let review = &self.panes.changes.review;
            let message = self.review_text(&review.comments, &review.note, groups);

            match self.send_to_agent(&agent, message) {
                Ok(said) => {
                    let review = &mut self.panes.changes.review;

                    (review.comments, review.note) = (Vec::new(), String::new());
                    self.toast(format!("Revisão enviada. {said}"));
                }
                Err(why) => self.toast(why),
            }
        }

        if ask {
            let message = self.pr_text(groups);

            match self.send_to_agent(&agent, message) {
                Ok(said) => self.toast(format!("Análise de PR pedida. {said}")),
                Err(why) => self.toast(why),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lines a comment shows: without the indentation they share, tabs as spaces, four at most and the rest counted.
    #[test]
    fn a_comments_lines_lose_their_shared_indentation() {
        let excerpt = "-            \"action\": { \"type\": \"string\" },\n+                \"enum\": [],\n \n+\t\t\tfim";
        let (lines, more) = excerpt_lines(excerpt);

        assert_eq!(lines[0], ('-', "\"action\": { \"type\": \"string\" },".to_string()));
        assert_eq!(lines[1], ('+', "    \"enum\": [],".to_string()));
        assert_eq!(lines[2], (' ', String::new()), "a blank line stays blank");
        assert_eq!(lines[3], ('+', "fim".to_string()), "a tab is four spaces");
        assert_eq!(more, 0);
        let long = "+ x\n".repeat(7);

        assert_eq!(excerpt_lines(&long), (vec![('+', "x".to_string()); 4], 3));
    }
}
