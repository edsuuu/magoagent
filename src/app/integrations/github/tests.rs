use super::token::token_looks_right;
use super::*;

/// Shaped as a token, and nobody's.
const TOKEN: &str = "ghp_0123456789abcdefghijABCDEFGHIJ012345";

/// A pulse as GitHub answers it (recorded shape, made-up names).
pub(crate) fn recorded_pulse() -> Value {
    serde_json::from_str(include_str!("../fixtures/github_pulse.json")).unwrap()
}

fn request(id: &str, ci: CiState, head: Option<&str>) -> PullRequest {
    PullRequest { id: id.into(), title: "T".into(), url: "https://github.com/a/b/pull/1".into(), ci, head: head.map(String::from), ..Default::default() }
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
        ["ana-dev/loja#42", "ana-dev/loja#43", "ana-dev/site#7", "ana-dev/loja#40", "ana-dev/site#39"],
        "a duplicate, one without a number and one with an address outside GitHub are left out; the merged and the closed come too"
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

    assert_eq!((activity.total, activity.today, activity.repos, activity.stars), (1412, 3, 12, 1340), "a star count below zero or missing adds none");
    let levels: Vec<Vec<u8>> = activity.weeks.iter().map(|week| week.iter().map(|day| day.level).collect()).collect();

    assert_eq!(levels, [vec![0, 1, 2, 3, 4, 0, 0], vec![0, 4, 0]], "an unknown level reads as none; the last week is the current one");
    let last = &activity.weeks[1];

    assert_eq!((last[1].date.as_str(), last[1].count, last[1].weekday), ("2026-11-16", 22, 1));
    assert_eq!((last[2].date.as_str(), last[2].weekday), ("", 2), "no date nor weekday: its place in the week");
    assert_eq!(activity.summary(), "1.412 contribuições no último ano · 12 repositórios · ★ 1,3 mil");
    assert_eq!(day_words(&last[1]), "16 de nov. · 22 contribuições");
    assert_eq!(day_words(&activity.weeks[0][0]), "1 de nov. · nenhuma contribuição");
    assert_eq!(day_words(&last[2]), "3 contribuições");
    // the last seven days, graded against the busiest of them
    let week: Vec<(i64, u8)> = activity.last_days(7).iter().map(|day| (day.count, day.level)).collect();

    assert_eq!(week, [(7, 2), (15, 3), (0, 0), (0, 0), (0, 0), (22, 4), (3, 1)]);
    assert_eq!(Activity { total: 1, ..Default::default() }.summary(), "1 contribuição no último ano");
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
    github.activity = Some(Activity { total: 412, today: 3, ..Default::default() });
    let report = github.report();

    assert!(report.starts_with("GitHub: conectado como @ana-dev · 3 PRs abertos"), "{report}");
    assert!(report.contains("- ana-dev/loja#42 Adiciona o checkout (checks passaram · aprovado) https://github.com/ana-dev/loja/pull/42"), "{report}");
    assert!(report.contains("- ana-dev/loja#43 Rascunho do frete (sem checks · rascunho)"), "{report}");
    assert!(report.contains("- revisar time/api#310"), "{report}");
    assert!(report.ends_with("412 contribuições no último ano; 3 hoje."), "{report}");
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

/// A pull request's detail as GitHub answers it (recorded shape, made-up names).
fn recorded_detail() -> Value {
    serde_json::from_str(include_str!("../fixtures/github_pr_detail.json")).unwrap()
}

/// Noon of the day the fixtures were recorded, ms.
fn noon() -> i64 {
    crate::transcript::iso_ms("2026-11-16T12:00:00Z").unwrap()
}

/// GitHub on, with a token and the recorded pulse: what the tab reads and writes.
fn app_with_pulse() -> DrawingApp {
    let mut app = DrawingApp::default();

    app.prefs.integrations.github = true;
    app.integrations.github = GitHub::with_token(TOKEN);
    app.integrations.github.pulse = Pulse::parse(&recorded_pulse());
    app
}

#[test]
fn the_poll_asks_for_the_last_20_of_every_state_and_no_description() {
    assert!(PULSE_QUERY.contains("pullRequests(states: [OPEN, MERGED, CLOSED], first: 20, orderBy: {field: UPDATED_AT, direction: DESC})"));

    for field in ["id number", "state mergeable", "updatedAt headRefName baseRefName", "contexts(first: 25)", "detailsUrl", "targetUrl"] {
        assert!(PULSE_QUERY.contains(field), "{field}");
    }

    assert!(!PULSE_QUERY.contains("body"), "the description is read when a pull request is picked, never in the polls");
}

#[test]
fn the_pulse_reads_each_pull_requests_state_branches_and_failed_checks() {
    let pulse = Pulse::parse(&recorded_pulse()).unwrap();
    let checkout = &pulse.mine[0];

    assert_eq!((checkout.state, checkout.mergeable, checkout.node_id.as_str()), (PrState::Open, MergeableState::Mergeable, "PR_kwDOAAAB42"));
    assert_eq!((checkout.head_ref.as_str(), checkout.base_ref.as_str(), checkout.updated_at), ("feat/checkout", "main", noon() - 4 * 60_000));
    assert!(checkout.failing.is_empty(), "its checks passed");
    let cart = &pulse.mine[2];

    assert_eq!(cart.mergeable, MergeableState::Conflicting);

    assert_eq!(
        cart.failing,
        [
            Check { name: "build".into(), url: Some("https://github.com/ana-dev/site/actions/runs/9/job/2".into()) },
            Check { name: "ci/vercel".into(), url: Some("https://vercel.com/ana-dev/site/abc".into()) },
            Check { name: "deploy".into(), url: None },
        ],
        "a failed run, a status in error, a cancelled run whose page is no https address; each name once, the ones that passed out"
    );

    assert_eq!((pulse.mine[3].state, pulse.mine[4].state), (PrState::Merged, PrState::Closed));
    assert!(pulse.to_review[0].review_requested_of_me && !pulse.mine.iter().any(|request| request.review_requested_of_me));
    assert_eq!((pulse.to_review[0].head_ref.as_str(), pulse.to_review[0].node_id.as_str()), ("troca-http", "PR_kwDOAAAB310"));

    let old = Pulse::parse(&json!({ "data": { "viewer": { "login": "ana", "pullRequests": { "nodes": [
        { "number": 1, "title": "Velho", "url": "https://github.com/a/b/pull/1", "repository": { "nameWithOwner": "a/b" } }
    ] } } } }))
    .unwrap();

    let before = &old.mine[0];

    assert_eq!(
        (before.state, before.mergeable, before.updated_at, before.node_id.as_str()),
        (PrState::Open, MergeableState::Unknown, 0, ""),
        "fields left out"
    );
}

#[test]
fn a_pull_request_merged_or_closed_tells_nothing() {
    let pulse = Pulse::parse(&recorded_pulse()).unwrap();
    let mut open = pulse.clone();

    open.mine.retain(PullRequest::is_open);
    assert_eq!(alerts(Some(&open), &pulse), [], "a merged one with its checks failed and a closed one come: no news");
    let mut running = pulse.clone();

    (running.mine[3].state, running.mine[3].ci) = (PrState::Open, CiState::Pending);
    assert_eq!(alerts(Some(&running), &pulse), [], "it was open and running; merged now, its failure is no news");
    let mut still_open = pulse.clone();

    still_open.mine[3].state = PrState::Open;
    assert_eq!(alerts(Some(&running), &still_open), [Alert::CheckFailed("ana-dev/loja#40".into())], "still open: told");
}

#[test]
fn a_pull_request_needs_the_person_when_it_is_open_and_waits_for_them() {
    let calm = PullRequest { id: "a/b#1".into(), ci: CiState::Success, mergeable: MergeableState::Mergeable, ..Default::default() };

    assert!(!calm.needs_you());

    for (why, request) in [
        ("checks failed", PullRequest { ci: CiState::Failure, ..calm.clone() }),
        ("a conflict with its base", PullRequest { mergeable: MergeableState::Conflicting, ..calm.clone() }),
        ("changes asked for", PullRequest { review: CHANGES_REQUESTED, ..calm.clone() }),
        ("its review asked of the person", PullRequest { review_requested_of_me: true, ..calm.clone() }),
    ] {
        assert!(request.needs_you(), "{why}");
        assert!(!PullRequest { state: PrState::Merged, ..request.clone() }.needs_you(), "{why}, but merged");
        assert!(!PullRequest { state: PrState::Closed, ..request }.needs_you(), "{why}, but closed");
    }

    let waiting = PullRequest { ci: CiState::Pending, mergeable: MergeableState::Unknown, review: "aguarda revisão", draft: true, ..calm };

    assert!(!waiting.needs_you(), "running, not yet worked out, waiting for others: nothing for the person");
}

#[test]
fn the_tabs_filters_split_the_list_the_last_changed_first() {
    let pulse = Pulse::parse(&recorded_pulse()).unwrap();
    let ids = |filter| pulse.listed(filter).iter().map(|request| request.id.clone()).collect::<Vec<_>>();
    let all = ["ana-dev/loja#42", "time/api#310", "ana-dev/loja#43", "ana-dev/site#7", "ana-dev/loja#40", "ana-dev/site#39"];

    assert_eq!(ids(PrFilter::All), all, "the review asked for among the person's own");
    assert_eq!(ids(PrFilter::Open), all[..4]);
    assert_eq!(ids(PrFilter::NeedsYou), ["time/api#310", "ana-dev/site#7"], "a review asked of the person; checks failed, a conflict, changes asked for");
    assert_eq!(ids(PrFilter::Closed), ["ana-dev/loja#40", "ana-dev/site#39"], "merged and closed");
    assert_eq!(PrFilter::EVERY.map(PrFilter::label), ["Todas", "Abertas", "Precisa de você", "Fechadas"]);
    assert_eq!(pulse.find("ANA-DEV/Loja#42").map(|request| request.id.as_str()), Some("ana-dev/loja#42"), "GitHub's names ignore case");
}

#[test]
fn ver_checks_opens_the_failed_run_when_it_is_on_github() {
    let pulse = Pulse::parse(&recorded_pulse()).unwrap();

    assert_eq!(pulse.mine[2].checks_page(), "https://github.com/ana-dev/site/actions/runs/9/job/2");
    let elsewhere = PullRequest { failing: vec![Check { name: "ci/vercel".into(), url: Some("https://vercel.com/a".into()) }], ..pulse.mine[2].clone() };

    assert_eq!(elsewhere.checks_page(), "https://github.com/ana-dev/site/pull/7/checks", "the app opens only GitHub's own pages");
    assert_eq!(pulse.mine[0].checks_page(), "https://github.com/ana-dev/loja/pull/42/checks");
}

#[test]
fn how_long_ago_in_portuguese() {
    let now = noon();

    assert_eq!(ago(now - 20_000, now), "agora");
    assert_eq!(ago(now - 4 * 60_000, now), "há 4 min");
    assert_eq!(ago(now - 27 * 3_600_000, now), "ontem");
    assert_eq!(ago(now - 3 * 86_400_000, now), "há 3 dias");
    assert_eq!(ago(0, now), "", "GitHub did not say");
    let pulse = Pulse::parse(&recorded_pulse()).unwrap();

    assert_eq!(pulse.mine[0].subtitle(now), "ana-dev/loja #42 · feat/checkout → main · há 4 min");
    assert_eq!(pulse.mine[2].subtitle(now), "ana-dev/site #7 · fix/carrinho → main · ontem");
    assert_eq!(pulse.mine[3].subtitle(now), "ana-dev/loja #40 · feat/logotipo → main · há 3 dias");
    assert_eq!(request("a/b#1", CiState::Unknown, None).subtitle(now), "a/b #1", "no branches, no time");
}

#[test]
fn a_recorded_detail_is_read_with_its_reviews_and_what_blocks_it() {
    let detail = PrDetail::parse(&recorded_detail(), 5).unwrap();

    assert_eq!(detail.body, "## O que muda\n\n- conserta o total\n- testa o frete\n\n\tFecha #3.", "GitHub's line breaks as \\n, a control character out");
    assert!(!detail.truncated);
    assert_eq!((detail.approved, detail.changes_requested), (2, 2), "the latest review of each reviewer: duda changed her mind; a dismissed one is neither");
    let request = &detail.request;

    assert_eq!(
        (request.id.as_str(), request.node_id.as_str(), request.mergeable, detail.fetched_at),
        ("ana-dev/site#7", "PR_kwDOAAAB07", MergeableState::Conflicting, 5)
    );

    let blockers = detail.blockers();
    let read: Vec<(&str, Option<&str>)> = blockers.iter().map(|blocker| (blocker.text.as_str(), blocker.url.as_deref())).collect();

    assert_eq!(
        read,
        [
            ("Conflito com main", Some("https://github.com/ana-dev/site/pull/7")),
            ("Check falhou: build", Some("https://github.com/ana-dev/site/actions/runs/9/job/2")),
            ("Check falhou: e2e", Some("https://github.com/ana-dev/site/actions/runs/9/job/3")),
            ("Check falhou: ci/vercel", Some("https://vercel.com/ana-dev/site/abc")),
            ("2 revisores pediram mudanças", Some("https://github.com/ana-dev/site/pull/7/files")),
        ]
    );

    assert_eq!(detail.reviews_words(), "2 aprovações · 2 pedidos de mudanças");
    let merged = PrDetail { request: PullRequest { state: PrState::Merged, ..detail.request.clone() }, ..detail.clone() };

    assert!(merged.blockers().is_empty(), "merged: nothing blocks it");
    let report = detail.report(noon());

    assert!(report.starts_with("ana-dev/site#7 · aberto · Corrige o carrinho quebrado · fix/carrinho → main · ontem · checks falharam"), "{report}");
    assert!(report.contains("\nBloqueios: Conflito com main (https://github.com/ana-dev/site/pull/7); Check falhou: build"), "{report}");
    assert!(report.ends_with("Descrição (Markdown):\n## O que muda\n\n- conserta o total\n- testa o frete\n\n\tFecha #3."), "{report}");
    assert_eq!(PrDetail::parse(&json!({ "data": { "repository": { "pullRequest": null } } }), 0), None);
    assert_eq!(PrDetail::parse(&json!({ "data": { "repository": null } }), 0), None);
}

#[test]
fn a_long_description_is_cut_and_never_saved_back() {
    let (body, cut) = super::detail::body_text(&"á".repeat(20_001));

    assert_eq!((body.chars().count(), cut), (20_000, true));
    assert_eq!(super::detail::body_text("curta\r\nduas"), ("curta\nduas".to_string(), false));
    let mut app = app_with_pulse();
    let mut long = PrDetail::parse(&recorded_detail(), 5).unwrap();

    long.truncated = true;
    app.integrations.github.receive(Message::Detail("ana-dev/site#7".into(), Ok(Box::new(long))), 10);
    // the island's Salvar: what goes wrong is where the tab reads how the last write went
    assert!(app.github_save_body("ana-dev/site#7", "x").is_err());
    let said = app.github_last_write().cloned().unwrap().unwrap_err();

    assert!(said.contains("longa demais"), "saving it would cut the rest: {said}");
    assert!(app.github_comment("ana-dev/site#7", &"x".repeat(65_537)).unwrap_err().contains("limite do GitHub"));
    assert!(!app.github_writing() && app.integrations.github.sent.is_empty(), "nothing was sent");
}

#[test]
fn a_pull_request_is_named_by_its_id_or_its_address() {
    for (text, owner, name, number) in [
        ("ana-dev/site#7", "ana-dev", "site", 7),
        (" Ana-Dev/site.io#12 ", "Ana-Dev", "site.io", 12),
        ("https://github.com/ana-dev/site/pull/7", "ana-dev", "site", 7),
        ("https://github.com/ana-dev/site/pull/7/files#diff-1", "ana-dev", "site", 7),
        ("https://github.com/ana-dev/site/pull/7#issuecomment-1", "ana-dev", "site", 7),
    ] {
        assert_eq!(parse_id(text), Some((owner.to_string(), name.to_string(), number)), "{text}");
    }

    for bad in ["", "site#7", "ana-dev/site", "ana-dev/site#0", "ana-dev/site#x", "ana dev/site#7", "../site#1", "a/b/c#1"] {
        assert_eq!(parse_id(bad), None, "{bad}");
    }

    for bad in ["https://github.com/ana-dev/site/issues/7", "https://evil.example/ana-dev/site/pull/7"] {
        assert_eq!(parse_id(bad), None, "{bad}");
    }
}

#[test]
fn the_requests_carry_the_pull_request_and_the_text_as_variables() {
    let detail = detail_request("ana-dev", "site", 7);

    assert_eq!(detail["variables"], json!({ "owner": "ana-dev", "name": "site", "number": 7 }));
    let query = detail["query"].as_str().unwrap();

    assert!(query.contains("pullRequest(number: $number)") && query.contains(" body ") && query.contains("latestOpinionatedReviews"), "{query}");
    let text = "Descrição \"nova\"\nlinha 2\nurl = \"https://evil.example\"";
    let save = save_body_request("PR_kwDOAAAB07", text);

    assert!(save["query"].as_str().unwrap().contains("updatePullRequest(input: {pullRequestId: $id, body: $body})"));
    assert_eq!(save["variables"], json!({ "id": "PR_kwDOAAAB07", "body": text }), "the text as it is, never inside the query");
    let comment = comment_request("PR_kwDOAAAB07", "Bom trabalho!");

    assert!(comment["query"].as_str().unwrap().contains("addComment(input: {subjectId: $id, body: $body})"));
    assert_eq!(comment["variables"], json!({ "id": "PR_kwDOAAAB07", "body": "Bom trabalho!" }));
    let config = http::config(&super::request(TOKEN, "POST", GRAPHQL, Some(save.to_string())));

    assert_eq!(config.lines().count(), 7, "url, method, four headers and the body: the text adds no line of curl's own: {config}");
}

#[test]
fn a_write_says_whether_it_was_done_and_a_token_that_only_reads_is_told_what_to_give() {
    let answer = |status, body: Value| Ok(http::Answer { status, body: body.to_string() });

    assert_eq!(written(answer(200, json!({ "data": { "updatePullRequest": { "pullRequest": { "id": "x" } } } })), "updatePullRequest"), Ok(()));

    for refusal in [
        json!({ "data": { "updatePullRequest": null }, "errors": [{ "type": "FORBIDDEN", "message": "Resource not accessible by personal access token" }] }),
        json!({ "data": null, "errors": [{ "type": "INSUFFICIENT_SCOPES", "message": "Your token has not been granted the required scopes" }] }),
        json!({ "data": { "addComment": null }, "errors": [{ "message": "Resource not accessible by integration" }] }),
    ] {
        assert_eq!(written(answer(200, refusal.clone()), "addComment").unwrap_err().message, NO_WRITE, "{refusal}");
    }

    let forbidden = Ok(http::Answer { status: 403, body: "{\"message\":\"Resource not accessible by personal access token\"}".into() });

    assert_eq!(written(forbidden, "addComment").unwrap_err().message, NO_WRITE);
    let spent = written(Ok(http::Answer { status: 403, body: "{\"message\":\"API rate limit exceeded\"}".into() }), "addComment").unwrap_err();

    assert!(spent.message.contains("limite de uso"), "the hour's budget, not the permission: {}", spent.message);
    assert!(written(answer(401, json!({ "message": "Bad credentials" })), "addComment").unwrap_err().unauthorized);
    let gone = json!({ "data": { "addComment": null }, "errors": [{ "type": "NOT_FOUND", "message": "Could not resolve to a node" }] });

    assert_eq!(written(answer(200, gone), "addComment").unwrap_err().message, "O GitHub recusou: Could not resolve to a node");
    assert_eq!(written(answer(200, json!({ "data": { "addComment": null } })), "addComment").unwrap_err().message, "O GitHub não confirmou a escrita.");
    assert_eq!(written(Err("sem conexão".into()), "addComment").unwrap_err().message, "Sem conexão com o GitHub.");
    assert_eq!(NO_WRITE, "O token não pode escrever em pull requests: dê a permissão Pull requests (leitura e escrita) ao token.");
}

#[test]
fn a_picked_pull_request_reads_its_detail_and_keeps_it_by_its_id() {
    let mut app = app_with_pulse();

    assert!(app.github_load_detail("loja").is_err(), "no pull request named");
    app.github_load_detail("https://github.com/ANA-DEV/site/pull/7").unwrap();
    assert!(app.github_detail_loading("ana-dev/site#7") && app.github_detail("ana-dev/site#7").is_none(), "kept as the pulse writes its id");
    let detail = PrDetail::parse(&recorded_detail(), 5).unwrap();

    app.integrations.github.receive(Message::Detail("ana-dev/site#7".into(), Ok(Box::new(detail.clone()))), 10);
    assert!(!app.github_detail_loading("ana-dev/site#7"));
    assert_eq!(app.github_detail("Ana-Dev/Site#7"), Some(&detail));
    // a failure is kept, and the detail read before stays
    app.github_load_detail("ana-dev/site#7").unwrap();
    let offline = Failure { message: "Sem conexão com o GitHub.".into(), unauthorized: false };

    app.integrations.github.receive(Message::Detail("ana-dev/site#7".into(), Err(offline)), 20);
    assert_eq!((app.github_detail_error("ana-dev/site#7"), app.github_detail("ana-dev/site#7")), (Some("Sem conexão com o GitHub."), Some(&detail)));
    // the island reads a pull request once it shows
    app.github_load_detail("ana-dev/loja#42").unwrap();
    assert!(app.github_detail_loading("ana-dev/loja#42"));
    // a refused token stops the reads until it changes
    let refused = Failure { message: "Token inválido ou expirado.".into(), unauthorized: true };

    app.integrations.github.receive(Message::Detail("ana-dev/loja#42".into(), Err(refused)), 30);
    assert_eq!(app.github_load_detail("ana-dev/loja#43").unwrap_err(), "Token inválido ou expirado.");
    // off: nothing is asked, and what was read goes
    app.prefs.integrations.github = false;
    assert_eq!(app.github_load_detail("ana-dev/loja#43").unwrap_err(), "O GitHub está desligado em Ajustes › Integrações.");
    app.github_tick(&Context::default());
    assert!(app.github_detail("ana-dev/site#7").is_none());
}

#[test]
fn the_description_is_saved_and_a_comment_sent_one_write_at_a_time() {
    let mut app = app_with_pulse();

    app.integrations.github.receive(Message::Detail("ana-dev/site#7".into(), Ok(Box::new(PrDetail::parse(&recorded_detail(), 5).unwrap()))), 10);
    app.github_save_body("ana-dev/site#7", "## Novo\n\nTexto").unwrap();
    assert!(app.github_writing() && app.github_last_write().is_none());
    assert_eq!(app.integrations.github.sent, [save_body_request("PR_kwDOAAAB07", "## Novo\n\nTexto")]);
    assert_eq!(app.github_comment("ana-dev/site#7", "Oi").unwrap_err(), "Espere a escrita anterior terminar.");
    app.integrations.github.receive(Message::Wrote(Write::Body("## Novo\n\nTexto".into()), "ana-dev/site#7".into(), Ok(())), 20);
    assert!(!app.github_writing());
    assert_eq!(app.github_last_write(), Some(&Ok("Descrição salva.".to_string())));
    assert_eq!(app.github_detail("ana-dev/site#7").unwrap().body, "## Novo\n\nTexto", "what was read is now what was saved");
    // a comment with nothing in it is not sent, and says why
    assert!(app.github_comment("ana-dev/site#7", "  \n").is_err());
    assert_eq!(app.github_last_write(), Some(&Err("Escreva o comentário antes de enviar.".to_string())));
    app.github_comment("ana-dev/loja#42", "Bom trabalho!").unwrap();
    assert_eq!(app.integrations.github.sent.last(), Some(&comment_request("PR_kwDOAAAB42", "Bom trabalho!")), "the node's id from the pulse");
    let read_only = Failure { message: NO_WRITE.into(), unauthorized: false };

    app.integrations.github.receive(Message::Wrote(Write::Comment("Bom trabalho!".into()), "ana-dev/loja#42".into(), Err(read_only)), 30);
    assert_eq!(app.github_last_write(), Some(&Err(NO_WRITE.to_string())));
    assert!(app.integrations.github.report().ends_with(&format!("A última escrita falhou: {NO_WRITE}")));
    app.github_comment("ana-dev/loja#42", "De novo").unwrap();
    app.integrations.github.receive(Message::Wrote(Write::Comment("De novo".into()), "ana-dev/loja#42".into(), Ok(())), 40);
    assert_eq!(app.github_last_write(), Some(&Ok("Comentário enviado.".to_string())));
    assert!(app.github_save_body("outra/coisa#1", "x").unwrap_err().contains("leia o pull request antes"), "GitHub's id of its node is needed");
}

#[test]
fn a_pull_request_is_approved_and_merged_only_when_it_can_be() {
    let mut app = app_with_pulse();

    // Aprovar: someone else's, never the person's own
    assert_eq!(app.github_approve("ana-dev/loja#42").unwrap_err(), "O GitHub não deixa aprovar o próprio pull request.");
    app.github_approve("time/api#310").unwrap();
    assert_eq!(app.integrations.github.sent, [approve_request("PR_kwDOAAAB310")]);
    let query = approve_request("x")["query"].as_str().unwrap().to_string();

    assert!(query.contains("addPullRequestReview(input: {pullRequestId: $id, event: APPROVE})"), "{query}");
    app.integrations.github.receive(Message::Detail("time/api#310".into(), Ok(Box::new(PrDetail::default()))), 5);
    app.integrations.github.receive(Message::Wrote(Write::Approve, "time/api#310".into(), Ok(())), 10);
    assert_eq!(app.github_last_write(), Some(&Ok("Pull request aprovado.".to_string())));
    assert!(app.github_detail("time/api#310").is_none() && app.integrations.github.next_pulse <= 10, "read again, and the list soon");
    // Mesclar: only one read, and ready
    assert!(app.github_merge("ana-dev/loja#42").unwrap_err().starts_with("Leia ana-dev/loja#42 antes de mesclar"));
    let conflicting = PrDetail::parse(&recorded_detail(), 5).unwrap();

    assert_eq!(conflicting.merge_method, "SQUASH");
    app.integrations.github.receive(Message::Detail("ana-dev/site#7".into(), Ok(Box::new(conflicting.clone()))), 20);
    assert_eq!(app.github_merge("ana-dev/site#7").unwrap_err(), "ana-dev/site#7 tem conflito com main: resolva antes.");
    assert_eq!(app.github_last_write(), Some(&Err("ana-dev/site#7 tem conflito com main: resolva antes.".to_string())), "the tab reads why");

    let with = |change: fn(&mut PullRequest)| {
        let mut detail = conflicting.clone();

        change(&mut detail.request);
        detail.unmergeable()
    };

    assert_eq!(with(|request| request.draft = true).unwrap(), "ana-dev/site#7 é um rascunho: marque como pronto para revisão antes.");
    assert_eq!(with(|request| request.state = PrState::Merged).unwrap(), "ana-dev/site#7 já foi mesclado.");
    assert!(with(|request| request.mergeable = MergeableState::Unknown).unwrap().contains("ainda está vendo"));
    assert_eq!(with(|request| request.mergeable = MergeableState::Mergeable), None);
    let ready = PrDetail { request: app.integrations.github.pulse.as_ref().unwrap().mine[0].clone(), merge_method: "REBASE", ..Default::default() };

    app.integrations.github.receive(Message::Detail("ana-dev/loja#42".into(), Ok(Box::new(ready))), 30);
    app.github_merge("ana-dev/loja#42").unwrap();
    assert_eq!(app.integrations.github.sent.last(), Some(&merge_request("PR_kwDOAAAB42", "REBASE", Some("aaa111"))), "only at the commit read");
    let merge = merge_request("x", "MERGE", None);

    assert!(merge["query"].as_str().unwrap().contains("mergePullRequest(input: {pullRequestId: $id, mergeMethod: $method, expectedHeadOid: $head})"));
    assert_eq!(merge["variables"], json!({ "id": "x", "method": "MERGE", "head": null }));
    app.integrations.github.receive(Message::Wrote(Write::Merge("REBASE", Some("aaa111".into())), "ana-dev/loja#42".into(), Ok(())), 40);
    assert_eq!(app.github_last_write(), Some(&Ok("Pull request mesclado.".to_string())));
    assert_eq!((conflicting.merge_words(), PrDetail::default().merge_words()), ("em um commit só (squash)", "com um commit de merge"));
}

#[test]
fn the_agents_list_read_edit_and_comment_on_pull_requests_through_the_mcp() {
    let mut app = app_with_pulse();

    let github = |app: &mut DrawingApp, args: Value| {
        let mut call = json!({ "integration": "github" });

        call.as_object_mut().unwrap().extend(args.as_object().unwrap().clone());
        app.act("integrations", &call)
    };

    let all = github(&mut app, json!({ "action": "prs" })).unwrap();

    assert!(all.contains("\n- ana-dev/loja#40 · mesclado · Troca o logotipo · feat/logotipo → main · "), "{all}");

    assert!(
        all.contains("\n- ana-dev/site#7 · aberto · Corrige o carrinho quebrado · fix/carrinho → main · ") && all.contains("conflito · precisa de você"),
        "{all}"
    );

    let needs = github(&mut app, json!({ "action": "prs", "filter": "needs_you" })).unwrap();

    assert_eq!(needs.lines().count(), 3, "a heading, time/api#310 and ana-dev/site#7: {needs}");
    assert!(github(&mut app, json!({ "action": "prs", "filter": "minhas" })).is_err());
    assert!(github(&mut app, json!({ "action": "pr" })).is_err(), "which one?");
    let first = github(&mut app, json!({ "action": "pr", "pr": "ana-dev/site#7" })).unwrap();

    assert!(first.starts_with("Buscando ana-dev/site#7 no GitHub"), "the first read only asks: {first}");
    app.integrations.github.receive(Message::Detail("ana-dev/site#7".into(), Ok(Box::new(PrDetail::parse(&recorded_detail(), now_ms()).unwrap()))), now_ms());
    let read = github(&mut app, json!({ "action": "pr", "pr": "https://github.com/ana-dev/site/pull/7" })).unwrap();

    assert!(read.contains("Bloqueios: Conflito com main") && read.contains("Revisões: 2 aprovações") && read.contains("\n## O que muda\n"), "{read}");
    let text = "## Novo\n\nCom Maiúsculas e linhas";
    let sent = github(&mut app, json!({ "action": "pr_body", "pr": "ana-dev/site#7", "text": text })).unwrap();

    assert!(sent.starts_with("Enviando a descrição nova de ana-dev/site#7"), "{sent}");
    assert_eq!(app.integrations.github.sent.last(), Some(&save_body_request("PR_kwDOAAAB07", text)), "the text keeps its case and its lines");
    app.integrations.github.receive(Message::Wrote(Write::Body(text.into()), "ana-dev/site#7".into(), Ok(())), now_ms());
    let status = github(&mut app, json!({ "action": "status" })).unwrap();

    assert!(status.ends_with("Última escrita: Descrição salva.") && !status.contains("#40"), "the merged ones stay out of status: {status}");
    github(&mut app, json!({ "action": "pr_comment", "pr": "time/api#310", "text": "Pode revisar de novo?" })).unwrap();
    assert_eq!(app.integrations.github.sent.last(), Some(&comment_request("PR_kwDOAAAB310", "Pode revisar de novo?")));
    app.integrations.github.receive(Message::Wrote(Write::Comment("Pode revisar de novo?".into()), "time/api#310".into(), Ok(())), now_ms());
    let approving = github(&mut app, json!({ "action": "pr_approve", "pr": "time/api#310" })).unwrap();

    assert!(approving.starts_with("Enviando a aprovação de time/api#310"), "{approving}");
    assert_eq!(app.integrations.github.sent.last(), Some(&approve_request("PR_kwDOAAAB310")));
    app.integrations.github.receive(Message::Wrote(Write::Approve, "time/api#310".into(), Ok(())), now_ms());
    assert!(github(&mut app, json!({ "action": "pr_merge", "pr": "ana-dev/site#7" })).unwrap_err().contains("tem conflito com main"));
    assert!(!format!("{all}{read}{status}").contains(TOKEN), "never the token");
}
