//! Auth flow integration tests (spec §10).  Gated behind `pg-tests` (on by
//! default); skipped gracefully when Postgres is unreachable.
#![cfg(feature = "pg-tests")]


use axum::http::{Method, StatusCode};
use tally_tests_common::{assert_error, json_body, request, request_with_guest, TestApp};
use serde_json::json;
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

// ---------------------------------------------------------------------------
// Temporary (guest) users — temp-user spec §5
// ---------------------------------------------------------------------------

/// `POST /auth/guest` with an `X-Guest-Id` → `{ token, user }`.
async fn bootstrap_guest(app: &TestApp, guest_id: &str) -> (String, serde_json::Value) {
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
    let json = json_body(resp).await;
    let token = json["token"].as_str().expect("token").to_string();
    (token, json["user"].clone())
}

#[tokio::test]
async fn guest_bootstrap_is_idempotent_and_adopts_in_place() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let guest_id = "browse-1234";

    // Bootstrap twice with the same id → same user, fresh token, temp flag.
    let (token1, user1) = bootstrap_guest(&app, guest_id).await;
    let (token2, user2) = bootstrap_guest(&app, guest_id).await;
    assert_eq!(user1["id"], user2["id"], "same guest id → same temp user");
    assert_ne!(token1, token2, "each bootstrap issues a fresh session");
    assert_eq!(user1["is_temporary"], true);
    assert_eq!(user1["guest_id"], guest_id);
    assert_eq!(user1["display_name"], "Guest");
    let placeholder = user1["email"].as_str().unwrap().to_string();
    assert!(placeholder.starts_with("temp+") && placeholder.ends_with("@local"));

    // The guest token is a real session: /auth/me resolves it.
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&token2), None))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["is_temporary"], true);

    // The temp user cannot log in (placeholder email + dummy hash).
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(&json!({ "email": placeholder, "password": "whatever123" })),
        ))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "invalid_credentials").await;

    // Register with the guest header → in-place upgrade: same user id, temp
    // flag cleared, guest id dropped, old guest sessions revoked.
    let user_id = user1["id"].as_str().unwrap().to_string();
    let resp = app
        .send(request_with_guest(
            Method::POST,
            "/api/v1/auth/register",
            Some(guest_id),
            None,
            Some(&json!({
                "display_name": "Ada Guest",
                "email": "ada-guest@example.com",
                "password": "hunter2hunter2",
            })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let adopted = json_body(resp).await;
    assert_eq!(adopted["user"]["id"], user_id, "in-place upgrade keeps the row id");
    assert_eq!(adopted["user"]["is_temporary"], false);
    assert!(adopted["user"]["guest_id"].is_null(), "guest_id cleared on adoption");
    assert_eq!(adopted["user"]["email"], "ada-guest@example.com");

    // The pre-adoption guest session is revoked.
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&token1), None))
        .await;
    assert_error(resp, StatusCode::UNAUTHORIZED, "auth_invalid").await;

    // The adopted row's guest id is cleared — the workspace is now the
    // account's; a fresh bootstrap creates a brand-new temp user below.

    // The new account logs in with its own credentials.
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(&json!({ "email": "ada-guest@example.com", "password": "hunter2hunter2" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["user"]["id"], user_id);

    // The same guest id after adoption: §5.2 cleared it from the row, so a
    // re-bootstrap starts a *fresh* temp workspace (the old workspace was
    // adopted — the browser now holds a real session instead).
    let (_, user3) = bootstrap_guest(&app, guest_id).await;
    assert_ne!(user3["id"].as_str().unwrap(), user_id, "new temp workspace");
    assert_eq!(user3["is_temporary"], true);
}

#[tokio::test]
async fn guest_bootstrap_rejects_an_adopted_row_that_still_has_a_guest_id() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let guest_id = "corrupt-guest";
    let (_, user_id) = bootstrap_guest(&app, guest_id).await;

    // The adoption path clears `is_temporary` and `guest_id` together, so a
    // non-temp row that still carries a guest id is a corrupted state — the
    // §5.1 guard returns 400 rather than issuing a session for it.
    let mut db = app.db.clone();
    toasty::sql::statement("UPDATE \"users\" SET \"is_temporary\" = FALSE WHERE \"id\" = $1")
        .bind(user_id["id"].as_str().unwrap().parse::<uuid::Uuid>().unwrap())
        .exec(&mut db)
        .await
        .expect("corrupt row");

    let resp = app
        .send(request_with_guest(
            Method::POST,
            "/api/v1/auth/guest",
            Some(guest_id),
            None,
            None,
        ))
        .await;
    assert_error(resp, StatusCode::BAD_REQUEST, "guest_already_adopted").await;
}

#[tokio::test]
async fn guest_bootstrap_requires_the_header() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };

    let resp = app
        .send(request(Method::POST, "/api/v1/auth/guest", None, None))
        .await;
    assert_error(resp, StatusCode::BAD_REQUEST, "guest_id_required").await;

    // Blank header → same 400.
    let resp = app
        .send(request_with_guest(
            Method::POST,
            "/api/v1/auth/guest",
            Some("   "),
            None,
            None,
        ))
        .await;
    assert_error(resp, StatusCode::BAD_REQUEST, "guest_id_required").await;
}

#[tokio::test]
async fn guest_register_with_taken_email_rejects_without_merging() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };

    // A real account already owns the email.
    app.register("taken@example.com").await;

    // A guest workspace exists.
    let (token, user) = bootstrap_guest(&app, "guest-with-taken-email").await;
    let user_id = user["id"].as_str().unwrap().to_string();
    let guest_id = "guest-with-taken-email";

    // Register with the guest header + the taken email → email_taken (no
    // merge into the existing account).
    let resp = app
        .send(request_with_guest(
            Method::POST,
            "/api/v1/auth/register",
            Some(guest_id),
            None,
            Some(&json!({ "display_name": "Wants Merge", "email": "taken@example.com", "password": "hunter2hunter2" })),
        ))
        .await;
    assert_error(resp, StatusCode::CONFLICT, "email_taken").await;

    // The guest workspace is untouched: the temp user is still there and
    // the guest session still works.
    let resp = app
        .send(request(Method::GET, "/api/v1/auth/me", Some(&token), None))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["id"], user_id);

    let mut db = app.db.clone();
    let user = User::filter_by_id(user_id.parse::<uuid::Uuid>().unwrap())
        .first()
        .exec(&mut db)
        .await
        .expect("load user")
        .expect("temp user still exists");
    assert!(user.is_temporary, "still a temp user after rejected adoption");
}

#[tokio::test]
async fn guest_register_without_header_is_plain_register() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };

    // A guest id exists but the register call omits the header → a brand
    // new real user, unrelated to the guest workspace.
    bootstrap_guest(&app, "ghost-header").await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/register",
            None,
            Some(&json!({ "display_name": "Plain", "email": "plain@example.com", "password": "hunter2hunter2" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let registered = json_body(resp).await;
    assert_eq!(registered["user"]["is_temporary"], false);

    let mut db = app.db.clone();
    let guest = User::filter_by_guest_id("ghost-header")
        .first()
        .exec(&mut db)
        .await
        .expect("load user")
        .expect("guest still exists");
    assert!(guest.is_temporary, "guest workspace not adopted");
}

#[tokio::test]
async fn guest_can_use_company_endpoints() {
    let Some(app) = TestApp::setup().await else {
        eprintln!("skipping: no Postgres at DATABASE_URL");
        return;
    };
    let (token, _) = bootstrap_guest(&app, "guest-workspace").await;

    // A guest's add goes through the same POST /companies path as a
    // signed-in user's (one auth path — temp-user spec §5.5).
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "Guest Co Ltd", "accounting_standard": "FRS 102" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let company = json_body(resp).await;
    assert_eq!(company["accounting_standard"], "FRS 102");

    let resp = app
        .send(request(Method::GET, "/api/v1/companies", Some(&token), None))
        .await;
    let list = json_body(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}
