//! Shared harness for the pg-gated integration tests.
//!
//! Every test gets its own throwaway database (`tally_test_<uuid>`), created
//! and dropped via an admin connection, so tests can run in parallel (within
//! and across binaries) without colliding.  `setup()` returns `None` when the
//! database is unreachable, and callers skip (the spec's graceful no-DB
//! behaviour); `cargo test -p tally-api --no-default-features` disables the
//! whole suite at compile time.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, HeaderValue, Method, Request, StatusCode};
use axum::response::Response;
use axum::Router;
use serde_json::Value;
use tally_api::app::{router, AppState};
use tally_api::models::{Account, Company, Ledger, Session, Split, Transaction, User};
use tempfile::TempDir;
use tokio_postgres::NoTls;
use tower::ServiceExt;

/// Matches `main.rs` (and `docker-compose.yml`).
pub const DEFAULT_DB_URL: &str = "postgres://tally:tally@localhost:5432/tally";
/// The basic FRS 105 fixture book used by the upload/report tests.
#[allow(dead_code)] // only some of the test binaries use it
pub const FIXTURE_GNUCASH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libs/ixbrl/example_data/basic-1/input.gnucash"
);

/// A running test app + the admin handle that owns its database.
pub struct TestApp {
    pub app: Router,
    admin: Option<tokio_postgres::Client>,
    db_name: String,
    _upload_dir: TempDir,
}

impl TestApp {
    /// Build the app on a fresh database.  `None` when Postgres is
    /// unreachable (test then skips).
    pub async fn setup() -> Option<Self> {
        let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB_URL.to_string());

        // Admin connection to the default DB (same server): create the
        // per-test database.
        let (admin, conn) = tokio_postgres::connect(&db_url, NoTls).await.ok()?;
        tokio::spawn(conn);
        let db_name = format!("tally_test_{}", uuid::Uuid::new_v4().simple());
        admin
            .execute(&format!("CREATE DATABASE {db_name}"), &[])
            .await
            .ok()?;

        let test_url = swap_dbname(&db_url, &db_name);
        let connect = toasty::db::Connect::new(&test_url).await.ok()?;
        let mut builder = toasty::Db::builder();
        builder.models(toasty::models!(User, Session, Company, Ledger, Account, Transaction, Split));
        let db = builder.build(connect).await.ok()?;
        db.push_schema().await.ok()?;

        let upload_dir = TempDir::new().ok()?;
        let state = Arc::new(AppState {
            db,
            ch: None,
            upload_dir: upload_dir.path().to_path_buf(),
            max_upload_bytes: 50 * 1024 * 1024,
        });

        Some(Self {
            app: router(state),
            admin: Some(admin),
            db_name,
            _upload_dir: upload_dir,
        })
    }

    /// `POST /api/v1/auth/register`, returning the bearer token.
    pub async fn register(&self, email: &str) -> String {
        let body = serde_json::json!({
            "display_name": "Test User",
            "email": email,
            "password": "correct horse battery staple",
        });
        let resp = self
            .send(request(Method::POST, "/api/v1/auth/register", None, Some(&body)))
            .await;
        assert_eq!(resp.status(), StatusCode::OK, "register {email}: {:?}", body);
        let json = json_body(resp).await;
        json["token"].as_str().expect("token").to_string()
    }

    /// A convenience: a second user on the same app (for ownership tests).
    #[allow(dead_code)] // only some of the test binaries use it
    pub async fn register_second(&self, email: &str) -> String {
        self.register(email).await
    }

    /// Run one request through the router.  Every response must carry the
    /// `X-Request-Id` header (spec §11.4), so the harness asserts it here.
    pub async fn send(&self, req: Request<Body>) -> Response {
        let resp = self.app.clone().oneshot(req).await.expect("router answers");
        assert!(
            resp.headers().contains_key("x-request-id"),
            "every response carries X-Request-Id (got {:?})",
            resp.headers(),
        );
        resp
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        // Best-effort cleanup on whatever runtime the test ran on; ignore
        // failures (the DB name is unique, so leftovers are harmless).
        if let Ok(handle) = tokio::runtime::Handle::try_current()
            && let Some(admin) = self.admin.take()
        {
            let db_name = self.db_name.clone();
            handle.spawn(async move {
                let _ = admin
                    .execute(&format!("DROP DATABASE {db_name} WITH (FORCE)"), &[])
                    .await;
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Request / response helpers
// ---------------------------------------------------------------------------

/// Build a request; `token` becomes `Authorization: Bearer …`, `body`
/// becomes a JSON body with the proper content type.
pub fn request(method: Method, uri: &str, token: Option<&str>, body: Option<&Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {token}")).unwrap());
    }
    let body = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    builder.body(body).expect("valid request")
}

/// Read the response body as JSON (asserting it parses).
pub async fn json_body(resp: Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 16 * 1024 * 1024)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("response body is JSON")
}

/// Assert the §11.1 envelope: exact status + `error.code`.
pub async fn assert_error(resp: Response, status: StatusCode, code: &str) {
    assert_eq!(resp.status(), status, "status for expected code {code}");
    let json = json_body(resp).await;
    assert_eq!(json["error"]["code"], code, "envelope code: {json}");
}

/// Multipart body for the ledger upload endpoint.
#[allow(dead_code)] // only some of the test binaries use it
pub fn multipart_body(boundary: &str, filename: &str, bytes: &[u8]) -> Body {
    let mut buf = Vec::new();
    buf.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    buf.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n").as_bytes(),
    );
    buf.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    buf.extend_from_slice(bytes);
    buf.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Body::from(buf)
}

/// Swap the database name in a `postgres://` URL.
fn swap_dbname(url: &str, db_name: &str) -> String {
    let mut parts = url.split('/').collect::<Vec<_>>();
    if let Some(last) = parts.last_mut() {
        *last = db_name;
    }
    parts.join("/")
}
