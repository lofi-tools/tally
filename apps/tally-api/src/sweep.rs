//! One-shot startup sweep of abandoned temporary (guest) users (temp-user
//! spec §8).
//!
//! Runs once at API startup (no timer): every temp user whose last activity
//! — `MAX(updated_at)` across its owned rows (`companies.updated_at`,
//! `jobs.updated_at`, `filings.fetched_at`, `balance_sheets.created_at`,
//! `ledgers.uploaded_at`), falling back to `users.created_at` when it owns
//! nothing — is older than the TTL is hard-deleted along with its sessions,
//! companies and everything they own (via the shared delete cascade). Real
//! users (`is_temporary = false`) are never touched.

use chrono::{Duration, Utc};

use crate::error::AppError;
use crate::models::{Session, User};

/// Default TTL since last activity, in days. Overridable via the
/// `TALLY_GUEST_TTL_DAYS` env var (spec §12.3).
pub const DEFAULT_GUEST_TTL_DAYS: i64 = 90;

/// Delete abandoned temp users and everything they own. Returns the number
/// of temp users deleted.
pub async fn sweep_abandoned_guests(db: &mut toasty::Db, ttl_days: i64) -> Result<usize, AppError> {
    let cutoff = Utc::now() - Duration::days(ttl_days);
    let users = User::all()
        .filter(User::fields().is_temporary().eq(true))
        .exec(db)
        .await?;

    let mut deleted = 0;
    for user in users {
        let last = last_activity(db, user.id)
            .await?
            .unwrap_or_else(|| user.created_at.clone());
        if !older_than(&last, cutoff) {
            continue;
        }

        tracing::info!(
            user = %user.id,
            guest = %user.guest_id.as_deref().unwrap_or(""),
            last_activity = %last,
            "sweeping abandoned guest user"
        );
        let mut tx = db.transaction().await?;
        Session::filter_by_user_id(user.id).delete().exec(&mut tx).await?;
        let companies = crate::models::Company::filter_by_user_id(user.id).exec(&mut tx).await?;
        for company in &companies {
            crate::companies::delete_company_and_owned(&mut tx, company).await?;
        }
        User::filter_by_id(user.id).delete().exec(&mut tx).await?;
        tx.commit().await?;
        deleted += 1;
    }
    Ok(deleted)
}

/// The most recent activity timestamp among a user's owned rows (spec §8) —
/// `None` when the user owns nothing (the caller then falls back to
/// `users.created_at`).
async fn last_activity(db: &mut toasty::Db, user_id: uuid::Uuid) -> Result<Option<String>, AppError> {
    let rows = toasty::sql::query(
        r#"SELECT GREATEST(
                (SELECT MAX("updated_at") FROM "companies" WHERE "user_id" = $1),
                (SELECT MAX(j."updated_at") FROM "jobs" j JOIN "companies" c ON c."id" = j."company_id" WHERE c."user_id" = $1),
                (SELECT MAX(f."fetched_at") FROM "filings" f JOIN "companies" c ON c."id" = f."company_id" WHERE c."user_id" = $1),
                (SELECT MAX(bs."created_at") FROM "balance_sheets" bs JOIN "companies" c ON c."id" = bs."company_id" WHERE c."user_id" = $1),
                (SELECT MAX(l."uploaded_at") FROM "ledgers" l JOIN "companies" c ON c."id" = l."company_id" WHERE c."user_id" = $1)
            )"#,
    )
    .bind(user_id)
    .column_types([toasty::stmt::Type::String])
    .exec(db)
    .await?;

    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    let toasty::stmt::Value::Record(record) = row else {
        return Ok(None);
    };
    match record.first() {
        Some(toasty::stmt::Value::String(s)) => Ok(Some(s.clone())),
        _ => Ok(None), // no owned rows → NULL
    }
}

/// Is an RFC 3339 UTC timestamp older than `cutoff`? Unparseable timestamps
/// count as *not* old — never sweep away data on a format bug.
fn older_than(rfc3339: &str, cutoff: chrono::DateTime<Utc>) -> bool {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| t.with_timezone(&Utc) < cutoff)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutoff_comparison() {
        let cutoff = Utc::now() - Duration::days(30);
        let old = (Utc::now() - Duration::days(100)).to_rfc3339();
        let recent = (Utc::now() - Duration::days(1)).to_rfc3339();
        assert!(older_than(&old, cutoff), "100 days ago is past a 30-day cutoff");
        assert!(!older_than(&recent, cutoff), "1 day ago is within a 30-day cutoff");
        assert!(!older_than("not-a-timestamp", cutoff), "unparseable → never old");
    }
}
