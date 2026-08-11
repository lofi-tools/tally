//! Ledger upload + JSON views integration tests (spec §9).  Gated behind
//! `pg-tests` (on by default); skipped gracefully when Postgres is
//! unreachable.
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::{Method, StatusCode};
use common::{assert_error, json_body, multipart_body, request, TestApp, FIXTURE_GNUCASH};
use serde_json::json;

/// Create a company, returning (token, company_id).
async fn seed_company(app: &TestApp, email: &str) -> (String, String) {
    let token = app.register(email).await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "Ledger Ltd", "company_number": "01234567" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let id = json_body(resp).await["id"].as_str().unwrap().to_string();
    (token, id)
}

#[tokio::test]
async fn upload_list_get_views_delete() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let (token, company_id) = seed_company(&app, "ledger@example.com").await;

    let bytes = std::fs::read(FIXTURE_GNUCASH).expect("fixture exists");
    let boundary = "----tally-test-boundary";
    let resp = app
        .send(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/companies/{company_id}/ledgers"))
                .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
                .header(
                    axum::http::header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(multipart_body(boundary, "input.gnucash", &bytes))
                .expect("request"),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ledger = json_body(resp).await;
    let ledger_id = ledger["id"].as_str().expect("ledger id").to_string();
    assert!(ledger["accounts_count"].as_i64().unwrap() > 0);
    assert!(ledger["transactions_count"].as_i64().unwrap() > 0);
    assert!(!ledger["file_sha256"].as_str().unwrap().is_empty());

    // list
    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/companies/{company_id}/ledgers"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let list = json_body(resp).await;
    assert!(list.as_array().unwrap().iter().any(|l| l["id"] == ledger_id));

    // get
    let resp = app
        .send(request(Method::GET, &format!("/api/v1/ledgers/{ledger_id}"), Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["id"], ledger_id);

    // accounts view
    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/ledgers/{ledger_id}/accounts"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let accounts = json_body(resp).await;
    assert!(accounts["accounts"].as_array().unwrap().len() > 0);
    assert!(accounts["net_assets"].is_string());

    // transactions view
    let resp = app
        .send(request(
            Method::GET,
            &format!("/api/v1/ledgers/{ledger_id}/transactions"),
            Some(&token),
            None,
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let page = json_body(resp).await;
    assert!(page["items"].as_array().unwrap().len() > 0);
    assert_eq!(page["limit"], 50);
    assert_eq!(page["offset"], 0);

    // delete → 204, then get → 404 not_found
    let resp = app
        .send(request(Method::DELETE, &format!("/api/v1/ledgers/{ledger_id}"), Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = app
        .send(request(Method::GET, &format!("/api/v1/ledgers/{ledger_id}"), Some(&token), None))
        .await;
    assert_error(resp, StatusCode::NOT_FOUND, "not_found").await;
}

#[tokio::test]
async fn upload_rejects_wrong_extension() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let (token, company_id) = seed_company(&app, "ext@example.com").await;

    let boundary = "----tally-test-boundary";
    let resp = app
        .send(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/companies/{company_id}/ledgers"))
                .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
                .header(
                    axum::http::header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(multipart_body(boundary, "ledger.csv", b"a,b,c\n1,2,3"))
                .expect("request"),
        )
        .await;
    assert_error(resp, StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_file_type").await;
}

#[tokio::test]
async fn upload_to_foreign_company_is_not_found() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let (alice, alice_company) = seed_company(&app, "alice@example.com").await;
    let bob = app.register_second("bob@example.com").await;
    let _ = alice_company;

    let bytes = std::fs::read(FIXTURE_GNUCASH).expect("fixture exists");
    let boundary = "----tally-test-boundary";
    let resp = app
        .send(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/companies/{alice_company}/ledgers"))
                .header(axum::http::header::AUTHORIZATION, format!("Bearer {bob}"))
                .header(
                    axum::http::header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(multipart_body(boundary, "input.gnucash", &bytes))
                .expect("request"),
        )
        .await;
    assert_error(resp, StatusCode::NOT_FOUND, "not_found").await;

    let _ = alice;
}

#[tokio::test]
async fn upload_requires_file_field() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let (token, company_id) = seed_company(&app, "nofile@example.com").await;

    // A multipart body with no `file` field → 422 validation_failed.
    let boundary = "----tally-test-boundary";
    let body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"other\"\r\n\r\nx\r\n--{boundary}--\r\n");
    let resp = app
        .send(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/companies/{company_id}/ledgers"))
                .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
                .header(
                    axum::http::header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(axum::body::Body::from(body))
                .expect("request"),
        )
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;
}
