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
pub mod otel;
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
// trace layer: it generates the `x-request-id`, roots the fastrace trace
// with it (trace id == request id, so exported spans correlate with logs),
// and emits the access-log line. tower-http's own on_response event is
// silenced (the access log lives in `request_id_layer`, which has the full
// method/uri/request_id/status/latency).

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use fastrace::future::FutureExt as _;
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
/// responses echo it), reflects it as the `X-Request-Id` header on both the
/// request and the response, roots the fastrace trace for the request (trace
/// id == the request id, so exported spans correlate with the logs via
/// `FastraceDiagnostic`), and emits one access-log line per request
/// (method/uri/request_id/status/latency; 5xx at ERROR).
async fn request_id_layer(request: Request, next: Next) -> Response {
    let id = uuid::Uuid::new_v4();
    let id_str = id.to_string();
    let value = HeaderValue::from_str(&id_str).expect("uuid is a valid header value");
    let mut request = request;
    request.headers_mut().insert("x-request-id", value.clone());

    // fastrace root: trace id == x-request-id (the uuid bytes). The tracing
    // spans/events for this request (tower-http's `request` span, handlers,
    // db_log errors) become its children via the FastraceCompatLayer; the
    // local parent also feeds logforth's FastraceDiagnostic. `in_span`
    // re-establishes the local parent on every poll, so it survives tokio
    // moving the future between threads (and keeps the future Send — the
    // LocalParentGuard itself is !Send).
    let root = fastrace::Span::root(
        "http_request",
        fastrace::collector::SpanContext::new(
            fastrace::collector::TraceId(id.as_u128()),
            fastrace::collector::SpanId(0),
        ),
    );

    let method = request.method().clone();
    let uri = request.uri().clone();

    let mut response = REQUEST_ID
        .scope(id_str.clone(), async {
            let started = std::time::Instant::now();
            // The access-log line is emitted *inside* the in_span'd future:
            // the fastrace local parent is set for the whole poll, so
            // logforth's FastraceDiagnostic stamps trace_id on it (the trace
            // id == x-request-id, so logs correlate with Traceway). The
            // LocalParentGuard never crosses an await, keeping the future
            // Send.
            async {
                let response = next.run(request).await;
                let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
                let status = response.status().as_u16();
                if response.status().is_server_error() {
                    tracing::error!(
                        method = %method,
                        uri = %uri,
                        request_id = %id_str,
                        status,
                        latency_ms,
                        "http request failed"
                    );
                } else {
                    tracing::info!(
                        method = %method,
                        uri = %uri,
                        request_id = %id_str,
                        status,
                        latency_ms,
                        "http request"
                    );
                }
                response
            }
            .in_span(root)
            .await
        })
        .await;
    response.headers_mut().insert("x-request-id", value);
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
            // The `request` span feeds fastrace (via the compat layer): its
            // properties (method/uri/request_id) become the trace's root
            // span attributes. The access-log line itself is emitted by the
            // outer request_id_layer (it has the full field set, and 5xx at
            // ERROR), so tower-http's own on_response event is dropped to
            // DEBUG (the stock default — invisible at the default
            // `tower_http=info` filter).
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
                .on_response(DefaultOnResponse::new().level(Level::DEBUG))
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
