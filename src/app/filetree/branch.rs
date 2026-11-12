//! The branch chip of a tree's header: a menu of the local branches (the one checked out ticked) and the switch to another.

use crate::app::filetree::FileTreeSpec;
use crate::app::filetree::ui::Act;
use crate::app::ui::menu_look::menu_colors;
use crate::ide::run_git;
use crate::prelude::*;
use std::path::Path;

/// The local branches of the repository `root` is in, by name.
pub(crate) fn local_branches(root: &Path) -> Result<Vec<String>, String> {
    let out = run_git(root, &["for-each-ref", "--format=%(refname:short)", "--sort=refname", "refs/heads"])?;
    Ok(out.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
}

/// `git checkout branch`, with the reason in words when git refuses: changes that the switch would overwrite.
pub(crate) fn checkout(root: &Path, branch: &str) -> Result<String, String> {
    let branches = local_branches(root)?;
    if !branches.iter().any(|b| b == branch) {
        return Err(format!("O ramo \"{branch}\" não existe neste repositório."));
    }
    let current = run_git(root, &["symbolic-ref", "--short", "-q", "HEAD"]).map(|b| b.trim().to_string()).unwrap_or_default();
    if current == branch {
        return Ok(format!("Já está no ramo {branch}."));
    }
    run_git(root, &["checkout", branch, "--"]).map_err(|why| {
        if why.contains("overwritten") || why.contains("local changes") {
            format!("Não foi possível trocar para {branch}: há mudanças locais que seriam sobrescritas. Faça commit, guarde (stash) ou descarte antes.")
        } else {
            format!("Não foi possível trocar para {branch}: {}", why.lines().next().unwrap_or_default())
        }
    })?;
    Ok(format!("Ramo trocado para {branch}."))
}

/// lucide's git-branch, `size` across, centered at `c`.
fn branch_icon(p: &Painter, c: Pos2, size: f32, stroke: Stroke) {
    use crate::app::filetree::menu::arc;
    use crate::app::filetree::ui::outline;
    outline(p, c, size, &[(6.0, 3.0), (6.0, 15.0)], false, stroke);
    outline(p, c, size, &arc(18.0, 6.0, 3.0, 0.0, 360.0), true, stroke);
    outline(p, c, size, &arc(6.0, 18.0, 3.0, 0.0, 360.0), true, stroke);
    outline(p, c, size, &arc(9.0, 9.0, 9.0, 0.0, 90.0), false, stroke);
}

/// The branches `query` keeps (a part of the name, any case), the one checked out first.
pub(crate) fn branches_shown<'a>(branches: &'a [String], current: &str, query: &str) -> Vec<&'a String> {
    let q = query.trim().to_lowercase();
    let mut shown: Vec<&String> = branches.iter().filter(|b| q.is_empty() || b.to_lowercase().contains(&q)).collect();
    shown.sort_by_key(|b| b.as_str() != current);
    shown
}

/// The chip's picker, as the reference's: a search over the local branches (focused as it opens; Enter takes
/// the first other branch found), then the list, the one checked out first with its "atual" pill, every
/// other row a shade apart; a click switches.
pub(crate) fn branch_menu(ui: &mut egui::Ui, branches: &[String], current: &str, acts: &mut Vec<Act>, query: &mut String) {
    let k = menu_colors(ui.visuals().dark_mode);
    ui.set_width(300.0);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let field = egui::Frame::new()
        .fill(k.text.gamma_multiply(0.06))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (icon, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                paint_icon_scaled(ui.painter(), Icon::Search, icon.center(), k.muted, 0.75, 0.0);
                ui.add_space(6.0);
                ui.add(
                    egui::TextEdit::singleline(query)
                        .hint_text("Buscar ramo")
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::ZERO)
                        .align(egui::Align2::LEFT_CENTER)
                        .desired_width(f32::INFINITY)
                        .font(FontId::proportional(12.5)),
                )
            })
            .inner
        })
        .inner;
    if ui.memory(|m| m.focused().is_none()) {
        field.request_focus();
    }
    ui.add_space(6.0);
    let shown = branches_shown(branches, current, query);
    let mut pick = None;
    if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        pick = shown.iter().find(|b| b.as_str() != current).map(|b| (*b).clone());
    }
    if shown.is_empty() {
        let text = if branches.is_empty() { "Nenhum ramo local" } else { "Nenhum ramo com esse nome" };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::hover());
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, FontId::proportional(12.0), k.muted);
    }
    egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, true]).show(ui, |ui| {
        for (i, branch) in shown.iter().enumerate() {
            let on = branch.as_str() == current;
            let (rect, row) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), if on { Sense::hover() } else { Sense::click() });
            let row = if on { row } else { row.on_hover_cursor(CursorIcon::PointingHand) };
            let p = ui.painter();
            if row.hovered() && !on {
                p.rect_filled(rect, 4, k.hover);
            } else if i % 2 == 0 {
                p.rect_filled(rect, 4, k.text.gamma_multiply(0.03));
            }
            branch_icon(p, Pos2::new(rect.left() + 15.0, rect.center().y), 11.0, Stroke::new(1.3, k.muted));
            let pill = if on { p.layout_no_wrap("atual".into(), FontId::proportional(10.0), k.muted).size().x + 12.0 } else { 0.0 };
            let room = rect.width() - 28.0 - 10.0 - pill - if on { 6.0 } else { 0.0 };
            let name = crate::app::chat::list::clipped(p, (*branch).clone(), FontId::monospace(12.0), k.text, room);
            p.galley(Pos2::new(rect.left() + 28.0, rect.center().y - name.size().y / 2.0), name, k.text);
            if on {
                let chip = Rect::from_min_size(Pos2::new(rect.right() - 10.0 - pill, rect.center().y - 8.0), Vec2::new(pill, 16.0));
                p.rect_stroke(chip, 8, Stroke::new(1.0, k.border), StrokeKind::Inside);
                p.text(chip.center(), egui::Align2::CENTER_CENTER, "atual", FontId::proportional(10.0), k.muted);
            }
            if row.clicked() {
                pick = Some((*branch).clone());
            }
        }
    });
    let n = branches.len();
    ui.add_space(6.0);
    let (foot, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
    let count = if shown.len() == n { format!("{n} ramo{}", if n == 1 { "" } else { "s" }) } else { format!("{} de {n} ramos", shown.len()) };
    ui.painter().text(foot.left_center() + Vec2::new(4.0, 0.0), egui::Align2::LEFT_CENTER, count, FontId::proportional(11.0), k.muted);
    if let Some(branch) = pick {
        acts.push(Act::Branch(branch));
        query.clear();
        ui.close();
    }
}

impl DrawingApp {
    /// Switches tree `id`'s repository to `branch`; what git said (or the refusal) is the answer.
    pub(crate) fn ft_checkout(&mut self, id: &str, branch: &str) -> Result<String, String> {
        let root = self.ft_spec(id).map(FileTreeSpec::root_path).ok_or("árvore não encontrada")?;
        let done = checkout(&root, branch)?;
        self.ft_refresh(id);
        Ok(done)
    }

    /// The `file_tree` actions `branches` (the local ones, the current marked with `*`) and `checkout` (`name`).
    pub(crate) fn act_tree_branch(&mut self, id: &str, action: &str, name: &str) -> Result<String, String> {
        if action == "checkout" {
            return self.ft_checkout(id, name.trim());
        }
        let root = self.ft_spec(id).map(FileTreeSpec::root_path).ok_or("árvore não encontrada")?;
        let current = run_git(&root, &["symbolic-ref", "--short", "-q", "HEAD"]).map(|b| b.trim().to_string()).unwrap_or_default();
        let branches = local_branches(&root)?;
        Ok(branches.iter().map(|b| format!("{} {b}", if *b == current { "*" } else { " " })).collect::<Vec<_>>().join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_search_keeps_the_matching_branches_with_the_current_first() {
        let branches: Vec<String> = ["backup/port-ide", "main", "porte-referencia", "tui"].map(String::from).to_vec();
        let names = |q: &str| branches_shown(&branches, "porte-referencia", q).into_iter().cloned().collect::<Vec<_>>();
        assert_eq!(names(""), ["porte-referencia", "backup/port-ide", "main", "tui"]);
        assert_eq!(names(" PORT "), ["porte-referencia", "backup/port-ide"], "any case, spaces around ignored");
        assert!(names("nada").is_empty());
    }
}
