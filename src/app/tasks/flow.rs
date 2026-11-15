//! The flow of an implementation: 12 stages kept on a task (`Task.flow`). The app holds only the state machine and the
//! texts; who does each stage is the Maestro and the agents. Pure: the MCP's `flow` (flow_act.rs) drives it.

use super::TaskState;
use crate::app::maestro::skills::{CODE_REVIEW_BRIEF, PONYTAIL_BRIEF, style_brief};
use crate::prelude::*;

pub(crate) struct Stage {
    /// The stage's id and the suffix of its note in the binder (`<task>-<code>`).
    pub(crate) code: &'static str,
    pub(crate) name: &'static str,
    pub(crate) who: &'static str,
    /// What the task is in while the stage runs.
    pub(crate) state: TaskState,
    /// Stops and waits for the person.
    pub(crate) person: bool,
    /// Where a failure goes back to; None: the stage is repeated.
    pub(crate) back: Option<usize>,
    /// What the agent is handed (`{id}`, `{projeto}`, `{skill}`, `{pasta}` are filled in): all in text, so Claude and Codex
    /// both follow it; a skill comes as the file to read.
    pub(crate) brief: &'static str,
}

/// The stage a failure of 6 to 12 goes back to: the implementation.
const IMPLEMENT: usize = 4;
/// Failures in a row in one stage that block the task.
pub(crate) const MAX_FAILS: u32 = 3;

const fn stage(code: &'static str, name: &'static str, who: &'static str, state: TaskState, person: bool, back: Option<usize>, brief: &'static str) -> Stage {
    Stage { code, name, who, state, person, back, brief }
}

pub(crate) const STAGES: [Stage; 12] = [
    stage(
        "planejar",
        "Planejar",
        "Maestro",
        TaskState::InProgress,
        false,
        None,
        "Leia as notas `projeto-{projeto}` e `licoes-{projeto}` do fichário (se existirem). Escreva o objetivo, o que fica de fora e os critérios de aceite, cada um verificável.",
    ),
    stage(
        "analisar",
        "Analisar codebase",
        "agente leitor (só lê)",
        TaskState::InProgress,
        false,
        None,
        "Só leia, não mude nada. Liste os arquivos que o plano toca, quem os chama, os padrões do projeto (leia `AGENTS.md`/`CLAUDE.md`) e os testes que já existem.",
    ),
    stage(
        "arquitetar",
        "Arquitetar",
        "agente de plano",
        TaskState::InProgress,
        false,
        None,
        "Divida em fatias pequenas. Cada fatia tem os arquivos que só ela mexe, a ordem e o teste que a prova; duas fatias nunca mexem no mesmo arquivo. Leia também `licoes-{projeto}`.",
    ),
    stage(
        "duvidas",
        "Dúvidas",
        "Maestro",
        TaskState::WaitingApproval,
        true,
        None,
        "Junte os pontos abertos das etapas 1 a 3 e pergunte à pessoa pela ferramenta `ask_person`. Grave as respostas na nota (ou \"sem dúvidas\"). Só chame `flow advance` com `ok` depois que a pessoa responder tudo.",
    ),
    stage(
        "implementar",
        "Delegação e implementação",
        "Maestro delega, agentes implementam",
        TaskState::InProgress,
        false,
        None,
        "Crie uma tarefa-filha por fatia (parent {id}) e recrute um agente por fatia com `recruit --worktree`. Leia `licoes-{projeto}` antes. Quando todas as filhas estiverem `done` e compilar, chame `flow advance` com `ok` e `branches`: os ramos `mago/<nome>` de cada agente. Na nota, liste os ramos e os arquivos mexidos.",
    ),
    stage(
        "testes",
        "Testes unitários e de integração",
        "agente de teste",
        TaskState::InTest,
        false,
        Some(IMPLEMENT),
        "Rode os testes do projeto (no MagoAgent: `cargo test`) e escreva os que faltam: toda lógica nova deixa um teste. Na nota: o comando, o resultado e os testes novos. Falhou: `flow advance` com `fail` e o que quebrou.",
    ),
    stage(
        "usos",
        "Validar onde é usado",
        "agente leitor",
        TaskState::InReview,
        false,
        Some(IMPLEMENT),
        "Para cada símbolo que o diff mudou, procure os usos (`ide_search`, git grep) e confira se continuam certos. Na nota: símbolo, usos e o veredito. Uso quebrado: `fail`.",
    ),
    stage(
        "padrao",
        "Padronização (skill style-edsu)",
        "agente revisor",
        TaskState::InReview,
        false,
        Some(IMPLEMENT),
        "{skill} Leia também `AGENTS.md`/`CLAUDE.md` do projeto. Na nota, uma linha por regra: segue / não segue (arquivo:linha). Qualquer \"não segue\": `fail`.",
    ),
    stage(
        "ponytail",
        "Revisor ponytail",
        "agente revisor",
        TaskState::InReview,
        false,
        Some(IMPLEMENT),
        "Revise: {skill} Na nota: os achados, cada um bloqueia ou não. Bloqueio: `fail`.",
    ),
    stage(
        "navegador",
        "Navegador e cenários (Playwright)",
        "agente de QA",
        TaskState::InTest,
        false,
        Some(IMPLEMENT),
        "Com o app rodando, passe os critérios de aceite por cenários: lógica, cores, permissões e responsividade (portal_* ou o Playwright: larguras 375/768/1280, tema claro e escuro). Em projeto web (tem `package.json`), grave os cenários em `e2e/*.spec.ts` e rode `npx playwright test`. Sem tela: nota \"nada a fazer: sem tela\". Na nota: um cenário por linha (passou/falhou, print, spec). Falhou: `fail`.",
    ),
    stage(
        "review-agente",
        "Code review do agente",
        "outro agente, que não implementou",
        TaskState::InReview,
        false,
        Some(IMPLEMENT),
        "Em {pasta}, revise: {skill} Leia as notas anteriores `{id}-*`. Na nota: os achados, cada um bloqueia ou não. Bloqueio: `fail`.",
    ),
    stage(
        "review-humano",
        "Code review humano",
        "a pessoa",
        TaskState::WaitingApproval,
        true,
        Some(IMPLEMENT),
        "Abra o diff de {pasta} para a pessoa (`review`) e resuma as notas `{id}-*`. Aprovou: `flow advance` com `ok` (isso junta os ramos `mago/*` no ramo atual do projeto). Pediu ajuste: `fail` com o texto dela, e volta à implementação.",
    ),
];

/// One go at a stage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Run {
    pub(crate) stage: usize,
    /// ms since 1970; `ended` is 0 while it runs.
    pub(crate) started: i64,
    pub(crate) ended: i64,
    /// "" while it runs, then "ok", "fail" or "blocked" (the failure that blocked the task).
    pub(crate) result: String,
    pub(crate) note: String,
    pub(crate) agent: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Flow {
    /// The stage now (an index in `STAGES`); `STAGES.len()` once it is all done.
    pub(crate) stage: usize,
    /// The repository or worktree: where the diff is and where the branches are joined.
    pub(crate) folder: String,
    pub(crate) runs: Vec<Run>,
    /// The agents' branches (`mago/<name>`) joined after the human review.
    pub(crate) branches: Vec<String>,
}

/// Where `Flow::advance` left the flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Move {
    Next,
    Back,
    Repeat,
    Blocked,
    Finished,
}

impl Flow {
    pub(crate) fn new(folder: &str, now: i64) -> Flow {
        Flow { folder: folder.into(), runs: vec![Run { started: now, ..Default::default() }], ..Default::default() }
    }

    pub(crate) fn current(&self) -> Option<&'static Stage> {
        STAGES.get(self.stage)
    }

    /// The failures in a row of `stage`: its last runs that failed, since it last passed or blocked.
    pub(crate) fn fails(&self, stage: usize) -> u32 {
        self.runs.iter().rev().filter(|r| r.stage == stage && !r.result.is_empty()).take_while(|r| r.result == "fail").count() as u32
    }

    /// Closes the run of the stage and opens the next: forward on `ok`; on a failure back to the stage's `back` (or the same
    /// one), and the third failure in a row in a stage blocks it (the same stage waits for another go).
    pub(crate) fn advance(&mut self, ok: bool, note: &str, agent: &str, now: i64) -> Move {
        let Some(stage) = self.current() else { return Move::Finished };
        let blocks = !ok && self.fails(self.stage) + 1 >= MAX_FAILS;
        if let Some(run) = self.runs.last_mut() {
            let result = if ok {
                "ok"
            } else if blocks {
                "blocked"
            } else {
                "fail"
            };
            (run.ended, run.result, run.note, run.agent) = (now, result.into(), note.into(), agent.into());
        }
        let (mv, to) = match (ok, blocks, stage.back) {
            (true, ..) if self.stage + 1 == STAGES.len() => (Move::Finished, STAGES.len()),
            (true, ..) => (Move::Next, self.stage + 1),
            (_, true, _) => (Move::Blocked, self.stage),
            (_, _, Some(back)) => (Move::Back, back),
            _ => (Move::Repeat, self.stage),
        };
        self.stage = to;
        if mv != Move::Finished {
            self.runs.push(Run { stage: to, started: now, ..Default::default() });
        }
        mv
    }

    /// The task's state for where the flow is (`Done` after the last stage).
    pub(crate) fn state(&self) -> TaskState {
        self.current().map_or(TaskState::Done, |s| s.state)
    }
}

/// What an agent is handed for stage `index` of task `id`: who, what to read, the stage's brief and how to close it.
pub(crate) fn brief(index: usize, id: &str, project: &str, style_skill: &str, folder: &str) -> String {
    let stage = &STAGES[index];
    let read = if index == 0 { String::new() } else { format!(" Leia antes a nota `{id}-{}`.", STAGES[index - 1].code) };
    let skill = match stage.code {
        "padrao" => style_brief(style_skill),
        "ponytail" => PONYTAIL_BRIEF.to_string(),
        "review-agente" => CODE_REVIEW_BRIEF.to_string(),
        _ => String::new(),
    };
    let body = stage.brief.replace("{skill}", &skill).replace("{id}", id).replace("{projeto}", project).replace("{pasta}", folder);
    let wait = if stage.person { " Esta etapa espera a pessoa." } else { "" };
    format!(
        "Fluxo {id}, etapa {}/{}: {} ({}).{wait}{read}\n{body}\nGrave o resultado na nota `{id}-{}` do fichário (binder write; a primeira linha é `Entrada: [[{id}-<etapa anterior>]]`). Depois chame a ferramenta `flow` (action advance, id {id}, result ok e note com o resumo; se falhou, result fail e o motivo).",
        index + 1,
        STAGES.len(),
        stage.name,
        stage.who,
        stage.code
    )
}
