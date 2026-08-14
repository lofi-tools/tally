//! Durable background jobs (spec: ch-filings-sync-spec.md §3).
//!
//! A `jobs` table in Postgres (status, attempts, last_error) plus a tokio
//! worker started from `main.rs`:
//!
//! - jobs are claimed with `SELECT … FOR UPDATE SKIP LOCKED` inside a
//!   transaction, so concurrent workers (or processes) never double-claim;
//!   the row lock is held only for the claim, never for the job's execution;
//! - on startup, rows stuck in `running` are reset to `pending` — their
//!   lease died with the previous process, so the work is retried (the
//!   durability guarantee);
//! - the worker runs jobs with structured concurrency: a bounded
//!   [`JoinSet`], a child [`CancellationToken`] per job, and a graceful
//!   drain on shutdown;
//! - fail-fast policy: one attempt per job; the error lands in `last_error`
//!   and the job is marked `failed` (a manual refresh re-enqueues).
//!   `attempts` / `next_retry_at` are written but unused by policy today.

use std::sync::Arc;
use std::time::Duration;

use fastrace::future::FutureExt as _;
use snafu::{ResultExt, Snafu};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::AppState;

/// How many jobs may run concurrently.
const CONCURRENCY: usize = 4;
/// How often the worker polls for due jobs when idle.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Errors from the job machinery itself (enqueue / claim). A job *execution*
/// failure is not an error here — it is persisted on the job row.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum JobError {
    #[snafu(display("database error: {source}"))]
    Database { source: toasty::Error },
}

impl From<JobError> for crate::error::AppError {
    fn from(e: JobError) -> Self {
        match e {
            JobError::Database { source } => crate::error::AppError::Db { source },
        }
    }
}

/// A job claimed for execution.
pub struct ClaimedJob {
    pub id: uuid::Uuid,
    pub kind: String,
    pub company_id: uuid::Uuid,
}

/// RFC 3339 UTC now (the app's timestamp convention, see auth.rs).
pub(crate) fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Enqueue a job. The partial unique index
/// `(kind, company_id) WHERE status IN ('pending','running')` makes a
/// duplicate in-flight enqueue a no-op; a done/failed job is freely
/// re-enqueued (that is what Refresh does).
///
/// Returns `Some(job_id)` when a new job was inserted and `None` when the
/// enqueue was a no-op (a pending/running job already exists) — the refresh
/// endpoint uses this to return 202 vs 200.
pub async fn enqueue(db: &mut toasty::Db, kind: &str, company_id: uuid::Uuid) -> Result<Option<uuid::Uuid>, JobError> {
    let now = now_rfc3339();
    let rows = toasty::sql::query(
        r#"INSERT INTO "jobs" ("id", "kind", "company_id", "status", "attempts", "last_error", "next_retry_at", "created_at", "updated_at")
           VALUES ($1, $2, $3, 'pending', 0, NULL, NULL, $4, $4)
           ON CONFLICT DO NOTHING
           RETURNING "id""#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(kind)
    .bind(company_id)
    .bind(&now)
    .column_types([toasty::stmt::Type::Uuid])
    .exec(db)
    .await
    .context(DatabaseSnafu)?;
    Ok(rows
        .into_iter()
        .next()
        .and_then(|row| match row {
            toasty::stmt::Value::Record(record) => {
                record.first().and_then(|v| match v {
                    toasty::stmt::Value::Uuid(id) => Some(*id),
                    _ => None,
                })
            }
            _ => None,
        }))
}

/// The latest job row for a `(kind, company)`, if any (the refresh endpoint
/// uses it to distinguish a fresh enqueue from an in-flight no-op).
pub async fn latest_for_company(
    db: &mut toasty::Db,
    kind: &str,
    company_id: uuid::Uuid,
) -> Result<Option<ClaimedJob>, JobError> {
    let rows = toasty::sql::query(
        r#"SELECT "id", "kind", "company_id" FROM "jobs"
           WHERE "kind" = $1 AND "company_id" = $2
           ORDER BY "created_at" DESC
           LIMIT 1"#,
    )
    .bind(kind)
    .bind(company_id)
    .column_types([
        toasty::stmt::Type::Uuid,
        toasty::stmt::Type::String,
        toasty::stmt::Type::Uuid,
    ])
    .exec(db)
    .await
    .context(DatabaseSnafu)?;
    Ok(rows.into_iter().next().and_then(parse_claimed_job))
}

/// Map one raw `SELECT` row to a [`ClaimedJob`] (`id`, `kind`, `company_id`).
fn parse_claimed_job(row: toasty::stmt::Value) -> Option<ClaimedJob> {
    if let toasty::stmt::Value::Record(record) = row
        && let (
            Some(toasty::stmt::Value::Uuid(id)),
            Some(toasty::stmt::Value::String(kind)),
            Some(toasty::stmt::Value::Uuid(company_id)),
        ) = (record.first(), record.get(1), record.get(2))
    {
        return Some(ClaimedJob {
            id: *id,
            kind: kind.clone(),
            company_id: *company_id,
        });
    }
    None
}

/// Claim up to `limit` pending jobs: `SELECT … FOR UPDATE SKIP LOCKED`
/// inside a transaction, flipping each claimed row to `running` before
/// commit. The lock is held only for the claim, so a crash mid-job leaves a
/// `running` row that the next boot re-claims.
async fn claim_jobs(db: &mut toasty::Db, limit: usize) -> Result<Vec<ClaimedJob>, JobError> {
    let mut tx = db
        .transaction()
        .await
        .map_err(|source| JobError::Database { source })?;
    let rows = toasty::sql::query(
        r#"SELECT "id", "kind", "company_id" FROM "jobs"
           WHERE "status" = 'pending'
           ORDER BY "created_at" ASC
           LIMIT $1
           FOR UPDATE SKIP LOCKED"#,
    )
    .bind(limit as i64)
    .column_types([
        toasty::stmt::Type::Uuid,
        toasty::stmt::Type::String,
        toasty::stmt::Type::Uuid,
    ])
    .exec(&mut tx)
    .await
    .map_err(|source| JobError::Database { source })?;

    let jobs = rows.into_iter().filter_map(parse_claimed_job).collect::<Vec<_>>();
    if !jobs.is_empty() {
        let now = now_rfc3339();
        for job in &jobs {
            toasty::sql::statement(
                r#"UPDATE "jobs" SET "status" = 'running', "updated_at" = $1 WHERE "id" = $2"#,
            )
            .bind(&now)
            .bind(job.id)
            .exec(&mut tx)
            .await
            .map_err(|source| JobError::Database { source })?;
        }
        tx.commit()
            .await
            .map_err(|source| JobError::Database { source })?;
    }
    Ok(jobs)
}

/// Reset rows stuck in `running` back to `pending` (their lease died with
/// the previous process). Best-effort: a DB fault here shouldn't take down
/// startup.
async fn reclaim_stale_running(db: &mut toasty::Db) {
    let _ = toasty::sql::statement(
        r#"UPDATE "jobs" SET "status" = 'pending', "updated_at" = $1 WHERE "status" = 'running'"#,
    )
    .bind(&now_rfc3339())
    .exec(db)
    .await;
}

/// Run the worker until `shutdown` fires: claim due jobs, run them with
/// bounded fan-out, then drain in-flight work gracefully.
pub async fn run_worker(state: Arc<AppState>, shutdown: CancellationToken) {
    let mut db = state.db.clone();

    // Startup re-claim: 'running' rows are orphans from a previous process.
    reclaim_stale_running(&mut db).await;

    let mut workers = JoinSet::new();
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tokio::time::sleep(POLL_INTERVAL) => {},
            _ = workers.join_next(), if !workers.is_empty() => {}, // a slot freed
        }
        let free = CONCURRENCY.saturating_sub(workers.len());
        if free == 0 {
            continue;
        }
        match claim_jobs(&mut db, free).await {
            Ok(jobs) => {
                for job in jobs {
                    workers.spawn(run_job(state.clone(), job, shutdown.child_token()));
                }
            }
            Err(e) => tracing::error!(error = %e, "job claim failed"),
        }
    }

    // Graceful drain: the children already hold cancelled tokens; await them
    // to empty (each exits at a safe point between steps).
    while workers.join_next().await.is_some() {}
}

/// Run one claimed job: dispatch on `kind`, persist the outcome.
///
/// The whole run sits inside a fastrace root span marked `span.kind=consumer`
/// (fastrace-opentelemetry maps that property to OTel SpanKind::Consumer,
/// which is how Traceway renders background jobs). Rooting the job also gives
/// every db query / log emitted during it a trace context, so logs that
/// *didn't* originate from a request still link to a trace in Traceway.
async fn run_job(state: Arc<AppState>, job: ClaimedJob, token: CancellationToken) {
    let root = fastrace::Span::root(
        format!("job.{}", job.kind),
        fastrace::collector::SpanContext::random(),
    )
    .with_property(|| ("span.kind", "consumer"))
    .with_property(|| ("job.id", job.id.to_string()))
    .with_property(|| ("job.kind", job.kind.clone()));

    async move {
        let result = match job.kind.as_str() {
            "fetch_filings" => crate::filings::fetch_and_store(&state, job.company_id, &token).await,
            other => Err(crate::error::AppError::Validation {
                fields: vec![crate::error::FieldIssue {
                    field: "kind".into(),
                    reason: format!("unknown job kind '{other}'"),
                }],
            }),
        };
        let mut db = state.db.clone();
        let (status, err) = match result {
            Ok(()) => ("done", None),
            Err(e) => {
                tracing::error!(job = %job.id, kind = %job.kind, error = %e, "job failed");
                ("failed", Some(e.to_string()))
            }
        };
        let _ = toasty::sql::statement(
            r#"UPDATE "jobs" SET "status" = $1, "last_error" = $2, "attempts" = "attempts" + 1, "updated_at" = $3
               WHERE "id" = $4"#,
        )
        .bind(status)
        .bind(err.as_deref())
        .bind(&now_rfc3339())
        .bind(job.id)
        .exec(&mut db)
        .await;
    }
    .in_span(root)
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_rfc3339_utc() {
        let s = now_rfc3339();
        assert!(s.contains('T'), "RFC 3339 UTC: {s}");
        let parsed = chrono::DateTime::parse_from_rfc3339(&s).expect("RFC 3339 parses");
        assert_eq!(parsed.offset().local_minus_utc(), 0, "UTC offset: {s}");
    }

    #[test]
    fn claim_row_parses() {
        let id = uuid::Uuid::new_v4();
        let company_id = uuid::Uuid::new_v4();
        let row = toasty::stmt::Value::record_from_vec(vec![
            toasty::stmt::Value::Uuid(id),
            toasty::stmt::Value::String("fetch_filings".into()),
            toasty::stmt::Value::Uuid(company_id),
        ]);
        let job = parse_claimed_job(row).expect("parses");
        assert_eq!(job.id, id);
        assert_eq!(job.kind, "fetch_filings");
        assert_eq!(job.company_id, company_id);
    }
}
