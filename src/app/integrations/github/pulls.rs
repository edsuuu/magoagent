//! A pull request as the island's GitHub tab lists it: its state (open, merged, closed), its branches, whether it
//! conflicts with its base, which checks failed and whether it waits for the person. Read from the pulse's answer by
//! plain functions, away from the network.

use super::{CiState, Pulse, nodes, text_at};
use crate::prelude::*;

/// The most failed checks a pull request keeps.
const FAILING_MOST: usize = 5;
/// A check run's conclusions that count as failed (`CheckConclusionState`); a status's are ERROR and FAILURE.
const FAILED_RUNS: [&str; 5] = ["FAILURE", "TIMED_OUT", "CANCELLED", "STARTUP_FAILURE", "ACTION_REQUIRED"];
/// `reviewDecision` CHANGES_REQUESTED in words.
pub(crate) const CHANGES_REQUESTED: &str = "mudanças pedidas";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PrState {
    #[default]
    Open,
    Merged,
    Closed,
}

impl PrState {
    /// GitHub's `PullRequestState`; none is open (the search of the reviews asked for asks for open ones only).
    pub(crate) fn from_github(raw: Option<&str>) -> Self {
        match raw {
            Some("MERGED") => PrState::Merged,
            Some("CLOSED") => PrState::Closed,
            _ => PrState::Open,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            PrState::Open => "aberto",
            PrState::Merged => "mesclado",
            PrState::Closed => "fechado",
        }
    }
}

/// Whether it merges into its base: GitHub works it out after a push, and says UNKNOWN meanwhile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MergeableState {
    Mergeable,
    Conflicting,
    #[default]
    Unknown,
}

impl MergeableState {
    /// GitHub's `MergeableState`.
    pub(crate) fn from_github(raw: Option<&str>) -> Self {
        match raw {
            Some("MERGEABLE") => MergeableState::Mergeable,
            Some("CONFLICTING") => MergeableState::Conflicting,
            _ => MergeableState::Unknown,
        }
    }
}

/// A check that failed: its name, and the page of its run when it gave one (an https address anywhere: the app only
/// opens GitHub's own, `integrations::page_allowed`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Check {
    pub(crate) name: String,
    pub(crate) url: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PullRequest {
    /// "owner/repo#number".
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) draft: bool,
    pub(crate) ci: CiState,
    /// `reviewDecision` in words, when there is one.
    pub(crate) review: &'static str,
    /// The last commit's oid, when GitHub gave it.
    pub(crate) head: Option<String>,
    pub(crate) state: PrState,
    pub(crate) mergeable: MergeableState,
    /// Its branch and the one it goes into: "feat/frete" → "main".
    pub(crate) head_ref: String,
    pub(crate) base_ref: String,
    /// When it last changed, ms since 1970; 0 when GitHub did not say.
    pub(crate) updated_at: i64,
    /// GitHub's id of its node: what the writes (description, comment) name it by.
    pub(crate) node_id: String,
    /// Up to 5 checks of the last commit that failed.
    pub(crate) failing: Vec<Check>,
    /// Someone asked the person to review it (it came in the pulse's `reviewRequested`).
    pub(crate) review_requested_of_me: bool,
}

impl PullRequest {
    /// The page a click opens: its checks when they failed (where the failure is), else the pull request.
    pub(crate) fn page(&self) -> String {
        if self.ci == CiState::Failure { format!("{}/checks", self.url) } else { self.url.clone() }
    }

    /// Ver checks: the run of the first failed check that is a page of GitHub, else the pull request's checks.
    pub(crate) fn checks_page(&self) -> String {
        let run = self.failing.iter().filter_map(|check| check.url.as_deref()).find(|url| crate::app::integrations::page_allowed(url));
        run.map_or_else(|| format!("{}/checks", self.url), str::to_string)
    }

    /// "checks passaram · aprovado · rascunho".
    pub(crate) fn state_words(&self) -> String {
        let words = [self.ci.label().1, self.review, if self.draft { "rascunho" } else { "" }];
        words.iter().filter(|word| !word.is_empty()).copied().collect::<Vec<_>>().join(" · ")
    }

    pub(crate) fn is_open(&self) -> bool {
        self.state == PrState::Open
    }

    pub(crate) fn conflicts(&self) -> bool {
        self.mergeable == MergeableState::Conflicting
    }

    pub(crate) fn changes_requested(&self) -> bool {
        self.review == CHANGES_REQUESTED
    }

    /// Open and waiting for the person: its checks failed, it conflicts with its base, a reviewer asked for changes,
    /// or the person was asked to review it.
    pub(crate) fn needs_you(&self) -> bool {
        self.is_open() && (self.ci == CiState::Failure || self.conflicts() || self.changes_requested() || self.review_requested_of_me)
    }

    /// The pull request in one line, for the MCP: "ana/loja#42 · aberto · Título · feat → main · há 4 min · checks
    /// falharam · conflito · precisa de você · https://…".
    pub(crate) fn line(&self, now: i64) -> String {
        let state = if self.draft && self.is_open() { "rascunho" } else { self.state.label() };
        let mut parts = vec![self.id.clone(), state.to_string(), self.title.clone()];
        if !self.head_ref.is_empty() && !self.base_ref.is_empty() {
            parts.push(format!("{} → {}", self.head_ref, self.base_ref));
        }
        parts.push(ago(self.updated_at, now));
        if self.ci != CiState::Unknown {
            parts.push(self.ci.label().1.to_string());
        }
        if !self.failing.is_empty() {
            parts.push(format!("falhou: {}", self.failing.iter().map(|check| check.name.as_str()).collect::<Vec<_>>().join(", ")));
        }
        parts.push(self.review.to_string());
        if self.conflicts() {
            parts.push("conflito".into());
        }
        if self.review_requested_of_me {
            parts.push("pedem a sua revisão".into());
        }
        if self.needs_you() {
            parts.push("precisa de você".into());
        }
        parts.push(self.url.clone());
        parts.retain(|part| !part.is_empty());
        parts.join(" · ")
    }
}

/// What a row of the island's GitHub tab is drawn with.
// ponytail: only the island's GitHub tab calls these (it is drawn apart); drop the allow once it does
#[cfg_attr(not(test), allow(dead_code))]
impl PullRequest {
    /// "owner/repo" out of the id.
    pub(crate) fn repo(&self) -> &str {
        self.id.rsplit_once('#').map_or(self.id.as_str(), |(repo, _)| repo)
    }

    /// "42" out of the id.
    pub(crate) fn number(&self) -> &str {
        self.id.rsplit_once('#').map_or("", |(_, number)| number)
    }

    /// A row's second line: "ana/loja #42 · feat/frete → main · há 4 min" (what GitHub did not say left out).
    pub(crate) fn subtitle(&self, now: i64) -> String {
        let mut parts = vec![format!("{} #{}", self.repo(), self.number())];
        if !self.head_ref.is_empty() && !self.base_ref.is_empty() {
            parts.push(format!("{} → {}", self.head_ref, self.base_ref));
        }
        parts.push(ago(self.updated_at, now));
        parts.retain(|part| !part.is_empty());
        parts.join(" · ")
    }
}

/// How long ago `at` was (ms since 1970, as `now`), in pt-BR: "agora", "há 4 min", "há 2 h", "ontem", "há 3 dias",
/// "há 2 meses"; empty when it is unknown (0).
pub(crate) fn ago(at: i64, now: i64) -> String {
    if at <= 0 { String::new() } else { crate::app::filetree::vision::age(at / 1000, now / 1000) }
}

/// The tab's filters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PrFilter {
    #[default]
    All,
    Open,
    NeedsYou,
    Closed,
}

impl PrFilter {
    pub(crate) const EVERY: [PrFilter; 4] = [PrFilter::All, PrFilter::Open, PrFilter::NeedsYou, PrFilter::Closed];

    pub(crate) fn label(self) -> &'static str {
        match self {
            PrFilter::All => "Todas",
            PrFilter::Open => "Abertas",
            PrFilter::NeedsYou => "Precisa de você",
            PrFilter::Closed => "Fechadas",
        }
    }

    /// Its name in the MCP's `filter`.
    pub(crate) fn key(self) -> &'static str {
        match self {
            PrFilter::All => "all",
            PrFilter::Open => "open",
            PrFilter::NeedsYou => "needs_you",
            PrFilter::Closed => "closed",
        }
    }

    /// Fechadas are the merged ones too.
    pub(crate) fn keeps(self, request: &PullRequest) -> bool {
        match self {
            PrFilter::All => true,
            PrFilter::Open => request.is_open(),
            PrFilter::NeedsYou => request.needs_you(),
            PrFilter::Closed => !request.is_open(),
        }
    }
}

impl Pulse {
    /// The person's open pull requests (`mine` also has the ones merged or closed of late).
    pub(crate) fn open(&self) -> impl Iterator<Item = &PullRequest> {
        self.mine.iter().filter(|request| request.is_open())
    }

    /// The tab's list: the person's pull requests and the ones that ask for their review, the last changed first, the
    /// ones `filter` keeps.
    pub(crate) fn listed(&self, filter: PrFilter) -> Vec<&PullRequest> {
        let mut list: Vec<&PullRequest> = self.mine.iter().collect();
        list.extend(self.to_review.iter().filter(|request| !self.mine.iter().any(|known| known.id == request.id)));
        list.retain(|request| filter.keeps(request));
        list.sort_by_key(|request| std::cmp::Reverse(request.updated_at));
        list
    }

    /// A pull request of the tab by its id, in any case ("Ana/Loja#42" is "ana/loja#42": GitHub's names ignore it).
    pub(crate) fn find(&self, id: &str) -> Option<&PullRequest> {
        self.mine.iter().chain(&self.to_review).find(|request| request.id.eq_ignore_ascii_case(id))
    }
}

/// The checks of a `statusCheckRollup` that failed, each name once, up to 5: a check run by its conclusion, a commit
/// status by its state. A page that is not https is dropped.
pub(super) fn failing_checks(rollup: Option<&Value>) -> Vec<Check> {
    let mut found: Vec<Check> = Vec::new();
    for context in nodes(rollup.and_then(|rollup| rollup.get("contexts"))) {
        let status = context.get("context").is_some();
        let (name, state, page) = match status {
            true => (text_at(context, "context", 120), context.get("state"), "targetUrl"),
            false => (text_at(context, "name", 120), context.get("conclusion"), "detailsUrl"),
        };
        let failed = context_failed(status, state.and_then(Value::as_str));
        let Some(name) = name.filter(|_| failed) else { continue };
        if found.iter().any(|check| check.name == name) {
            continue;
        }
        let url = text_at(context, page, 400).filter(|url| url.starts_with("https://") && !url.contains(' '));
        found.push(Check { name, url });
        if found.len() == FAILING_MOST {
            break;
        }
    }
    found
}

fn context_failed(status: bool, state: Option<&str>) -> bool {
    match (status, state) {
        (true, Some(state)) => matches!(state, "ERROR" | "FAILURE"),
        (false, Some(state)) => FAILED_RUNS.contains(&state),
        _ => false,
    }
}

/// A pull request's node; None when a field is missing or its address is not a GitHub page (it is opened on a click).
pub(super) fn pull_request(node: &Value) -> Option<PullRequest> {
    let number = node.get("number")?.as_i64().filter(|number| *number > 0)?;
    let repository = text_at(node.get("repository")?, "nameWithOwner", 140)?;
    let url = text_at(node, "url", 400).filter(|url| url.starts_with("https://github.com/"))?;
    let commit =
        node.get("commits").and_then(|commits| commits.get("nodes")).and_then(Value::as_array).and_then(|list| list.last()).and_then(|last| last.get("commit"));
    let rollup = commit.and_then(|commit| commit.get("statusCheckRollup"));
    let review = match node.get("reviewDecision").and_then(Value::as_str) {
        Some("APPROVED") => "aprovado",
        Some("CHANGES_REQUESTED") => CHANGES_REQUESTED,
        Some("REVIEW_REQUIRED") => "aguarda revisão",
        _ => "",
    };
    let raw = |key: &str| node.get(key).and_then(Value::as_str);
    Some(PullRequest {
        id: format!("{repository}#{number}"),
        title: text_at(node, "title", 200)?,
        url,
        draft: node.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
        ci: CiState::from_github(rollup.and_then(|rollup| rollup.get("state")).and_then(Value::as_str)),
        review,
        head: commit.and_then(|commit| text_at(commit, "oid", 64)),
        state: PrState::from_github(raw("state")),
        mergeable: MergeableState::from_github(raw("mergeable")),
        head_ref: text_at(node, "headRefName", 200).unwrap_or_default(),
        base_ref: text_at(node, "baseRefName", 200).unwrap_or_default(),
        updated_at: raw("updatedAt").and_then(crate::transcript::iso_ms).unwrap_or(0),
        node_id: text_at(node, "id", 120).unwrap_or_default(),
        failing: failing_checks(rollup),
        review_requested_of_me: false,
    })
}
