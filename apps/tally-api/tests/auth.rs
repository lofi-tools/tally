//! Auth flow integration tests (spec §10).  Gated behind `pg-tests` (on by
//! default); skipped gracefully when Postgres is unreachable.
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::{Method, StatusCode};
use common::{assert_error, json_body, request, TestApp};

#[tokio::test]
async fn register_login_me_logout_roundtrip() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };

    // register → token + user
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/register",
            None,
            Some(&serde_json::json!({
                "display_name": "Ada Lovelace",
                "email": "ada@example.com",
                "password": "hunter2hunter2",
            })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json = json_body(resp).await;
    let token = json["token"].as_str().expect("token").to_string();
    assert_eq!(json["user"]["email"], "ada@example.com");

    // me → same user
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let me = json_body(resp).await;
    assert_eq!(me["email"], "ada@example.com");

    // login → a fresh token works
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(&serde_json::json!({
                "email": "ada@example.com",
                "password": "hunter2hunter2",
            })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let login = json_body(resp).await;
    assert_eq!(login["user"]["display_name"], "Ada Lovelace");

    // logout revokes the session
    let resp = app
        .send(request(Method::POST, "/api/v1/auth/logout", Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&token), None))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "auth_invalid").await;
}

#[tokio::test]
async fn register_validates_semantics() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };

    // short password + bad email → 422 validation_failed with field issues
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/register",
            None,
            Some(&serde_json::json!({
                "display_name": "  ",
                "email": "not-an-email",
                "password": "short",
            })),
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;

    // duplicate email → 409 email_taken
    let email = "dup@example.com";
    app.register(email).await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/register",
            None,
            Some(&serde_json::json!({
                "display_name": "Dup",
                "email": email,
                "password": "hunter2hunter2",
            })),
        ))
        .await;
    assert_error(resp, StatusCode::CONFLICT, "email_taken").await;
}

#[tokio::test]
async fn login_rejects_bad_credentials_without_enumeration() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };

    app.register("known@example.com").await;

    // wrong password → 401 invalid_credentials
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(&serde_json::json!({
                "email": "known@example.com",
                "password": "wrong-password",
            })),
        ))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "invalid_credentials").await;

    // unknown email → the same envelope (no user enumeration)
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(&serde_json::json!({
                "email": "ghost@example.com",
                "password": "whatever123",
            })),
        ))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "invalid_credentials").await;
}

#[tokio::test]
async fn protected_routes_require_bearer_token() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };

    // no header → 401 auth_missing
    let resp = app.send(request(Method::GET, "/api/v1/companies", None, None)).await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "auth_missing").await;

    // non-bearer scheme → 401 auth_missing
    let resp = app
        .send(request(Method::GET, "/api/v1/companies", Some("nope"), None))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "auth_missing").await;

    // garbage token → 401 auth_invalid
    let resp = app
        .send(request(Method::GET, "/api/v1/companies", Some(&"a".repeat(64)), None))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "auth_invalid").await;
}
