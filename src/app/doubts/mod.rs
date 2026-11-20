//! Dúvidas: the questions an agent leaves for the person while it works (the MCP's `doubts`), answered in a panel on
//! the canvas and over the chat (panes/doubts.rs) and sent back to the agent as one message: the answered ones, or all
//! of them with the rest left to it. Kept in duvidas.json beside the drawing until they are sent.

use crate::app::panes::Pane;
use crate::prelude::*;

/// A question of an agent's and the person's answer so far.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Doubt {
    pub(crate) id: u64,
    /// The chat or terminal that asked: the answers go back to it.
    pub(crate) agent: String,
    pub(crate) question: String,
    /// Answers it suggests: a click puts one in the answer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) options: Vec<String>,
    #[serde(default)]
    pub(crate) answer: String,
    /// What the agent would pick: shown apart from the answers, as information.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) recommendation: String,
    /// The git branch the agent was on when it asked: the panel's section says it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) branch: String,
}

impl Doubt {
    pub(crate) fn answered(&self) -> bool {
        !self.answer.trim().is_empty()
    }

    /// The question and the agent's recommendation apart: its own field, else what the question itself says after
    /// "Recomendação:" or on lines that start with "Recomendo"/"Minha recomendação" (agents that wrote it inside).
    pub(crate) fn parts(&self) -> (String, String) {
        let question = self.question.trim();

        if !self.recommendation.trim().is_empty() {
            return (question.into(), self.recommendation.trim().into());
        }

        let recommends = |line: &&str| {
            let line = line.trim_start_matches(|c: char| c.is_whitespace() || "*_->".contains(c)).to_lowercase();

            line.starts_with("recomend") || line.starts_with("minha recomend")
        };

        let (said, asked): (Vec<&str>, Vec<&str>) = question.lines().partition(recommends);
        let (mut asked, mut said) = (asked.join("\n").trim().to_string(), said.join("\n").trim().to_string());

        if said.is_empty()
            && let Some(at) = asked.find("Recomendação:").or_else(|| asked.find("recomendação:"))
        {
            said = asked[at..].trim().to_string();
            asked = asked[..at].trim().to_string();
        }

        if asked.is_empty() { (question.into(), String::new()) } else { (asked, said) }
    }
}

/// The questions waiting on the person, as duvidas.json has them.
#[derive(Default)]
pub(crate) struct DoubtBoard {
    pub(crate) list: Vec<Doubt>,
    loaded: bool,
    /// An answer typed and not written yet: since when (egui time). It is written a second after the last key.
    pub(crate) changed_at: Option<f64>,
}

impl DoubtBoard {
    /// How many wait on an answer: the panel button's badge.
    pub(crate) fn unanswered(&self) -> usize {
        self.list.iter().filter(|doubt| !doubt.answered()).count()
    }

    /// The agents with questions, in the order they first asked.
    pub(crate) fn agents(&self) -> Vec<String> {
        let mut agents: Vec<String> = Vec::new();

        for doubt in &self.list {
            if !agents.contains(&doubt.agent) {
                agents.push(doubt.agent.clone());
            }
        }

        agents
    }

    /// The branch `agent` was on when it last asked (empty: none known).
    pub(crate) fn branch(&self, agent: &str) -> &str {
        self.list.iter().rev().find(|doubt| doubt.agent == agent && !doubt.branch.is_empty()).map_or("", |doubt| doubt.branch.as_str())
    }

    /// `agent`'s questions.
    pub(crate) fn of<'a>(&'a self, agent: &'a str) -> impl Iterator<Item = &'a Doubt> {
        self.list.iter().filter(move |doubt| doubt.agent == agent)
    }
}

/// The message that answers `doubts`: each question with its answer; with `all`, the unanswered ones go as the agent's
/// to decide. None when there is nothing to send.
pub(crate) fn answers_message(doubts: &[&Doubt], all: bool) -> Option<String> {
    let picked: Vec<&&Doubt> = doubts.iter().filter(|doubt| all || doubt.answered()).collect();

    if picked.is_empty() {
        return None;
    }

    let mut lines = vec!["Respostas às suas dúvidas (tela Dúvidas):".to_string()];

    for (index, doubt) in picked.iter().enumerate() {
        let answer = if doubt.answered() { doubt.answer.trim().replace('\n', "\n   ") } else { "sem resposta: decida você".into() };

        lines.push(format!("{}. {}\n   → {answer}", index + 1, doubt.question.trim()));
    }

    Some(lines.join("\n"))
}

/// What `add` takes: `questions`, a list of texts or of {question, options, recommendation}, or one `question` with its
/// `options` and `recommendation`.
pub(crate) fn asked_questions(args: &Value) -> Vec<(String, Vec<String>, String)> {
    let options = |value: &Value| -> Vec<String> {
        value.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::trim).filter(|option| !option.is_empty()).map(String::from).collect()
    };

    // a text, or an object with its question
    let one = |item: &Value| {
        let question = item.as_str().or(item["question"].as_str()).map(str::trim).filter(|question| !question.is_empty())?;

        Some((question.to_string(), options(&item["options"]), item["recommendation"].as_str().unwrap_or_default().trim().to_string()))
    };

    match args["questions"].as_array() {
        Some(list) => list.iter().filter_map(one).collect(),
        None => one(args).into_iter().collect(),
    }
}

impl DrawingApp {
    fn doubts_path(&self) -> Option<PathBuf> {
        self.autosave.as_ref().map(|path| path.with_file_name("duvidas.json"))
    }

    /// Each frame: the questions come from duvidas.json once there's a drawing; an answer being typed is written a
    /// second after its last key.
    pub(crate) fn doubts_tick(&mut self, ctx: &Context) {
        if !self.doubts.loaded
            && let Some(path) = self.doubts_path()
        {
            self.doubts.loaded = true;
            let data = fs::read(&path).ok();

            match data.map(|data| serde_json::from_slice::<Vec<Doubt>>(&data)) {
                Some(Ok(list)) => self.doubts.list = list,
                Some(Err(_)) => {
                    if let Ok(kept) = set_aside(&path) {
                        self.toast(format!("As dúvidas não puderam ser lidas e começaram vazias. O arquivo antigo ficou como {}.", kept_name(&kept)));
                    }
                }
                None => {}
            }
        }

        if let Some(at) = self.doubts.changed_at {
            let since = ctx.input(|input| input.time) - at;

            if since >= 1.0 {
                self.doubts.changed_at = None;
                self.save_doubts();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(1.0 - since));
            }
        }
    }

    /// The git branch `agent` works on, read once when it asks (empty: no folder, or not a repository).
    fn agent_branch(&self, agent: &str) -> String {
        let cwd = self.agent_rows().into_iter().find(|row| row.id == agent).map(|row| row.spec.cwd).filter(|cwd| !cwd.is_empty());
        let out = cwd.and_then(|cwd| crate::editor::run_git(std::path::Path::new(&cwd), &["branch", "--show-current"]).ok());

        out.map_or(String::new(), |out| out.trim().to_string())
    }

    pub(crate) fn save_doubts(&mut self) {
        let Some(path) = self.doubts_path() else { return };
        let written = serde_json::to_string_pretty(&self.doubts.list).map_err(std::io::Error::other).and_then(|json| write_atomic(&path, json));

        if let Err(error) = written {
            self.toast(format!("Não foi possível gravar duvidas.json: {error}"));
        }
    }

    /// Enviar respondidas (`all` false) or Enviar todas: `agent`'s questions in one message to it, and out of the
    /// panel; what happened.
    pub(crate) fn doubts_send(&mut self, agent: &str, all: bool) -> Result<String, String> {
        let open: Vec<&Doubt> = self.doubts.of(agent).collect();
        let Some(text) = answers_message(&open, all) else { return Err("Nenhuma dúvida respondida para enviar.".into()) };
        let sent: Vec<u64> = open.iter().filter(|doubt| all || doubt.answered()).map(|doubt| doubt.id).collect();
        let said = self.send_to_agent(agent, text)?;

        self.doubts.list.retain(|doubt| !sent.contains(&doubt.id));
        self.save_doubts();
        Ok(said)
    }

    /// Descartar: `agent`'s questions out of the panel, not sent.
    pub(crate) fn doubts_discard(&mut self, agent: &str) {
        self.doubts.list.retain(|doubt| doubt.agent != agent);
        self.save_doubts();
    }

    /// The MCP's `doubts`. An agent adds its questions (`add`), reads them with the answers so far (`list`) and takes
    /// back the ones it no longer needs (`remove`); `answer` and `send` are what the person does in the panel.
    pub(crate) fn act_doubts(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let asker = text("asker");

        // the agent named, else the one asking
        let agent = match text("agent") {
            name if name.is_empty() => asker.clone(),
            name => self.agent_named(&name)?.0,
        };

        let ids: Vec<u64> = args["ids"].as_array().into_iter().flatten().filter_map(Value::as_u64).chain(args["id"].as_u64()).collect();

        match text("action").as_str() {
            "add" => {
                if asker.is_empty() {
                    return Err("Só um agente do app (chat ou terminal) deixa dúvidas: as respostas voltam para ele.".into());
                }

                let questions = asked_questions(args);

                if questions.is_empty() {
                    return Err("Mande `questions`: uma lista de perguntas (texto, ou {question, options}).".into());
                }

                let mut next = self.doubts.list.iter().map(|doubt| doubt.id).max().unwrap_or(0);
                let mut added = Vec::new();
                let branch = self.agent_branch(&asker);

                for (question, options, recommendation) in questions {
                    next += 1;
                    added.push(next);
                    let (agent, branch) = (asker.clone(), branch.clone());

                    self.doubts.list.push(Doubt { id: next, agent, question, options, answer: String::new(), recommendation, branch });
                }

                self.save_doubts();
                self.set_pane(Some(Pane::Doubts));
                let ids: Vec<String> = added.iter().map(u64::to_string).collect();

                Ok(format!(
                    "{} dúvida(s) na tela Dúvidas (ids {}). A pessoa responde lá e as respostas chegam a você numa mensagem; continue o que \
                     não depende delas.",
                    added.len(),
                    ids.join(", ")
                ))
            }
            "list" | "" => {
                let lines: Vec<String> = self
                    .doubts
                    .list
                    .iter()
                    .filter(|doubt| agent.is_empty() || doubt.agent == agent)
                    .map(|doubt| {
                        let answer = if doubt.answered() { format!(" → {}", doubt.answer.trim()) } else { " (sem resposta ainda)".into() };

                        format!("{}. {}{answer}", doubt.id, doubt.question)
                    })
                    .collect();

                Ok(if lines.is_empty() { "Nenhuma dúvida aberta.".into() } else { lines.join("\n") })
            }
            "answer" => {
                let doubt = self.doubts.list.iter_mut().find(|doubt| ids.contains(&doubt.id)).ok_or("Nenhuma dúvida com esse id.")?;

                doubt.answer = text("answer");
                self.save_doubts();
                Ok(format!("Resposta da dúvida {} guardada.", doubt_id(&ids)))
            }
            "send" => self.doubts_send(&agent, args["all"] == true),
            "remove" => {
                let before = self.doubts.list.len();

                self.doubts.list.retain(|doubt| doubt.agent != agent || !(ids.is_empty() || ids.contains(&doubt.id)));
                let removed = before - self.doubts.list.len();

                self.save_doubts();
                Ok(format!("{removed} dúvida(s) tirada(s) da tela Dúvidas."))
            }
            other => Err(format!("action é add, list, answer, send ou remove, não {other}.")),
        }
    }
}

fn doubt_id(ids: &[u64]) -> u64 {
    ids.first().copied().unwrap_or_default()
}
