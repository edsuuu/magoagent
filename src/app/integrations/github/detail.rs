//! A pull request's detail on the island's GitHub tab, read when it is picked (the polls never carry a description):
//! its description, the latest review of each reviewer and what blocks it (the writes are in `write.rs`). Each read is
//! one GraphQL request off the UI's thread, as the polls; the request bodies and what the answers say are plain
//! functions, away from the network.

use super::{CiState, Failure, GitHub, MergeableState, Message, PrFilter, PrState, PullRequest, ago, nodes, pull_request, text_at};
use crate::prelude::*;

/// The description is shown and edited up to this many characters.
const BODY_MOST: usize = 20_000;
/// A detail older than this is read again when the MCP asks for it, ms.
const DETAIL_FRESH: i64 = 60_000;

/// A pull request with its description and its reviews; its failed checks among the first 100.
const DETAIL_QUERY: &str = r#"query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      id number title url body isDraft state mergeable reviewDecision updatedAt headRefName baseRefName
      repository { nameWithOwner viewerDefaultMergeMethod }
      latestOpinionatedReviews(first: 50) { nodes { state author { login } } }
      commits(last: 1) { nodes { commit { oid statusCheckRollup { state contexts(first: 100) { nodes {
        ... on CheckRun { name conclusion detailsUrl }
        ... on StatusContext { context state targetUrl }
      } } } } } }
    }
  }
  rateLimit { remaining }
}"#;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PrDetail {
    /// The pull request as this read saw it: `mergeable`, `failing` (up to 5 of its first 100 checks) and the rest fresh.
    pub(crate) request: PullRequest,
    /// The description in Markdown, up to 20 000 characters.
    pub(crate) body: String,
    /// The description was longer: only its start is here, and the app does not save it (that would cut it).
    pub(crate) truncated: bool,
    /// Reviewers whose latest review approved it, and who asked for changes.
    pub(crate) approved: usize,
    pub(crate) changes_requested: usize,
    /// How the person merges in its repository, as GitHub names it: MERGE, SQUASH or REBASE.
    pub(crate) merge_method: &'static str,
    /// When it was read, ms.
    pub(crate) fetched_at: i64,
}

/// One thing that holds a pull request back, and the page that shows it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Blocker {
    pub(crate) text: String,
    pub(crate) url: Option<String>,
}

impl PrDetail {
    /// The detail of a `DETAIL_QUERY` answer; None without the pull request.
    pub(crate) fn parse(root: &Value, now: i64) -> Option<PrDetail> {
        let node = root.get("data")?.get("repository")?.get("pullRequest").filter(|node| node.is_object())?;
        let request = pull_request(node)?;
        let (body, truncated) = body_text(node.get("body").and_then(Value::as_str).unwrap_or_default());
        // one review a reviewer, the latest (GitHub already gives one each; a review with no author counts alone)
        let mut latest: Vec<(String, String)> = Vec::new();

        for review in nodes(node.get("latestOpinionatedReviews")) {
            let who = review.get("author").and_then(|author| text_at(author, "login", 60)).unwrap_or_default();
            let state = review.get("state").and_then(Value::as_str).unwrap_or_default().to_string();

            match latest.iter_mut().find(|(login, _)| !who.is_empty() && *login == who) {
                Some(known) => known.1 = state,
                None => latest.push((who, state)),
            }
        }

        let count = |wanted: &str| latest.iter().filter(|(_, state)| state == wanted).count();

        let merge_method = match node.get("repository").and_then(|repository| repository.get("viewerDefaultMergeMethod")).and_then(Value::as_str) {
            Some("SQUASH") => "SQUASH",
            Some("REBASE") => "REBASE",
            _ => "MERGE",
        };

        let (approved, changes_requested) = (count("APPROVED"), count("CHANGES_REQUESTED"));

        Some(PrDetail { approved, changes_requested, merge_method, request, body, truncated, fetched_at: now })
    }

    /// What blocks it, in order: a conflict with its base, each failed check, the changes asked for. Nothing once it
    /// is merged or closed.
    pub(crate) fn blockers(&self) -> Vec<Blocker> {
        let request = &self.request;
        let mut found = Vec::new();

        if !request.is_open() {
            return found;
        }

        if request.conflicts() {
            let base = if request.base_ref.is_empty() { "a base".to_string() } else { request.base_ref.clone() };

            found.push(Blocker { text: format!("Conflito com {base}"), url: Some(request.url.clone()) });
        }

        found.extend(request.failing.iter().map(|check| Blocker { text: format!("Check falhou: {}", check.name), url: check.url.clone() }));

        if request.failing.is_empty() && request.ci == CiState::Failure {
            found.push(Blocker { text: "Os checks falharam".into(), url: Some(request.checks_page()) });
        }

        let changes = match self.changes_requested {
            0 if request.changes_requested() => "Mudanças pedidas".to_string(),
            0 => String::new(),
            1 => "1 revisor pediu mudanças".into(),
            count => format!("{count} revisores pediram mudanças"),
        };

        if !changes.is_empty() {
            found.push(Blocker { text: changes, url: Some(format!("{}/files", request.url)) });
        }

        found
    }

    /// "2 aprovações · 1 pedido de mudanças"; empty when nobody reviewed it.
    pub(crate) fn reviews_words(&self) -> String {
        let mut parts = Vec::new();

        match self.approved {
            0 => {}
            1 => parts.push("1 aprovação".to_string()),
            count => parts.push(format!("{count} aprovações")),
        }

        match self.changes_requested {
            0 => {}
            1 => parts.push("1 pedido de mudanças".to_string()),
            count => parts.push(format!("{count} pedidos de mudanças")),
        }

        parts.join(" · ")
    }

    /// Why Mesclar cannot be pressed now, if so (GitHub may still refuse: a required check, a review, the permission).
    pub(crate) fn unmergeable(&self) -> Option<String> {
        let request = &self.request;

        match (request.state, request.draft, request.mergeable) {
            (PrState::Merged, ..) => Some(format!("{} já foi mesclado.", request.id)),
            (PrState::Closed, ..) => Some(format!("{} está fechado.", request.id)),
            (_, true, _) => Some(format!("{} é um rascunho: marque como pronto para revisão antes.", request.id)),
            (_, _, MergeableState::Conflicting) => Some(format!("{} tem conflito com {}: resolva antes.", request.id, request.base_ref)),
            (_, _, MergeableState::Unknown) => Some("O GitHub ainda está vendo se ele mescla: espere um pouco.".into()),
            _ => None,
        }
    }

    /// The merge in the person's words: "em um commit só (squash)".
    pub(crate) fn merge_words(&self) -> &'static str {
        match self.merge_method {
            "SQUASH" => "em um commit só (squash)",
            "REBASE" => "por rebase",
            _ => "com um commit de merge",
        }
    }

    /// The detail in words, for the MCP's `pr`.
    pub(crate) fn report(&self, now: i64) -> String {
        let mut lines = vec![self.request.line(now)];

        if self.request.is_open() {
            let blockers: Vec<String> = self
                .blockers()
                .iter()
                .map(|blocker| match &blocker.url {
                    Some(url) => format!("{} ({url})", blocker.text),
                    None => blocker.text.clone(),
                })
                .collect();

            lines.push(if blockers.is_empty() { "Nada bloqueia.".into() } else { format!("Bloqueios: {}", blockers.join("; ")) });
        }

        if !self.reviews_words().is_empty() {
            lines.push(format!("Revisões: {}", self.reviews_words()));
        }

        lines.push("Descrição (Markdown):".into());
        lines.push(if self.body.trim().is_empty() { "(sem descrição)".into() } else { self.body.clone() });

        if self.truncated {
            lines.push(format!("(A descrição passa de {BODY_MOST} caracteres: aqui vai só o começo, e o app não a salva.)"));
        }

        lines.join("\n")
    }
}

/// A description as the tab shows and edits it: GitHub's \r\n as \n, no control characters but line breaks and tabs, up
/// to 20 000 characters; and whether it was longer.
pub(super) fn body_text(raw: &str) -> (String, bool) {
    let clean = raw.replace("\r\n", "\n");
    let mut letters = clean.chars().filter(|letter| !letter.is_control() || matches!(letter, '\n' | '\t'));
    let body: String = letters.by_ref().take(BODY_MOST).collect();

    (body, letters.next().is_some())
}

/// "owner/repo#42" or a pull request's address on GitHub, as (owner, repo, number).
pub(crate) fn parse_id(text: &str) -> Option<(String, String, u64)> {
    let text = text.trim();

    let (repo, number) = match text.strip_prefix("https://github.com/") {
        Some(path) => {
            let mut parts = path.split('/');
            let (owner, name) = (parts.next()?, parts.next()?);

            (parts.next()? == "pull").then_some(())?;
            (format!("{owner}/{name}"), parts.next()?.split(['#', '?']).next()?.to_string())
        }
        None => text.split_once('#').map(|(repo, number)| (repo.to_string(), number.to_string()))?,
    };

    let (owner, name) = repo.split_once('/')?;

    let sane = |part: &str| {
        part.len() <= 100 && part.chars().any(|letter| letter != '.') && part.chars().all(|letter| letter.is_ascii_alphanumeric() || "-_.".contains(letter))
    };

    (sane(owner) && sane(name)).then_some(())?;
    let number = number.parse::<u64>().ok().filter(|number| *number > 0)?;

    Some((owner.to_string(), name.to_string(), number))
}

/// The body of the detail's request: the query and which pull request.
pub(crate) fn detail_request(owner: &str, name: &str, number: u64) -> Value {
    json!({ "query": DETAIL_QUERY, "variables": { "owner": owner, "name": name, "number": number } })
}

fn fetch_detail(token: &str, owner: &str, name: &str, number: u64) -> Result<PrDetail, Failure> {
    let root = super::post(token, &detail_request(owner, name, number))?;

    PrDetail::parse(&root, now_ms()).ok_or_else(|| {
        let message = match text_at(&root["errors"][0], "message", 160) {
            Some(why) => format!("O GitHub recusou a consulta: {why}"),
            None => "O GitHub não achou esse pull request.".into(),
        };

        Failure { message, unauthorized: false }
    })
}

/// A pull request's detail as it stands: the last one read, a read out, why the last one failed.
#[derive(Default)]
pub(super) struct DetailEntry {
    pub(super) detail: Option<PrDetail>,
    pub(super) loading: bool,
    pub(super) error: Option<String>,
}

pub(super) fn unknown(id: &str) -> String {
    format!("Não entendi o pull request {id:?}: diga dono/repo#número ou o endereço dele no GitHub.")
}

impl GitHub {
    /// The id the tab knows a pull request by: as the pulse or an earlier read wrote it (GitHub's names ignore case).
    pub(super) fn key(&self, owner: &str, name: &str, number: u64) -> String {
        let id = format!("{owner}/{name}#{number}");
        let known = self.pulse.as_ref().and_then(|pulse| pulse.find(&id)).map(|request| request.id.clone());

        known.or_else(|| self.details.keys().find(|key| key.eq_ignore_ascii_case(&id)).cloned()).unwrap_or(id)
    }

    fn entry(&self, id: &str) -> Option<&DetailEntry> {
        self.details.get(id).or_else(|| parse_id(id).and_then(|(owner, name, number)| self.details.get(&self.key(&owner, &name, number))))
    }

    /// GitHub's id of a pull request's node, from the pulse or a read.
    pub(super) fn node_id(&self, key: &str) -> Option<String> {
        let polled = self.pulse.as_ref().and_then(|pulse| pulse.find(key)).map(|request| request.node_id.clone());
        let read = || self.details.get(key).and_then(|entry| entry.detail.as_ref()).map(|detail| detail.request.node_id.clone());

        polled.filter(|id| !id.is_empty()).or_else(read).filter(|id| !id.is_empty())
    }

    /// Why nothing can be asked of GitHub now, if so.
    pub(super) fn unable(&self, enabled: bool) -> Option<String> {
        if !enabled {
            Some("O GitHub está desligado em Ajustes › Integrações.".into())
        } else if !self.has_token() {
            Some("Sem token do GitHub: a pessoa cola um em Ajustes › Integrações.".into())
        } else if self.refused() {
            Some(self.error.clone().unwrap_or_else(|| "Token inválido ou expirado.".into()))
        } else {
            None
        }
    }

    /// A detail as if GitHub had answered it, for the island's tests.
    #[cfg(test)]
    pub(crate) fn answered(&mut self, detail: PrDetail) {
        self.take_detail(detail.request.id.clone(), Ok(Box::new(detail)), 0);
    }

    pub(super) fn take_detail(&mut self, id: String, result: Result<Box<PrDetail>, Failure>, now: i64) {
        let entry = self.details.entry(id).or_default();

        entry.loading = false;

        match result {
            Ok(detail) => (entry.detail, entry.error) = (Some(*detail), None),
            Err(failure) => {
                entry.error = Some(failure.message.clone());

                if failure.unauthorized {
                    self.failed(failure, now);
                }
            }
        }
    }
}

impl DrawingApp {
    /// Reads a pull request's detail (`id`: "owner/repo#42" or its address); the answer comes by a later tick, into
    /// `github_detail` (meanwhile `github_detail_loading`; a failure in `github_detail_error`). Err when GitHub is
    /// off, has no token, or `id` names no pull request. A read already out is not doubled.
    pub(crate) fn github_load_detail(&mut self, id: &str) -> Result<(), String> {
        let enabled = self.prefs.integrations.github;
        let github = &mut self.integrations.github;

        github.load_token();
        let (owner, name, number) = parse_id(id).ok_or_else(|| unknown(id))?;
        let key = github.key(&owner, &name, number);

        if let Some(why) = github.unable(enabled) {
            github.details.entry(key).or_default().error = Some(why.clone());

            return Err(why);
        }

        let entry = github.details.entry(key.clone()).or_default();

        if entry.loading {
            return Ok(());
        }

        (entry.loading, entry.error) = (true, None);

        if let Some(pulse) = super::fake::pulse() {
            let request = pulse.find(&key).cloned().unwrap_or_default();

            (entry.loading, entry.detail) = (false, Some(PrDetail { request, body: super::fake::BODY.into(), ..Default::default() }));

            return Ok(());
        }

        // never the network in a test: there the answer is handed to `receive`
        if !cfg!(test) {
            github.spawn(move |token| {
                let result = fetch_detail(token, &owner, &name, number).map(Box::new);

                Message::Detail(key, result)
            });
        }

        Ok(())
    }

    /// The last detail read of a pull request (by its id, "owner/repo#42", or its address).
    pub(crate) fn github_detail(&self, id: &str) -> Option<&PrDetail> {
        self.integrations.github.entry(id).and_then(|entry| entry.detail.as_ref())
    }

    /// A read of the pull request's detail is out.
    pub(crate) fn github_detail_loading(&self, id: &str) -> bool {
        self.integrations.github.entry(id).is_some_and(|entry| entry.loading)
    }

    /// Why the last read of the pull request's detail failed.
    pub(crate) fn github_detail_error(&self, id: &str) -> Option<&str> {
        self.integrations.github.entry(id).and_then(|entry| entry.error.as_deref())
    }

    /// The MCP's `prs`: the tab's list as `filter` (a `PrFilter::key`, empty: all) keeps it, a pull request a line.
    pub(crate) fn github_prs_report(&mut self, filter: &str) -> Result<String, String> {
        let keys = PrFilter::EVERY.map(PrFilter::key).join(", ");

        let filter = match filter {
            "" => PrFilter::All,
            key => PrFilter::EVERY.into_iter().find(|known| known.key() == key).ok_or_else(|| format!("`filter` é um de: {keys}"))?,
        };

        let enabled = self.prefs.integrations.github;
        let github = &mut self.integrations.github;

        github.load_token();

        if let Some(why) = github.unable(enabled) {
            return Err(why);
        }

        let Some(pulse) = &github.pulse else { return Ok("O GitHub ainda não respondeu: pergunte de novo em instantes.".into()) };
        let list = pulse.listed(filter);

        if list.is_empty() {
            return Ok(format!("@{}: nenhum pull request em {}.", pulse.login, filter.label()));
        }

        let now = now_ms();

        let mut lines = vec![format!(
            "Pull requests de @{} em {} (os mudados por último, abertos, mesclados ou fechados, e os que pedem a revisão dela):",
            pulse.login,
            filter.label()
        )];

        lines.extend(list.iter().map(|request| format!("- {}", request.line(now))));
        Ok(lines.join("\n"))
    }

    /// The MCP's `pr`: the detail when it was read (an old one is read again), else the read is asked for and the
    /// agent asks again.
    pub(crate) fn github_pr_report(&mut self, id: &str) -> Result<String, String> {
        let now = now_ms();
        let (owner, name, number) = parse_id(id).ok_or_else(|| unknown(id))?;
        let key = self.integrations.github.key(&owner, &name, number);

        let (detail, loading, error) =
            (self.github_detail(&key).cloned(), self.github_detail_loading(&key), self.github_detail_error(&key).map(str::to_string));

        let waiting = format!("Buscando {key} no GitHub: chame `pr` de novo em alguns segundos.");

        if let Some(detail) = detail {
            let mut text = detail.report(now);

            if now - detail.fetched_at > DETAIL_FRESH && !loading && self.github_load_detail(&key).is_ok() {
                text += &format!("\n(Lido {}; uma leitura nova foi pedida.)", ago(detail.fetched_at, now));
            }

            return Ok(text);
        }

        if loading {
            return Ok(waiting);
        }

        self.github_load_detail(&key)?;

        match error {
            Some(why) => Err(format!("{why} Pedi de novo: chame `pr` em alguns segundos.")),
            None => Ok(waiting),
        }
    }
}
