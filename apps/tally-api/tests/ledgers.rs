//! Ledger upload + JSON views integration tests (spec §9).  Gated behind
//! `pg-tests` (on by default); the harness auto-starts the `test-api-db`
//! container and fails hard when the database can't be reached.
#![cfg(feature = "pg-tests")]


use axum::http::{Method, StatusCode};
use tally_tests_common::{assert_error, json_body, multipart_body, request, unique_email, TestApp, FIXTURE_GNUCASH};
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
    let app = TestApp::setup().await;
    let (token, company_id) = seed_company(&app, &unique_email("ledger")).await;

    let bytes = std::fs::read(&*FIXTURE_GNUCASH).expect("fixture exists");
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
    // §15: every node carries its GnuCash guid (for split→name resolution).
    let first_account = &accounts["accounts"][0];
    assert!(first_account["guid"].as_str().unwrap_or("").len() >= 8);

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
    let items = page["items"].as_array().unwrap();
    assert!(items.len() > 0);
    assert_eq!(page["limit"], 50);
    assert_eq!(page["offset"], 0);
    // §15: the GnuCash description survives ingest and is returned.
    assert!(items[0]["description"].is_string());
    assert!(items[0]["splits"].as_array().unwrap().len() >= 2);

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
    let app = TestApp::setup().await;
    let (token, company_id) = seed_company(&app, &unique_email("ext")).await;

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
    let app = TestApp::setup().await;
    let (alice, alice_company) = seed_company(&app, &unique_email("alice")).await;
    let bob = app.register_second(&unique_email("bob")).await;
    let _ = alice_company;

    let bytes = std::fs::read(&*FIXTURE_GNUCASH).expect("fixture exists");
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
    let app = TestApp::setup().await;
    let (token, company_id) = seed_company(&app, &unique_email("nofile")).await;

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

#[tokio::test]
async fn upload_garbage_gnucash_is_rejected() {
    let app = TestApp::setup().await;
    let (token, company_id) = seed_company(&app, &unique_email("garbage")).await;

    // Valid extension, garbage content: the parser must fail with the 422
    // `ledger_parse_failed` envelope, not 500.  (This relies on rucash
    // rejecting the malformed XML — a lenient parser would instead yield an
    // empty book and a 200.)
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
                .body(multipart_body(boundary, "bad.gnucash", b"<gnc-v2>not a real gnucash book"))
                .expect("request"),
        )
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "ledger_parse_failed").await;
}

#[tokio::test]
async fn upload_too_large_is_rejected() {
    // A tiny cap so a 1 KiB body trips the streaming size check mid-upload.
    let app = TestApp::setup_with_max_upload_bytes(256).await;
    let (token, company_id) = seed_company(&app, &unique_email("big")).await;

    let boundary = "----tally-test-boundary";
    let payload = vec![b'x'; 1024];
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
                .body(multipart_body(boundary, "big.gnucash", &payload))
                .expect("request"),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let json = json_body(resp).await;
    assert_eq!(json["error"]["code"], "file_too_large");
    assert_eq!(json["error"]["details"]["limit_bytes"], 256);
}

#[tokio::test]
async fn upload_malformed_multipart_is_rejected() {
    let app = TestApp::setup().await;
    let (token, company_id) = seed_company(&app, &unique_email("mime")).await;

    // Declares the boundary and a `file` field but never closes it: the
    // stream ends mid-field, so multer errors while reading chunks → 400
    // `multipart_error` (not a hang, not a parse failure).
    let boundary = "----tally-test-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"x.gnucash\"\r\n\r\nsome bytes but no closing boundary"
    );
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
    assert_error(resp, StatusCode::BAD_REQUEST, "multipart_error").await;
}

#[tokio::test]
async fn ledger_route_rejects_invalid_uuid() {
    let app = TestApp::setup().await;
    let token = app.register(&unique_email("badpath")).await;

    // A non-UUID path segment fails the `Path<Uuid>` extractor before the
    // handler runs → field-level 422 `validation_failed`.
    let resp = app
        .send(request(Method::GET, "/api/v1/ledgers/not-a-uuid", Some(&token), None))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;
}

#[tokio::test]
async fn transactions_view_rejects_bad_query() {
    let app = TestApp::setup().await;
    let token = app.register(&unique_email("badquery")).await;

    // The `Query` extractor rejects before the handler runs, so the ledger
    // id need not exist; `limit=abc` fails u32 deserialization → 422.
    let resp = app
        .send(request(
            Method::GET,
            "/api/v1/ledgers/00000000-0000-0000-0000-000000000000/transactions?limit=abc",
            Some(&token),
            None,
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;
}
