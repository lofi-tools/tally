//! Return-period resolution (spec §7), mirroring `tally-cli`'s
//! `ConfigBuilder::resolve_period` chain:
//!
//! 1. an explicit `period` in the request wins;
//! 2. else the `made_up_to` date → the 12 months ending on it;
//! 3. else the company's next accounting period from Companies House
//!    (`next_accounting_period_from` on the stored profile; needs a key).

use std::sync::Arc;

use chrono::{Duration, Months, NaiveDate};
use ixbrl::company::{AccountingPeriod, Company as LibCompany};

use crate::AppState;
use crate::error::AppError;
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
    db: &mut toasty::Db,
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

    // 3. Next accounting period from Companies House (needs a key).
    resolve_from_ch(state, db, company).await
}

/// The chain's CH branch. Exposed separately so the tests can exercise it
/// against a real (or sandbox) API.
pub async fn resolve_from_ch(
    state: &Arc<AppState>,
    _db: &mut toasty::Db,
    company: &Company,
) -> Result<AccountingPeriod, AppError> {
    let ch = state.ch.as_ref().ok_or_else(|| AppError::CompaniesHouseKeyMissing {
        hint: "set COMPANIES_HOUSE_API_KEY (live) or COMPANIES_HOUSE_SANDBOX_API_KEY (sandbox), \
               or pass an explicit period / made_up_to date in the request"
            .into(),
    })?;

    if company.company_number.is_empty() {
        return Err(AppError::MissingCompanyNumber);
    }

    let profile = ch.profile(&company.company_number).await?;

    let today = chrono::Utc::now().date_naive();
    let registration_date = company
        .registration_date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or(today);
    let mut provisional = LibCompany::new("", "", company.company_number.clone());
    provisional.registration_date = registration_date;

    let next = ct600::next_accounting_period_from(&provisional, profile.accounts.as_ref());
    Ok(next.period)
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
