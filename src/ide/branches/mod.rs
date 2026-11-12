//! Branches, remotes and merge: the branch popup of the toolbar, the questions it asks (a new
//! branch, a delete, a merge) and the git operations behind them, which the MCP's `ide_git`
//! (`agent_git`) runs too. Stash is in stash.rs, the log in history.rs. Nothing here forces a
//! push or resets hard: what loses work asks first and the rest is left to git's own refusals.

use super::git::{Status, git};
use super::stash::Stash;
use super::*;
mod plan;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BranchInfo {
    /// `main` for a local branch, `origin/main` for a remote one
    pub name: String,
    pub remote: bool,
    pub current: bool,
    pub upstream: String,
    pub ahead: u32,
    pub behind: u32,
}

/// What a git refresh learns beyond the changed files.
#[derive(Default)]
pub struct Snapshot {
    pub branches: Vec<BranchInfo>,
    pub remotes: Vec<String>,
    /// a merge stopped before its commit (conflicts, or waiting for the commit)
    pub merging: bool,
    pub stashes: Vec<Stash>,
}

impl Snapshot {
    pub fn read(root: &Path) -> Self {
        let refs =
            git(root, &["for-each-ref", "--format=%(HEAD)%09%(refname)%09%(upstream:short)%09%(upstream:track,nobracket)", "refs/heads", "refs/remotes"]);
        let list = |args: &[&str]| git(root, args).unwrap_or_default();
        Snapshot {
            branches: parse_refs(&refs.unwrap_or_default()),
            remotes: list(&["remote"]).lines().map(str::trim).filter(|r| !r.is_empty()).map(String::from).collect(),
            merging: git(root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok(),
            stashes: super::stash::parse(&list(&["stash", "list", "--format=%gd%x09%gs%x09%ci"])),
        }
    }

    pub fn current(&self) -> Option<&BranchInfo> {
        self.branches.iter().find(|b| b.current && !b.remote)
    }

    pub fn locals(&self) -> impl Iterator<Item = &BranchInfo> {
        self.branches.iter().filter(|b| !b.remote)
    }

    pub fn has_local(&self, name: &str) -> bool {
        self.locals().any(|b| b.name == name)
    }
}

/// `for-each-ref` lines (HEAD mark, full ref, upstream, track) -> branches; `origin/HEAD` is not one.
pub fn parse_refs(out: &str) -> Vec<BranchInfo> {
    let mut v = vec![];
    for line in out.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let [head, full, upstream, track] = f[..] else { continue };
        let (name, remote) = match (full.strip_prefix("refs/heads/"), full.strip_prefix("refs/remotes/")) {
            (Some(n), _) => (n, false),
            (_, Some(n)) if !n.ends_with("/HEAD") => (n, true),
            _ => continue,
        };
        let count = |word: &str| track.split(", ").find_map(|p| p.strip_prefix(word)?.trim().parse().ok()).unwrap_or(0);
        v.push(BranchInfo { name: name.into(), remote, current: head == "*", upstream: upstream.into(), ahead: count("ahead "), behind: count("behind ") });
    }
    v
}

/// Why `name` can't be a branch's name, in the words the question shows; None when it can.
pub fn name_problem(name: &str) -> Option<&'static str> {
    let bad = name.is_empty()
        || name.starts_with(['-', '/', '.'])
        || name.ends_with(['/', '.'])
        || name.ends_with(".lock")
        || name == "@"
        || ["..", "//", "/.", "@{"].iter().any(|p| name.contains(p))
        || name.contains(|c: char| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c));
    bad.then_some("Nome de ramo inválido: sem espaços, sem «..» e sem ~ ^ : ? * [ \\")
}

/// A failed git run in plain pt-BR (git's messages are asked in English, see `git_command`); the
/// last line of what git said when it is none of the usual ones.
pub fn explain(err: &str) -> String {
    let has = |p: &str| err.contains(p);
    let conflicts = err.matches("CONFLICT").count();
    if conflicts > 0 || has("Automatic merge failed") {
        let n = if conflicts == 1 { "1 arquivo".to_string() } else { format!("{} arquivos", conflicts.max(1)) };
        return format!("parou com conflitos ({n}). Resolva-os no editor, a lista está no painel Histórico");
    }
    let said = if has("no upstream branch") || has("has no upstream") {
        "este ramo ainda não está no remoto. Publique-o primeiro"
    } else if has("No configured push destination") || has("No remote repository specified") || has("No such remote") {
        "este repositório não tem remoto"
    } else if has("non-fast-forward") || has("fetch first") || has("[rejected]") {
        "o remoto recusou o push: ele tem commits que você ainda não tem. Faça o pull primeiro"
    } else if has("Authentication failed") || has("terminal prompts disabled") || has("Permission denied") || has("could not read Username") {
        "o acesso ao remoto foi recusado ou pede senha. Entre pelo terminal com suas credenciais e tente de novo"
    } else if has("not fully merged") {
        "este ramo tem commits que nenhum outro tem; excluí-lo perderia esse trabalho. A exclusão forçada o remove mesmo assim"
    } else if has("would be overwritten") || has("commit your changes or stash them") || has("have your local changes") {
        "há alterações não commitadas no caminho. Faça o commit ou o stash primeiro"
    } else if has("Not possible to fast-forward") || has("Diverging branches") {
        "seu ramo e o remoto divergiram: o pull só avança em linha reta. Mescle o ramo do remoto neste"
    } else if has("index.lock") {
        "outro processo está usando este repositório. Espere ele terminar"
    } else if has("refusing to merge unrelated histories") {
        "os dois ramos não têm história em comum"
    } else if has("already exists") {
        "esse nome já existe"
    } else if has("is not a commit") || has("not a valid") || has("unknown revision") || has("did not match any") {
        "não achei esse ramo ou commit neste repositório"
    } else {
        return err.lines().last().unwrap_or(err).to_string();
    };
    said.to_string()
}

/// A git operation of the branch popup, the History panel and the MCP.
#[derive(Clone, Debug, PartialEq)]
pub enum GitOp {
    Fetch,
    Pull,
    Push,
    Publish,
    /// a local branch, or a remote one (`origin/x`, or just `x`) that becomes a local one
    Checkout(String),
    CreateBranch {
        name: String,
        from: String,
        checkout: bool,
    },
    DeleteBranch {
        name: String,
        force: bool,
    },
    Merge(String),
    MergeAbort,
    MergeFinish,
    StashPush {
        message: String,
        untracked: bool,
    },
    StashApply(usize),
    StashPop(usize),
    StashDrop(usize),
}

/// What the person is asked before a `GitOp`.
pub enum Ask {
    NewBranch { from: String, name: String, checkout: bool },
    Delete { name: String, force: bool },
    Merge { src: String },
    MergeAbort,
    StashDrop(usize),
}

fn cmd(v: &[&str]) -> Vec<String> {
    v.iter().map(|a| a.to_string()).collect()
}

fn not_option(name: &str) -> Result<(), String> {
    if name.is_empty() || name.starts_with('-') { Err(format!("«{name}» não é um ramo nem um commit")) } else { Ok(()) }
}

impl Ide {
    /// The toolbar's branch label: its name and, when it is ahead or behind its upstream, by how much.
    pub(crate) fn branch_label(&self) -> String {
        let b = self.git.snap.current();
        let mut label = self.branch.clone();
        if let Some(b) = b {
            if b.behind > 0 {
                label += &format!(" ↓{}", b.behind);
            }
            if b.ahead > 0 {
                label += &format!(" ↑{}", b.ahead);
            }
        }
        label
    }

    /// The toolbar's branch popup: project actions, a new branch, and local and remote branches.
    pub(crate) fn branch_menu(&mut self, ui: &mut Ui) {
        ui.set_min_width(300.0);
        let mut pick: Option<Pick> = None;
        let current = self.branch.clone();
        let snap = &self.git.snap;
        let publish = snap.current().is_some_and(|b| b.upstream.is_empty()) && !snap.remotes.is_empty();
        if snap.merging {
            ui.label(egui::RichText::new("Merge em andamento").color(t::ERROR));
            if ui.button("Ver conflitos e concluir").clicked() {
                pick = Some(Pick::History);
            }
            ui.separator();
        }
        if menu_item(ui, "Atualizar projeto…", "Ctrl+T") {
            pick = Some(Pick::Op(GitOp::Pull));
        }
        if menu_item(ui, "Commit…", "Ctrl+K") {
            pick = Some(Pick::Commit);
        }
        if menu_item(ui, "Push…", "Ctrl+Shift+K") {
            pick = Some(Pick::Op(GitOp::Push));
        }
        if ui.button("Buscar (fetch)").clicked() {
            pick = Some(Pick::Op(GitOp::Fetch));
        }
        if publish && ui.button("Publicar ramo").clicked() {
            pick = Some(Pick::Op(GitOp::Publish));
        }
        ui.separator();
        if ui.button(format!("Novo ramo a partir de {current}…")).clicked() {
            pick = Some(Pick::Ask(Ask::NewBranch { from: current.clone(), name: String::new(), checkout: true }));
        }
        if ui.button("Histórico, merge e stash…").clicked() {
            pick = Some(Pick::History);
        }
        ui.separator();
        ui.add(egui::TextEdit::singleline(&mut self.git.filter).hint_text("Buscar ramos…").desired_width(f32::INFINITY));
        let filter = self.git.filter.trim().to_lowercase();
        let branches: Vec<BranchInfo> = self.git.snap.branches.iter().filter(|b| b.name.to_lowercase().contains(&filter)).cloned().collect();
        egui::ScrollArea::vertical().max_height(300.0).auto_shrink([true, true]).show(ui, |ui| {
            for remote in [false, true] {
                let group: Vec<&BranchInfo> = branches.iter().filter(|b| b.remote == remote).collect();
                if group.is_empty() {
                    continue;
                }
                ui.label(egui::RichText::new(if remote { "Remotos" } else { "Locais" }).color(t::SECONDARY).small());
                for b in group {
                    let mut label = if b.current { format!("★ {}", b.name) } else { b.name.clone() };
                    if b.behind > 0 {
                        label += &format!("  ↓{}", b.behind);
                    }
                    if b.ahead > 0 {
                        label += &format!("  ↑{}", b.ahead);
                    }
                    ui.menu_button(label, |ui| {
                        if !b.current && ui.button(format!("Trocar para {}", b.name)).clicked() {
                            pick = Some(Pick::Op(GitOp::Checkout(b.name.clone())));
                        }
                        if ui.button(format!("Novo ramo a partir de {}…", b.name)).clicked() {
                            pick = Some(Pick::Ask(Ask::NewBranch { from: b.name.clone(), name: String::new(), checkout: true }));
                        }
                        if !b.current && ui.button(format!("Mesclar {} em {current}…", b.name)).clicked() {
                            pick = Some(Pick::Ask(Ask::Merge { src: b.name.clone() }));
                        }
                        if !b.remote && !b.current && ui.button(format!("Excluir {}…", b.name)).clicked() {
                            pick = Some(Pick::Ask(Ask::Delete { name: b.name.clone(), force: false }));
                        }
                    });
                }
            }
        });
        let Some(pick) = pick else { return };
        ui.close();
        self.git.filter.clear();
        match pick {
            Pick::Op(op) => self.git_op_or_say(op),
            Pick::Ask(ask) => self.git.ask = Some(ask),
            Pick::History => self.open_history(),
            Pick::Commit => self.left = Some(Left::Commit),
        }
    }

    /// The open question's window: Enter answers the harmless ones, a destructive one wants its button.
    pub(crate) fn git_ask_ui(&mut self, ctx: &egui::Context) {
        let current = self.branch.clone();
        let Some(ask) = &mut self.git.ask else { return };
        let snap = &self.git.snap;
        let mut answer = None;
        let modal = egui::Modal::new(self.id.with("git_ask")).show(ctx, |ui| {
            t::style(ui.style_mut());
            ui.set_width(360.0);
            let (ok, label, enter) = match ask {
                Ask::NewBranch { from, name, checkout } => {
                    ui.label(egui::RichText::new("Novo ramo").strong());
                    ui.label(format!("A partir de {from}"));
                    let field_id = self.id.with("git_ask_name");
                    let field = egui::TextEdit::singleline(name).id(field_id).hint_text("Nome do novo ramo").desired_width(f32::INFINITY).show(ui);
                    if name.is_empty() && !field.response.has_focus() {
                        field.response.request_focus();
                    }
                    ui.checkbox(checkout, "Trocar para o novo ramo");
                    let problem = if name.is_empty() {
                        None
                    } else if snap.has_local(name.trim()) {
                        Some("Já existe um ramo com esse nome.")
                    } else {
                        name_problem(name.trim())
                    };
                    if let Some(why) = problem {
                        ui.colored_label(t::ERROR, why);
                    }
                    (problem.is_none() && !name.trim().is_empty(), "Criar", true)
                }
                Ask::Delete { name, force } => {
                    ui.label(egui::RichText::new("Excluir ramo?").strong());
                    ui.label(format!("Isso exclui o ramo local {name}."));
                    ui.checkbox(force, "Forçar: perde os commits que nenhum outro ramo tem");
                    (true, "Excluir", false)
                }
                Ask::Merge { src } => {
                    ui.label(egui::RichText::new(format!("Mesclar {src} em {current}?")).strong());
                    ui.label("O git vai incorporar esse ramo neste. Se houver conflitos, os arquivos ficam marcados para você resolver no editor.");
                    (true, "Mesclar", true)
                }
                Ask::MergeAbort => {
                    ui.label(egui::RichText::new("Interromper o merge?").strong());
                    ui.label("O merge é desfeito e os arquivos voltam como estavam antes dele. O que você resolveu nele se perde.");
                    (true, "Interromper o merge", false)
                }
                Ask::StashDrop(i) => {
                    ui.label(egui::RichText::new("Descartar este stash?").strong());
                    ui.label(format!("stash@{{{i}}} será jogado fora. O git não consegue trazer de volta."));
                    (true, "Descartar", false)
                }
            };
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.add_enabled(ok, egui::Button::new(label)).clicked() || (ok && enter && ui.input(|i| i.key_pressed(Key::Enter))) {
                    answer = Some(true);
                }
                if ui.button("Cancelar").clicked() {
                    answer = Some(false);
                }
            });
        });
        if modal.should_close() {
            answer.get_or_insert(false);
        }
        let Some(go) = answer else { return };
        let Some(ask) = self.git.ask.take() else { return };
        if !go {
            return;
        }
        self.git_op_or_say(match ask {
            Ask::NewBranch { from, name, checkout } => GitOp::CreateBranch { name: name.trim().to_string(), from, checkout },
            Ask::Delete { name, force } => GitOp::DeleteBranch { name, force },
            Ask::Merge { src } => GitOp::Merge(src),
            Ask::MergeAbort => GitOp::MergeAbort,
            Ask::StashDrop(i) => GitOp::StashDrop(i),
        });
    }

    /// The MCP's other `ide_git` actions (branches, fetch, publish, merge, stash, log, show). What
    /// loses work wants `confirm: true`, which the agent gives after asking the person; none
    /// of them forces a push or resets hard. Reads answer now, the rest starts and shows in the IDE.
    pub(crate) fn agent_git(&mut self, args: &serde_json::Value) -> Result<String, String> {
        let text = |k: &str| args[k].as_str().unwrap_or("").trim().to_string();
        let (action, name) = (text("action"), text("name"));
        let index = args["index"].as_u64().unwrap_or(0) as usize;
        let root = self.root.clone().ok_or("o IDE não está numa pasta")?;
        let need = |v: &str, what: &str| if v.is_empty() { Err(format!("diga {what}")) } else { Ok(v.to_string()) };
        let confirm = |what: &str| {
            if args["confirm"].as_bool().unwrap_or(false) {
                Ok(())
            } else {
                Err(format!("{what} perde trabalho: pergunte à pessoa e repita com confirm: true"))
            }
        };
        let op = match action.as_str() {
            "branches" => return Ok(describe(&Snapshot::read(&root))),
            "stash_list" => return Ok(super::stash::describe(&Snapshot::read(&root).stashes)),
            "log" => return super::history::agent_log(&root, args["limit"].as_u64().unwrap_or(20).clamp(1, 200) as usize),
            "show" => return self.agent_show(&name, &text("path")),
            "fetch" => GitOp::Fetch,
            "publish" => GitOp::Publish,
            "checkout" => GitOp::Checkout(need(&name, "o ramo (name)")?),
            "branch_create" => {
                GitOp::CreateBranch { name: need(&name, "o nome do ramo (name)")?, from: text("from"), checkout: args["checkout"].as_bool().unwrap_or(true) }
            }
            "branch_delete" => {
                confirm("Excluir um ramo")?;
                GitOp::DeleteBranch { name: need(&name, "o ramo (name)")?, force: false }
            }
            "merge" => GitOp::Merge(need(&name, "o ramo a mesclar (name)")?),
            "merge_abort" => {
                confirm("Interromper o merge")?;
                GitOp::MergeAbort
            }
            "merge_finish" => GitOp::MergeFinish,
            "stash" => GitOp::StashPush { message: text("message"), untracked: args["untracked"].as_bool().unwrap_or(false) },
            "stash_apply" => GitOp::StashApply(index),
            "stash_pop" => GitOp::StashPop(index),
            "stash_drop" => {
                confirm("Descartar um stash")?;
                GitOp::StashDrop(index)
            }
            other => return Err(format!("ação desconhecida: {other}")),
        };
        let started = format!("{action} começou no IDE; o resultado aparece nele.");
        self.git_op(op).map(|()| started)
    }
}

/// The `ide_git` actions `agent_git` answers (the older ones are in app/actions.rs).
pub(crate) fn more_git_action(action: &str) -> bool {
    [
        "branches",
        "fetch",
        "publish",
        "checkout",
        "branch_create",
        "branch_delete",
        "merge",
        "merge_abort",
        "merge_finish",
        "stash",
        "stash_list",
        "stash_apply",
        "stash_pop",
        "stash_drop",
        "log",
        "show",
    ]
    .contains(&action)
}

/// A line starting with a conflict marker.
fn has_markers(text: &str) -> bool {
    text.lines().any(|l| l.starts_with("<<<<<<< ") || l.starts_with(">>>>>>> ") || l == "=======")
}

fn describe(snap: &Snapshot) -> String {
    let names = |remote: bool| {
        let v: Vec<String> = snap
            .branches
            .iter()
            .filter(|b| b.remote == remote)
            .map(|b| match (b.current, b.ahead, b.behind) {
                (true, a, d) => format!("{} (atual, à frente {a}, atrás {d})", b.name),
                _ => b.name.clone(),
            })
            .collect();
        if v.is_empty() { "nenhum".to_string() } else { v.join(", ") }
    };
    let current = snap.current().map_or("HEAD solto".to_string(), |b| {
        if b.upstream.is_empty() { format!("{} (não publicado)", b.name) } else { format!("{} → {}", b.name, b.upstream) }
    });
    format!(
        "Ramo atual: {current}\nLocais: {}\nRemotos: {}\nRemotos configurados: {}\nMerge em andamento: {}",
        names(false),
        names(true),
        if snap.remotes.is_empty() { "nenhum".into() } else { snap.remotes.join(", ") },
        if snap.merging { "sim" } else { "não" }
    )
}

enum Pick {
    Op(GitOp),
    Ask(Ask),
    History,
    Commit,
}
