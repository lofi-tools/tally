//! Shared harness for tally-api's pg-gated integration tests.
//!
//! Tests share one database — the docker-compose `test-api-db` service on
//! `localhost:5433/tally_test` (`TEST_DATABASE_URL` overrides).  The harness
//! auto-starts the container when it is not running, waits for readiness, and
//! re-initialises the schema once per run (guarded by a Postgres advisory
//! lock + a migration-checksum marker): a missing/edited migration wipes and
//! re-applies the schema; an unchanged one is reused.  Tests arrange their
//! own data (see [`unique_email`]), so a shared DB has minimal effects
//! between tests.
//!
//! An unavailable database is a **hard failure** (panic), not a skip — the
//! pg-tests must not silently pass without a database.  `cargo test -p
//! tally-api --no-default-features` disables the whole suite at compile time
//! (the `pg-tests` feature gate).
//!
//! This lives in a separate **dev-dependency crate** rather than a module
//! under `tests/` so that every item here is `pub` in a library: rustc's
//! `dead_code` lint never flags `pub` items of a lib, however many (or few)
//! test binaries use a given helper.  A module shared across test binaries
//! would instead be compiled per-binary, and any helper unused in a
//! particular binary (e.g. `assert_error` in the body-capture / db-log tests)
//! would be reported as dead code there.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, LazyLock, Mutex};

use axum::body::Body;
use axum::http::{header, HeaderValue, Method, Request, StatusCode};
use axum::response::Response;
use axum::Router;
use serde_json::Value;
use tally_api::models::{Account, BalanceSheet, Company, Filing, Job, Ledger, Session, Split, Transaction, User};
use tally_api::{router, AppState};
use tempfile::TempDir;
use tokio_postgres::NoTls;
use tower::ServiceExt;

/// The shared test database: the docker-compose `test-api-db` service
/// (separate from the dev `db` on 5432), overridable with `TEST_DATABASE_URL`.
pub const DEFAULT_TEST_DB_URL: &str = "postgres://tally:tally@localhost:5433/tally_test";

/// The shared test database URL: `TEST_DATABASE_URL` when set, else the
/// docker-compose `test-api-db` default.
pub fn test_db_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_TEST_DB_URL.to_string())
}

/// A unique email for a test: the shared test DB persists across runs (it is
/// re-initialised once per run, not per test), so tests must never collide
/// on the `users.email` unique index.
pub fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@example.com", uuid::Uuid::new_v4().simple())
}

/// The repository root directory.
pub static REPO: LazyLock<PathBuf> = LazyLock::new(|| {
    let path_bytes = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .unwrap()
        .stdout;
    let path_str = std::str::from_utf8(&path_bytes).unwrap().trim();
    PathBuf::from(path_str)
});

/// The basic FRS 105 fixture book used by the upload/report tests, resolved
/// from the repository root.
pub static FIXTURE_GNUCASH: LazyLock<PathBuf> =
    LazyLock::new(|| REPO.join("libs/ixbrl/example_data/basic-1/input.gnucash"));
/// The per-upload size cap `setup()` uses (mirrors production `main.rs`).
/// Tests that want to trip the 413 branch use
/// [`setup_with_max_upload_bytes`](TestApp::setup_with_max_upload_bytes)
/// with a tiny cap.
pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 50 * 1024 * 1024;

/// A `MakeWriter` that captures every formatted log line for assertions
/// (feed it to `tracing_subscriber::fmt().with_writer(Capture(captured))`).
#[derive(Clone)]
pub struct Capture(pub Arc<Mutex<Vec<String>>>);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = CaptureWriter;

    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter(self.0.clone())
    }
}

pub struct CaptureWriter(Arc<Mutex<Vec<String>>>);

impl Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let line = String::from_utf8_lossy(buf).into_owned();
        self.0.lock().unwrap().push(line);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A running test app on the shared test database.
pub struct TestApp {
    pub app: Router,
    /// The app's toasty `Db` (clone), exposed so tests can poke at rows
    /// directly — e.g. backdate a session's `expires_at` for the
    /// `auth_expired` path.
    pub db: toasty::Db,
    _upload_dir: TempDir,
}

impl TestApp {
    /// Build the app on the shared test database (auto-starting + waiting on
    /// the `test-api-db` container when it is not running), with the
    /// default upload cap.
    ///
    /// The database is re-initialised once per run (see [`reinit_if_needed`])
    /// and shared by every test in the run — tests arrange their own data
    /// (e.g. [`unique_email`]), so one shared DB has minimal effects between
    /// tests.  An unavailable database is a hard failure (panic), not a skip.
    pub async fn setup() -> Self {
        Self::setup_with_max_upload_bytes(DEFAULT_MAX_UPLOAD_BYTES).await
    }

    /// Like [`setup`](Self::setup), but with a custom per-upload size cap
    /// (the 413 test needs a small one so a tiny fixture body trips it).
    pub async fn setup_with_max_upload_bytes(max_upload_bytes: u64) -> Self {
        let db_url = test_db_url();
        ensure_test_db(&db_url).await;
        reinit_if_needed(&db_url).await;

        let connect = toasty::db::Connect::new(&db_url)
            .await
            .expect("connect to the shared test database");
        let mut builder = toasty::Db::builder();
        builder.models(toasty::models!(
            User, Session, Company, Ledger, Account, Transaction, Split, Job, Filing, BalanceSheet
        ));
        let db = builder
            .build(connect)
            .await
            .expect("build the toasty Db on the shared test database");
        // The schema is already current: `reinit_if_needed` applies the
        // committed migrations (the marker makes it a once-per-run step).

        let upload_dir = TempDir::new().expect("a temp upload dir");
        let state = Arc::new(AppState {
            db: db.clone(),
            ch: None,
            upload_dir: upload_dir.path().to_path_buf(),
            max_upload_bytes,
        });

        Self {
            app: router(state),
            db,
            _upload_dir: upload_dir,
        }
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

// ---------------------------------------------------------------------------
// Test-database lifecycle: auto-start, wait, re-initialise once per run
// ---------------------------------------------------------------------------

/// The `pg_advisory_lock` key serialising the once-per-run re-initialisation
/// across concurrently-running test binaries (arbitrary constant).
const REINIT_LOCK_KEY: i64 = 0x7461_6C6C_795F_7067; // "tally_pg"

/// Make sure the shared test database is reachable: when it is not, start
/// the `test-api-db` docker-compose service and wait for it to come up.
/// A database that cannot be started is a hard failure — the pg-tests must
/// not silently skip.
pub async fn ensure_test_db(url: &str) {
    if connect_ok(url).await {
        return;
    }

    // Not running: try the project's own docker-compose `test-api-db`.
    let status = std::process::Command::new("docker")
        .args(["compose", "up", "-d", "--wait", "test-api-db"])
        .current_dir(REPO.as_path())
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => panic!(
            "`docker compose up -d test-api-db` exited with {status}; the pg-tests need the \
             test database at {url} — start it manually or disable the `pg-tests` feature"
        ),
        Err(source) => panic!(
            "docker unavailable ({source}): the pg-tests auto-start their database via docker \
             compose — install/start Docker or disable the `pg-tests` feature"
        ),
    }

    // `docker compose up --wait` already waits for the healthcheck; poll as
    // a fallback (e.g. a container that was mid-restart).
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if connect_ok(url).await {
            return;
        }
    }
    panic!(
        "the test database at {url} did not become reachable after `docker compose up -d \
         test-api-db` (is the docker daemon running?)"
    );
}

/// Can the URL accept a connection + a trivial query?
async fn connect_ok(url: &str) -> bool {
    match tokio_postgres::connect(url, NoTls).await {
        Ok((client, conn)) => {
            tokio::spawn(conn);
            client.simple_query("SELECT 1").await.is_ok()
        }
        Err(_) => false,
    }
}

/// Re-initialise the shared test database once per run: when the committed
/// migration set (fingerprint) differs from the `_test_schema_marker` row,
/// drop the `public` schema and re-apply every migration; otherwise leave it
/// untouched.
///
/// The `pg_advisory_lock` serialises the check across concurrently-running
/// test binaries, so exactly one of them performs the (rare) wipe — the rest
/// see the fresh marker and proceed straight to their data.  A plain re-run
/// with unchanged migrations reuses the existing schema; tests use unique
/// data ([`unique_email`]), so leftover rows never collide.
pub async fn reinit_if_needed(url: &str) {
    let (mut admin, conn) = tokio_postgres::connect(url, NoTls)
        .await
        .unwrap_or_else(|e| panic!("connect to the shared test database at {url}: {e}"));
    tokio::spawn(conn);

    admin
        .execute("SELECT pg_advisory_lock($1)", &[&REINIT_LOCK_KEY])
        .await
        .expect("acquire the test-db reinit lock");
    let result = reinit_locked(&mut admin, url).await;
    let _ = admin
        .execute("SELECT pg_advisory_unlock($1)", &[&REINIT_LOCK_KEY])
        .await;
    result.expect("re-initialise the test database");
}

async fn reinit_locked(
    admin: &mut tokio_postgres::Client,
    url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let fingerprint = migrations_fingerprint();

    // The marker table may not exist yet (first run) — treat that as needing
    // a re-initialisation.
    let marker = match admin
        .query_opt("SELECT checksum FROM _test_schema_marker", &[])
        .await
    {
        Ok(row) => row.map(|row| row.get::<_, String>(0)),
        Err(_) => None,
    };
    if marker.as_deref() == Some(fingerprint.as_str()) {
        return Ok(());
    }

    // Wipe + recreate the public schema, then apply the committed migrations
    // (the same path production startup uses).  No other test binary can be
    // mid-run against it: they all wait on the advisory lock above before
    // opening their connections.
    admin
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await?;

    let connect = toasty::db::Connect::new(url).await?;
    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(
        User, Session, Company, Ledger, Account, Transaction, Split, Job, Filing, BalanceSheet
    ));
    let mut db = builder.build(connect).await?;
    tally_api::migrations::apply_pending(&mut db).await?;

    admin
        .batch_execute("CREATE TABLE _test_schema_marker (checksum TEXT NOT NULL)")
        .await?;
    admin
        .execute(
            "INSERT INTO _test_schema_marker (checksum) VALUES ($1)",
            &[&fingerprint],
        )
        .await?;
    Ok(())
}

/// The identity of the committed migration set: every file's `name:checksum`
/// in apply order.  Any added or edited migration changes the fingerprint and
/// triggers a re-initialisation on the next run.
fn migrations_fingerprint() -> String {
    tally_api::migrations::list_all_migrations()
        .iter()
        .map(|entry| format!("{}:{}", entry.name, entry.checksum))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Request / response helpers
// ---------------------------------------------------------------------------

/// Build a request; `token` becomes `Authorization: Bearer …`, `body`
/// becomes a JSON body with the proper content type.
pub fn request(method: Method, uri: &str, token: Option<&str>, body: Option<&Value>) -> Request<Body> {
    request_with_guest(method, uri, None, token, body)
}

/// Like [`request`], with an `X-Guest-Id` header (temp-user spec §5).
pub fn request_with_guest(
    method: Method,
    uri: &str,
    guest_id: Option<&str>,
    token: Option<&str>,
    body: Option<&Value>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(guest_id) = guest_id {
        builder = builder.header("x-guest-id", guest_id);
    }
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

