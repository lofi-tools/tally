//! `tally-api` binary (spec §4, §13): env config → tracing → DB connect +
//! migrations → serve.  The router and handlers live in the lib target.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tally_api::{router, AppState};
use tally_api::companies_house::ChApi;
use tally_api::models::{Account, Company, Ledger, Session, Split, Transaction, User};
use toasty::db::Connect;

/// Default bind (spec §13: LTS owns 8081, so the API defaults to 8080).
const DEFAULT_ADDR: &str = "127.0.0.1:8080";
/// Default DB URL matching `docker-compose.yml`.
const DEFAULT_DB_URL: &str = "postgres://tally:tally@localhost:5432/tally";
/// Default per-upload cap: 50 MB (spec §9).
const DEFAULT_MAX_UPLOAD_BYTES: u64 = 50 * 1024 * 1024;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tally_api=info,tower_http=info".into()),
        )
        .init();

    let db_url = env_or("DATABASE_URL", DEFAULT_DB_URL);
    let addr = env_or("TALLY_API_ADDR", &env_or("PORT", DEFAULT_ADDR));
    let upload_dir = PathBuf::from(env_or("TALLY_API_UPLOAD_DIR", ".cache/tally-api/uploads"));
    let max_upload_bytes = std::env::var("TALLY_API_MAX_UPLOAD_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_UPLOAD_BYTES);

    // Upload dir must exist before the first upload.
    std::fs::create_dir_all(&upload_dir)
        .with_context(|| format!("create upload dir '{}'", upload_dir.display()))?;

    // --- database -----------------------------------------------------------
    let connect = Connect::new(&db_url)
        .await
        .with_context(|| format!("parse DATABASE_URL '{db_url}'"))?;
    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(User, Session, Company, Ledger, Account, Transaction, Split));
    let mut db = builder
        .build(connect)
        .await
        .with_context(|| format!("connect to '{db_url}'"))?;
    // Idempotent: plays only the committed SQL migrations that are missing
    // (see src/migrations.rs), so restarting against an existing schema is
    // safe — unlike the previous startup `push_schema()`.
    tally_api::migrations::apply_pending(&mut db)
        .await
        .context("apply schema migrations")?;
    tracing::info!(db_url = %db_url, "connected to postgres; migrations applied");

    // --- state + serve --------------------------------------------------------
    let state = Arc::new(AppState {
        db,
        ch: ChApi::from_env(),
        upload_dir,
        max_upload_bytes,
    });

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, "tally-api listening");
    axum::serve(listener, router(state)).await.context("serve")?;
    Ok(())
}

/// Env var or default (empty counts as unset).
fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}
