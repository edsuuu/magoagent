use super::*;

impl Ide {
    /// The label (what the status line says while it runs) and the git commands of `op`, or why not.
    pub(super) fn plan(&self, op: &GitOp) -> Result<(String, Vec<Vec<String>>), String> {
        let snap = &self.git.snap;
        let current = snap.current().map(|b| b.name.clone());
        let remote = || snap.remotes.first().map(|_| ()).ok_or("Este repositório não tem remoto: adicione um com git remote add e haverá para onde publicar.");
        let stash = |i: usize| snap.stashes.iter().any(|s| s.index == i).then(|| format!("stash@{{{i}}}")).ok_or(format!("Não há o stash {i}."));
        Ok(match op {
            GitOp::Fetch => {
                remote()?;
                ("Buscar".into(), vec![cmd(&["fetch", "--all", "--prune"])])
            }
            GitOp::Pull => ("Atualizar projeto".into(), vec![cmd(&["pull", "--ff-only"])]),
            GitOp::Push => ("Push".into(), vec![cmd(&["push"])]),
            GitOp::Publish => {
                remote()?;
                let b = snap.current().ok_or("HEAD solto: troque ou crie um ramo antes de publicar.")?;
                if !b.upstream.is_empty() {
                    return Err(format!("{} já está publicado em {}.", b.name, b.upstream));
                }
                let to = snap.remotes.iter().find(|r| *r == "origin").unwrap_or(&snap.remotes[0]);
                ("Publicar ramo".into(), vec![cmd(&["push", "-u", to, &b.name])])
            }
            GitOp::Checkout(name) => {
                not_option(name)?;
                if current.as_deref() == Some(name) {
                    return Err(format!("Você já está em {name}."));
                }
                if snap.has_local(name) {
                    (format!("Checkout {name}"), vec![cmd(&["checkout", name, "--"])])
                } else {
                    // a remote branch: exactly, or by its short name when only one remote has it
                    let mut found = snap.branches.iter().filter(|b| b.remote && (b.name == *name || b.name.split_once('/').is_some_and(|(_, n)| n == name)));
                    let (first, second) = (found.next(), found.next());
                    let b = first
                        .filter(|_| second.is_none())
                        .ok_or(format!("Não achei o ramo {name}, ou há mais de um com esse nome nos remotos: diga qual (origem/nome)."))?;
                    let short = b.name.split_once('/').map_or(b.name.as_str(), |(_, n)| n);
                    if snap.has_local(short) {
                        (format!("Checkout {short}"), vec![cmd(&["checkout", short, "--"])])
                    } else {
                        (format!("Checkout {short}"), vec![cmd(&["checkout", "--track", &b.name])])
                    }
                }
            }
            GitOp::CreateBranch { name, from, checkout } => {
                if let Some(why) = name_problem(name) {
                    return Err(why.into());
                }
                if snap.has_local(name) {
                    return Err(format!("Já existe um ramo chamado {name}."));
                }
                if !from.is_empty() {
                    not_option(from)?;
                }
                let mut c = if *checkout { cmd(&["checkout", "-b", name]) } else { cmd(&["branch", name]) };
                c.extend(Some(from.clone()).filter(|f| !f.is_empty()));
                (format!("Criar ramo {name}"), vec![c])
            }
            GitOp::DeleteBranch { name, force } => {
                if !snap.has_local(name) {
                    return Err(format!("{name} não é um ramo local: só os locais podem ser excluídos."));
                }
                if current.as_deref() == Some(name) {
                    return Err("Não dá para excluir o ramo em que você está: troque de ramo antes.".into());
                }
                (format!("Excluir ramo {name}"), vec![cmd(&["branch", if *force { "-D" } else { "-d" }, name])])
            }
            GitOp::Merge(src) => {
                not_option(src)?;
                if snap.merging {
                    return Err("Já há um merge em andamento: conclua ou interrompa antes.".into());
                }
                if current.as_deref() == Some(src) {
                    return Err("Não dá para mesclar um ramo nele mesmo.".into());
                }
                if !snap.branches.iter().any(|b| b.name == *src) {
                    return Err(format!("Não achei o ramo {src}."));
                }
                (format!("Mesclar {src}"), vec![cmd(&["merge", "--no-edit", src])])
            }
            GitOp::MergeAbort => {
                if !snap.merging {
                    return Err("Não há merge em andamento.".into());
                }
                ("Interromper merge".into(), vec![cmd(&["merge", "--abort"])])
            }
            GitOp::MergeFinish => {
                if !snap.merging {
                    return Err("Não há merge em andamento.".into());
                }
                let root = self.root.clone().unwrap_or_default();
                let files: Vec<String> = self
                    .git
                    .changes
                    .iter()
                    .filter(|c| c.status == Status::Conflict)
                    .map(|c| c.path.strip_prefix(&root).unwrap_or(&c.path).to_string_lossy().replace('\\', "/"))
                    .collect();
                // a file still carrying the markers is not resolved: the merge would commit them
                let marked: Vec<&String> = files.iter().filter(|f| has_markers(&std::fs::read_to_string(root.join(f)).unwrap_or_default())).collect();
                if !marked.is_empty() {
                    return Err(format!("Ainda há marcas de conflito em: {}.", marked.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(", ")));
                }
                let mut cmds = vec![];
                if !files.is_empty() {
                    cmds.push(cmd(&["add", "-A", "--"]).into_iter().chain(files).collect());
                }
                cmds.push(cmd(&["commit", "--no-edit"]));
                ("Concluir merge".into(), cmds)
            }
            GitOp::StashPush { message, untracked } => {
                if !self.git.changes.iter().any(|c| *untracked || c.status != Status::Untracked) {
                    return Err("Nada para guardar. O diretório de trabalho está limpo.".into());
                }
                let mut c = cmd(&["stash", "push"]);
                if *untracked {
                    c.push("-u".into());
                }
                if !message.trim().is_empty() {
                    c.extend(["-m".into(), message.trim().into()]);
                }
                ("Guardar stash".into(), vec![c])
            }
            GitOp::StashApply(i) => ("Aplicar stash".into(), vec![vec!["stash".into(), "apply".into(), stash(*i)?]]),
            GitOp::StashPop(i) => ("Pop do stash".into(), vec![vec!["stash".into(), "pop".into(), stash(*i)?]]),
            GitOp::StashDrop(i) => ("Descartar stash".into(), vec![vec!["stash".into(), "drop".into(), stash(*i)?]]),
        })
    }

    /// Runs `op` in the background, the tabs saved first. Refused while another git command runs.
    pub(crate) fn git_op(&mut self, op: GitOp) -> Result<(), String> {
        if self.root.is_none() {
            return Err("O IDE não está numa pasta.".into());
        }
        if !self.git.repo {
            return Err("Esta pasta não é um repositório Git.".into());
        }
        if let Some(busy) = &self.git.busy {
            return Err(format!("O git do IDE está ocupado ({busy}): espere terminar."));
        }
        self.save_all();
        let (label, cmds) = self.plan(&op)?;
        self.git_run(&label, cmds);
        Ok(())
    }

    /// `git_op` for a click: a refusal goes to the balloon.
    pub(in super::super) fn git_op_or_say(&mut self, op: GitOp) {
        if let Err(why) = self.git_op(op) {
            self.notify(why);
        }
    }
}
