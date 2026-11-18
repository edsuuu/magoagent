//! The island's GitHub tab's writes on a pull request: the description saved, a comment posted, an approval and a merge.
//! Each is one GraphQL request off the UI's thread, as the polls; the request bodies and what the answers say are plain
//! functions, away from the network.

use super::{Failure, GitHub, Message, body_text, http, parse_id, text_at, unknown};
use crate::prelude::*;
/// What GitHub takes in a description or a comment.
const GITHUB_MOST: usize = 65_536;
/// What a token that may only read hears when it writes.
pub(crate) const NO_WRITE: &str = "O token não pode escrever em pull requests: dê a permissão Pull requests (leitura e escrita) ao token.";

const SAVE_BODY: &str = r#"mutation($id: ID!, $body: String!) {
  updatePullRequest(input: {pullRequestId: $id, body: $body}) { pullRequest { id } }
}"#;

const ADD_COMMENT: &str = r#"mutation($id: ID!, $body: String!) {
  addComment(input: {subjectId: $id, body: $body}) { commentEdge { node { url } } }
}"#;

const APPROVE: &str = r#"mutation($id: ID!) {
  addPullRequestReview(input: {pullRequestId: $id, event: APPROVE}) { pullRequestReview { id } }
}"#;

/// Only while its last commit is the one read: a push in between is not merged unseen.
const MERGE: &str = r#"mutation($id: ID!, $method: PullRequestMergeMethod!, $head: GitObjectID) {
  mergePullRequest(input: {pullRequestId: $id, mergeMethod: $method, expectedHeadOid: $head}) { pullRequest { id } }
}"#;

/// The body of Salvar's request: the pull request's node and its whole new description.
pub(crate) fn save_body_request(node_id: &str, body: &str) -> Value {
    json!({ "query": SAVE_BODY, "variables": { "id": node_id, "body": body } })
}

/// The body of Comentar's request: the pull request's node and the comment.
pub(crate) fn comment_request(node_id: &str, text: &str) -> Value {
    json!({ "query": ADD_COMMENT, "variables": { "id": node_id, "body": text } })
}

/// The body of Aprovar's request.
pub(crate) fn approve_request(node_id: &str) -> Value {
    json!({ "query": APPROVE, "variables": { "id": node_id } })
}

/// The body of Mesclar's request: how, and the last commit it was read at (none: any).
pub(crate) fn merge_request(node_id: &str, method: &str, head: Option<&str>) -> Value {
    json!({ "query": MERGE, "variables": { "id": node_id, "method": method, "head": head } })
}

/// What a write's answer says: done (`data.<field>` came), or why not; a token that may only read is told what to give it.
pub(super) fn written(answer: Result<http::Answer, String>, field: &str) -> Result<(), Failure> {
    let refused = |message: &str| Failure { message: message.to_string(), unauthorized: false };
    // a 403 is the token's permission, unless it is the hour's budget
    if let Ok(answer) = &answer
        && answer.status == 403
        && !answer.body.to_ascii_lowercase().contains("rate limit")
    {
        return Err(refused(NO_WRITE));
    }
    let root = super::read(answer)?;
    if let Some(error) = root.get("errors").and_then(Value::as_array).and_then(|errors| errors.first()) {
        let kind = error.get("type").and_then(Value::as_str).unwrap_or_default();
        let message = text_at(error, "message", 200).unwrap_or_default();
        let lower = message.to_lowercase();
        if matches!(kind, "FORBIDDEN" | "INSUFFICIENT_SCOPES") || ["not accessible", "permission", "scope"].iter().any(|word| lower.contains(word)) {
            return Err(refused(NO_WRITE));
        }
        return Err(refused(&format!("O GitHub recusou: {}", if message.is_empty() { kind } else { &message })));
    }
    match root.get("data").and_then(|data| data.get(field)).is_some_and(Value::is_object) {
        true => Ok(()),
        false => Err(refused("O GitHub não confirmou a escrita.")),
    }
}

fn send_write(token: &str, payload: &Value, field: &str) -> Result<(), Failure> {
    written(http::send(&super::request(token, "POST", super::GRAPHQL, Some(payload.to_string()))), field)
}

/// One of the tab's writes, with its text.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Write {
    Body(String),
    Comment(String),
    Approve,
    /// How (MERGE, SQUASH or REBASE), and the last commit it was read at.
    Merge(&'static str, Option<String>),
}

impl Write {
    fn done(&self) -> &'static str {
        match self {
            Write::Body(_) => "Descrição salva.",
            Write::Comment(_) => "Comentário enviado.",
            Write::Approve => "Pull request aprovado.",
            Write::Merge(..) => "Pull request mesclado.",
        }
    }
}

impl GitHub {
    pub(super) fn take_write(&mut self, write: Write, id: String, result: Result<(), Failure>, now: i64) {
        self.writing = false;
        self.last_write = Some(match result {
            Ok(()) => {
                // the description read is now the one saved
                if let Write::Body(body) = &write
                    && let Some(detail) = self.details.get_mut(&id).and_then(|entry| entry.detail.as_mut())
                {
                    (detail.body, detail.truncated) = body_text(body);
                }
                // approved or merged: it is read again, and the list soon after
                if matches!(write, Write::Approve | Write::Merge(..)) {
                    self.details.remove(&id);
                    self.next_pulse = self.next_pulse.min(now);
                }
                Ok(write.done().to_string())
            }
            Err(failure) => {
                let message = failure.message.clone();
                if failure.unauthorized {
                    self.failed(failure, now);
                }
                Err(message)
            }
        });
    }
}

impl DrawingApp {
    /// Salvar: `body` becomes the pull request's whole description. The outcome comes by a later tick, in
    /// `github_last_write`; Err (also kept there) when it cannot be sent.
    pub(crate) fn github_save_body(&mut self, id: &str, body: &str) -> Result<(), String> {
        self.github_write(id, Write::Body(body.to_string()))
    }

    /// Comentar: posts `text` as a new comment on the pull request. As `github_save_body`.
    pub(crate) fn github_comment(&mut self, id: &str, text: &str) -> Result<(), String> {
        self.github_write(id, Write::Comment(text.to_string()))
    }

    /// Aprovar: an approving review of someone else's pull request. As `github_save_body`.
    pub(crate) fn github_approve(&mut self, id: &str) -> Result<(), String> {
        self.github_write(id, Write::Approve)
    }

    /// Mesclar: merges the pull request as its repository merges, only while its last commit is the one read. It must
    /// have been read (`github_load_detail`) and be ready. As `github_save_body`.
    pub(crate) fn github_merge(&mut self, id: &str) -> Result<(), String> {
        let detail = parse_id(id).and_then(|(owner, name, number)| self.github_detail(&self.integrations.github.key(&owner, &name, number)).cloned());
        let ready = match &detail {
            None => Err(format!("Leia {id} antes de mesclar (`pr`): o app confere se ele está pronto.")),
            Some(detail) => detail.unmergeable().map_or(Ok(()), Err),
        };
        if let Err(why) = ready {
            self.integrations.github.last_write = Some(Err(why.clone()));
            return Err(why);
        }
        let detail = detail.unwrap_or_default();
        self.github_write(id, Write::Merge(detail.merge_method, detail.request.head.clone()))
    }

    fn github_write(&mut self, id: &str, write: Write) -> Result<(), String> {
        let result = self.github_send_write(id, write);
        if let Err(why) = &result {
            self.integrations.github.last_write = Some(Err(why.clone()));
        }
        result
    }

    fn github_send_write(&mut self, id: &str, write: Write) -> Result<(), String> {
        let enabled = self.prefs.integrations.github;
        let github = &mut self.integrations.github;
        github.load_token();
        if let Some(why) = github.unable(enabled) {
            return Err(why);
        }
        if github.writing {
            return Err("Espere a escrita anterior terminar.".into());
        }
        let (owner, name, number) = parse_id(id).ok_or_else(|| unknown(id))?;
        let key = github.key(&owner, &name, number);
        let text = match &write {
            Write::Body(body) => body,
            Write::Comment(text) if text.trim().is_empty() => return Err("Escreva o comentário antes de enviar.".into()),
            Write::Comment(text) => text,
            Write::Approve if github.pulse.as_ref().is_some_and(|pulse| pulse.mine.iter().any(|request| request.id == key)) => {
                return Err("O GitHub não deixa aprovar o próprio pull request.".into());
            }
            Write::Approve | Write::Merge(..) => "",
        };
        if text.chars().count() > GITHUB_MOST {
            return Err(format!("O texto passa do limite do GitHub ({GITHUB_MOST} caracteres)."));
        }
        let cut = github.details.get(&key).and_then(|entry| entry.detail.as_ref()).is_some_and(|detail| detail.truncated);
        if matches!(write, Write::Body(_)) && cut {
            return Err("A descrição é longa demais para editar aqui: edite no navegador.".into());
        }
        let node_id = github.node_id(&key).ok_or_else(|| format!("O app ainda não sabe o id de {key} no GitHub: leia o pull request antes."))?;
        let (payload, field) = match &write {
            Write::Body(body) => (save_body_request(&node_id, body), "updatePullRequest"),
            Write::Comment(text) => (comment_request(&node_id, text), "addComment"),
            Write::Approve => (approve_request(&node_id), "addPullRequestReview"),
            Write::Merge(method, head) => (merge_request(&node_id, method, head.as_deref()), "mergePullRequest"),
        };
        if super::fake::pulse().is_some() {
            github.last_write = Some(Ok(format!("{} (cena de teste).", write.done().trim_end_matches('.'))));
            return Ok(());
        }
        (github.writing, github.last_write) = (true, None);
        // never the network in a test: the request is kept to be looked at, and the answer is handed to `receive`
        #[cfg(test)]
        github.sent.push(payload.clone());
        if !cfg!(test) {
            github.spawn(move |token| {
                let result = send_write(token, &payload, field);
                Message::Wrote(write, key, result)
            });
        }
        Ok(())
    }
}

/// What only the island's GitHub tab reads and asks (it is drawn where the app is only read).
impl DrawingApp {
    /// A write is out (Salvar and Enviar wait for it).
    pub(crate) fn github_writing(&self) -> bool {
        self.integrations.github.writing
    }

    /// How the last write went: Ok("Descrição salva." / "Comentário enviado.") or Err(why), in pt-BR.
    pub(crate) fn github_last_write(&self) -> Option<&Result<String, String>> {
        self.integrations.github.last_write.as_ref()
    }
}
