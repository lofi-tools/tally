//! Guest sweep integration tests (temp-user spec §8). Gated behind
//! `pg-tests` (on by default); the harness auto-starts the `test-api-db`
//! container and fails hard when the database can't be reached.
#![cfg(feature = "pg-tests")]


use axum::http::{Method, StatusCode};
use tally_tests_common::{assert_error, json_body, request, request_with_guest, unique_email, TestApp};
use serde_json::json;
use tally_api::models::{Company, Job, User};

/// `POST /auth/guest` → `(token, user id)`.
async fn bootstrap_guest(app: &TestApp, guest_id: &str) -> (String, uuid::Uuid) {
    let resp = app
        .send(request_with_guest(
            Method::POST,
            "/api/v1/auth/guest",
            Some(guest_id),
            None,
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "guest bootstrap");
    let body = json_body(resp).await;
    (
        body["token"].as_str().expect("token").to_string(),
        body["user"]["id"].as_str().expect("user id").parse().expect("uuid"),
    )
}

/// `POST /companies` → company id.
async fn add_company(app: &TestApp, token: &str, name: &str) -> uuid::Uuid {
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(token),
            Some(&json!({ "name": name })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "create {name}");
    json_body(resp).await["id"].as_str().expect("company id").parse().expect("uuid")
}

#[tokio::test]
async fn sweep_deletes_abandoned_guests_and_keeps_recent_and_real_users() {
    let app = TestApp::setup().await;

    // A real user + company (never swept).
    let keeper_email = unique_email("keeper");
    let real_token = app.register(&keeper_email).await;
    let real_company = add_company(&app, &real_token, "Real Ltd").await;

    // Guest A: abandoned — activity backdated 100 days.
    let (guest_a_token, guest_a_id) = bootstrap_guest(&app, "sweep-stale").await;
    let stale_company = add_company(&app, &guest_a_token, "Stale Guest Ltd").await;

    // A job row proves the cascade deletes the owned rows with the company.
    let mut db = app.db.clone();
    toasty::create!(Job {
        kind: "fetch_filings".to_string(),
        company_id: stale_company,
        status: "pending".to_string(),
        attempts: 0,
        last_error: None,
        next_retry_at: None,
        created_at: "2020-01-01T00:00:00Z".to_string(),
        updated_at: "2020-01-01T00:00:00Z".to_string(),
    })
    .exec(&mut db)
    .await
    .expect("insert job");

    // Backdate the activity clock (companies.updated_at + users.created_at).
    let old = "2020-01-01T00:00:00Z";
    toasty::sql::statement("UPDATE \"companies\" SET \"updated_at\" = $1 WHERE \"id\" = $2")
        .bind(old)
        .bind(stale_company)
        .exec(&mut db)
        .await
        .expect("backdate company");
    toasty::sql::statement("UPDATE \"users\" SET \"created_at\" = $1 WHERE \"id\" = $2")
        .bind(old)
        .bind(guest_a_id)
        .exec(&mut db)
        .await
        .expect("backdate user");

    // Guest B: recent activity — survives.
    let (guest_b_token, guest_b_id) = bootstrap_guest(&app, "sweep-recent").await;
    let recent_company = add_company(&app, &guest_b_token, "Recent Guest Ltd").await;

    // Run the sweep with the default 90-day TTL.
    let deleted = tally_api::sweep::sweep_abandoned_guests(&mut db, 90)
        .await
        .expect("sweep runs");
    assert_eq!(deleted, 1, "only the stale guest is swept");

    // Guest A is gone: user, company, job, session.
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&guest_a_token), None))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "auth_invalid").await;
    assert!(
        User::filter_by_id(guest_a_id).first().exec(&mut db).await.expect("query").is_none(),
        "stale guest user deleted"
    );
    assert!(
        Company::filter_by_id(stale_company).first().exec(&mut db).await.expect("query").is_none(),
        "stale guest company deleted"
    );
    let job_rows = toasty::sql::query("SELECT \"id\" FROM \"jobs\" WHERE \"company_id\" = $1")
        .bind(stale_company)
        .exec(&mut db)
        .await
        .expect("query jobs");
    assert!(job_rows.is_empty(), "job cascade-deleted with the company");

    // Guest B survives with its company and session.
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&guest_b_token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "recent guest session intact");
    assert!(
        User::filter_by_id(guest_b_id).first().exec(&mut db).await.expect("query").is_some(),
        "recent guest kept"
    );
    assert!(
        Company::filter_by_id(recent_company).first().exec(&mut db).await.expect("query").is_some(),
        "recent guest company kept"
    );

    // The real user is untouched.
    assert!(
        User::filter_by_email(&keeper_email).first().exec(&mut db).await.expect("query").is_some(),
        "real user kept"
    );
    assert!(
        Company::filter_by_id(real_company).first().exec(&mut db).await.expect("query").is_some(),
        "real company kept"
    );
}

#[tokio::test]
async fn sweep_leaves_guest_with_no_owned_rows_alone_until_ttl() {
    let app = TestApp::setup().await;

    // A guest that bootstrapped but never added anything, with a *recent*
    // created_at: no owned rows → activity falls back to created_at → kept.
    let (token, id) = bootstrap_guest(&app, "sweep-idle").await;
    let mut db = app.db.clone();
    let deleted = tally_api::sweep::sweep_abandoned_guests(&mut db, 90)
        .await
        .expect("sweep runs");
    assert_eq!(deleted, 0);
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Backdate it past the TTL → now it is swept (spec §9: "Guest deletes
    // their only company → the temp user row remains until the sweep").
    toasty::sql::statement("UPDATE \"users\" SET \"created_at\" = $1 WHERE \"id\" = $2")
        .bind("2020-01-01T00:00:00Z")
        .bind(id)
        .exec(&mut db)
        .await
        .expect("backdate user");
    let deleted = tally_api::sweep::sweep_abandoned_guests(&mut db, 90)
        .await
        .expect("sweep runs");
    assert_eq!(deleted, 1);
    assert!(
        User::filter_by_id(id).first().exec(&mut db).await.expect("query").is_none(),
        "idle guest swept once old"
    );
}
