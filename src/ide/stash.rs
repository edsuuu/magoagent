//! Stash: the list `git stash list` gives and the History panel's section to put changes aside
//! and bring them back (apply keeps the entry, pop removes it, drop asks first).

use super::branches::{Ask, GitOp};
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Stash {
    pub index: usize,
    pub message: String,
    pub date: String,
}

#[derive(Default)]
pub struct StashForm {
    pub message: String,
    pub untracked: bool,
}

/// `git stash list --format=%gd%x09%gs%x09%ci` lines (`stash@{0}`, subject, ISO date) -> entries.
/// (`--date` would turn `%gd` into the date, so the date is reshaped here.)
pub fn parse(out: &str) -> Vec<Stash> {
    out.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let index = f.next()?.strip_prefix("stash@{")?.strip_suffix('}')?.parse().ok()?;
            Some(Stash { index, message: f.next()?.to_string(), date: day(f.next().unwrap_or("")) })
        })
        .collect()
}

/// `2026-11-10 09:00:00 -0300` -> `10/11/2026 09:00`.
fn day(iso: &str) -> String {
    match (iso.get(0..4), iso.get(5..7), iso.get(8..10), iso.get(11..16)) {
        (Some(y), Some(m), Some(d), Some(hm)) => format!("{d}/{m}/{y} {hm}"),
        _ => iso.to_string(),
    }
}

pub fn describe(list: &[Stash]) -> String {
    if list.is_empty() {
        return "Nenhum stash.".into();
    }
    list.iter().map(|s| format!("stash@{{{}}}: {} ({})", s.index, s.message, s.date)).collect::<Vec<_>>().join("\n")
}

impl Ide {
    /// The History panel's Stash section: a form to put the changes aside, then the entries.
    pub(super) fn stash_section(&mut self, ui: &mut Ui) {
        let list = self.git.snap.stashes.clone();
        let title = if list.is_empty() { "Stash".to_string() } else { format!("Stash ({})", list.len()) };
        let mut op = None;
        egui::CollapsingHeader::new(egui::RichText::new(title).color(t::TEXT)).id_salt(self.id.with("stash")).default_open(!list.is_empty()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let w = (ui.available_width() - 8.0).max(60.0);
                ui.add(egui::TextEdit::singleline(&mut self.git.stash.message).hint_text("Mensagem do stash (opcional)").desired_width(w));
            });
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.checkbox(&mut self.git.stash.untracked, "Incluir não versionados");
                if ui.button("Guardar alterações").clicked() {
                    op = Some(GitOp::StashPush { message: self.git.stash.message.clone(), untracked: self.git.stash.untracked });
                }
            });
            if list.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Nenhum stash.").color(t::MUTED));
                });
            }
            for s in &list {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(egui::RichText::new(format!("stash@{{{}}} {}", s.index, s.message)).color(t::TEXT)).truncate());
                });
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(&s.date).color(t::SECONDARY).small());
                });
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    // the same words on every entry: the name says which one
                    let named = |r: egui::Response, what: &str| named(r, format!("{what} stash@{{{}}}", s.index));
                    if named(ui.button("Aplicar"), "Aplicar").on_hover_text("Aplica e mantém o stash na lista").clicked() {
                        op = Some(GitOp::StashApply(s.index));
                    }
                    if named(ui.button("Pop"), "Pop").on_hover_text("Aplica e remove o stash da lista").clicked() {
                        op = Some(GitOp::StashPop(s.index));
                    }
                    if named(ui.button("Descartar"), "Descartar").clicked() {
                        self.git.ask = Some(Ask::StashDrop(s.index));
                    }
                });
            }
        });
        if let Some(op) = op {
            let pushed = matches!(op, GitOp::StashPush { .. });
            match self.git_op(op) {
                Ok(()) if pushed => self.git.stash.message.clear(),
                Ok(()) => {}
                Err(why) => self.notify(why),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::git::git;
    use super::gittest::*;
    use super::*;

    #[test]
    fn the_list_parses() {
        let v = parse("stash@{0}\tOn main: um\t2026-11-10 09:00:12 -0300\nstash@{1}\tWIP on main: abc base\t2026-11-09 18:30:00 +0000\nlixo\n");
        assert_eq!(v.len(), 2);
        assert_eq!((v[1].index, v[1].message.as_str(), v[1].date.as_str()), (1, "WIP on main: abc base", "09/11/2026 18:30"));
        assert_eq!(describe(&[]), "Nenhum stash.");
    }

    #[test]
    fn changes_are_stashed_applied_popped_and_dropped() {
        let dir = repo("stash");
        let mut ide = ide(&dir);
        // a clean tree has nothing to put aside; untracked files only when asked
        assert!(ide.git_op(GitOp::StashPush { message: String::new(), untracked: false }).unwrap_err().contains("Nada para guardar"));
        std::fs::write(dir.join("novo.txt"), "n\n").unwrap();
        ide.git_refresh();
        settle(&mut ide);
        assert!(ide.git_op(GitOp::StashPush { message: String::new(), untracked: false }).is_err());
        std::fs::write(dir.join("a.txt"), "um\nMUDOU\ntres\n").unwrap();
        ide.git_refresh();
        settle(&mut ide);
        run(&mut ide, GitOp::StashPush { message: "minha ideia".into(), untracked: true }).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "um\ndois\ntres\n");
        assert!(!dir.join("novo.txt").exists() && ide.git.changes.is_empty());
        assert_eq!(ide.git.snap.stashes.len(), 1);
        assert!(ide.git.snap.stashes[0].message.contains("minha ideia"), "{:?}", ide.git.snap.stashes);
        // apply keeps the entry; pop removes it
        run(&mut ide, GitOp::StashApply(0)).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "um\nMUDOU\ntres\n");
        assert_eq!(ide.git.snap.stashes.len(), 1);
        git(&dir, &["checkout", "--", "a.txt"]).unwrap();
        git(&dir, &["clean", "-fq"]).unwrap();
        run(&mut ide, GitOp::StashPop(0)).unwrap();
        assert!(dir.join("novo.txt").exists() && ide.git.snap.stashes.is_empty());
        assert!(ide.git_op(GitOp::StashPop(0)).unwrap_err().contains("Não há o stash 0"));
        // a second one is dropped on the MCP's confirmation, never without it
        run(&mut ide, GitOp::StashPush { message: String::new(), untracked: true }).unwrap();
        let ask = |ide: &mut Ide, confirm: bool| ide.agent_git(&serde_json::json!({ "action": "stash_drop", "index": 0, "confirm": confirm }));
        assert!(ask(&mut ide, false).is_err());
        assert_eq!(ide.git.snap.stashes.len(), 1);
        ask(&mut ide, true).unwrap();
        settle(&mut ide);
        assert!(ide.git.snap.stashes.is_empty());
        let said = ide.agent_git(&serde_json::json!({ "action": "stash_list" })).unwrap();
        assert_eq!(said, "Nenhum stash.");
        drop(ide);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
