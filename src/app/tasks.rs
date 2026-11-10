//! Tasks: what was asked of whom, in what state, depending on what. Kept in tarefas.json beside the
//! drawing; each creation and change of state is an event (the agents' history), and a task stuck
//! on the person is a demand with its attention points. The MCP's `tasks` and the agents panel's
//! Tarefas tab (ui/tasks.rs) drive them.

use super::demands::{Demand, DemandKind};
use crate::app::ui::agents::{AgentState, ago};
use crate::prelude::*;
use std::path::Path;

/// Untouched this long (seconds), a task is an attention point.
const STALE: f64 = 600.0;
/// This many tasks in progress with one agent is an attention point.
const BUSY: usize = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskState {
    #[default]
    Pending,
    InProgress,
    WaitingAgent,
    WaitingApproval,
    InReview,
    InTest,
    Blocked,
    Done,
    Failed,
    Cancelled,
}

/// Each state's stable id (file, MCP) and name (screen, answers), in the order the lists show
/// them: what needs the person first.
pub(crate) const STATES: [(TaskState, &str, &str); 10] = [
    (TaskState::WaitingApproval, "waiting_approval", "Aguardando aprovação"),
    (TaskState::Blocked, "blocked", "Bloqueada"),
    (TaskState::Failed, "failed", "Falhou"),
    (TaskState::InProgress, "in_progress", "Em andamento"),
    (TaskState::WaitingAgent, "waiting_agent", "Aguardando agente"),
    (TaskState::InReview, "in_review", "Em revisão"),
    (TaskState::InTest, "in_test", "Em teste"),
    (TaskState::Pending, "pending", "Pendente"),
    (TaskState::Done, "done", "Concluída"),
    (TaskState::Cancelled, "cancelled", "Cancelada"),
];

impl TaskState {
    pub(crate) fn rank(self) -> usize {
        STATES.iter().position(|s| s.0 == self).unwrap_or(0)
    }

    pub(crate) fn code(self) -> &'static str {
        STATES[self.rank()].1
    }

    pub(crate) fn label(self) -> &'static str {
        STATES[self.rank()].2
    }

    /// Done, failed and cancelled: only reopening (to pending) moves them.
    pub(crate) fn is_final(self) -> bool {
        matches!(self, TaskState::Done | TaskState::Failed | TaskState::Cancelled)
    }

    /// Stuck on the person: a demand.
    pub(crate) fn needs_you(self) -> bool {
        matches!(self, TaskState::WaitingApproval | TaskState::Blocked | TaskState::Failed)
    }

    /// By its id or its name, any case.
    pub(crate) fn parse(text: &str) -> Result<TaskState, String> {
        let wanted = text.trim().to_lowercase();
        STATES
            .iter()
            .find(|s| s.1 == wanted || s.2.to_lowercase() == wanted)
            .map(|s| s.0)
            .ok_or_else(|| format!("Estado desconhecido: {text}. Use {}.", STATES.map(|s| s.1).join(", ")))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Task {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) state: TaskState,
    /// The agent doing it (a terminal's or a chat's id; empty: nobody yet) and its name then.
    pub(crate) owner: String,
    pub(crate) owner_name: String,
    /// Who asked: "você" (the panel), the Maestro, an agent's name.
    pub(crate) requester: String,
    /// The task this one is part of; empty: none.
    pub(crate) parent: String,
    pub(crate) deps: Vec<String>,
    /// The workspace on screen when it was made (its id).
    // ponytail: kept, not yet used to filter the list; show only the space's tasks when they get too many
    pub(crate) workspace: String,
    /// What it made or touched: files, diffs, portals, conversations.
    pub(crate) artifacts: Vec<String>,
    /// ms since 1970.
    pub(crate) created: i64,
    pub(crate) updated: i64,
    /// Why it's in its state: why it blocked, why it failed.
    pub(crate) note: String,
    /// Blocked by its dependencies, not by hand: back to pending once they're all done.
    pub(crate) by_deps: bool,
}

/// The dependencies not done yet, as "t2 (em andamento)".
fn unfinished(tasks: &[Task], deps: &[String]) -> Vec<String> {
    deps.iter()
        .filter_map(|d| match tasks.iter().find(|t| t.id == *d) {
            Some(t) if t.state == TaskState::Done => None,
            Some(t) => Some(format!("{d} ({})", t.state.label().to_lowercase())),
            None => Some(format!("{d} (não existe)")),
        })
        .collect()
}

/// Where `task`, asked to go `to`, lands, and why when it isn't there: a final state only reopens
/// to pending; in progress with a dependency not done is blocked by it.
pub(crate) fn transition(tasks: &[Task], task: &Task, to: TaskState) -> Result<(TaskState, String), String> {
    if task.state.is_final() && to != task.state && to != TaskState::Pending {
        return Err(format!("A tarefa {} está {}: um estado final só reabre para pendente (state pending).", task.id, task.state.label().to_lowercase()));
    }
    let waits = unfinished(tasks, &task.deps);
    if to == TaskState::InProgress && !waits.is_empty() {
        return Ok((TaskState::Blocked, format!("depende de {}", waits.join(", "))));
    }
    Ok((to, String::new()))
}

/// The tasks blocked by their dependencies that are all done now.
pub(crate) fn unblocked(tasks: &[Task]) -> Vec<String> {
    tasks.iter().filter(|t| t.state == TaskState::Blocked && t.by_deps && unfinished(tasks, &t.deps).is_empty()).map(|t| t.id.clone()).collect()
}

/// A path of dependencies from `from` to `to`, both in it; `seen` keeps it finite.
fn dep_path(tasks: &[Task], from: &str, to: &str, seen: &mut Vec<String>) -> Option<Vec<String>> {
    if from == to {
        return Some(vec![to.to_string()]);
    }
    if seen.iter().any(|s| s == from) {
        return None;
    }
    seen.push(from.to_string());
    let deps = tasks.iter().find(|t| t.id == from)?.deps.clone();
    let mut path = deps.iter().find_map(|d| dep_path(tasks, d, to, seen))?;
    path.insert(0, from.to_string());
    Some(path)
}

/// The parent and dependencies of task `id` (new or not): each must exist, and no chain of
/// parents or of dependencies may come back to it.
pub(crate) fn check_links(tasks: &[Task], id: &str, parent: &str, deps: &[String]) -> Result<(), String> {
    let exists = |x: &str| tasks.iter().any(|t| t.id == x);
    if !parent.is_empty() {
        if parent == id {
            return Err("Uma tarefa não pode ser a tarefa-mãe de si mesma.".into());
        }
        if !exists(parent) {
            return Err(format!("A tarefa-mãe {parent} não existe."));
        }
        let mut chain = vec![id.to_string(), parent.to_string()];
        let mut at = parent.to_string();
        while let Some(up) = tasks.iter().find(|t| t.id == at).map(|t| t.parent.clone()).filter(|p| !p.is_empty()) {
            chain.push(up.clone());
            if up == id {
                return Err(format!("Ciclo de tarefas-mãe: {}.", chain.join(" → ")));
            }
            // a cycle already there that doesn't pass through `id`
            if chain.len() > tasks.len() + 2 {
                break;
            }
            at = up;
        }
    }
    for dep in deps {
        if dep == id {
            return Err("Uma tarefa não pode depender de si mesma.".into());
        }
        if !exists(dep) {
            return Err(format!("A dependência {dep} não existe."));
        }
        if let Some(path) = dep_path(tasks, dep, id, &mut Vec::new()) {
            return Err(format!("Ciclo de dependências: {id} → {}.", path.join(" → ")));
        }
    }
    Ok(())
}

/// What deserves a look on a task, from the data only: untouched for long, a dependency that
/// failed or was cancelled, its owner gone (`owner` None) or ended (Some(false)), its owner with
/// many tasks in progress.
pub(crate) fn task_points(tasks: &[Task], task: &Task, now: i64, owner: Option<bool>) -> Vec<String> {
    let mut points = Vec::new();
    let idle = (now - task.updated).max(0) as f64 / 1000.0;
    if idle >= STALE {
        points.push(format!("parada {}", ago(idle)));
    }
    for dep in &task.deps {
        match tasks.iter().find(|t| t.id == *dep).map(|t| t.state) {
            Some(TaskState::Failed) => points.push(format!("a dependência {dep} falhou")),
            Some(TaskState::Cancelled) => points.push(format!("a dependência {dep} foi cancelada")),
            _ => {}
        }
    }
    if !task.owner.is_empty() {
        let name = &task.owner_name;
        match owner {
            None => points.push(format!("o responsável {name} não existe mais")),
            Some(false) => points.push(format!("o responsável {name} está encerrado")),
            Some(true) => {}
        }
        let load = tasks.iter().filter(|t| t.owner == task.owner && t.state == TaskState::InProgress).count();
        if load >= BUSY {
            points.push(format!("{name} tem {load} tarefas em andamento"));
        }
    }
    points
}

/// What goes to the owner's terminal or chat: the task, its id, and how to say where it stands.
pub(crate) fn task_message(task: &Task, tasks: &[Task]) -> String {
    let mut text = format!("Tarefa {} para você: {}.", task.id, task.title.trim_end_matches('.'));
    if !task.description.trim().is_empty() {
        text += &format!("\n{}", task.description.trim());
    }
    let waits = unfinished(tasks, &task.deps);
    if !waits.is_empty() {
        text += &format!("\nDepende de: {}.", waits.join(", "));
    }
    text += &format!(
        "\nAvise em que pé ela está pela ferramenta tasks do MagoAgent (action update, id {}): state in_progress ao começar; done, failed ou blocked ao terminar ou travar, com note dizendo por quê; artifacts com os arquivos ou o diff.",
        task.id
    );
    text
}

/// The next free id: t1, t2, …
fn next_id(tasks: &[Task]) -> String {
    let last = tasks.iter().filter_map(|t| t.id.strip_prefix('t')?.parse::<u64>().ok()).max().unwrap_or(0);
    format!("t{}", last + 1)
}

/// tarefas.json. Like `read_chats`: an unreadable file is set aside, so the next save doesn't
/// write an empty list over it, and where it went comes back.
pub(crate) fn read_tasks(path: &Path) -> (Vec<Task>, Option<PathBuf>) {
    let Ok(data) = fs::read(path) else { return (Vec::new(), None) };
    match serde_json::from_slice(&data) {
        Ok(tasks) => (tasks, None),
        Err(_) => (Vec::new(), set_aside(path).ok()),
    }
}

/// The tasks and the panel's tabs (ui/tasks.rs).
#[derive(Default)]
pub(crate) struct TaskBoard {
    pub(crate) list: Vec<Task>,
    loaded: bool,
    /// The agents panel's tab: 0 agents, 1 tasks, 2 binder.
    pub(crate) tab: usize,
    /// The new task's title, owner (id) and whether it goes to the owner.
    pub(crate) title: String,
    pub(crate) owner: String,
    pub(crate) send: bool,
    /// The binder's notes as last read (when, app seconds; name, size, first line).
    pub(crate) notes: Option<(f64, Vec<(String, u64, String)>)>,
}

fn strings(value: &Value) -> Option<Vec<String>> {
    let list = value.as_array()?.iter().filter_map(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let mut out: Vec<String> = Vec::new();
    for s in list {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    Some(out)
}

impl DrawingApp {
    fn tasks_path(&self) -> Option<PathBuf> {
        self.autosave.as_ref().map(|p| p.with_file_name("tarefas.json"))
    }

    /// Each frame (and before any change): the tasks come from tarefas.json once there's a drawing.
    pub(crate) fn tasks_tick(&mut self) {
        if self.tasks.loaded {
            return;
        }
        let Some(path) = self.tasks_path() else { return };
        self.tasks.loaded = true;
        let (list, kept) = read_tasks(&path);
        self.tasks.list = list;
        if let Some(kept) = kept {
            let earlier = std::mem::take(&mut self.status);
            let message = format!("Não foi possível ler tarefas.json: ficou guardado em {} e a lista de tarefas começou vazia.", kept.display());
            self.toast(if earlier.is_empty() { message } else { format!("{earlier}\n{message}") });
        }
    }

    fn save_tasks(&mut self) {
        let Some(path) = self.tasks_path() else { return };
        let written = serde_json::to_string_pretty(&self.tasks.list).map_err(std::io::Error::other).and_then(|json| write_atomic(&path, json));
        if let Err(e) = written {
            self.toast(format!("Não foi possível gravar tarefas.json: {e}"));
        }
    }

    /// An agent (terminal or chat) by id or name, any case: (id, name).
    pub(crate) fn agent_named(&self, name: &str) -> Result<(String, String), String> {
        let wanted = name.trim().to_lowercase();
        let rows = self.agent_rows();
        let row = rows.iter().find(|r| r.id == name.trim() || r.spec.label().to_lowercase() == wanted);
        row.map(|r| (r.id.clone(), r.spec.label().to_string())).ok_or_else(|| format!("Nenhum agente chamado {name}."))
    }

    /// The owner's name now, else the one it had.
    fn task_owner(&self, task: &Task) -> String {
        match task.owner.as_str() {
            "" => "sem responsável".into(),
            id if self.agent_rows().iter().any(|r| r.id == id) => self.agent_label(id),
            _ => task.owner_name.clone(),
        }
    }

    /// One task's event, under its owner (the agents' history and the timeline).
    fn task_event(&mut self, task: &Task, change: &str) {
        let name = if task.owner.is_empty() { "Tarefas".to_string() } else { self.task_owner(task) };
        let mut event = AgentEvent::new(&task.owner, &name, EventKind::Task, format!("tarefa {} \"{}\": {change}", task.id, task.title));
        event.data = json!({ "task": task.id, "state": task.state.code() });
        self.record(event);
    }

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

    fn task_index(&self, id: &str) -> Result<usize, String> {
        if id.is_empty() {
            return Err("Diga o id da tarefa (id), como tasks list mostra.".into());
        }
        self.tasks.list.iter().position(|t| t.id.eq_ignore_ascii_case(id)).ok_or_else(|| format!("Nenhuma tarefa {id}."))
    }

    /// The task, as text, to its owner's terminal (typed as a prompt) or chat.
    fn deliver_task(&mut self, id: &str) -> Result<String, String> {
        let task = self.tasks.list[self.task_index(id)?].clone();
        if task.owner.is_empty() {
            return Err("Não entreguei: a tarefa não tem responsável.".into());
        }
        let (name, text) = (self.task_owner(&task), task_message(&task, &self.tasks.list));
        if self.is_chat(&task.owner) {
            self.chat_outbox.push((task.owner.clone(), text));
        } else if matches!(self.terminals.get(&task.owner), Some(Ok(_))) {
            // one it can't take now (a menu on its screen) comes back to its composer or the clipboard
            self.send_prompt_after(&task.owner, &text, std::time::Duration::ZERO);
        } else {
            return Err(format!("Não entreguei: {name} não está rodando."));
        }
        self.task_event(&task, &format!("entregue a {name}"));
        Ok(format!("Entregue a {name}."))
    }

    /// The tasks, what needs the person first, then by state; filtered by state and owner.
    fn task_list(&self, state: &str, owner: &str) -> Result<String, String> {
        let state = if state.is_empty() { None } else { Some(TaskState::parse(state)?) };
        let owner = owner.to_lowercase();
        let mut shown: Vec<&Task> = self
            .tasks
            .list
            .iter()
            .filter(|t| state.is_none_or(|s| t.state == s))
            .filter(|t| owner.is_empty() || t.owner.to_lowercase() == owner || self.task_owner(t).to_lowercase() == owner)
            .collect();
        if shown.is_empty() {
            return Ok(if self.tasks.list.is_empty() { "Nenhuma tarefa.".into() } else { "Nenhuma tarefa com esse filtro.".into() });
        }
        shown.sort_by_key(|t| (t.state.rank(), -t.updated));
        let now = now_ms();
        let mut out = match shown.len() {
            1 => "1 tarefa:".to_string(),
            n => format!("{n} tarefas, as travadas primeiro:"),
        };
        for t in shown {
            let note = if t.note.is_empty() { String::new() } else { format!(" — {}", t.note) };
            let when = ago((now - t.updated).max(0) as f64 / 1000.0);
            out += &format!("\n{} · {} · {} · {}{note} · {when}", t.id, t.state.label(), self.task_owner(t), t.title);
        }
        Ok(out)
    }

    fn task_get(&self, id: &str) -> Result<String, String> {
        let t = &self.tasks.list[self.task_index(id)?];
        let now = now_ms();
        let secs = |ms: i64| ago((now - ms).max(0) as f64 / 1000.0);
        let mut out = format!("{}: {}\nEstado: {}", t.id, t.title, t.state.label());
        if !t.note.is_empty() {
            out += &format!(" ({})", t.note);
        }
        out += &format!("\nResponsável: {} · pediu: {}", self.task_owner(t), t.requester);
        if !t.description.is_empty() {
            out += &format!("\nDescrição: {}", t.description);
        }
        if !t.parent.is_empty() {
            out += &format!("\nTarefa-mãe: {}", t.parent);
        }
        if !t.deps.is_empty() {
            let deps: Vec<String> =
                t.deps.iter().map(|d| unfinished(&self.tasks.list, std::slice::from_ref(d)).pop().unwrap_or(format!("{d} (concluída)"))).collect();
            out += &format!("\nDepende de: {}", deps.join(", "));
        }
        let children: Vec<String> =
            self.tasks.list.iter().filter(|c| c.parent == t.id).map(|c| format!("{} ({})", c.id, c.state.label().to_lowercase())).collect();
        if !children.is_empty() {
            out += &format!("\nSubtarefas: {}", children.join(", "));
        }
        if !t.artifacts.is_empty() {
            out += &format!("\nArtefatos: {}", t.artifacts.join(", "));
        }
        out += &format!("\nCriada {} · atualizada {}", secs(t.created), secs(t.updated));
        let points = self.task_points_now(t);
        if !points.is_empty() {
            out += &format!("\nAtenção: {}", points.join("; "));
        }
        Ok(out)
    }

    fn task_points_now(&self, task: &Task) -> Vec<String> {
        let owner = self.agent_rows().into_iter().find(|r| r.id == task.owner).map(|r| r.state != AgentState::Ended);
        task_points(&self.tasks.list, task, now_ms(), owner)
    }

    /// The tasks stuck on the person (waiting for approval, blocked, failed), as demands.
    pub(crate) fn task_demands(&self) -> Vec<Demand> {
        let now = now_ms();
        let mut stuck: Vec<&Task> = self.tasks.list.iter().filter(|t| t.state.needs_you()).collect();
        stuck.sort_by_key(|t| (t.state.rank(), t.updated));
        stuck
            .into_iter()
            .map(|t| {
                let note = if t.note.is_empty() { String::new() } else { format!(": {}", t.note) };
                Demand {
                    id: t.owner.clone(),
                    name: if t.owner.is_empty() { "Tarefa sem responsável".into() } else { self.task_owner(t) },
                    kind: DemandKind::Task,
                    what: format!("tarefa {} \"{}\" {}{note}", t.id, t.title, t.state.label().to_lowercase()),
                    waited: Some((now - t.updated).max(0) as f64 / 1000.0),
                    points: self.task_points_now(t),
                    answerable: false,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
