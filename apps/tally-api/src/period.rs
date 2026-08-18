//! Return-period resolution (spec §7), mirroring `tally-cli`'s
//! `ConfigBuilder::resolve_period` chain:
//!
//! 1. an explicit `period` in the request wins;
//! 2. else the `made_up_to` date → the 12 months ending on it;
//! 3. else the company's next accounting period from Companies House
//!    (`next_accounting_period_from` on the stored profile) — an enrichment
//!    only, used when a key is set;
//! 4. else the registration-date anniversary schedule (no key needed).

use std::sync::Arc;

use chrono::{Duration, Months, NaiveDate};
use reports::company::{AccountingPeriod, Company as LibCompany};

use crate::AppState;
use crate::error::{AppError, FieldIssue};
use crate::models::Company;

/// The period-relevant subset of a report request body (defined fully in
/// `reports.rs`; `period`/`made_up_to` are what resolution consumes).
pub trait PeriodRequest {
    fn period(&self) -> Option<AccountingPeriod>;
    fn made_up_to(&self) -> Option<NaiveDate>;
}

/// Resolve the return period for a report on `company`.
pub async fn resolve_period(
    state: &Arc<AppState>,
    _db: &mut toasty::Db,
    company: &Company,
    request: &impl PeriodRequest,
) -> Result<AccountingPeriod, AppError> {
    // 1. Explicit period wins.
    if let Some(period) = request.period() {
        return Ok(period);
    }

    // 2. Made-up-to date → the 12 months ending on it.
    if let Some(end) = request.made_up_to() {
        return Ok(AccountingPeriod {
            start: end - Months::new(12) + Duration::days(1),
            end,
        });
    }

    // 3. CH next-accounts when a key is set — an enrichment only: a missing
    //    key or a fetch failure falls through to the registration-date guess
    //    rather than failing the report.
    if let Some(period) = resolve_from_ch(state, company).await {
        return Ok(period);
    }

    // 4. Registration-date anniversary schedule (no CH key needed).
    resolve_from_registration(company)
}

/// The chain's CH branch: `Some` only when a key is set, the company has a
/// number, and the profile fetch succeeds.
async fn resolve_from_ch(state: &Arc<AppState>, company: &Company) -> Option<AccountingPeriod> {
    let ch = state.ch.as_ref()?;
    if company.company_number.is_empty() {
        return None;
    }
    let profile = ch.profile(&company.company_number).await.ok()?;

    let today = chrono::Utc::now().date_naive();
    let registration_date = company
        .registration_date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or(today);
    let mut provisional = LibCompany::new("", "", company.company_number.clone());
    provisional.registration_date = registration_date;

    Some(ct600::next_accounting_period_from(&provisional, profile.accounts.as_ref()).period)
}

/// The chain's registration-date branch: the ARD anniversary schedule from
/// the stored registration date (pure — no CH call).
fn resolve_from_registration(company: &Company) -> Result<AccountingPeriod, AppError> {
    let registration_date = company
        .registration_date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .ok_or_else(|| AppError::Validation {
            fields: vec![FieldIssue {
                field: "registration_date".into(),
                reason: "required to guess the return period — set it on the company or pass an explicit period / made_up_to date in the request".into(),
            }],
        })?;
    let mut provisional = LibCompany::new("", "", company.company_number.clone());
    provisional.registration_date = registration_date;
    Ok(ct600::next_accounting_period_from(&provisional, None).period)
}

/// Pure form of steps 1–2 (used by the offline unit tests).
pub fn period_from_request(
    period: Option<AccountingPeriod>,
    made_up_to: Option<NaiveDate>,
) -> Option<AccountingPeriod> {
    if let Some(period) = period {
        return Some(period);
    }
    made_up_to.map(|end| AccountingPeriod {
        start: end - Months::new(12) + Duration::days(1),
        end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_period_wins() {
        let p = AccountingPeriod {
            start: NaiveDate::from_ymd_opt(2023, 4, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2024, 3, 31).unwrap(),
        };
        assert_eq!(period_from_request(Some(p), Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap())), Some(p));
    }

    #[test]
    fn made_up_to_gives_twelve_months_ending_on_it() {
        let end = NaiveDate::from_ymd_opt(2024, 3, 31).unwrap();
        let p = period_from_request(None, Some(end)).unwrap();
        assert_eq!(p.end, end);
        assert_eq!(p.start, NaiveDate::from_ymd_opt(2023, 4, 1).unwrap());
    }

    #[test]
    fn nothing_gives_none() {
        assert_eq!(period_from_request(None, None), None);
    }
}
