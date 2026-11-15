use crate::prelude::*;
use std::sync::{Arc, Mutex};

const API_URL: &str = "https://api.github.com/graphql";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CiState {
    Pending,
    Success,
    Failure,
    Unknown,
}

impl CiState {
    pub(crate) fn from_github(raw: Option<&str>) -> Self {
        match raw.map(str::to_ascii_uppercase).as_deref() {
            Some("PENDING") | Some("EXPECTED") => CiState::Pending,
            Some("SUCCESS") => CiState::Success,
            Some("ERROR") | Some("FAILURE") => CiState::Failure,
            _ => CiState::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct GitHubPr {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) repo: String,
    pub(crate) number: i64,
    pub(crate) is_draft: bool,
    pub(crate) ci: CiState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) head_sha: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) enum GitHubState {
    #[default]
    Idle,
    Loading,
    Loaded(Vec<GitHubPr>),
    Failed(String),
}

#[derive(Default)]
pub(crate) struct GitHub {
    pub(crate) state: GitHubState,
    pub(crate) last_fetch: Option<u64>,
    pub(crate) etag: Option<String>,
}

const PULSE_QUERY: &str = r#"
query {
  viewer {
    login
    pullRequests(states: OPEN, first: 20, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        number title url isDraft
        repository { nameWithOwner url }
        commits(last: 1) {
          nodes { commit { oid statusCheckRollup { state } } }
        }
      }
    }
  }
  reviewRequested: search(query: "is:pr is:open review-requested:@me archived:false", type: ISSUE, first: 20) {
    nodes {
      ... on PullRequest {
        number title url isDraft
        repository { nameWithOwner url }
      }
    }
  }
}
"#;

fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn nodes<'a>(parent: Option<&'a Value>) -> &'a [Value] {
    parent.and_then(|p| p.get("nodes")).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn pr_basics(node: &Value) -> Option<(String, String, String, String, i64, bool)> {
    let number = node.get("number")?.as_i64()?;
    let title = str_at(node, "title")?.to_string();
    let url = str_at(node, "url")?.to_string();
    let repo = node.get("repository").and_then(|r| str_at(r, "nameWithOwner"))?.to_string();
    let is_draft = node.get("isDraft").and_then(Value::as_bool).unwrap_or(false);
    Some((format!("{repo}#{number}"), title, url, repo, number, is_draft))
}

pub(crate) fn parse_prs(root: &Value) -> Result<Vec<GitHubPr>, String> {
    let data = root.get("data").and_then(Value::as_object).ok_or("resposta sem data")?;
    let viewer = data.get("viewer").and_then(Value::as_object).ok_or("resposta sem viewer")?;

    let mut prs = Vec::new();

    for node in nodes(viewer.get("pullRequests")) {
        let Some((id, title, url, repo, number, is_draft)) = pr_basics(node) else { continue };
        if prs.iter().any(|p: &GitHubPr| p.id == id) {
            continue;
        }
        let commit = node.get("commits").and_then(|c| c.get("nodes")).and_then(Value::as_array).and_then(|a| a.last()).and_then(|n| n.get("commit"));
        let ci_raw = commit.and_then(|c| c.get("statusCheckRollup")).and_then(|r| str_at(r, "state"));
        let head_sha = commit.and_then(|c| str_at(c, "oid")).map(str::to_string);
        prs.push(GitHubPr { id, title, url, repo, number, is_draft, ci: CiState::from_github(ci_raw), head_sha });
    }

    for node in nodes(data.get("reviewRequested")) {
        let Some((id, title, url, repo, number, is_draft)) = pr_basics(node) else { continue };
        if prs.iter().any(|p| p.id == id) {
            continue;
        }
        prs.push(GitHubPr { id, title, url, repo, number, is_draft, ci: CiState::Unknown, head_sha: None });
    }

    Ok(prs)
}

pub(crate) fn fetch_prs(token: &str, _etag: Option<&str>) -> Result<(Vec<GitHubPr>, Option<String>), String> {
    let body = json!({ "query": PULSE_QUERY }).to_string();
    let auth_header = format!("Authorization: Bearer {token}");
    let headers = vec!["Content-Type: application/json", "Accept: application/vnd.github+json", "User-Agent: MagoAgent", auth_header.as_str()];

    let mut args = vec!["-sS", "--fail-with-body", "--max-time", "10", "-X", "POST"];
    for h in &headers {
        args.extend_from_slice(&["-H", h]);
    }
    args.extend_from_slice(&["-d", &body, API_URL]);

    let child = crate::editor::command("curl")
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl: {e}"))?;

    let output = child.wait_with_output().map_err(|e| format!("curl: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.contains("401") || stderr.contains("Unauthorized") {
            "token do GitHub inválido".to_string()
        } else if stderr.contains("403") || stderr.contains("Forbidden") {
            "acesso negado pelo GitHub".to_string()
        } else {
            format!("erro ao buscar PRs: {stderr}")
        });
    }

    let response_text = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&response_text).map_err(|_| "resposta inválida do GitHub")?;

    if let Some(errors) = json.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            return Err("erro na consulta GraphQL do GitHub".to_string());
        }
    }

    let prs = parse_prs(&json)?;
    Ok((prs, None))
}

pub(crate) fn poll_github(github: Arc<Mutex<GitHub>>, token: String, ctx: Context) {
    {
        let mut state = github.lock().unwrap();
        if matches!(state.state, GitHubState::Loading) {
            return;
        }
        state.state = GitHubState::Loading;
    }

    std::thread::spawn(move || {
        let etag = github.lock().unwrap().etag.clone();
        let result = fetch_prs(&token, etag.as_deref());

        let mut state = github.lock().unwrap();
        match result {
            Ok((prs, new_etag)) => {
                state.state = GitHubState::Loaded(prs);
                state.last_fetch = Some(now_ms() as u64);
                if let Some(etag) = new_etag {
                    state.etag = Some(etag);
                }
            }
            Err(why) => {
                state.state = GitHubState::Failed(why);
            }
        }
        ctx.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_response() -> Value {
        json!({
          "data": {
            "viewer": {
              "login": "testuser",
              "pullRequests": { "nodes": [
                {
                  "number": 42, "title": "Add feature",
                  "url": "https://github.com/testuser/myrepo/pull/42",
                  "isDraft": false,
                  "repository": {"nameWithOwner": "testuser/myrepo"},
                  "commits": {"nodes": [{"commit": {"oid": "abc123", "statusCheckRollup": {"state": "SUCCESS"}}}]}
                }
              ]},
            },
            "reviewRequested": {
              "nodes": [{
                "number": 7, "title": "Review this",
                "url": "https://github.com/other/repo/pull/7",
                "isDraft": false,
                "repository": {"nameWithOwner": "other/repo"}
              }]
            }
          }
        })
    }

    #[test]
    fn parses_github_response() {
        let prs = parse_prs(&valid_response()).unwrap();
        assert_eq!(prs.len(), 2);
        assert_eq!(prs[0].id, "testuser/myrepo#42");
        assert_eq!(prs[0].ci, CiState::Success);
        assert_eq!(prs[0].head_sha.as_deref(), Some("abc123"));
        assert_eq!(prs[1].id, "other/repo#7");
        assert_eq!(prs[1].ci, CiState::Unknown);
    }

    #[test]
    fn maps_ci_states() {
        assert_eq!(CiState::from_github(None), CiState::Unknown);
        assert_eq!(CiState::from_github(Some("PENDING")), CiState::Pending);
        assert_eq!(CiState::from_github(Some("SUCCESS")), CiState::Success);
        assert_eq!(CiState::from_github(Some("FAILURE")), CiState::Failure);
    }

    #[test]
    fn rejects_bad_data() {
        assert!(parse_prs(&json!({})).is_err());
        assert!(parse_prs(&json!({"data": null})).is_err());
    }
}
