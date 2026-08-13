//! Schema migrations (spec §4/§5): committed SQL files, applied on startup.
//!
//! The pattern mirrors `libs/storage`'s `migrations.rs` (which drives the
//! todo-lofi Turso store): migration SQL lives in committed `.sql` files under
//! `apps/tally-api/migrations/`, is embedded into the binary at compile time
//! via [`include_dir!`], and every API startup plays the ones that are missing.
//!
//! A `_migrations_history` table records each applied migration with the
//! sha256 checksum of its SQL, so a run is:
//!
//! 1. create `_migrations_history` if needed;
//! 2. list every embedded migration, sorted by filename (names are
//!    zero-padded `NNNN_name.sql` so lexical order == apply order);
//! 3. for each, verify the checksum if already applied (a committed-but-edited
//!    migration is a hard error, not a silent re-run) or play it and record it.
//!
//! This replaces the previous startup `push_schema()` (toasty's schema sync),
//! which re-issued `CREATE TABLE` on every boot and therefore failed against
//! an existing schema. The initial migration's SQL is byte-for-byte what
//! `push_schema` produced for the models in `models.rs`, so behaviour for a
//! fresh database is unchanged.

use include_dir::{Dir, include_dir};
use sha2::{Digest, Sha256};
use snafu::{ResultExt, Snafu};
use std::collections::HashMap;

/// The committed migration SQL files, embedded at compile time.
pub static MIGRATIONS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

/// A single migration: its SQL plus the identity/checksum recorded in
/// `_migrations_history`.
#[derive(Debug, Clone)]
pub struct MigrationEntry {
    /// Stable id (first 8 bytes of the sha256 of the filename), stored so the
    /// history table has a stable primary key.
    pub id: u64,
    /// Filename, e.g. `0001_init.sql`.
    pub name: String,
    /// The full SQL body of the migration file.
    pub sql: String,
    /// sha256 hex of [`Self::sql`].
    pub checksum: String,
}

impl MigrationEntry {
    fn from_file(name: String, sql: String) -> Self {
        let checksum = Self::compute_checksum(&sql);
        let id = Self::compute_id(&name);
        Self {
            id,
            name,
            sql,
            checksum,
        }
    }

    fn compute_id(name: &str) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        let result = hasher.finalize();
        u64::from_be_bytes(result[..8].try_into().unwrap())
    }

    fn compute_checksum(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let bytes = hasher.finalize();
        bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    }
}

/// Split a multi-statement SQL string into individual statements.
///
/// Handles single-quoted, double-quoted, and backtick-quoted strings, ignores
/// SQL comments (`--` and `/* ... */`), and defers splitting on `;` while
/// inside `BEGIN...END` blocks (trigger bodies, etc.).
fn split_sql(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();
    let mut begin_depth: u32 = 0;

    while let Some(ch) = chars.next() {
        match ch {
            '\'' | '"' | '`' => {
                current.push(ch);
                let quote = ch;
                while let Some(c) = chars.next() {
                    current.push(c);
                    if c == quote {
                        if chars.peek() == Some(&quote) {
                            current.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                }
            }
            '-' if chars.peek() == Some(&'-') => {
                chars.next();
                for c in chars.by_ref() {
                    if c == '\n' {
                        current.push(c);
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = '\0';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            }
            ';' if begin_depth == 0 => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    statements.push(trimmed);
                }
                current.clear();
            }
            _ => {
                current.push(ch);
                if !in_quote(&current) {
                    check_begin_end(&current, &mut begin_depth);
                }
            }
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        statements.push(trimmed);
    }

    statements
}

fn in_quote(s: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    for ch in s.chars() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ => {}
        }
    }
    in_single || in_double
}

fn check_begin_end(current: &str, depth: &mut u32) {
    let lower = current.to_lowercase();
    let bytes = lower.as_bytes();
    let len = bytes.len();

    let check = |keyword: &[u8]| -> bool {
        if len < keyword.len() {
            return false;
        }
        if &bytes[len - keyword.len()..] != keyword {
            return false;
        }
        if len > keyword.len() {
            !bytes[len - keyword.len() - 1].is_ascii_alphanumeric()
        } else {
            true
        }
    };

    if check(b" begin") {
        *depth += 1;
    } else if check(b" end") {
        *depth = depth.saturating_sub(1);
    }
}

/// Every committed migration, sorted by filename (apply order).
pub fn list_all_migrations() -> Vec<MigrationEntry> {
    let mut entries: Vec<_> = MIGRATIONS_DIR
        .files()
        .filter(|f| f.path().extension().is_some_and(|ext| ext == "sql"))
        .map(|f| {
            let name = f.path().file_name().unwrap().to_string_lossy().to_string();
            let sql = f.contents_utf8().unwrap().to_string();
            MigrationEntry::from_file(name, sql)
        })
        .collect();
    entries.sort_by_key(|e| e.name.clone());
    entries
}

/// The migrations already recorded in `_migrations_history` (name → checksum).
async fn list_applied_migrations(db: &mut toasty::Db) -> Result<HashMap<String, String>, MigrationError> {
    let rows = toasty::sql::query(r#"SELECT "name", "checksum" FROM "_migrations_history""#)
        .column_types([toasty::stmt::Type::String, toasty::stmt::Type::String])
        .exec(db)
        .await
        .context(DatabaseSnafu)?;

    let mut applied = HashMap::new();
    for row in rows {
        if let toasty::stmt::Value::Record(record) = row
            && let (
                Some(toasty::stmt::Value::String(name)),
                Some(toasty::stmt::Value::String(checksum)),
            ) = (record.first(), record.get(1))
        {
            applied.insert(name.clone(), checksum.clone());
        }
    }
    Ok(applied)
}

/// Create `_migrations_history` (idempotent) and play every committed
/// migration that has not been applied yet, verifying checksums of the ones
/// that have. Call once at startup, before the server accepts requests.
pub async fn apply_pending(db: &mut toasty::Db) -> Result<(), MigrationError> {
    toasty::sql::statement(
        r#"CREATE TABLE IF NOT EXISTS "_migrations_history" (
                "id" BIGINT PRIMARY KEY,
                "name" TEXT NOT NULL UNIQUE,
                "checksum" TEXT NOT NULL
            )"#,
    )
    .exec(db)
    .await
    .context(DatabaseSnafu)?;

    let all = list_all_migrations();
    let applied = list_applied_migrations(db).await?;

    for entry in &all {
        if let Some(existing_checksum) = applied.get(&entry.name) {
            if existing_checksum != &entry.checksum {
                return Err(MigrationError::ChecksumMismatch {
                    name: entry.name.clone(),
                    expected: existing_checksum.clone(),
                    actual: entry.checksum.clone(),
                });
            }
            tracing::debug!(name = %entry.name, "migration verified");
        } else {
            tracing::info!(name = %entry.name, "applying migration");
            let stmts = split_sql(&entry.sql);
            for stmt in &stmts {
                toasty::sql::statement(stmt.clone())
                    .exec(db)
                    .await
                    .context(DatabaseSnafu)?;
            }

            toasty::sql::statement(
                r#"INSERT INTO "_migrations_history" ("id", "name", "checksum")
                   VALUES ($1, $2, $3)"#,
            )
            .bind(entry.id as i64)
            .bind(&entry.name)
            .bind(&entry.checksum)
            .exec(db)
            .await
            .context(DatabaseSnafu)?;
        }
    }

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum MigrationError {
    #[snafu(display("database error: {source}"))]
    Database { source: toasty::Error },

    #[snafu(display(
        "checksum mismatch for migration '{name}': expected {expected}, got {actual}"
    ))]
    ChecksumMismatch {
        name: String,
        expected: String,
        actual: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_migrations() {
        let migrations = list_all_migrations();
        assert!(migrations.len() >= 1, "no migrations embedded");
        // Names are zero-padded so lexical sort is apply order.
        assert!(
            migrations.windows(2).all(|w| w[0].name < w[1].name),
            "migrations not in lexical (apply) order: {:?}",
            migrations.iter().map(|m| &m.name).collect::<Vec<_>>(),
        );
        // The initial migration must create every model table.
        let init = migrations
            .iter()
            .find(|m| m.name == "0001_init.sql")
            .expect("0001_init.sql exists");
        for table in ["users", "sessions", "companies", "ledgers", "accounts", "transactions", "splits"] {
            assert!(
                init.sql.contains(&format!("CREATE TABLE \"{table}\"")),
                "0001_init.sql missing table {table}"
            );
        }
        // Deterministic ids/checksums.
        let again = list_all_migrations();
        for (a, b) in migrations.iter().zip(again.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.checksum, b.checksum);
        }
    }

    #[test]
    fn test_split_sql_simple() {
        let sql = "CREATE TABLE a (id INT);\nCREATE TABLE b (id INT);";
        let stmts = split_sql(sql);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "CREATE TABLE a (id INT)");
        assert_eq!(stmts[1], "CREATE TABLE b (id INT)");
    }

    #[test]
    fn test_split_sql_no_trailing_semicolon() {
        let sql = "CREATE TABLE a (id INT)";
        let stmts = split_sql(sql);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "CREATE TABLE a (id INT)");
    }

    #[test]
    fn test_split_sql_quoted_semicolon() {
        let sql = r#"INSERT INTO a VALUES ('hello; world');CREATE TABLE b (id INT);"#;
        let stmts = split_sql(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("hello; world"));
    }

    #[test]
    fn test_split_sql_comments() {
        let sql =
            "-- comment\nCREATE TABLE a (id INT);\n/* block comment */\nCREATE TABLE b (id INT);";
        let stmts = split_sql(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_split_sql_breaks_init_into_statements() {
        let init = list_all_migrations()
            .into_iter()
            .find(|m| m.name == "0001_init.sql")
            .expect("0001_init.sql exists");
        let stmts = split_sql(&init.sql);
        assert!(stmts.len() >= 14, "expected many statements, got {}", stmts.len());
        // Every statement must be executable on its own: none may retain a
        // trailing comment line, and all CREATE statements carry a ';'-free
        // but complete body.
        for s in &stmts {
            assert!(!s.ends_with("--"), "statement ends with a comment: {s:?}");
            assert!(!s.starts_with("--"), "statement starts with a comment: {s:?}");
        }
    }
}
