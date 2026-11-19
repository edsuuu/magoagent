//! Ajustes › Skills: the style skill the flow's Padronização step reads (`~/.magoagent/Skills/estilo/SKILL.md` at
//! first) and the skills the app hands the Claude Code it starts (maestro/skills.rs), listed from their folder.

use super::kit::*;
use crate::app::maestro::skills::{Skill, list, skills_path};
use crate::prelude::*;
use std::sync::{Arc, Mutex};

/// The skills of the folder, read on a thread each time the page opens (never on the frame).
type Listed = Arc<Mutex<Option<Vec<Skill>>>>;

impl DrawingApp {
    pub(super) fn settings_skills(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "SKILL DE ESTILO");
        self.style_skill_row(ui, k);
        divider(ui, k);
        title(ui, k, "SKILLS DOS AGENTES DO APP");
        note(ui, k, "O Claude Code que o app inicia recebe estas skills de uma pasta só do app: o seu ~/.claude fica como está.");
        ui.add_space(3.25);
        let folder = skills_path();

        match listed(ui, folder.as_deref()) {
            None => note(ui, k, "Lendo a pasta…"),
            Some(skills) if skills.is_empty() => note(ui, k, "Nenhuma ainda: cada skill é uma pasta dentro de skills/, com um SKILL.md."),
            Some(skills) => {
                for skill in skills {
                    row(ui, k, &skill.name, &skill.description, |_| ());
                }
            }
        }

        if let Some(folder) = folder {
            let shown = folder.join("skills").display().to_string();

            if row(ui, k, "Pasta das skills", &shown, |ui| button(ui, k, "Abrir").clicked())
                && let Err(why) = crate::app::maestro::skills::open(&folder)
            {
                self.toast(why);
            }
        }
    }

    /// "Skill de estilo": the file the flow's Padronização step reads, and Abrir to show it in a file tree on the canvas.
    fn style_skill_row(&mut self, ui: &mut egui::Ui, k: &K) {
        let path = &mut self.prefs.style_skill;
        let hint = "O SKILL.md com o seu estilo de código: a etapa Padronização do fluxo da Luna manda o agente lê-lo e conferir o diff regra a regra.";
        let mut open = false;

        row(ui, k, "Skill de estilo", hint, |ui| {
            open = button(ui, k, "Abrir").clicked();
            input(ui, k, egui::Id::unique("settings_style_skill"), "Skill de estilo", path, "~/.magoagent/Skills/estilo/SKILL.md", 230.0, false);
        });

        if open && let Err(why) = self.show_in_tree(&self.prefs.style_skill.clone()) {
            self.toast(why);
        }
    }
}

/// The skills of `folder` as read when the page last opened: None while the thread reads them.
fn listed(ui: &egui::Ui, folder: Option<&std::path::Path>) -> Option<Vec<Skill>> {
    let (id, seen) = (egui::Id::unique("settings_skills_listed"), egui::Id::unique("settings_skills_seen"));
    let pass = ui.ctx().cumulative_pass_nr();
    let before = ui.ctx().data_mut(|data| std::mem::replace(data.get_temp_mut_or(seen, 0u64), pass));
    let slot: Listed = ui.ctx().data_mut(|data| data.get_temp_mut_or_default::<Listed>(id).clone());

    // drawn the pass before: still open; a gap: it opened now, and the folder is read again
    if before == 0 || pass - before > 1 {
        *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        let (slot, folder, ctx) = (slot.clone(), folder.map(|folder| folder.to_path_buf()), ui.ctx().clone());

        std::thread::spawn(move || {
            let skills = folder.map(|folder| list(&folder)).unwrap_or_default();

            *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(skills);
            ctx.request_repaint();
        });
    }

    slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}
