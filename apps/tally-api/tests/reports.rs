//! Report generation integration tests (spec §6).  Gated behind `pg-tests`
//! (on by default); the harness auto-starts the `test-postgres` container
//! and fails hard when the database can't be reached.
//!
//! Reports need a company with the required dates (`report_date`,
//! `authorised_date`). The period resolves from the company's registration
//! date when no explicit `period` is given (the tests never configure a CH
//! key, so they exercise the registration-date branch).
#![cfg(feature = "pg-tests")]


use axum::http::{Method, StatusCode};
use tally_tests_common::{assert_error, json_body, multipart_body, request, unique_email, TestApp, FIXTURE_GNUCASH};
use serde_json::json;

/// Seed a report-ready company (with dates) + an uploaded ledger.
/// Returns (token, company_id, ledger_id).
async fn seed(app: &TestApp, email: &str) -> (String, String, String) {
    seed_with(app, email, &json!({})).await
}

/// Like [`seed`], with extra fields merged over the defaults (e.g. to blank
/// out the UTR or registration date for the gate tests).
async fn seed_with(
    app: &TestApp,
    email: &str,
    extra: &serde_json::Value,
) -> (String, String, String) {
    let token = app.register(email).await;
    let mut company = json!({
        "name": "Report Co Ltd",
        "company_number": "01234567",
        "tax_reference": "1234567890",
        "report_date": "2020-05-30",
        "authorised_date": "2020-06-01",
        "incorporation_date": "2015-03-01",
        "registration_date": "2015-03-01",
        "signed_by": "Ada Lovelace",
        "directors": ["Ada Lovelace"],
    });
    if let Some(extra_obj) = extra.as_object()
        && let Some(obj) = company.as_object_mut()
    {
        for (k, v) in extra_obj {
            obj.insert(k.clone(), v.clone());
        }
    }
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&company),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "seed company");
    let company_id = json_body(resp).await["id"].as_str().unwrap().to_string();

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
    assert_eq!(resp.status(), StatusCode::OK, "seed ledger");
    let ledger_id = json_body(resp).await["id"].as_str().unwrap().to_string();
    (token, company_id, ledger_id)
}

/// The report request body shared by the four endpoints.
fn report_body(ledger_id: &str) -> serde_json::Value {
    json!({
        "ledger_id": ledger_id,
        "period": { "start": "2019-04-01", "end": "2020-03-31" },
    })
}

#[tokio::test]
async fn accounts_report_renders_html() {
    let app = TestApp::setup().await;
    let (token, company_id, ledger_id) = seed(&app, &unique_email("acc")).await;

    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/accounts"),
            Some(&token),
            Some(&report_body(&ledger_id)),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024).await.unwrap();
    let html = String::from_utf8(body.to_vec()).expect("html");
    assert!(html.contains("<html") || html.contains("<!DOCTYPE") || html.contains("ixbrl"), "html output");
}

#[tokio::test]
async fn corp_tax_report_and_json() {
    let app = TestApp::setup().await;
    let (token, company_id, ledger_id) = seed(&app, &unique_email("tax")).await;

    // html
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/corp-tax"),
            Some(&token),
            Some(&report_body(&ledger_id)),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024).await.unwrap();
    let html = String::from_utf8(body.to_vec()).expect("html");
    assert!(html.contains("ixbrl") || html.contains("<html"), "corp-tax html output");

    // json
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/corp-tax.json"),
            Some(&token),
            Some(&report_body(&ledger_id)),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json = json_body(resp).await;
    assert_eq!(json["company_name"], "Report Co Ltd");
    assert!(json["corporation_tax_chargeable"].is_number() || json["corporation_tax_chargeable"].is_null());
}

#[tokio::test]
async fn ct600_renders_govtalk_xml() {
    let app = TestApp::setup().await;
    let (token, company_id, ledger_id) = seed(&app, &unique_email("ct600")).await;

    let body = json!({
        "ledger_id": ledger_id,
        "period": { "start": "2019-04-01", "end": "2020-03-31" },
        "declaration": { "name": "Ada Lovelace", "status": "Director" },
    });
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/ct600"),
            Some(&token),
            Some(&body),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(bytes.to_vec()).expect("xml");
    assert!(xml.contains("GovTalkMessage"), "govtalk envelope");
}

#[tokio::test]
async fn report_requires_dates_on_the_company() {
    let app = TestApp::setup().await;
    let token = app.register(&unique_email("nodate")).await;
    // No report_date / authorised_date.
    let resp = app
        .send(request(
            Method::POST,
            "/api/v1/companies",
            Some(&token),
            Some(&json!({ "name": "No Dates Ltd" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let company_id = json_body(resp).await["id"].as_str().unwrap().to_string();

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
    let ledger_id = json_body(resp).await["id"].as_str().unwrap().to_string();

    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/accounts"),
            Some(&token),
            Some(&report_body(&ledger_id)),
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;
}

#[tokio::test]
async fn report_without_period_resolves_from_registration_date() {
    let app = TestApp::setup().await;
    let (token, company_id, ledger_id) = seed(&app, &unique_email("noperiod")).await;

    // No explicit period and no CH key → the period is guessed from the
    // stored registration date (was companies_house_key_missing before).
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/accounts"),
            Some(&token),
            Some(&json!({ "ledger_id": ledger_id })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "period from registration date");
}

#[tokio::test]
async fn report_without_registration_date_needs_an_explicit_period() {
    let app = TestApp::setup().await;
    let (token, company_id, ledger_id) =
        seed_with(&app, &unique_email("noregdate"), &json!({ "registration_date": null })).await;

    // No explicit period, no CH key, and no registration date → a
    // validation error telling the caller to set it (or pass a period).
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/accounts"),
            Some(&token),
            Some(&json!({ "ledger_id": ledger_id })),
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;
}

#[tokio::test]
async fn corp_tax_and_ct600_require_utr() {
    let app = TestApp::setup().await;
    let (token, company_id, ledger_id) =
        seed_with(&app, &unique_email("noutr"), &json!({ "tax_reference": null })).await;

    // corp-tax.json → 422 validation_failed (UTR missing).
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/corp-tax.json"),
            Some(&token),
            Some(&report_body(&ledger_id)),
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;

    // ct600 → 422 too.
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/ct600"),
            Some(&token),
            Some(&report_body(&ledger_id)),
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;

    // The accounts report carries no UTR → not gated.
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/accounts"),
            Some(&token),
            Some(&report_body(&ledger_id)),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "accounts not gated on UTR");

    // Setting a valid UTR unblocks corp-tax.
    let resp = app
        .send(request(
            Method::PATCH,
            &format!("/api/v1/companies/{company_id}"),
            Some(&token),
            Some(&json!({ "tax_reference": "0987654321" })),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "patch UTR");
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/corp-tax.json"),
            Some(&token),
            Some(&report_body(&ledger_id)),
        ))
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "corp-tax after UTR set");
}

#[tokio::test]
async fn corp_tax_rejects_malformed_utr() {
    let app = TestApp::setup().await;
    // A short, non-10-digit reference.
    let (token, company_id, ledger_id) =
        seed_with(&app, &unique_email("badutr"), &json!({ "tax_reference": "12345" })).await;

    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_id}/reports/corp-tax.json"),
            Some(&token),
            Some(&report_body(&ledger_id)),
        ))
        .await;
    assert_error(resp, StatusCode::UNPROCESSABLE_ENTITY, "validation_failed").await;
}

#[tokio::test]
async fn report_with_foreign_ledger_is_rejected() {
    let app = TestApp::setup().await;
    // Two companies, one ledger each; use company A's ledger on company B.
    let (token_a, _company_a, ledger_a) = seed(&app, &unique_email("a")).await;
    let (_token_b, company_b, _ledger_b) = {
        let token_b = app.register_second(&unique_email("b")).await;
        let resp = app
            .send(request(
                Method::POST,
                "/api/v1/companies",
                Some(&token_b),
                Some(&json!({
                    "name": "B Ltd",
                    "report_date": "2020-05-30",
                    "authorised_date": "2020-06-01",
                })),
            ))
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let company_b = json_body(resp).await["id"].as_str().unwrap().to_string();
        let bytes = std::fs::read(&*FIXTURE_GNUCASH).expect("fixture exists");
        let boundary = "----tally-test-boundary";
        let resp = app
            .send(
                axum::http::Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/companies/{company_b}/ledgers"))
                    .header(axum::http::header::AUTHORIZATION, format!("Bearer {token_b}"))
                    .header(
                        axum::http::header::CONTENT_TYPE,
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(multipart_body(boundary, "input.gnucash", &bytes))
                    .expect("request"),
            )
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let ledger_b = json_body(resp).await["id"].as_str().unwrap().to_string();
        (token_b, company_b, ledger_b)
    };

    // ledger_a on company_b (different owners entirely) → 404 via ownership.
    let resp = app
        .send(request(
            Method::POST,
            &format!("/api/v1/companies/{company_b}/reports/accounts"),
            Some(&token_a),
            Some(&report_body(&ledger_a)),
        ))
        .await;
    assert_error(resp, StatusCode::NOT_FOUND, "not_found").await;
}
