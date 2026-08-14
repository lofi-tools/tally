//! Filings sync integration tests (spec: ch-filings-sync-spec.md §5, §7).
//! Gated behind `pg-tests` (on by default); skipped gracefully when Postgres
//! is unreachable.
//!
//! The harness builds `AppState { ch: None }` (no CH key), so these tests
//! cover the no-key paths: the refresh endpoint's `companies_house_key_missing`,
//! the period derivation from the stored registration date, and the job
//! enqueue/status plumbing (via the harness's `db` handle, where jobs can be
//! poked directly).
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::{Method, StatusCode};
use common::{assert_error, json_body, request, TestApp};
use ixbrl::reports::uk_frs105_accounts::PreviousYearFigures;
use serde_json::json;
use tally_api::models::{BalanceSheet, Job};

/// Seed a company with a registration date (so periods derive) + a number
/// (so a job would be enqueued if a key were set). Returns (token, company_id).
async fn seed_company(app: &TestApp, email: &str) -> (String, String) {
    let token = app.register(email).await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({
                "name": "Filings Co Ltd",
                "company_number": "01234567",
                "registration_date": "2015-03-01",
                "report_date": "2020-05-30",
                "authorised_date": "2020-06-01",
            })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "seed company");
    let company_id = json_body(resp).await["id"].as_str().unwrap().to_string();
    (token, company_id)
}

/// The expected registration-date schedule ends (incorporation → today),
/// newest first — the same schedule the derivation walks, computed here via
/// the ixbrl lib so the assertion is independent of the backend's internals.
fn schedule_ends_desc(reg: &str) -> Vec<String> {
    let today = chrono::Utc::now().date_naive();
    let mut lib = ixbrl::company::Company::new("", "", String::new());
    lib.registration_date = chrono::NaiveDate::parse_from_str(reg, "%Y-%m-%d").unwrap();
    let mut ends: Vec<String> = Vec::new();
    let mut n = 0u32;
    loop {
        let p = lib.accounting_period_n(n);
        if p.start > today {
            break;
        }
        ends.push(p.end.to_string());
        if p.contains(today) {
            break;
        }
        n += 1;
    }
    ends.sort_by(|a, b| b.cmp(a)); // newest first
    ends
}

#[tokio::test]
async fn refresh_without_ch_key_is_rejected() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let (token, company_id) = seed_company(&app, "refresh@example.com").await;

    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/filings/refresh"),
            Some(&token),
            None,
        ))
        .await;
    assert_error(resp, StatusCode::BAD_REQUEST, "companies_house_key_missing").await;
}

#[tokio::test]
async fn refresh_without_company_number_is_rejected() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    // No company_number → the refresh would be a no-op, so it must 422.
    let token = app.register("nonumber@example.com").await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "Hand Made Ltd" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let company_id = json_body(resp).await["id"].as_str().unwrap().to_string();

    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/filings/refresh"),
            Some(&token),
            None,
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;
}

#[tokio::test]
async fn list_derives_periods_from_registration_date() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let (token, company_id) = seed_company(&app, "periods@example.com").await;

    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/companies/{company_id}/filings"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json = json_body(resp).await;

    // No filings ever fetched → status none, no balance sheets.
    assert_eq!(json["status"]["state"], "none");
    assert_eq!(json["balance_sheets"].as_array().unwrap().len(), 0);

    // Periods derive from the 2015-03-01 registration-date schedule, newest
    // first, with the period containing today marked ongoing.
    let periods = json["periods"].as_array().unwrap();
    assert!(!periods.is_empty(), "periods from registration date");
    let ongoing = periods
        .iter()
        .find(|p| p["status"] == "ongoing")
        .expect("an ongoing period");
    assert!(ongoing["due"]["hmrc"].is_string());
    assert!(ongoing["due"]["ch"].is_string());
    // The ongoing period's expected filings are the not-sent accounts + CT600.
    let filings = ongoing["filings"].as_array().unwrap();
    assert!(
        filings.iter().any(|f| f["kind"] == "accounts" && f["state"] == "not-sent"),
        "expected accounts not-sent: {filings:?}"
    );
    assert!(
        filings.iter().any(|f| f["kind"] == "corporation-tax" && f["state"] == "not-sent"),
        "expected corporation-tax not-sent: {filings:?}"
    );

    // Nothing was ever fetched (no CH key in the harness) → every ended
    // period is provisional: the full schedule since incorporation is
    // listed up front, structure-only (no deadlines, no expected filings).
    assert!(
        periods.iter().all(|p| p["status"] != "pending"),
        "no completed fetch → nothing is pending yet"
    );
    let provisional: Vec<_> = periods.iter().filter(|p| p["status"] == "provisional").collect();
    assert!(!provisional.is_empty(), "pre-fetch periods are provisional");
    for p in &provisional {
        assert!(
            p["due"].is_null(),
            "provisional is structure-only (no due): {p}"
        );
        assert_eq!(
            p["filings"].as_array().unwrap().len(),
            0,
            "provisional is structure-only (no filings): {p}"
        );
    }
    // Every period is either the functional ongoing or a provisional estimate.
    assert_eq!(provisional.len(), periods.len() - 1);
}

#[tokio::test]
async fn no_company_number_lists_deterministic_provisional_periods() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    // A company with a registration date but no CH number can never spawn a
    // fetch job (the refresh endpoint rejects it), so the provisional
    // derivation is fully deterministic: the whole registration-date
    // schedule appears, every ended period is provisional + structure-only,
    // exactly one functional ongoing, and nothing pending (spec §8).
    let token = app.register("nonumber-provisional@example.com").await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({
                "name": "No Number Ltd",
                "registration_date": "2015-03-01",
            })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let company_id = json_body(resp).await["id"].as_str().unwrap().to_string();

    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/companies/{company_id}/filings"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json = json_body(resp).await;

    // No fetch possible → status stays `none`.
    assert_eq!(json["status"]["state"], "none");
    assert_eq!(json["balance_sheets"].as_array().unwrap().len(), 0);

    // The period list is exactly the schedule ends, newest first.
    let expected = schedule_ends_desc("2015-03-01");
    let periods = json["periods"].as_array().unwrap();
    assert_eq!(
        periods.iter().map(|p| p["end"].as_str().unwrap()).collect::<Vec<_>>(),
        expected.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        "exactly the schedule periods since incorporation"
    );

    // The current period is functional; every ended period is a
    // structure-only provisional estimate; nothing is pending.
    let ongoing_end = &expected[0]; // newest = the period containing today
    assert_eq!(periods.len(), expected.len());
    for p in periods {
        if p["end"] == ongoing_end.as_str() {
            assert_eq!(p["status"], "ongoing");
            assert!(p["due"]["hmrc"].is_string(), "ongoing keeps its deadlines");
            assert_eq!(
                p["filings"].as_array().unwrap().len(),
                2,
                "ongoing expected filings"
            );
        } else {
            assert_eq!(p["status"], "provisional", "period {}", p["end"]);
            assert!(p["due"].is_null(), "provisional is structure-only");
            assert_eq!(
                p["filings"].as_array().unwrap().len(),
                0,
                "provisional is structure-only"
            );
        }
    }
}

#[tokio::test]
async fn fetch_enriches_and_invalidates_provisional_periods() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    // Enrichment + invalidation round-trip (spec §4.3, §7): a completed
    // fetch stores a balance sheet whose period end (2020-06-30) does not
    // match the 03-31 ARD schedule — the CH-derived period anchors a `filed`
    // row and the overlapping 03-31 estimates are dropped, while covered
    // ends become `pending` and uncovered ones stay `provisional`.
    let token = app.register("enrich@example.com").await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({
                "name": "Enrich Co Ltd",
                "registration_date": "2015-03-01",
            })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let company_id = json_body(resp).await["id"].as_str().unwrap().to_string();
    let company_uuid = uuid::Uuid::parse_str(&company_id).unwrap();

    let figures = PreviousYearFigures {
        fixed_assets: 1000.0,
        current_assets: 5000.0,
        creditors_within_1_year: -2000.0,
        net_assets: 4000.0,
        capital_and_reserves: 4000.0,
        ..Default::default()
    };
    let mut db = app.db.clone();
    // The fetch's artefacts: a done job (coverage 2024-06-01) + the parsed
    // balance sheet for the changed-ARD period.
    toasty::create!(Job {
        kind: "fetch_filings".to_string(),
        company_id: company_uuid,
        status: "done".to_string(),
        attempts: 1,
        last_error: None,
        next_retry_at: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-06-01T00:00:00Z".to_string(),
    })
    .exec(&mut db)
    .await
    .expect("insert done job");
    toasty::create!(BalanceSheet {
        company_id: company_uuid,
        period_end: "2020-06-30".to_string(),
        filed_on: Some("2020-11-12".to_string()),
        source_filing_id: None,
        figures: toasty::Json(figures),
        raw_document: None,
        parsed_document: None,
        created_at: "2024-06-01T00:00:01Z".to_string(),
    })
    .exec(&mut db)
    .await
    .expect("insert balance sheet");

    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/companies/{company_id}/filings"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json = json_body(resp).await;
    assert_eq!(json["status"]["state"], "done");

    let periods = json["periods"].as_array().unwrap();
    let by_end = |end: &str| periods.iter().find(|p| p["end"] == end);

    // The CH-derived period anchors a `filed` row with its balance sheet…
    let filed = by_end("2020-06-30").expect("filed end anchors");
    assert_eq!(filed["status"], "filed");
    assert!(filed["due"].is_null());
    // …and the overlapping 03-31 estimates (before and after the change)
    // are invalidated.
    assert!(by_end("2020-03-31").is_none(), "overlapping estimate dropped");
    assert!(by_end("2021-03-31").is_none(), "phantom after ARD change dropped");

    // Covered-and-unfiled ends are actionable `pending`…
    let covered = by_end("2024-03-31").expect("covered end");
    assert_eq!(covered["status"], "pending");
    assert!(covered["due"]["hmrc"].is_string());
    // …uncovered ended periods stay `provisional` structure-only…
    let uncovered = by_end("2025-03-31").expect("uncovered end");
    assert_eq!(uncovered["status"], "provisional");
    assert!(uncovered["due"].is_null());
    // …and the ongoing period stays functional.
    assert!(periods.iter().any(|p| p["status"] == "ongoing" && p["due"]["hmrc"].is_string()));
}

#[tokio::test]
async fn list_without_registration_date_and_history_is_empty() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let token = app.register("noanchor@example.com").await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "No Anchor Ltd" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let company_id = json_body(resp).await["id"].as_str().unwrap().to_string();

    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/companies/{company_id}/filings"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json = json_body(resp).await;
    assert_eq!(json["periods"].as_array().unwrap().len(), 0, "no anchor → no periods");
    assert_eq!(json["status"]["state"], "none");
}

#[tokio::test]
async fn list_without_registration_date_but_with_filed_history_is_anchored_by_balance_sheet() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    // A name-only company (no registration date → no ongoing period, no
    // schedule) that has a stored balance sheet: the filed period must still
    // appear and the endpoint must not panic (regression: an eager `expect`
    // fired whenever the registration date was missing).
    let token = app.register("bsanchor@example.com").await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "Filed History Ltd" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let company_id = json_body(resp).await["id"].as_str().unwrap().to_string();
    let company_uuid = uuid::Uuid::parse_str(&company_id).unwrap();

    let figures = PreviousYearFigures {
        fixed_assets: 1000.0,
        current_assets: 5000.0,
        creditors_within_1_year: -2000.0,
        net_assets: 4000.0,
        capital_and_reserves: 4000.0,
        ..Default::default()
    };
    let mut db = app.db.clone();
    toasty::create!(BalanceSheet {
        company_id: company_uuid,
        period_end: "2025-03-31".to_string(),
        filed_on: Some("2025-12-12".to_string()),
        source_filing_id: None,
        figures: toasty::Json(figures),
        raw_document: None,
        parsed_document: None,
        created_at: "2026-01-01T00:00:00Z".to_string(),
    })
    .exec(&mut db)
    .await
    .expect("insert balance sheet");

    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/companies/{company_id}/filings"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json = json_body(resp).await;

    // The stored balance sheet comes back with its figures…
    let bss = json["balance_sheets"].as_array().unwrap();
    assert_eq!(bss.len(), 1);
    assert_eq!(bss[0]["period_end"], "2025-03-31");
    assert_eq!(bss[0]["figures"]["net_assets"], 4000.0);

    // …and its period end anchors a single `filed` period.
    let periods = json["periods"].as_array().unwrap();
    assert_eq!(periods.len(), 1);
    assert_eq!(periods[0]["end"], "2025-03-31");
    assert_eq!(periods[0]["status"], "filed");
}

#[tokio::test]
async fn coverage_uses_last_done_job_even_when_a_later_fetch_failed() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    // Spec §7 edge case: a refresh fails after a prior success must not
    // revert covered periods — the derivation anchors coverage to the last
    // *done* job, not the latest job.
    let (token, company_id) = seed_company(&app, "coverage@example.com").await;
    let company_uuid = uuid::Uuid::parse_str(&company_id).unwrap();
    let mut db = app.db.clone();

    // A successful fetch completing 2024-06-01…
    toasty::create!(Job {
        kind: "fetch_filings".to_string(),
        company_id: company_uuid,
        status: "done".to_string(),
        attempts: 1,
        last_error: None,
        next_retry_at: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-06-01T00:00:00Z".to_string(),
    })
    .exec(&mut db)
    .await
    .expect("insert done job");
    // …then a failed refresh afterwards (the latest job).
    toasty::create!(Job {
        kind: "fetch_filings".to_string(),
        company_id: company_uuid,
        status: "failed".to_string(),
        attempts: 1,
        last_error: Some("boom".to_string()),
        next_retry_at: None,
        created_at: "2026-06-01T00:00:00Z".to_string(),
        updated_at: "2026-07-01T00:00:00Z".to_string(),
    })
    .exec(&mut db)
    .await
    .expect("insert failed job");

    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/companies/{company_id}/filings"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json = json_body(resp).await;

    // The banner state reflects the latest (failed) fetch…
    assert_eq!(json["status"]["state"], "failed");
    assert_eq!(json["status"]["last_error"], "boom");

    // …but periods the 2024-06-01 success covered stay pending, and only
    // later schedule ends are provisional (per-row, no global revert).
    let periods = json["periods"].as_array().unwrap();
    let by_end = |end: &str| {
        periods
            .iter()
            .find(|p| p["end"] == end)
            .unwrap_or_else(|| panic!("period {end} in {:?}", periods.iter().map(|p| &p["end"]).collect::<Vec<_>>()))
    };
    let covered = by_end("2024-03-31");
    assert_eq!(covered["status"], "pending", "covered by the last done fetch");
    assert!(covered["due"]["hmrc"].is_string());
    let uncovered = by_end("2025-03-31");
    assert_eq!(uncovered["status"], "provisional", "ended after the last success");
    assert!(uncovered["due"].is_null());
    // The ongoing period stays functional.
    assert!(periods.iter().any(|p| p["status"] == "ongoing" && p["due"]["hmrc"].is_string()));
}

#[tokio::test]
async fn list_is_ownership_scoped() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let (token, company_id) = seed_company(&app, "owner@example.com").await;
    let other = app.register_second("intruder@example.com").await;

    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/companies/{company_id}/filings"),
            Some(&other),
            None,
        ))
        .await;
    assert_error(resp, StatusCode::NOT_FOUND, "not_found").await;

    let _ = token; // the owner's token is only used to prove ownership above
}

#[tokio::test]
async fn enqueue_dedupes_inflight_jobs_and_status_tracks_them() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let (token, company_id) = seed_company(&app, "jobs@example.com").await;

    // Poke the job table directly (the harness has no CH key, so the
    // refresh endpoint can't enqueue): a first enqueue inserts…
    let mut db = app.db.clone();
    let company_uuid = uuid::Uuid::parse_str(&company_id).unwrap();
    let first = tally_api::jobs::enqueue(&mut db, "fetch_filings", company_uuid)
        .await
        .expect("enqueue");
    assert!(first.is_some(), "first enqueue inserts a job");

    // …and a second (while the first is still pending) is a no-op.
    let second = tally_api::jobs::enqueue(&mut db, "fetch_filings", company_uuid)
        .await
        .expect("enqueue");
    assert_eq!(second, None, "duplicate in-flight enqueue is a no-op");

    // The list endpoint reports the pending job.
    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/companies/{company_id}/filings"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json = json_body(resp).await;
    assert_eq!(json["status"]["state"], "pending");
}
