//! DB logging integration tests (src/db_log.rs + the toasty driver's
//! per-query `toasty::query` event):
//!
//! - the driver wrapper must log a failed request at ERROR with the whole
//!   evaluated request (SQL + bound params), while successful requests stay
//!   silent at that level;
//! - the stock driver's per-query event (target `toasty::query`) carries the
//!   full evaluated request — statement + params — when the Db is built with
//!   `log_statement_params(true)` (as `main.rs` does), and omits params by
//!   default.
//! Gated behind `pg-tests` (on by default); an unreachable database is a
//! hard failure, not a skip (the harness auto-starts the test Postgres).
#![cfg(feature = "pg-tests")]


use std::sync::{Arc, Mutex};

use tally_tests_common::{Capture, ensure_test_db, reinit_if_needed, test_db_url};
use tally_api::db_log::LoggingDriver;
use tally_api::models::{Account, BalanceSheet, Company, Filing, Job, Ledger, Session, Split, Transaction, User};

/// A `Db` on the shared test database, built on the `LoggingDriver` wrapper
/// (which logs every request). `log_params` mirrors production's
/// `log_statement_params(true)`.  An unavailable database is a hard failure
/// (panic), not a skip.
async fn setup_with_logging_driver(log_params: bool) -> toasty::Db {
    let db_url = test_db_url();
    ensure_test_db(&db_url).await;
    reinit_if_needed(&db_url).await;

    let driver = LoggingDriver::new(&db_url).expect("the logging driver accepts the URL");
    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(
        User, Session, Company, Ledger, Account, Transaction, Split, Job, Filing, BalanceSheet
    ));
    if log_params {
        builder.log_statement_params(true);
    }
    builder
        .build(driver)
        .await
        .expect("build the toasty Db on the shared test database")
}

#[tokio::test]
async fn failed_request_logs_evaluated_request_at_error() {
    let mut db = setup_with_logging_driver(false).await;

    // Capture ERROR events only — a successful request must never appear.
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(Capture(captured.clone()))
        .with_max_level(tracing::Level::ERROR)
        .without_time()
        .with_ansi(false)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);

    // A success must not log at ERROR...
    let ok = toasty::sql::statement("SELECT 1").exec(&mut db).await;
    assert!(ok.is_ok(), "control query should succeed: {ok:?}");

    // ...while a failure must log the whole evaluated request (SQL + params).
    let err = toasty::sql::statement("SELECT * FROM tally_missing_table WHERE id = $1")
        .bind(42_i64)
        .exec(&mut db)
        .await;
    assert!(err.is_err(), "query against a missing table must fail");

    drop(guard);

    let lines = captured.lock().unwrap().clone();
    let joined = lines.join("\n");
    assert!(
        lines.iter().any(|l| l.contains("db request failed")),
        "expected a 'db request failed' error line, got:\n{joined}"
    );
    assert!(
        lines.iter().any(|l| l.contains("tally_missing_table")),
        "expected the evaluated SQL in the error line, got:\n{joined}"
    );
    assert!(
        lines.iter().any(|l| l.contains("42")),
        "expected the bound params in the error line, got:\n{joined}"
    );
    assert!(
        lines.iter().any(|l| l.contains("does not exist")),
        "expected the postgres error text in the line, got:\n{joined}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("SELECT 1")),
        "successful requests must stay silent at ERROR, got:\n{joined}"
    );

}

#[tokio::test]
async fn per_query_events_carry_statement_and_params_when_enabled() {
    let mut db = setup_with_logging_driver(true).await;

    // Capture DEBUG and above: the per-query `toasty::query` event fires at
    // DEBUG for a fast successful statement.
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(Capture(captured.clone()))
        .with_max_level(tracing::Level::DEBUG)
        .without_time()
        .with_ansi(false)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);

    let ok = toasty::sql::statement("SELECT $1::int AS n")
        .bind(42_i64)
        .exec(&mut db)
        .await;
    assert!(ok.is_ok(), "control query should succeed: {ok:?}");

    drop(guard);

    let lines = captured.lock().unwrap().clone();
    let joined = lines.join("\n");
    assert!(
        lines.iter().any(|l| l.contains("query executed") && l.contains("db.statement=")),
        "expected a per-query event with the statement, got:\n{joined}"
    );
    assert!(
        lines.iter().any(|l| l.contains("SELECT $1::int AS n")),
        "expected the evaluated SQL in the per-query event, got:\n{joined}"
    );
    assert!(
        lines.iter().any(|l| l.contains("db.params=[42]")),
        "expected the bound params in the per-query event, got:\n{joined}"
    );

}

#[tokio::test]
async fn per_query_events_omit_params_by_default() {
    let mut db = setup_with_logging_driver(false).await;

    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(Capture(captured.clone()))
        .with_max_level(tracing::Level::DEBUG)
        .without_time()
        .with_ansi(false)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);

    let ok = toasty::sql::statement("SELECT $1::int AS n")
        .bind(42_i64)
        .exec(&mut db)
        .await;
    assert!(ok.is_ok(), "control query should succeed: {ok:?}");

    drop(guard);

    let lines = captured.lock().unwrap().clone();
    let joined = lines.join("\n");
    assert!(
        lines.iter().any(|l| l.contains("query executed") && l.contains("db.statement=")),
        "expected a per-query event with the statement, got:\n{joined}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("db.params=")),
        "params must be omitted without log_statement_params(true), got:\n{joined}"
    );

}
