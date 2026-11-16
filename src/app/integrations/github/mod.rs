//! GitHub in Ajustes › Integrações and on the island, as Coucou's (MIT) GitHub pulse: the person's open pull requests
//! with the state of their checks, the ones that wait for their review, and the contribution calendar. One GraphQL
//! request a poll (every minute while a check runs, every five otherwise; GraphQL is a POST, so there is no ETag to
//! send: the answer's own `rateLimit` holds the polls back instead), with a personal token that lives in the keychain.
//! What reads the answers and tells the alerts is plain functions, away from the network.

use super::http::{self, Request};
use crate::prelude::*;
use std::sync::mpsc::{Receiver, Sender, channel};

/// The token's item in the keychain.
pub(crate) const TOKEN_ID: &str = "integration-github-token";
pub(crate) const TOKENS_PAGE: &str = "https://github.com/settings/personal-access-tokens";
pub(crate) const PULLS_PAGE: &str = "https://github.com/pulls";
const GRAPHQL: &str = "https://api.github.com/graphql";
const USER: &str = "https://api.github.com/user";
/// Between two polls: while a check runs, otherwise, and when the hour's budget of requests is nearly spent.
const PENDING_EVERY: i64 = 60_000;
const IDLE_EVERY: i64 = 300_000;
const SPARING_EVERY: i64 = 900_000;
const ACTIVITY_EVERY: i64 = 1_800_000;
/// Under this many points left of the hour's 5000, the polls go sparing.
const LOW_BUDGET: i64 = 100;
/// How long an alert stays on the pill once its card was seen, and at most.
const SEEN_FOR: i64 = 6_000;
const NOTICE_FOR: i64 = 600_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CiState {
    Pending,
    Success,
    Failure,
    Unknown,
}

impl CiState {
    /// GitHub's `StatusState` (`statusCheckRollup.state`).
    pub(crate) fn from_github(raw: Option<&str>) -> Self {
        match raw.map(str::to_ascii_uppercase).as_deref() {
            Some("PENDING" | "EXPECTED") => CiState::Pending,
            Some("SUCCESS") => CiState::Success,
            Some("ERROR" | "FAILURE") => CiState::Failure,
            _ => CiState::Unknown,
        }
    }

    /// The mark and the words of a check's state.
    pub(crate) fn label(self) -> (&'static str, &'static str) {
        match self {
            CiState::Pending => ("…", "checks rodando"),
            CiState::Success => ("✓", "checks passaram"),
            CiState::Failure => ("✕", "checks falharam"),
            CiState::Unknown => ("•", "sem checks"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
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
}

impl PullRequest {
    /// The page a click opens: its checks when they failed (where the failure is), else the pull request.
    pub(crate) fn page(&self) -> String {
        if self.ci == CiState::Failure { format!("{}/checks", self.url) } else { self.url.clone() }
    }

    /// "checks passaram · aprovado · rascunho".
    pub(crate) fn state_words(&self) -> String {
        let words = [self.ci.label().1, self.review, if self.draft { "rascunho" } else { "" }];
        words.iter().filter(|word| !word.is_empty()).copied().collect::<Vec<_>>().join(" · ")
    }
}

/// The checks of a repository's default branch, on its last commit.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BranchCi {
    /// "owner/repo".
    pub(crate) repo: String,
    pub(crate) url: String,
    pub(crate) branch: String,
    pub(crate) ci: CiState,
    pub(crate) head: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Pulse {
    pub(crate) login: String,
    pub(crate) mine: Vec<PullRequest>,
    pub(crate) to_review: Vec<PullRequest>,
    /// The default branches of the person's repositories pushed to last (archived ones left out).
    pub(crate) main: Vec<BranchCi>,
    /// Points left of the hour's budget, when the answer says.
    pub(crate) remaining: Option<i64>,
}

/// What a poll can warn of.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Alert {
    CheckFailed(String),
    /// A default branch's checks failed ("owner/repo").
    MainFailed(String),
    ReviewRequested(String),
    CheckPassed(String),
}

/// A text of the answer as the app shows it: one line, no control characters, at most `most` characters.
fn text_at(value: &Value, key: &str, most: usize) -> Option<String> {
    let text: String = value.get(key)?.as_str()?.chars().filter(|letter| !letter.is_control()).take(most).collect();
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn nodes(parent: Option<&Value>) -> &[Value] {
    parent.and_then(|parent| parent.get("nodes")).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// A pull request's node; None when a field is missing or its address is not a GitHub page (it is opened on a click).
fn pull_request(node: &Value) -> Option<PullRequest> {
    let number = node.get("number")?.as_i64().filter(|number| *number > 0)?;
    let repository = text_at(node.get("repository")?, "nameWithOwner", 140)?;
    let url = text_at(node, "url", 400).filter(|url| url.starts_with("https://github.com/"))?;
    let commit =
        node.get("commits").and_then(|commits| commits.get("nodes")).and_then(Value::as_array).and_then(|list| list.last()).and_then(|last| last.get("commit"));
    let review = match node.get("reviewDecision").and_then(Value::as_str) {
        Some("APPROVED") => "aprovado",
        Some("CHANGES_REQUESTED") => "mudanças pedidas",
        Some("REVIEW_REQUIRED") => "aguarda revisão",
        _ => "",
    };
    Some(PullRequest {
        id: format!("{repository}#{number}"),
        title: text_at(node, "title", 200)?,
        url,
        draft: node.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
        ci: CiState::from_github(commit.and_then(|commit| commit.get("statusCheckRollup")).and_then(|rollup| rollup.get("state")).and_then(Value::as_str)),
        review,
        head: commit.and_then(|commit| text_at(commit, "oid", 64)),
    })
}

/// A repository's node with its default branch; None when archived, with no default branch or not a GitHub page.
fn branch_ci(node: &Value) -> Option<BranchCi> {
    if node.get("isArchived").and_then(Value::as_bool).unwrap_or(false) {
        return None;
    }
    let branch = node.get("defaultBranchRef").filter(|branch| branch.is_object())?;
    let target = branch.get("target");
    Some(BranchCi {
        repo: text_at(node, "nameWithOwner", 140)?,
        url: text_at(node, "url", 400).filter(|url| url.starts_with("https://github.com/"))?,
        branch: text_at(branch, "name", 100)?,
        ci: CiState::from_github(target.and_then(|target| target.get("statusCheckRollup")).and_then(|rollup| rollup.get("state")).and_then(Value::as_str)),
        head: target.and_then(|target| text_at(target, "oid", 64)),
    })
}

impl Pulse {
    /// The pulse of a GraphQL answer; None without `data.viewer`.
    pub(crate) fn parse(root: &Value) -> Option<Pulse> {
        let data = root.get("data")?;
        let viewer = data.get("viewer").filter(|viewer| viewer.is_object())?;
        let mut pulse = Pulse { login: text_at(viewer, "login", 60).unwrap_or_default(), ..Default::default() };
        pulse.remaining = data.get("rateLimit").and_then(|limit| limit.get("remaining")).and_then(Value::as_i64);
        for request in nodes(viewer.get("pullRequests")).iter().filter_map(pull_request) {
            if !pulse.mine.iter().any(|known| known.id == request.id) {
                pulse.mine.push(request);
            }
        }
        for request in nodes(data.get("reviewRequested")).iter().filter_map(pull_request) {
            if !pulse.to_review.iter().any(|known| known.id == request.id) {
                pulse.to_review.push(request);
            }
        }
        for branch in nodes(viewer.get("repositories")).iter().filter_map(branch_ci) {
            if !pulse.main.iter().any(|known| known.repo == branch.repo) {
                pulse.main.push(branch);
            }
        }
        Some(pulse)
    }

    /// A check still runs, of a pull request or a default branch: the next poll comes sooner.
    pub(crate) fn has_pending(&self) -> bool {
        self.mine.iter().any(|request| request.ci == CiState::Pending) || self.main.iter().any(|branch| branch.ci == CiState::Pending)
    }

    /// "3 PRs · 1 falhou · 2 para revisar".
    pub(crate) fn summary(&self) -> String {
        let failed = self.mine.iter().filter(|request| request.ci == CiState::Failure).count();
        let mut parts = vec![match self.mine.len() {
            0 => "nenhum PR aberto".to_string(),
            1 => "1 PR aberto".to_string(),
            count => format!("{count} PRs abertos"),
        }];
        if failed > 0 {
            parts.push(format!("{failed} com checks falhando"));
        }
        if !self.to_review.is_empty() {
            parts.push(format!("{} para revisar", self.to_review.len()));
        }
        parts.join(" · ")
    }
}

/// What changed between two polls, as Coucou tells it. The first poll (no `old`) is silent. On the same commit, the
/// plain changes of state; on a new commit or a new pull request, its checks at once if they already ended (a run
/// shorter than the interval), and nothing while they run: the next poll, on the same commit, sees them end. A
/// default branch only ever tells that it failed, never that it passed.
pub(crate) fn alerts(old: Option<&Pulse>, new: &Pulse) -> Vec<Alert> {
    let Some(old) = old else { return Vec::new() };
    let mut found = Vec::new();
    for request in &new.mine {
        match old.mine.iter().find(|known| known.id == request.id) {
            Some(before) if before.head == request.head => {
                if request.ci == CiState::Failure && before.ci != CiState::Failure {
                    found.push(Alert::CheckFailed(request.id.clone()));
                } else if request.ci == CiState::Success && before.ci == CiState::Pending {
                    found.push(Alert::CheckPassed(request.id.clone()));
                }
            }
            _ => match request.ci {
                CiState::Failure => found.push(Alert::CheckFailed(request.id.clone())),
                CiState::Success => found.push(Alert::CheckPassed(request.id.clone())),
                _ => {}
            },
        }
    }
    for branch in &new.main {
        let failed = match old.main.iter().find(|known| known.repo == branch.repo) {
            Some(before) if before.head == branch.head => branch.ci == CiState::Failure && before.ci != CiState::Failure,
            _ => branch.ci == CiState::Failure,
        };
        if failed {
            found.push(Alert::MainFailed(branch.repo.clone()));
        }
    }
    for request in &new.to_review {
        if !old.to_review.iter().any(|known| known.id == request.id) {
            found.push(Alert::ReviewRequested(request.id.clone()));
        }
    }
    found
}

/// The one alert a poll shows: a failure before a review asked for, that before a success.
pub(crate) fn foremost(alerts: &[Alert]) -> Option<&Alert> {
    let rank = |alert: &Alert| match alert {
        Alert::CheckFailed(_) | Alert::MainFailed(_) => 0,
        Alert::ReviewRequested(_) => 1,
        Alert::CheckPassed(_) => 2,
    };
    alerts.iter().min_by_key(|alert| rank(alert))
}

/// The contribution calendar: the year's total and each week's days as levels 0 to 4, the oldest week first.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Activity {
    pub(crate) total: i64,
    pub(crate) weeks: Vec<Vec<u8>>,
    /// Contributions in the last day of the calendar (today).
    pub(crate) today: i64,
}

impl Activity {
    pub(crate) fn parse(root: &Value) -> Option<Activity> {
        let calendar = root.get("data")?.get("viewer")?.get("contributionsCollection")?.get("contributionCalendar")?;
        let mut activity = Activity { total: calendar.get("totalContributions")?.as_i64()?.max(0), ..Default::default() };
        // a year is 53 weeks of 7 days: anything longer is not GitHub's answer
        for week in calendar.get("weeks")?.as_array()?.iter().take(60) {
            let Some(days) = week.get("contributionDays").and_then(Value::as_array) else { continue };
            let levels: Vec<u8> = days
                .iter()
                .take(7)
                .map(|day| match day.get("contributionLevel").and_then(Value::as_str) {
                    Some("FIRST_QUARTILE") => 1,
                    Some("SECOND_QUARTILE") => 2,
                    Some("THIRD_QUARTILE") => 3,
                    Some("FOURTH_QUARTILE") => 4,
                    _ => 0,
                })
                .collect();
            if let Some(count) = days.iter().take(7).next_back().and_then(|day| day.get("contributionCount")).and_then(Value::as_i64) {
                activity.today = count.max(0);
            }
            if !levels.is_empty() {
                activity.weeks.push(levels);
            }
        }
        Some(activity)
    }
}

/// The reference's pulse query (with the default branches of the 10 repositories pushed to last), plus what is left of the
/// hour's budget.
const PULSE_QUERY: &str = r#"query {
  viewer {
    login
    pullRequests(states: OPEN, first: 20, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        number title url isDraft reviewDecision
        repository { nameWithOwner }
        commits(last: 1) { nodes { commit { oid statusCheckRollup { state } } } }
      }
    }
    repositories(first: 10, ownerAffiliations: [OWNER], orderBy: {field: PUSHED_AT, direction: DESC}) {
      nodes {
        nameWithOwner url isArchived
        defaultBranchRef { name target { ... on Commit { oid statusCheckRollup { state } } } }
      }
    }
  }
  reviewRequested: search(query: "is:pr is:open review-requested:@me archived:false", type: ISSUE, first: 20) {
    nodes { ... on PullRequest { number title url isDraft repository { nameWithOwner } } }
  }
  rateLimit { remaining }
}"#;

const ACTIVITY_QUERY: &str = r#"query {
  viewer { contributionsCollection { contributionCalendar {
    totalContributions
    weeks { contributionDays { contributionCount contributionLevel } }
  } } }
}"#;

/// Why a request failed; `unauthorized` stops the polls until the token changes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Failure {
    pub(crate) message: String,
    pub(crate) unauthorized: bool,
}

/// The token as GitHub makes them: printable ASCII with no spaces, of a sane length.
pub(crate) fn token_looks_right(token: &str) -> bool {
    (20..=255).contains(&token.len()) && token.chars().all(|letter| letter.is_ascii_graphic())
}

fn request<'a>(token: &str, method: &'a str, url: &'a str, body: Option<String>) -> Request<'a> {
    let mut headers = vec![format!("Authorization: Bearer {token}"), "Accept: application/vnd.github+json".into(), "User-Agent: MagoAgent".into()];
    if body.is_some() {
        headers.push("Content-Type: application/json".into());
    }
    Request { method, url, headers, body, timeout_seconds: 15 }
}

/// GitHub's answer as JSON, or why not, by its HTTP status.
fn read(answer: Result<http::Answer, String>) -> Result<Value, Failure> {
    let failure = |message: &str, unauthorized| Failure { message: message.to_string(), unauthorized };
    let answer = answer.map_err(|_| failure("Sem conexão com o GitHub.", false))?;
    match answer.status {
        200 => serde_json::from_str(&answer.body).map_err(|_| failure("A resposta do GitHub não foi entendida.", false)),
        401 => Err(failure("Token inválido ou expirado.", true)),
        403 | 429 => Err(failure("O GitHub recusou: limite de uso atingido ou token sem permissão.", false)),
        status => Err(failure(&format!("O GitHub respondeu {status}."), false)),
    }
}

fn graphql(token: &str, query: &str) -> Result<Value, Failure> {
    let root = read(http::send(&request(token, "POST", GRAPHQL, Some(json!({ "query": query }).to_string()))))?;
    if root.get("data").is_none_or(Value::is_null) {
        let why = text_at(&root["errors"][0], "message", 160).unwrap_or_else(|| "sem dados".into());
        return Err(Failure { message: format!("O GitHub recusou a consulta: {why}"), unauthorized: false });
    }
    Ok(root)
}

/// Who the token is (`GET /user`): Testar's answer.
fn fetch_login(token: &str) -> Result<String, Failure> {
    let user = read(http::send(&request(token, "GET", USER, None)))?;
    text_at(&user, "login", 60).ok_or(Failure { message: "A resposta do GitHub não tem o login.".into(), unauthorized: false })
}

enum Message {
    Pulse(Result<Pulse, Failure>),
    Activity(Result<Activity, Failure>),
    Login(Result<String, Failure>),
}

/// What the pill warns of.
pub(crate) struct Notice {
    pub(crate) text: String,
    pub(crate) failure: bool,
    /// The page the card's line opens.
    pub(crate) url: String,
    at: i64,
    seen: Option<i64>,
}

#[derive(Default)]
pub(crate) struct GitHub {
    pub(crate) pulse: Option<Pulse>,
    pub(crate) activity: Option<Activity>,
    /// Why the last request failed.
    pub(crate) error: Option<String>,
    pub(crate) notice: Option<Notice>,
    /// The alert to sound on the island, and when it came (the island takes it; an old one is dropped).
    pub(crate) ring: Option<(Alert, i64)>,
    /// Testar's request is out.
    pub(crate) testing: bool,
    /// The token as the keychain has it, read once; empty: none.
    token: Option<String>,
    /// Counts the tokens: an answer for an earlier one is dropped.
    generation: u64,
    channel: Option<(Sender<(u64, Message)>, Receiver<(u64, Message)>)>,
    busy_pulse: bool,
    busy_activity: bool,
    /// When the next polls are due, ms; `i64::MAX` once the token was refused.
    next_pulse: i64,
    next_activity: i64,
}

impl GitHub {
    /// A GitHub whose token is `token` without the keychain (empty: none), for the tests.
    #[cfg(test)]
    pub(crate) fn with_token(token: &str) -> Self {
        Self { token: Some(token.to_string()), ..Default::default() }
    }

    pub(crate) fn has_token(&self) -> bool {
        self.token.as_deref().is_some_and(|token| !token.is_empty())
    }

    /// A token is there and the last answer was good.
    pub(crate) fn connected(&self) -> bool {
        self.has_token() && self.pulse.is_some() && self.error.is_none()
    }

    /// The token out of the keychain, once (the tests' keychain is the mock one).
    fn load_token(&mut self) {
        #[cfg(test)]
        mock_keyring();
        if self.token.is_none() {
            self.token = Some(take_secret(TOKEN_ID).unwrap_or_default());
        }
    }

    /// The integration in words, for the MCP: who, each pull request with its checks, what waits for a review.
    pub(crate) fn report(&self) -> String {
        if !self.has_token() {
            return "GitHub: sem token. A pessoa cola um em Ajustes › Integrações (ele nunca passa pelo MCP).".into();
        }
        if let Some(error) = &self.error {
            return format!("GitHub: {error}");
        }
        let Some(pulse) = &self.pulse else { return "GitHub: token no cofre, ainda sem resposta.".into() };
        let mut lines = vec![format!("GitHub: conectado como @{} · {}", pulse.login, pulse.summary())];
        lines.extend(pulse.mine.iter().map(|request| format!("- {} {} ({}) {}", request.id, request.title, request.state_words(), request.url)));
        lines.extend(pulse.to_review.iter().map(|request| format!("- revisar {} {} {}", request.id, request.title, request.url)));
        lines.extend(
            pulse
                .main
                .iter()
                .filter(|branch| branch.ci != CiState::Unknown)
                .map(|branch| format!("- {} ({}): {}", branch.repo, branch.branch, branch.ci.label().1)),
        );
        if let Some(activity) = &self.activity {
            lines.push(format!("{} contribuições no último ano, {} hoje.", activity.total, activity.today));
        }
        lines.join("\n")
    }

    /// What was fetched goes, so the next poll is silent: for another token, or the integration turned off.
    fn forget(&mut self) {
        self.generation += 1;
        (self.pulse, self.activity, self.error, self.notice, self.ring) = (None, None, None, None, None);
        (self.busy_pulse, self.busy_activity, self.testing) = (false, false, false);
        (self.next_pulse, self.next_activity) = (0, 0);
    }

    fn spawn(&mut self, work: impl FnOnce(&str) -> Message + Send + 'static) {
        let sender = self.channel.get_or_insert_with(channel).0.clone();
        let (generation, token) = (self.generation, self.token.clone().unwrap_or_default());
        std::thread::spawn(move || sender.send((generation, work(&token))));
    }

    /// A good pulse: the alert it brings, and when the next one is due.
    pub(crate) fn take_pulse(&mut self, pulse: Pulse, now: i64) -> Option<Alert> {
        let alert = foremost(&alerts(self.pulse.as_ref(), &pulse)).cloned();
        if let Some(alert) = &alert {
            let (id, text, failure) = match alert {
                Alert::CheckFailed(id) => (id, format!("Os checks de {id} falharam"), true),
                Alert::MainFailed(repo) => {
                    let branch = pulse.main.iter().find(|branch| &branch.repo == repo).map_or("principal", |branch| branch.branch.as_str());
                    (repo, format!("O CI da {branch} de {repo} falhou"), true)
                }
                Alert::ReviewRequested(id) => (id, format!("Pediram a sua revisão em {id}"), false),
                Alert::CheckPassed(id) => (id, format!("Os checks de {id} passaram"), false),
            };
            let url = match alert {
                // the repository's Actions, where the failed run is
                Alert::MainFailed(_) => pulse.main.iter().find(|branch| &branch.repo == id).map(|branch| format!("{}/actions", branch.url)),
                _ => pulse.mine.iter().chain(&pulse.to_review).find(|request| &request.id == id).map(PullRequest::page),
            };
            self.notice = Some(Notice { text, failure, url: url.unwrap_or_default(), at: now, seen: None });
            self.ring = Some((alert.clone(), now));
        }
        let wait = match (pulse.remaining.is_some_and(|left| left < LOW_BUDGET), pulse.has_pending()) {
            (true, _) => SPARING_EVERY,
            (_, true) => PENDING_EVERY,
            _ => IDLE_EVERY,
        };
        (self.next_pulse, self.error, self.pulse) = (now + wait, None, Some(pulse));
        alert
    }
}

impl DrawingApp {
    /// Each frame: the token read once, the answers taken, the polls that are due sent. Off, nothing reaches the network.
    pub(crate) fn github_tick(&mut self, ctx: &Context) {
        let enabled = self.prefs.integrations.github;
        let github = &mut self.integrations.github;
        if !enabled {
            if github.pulse.is_some() || github.error.is_some() || github.busy_pulse {
                github.forget();
            }
            return;
        }
        github.load_token();
        let now = now_ms();
        let mut toast = None;
        while let Some((generation, message)) = github.channel.as_ref().and_then(|(_, receiver)| receiver.try_recv().ok()) {
            if generation != github.generation {
                continue;
            }
            let failed = |github: &mut GitHub, failure: Failure| {
                github.next_pulse = if failure.unauthorized { i64::MAX } else { now + IDLE_EVERY };
                github.error = Some(failure.message);
            };
            match message {
                Message::Pulse(result) => {
                    github.busy_pulse = false;
                    match result {
                        Ok(pulse) => toast = github.take_pulse(pulse, now).and(github.notice.as_ref().map(|notice| format!("GitHub: {}.", notice.text))),
                        Err(failure) => failed(github, failure),
                    }
                }
                Message::Activity(result) => {
                    github.busy_activity = false;
                    github.next_activity = now + ACTIVITY_EVERY;
                    github.activity = result.ok().or(github.activity.take());
                }
                Message::Login(result) => {
                    github.testing = false;
                    toast = Some(match result {
                        Ok(login) => {
                            github.error = None;
                            format!("GitHub conectado como @{login}.")
                        }
                        Err(failure) => {
                            let message = format!("GitHub: {}", failure.message);
                            failed(github, failure);
                            message
                        }
                    });
                }
            }
        }
        if let Some(notice) = &github.notice
            && (now - notice.at > NOTICE_FOR || notice.seen.is_some_and(|seen| now - seen > SEEN_FOR))
        {
            github.notice = None;
        }
        // never the network in a test
        if github.has_token() && !cfg!(test) {
            if !github.busy_pulse && now >= github.next_pulse {
                github.busy_pulse = true;
                github.spawn(|token| {
                    Message::Pulse(graphql(token, PULSE_QUERY).and_then(|root| {
                        Pulse::parse(&root).ok_or(Failure { message: "A resposta do GitHub veio sem os seus dados.".into(), unauthorized: false })
                    }))
                });
            }
            if !github.busy_activity && now >= github.next_activity && github.next_pulse != i64::MAX {
                github.busy_activity = true;
                github.spawn(|token| {
                    Message::Activity(
                        graphql(token, ACTIVITY_QUERY).and_then(|root| Activity::parse(&root).ok_or(Failure { message: String::new(), unauthorized: false })),
                    )
                });
            }
            // the answers come by the clock, with nobody moving the mouse
            ctx.request_repaint_after(std::time::Duration::from_secs(if github.busy_pulse || github.busy_activity || github.testing { 1 } else { 30 }));
        }
        if let Some(message) = toast {
            self.toast(message);
        }
    }

    /// Salvar: the token into the keychain (never a file), and the polls start over with it.
    pub(crate) fn github_save_token(&mut self, token: &str) -> Result<String, String> {
        #[cfg(test)]
        mock_keyring();
        let token = token.trim();
        if !token_looks_right(token) {
            return Err("Isso não parece um token do GitHub: cole o token inteiro, sem espaços.".into());
        }
        set_secret(TOKEN_ID, token).map_err(|why| format!("O cofre do sistema recusou o token: {why}"))?;
        let github = &mut self.integrations.github;
        github.forget();
        github.token = Some(token.to_string());
        self.github_test();
        Ok("Token do GitHub guardado no cofre do sistema.".into())
    }

    /// Remover: the token out of the keychain and everything fetched with it forgotten.
    pub(crate) fn github_remove_token(&mut self) -> Result<String, String> {
        #[cfg(test)]
        mock_keyring();
        remove_secret(TOKEN_ID)?;
        let github = &mut self.integrations.github;
        github.forget();
        github.token = Some(String::new());
        Ok("Token do GitHub removido.".into())
    }

    /// Testar: asks GitHub who the token is; the answer comes as a toast.
    pub(crate) fn github_test(&mut self) -> bool {
        let github = &mut self.integrations.github;
        github.load_token();
        if !github.has_token() || github.testing {
            return false;
        }
        (github.testing, github.next_pulse) = (!cfg!(test), 0);
        if !cfg!(test) {
            github.spawn(|token| Message::Login(fetch_login(token)));
        }
        true
    }

    /// GitHub in words, for the MCP's `integrations`: off, or what `GitHub::report` says.
    pub(crate) fn github_report(&mut self) -> String {
        if !self.prefs.integrations.github {
            return "GitHub: desligado.".into();
        }
        let github = &mut self.integrations.github;
        github.load_token();
        github.report()
    }

    /// The alert's card was shown: it goes a moment later.
    pub(crate) fn github_seen(&mut self) {
        if let Some(notice) = self.integrations.github.notice.as_mut() {
            notice.seen.get_or_insert_with(now_ms);
        }
    }
}

#[cfg(test)]
mod tests;
