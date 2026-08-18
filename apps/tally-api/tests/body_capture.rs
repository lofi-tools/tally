//! Request-body capture integration tests (src/lib.rs `request_id_layer`):
//! text-ish request bodies ride the access-log line (and the trace's
//! `http.request.body` span attribute), while `/api/v1/auth/*` bodies —
//! which carry plaintext passwords — are redacted.
//! Gated behind `pg-tests` (on by default); the harness auto-starts the
//! `test-api-db` container and fails hard when the database can't be reached.
#![cfg(feature = "pg-tests")]


use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use tally_tests_common::{request, Capture, unique_email, TestApp};
use serde_json::json;

#[tokio::test]
async fn json_request_body_captured_in_access_log() {
    let app = TestApp::setup().await;

    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let guard = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_writer(Capture(captured.clone()))
            .with_max_level(tracing::Level::INFO)
            .without_time()
            .with_ansi(false)
            .finish(),
    );

    // register is itself a body-bearing request (redacted — asserted below).
    let token = app.register(&unique_email("capture")).await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "Body Capture Co" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "create company");

    drop(guard);

    let lines = captured.lock().unwrap().clone();
    let joined = lines.join("\n");
    assert!(
        lines.iter().any(|l| {
            l.contains("http request")
                && l.contains("method=POST")
                && l.contains("/api/v1/companies")
                && l.contains("body=")
                && l.contains("Body Capture Co")
        }),
        "expected the request body on the access-log line, got:\n{joined}"
    );
    // Redaction: the register request hit /api/v1/auth/* — its body (which
    // contains the password) must not appear anywhere.
    assert!(
        !lines.iter().any(|l| l.contains("correct horse battery staple")),
        "auth bodies must be redacted, got:\n{joined}"
    );
}

#[tokio::test]
async fn auth_request_bodies_are_redacted() {
    let app = TestApp::setup().await;

    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let guard = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_writer(Capture(captured.clone()))
            .with_max_level(tracing::Level::INFO)
            .without_time()
            .with_ansi(false)
            .finish(),
    );

    let email = unique_email("redact");
    app.register(&email).await;
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(&json!({
                "email": email,
                "password": "totally-wrong-password-xyz",
            })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "wrong password");

    drop(guard);

    let lines = captured.lock().unwrap().clone();
    let joined = lines.join("\n");
    assert!(
        !lines.iter().any(|l| l.contains("totally-wrong-password-xyz")),
        "login bodies must be redacted, got:\n{joined}"
    );
    // The access-log line itself still fires for the login request — just
    // without a body field.
    assert!(
        lines.iter().any(|l| {
            l.contains("http request")
                && l.contains("method=POST")
                && l.contains("/api/v1/auth/login")
        }),
        "expected an access-log line for the login request, got:\n{joined}"
    );
}
