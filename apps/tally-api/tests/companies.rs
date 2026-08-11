//! Company CRUD + ownership integration tests (spec §5).  Gated behind
//! `pg-tests` (on by default); skipped gracefully when Postgres is
//! unreachable.
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::{Method, StatusCode};
use common::{assert_error, json_body, request, TestApp};
use serde_json::json;

#[tokio::test]
async fn create_list_get_patch_delete() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let token = app.register("owner@example.com").await;

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

#[tokio::test]
async fn create_requires_name() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let token = app.register("named@example.com").await;

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
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let token = app.register("dup@example.com").await;

    let body = json!({ "name": "First Ltd", "company_number": "01234567" });
    let resp = app.send(request(Method::POST, "/api/v1/companies", Some(&token), Some(&body))).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app.send(request(Method::POST, "/api/v1/companies", Some(&token), Some(&body))).await;
    assert_error(resp, StatusCode::CONFLICT, "duplicate_company").await;
}

#[tokio::test]
async fn companies_are_ownership_scoped() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let alice = app.register("alice@example.com").await;
    let bob = app.register_second("bob@example.com").await;

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
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let token = app.register("search@example.com").await;

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
