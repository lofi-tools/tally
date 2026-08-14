//! Driver wrapper that logs failed DB requests with the whole evaluated
//! request (SQL + bound params) at ERROR.
//!
//! toasty's postgres driver renders every request to SQL internally but only
//! surfaces it through its per-query log at DEBUG (target `toasty::query`);
//! a failed request is invisible at the default log level. This wrapper sits
//! between toasty's pool and the driver: each connection's `exec` is wrapped
//! so that on failure we serialize the operation back to SQL (the same
//! serializer the driver uses) and emit `tracing::error!` with it.

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use toasty_core::driver::operation::TypedValue;
use toasty_core::driver::{Capability, ConnectContext, Connection, Driver, ExecResponse, Operation};
use toasty_core::schema::{db, diff};
use toasty_core::{Result, Schema};
use toasty_driver_postgresql::PostgreSQL;
use toasty_sql::{Serializer, Statement};

/// A [`Driver`] wrapper around the toasty PostgreSQL driver that logs failed
/// requests with the whole evaluated request.
pub struct LoggingDriver {
    inner: PostgreSQL,
}

impl LoggingDriver {
    /// Build from a connection URL (same scheme validation as
    /// [`toasty::db::Connect`]).
    pub fn new(url: impl Into<String>) -> Result<Self> {
        Ok(Self { inner: PostgreSQL::new(url)? })
    }
}

impl std::fmt::Debug for LoggingDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoggingDriver")
            .field("url", &self.inner.url())
            .finish()
    }
}

#[async_trait]
impl Driver for LoggingDriver {
    fn url(&self) -> Cow<'_, str> {
        self.inner.url()
    }

    fn capability(&self) -> &'static Capability {
        self.inner.capability()
    }

    async fn connect(&self, cx: &ConnectContext) -> Result<Box<dyn Connection>> {
        Ok(Box::new(LoggingConnection {
            inner: self.inner.connect(cx).await?,
        }))
    }

    fn max_connections(&self) -> Option<usize> {
        self.inner.max_connections()
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> db::Migration {
        self.inner.generate_migration(schema_diff)
    }

    async fn reset_db(&self) -> Result<()> {
        self.inner.reset_db().await
    }
}

/// A [`Connection`] wrapper that logs a failed [`Connection::exec`] with the
/// whole evaluated request.
struct LoggingConnection {
    inner: Box<dyn Connection>,
}

impl std::fmt::Debug for LoggingConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoggingConnection").finish_non_exhaustive()
    }
}

#[async_trait]
impl Connection for LoggingConnection {
    async fn exec(&mut self, schema: &Arc<Schema>, plan: Operation) -> Result<ExecResponse> {
        // Snapshot the plan for the failure log (the driver consumes it).
        let log_plan = plan.clone();
        let result = self.inner.exec(schema, plan).await;
        if let Err(error) = &result {
            let (sql, params) = render_operation(schema, &log_plan);
            tracing::error!(
                error = %error,
                db.system = "postgresql",
                db.statement = %sql,
                db.params = ?params,
                "db request failed",
            );
        }
        result
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    async fn ping(&mut self) -> Result<()> {
        self.inner.ping().await
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        self.inner.push_schema(schema).await
    }

    async fn applied_migrations(&mut self) -> Result<Vec<db::AppliedMigration>> {
        self.inner.applied_migrations().await
    }

    async fn apply_migration(
        &mut self,
        id: u64,
        name: &str,
        migration: &db::Migration,
    ) -> Result<()> {
        self.inner.apply_migration(id, name, migration).await
    }
}

/// Serializes an operation back to the SQL the driver evaluates (mirrors the
/// postgres driver's own `exec`), plus its bound params for the log.
fn render_operation(schema: &Schema, plan: &Operation) -> (String, Vec<TypedValue>) {
    let serializer = Serializer::postgresql(&schema.db);
    match plan {
        Operation::Insert(op) => (
            serializer.serialize(&Statement::from(op.stmt.clone())),
            op.params.clone(),
        ),
        Operation::QuerySql(query) => (
            serializer.serialize(&Statement::from(query.stmt.clone())),
            query.params.clone(),
        ),
        Operation::RawSql(op) => (op.sql.clone(), op.params.clone()),
        Operation::Transaction(t) => (serializer.serialize_transaction(t), Vec::new()),
        other => (format!("{other:?}"), Vec::new()),
    }
}
