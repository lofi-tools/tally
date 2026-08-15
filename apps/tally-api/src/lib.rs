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
// wraps the request body in a recorder (so what was sent — up to a cap, and
// never on `/api/v1/auth/*` — lands on the trace as `http.request.body` and
// in the access-log line), and emits the access-log line. tower-http's own
// on_response event is silenced (the access log lives in
// `request_id_layer`, which has the full method/uri/request_id/status/
// latency/body).

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::extract::{MatchedPath, Request};
use axum::http::{header, HeaderValue, StatusCode};
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

/// Cap on the request-body bytes captured for observability. Bodies larger
/// than this are logged truncated (with a marker); multipart uploads are
/// never captured at all.
const REQUEST_BODY_CAPTURE_CAP: usize = 8 * 1024;

/// Whether to wrap this request's body in a recorder so the trace and access
/// log can show what was sent.
///
/// Redaction: `/api/v1/auth/*` bodies carry plaintext passwords — never
/// captured. Multipart uploads (ledger files) are skipped (large, binary).
/// Everything else text-ish is captured up to [`REQUEST_BODY_CAPTURE_CAP`].
fn should_capture_body(request: &Request) -> bool {
    if request.uri().path().starts_with("/api/v1/auth/") {
        return false;
    }
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if content_type.starts_with("multipart/") {
        return false;
    }
    // Text-ish bodies, plus untyped POST/PUT/PATCH bodies (almost always
    // JSON in this API). Empty bodies are a no-op for the recorder.
    content_type.starts_with("application/json")
        || content_type.starts_with("application/x-www-form-urlencoded")
        || content_type.starts_with("application/xml")
        || content_type.starts_with("text/")
        || (content_type.is_empty()
            && matches!(request.method().as_str(), "POST" | "PUT" | "PATCH"))
}

/// Accumulates request-body bytes (up to a cap) as the handler consumes the
/// body, so the trace and access log can include what was sent.
struct BodyRecorder {
    cap: usize,
    buf: std::sync::Mutex<Vec<u8>>,
    overflow: std::sync::atomic::AtomicBool,
}

impl BodyRecorder {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            buf: std::sync::Mutex::new(Vec::new()),
            overflow: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn push(&self, data: &[u8]) {
        let mut buf = self.buf.lock().expect("body recorder lock");
        let take = self.cap.saturating_sub(buf.len()).min(data.len());
        if take > 0 {
            buf.extend_from_slice(&data[..take]);
        }
        if take < data.len() {
            self.overflow
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// The captured body as lossy UTF-8 (with a truncation marker when the
    /// cap was hit), or `None` when nothing was captured.
    fn label(&self) -> Option<String> {
        let buf = self.buf.lock().expect("body recorder lock");
        let overflow = self.overflow.load(std::sync::atomic::Ordering::Relaxed);
        if buf.is_empty() && !overflow {
            return None;
        }
        let mut s = String::from_utf8_lossy(&buf).into_owned();
        if overflow {
            s.push_str("…(truncated)");
        }
        Some(s)
    }
}

/// A [`http_body::Body`] wrapper that copies each data frame into a
/// [`BodyRecorder`] as it streams through.
struct RecordingBody<B> {
    inner: B,
    recorder: Arc<BodyRecorder>,
}

impl<B> http_body::Body for RecordingBody<B>
where
    B: http_body::Body<Data = axum::body::Bytes>,
{
    type Data = axum::body::Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        // Manual projection: the pinned pointee is never moved, so
        // re-projecting `inner` as pinned and reading `recorder` by
        // reference are both sound.
        let this = unsafe { self.get_unchecked_mut() };
        match unsafe { Pin::new_unchecked(&mut this.inner) }.poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    this.recorder.push(data);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            poll => poll,
        }
    }
}

/// Emits the access-log line, including the captured request body only when
/// there was one (keeps GET and redacted requests clean). Called from within
/// the request's fastrace local parent, so the record carries the trace id.
fn emit_access_log(
    is_error: bool,
    method: &axum::http::Method,
    uri: &axum::http::Uri,
    request_id: &str,
    status: u16,
    latency_ms: f64,
    body: Option<&str>,
) {
    // The level must be a constant at the callsite (tracing builds a static
    // callsite per level), so branch on it explicitly.
    match (is_error, body) {
        (true, Some(body)) => tracing::error!(
            method = %method,
            uri = %uri,
            request_id = %request_id,
            status,
            latency_ms,
            body = %body,
            "http request failed"
        ),
        (true, None) => tracing::error!(
            method = %method,
            uri = %uri,
            request_id = %request_id,
            status,
            latency_ms,
            "http request failed"
        ),
        (false, Some(body)) => tracing::info!(
            method = %method,
            uri = %uri,
            request_id = %request_id,
            status,
            latency_ms,
            body = %body,
            "http request"
        ),
        (false, None) => tracing::info!(
            method = %method,
            uri = %uri,
            request_id = %request_id,
            status,
            latency_ms,
            "http request"
        ),
    }
}

/// Scopes a fresh request id into the [`REQUEST_ID`] task-local (5xx
/// responses echo it), reflects it as the `X-Request-Id` header on both the
/// request and the response, roots the fastrace trace for the request (trace
/// id == the request id, so exported spans correlate with the logs via
/// `FastraceDiagnostic`), wraps the body in a recorder (see
/// [`should_capture_body`]) so what was sent lands on the trace as
/// `http.request.body` and in the access-log line, and emits one access-log
/// line per request (method/uri/request_id/status/latency[/body]; 5xx at
/// ERROR).
async fn request_id_layer(request: Request, next: Next) -> Response {
    let id = uuid::Uuid::new_v4();
    let id_str = id.to_string();
    let value = HeaderValue::from_str(&id_str).expect("uuid is a valid header value");
    let mut request = request;
    request.headers_mut().insert("x-request-id", value.clone());

    // fastrace root: trace id == x-request-id (the uuid bytes). The tracing
    // spans/events for this request (tower-http's `request` span, handlers,
    // db_log errors, the driver's per-query `toasty::query` events) become
    // its children via the FastraceCompatLayer; the local parent also feeds
    // logforth's FastraceDiagnostic. `in_span` re-establishes the local
    // parent on every poll, so it survives tokio moving the future between
    // threads (and keeps the future Send — the LocalParentGuard itself is
    // !Send).
    //
    // `span.kind=server` makes fastrace-opentelemetry export this as an OTel
    // SpanKind::Server span, and the Arc is shared with the router so a
    // route_layer middleware can stamp `http.route` (the matched route
    // *pattern*, not the concrete path) onto it post-routing — Traceway
    // groups endpoints by that attribute.
    let root = fastrace::Span::root(
        "http_request",
        fastrace::collector::SpanContext::new(
            fastrace::collector::TraceId(id.as_u128()),
            fastrace::collector::SpanId(0),
        ),
    )
    .with_property(|| ("span.kind", "server"));
    let shared_root = Arc::new(root);
    request.extensions_mut().insert(SharedRootSpan(shared_root.clone()));

    let method = request.method().clone();
    let uri = request.uri().clone();

    // Wrap the body in a recorder (unless redacted/skipped) so the trace and
    // access log can show what was sent; see `should_capture_body`.
    let recorder = should_capture_body(&request)
        .then(|| Arc::new(BodyRecorder::new(REQUEST_BODY_CAPTURE_CAP)));
    if let Some(recorder) = &recorder {
        let original = std::mem::replace(request.body_mut(), axum::body::Body::empty());
        *request.body_mut() = axum::body::Body::new(RecordingBody {
            inner: original,
            recorder: recorder.clone(),
        });
    }
    // The root span's Arc is moved into `in_shared_span` below; keep a clone
    // here to stamp `http.request.body` after the handler has consumed it.
    let root_for_body = shared_root.clone();

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

                // Attach the captured body (if any) to the trace root as
                // `http.request.body` — Traceway shows it on the request
                // span. The access log carries the same value.
                let body = recorder.as_ref().and_then(|rec| rec.label());
                if let Some(body) = &body {
                    let body = body.clone();
                    root_for_body.add_property(|| ("http.request.body", body));
                }

                emit_access_log(
                    response.status().is_server_error(),
                    &method,
                    &uri,
                    &id_str,
                    status,
                    latency_ms,
                    body.as_deref(),
                );
                response
            }
            .in_shared_span(shared_root)
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

/// Stamps `http.route` (the matched route *pattern*) onto the request's
/// fastrace root span. Runs as a `route_layer`, i.e. only after the router
/// has matched the path — that's the one place `MatchedPath` is available
/// (the root span itself is created earlier, in `request_id_layer`, where the
/// trace id is pinned to `x-request-id`). Traceway groups endpoints by
/// `http.route`, so this is what makes `/companies/{id}` group as one
/// endpoint instead of one per concrete id.
async fn stamp_http_route(request: Request, next: Next) -> Response {
    if let (Some(shared), Some(pattern)) = (
        request.extensions().get::<SharedRootSpan>(),
        request.extensions().get::<MatchedPath>(),
    ) {
        shared.0.add_property(|| ("http.route", pattern.as_str().to_owned()));
    }
    next.run(request).await
}

/// The `/api/v1` routes (spec §6). The `route_layer` is applied here (not on
/// the outer router) so `MatchedPath` carries the full nested pattern
/// (`/api/v1/companies/{id}`), and only after all routes are declared (a
/// `route_layer` only wraps routes that exist at the time it is called).
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
        .route_layer(middleware::from_fn(stamp_http_route))
}

/// Shared handle to the per-request fastrace root span, threaded through
/// request extensions so `stamp_http_route` (post-routing) can add
/// `http.route` to the span that was created (pre-routing) in
/// `request_id_layer`.
#[derive(Clone)]
struct SharedRootSpan(Arc<fastrace::Span>);

/// `in_span` but taking `Arc<Span>`: sets the root as local parent at every
/// poll (surviving tokio moving the future between threads; the
/// `LocalParentGuard` itself is !Send), while the Arc keeps the span
/// reachable from `stamp_http_route` via request extensions.
trait SharedSpanFutureExt: Sized {
    fn in_shared_span(self, span: Arc<fastrace::Span>) -> InSharedSpan<Self>;
}

impl<T> SharedSpanFutureExt for T {
    fn in_shared_span(self, span: Arc<fastrace::Span>) -> InSharedSpan<Self> {
        InSharedSpan {
            inner: Box::pin(self),
            span,
        }
    }
}

struct InSharedSpan<T> {
    inner: Pin<Box<T>>,
    span: Arc<fastrace::Span>,
}

impl<T: Future> Future for InSharedSpan<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // The inner future is boxed and pinned; InSharedSpan itself is Unpin
        // (Pin<Box<T>> and Arc are both Unpin), so plain get_mut is sound.
        let this = self.get_mut();
        let _guard = this.span.set_local_parent();
        this.inner.as_mut().poll(cx)
    }
}
