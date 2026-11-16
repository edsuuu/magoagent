use super::*;

/// Shaped as a token, and nobody's.
const TOKEN: &str = "ghp_0123456789abcdefghijABCDEFGHIJ012345";

/// A pulse as GitHub answers it (recorded shape, made-up names).
pub(crate) fn recorded_pulse() -> Value {
    serde_json::from_str(include_str!("../fixtures/github_pulse.json")).unwrap()
}

fn request(id: &str, ci: CiState, head: Option<&str>) -> PullRequest {
    PullRequest { id: id.into(), title: "T".into(), url: "https://github.com/a/b/pull/1".into(), draft: false, ci, review: "", head: head.map(String::from) }
}

fn pulse(mine: Vec<PullRequest>, to_review: Vec<PullRequest>) -> Pulse {
    Pulse { login: "ana".into(), mine, to_review, ..Default::default() }
}

#[test]
fn a_recorded_pulse_is_read() {
    let pulse = Pulse::parse(&recorded_pulse()).unwrap();
    assert_eq!(pulse.login, "ana-dev");
    assert_eq!(pulse.remaining, Some(4987));
    let ids: Vec<&str> = pulse.mine.iter().map(|request| request.id.as_str()).collect();
    assert_eq!(
        ids,
        ["ana-dev/loja#42", "ana-dev/loja#43", "ana-dev/site#7"],
        "a duplicate, one without a number and one with an address outside GitHub are left out"
    );
    assert_eq!((pulse.mine[0].ci, pulse.mine[0].review, pulse.mine[0].head.as_deref()), (CiState::Success, "aprovado", Some("aaa111")));
    assert_eq!((pulse.mine[1].ci, pulse.mine[1].draft, pulse.mine[1].review), (CiState::Unknown, true, ""), "no rollup, no decision");
    assert_eq!(pulse.mine[2].ci, CiState::Failure);
    assert_eq!(pulse.mine[2].title, "Corrige o carrinho quebrado", "one line, no control characters");
    assert_eq!(pulse.to_review.len(), 1);
    assert_eq!((pulse.to_review[0].id.as_str(), pulse.to_review[0].ci), ("time/api#310", CiState::Unknown));
    assert!(!pulse.has_pending());
    assert_eq!(pulse.summary(), "3 PRs abertos · 1 com checks falhando · 1 para revisar");
}

#[test]
fn an_answer_without_the_viewer_is_no_pulse() {
    for bad in [json!({}), json!({ "data": null }), json!({ "data": { "viewer": null } }), json!({ "data": [1, 2] }), json!("texto")] {
        assert_eq!(Pulse::parse(&bad), None, "{bad}");
    }
    let empty = Pulse::parse(&json!({ "data": { "viewer": { "login": "ana" } } })).unwrap();
    assert!(empty.mine.is_empty() && empty.to_review.is_empty());
    assert_eq!(empty.summary(), "nenhum PR aberto");
}

#[test]
fn the_states_of_a_check_are_githubs() {
    for (raw, state) in [
        ("PENDING", CiState::Pending),
        ("EXPECTED", CiState::Pending),
        ("success", CiState::Success),
        ("ERROR", CiState::Failure),
        ("FAILURE", CiState::Failure),
    ] {
        assert_eq!(CiState::from_github(Some(raw)), state, "{raw}");
    }
    assert_eq!(CiState::from_github(None), CiState::Unknown);
    assert_eq!(CiState::from_github(Some("novo")), CiState::Unknown);
}

#[test]
fn the_first_poll_is_silent_and_the_next_tell_what_changed() {
    let before = pulse(vec![request("a/b#1", CiState::Pending, Some("s1")), request("a/b#2", CiState::Success, Some("s2"))], vec![]);
    assert!(alerts(None, &before).is_empty(), "nothing to compare with");
    assert!(alerts(Some(&before), &before).is_empty(), "nothing changed");
    // the same commit: its checks ended
    let failed = pulse(vec![request("a/b#1", CiState::Failure, Some("s1")), request("a/b#2", CiState::Success, Some("s2"))], vec![]);
    assert_eq!(alerts(Some(&before), &failed), [Alert::CheckFailed("a/b#1".into())]);
    assert!(alerts(Some(&failed), &failed).is_empty(), "a failure is told once");
    let passed = pulse(vec![request("a/b#1", CiState::Success, Some("s1"))], vec![]);
    assert_eq!(alerts(Some(&before), &passed), [Alert::CheckPassed("a/b#1".into())]);
    // a success that was not pending before is no news
    assert!(alerts(Some(&pulse(vec![request("a/b#1", CiState::Unknown, Some("s1"))], vec![])), &passed).is_empty());
}

#[test]
fn a_new_commit_tells_its_checks_at_once_if_they_ended() {
    let before = pulse(vec![request("a/b#1", CiState::Success, Some("s1"))], vec![]);
    let pushed_running = pulse(vec![request("a/b#1", CiState::Pending, Some("s2"))], vec![]);
    assert!(alerts(Some(&before), &pushed_running).is_empty(), "still running: the next poll tells");
    let pushed_failed = pulse(vec![request("a/b#1", CiState::Failure, Some("s2"))], vec![]);
    assert_eq!(alerts(Some(&before), &pushed_failed), [Alert::CheckFailed("a/b#1".into())]);
    let opened = pulse(vec![request("a/b#1", CiState::Success, Some("s1")), request("a/b#9", CiState::Success, Some("s9"))], vec![]);
    assert_eq!(alerts(Some(&before), &opened), [Alert::CheckPassed("a/b#9".into())], "a new pull request whose checks already passed");
}

#[test]
fn a_review_asked_for_is_told_once_and_a_failure_comes_first() {
    let before = pulse(vec![request("a/b#1", CiState::Pending, Some("s1"))], vec![request("c/d#5", CiState::Unknown, None)]);
    let now =
        pulse(vec![request("a/b#1", CiState::Failure, Some("s1"))], vec![request("c/d#5", CiState::Unknown, None), request("c/d#6", CiState::Unknown, None)]);
    let found = alerts(Some(&before), &now);
    assert_eq!(found, [Alert::CheckFailed("a/b#1".into()), Alert::ReviewRequested("c/d#6".into())]);
    assert_eq!(foremost(&found), Some(&Alert::CheckFailed("a/b#1".into())));
    assert_eq!(foremost(&[Alert::CheckPassed("x".into()), Alert::ReviewRequested("y".into())]), Some(&Alert::ReviewRequested("y".into())));
    assert_eq!(foremost(&[]), None);
}

#[test]
fn a_recorded_calendar_is_read() {
    let root: Value = serde_json::from_str(include_str!("../fixtures/github_activity.json")).unwrap();
    let activity = Activity::parse(&root).unwrap();
    assert_eq!((activity.total, activity.today), (412, 3));
    assert_eq!(activity.weeks, [vec![0, 1, 2, 3, 4, 0, 0], vec![0, 4, 0]], "an unknown level reads as none; the last week is the current one");
    assert_eq!(Activity::parse(&json!({ "data": { "viewer": {} } })), None);
}

#[test]
fn a_pulse_sets_the_next_poll_and_keeps_the_alert_for_the_pill() {
    let mut github = GitHub { token: Some("t".repeat(40)), ..Default::default() };
    let running = pulse(vec![request("a/b#1", CiState::Pending, Some("s1"))], vec![]);
    assert_eq!(github.take_pulse(running, 1_000), None, "the first is silent");
    assert_eq!(github.next_pulse, 1_000 + PENDING_EVERY, "a check runs: a minute");
    assert!(github.connected());
    let failed = pulse(vec![request("a/b#1", CiState::Failure, Some("s1"))], vec![]);
    assert_eq!(github.take_pulse(failed, 2_000), Some(Alert::CheckFailed("a/b#1".into())));
    assert_eq!(github.next_pulse, 2_000 + IDLE_EVERY);
    let notice = github.notice.as_ref().unwrap();
    assert!(notice.failure && notice.text == "Os checks de a/b#1 falharam" && notice.url == "https://github.com/a/b/pull/1/checks");
    let spent = Pulse { remaining: Some(12), ..pulse(vec![request("a/b#1", CiState::Pending, Some("s2"))], vec![]) };
    github.take_pulse(spent, 3_000);
    assert_eq!(github.next_pulse, 3_000 + SPARING_EVERY, "the hour's budget nearly spent: sparing, even with a check running");
    github.forget();
    assert!(github.pulse.is_none() && github.notice.is_none() && github.next_pulse == 0);
}

#[test]
fn the_token_must_look_like_one_and_every_request_carries_it_only_in_the_config() {
    assert!(token_looks_right("ghp_0123456789abcdefghijABCDEFGHIJ012345"));
    assert!(token_looks_right(&format!("github_pat_{}", "a".repeat(82))));
    for bad in ["", "curto", "ghp_com espaco 0123456789012345678", "ghp_quebra\n0123456789012345678901"] {
        assert!(!token_looks_right(bad), "{bad:?}");
    }
    let token = "ghp_0123456789abcdefghijABCDEFGHIJ012345";
    let request = request_for_test(token);
    assert!(http::arguments(&request).iter().all(|argument| !argument.contains(token)), "the token is not on the command line");
    assert!(http::config(&request).contains(&format!("header = \"Authorization: Bearer {token}\"")));
    assert!(!request.url.contains(token), "nor in the address");
}

fn request_for_test(token: &str) -> Request<'static> {
    super::request(token, "POST", GRAPHQL, Some(json!({ "query": PULSE_QUERY }).to_string()))
}

#[test]
fn an_http_status_says_why_it_failed() {
    let answer = |status, body: &str| Ok(http::Answer { status, body: body.to_string() });
    assert_eq!(read(answer(200, "{\"login\":\"ana\"}")).unwrap()["login"], "ana");
    let refused = read(answer(401, "{\"message\":\"Bad credentials\"}")).unwrap_err();
    assert!(refused.unauthorized && refused.message.contains("inválido"));
    assert!(!read(answer(403, "")).unwrap_err().unauthorized);
    assert!(read(answer(200, "<html>")).is_err());
    assert_eq!(read(Err("sem conexão".into())).unwrap_err().message, "Sem conexão com o GitHub.");
}

#[test]
fn the_report_says_who_and_each_pull_request_with_its_checks() {
    assert!(GitHub::with_token("").report().contains("sem token"));
    let mut github = GitHub::with_token(TOKEN);
    assert!(github.report().contains("ainda sem resposta"));
    github.pulse = Pulse::parse(&recorded_pulse());
    github.activity = Some(Activity { total: 412, weeks: vec![], today: 3 });
    let report = github.report();
    assert!(report.starts_with("GitHub: conectado como @ana-dev · 3 PRs abertos"), "{report}");
    assert!(report.contains("- ana-dev/loja#42 Adiciona o checkout (checks passaram · aprovado) https://github.com/ana-dev/loja/pull/42"), "{report}");
    assert!(report.contains("- ana-dev/loja#43 Rascunho do frete (sem checks · rascunho)"), "{report}");
    assert!(report.contains("- revisar time/api#310"), "{report}");
    assert!(report.ends_with("412 contribuições no último ano, 3 hoje."), "{report}");
    assert!(!report.contains("ghp_"), "never the token");
    github.error = Some("Token inválido ou expirado.".into());
    assert_eq!(github.report(), "GitHub: Token inválido ou expirado.");
}

/// Salvar and Remover, on the tests' keychain: the token is kept there and nowhere in the preferences.
#[test]
fn the_token_is_saved_to_the_keychain_and_removed_from_it() {
    mock_keyring();
    let mut app = DrawingApp::default();
    app.prefs.integrations.github = true;
    assert!(app.github_save_token("ghp curto").is_err(), "not a token: nothing is kept");
    assert!(app.github_save_token(&format!("  {TOKEN}\n")).is_ok(), "pasted with a line break after it");
    assert_eq!(take_secret(TOKEN_ID).as_deref(), Some(TOKEN));
    assert!(app.integrations.github.has_token() && !app.integrations.github.connected(), "kept, and no answer yet");
    assert!(!serde_json::to_string(&app.prefs).unwrap().contains(TOKEN), "the preferences never hold it");
    // a new app reads it back from the keychain
    let mut other = DrawingApp::default();
    other.prefs.integrations.github = true;
    other.github_tick(&Context::default());
    assert!(other.integrations.github.has_token());
    app.github_remove_token().unwrap();
    assert_eq!(take_secret(TOKEN_ID), None);
    assert!(!app.integrations.github.has_token());
}

#[test]
fn off_nothing_is_kept_and_an_alert_goes_after_its_card_was_seen() {
    let mut app = DrawingApp::default();
    app.prefs.integrations.github = true;
    app.integrations.github = GitHub::with_token(TOKEN);
    let ctx = Context::default();
    app.integrations.github.take_pulse(pulse(vec![request("a/b#1", CiState::Pending, Some("s1"))], vec![]), now_ms());
    app.integrations.github.take_pulse(pulse(vec![request("a/b#1", CiState::Failure, Some("s1"))], vec![]), now_ms());
    app.github_tick(&ctx);
    assert!(app.integrations.github.notice.is_some(), "the alert stays until it is seen");
    app.github_seen();
    app.integrations.github.notice.as_mut().unwrap().seen = Some(now_ms() - SEEN_FOR - 1);
    app.github_tick(&ctx);
    assert!(app.integrations.github.notice.is_none(), "seen: it goes a moment later");
    assert!(app.integrations.github.pulse.is_some());
    app.prefs.integrations.github = false;
    app.github_tick(&ctx);
    assert!(app.integrations.github.pulse.is_none(), "off: what was fetched is forgotten");
    assert_eq!(app.github_report(), "GitHub: desligado.");
}

fn branch(ci: CiState, head: &str) -> BranchCi {
    BranchCi { repo: "ana/loja".into(), url: "https://github.com/ana/loja".into(), branch: "main".into(), ci, head: Some(head.into()) }
}

fn with_main(main: Vec<BranchCi>) -> Pulse {
    Pulse { login: "ana".into(), main, ..Default::default() }
}

#[test]
fn the_default_branches_are_read_with_their_last_commit_s_checks() {
    let answer = serde_json::json!({ "data": { "viewer": { "login": "ana", "repositories": { "nodes": [
        { "nameWithOwner": "ana/loja", "url": "https://github.com/ana/loja", "isArchived": false,
          "defaultBranchRef": { "name": "main", "target": { "oid": "c1", "statusCheckRollup": { "state": "FAILURE" } } } },
        { "nameWithOwner": "ana/velho", "url": "https://github.com/ana/velho", "isArchived": true,
          "defaultBranchRef": { "name": "main", "target": { "oid": "c2", "statusCheckRollup": { "state": "FAILURE" } } } },
        { "nameWithOwner": "ana/vazio", "url": "https://github.com/ana/vazio", "isArchived": false, "defaultBranchRef": null },
        { "nameWithOwner": "ana/site", "url": "https://github.com/ana/site", "isArchived": false,
          "defaultBranchRef": { "name": "trunk", "target": { "oid": "c3", "statusCheckRollup": null } } }
    ] } } } });
    let pulse = Pulse::parse(&answer).unwrap();
    let read: Vec<(&str, &str, CiState, Option<&str>)> =
        pulse.main.iter().map(|branch| (branch.repo.as_str(), branch.branch.as_str(), branch.ci, branch.head.as_deref())).collect();
    assert_eq!(read, [("ana/loja", "main", CiState::Failure, Some("c1")), ("ana/site", "trunk", CiState::Unknown, Some("c3"))], "archived and empty ones out");
    assert!(PULSE_QUERY.contains("defaultBranchRef") && PULSE_QUERY.contains("ownerAffiliations: [OWNER]"));
    assert!(with_main(vec![branch(CiState::Pending, "c1")]).has_pending(), "a default branch running: the next poll comes sooner");
}

#[test]
fn a_default_branch_only_ever_tells_that_it_failed() {
    let old = with_main(vec![branch(CiState::Pending, "c1")]);
    assert_eq!(alerts(Some(&old), &with_main(vec![branch(CiState::Failure, "c1")])), [Alert::MainFailed("ana/loja".into())]);
    assert_eq!(alerts(Some(&old), &with_main(vec![branch(CiState::Success, "c1")])), [], "never that it passed");
    let failed = with_main(vec![branch(CiState::Failure, "c1")]);
    assert_eq!(alerts(Some(&failed), &failed.clone()), [], "still failing: told once");
    assert_eq!(alerts(Some(&failed), &with_main(vec![branch(CiState::Failure, "c2")])), [Alert::MainFailed("ana/loja".into())], "a new commit that failed too");
    assert_eq!(alerts(None, &failed), [], "the first poll is silent");
    let both = [Alert::CheckPassed("a/b#1".into()), Alert::ReviewRequested("a/b#2".into()), Alert::MainFailed("ana/loja".into())];
    assert_eq!(foremost(&both), Some(&Alert::MainFailed("ana/loja".into())), "a failure before the rest");
}

#[test]
fn a_failure_opens_where_it_failed_and_the_alert_is_kept_to_sound() {
    let mut github = GitHub::with_token(TOKEN);
    github.take_pulse(pulse(vec![request("a/b#1", CiState::Pending, Some("s1"))], vec![]), 1_000);
    assert_eq!(github.ring, None, "the first poll is silent");
    github.take_pulse(pulse(vec![request("a/b#1", CiState::Failure, Some("s1"))], vec![]), 2_000);
    assert_eq!(github.notice.as_ref().map(|notice| notice.url.as_str()), Some("https://github.com/a/b/pull/1/checks"), "the PR's checks");
    assert_eq!(github.ring, Some((Alert::CheckFailed("a/b#1".into()), 2_000)));
    assert_eq!(request("a/b#1", CiState::Success, None).page(), "https://github.com/a/b/pull/1", "a PR that did not fail: itself");
    let mut github = GitHub::with_token(TOKEN);
    github.take_pulse(with_main(vec![branch(CiState::Success, "c1")]), 1_000);
    github.take_pulse(with_main(vec![branch(CiState::Failure, "c2")]), 2_000);
    let notice = github.notice.as_ref().unwrap();
    assert_eq!((notice.text.as_str(), notice.url.as_str()), ("O CI da main de ana/loja falhou", "https://github.com/ana/loja/actions"));
}
