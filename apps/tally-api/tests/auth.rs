//! Auth flow integration tests (spec §10).  Gated behind `pg-tests` (on by
//! default); skipped gracefully when Postgres is unreachable.
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::{Method, StatusCode};
use common::{assert_error, json_body, request, TestApp};
use tally_api::models::{Session, User};

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

    // non-bearer scheme (e.g. Basic) → 401 auth_missing
    let resp = app
        .send(
            axum::http::Request::builder()
                .method(Method::GET)
                .uri("/api/v1/companies")
                .header(axum::http::header::AUTHORIZATION, "Basic abc")
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "auth_missing").await;

    // garbage token → 401 auth_invalid
    let resp = app
        .send(request(Method::GET, "/api/v1/companies", Some(&"a".repeat(64)), None))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "auth_invalid").await;
}

#[tokio::test]
async fn expired_session_returns_auth_expired() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };

    let token = app.register("expiring@example.com").await;

    // sanity: the fresh token works
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Backdate the session.  tokio::time::pause/advance can't do this: the
    // expiry check compares the stored `expires_at` against
    // `chrono::Utc::now()` (wall clock), which tokio's virtual clock does
    // not affect.  Instead, rewrite the row directly — same token_hash, past
    // `expires_at`.
    let mut db = app.db.clone();
    let token_hash = tally_api::auth::sha256_hex(token.as_bytes());
    Session::filter_by_token_hash(&token_hash)
        .delete()
        .exec(&mut db)
        .await
        .expect("delete current session");
    let user = User::filter_by_email("expiring@example.com")
        .first()
        .exec(&mut db)
        .await
        .expect("lookup user")
        .expect("user exists");
    toasty::create!(Session {
        user_id: user.id,
        token_hash,
        created_at: "2000-01-01T00:00:00Z".to_string(),
        expires_at: "2000-01-01T00:00:00Z".to_string(),
    })
    .exec(&mut db)
    .await
    .expect("recreate expired session");

    // Same bearer token now reports auth_expired (not auth_invalid — the
    // token hash is still valid, only the expiry differs).
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&token), None))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "auth_expired").await;

    // Expiry is per-session: a fresh login is unaffected.
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(&serde_json::json!({
                "email": "expiring@example.com",
                "password": "correct horse battery staple",
            })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
}
