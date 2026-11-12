use super::*;

impl DrawingApp {
    /// The MCP's `tasks`: create, update, list, get. `caller` is the terminal that asked (the
    /// bridge sets it); empty, the Maestro or the person.
    pub(crate) fn act_tasks(&mut self, args: &Value) -> Result<String, String> {
        self.tasks_tick();
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        // ponytail: no terminal behind the call reads as the Maestro, though any MCP client outside a
        // terminal (bridge.json) lands here too; the chat's id in the bridge's request would tell them apart
        let requester = match (text("caller"), text("requester")) {
            (caller, _) if !caller.is_empty() => self.agent_label(&caller),
            (_, requester) if !requester.is_empty() => requester,
            _ => "Maestro".into(),
        };
        match text("action").as_str() {
            "create" => self.task_create(args, &requester),
            "update" => self.task_update(args),
            "list" | "" => self.task_list(&text("state"), &text("owner")),
            "get" => self.task_get(&text("id")),
            other => Err(format!("action é create, update, list ou get, não {other}.")),
        }
    }

    pub(crate) fn task_create(&mut self, args: &Value, requester: &str) -> Result<String, String> {
        self.tasks_tick();
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let title = text("title");
        if title.is_empty() {
            return Err("Diga o título da tarefa (title).".into());
        }
        let (owner, owner_name) = match text("owner") {
            owner if owner.is_empty() => Default::default(),
            owner => self.agent_named(&owner)?,
        };
        let id = next_id(&self.tasks.list);
        let deps = strings(&args["deps"]).unwrap_or_default();
        check_links(&self.tasks.list, &id, &text("parent"), &deps)?;
        let now = now_ms();
        let task = Task {
            id: id.clone(),
            title,
            description: text("description"),
            owner,
            owner_name,
            requester: requester.to_string(),
            parent: text("parent"),
            deps,
            artifacts: strings(&args["artifacts"]).unwrap_or_default(),
            workspace: self.spaces.current.clone(),
            created: now,
            updated: now,
            ..Default::default()
        };
        self.tasks.list.push(task.clone());
        self.task_event(&task, &format!("criada por {requester}"));
        self.save_tasks();
        let mut said = match task.owner_name.as_str() {
            "" => format!("Tarefa {id} criada, sem responsável: {}.", task.title),
            name => format!("Tarefa {id} criada para {name}: {}.", task.title),
        };
        if args["send"] == true {
            said += &format!(" {}", self.deliver_task(&id).unwrap_or_else(|why| why));
        }
        Ok(said)
    }

    pub(crate) fn task_update(&mut self, args: &Value) -> Result<String, String> {
        self.tasks_tick();
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let id = text("id");
        let i = self.task_index(&id)?;
        let mut task = self.tasks.list[i].clone();
        let mut changes = Vec::new();
        if !text("title").is_empty() {
            task.title = text("title");
            changes.push("título".to_string());
        }
        if args["description"].is_string() {
            task.description = text("description");
            changes.push("descrição".into());
        }
        if !text("owner").is_empty() {
            (task.owner, task.owner_name) = self.agent_named(&text("owner"))?;
            changes.push(format!("responsável: {}", task.owner_name));
        }
        if args["parent"].is_string() {
            task.parent = text("parent");
        }
        if let Some(deps) = strings(&args["deps"]) {
            task.deps = deps;
        }
        if args["parent"].is_string() || args["deps"].is_array() {
            check_links(&self.tasks.list, &id, &task.parent, &task.deps)?;
            changes.push("ligações".into());
        }
        for artifact in strings(&args["artifacts"]).unwrap_or_default() {
            if !task.artifacts.contains(&artifact) {
                task.artifacts.push(artifact);
                changes.push("artefatos".into());
            }
        }
        let note = text("note");
        if !text("state").is_empty() {
            let wanted = TaskState::parse(&text("state"))?;
            let (state, why) = transition(&self.tasks.list, &task, wanted)?;
            let from = task.state;
            task.by_deps = state == TaskState::Blocked && wanted == TaskState::InProgress;
            task.state = state;
            task.note = if why.is_empty() { note.clone() } else { why };
            let detail = if task.note.is_empty() { String::new() } else { format!(" ({})", task.note) };
            changes.push(format!("{} → {}{detail}", from.label(), state.label()));
        } else if !note.is_empty() {
            task.note = note;
            changes.push(format!("nota: {}", task.note));
        }
        changes.dedup();
        if changes.is_empty() && args["send"] != true {
            return Err("Nada para mudar: diga state, note, owner, title, description, parent, deps ou artifacts.".into());
        }
        task.updated = now_ms();
        self.tasks.list[i] = task.clone();
        if !changes.is_empty() {
            self.task_event(&task, &changes.join("; "));
        }
        // what waited on this one's dependencies goes back to pending
        let mut said = format!("Tarefa {id}: {}.", if changes.is_empty() { "sem mudanças".to_string() } else { changes.join("; ") });
        for freed in unblocked(&self.tasks.list) {
            let Ok(j) = self.task_index(&freed) else { continue };
            let t = &mut self.tasks.list[j];
            (t.state, t.note, t.by_deps, t.updated) = (TaskState::Pending, "as dependências concluíram".into(), false, now_ms());
            let t = t.clone();
            self.task_event(&t, "Bloqueada → Pendente (as dependências concluíram)");
            said += &format!(" A tarefa {freed} voltou a pendente: as dependências concluíram.");
        }
        self.save_tasks();
        if args["send"] == true {
            said += &format!(" {}", self.deliver_task(&id).unwrap_or_else(|why| why));
        }
        Ok(said)
    }
}
