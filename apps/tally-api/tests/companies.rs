//! Company CRUD + ownership integration tests (spec §5).  Gated behind
//! `pg-tests` (on by default); the harness auto-starts the `test-api-db`
//! container and fails hard when the database can't be reached.
#![cfg(feature = "pg-tests")]


use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use tally_tests_common::{assert_error, json_body, request, unique_email, TestApp};
use serde_json::json;

#[tokio::test]
async fn create_list_get_patch_delete() {
    let app = TestApp::setup().await;
    let token = app.register(&unique_email("owner")).await;

    // create
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({
                "name": "Acme Widgets Ltd",
                "tax_reference": "1234567890",
                "company_number": "01234567",
            })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let company = json_body(resp).await;
    let id = company["id"].as_str().expect("company id").to_string();
    assert_eq!(company["name"], "Acme Widgets Ltd");

    // list
    let resp = app
        .send(request(Method::GET, "/api/v1/companies", Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let list = json_body(resp).await;
    let ids = list.as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect::<Vec<_>>();
    assert!(ids.contains(&id.as_str()), "created company in list");

    // get
    let resp = app
        .send(request(Method::GET, &format!("/api/v1/companies/{id}"), Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["name"], "Acme Widgets Ltd");

    // patch
    let resp = app
        .send(request(
            Method::PATCH,
            &format!("/api/v1/companies/{id}"),
            Some(&token),
            Some(&json!({ "name": "Acme Widgets Holdings Ltd" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["name"], "Acme Widgets Holdings Ltd");

    // delete → 204, then get → 404 not_found
    let resp = app
        .send(request(Method::DELETE, &format!("/api/v1/companies/{id}"), Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = app
        .send(request(Method::GET, &format!("/api/v1/companies/{id}"), Some(&token), None))
        .await;
    assert_error(resp, StatusCode::NOT_FOUND, "not_found").await;
}

/// temp-user spec §6.1: `accounting_standard` round-trips through create /
/// PATCH and is validated (FRS 105 / FRS 102 only).
#[tokio::test]
async fn accounting_standard_roundtrips() {
    let app = TestApp::setup().await;
    let token = app.register(&unique_email("standard")).await;

    // Explicit FRS 102 is persisted.
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "FRS102 Ltd", "accounting_standard": "FRS 102" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let id = json_body(resp).await["id"].as_str().unwrap().to_string();

    // Absent → default FRS 105.
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "Default Ltd" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["accounting_standard"], "FRS 105");

    // PATCH switches the standard.
    let resp = app
        .send(request(
            Method::PATCH,
            &format!("/api/v1/companies/{id}"),
            Some(&token),
            Some(&json!({ "accounting_standard": "FRS 105" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["accounting_standard"], "FRS 105");

    // An unknown standard → 422 validation_failed.
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "Bad Ltd", "accounting_standard": "GAAP 2020" })),
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;
}

#[tokio::test]
async fn create_requires_name() {
    let app = TestApp::setup().await;
    let token = app.register(&unique_email("named")).await;

    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "company_number": "01234567" })),
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;
}

#[tokio::test]
async fn duplicate_company_number_is_rejected() {
    let app = TestApp::setup().await;
    let token = app.register(&unique_email("dup")).await;

    let body = json!({ "name": "First Ltd", "company_number": "01234567" });
    let resp = app.send(request(Method::POST, "/api/v1/companies", Some(&token), Some(&body))).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app.send(request(Method::POST, "/api/v1/companies", Some(&token), Some(&body))).await;
    assert_error(resp, StatusCode::CONFLICT, "duplicate_company").await;
}

#[tokio::test]
async fn companies_are_ownership_scoped() {
    let app = TestApp::setup().await;
    let alice = app.register(&unique_email("alice")).await;
    let bob = app.register_second(&unique_email("bob")).await;

    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&alice),
            Some(&json!({ "name": "Alice Ltd" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let id = json_body(resp).await["id"].as_str().unwrap().to_string();

    // Bob cannot see Alice's company (404, not 403 — no resource leak).
    let resp = app
        .send(request(Method::GET, &format!("/api/v1/companies/{id}"), Some(&bob), None))
        .await;
    assert_error(resp, StatusCode::NOT_FOUND, "not_found").await;

    // Bob's list is empty.
    let resp = app.send(request(Method::GET, "/api/v1/companies", Some(&bob), None)).await;
    let list = json_body(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn search_without_key_is_a_clear_400() {
    let app = TestApp::setup().await;
    let token = app.register(&unique_email("search")).await;

    // Missing q → 422 validation_failed.
    let resp = app
        .send(request(Method::GET, "/api/v1/companies/search", Some(&token), None))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;

    // No API key configured (tests build AppState with ch: None) → 400.
    let resp = app
        .send(request(
            Method::GET,
            "/api/v1/companies/search?q=acme",
            Some(&token),
            None,
        ))
        .await;
    assert_error(resp, StatusCode::BAD_REQUEST, "companies_house_key_missing").await;
}

/// §7.2: search is deliberately unprotected — the web app queries it
/// pre-login. No bearer token → still a 422/400, never 401 auth_missing.
#[tokio::test]
async fn search_is_unprotected() {
    let app = TestApp::setup().await;

    // No token at all: missing q → 422 validation_failed (auth is not the
    // gate), and a real query with no CH key → 400, not 401.
    let resp = app
        .send(request(Method::GET, "/api/v1/companies/search", None, None))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;

    let resp = app
        .send(request(
            Method::GET,
            "/api/v1/companies/search?q=acme",
            None,
            None,
        ))
        .await;
    assert_error(resp, StatusCode::BAD_REQUEST, "companies_house_key_missing").await;
}

#[tokio::test]
async fn create_rejects_malformed_json() {
    let app = TestApp::setup().await;
    let token = app.register(&unique_email("badjson")).await;

    // A body that is not valid JSON (with the JSON content type) → 400
    // `invalid_json` with the serde position in details.
    let resp = app
        .send(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/companies")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{"))
                .expect("request"),
        )
        .await;
    // The §11.1 envelope: 400 + code, and details carry the serde message
    // with the position parsed from the rejection text (§11.2).
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = json_body(resp).await;
    assert_eq!(json["error"]["code"], "invalid_json");
    assert!(json["error"]["details"]["message"].is_string());
    assert_eq!(json["error"]["details"]["line"], 1);
    assert_eq!(json["error"]["details"]["column"], 1);
}
