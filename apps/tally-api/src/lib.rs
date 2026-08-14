//! `tally-api`: the Tally web API (spec: `docs/spec/api-backend-spec.md`).
//!
//! The library target exists so the pg-gated integration tests (`tests/`)
//! can build a real app against a Postgres; the binary (`main.rs`) is a thin
//! env/tracing/bind wrapper around [`router`].

pub mod auth;
pub mod companies;
pub mod companies_house;
pub mod db_log;
pub mod error;
pub mod extract;
pub mod filings;
pub mod jobs;
pub mod ledgers;
pub mod migrations;
pub mod models;
pub mod period;
pub mod reports;
pub mod sweep;

// Router assembly and shared state (spec §4).
//
// [`AppState`] carries the toasty `Db` (cheap to clone; each handler clones
// its own handle), the optional Companies House handle, and the upload
// configuration.
//
// Middleware stack (outermost → innermost): CORS, request-id scoping,
// trace, catch-panic, body limit. Request-id scoping sits *outside* the
// trace layer so the access-log span can carry the `x-request-id` (the
// seed for future trace correlation).

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::companies_house::ChApi;
use crate::error::{plain_error, REQUEST_ID};

/// Shared handler state.
pub struct AppState {
    /// Cheap to clone — handlers clone a handle per request rather than
    /// holding a lock across a read-then-write flow.
    pub db: toasty::Db,
    pub ch: Option<ChApi>,
    /// Where uploaded ledger files are stored.
    pub upload_dir: PathBuf,
    /// Per-upload size cap in bytes (the multipart handler rejects beyond
    /// this; the router's body limit sits slightly above it to let the
    /// handler produce the proper 413 envelope).
    pub max_upload_bytes: u64,
}

/// `GET /health` — liveness (no auth).
async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Route fallback: the §11.3 `route_not_found` envelope.
async fn not_found() -> Response {
    plain_error(StatusCode::NOT_FOUND, "route_not_found", "route not found")
}

/// Method-not-allowed fallback: the §11.3 `method_not_allowed` envelope.
async fn method_not_allowed() -> Response {
    plain_error(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed", "method not allowed")
}

/// Scopes a fresh request id into the [`REQUEST_ID`] task-local (5xx
/// responses echo it) and reflects it as the `X-Request-Id` header — on the
/// request as well as the response, so the outer access-log span can carry
/// it (the seed for future trace correlation).
async fn request_id_layer(request: Request, next: Next) -> Response {
    let id = uuid::Uuid::new_v4().to_string();
    let value = HeaderValue::from_str(&id).expect("uuid is a valid header value");
    let request = {
        let mut request = request;
        request.headers_mut().insert("x-request-id", value.clone());
        request
    };
    let mut response = REQUEST_ID
        .scope(id.clone(), async { next.run(request).await })
        .await;
    response
        .headers_mut()
        .insert("x-request-id", value);
    response
}

/// The full router: `/health` + `/api/v1/*` (spec §6).
pub fn router(state: Arc<AppState>) -> Router {
    let body_limit = state.max_upload_bytes.saturating_add(1024 * 1024);

    Router::new()
        .route("/health", get(health))
        .nest("/api/v1", api_v1())
        // Fallbacks must be registered *before* the layers: axum only wraps a
        // fallback with the layers applied after it is set, so a fallback set
        // here would otherwise bypass trace/CORS/request-id/panic handling
        // entirely (404s and 405s went untraced before this fix).
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(body_limit as usize))
        .layer(CatchPanicLayer::new())
        .layer(
            // Request access logs (level discipline: one INFO line per
            // request with status + latency; 5xx at ERROR). The span carries
            // the x-request-id set by the outer request_id_layer, so the
            // access log line correlates with the response header and the
            // 5xx error envelope — ready for OTel trace ids later.
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request| {
                    tracing::info_span!(
                        "request",
                        method = %request.method(),
                        uri = %request.uri(),
                        request_id = %request
                            .headers()
                            .get("x-request-id")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or(""),
                    )
                })
                .on_response(DefaultOnResponse::new().level(Level::INFO))
                .on_failure(DefaultOnFailure::new().level(Level::ERROR)),
        )
        .layer(middleware::from_fn(request_id_layer))
        .layer(CorsLayer::permissive())
}

/// The `/api/v1` routes (spec §6).
fn api_v1() -> Router<Arc<AppState>> {
    Router::new()
        // auth
        .route("/auth/register", post(auth::register))
        .route("/auth/guest", post(auth::guest))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        // companies
        .route("/companies", get(companies::list).post(companies::create))
        .route("/companies/search", get(companies::search))
        .route(
            "/companies/{id}",
            get(companies::get).patch(companies::patch).delete(companies::delete),
        )
        .route("/companies/{id}/enrich", post(companies::enrich))
        // filings
        .route("/companies/{id}/filings", get(filings::list))
        .route("/companies/{id}/filings/refresh", post(filings::refresh))
        // ledgers
        .route(
            "/companies/{id}/ledgers",
            get(ledgers::list).post(ledgers::upload),
        )
        .route("/ledgers/{id}", get(ledgers::get).delete(ledgers::delete))
        .route("/ledgers/{id}/accounts", get(ledgers::accounts_view))
        .route("/ledgers/{id}/transactions", get(ledgers::transactions_view))
        // reports
        .route("/companies/{id}/reports/accounts", post(reports::accounts))
        .route("/companies/{id}/reports/corp-tax", post(reports::corp_tax))
        .route("/companies/{id}/reports/corp-tax.json", post(reports::corp_tax_json))
        .route("/companies/{id}/reports/ct600", post(reports::ct600))
}
