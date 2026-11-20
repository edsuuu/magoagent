use super::flow::{Flow, MAX_FAILS, Move, STAGES, brief};
use super::*;
use crate::app::maestro::binder;

/// Joins `branches` into the branch checked out in `folder` (`git merge --no-ff`). A branch that doesn't join (a conflict,
/// or it doesn't exist) is aborted and the folder goes back to where it was before the first one, so nothing is left half
/// done; the error says why, with the files in conflict. The branches themselves stay, whatever happens.
pub(crate) fn merge_branches(folder: &Path, branches: &[String]) -> Result<(), String> {
    let git = crate::editor::run_git;

    if !git(folder, &["status", "--porcelain", "-uno"])?.trim().is_empty() {
        return Err(format!("{} tem mudanças não salvas no git: faça o commit antes de juntar os ramos.", folder.display()));
    }

    let head = git(folder, &["rev-parse", "HEAD"])?.trim().to_string();

    for branch in branches {
        let Err(why) = git(folder, &["merge", "--no-ff", "-m", &format!("Junta {branch}"), branch]) else { continue };
        let conflicts = git(folder, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
        // the folder was clean, so going back to `head` loses nothing
        let _ = git(folder, &["merge", "--abort"]);
        let _ = git(folder, &["reset", "--hard", &head]);
        let files = conflicts.lines().collect::<Vec<_>>().join(", ");

        return Err(if files.is_empty() { format!("o git não juntou {branch}: {why}") } else { format!("conflito ao juntar {branch} em: {files}") });
    }

    Ok(())
}

impl DrawingApp {
    /// The MCP's `flow`: start, advance, status.
    pub(crate) fn act_flow(&mut self, args: &Value) -> Result<String, String> {
        self.tasks_tick();

        match args["action"].as_str().unwrap_or_default().trim() {
            "start" => self.flow_start(args),
            "advance" => self.flow_advance(args),
            "status" | "" => self.flow_status(args["id"].as_str().unwrap_or_default().trim()),
            other => Err(format!("action é start, advance ou status, não {other}.")),
        }
    }

    fn binder_folder(&self) -> Result<PathBuf, String> {
        self.autosave.as_ref().map(|p| p.with_file_name("fichario")).ok_or_else(|| "Sem pasta de dados: o fichário não tem onde ficar.".into())
    }

    /// The project's name as a note's: that of the workspace the task was made in.
    fn flow_project(&self, task: &Task) -> String {
        let space = self.spaces.list.iter().find(|w| w.id == task.workspace).or_else(|| self.current_space());

        space.and_then(|w| binder::note_file(&w.name)).map_or("projeto".into(), |file| file.trim_end_matches(".md").to_string())
    }

    fn flow_brief(&self, task: &Task, flow: &Flow) -> String {
        match flow.current() {
            Some(_) => brief(flow.stage, &task.id, &self.flow_project(task), &self.prefs.style_skill, &flow.folder),
            None => String::new(),
        }
    }

    fn flow_start(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();

        let folder = match text("folder") {
            folder if !folder.is_empty() => folder,
            _ => self.current_space().map(|w| w.root.clone()).unwrap_or_default(),
        };

        self.task_create(&json!({ "title": text("title"), "description": text("description") }), "Luna")?;
        let i = self.tasks.list.len() - 1;
        let flow = Flow::new(&folder, now_ms());
        let task = &mut self.tasks.list[i];

        (task.state, task.flow) = (flow.state(), Some(flow.clone()));
        let task = task.clone();

        self.task_event(&task, "fluxo iniciado: etapa 1 (Planejar)");
        self.save_tasks();
        Ok(format!("Fluxo {} iniciado.\n\n{}", task.id, self.flow_brief(&task, &flow)))
    }

    fn flow_advance(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let i = self.task_index(&text("id"))?;
        let mut task = self.tasks.list[i].clone();
        let mut flow = task.flow.clone().ok_or_else(|| format!("A tarefa {} não é um fluxo (use flow start).", task.id))?;

        let stage = flow
            .current()
            .filter(|_| !task.state.is_final())
            .ok_or_else(|| format!("O fluxo {} já terminou ({}).", task.id, task.state.label().to_lowercase()))?;

        let ok = match text("result").as_str() {
            "ok" => true,
            "fail" => false,
            _ => return Err("Diga result: ok ou fail.".into()),
        };

        let note = text("note");
        let note_name = format!("{}-{}", task.id, stage.code);

        if ok {
            // the stage's note is its output and the next one's input
            if binder::read(&self.binder_folder()?, &note_name).is_err() {
                return Err(format!(
                    "Falta a nota `{note_name}` no fichário: grave o resultado da etapa {} ({}) com binder write e chame advance de novo.",
                    flow.stage + 1,
                    stage.name
                ));
            }
        } else if note.is_empty() {
            return Err("Diga o motivo da falha (note).".into());
        }

        let owner = match text("owner") {
            owner if owner.is_empty() => None,
            owner => Some(self.agent_named(&owner)?),
        };

        if !text("folder").is_empty() {
            flow.folder = text("folder");
        }

        for branch in strings(&args["branches"]).unwrap_or_default() {
            if !flow.branches.contains(&branch) {
                flow.branches.push(branch);
            }
        }

        // after the person's approval, the agents' branches join the project's
        let mut joined = String::new();

        if ok && flow.stage + 1 == STAGES.len() && !flow.branches.is_empty() {
            let folder = crate::app::chat::direct::folder(&flow.folder).ok_or("Sem pasta do projeto: diga folder, o repositório onde juntar os ramos.")?;

            match merge_branches(&folder, &flow.branches) {
                Ok(()) => joined = format!(" Ramos juntos: {}.", flow.branches.join(", ")),
                Err(why) if why.contains("não salvas") => return Err(why),
                Err(why) => {
                    (task.state, task.note, task.updated, task.flow) = (TaskState::Blocked, why.clone(), now_ms(), Some(flow));
                    self.tasks.list[i] = task.clone();
                    self.task_event(&task, &format!("Aguardando aprovação → Bloqueada ({why})"));
                    self.save_tasks();

                    return Err(format!("Não juntei os ramos, nada foi mudado: {why}. A tarefa {} ficou bloqueada; resolva e chame advance de novo.", task.id));
                }
            }
        }

        let was = flow.stage;
        let step = flow.advance(ok, &note, &task.owner_name, now_ms());
        let lesson = (!ok).then(|| self.flow_lesson(&task, was, &note));

        if let Some((name, owner_name)) = owner {
            (task.owner, task.owner_name) = (name, owner_name);
        }

        task.state = if step == Move::Blocked { TaskState::Blocked } else { flow.state() };
        task.note = if ok { String::new() } else { note.clone() };
        task.updated = now_ms();
        task.flow = Some(flow.clone());
        self.tasks.list[i] = task.clone();

        let said = match (step, flow.current()) {
            (Move::Finished, _) => {
                let project = self.flow_project(&task);

                format!(
                    "Fluxo {} concluído.{joined}\nFechamento: leia as notas `{}-*` e `licoes-{project}` e consolide as decisões e as lições na nota `projeto-{project}` (binder write); deixe as `{}-*` como histórico.",
                    task.id, task.id, task.id
                )
            }
            (Move::Blocked, _) => format!(
                "{MAX_FAILS} falhas seguidas na etapa {} ({}): a tarefa {} ficou bloqueada ({note}). A pessoa decide; quando liberar, chame advance de novo.",
                was + 1,
                STAGES[was].name,
                task.id
            ),
            (_, Some(next)) => {
                let outcome = match step {
                    Move::Back => format!("falhou ({note}); volta à etapa {} ({})", flow.stage + 1, next.name),
                    Move::Repeat => format!("falhou ({note}); repete a etapa"),
                    _ => format!("concluída; segue para a etapa {} ({})", flow.stage + 1, next.name),
                };

                format!("Etapa {} ({}) {outcome}.\n\n{}", was + 1, STAGES[was].name, self.flow_brief(&task, &flow))
            }
            _ => String::new(),
        };

        self.task_event(&task, &format!("fluxo: etapa {} ({}) → {}", was + 1, STAGES[was].name, task.state.label()));
        self.save_tasks();
        let mut said = said;

        if let Some(Err(why)) = lesson {
            said += &format!("\nNão gravei a lição: {why}");
        }

        if args["send"] == true && !matches!(step, Move::Finished | Move::Blocked) {
            let text = self.flow_brief(&task, &flow);

            said += &format!("\n{}", self.deliver_text(&task.id, text, args["asker"].as_str().unwrap_or_default()).unwrap_or_else(|why| why));
        }

        Ok(said)
    }

    /// A failure's line in `licoes-<project>`: the date, the stage's note, why.
    fn flow_lesson(&self, task: &Task, stage: usize, note: &str) -> Result<String, String> {
        let line = format!(
            "- {} [[{}-{}]] etapa {} ({}): {}",
            crate::app::filetree::annotate::date(now_ms() / 1000, "+0000"),
            task.id,
            STAGES[stage].code,
            stage + 1,
            STAGES[stage].name,
            note.replace('\n', " ")
        );

        binder::write(&self.binder_folder()?, &format!("licoes-{}", self.flow_project(task)), &line, true)
    }

    fn flow_status(&self, id: &str) -> Result<String, String> {
        if id.is_empty() {
            let rows: Vec<String> = self
                .tasks
                .list
                .iter()
                .filter_map(|t| {
                    let flow = t.flow.as_ref()?;
                    let at = flow.current().map_or("concluído".to_string(), |s| format!("{} {}/{}", s.name, flow.stage + 1, STAGES.len()));

                    Some(format!("{} · {} · {} · {}", t.id, at, t.state.label(), t.title))
                })
                .collect();

            return Ok(if rows.is_empty() { "Nenhum fluxo.".into() } else { format!("{} fluxos:\n{}", rows.len(), rows.join("\n")) });
        }

        let task = &self.tasks.list[self.task_index(id)?];
        let flow = task.flow.as_ref().ok_or_else(|| format!("A tarefa {} não é um fluxo.", task.id))?;
        let mut out = format!("{}: {} · {}", task.id, task.title, task.state.label());

        if !task.note.is_empty() {
            out += &format!(" ({})", task.note);
        }

        for (index, stage) in STAGES.iter().enumerate() {
            let last = flow.runs.iter().rev().find(|r| r.stage == index && !r.result.is_empty());

            let (mark, detail) = match last {
                _ if index == flow.stage => ("[>]", String::new()),
                Some(run) if run.result == "ok" => ("[x]", String::new()),
                Some(run) => ("[!]", format!(" falhou {}x: {}", flow.fails(index), run.note)),
                None => ("[ ]", String::new()),
            };

            out += &format!("\n{mark} {}. {} · {}{detail}", index + 1, stage.name, stage.who);
        }

        if !flow.branches.is_empty() {
            out += &format!("\nRamos: {}", flow.branches.join(", "));
        }

        Ok(out)
    }
}
