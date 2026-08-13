//! The filings sync (spec: ch-filings-sync-spec.md §4, §5).
//!
//! Two halves:
//!
//! - **`fetch_and_store`** — the `fetch_filings` job body (spawned by the
//!   worker in `jobs.rs`): fetch the company's complete filing history from
//!   Companies House (all pages), persist every item into `filings`, then
//!   download + parse the most recent accounts document and upsert a
//!   `balance_sheets` row. Cancellation is observed between steps (the job
//!   exits at a safe point when the worker is shutting down).
//! - **Handlers** — `GET /companies/{id}/filings` (periods + balance sheets +
//!   fetch status, for the web's two-pane view) and
//!   `POST /companies/{id}/filings/refresh` (re-enqueue the fetch job).

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Months, NaiveDate};
use ixbrl::company::{AccountingPeriod, AccountsMeta, Company as LibCompany};
use ixbrl::reports::uk_frs105_accounts::{Frs105Accounts, PreviousYearFigures};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::AppState;
use crate::auth::AuthUser;
use crate::companies::owned_company;
use crate::companies_house::key_missing_hint;
use crate::error::AppError;
use crate::extract::AppPath;
use crate::jobs;
use crate::models::{BalanceSheet, Company, Filing, Job};

/// RFC 3339 UTC now (same convention as auth.rs / jobs.rs).
fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Run `fut` to completion, unless the worker's shutdown token fires first —
/// the job then fails with a cancellation error (persisted like any other
/// failure; Refresh re-runs it, and the startup re-claim guarantees the row
/// is retried on the next boot).
async fn cancelable<F, T>(token: &CancellationToken, fut: F) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    tokio::select! {
        _ = token.cancelled() => Err(AppError::Internal { message: "job cancelled (worker shutting down)".into() }),
        res = fut => res,
    }
}

// ---------------------------------------------------------------------------
// The fetch job
// ---------------------------------------------------------------------------

/// The `fetch_filings` job body: full filing history (all pages) → `filings`
/// rows; most recent accounts document → `balance_sheets` row.
pub async fn fetch_and_store(
    state: &Arc<AppState>,
    company_id: uuid::Uuid,
    token: &CancellationToken,
) -> Result<(), AppError> {
    let mut db = state.db.clone();
    let company = Company::filter_by_id(company_id)
        .first()
        .exec(&mut db)
        .await?
        .ok_or_else(|| AppError::NotFound {
            resource: "company",
            id: company_id.to_string(),
        })?;

    let ch = state.ch.as_ref().ok_or_else(|| AppError::CompaniesHouseKeyMissing {
        hint: key_missing_hint().into(),
    })?;
    if company.company_number.is_empty() {
        return Err(AppError::MissingCompanyNumber);
    }

    // 1. Complete filing history (all pages, newest first).
    let history = cancelable(token, ch.filing_history_all(&company.company_number)).await?;

    // 2. Persist every item, idempotently (delete + reinsert of the full
    //    snapshot within one transaction; the unique index on
    //    (company_id, ch_transaction_id) keeps the DB consistent).
    let mut tx = db.transaction().await?;
    Filing::filter_by_company_id(company.id).delete().exec(&mut tx).await?;
    BalanceSheet::filter_by_company_id(company.id).delete().exec(&mut tx).await?;
    // CH's paginated history can repeat an item (a filing lands between
    // pages), and the no-link fallback key can collide — dedupe on the
    // transaction id so the unique index never trips.
    let mut filing_ids: BTreeMap<String, uuid::Uuid> = BTreeMap::new();
    for item in &history.items {
        let tx_id = ch_transaction_id(item);
        if filing_ids.contains_key(&tx_id) {
            continue;
        }
        let filing = toasty::create!(Filing {
            company_id: company.id,
            ch_transaction_id: tx_id.clone(),
            category: item.category.clone().unwrap_or_default(),
            form_type: item.form_type.clone().unwrap_or_default(),
            description: item.description.clone().unwrap_or_default(),
            filed_on: item.date.clone(),
            document_metadata_url: item
                .links
                .as_ref()
                .and_then(|l| l.document_metadata.clone())
                .unwrap_or_default(),
            raw: toasty::Json(serde_json::to_value(item).unwrap_or(serde_json::Value::Null)),
            fetched_at: now_rfc3339(),
        })
        .exec(&mut tx)
        .await?;
        filing_ids.insert(tx_id, filing.id);
    }
    tx.commit().await?;

    // 3. The most recent accounts filing with a downloadable document.
    let Some(candidate) = history
        .items
        .iter()
        .filter(|item| {
            item.category.as_deref() == Some("accounts")
                && item.links.as_ref().and_then(|l| l.document_metadata.as_deref()).is_some_and(|u| !u.is_empty())
        })
        .max_by_key(|item| item.filed_on().unwrap_or_default())
    else {
        // No downloadable accounts document (some companies have none) —
        // the history alone is a successful, complete sync.
        return Ok(());
    };
    let metadata_url = candidate
        .links
        .as_ref()
        .and_then(|l| l.document_metadata.clone())
        .expect("filtered for document_metadata");

    // 4. Download the document.
    let bytes = cancelable(token, ch.filing_document(&metadata_url)).await?;

    // 5. Interpret: zipped iXBRL → unzip to the .html; PDF / anything else
    //    unparseable → keep the raw bytes, no figures (partial success).
    let (parsed_document, parsed) = match unzip_ixbrl(&bytes) {
        Some(html) => {
            let parsed = parse_ixbrl(&html, &company);
            (Some(html), parsed)
        }
        None => (None, None),
    };
    let source_filing_id = filing_ids.get(&ch_transaction_id(candidate)).copied();

    // 6. Upsert the balance-sheet row. Unparseable documents still get a
    //    row (raw bytes kept, `parsed_document` null, default figures) so
    //    the period shows as filed; `previous_year_figures` skips rows
    //    without a parsed document, so comparatives stay unavailable.
    let period_end = parsed
        .as_ref()
        .and_then(|p| p.accounts.period.as_ref().map(|period| period.end))
        .or_else(|| candidate.filed_on())
        .unwrap_or_else(|| chrono::Utc::now().date_naive());
    let figures = parsed
        .as_ref()
        .map(figures_from_parsed)
        .unwrap_or_default();
    let mut db = state.db.clone();
    let mut tx = db.transaction().await?;
    toasty::create!(BalanceSheet {
        company_id: company.id,
        period_end: period_end.to_string(),
        filed_on: candidate.date.clone(),
        source_filing_id,
        figures: toasty::Json(figures),
        raw_document: Some(bytes),
        parsed_document,
        created_at: now_rfc3339(),
    })
    .exec(&mut tx)
    .await?;
    tx.commit().await?;

    Ok(())
}

/// The CH transaction id: the trailing path segment of `links.self`
/// (stable across refetches); a deterministic fallback when the link is
/// absent.
fn ch_transaction_id(item: &ct600::companies_house::FilingHistoryItem) -> String {
    item.links
        .as_ref()
        .and_then(|l| l.self_link.as_deref())
        .and_then(|url| url.rsplit('/').next().filter(|s| !s.is_empty()))
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "{}|{}|{}",
                item.category.as_deref().unwrap_or(""),
                item.form_type.as_deref().unwrap_or(""),
                item.date.as_deref().unwrap_or("")
            )
        })
}

/// Unzip a downloaded document to its iXBRL HTML, when it is a zip
/// (CH serves accounts as zipped iXBRL or a single `.html`). `None` when
/// the bytes are not a zip (plain HTML / PDF).
fn unzip_ixbrl(bytes: &[u8]) -> Option<String> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return None;
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).ok()?;
        if entry.name().ends_with(".html") || entry.name().ends_with(".htm") {
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf).ok()?;
            return String::from_utf8(buf).ok();
        }
    }
    None
}

/// Parse a filed accounts iXBRL document back into a `Frs105Accounts`. The
/// supplied company's identity (registration date) is preserved; the return
/// period is recovered from the document itself. `None` when the document
/// is not a parseable FRS 105 iXBRL file.
fn parse_ixbrl(html: &str, company: &Company) -> Option<Frs105Accounts> {
    let mut lib = LibCompany::new(
        company.name.clone(),
        company.tax_reference.clone(),
        company.company_number.clone(),
    );
    lib.registration_date = company
        .registration_date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or_default();
    // A placeholder period is required by the parser's fallback logic; the
    // document's own dates override it.
    let meta = AccountsMeta {
        period: Some(AccountingPeriod {
            start: NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(1970, 12, 31).unwrap(),
        }),
        ..AccountsMeta::default()
    };
    Frs105Accounts::from_ixbrl(html, &lib, &meta).ok()
}

/// The current-period column (index `[0]`) of a parsed report, in the
/// `PreviousYearFigures` shape — the historical balance sheet for the filed
/// period end.
fn figures_from_parsed(parsed: &Frs105Accounts) -> PreviousYearFigures {
    PreviousYearFigures {
        fixed_assets: parsed.fixed_assets[0],
        current_assets: parsed.current_assets[0],
        prepayments_and_accrued_income: parsed.prepayments_and_accrued_income[0],
        creditors_within_1_year: parsed.creditors_within_1_year[0],
        net_current_assets: parsed.net_current_assets[0],
        total_assets_less_liabilities: parsed.total_assets_less_liabilities[0],
        creditors_after_1_year: parsed.creditors_after_1_year[0],
        provisions_for_liabilities: parsed.provisions_for_liabilities[0],
        accruals_and_deferred_income: parsed.accruals_and_deferred_income[0],
        net_assets: parsed.net_assets[0],
        capital_and_reserves: parsed.capital_and_reserves[0],
    }
}

// ---------------------------------------------------------------------------
// Response shapes (§5)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Period {
    /// ISO-8601 (`YYYY-MM-DD`).
    pub start: String,
    /// ISO-8601 (`YYYY-MM-DD`).
    pub end: String,
    /// `filed` | `pending` | `ongoing`.
    pub status: &'static str,
    /// Filing deadlines; `null` for filed periods.
    pub due: Option<PeriodDue>,
    /// The period's filings (confirmed + expected-but-not-sent).
    pub filings: Vec<PeriodFiling>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodDue {
    /// CT600 deadline (HMRC): 12 months after the period end.
    pub hmrc: String,
    /// Accounts deadline (Companies House): 9 months after the period end.
    pub ch: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodFiling {
    /// `accounts` | `confirmation-statement` | `corporation-tax` | `other`.
    pub kind: &'static str,
    /// `confirmed` (filed at CH/HMRC) | `not-sent` (expected, not filed yet).
    pub state: &'static str,
    /// ISO-8601 (`YYYY-MM-DD`); present for confirmed filings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filed_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_metadata_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchStatus {
    /// `none` | `pending` | `running` | `done` | `failed`.
    pub state: &'static str,
    /// RFC 3339 — when the last successful fetch completed.
    pub fetched_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FilingsView {
    pub periods: Vec<Period>,
    pub balance_sheets: Vec<BalanceSheetView>,
    pub status: FetchStatus,
}

/// A `balance_sheets` row without the heavy document columns (the web only
/// needs the period + figures; the raw/parsed documents stay server-side).
#[derive(Debug, Clone, Serialize)]
pub struct BalanceSheetView {
    pub period_end: String,
    pub filed_on: Option<String>,
    pub figures: PreviousYearFigures,
}

impl From<&BalanceSheet> for BalanceSheetView {
    fn from(bs: &BalanceSheet) -> Self {
        Self {
            period_end: bs.period_end.clone(),
            filed_on: bs.filed_on.clone(),
            figures: bs.figures.clone().0,
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /companies/{id}/filings` — the company's financial periods (newest
/// first) with their filings, the stored balance sheets, and the latest
/// fetch job's status.
pub async fn list(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(company_id): AppPath<uuid::Uuid>,
) -> Result<Json<FilingsView>, AppError> {
    let mut db = state.db.clone();
    let company = owned_company(&mut db, user.id, company_id).await?;

    let balance_sheets = BalanceSheet::filter_by_company_id(company.id)
        .exec(&mut db)
        .await?;
    let filings = Filing::filter_by_company_id(company.id)
        .exec(&mut db)
        .await?;
    let job = Job::filter_by_company_id(company.id)
        .order_by(Job::fields().created_at().desc())
        .first()
        .exec(&mut db)
        .await?;

    let periods = derive_periods(&company, &balance_sheets, &filings);
    Ok(Json(FilingsView {
        periods,
        balance_sheets: balance_sheets.iter().map(BalanceSheetView::from).collect(),
        status: fetch_status(job.as_ref()),
    }))
}

/// `POST /companies/{id}/filings/refresh` — re-enqueue the fetch job.
/// 202 + job id when a new job was enqueued; 200 + the in-flight job id when
/// one is already pending/running (deduped by the partial unique index).
/// Errors `companies_house_key_missing` when no CH key is configured.
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(company_id): AppPath<uuid::Uuid>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let mut db = state.db.clone();
    let company = owned_company(&mut db, user.id, company_id).await?;

    if company.company_number.is_empty() {
        return Err(AppError::field(
            "company_number",
            "required to fetch filing history from Companies House",
        ));
    }
    if state.ch.is_none() {
        return Err(AppError::CompaniesHouseKeyMissing {
            hint: key_missing_hint().into(),
        });
    }

    let job_id = jobs::enqueue(&mut db, "fetch_filings", company.id).await?;
    match job_id {
        Some(id) => Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "job_id": id })))),
        // Already in flight — report the existing pending job.
        None => {
            let existing = Job::filter_by_company_id(company.id)
                .order_by(Job::fields().created_at().desc())
                .first()
                .exec(&mut db)
                .await?;
            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "job_id": existing.map(|j| j.id),
                })),
            ))
        }
    }
}

/// The latest job row → the `FetchStatus` view (`none` when never fetched).
fn fetch_status(job: Option<&Job>) -> FetchStatus {
    match job {
        Some(job) => FetchStatus {
            state: match job.status.as_str() {
                "pending" => "pending",
                "running" => "running",
                "done" => "done",
                "failed" => "failed",
                _ => "none",
            },
            fetched_at: (job.status == "done").then(|| job.updated_at.clone()),
            last_error: job.last_error.clone(),
        },
        None => FetchStatus {
            state: "none",
            fetched_at: None,
            last_error: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Period derivation (§5)
// ---------------------------------------------------------------------------

/// The company's financial periods, newest first, derived from the
/// registration-date schedule plus the filed history:
///
/// - **`ongoing`** — the schedule period containing today;
/// - **`filed`** — a period end with a stored balance sheet (or an accounts
///   filing whose period end — parsed from its description — matches);
/// - **`pending`** — an ended period with no confirmed accounts filing.
///
/// When the company has no registration date, the schedule cannot be built,
/// so only filed periods appear (from the balance-sheet / filing history);
/// with neither an anchor nor history, `periods` is empty.
fn derive_periods(company: &Company, balance_sheets: &[BalanceSheet], filings: &[Filing]) -> Vec<Period> {
    let today = chrono::Utc::now().date_naive();

    // The ongoing period: the registration-date ARD schedule period
    // containing today (needs a registration date).
    let reg_date = company
        .registration_date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    let mut ongoing_end: Option<NaiveDate> = None;
    if let Some(reg) = reg_date {
        let mut lib = LibCompany::new("", "", company.company_number.clone());
        lib.registration_date = reg;
        let mut n = 0u32;
        loop {
            let period = lib.accounting_period_n(n);
            if period.start > today {
                break;
            }
            if period.contains(today) {
                ongoing_end = Some(period.end);
                break;
            }
            n += 1;
        }
    }

    // Filed ends: stored balance sheets + accounts filings' period ends.
    let mut filed_ends: Vec<NaiveDate> = Vec::new();
    for bs in balance_sheets {
        if let Some(d) = parse_iso_date(&bs.period_end) {
            filed_ends.push(d);
        }
    }
    for filing in filings {
        if filing.category == "accounts" {
            if let Some(d) = period_end_from_description(&filing.description) {
                filed_ends.push(d);
            }
        }
    }

    // The period list covers the schedule from the earliest filed period up
    // to today, plus any filed end outside the schedule (e.g. shortened
    // periods / changed ARDs). A company with nothing filed shows just the
    // ongoing period — not the whole registration-date history as pending.
    if filed_ends.is_empty() && ongoing_end.is_none() {
        return Vec::new();
    }
    // `unwrap_or_else`, not `unwrap_or`: the `expect` must not fire when a
    // company has no registration date but does have filed history (the
    // balance-sheet/filing ends below still anchor the period list).
    let earliest = filed_ends
        .iter()
        .copied()
        .min()
        .unwrap_or_else(|| ongoing_end.expect("non-empty above"));
    let mut ends: Vec<NaiveDate> = Vec::new();
    if let Some(reg) = reg_date {
        let mut lib = LibCompany::new("", "", company.company_number.clone());
        lib.registration_date = reg;
        // Start from the schedule period covering the earliest filed end
        // (skip the pre-history periods entirely), walk up to today.
        let mut n = 0u32;
        loop {
            let period = lib.accounting_period_n(n);
            if period.end >= earliest {
                break;
            }
            n += 1;
        }
        loop {
            let period = lib.accounting_period_n(n);
            if period.start > today {
                break;
            }
            ends.push(period.end);
            if period.contains(today) {
                break;
            }
            n += 1;
        }
    }
    ends.extend(filed_ends.iter().copied());
    ends.sort_by(|a, b| b.cmp(a)); // newest first
    ends.dedup();
    if ends.is_empty() {
        return Vec::new();
    }

    let mut periods = Vec::with_capacity(ends.len());
    for (i, end) in ends.iter().enumerate() {
        // The period's start is the day after the next-older period's end
        // (the walk is newest-first, so the next-older end is ends[i+1]).
        let start = ends
            .get(i + 1)
            .map(|older| *older + Duration::days(1))
            .unwrap_or_else(|| *end - Months::new(12) + Duration::days(1));
        let is_ongoing = ongoing_end == Some(*end);
        let is_filed = filed_ends.contains(end);
        let status = if is_ongoing {
            "ongoing"
        } else if is_filed {
            "filed"
        } else {
            "pending"
        };
        let due = if is_ongoing || status == "pending" {
            Some(PeriodDue {
                hmrc: (*end + Months::new(12)).to_string(),
                ch: (*end + Months::new(9)).to_string(),
            })
        } else {
            None
        };
        let period_filings = filings_for_period(*end, is_filed, filings);
        periods.push(Period {
            start: start.to_string(),
            end: end.to_string(),
            status,
            due,
            filings: period_filings,
        });
    }
    periods
}

/// The period's filings: confirmed items from the CH history (accounts by
/// their derived period end, other categories by the period containing their
/// filing date) plus the expected-but-not-sent accounts + corporation-tax
/// rows when the period isn't fully filed.
fn filings_for_period(end: NaiveDate, is_filed: bool, filings: &[Filing]) -> Vec<PeriodFiling> {
    let mut out: Vec<PeriodFiling> = Vec::new();
    let mut has_confirmed_accounts = false;
    let mut has_confirmed_corp_tax = false;

    for filing in filings {
        // Accounts filings belong to the period ending on their period end.
        let belongs = if filing.category == "accounts" {
            let derived = period_end_from_description(&filing.description);
            derived == Some(end)
        } else {
            // Other categories: the period containing the filing date.
            filing
                .filed_on
                .as_deref()
                .and_then(parse_iso_date)
                .map(|d| d > end - Months::new(12) && d <= end)
                .unwrap_or(false)
        };
        if !belongs {
            continue;
        }
        let kind = kind_of(&filing.category);
        if kind == "accounts" {
            has_confirmed_accounts = true;
        }
        if kind == "corporation-tax" {
            has_confirmed_corp_tax = true;
        }
        out.push(PeriodFiling {
            kind,
            state: "confirmed",
            filed_on: filing.filed_on.clone(),
            form_type: (!filing.form_type.is_empty()).then(|| filing.form_type.clone()),
            description: (!filing.description.is_empty()).then(|| filing.description.clone()),
            document_metadata_url: (!filing.document_metadata_url.is_empty())
                .then(|| filing.document_metadata_url.clone()),
        });
    }

    // Expected-but-not-sent rows for periods that still need filings.
    if !is_filed {
        if !has_confirmed_accounts {
            out.push(PeriodFiling {
                kind: "accounts",
                state: "not-sent",
                filed_on: None,
                form_type: None,
                description: None,
                document_metadata_url: None,
            });
        }
        if !has_confirmed_corp_tax {
            out.push(PeriodFiling {
                kind: "corporation-tax",
                state: "not-sent",
                filed_on: None,
                form_type: None,
                description: None,
                document_metadata_url: None,
            });
        }
    }
    out
}

/// The API's filing-kind from the CH category.
fn kind_of(category: &str) -> &'static str {
    match category {
        "accounts" => "accounts",
        "confirmation-statement" => "confirmation-statement",
        _ => "other",
    }
}

/// Parse a `YYYY-MM-DD` string.
fn parse_iso_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

/// Derive an accounts filing's period end from its CH description, e.g.
/// `"micro company accounts made up to 31 March 2024"` → 2024-03-31.
fn period_end_from_description(description: &str) -> Option<NaiveDate> {
    let idx = description.find("made up to")?;
    let tail = &description[idx + "made up to".len()..];
    let candidate = tail.trim().trim_end_matches('.');
    // CH renders the date as `31 March 2024`; tolerate a few common shapes.
    for fmt in ["%d %B %Y", "%d %b %Y", "%d/%m/%Y", "%Y-%m-%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(candidate, fmt) {
            return Some(date);
        }
    }
    // Trailing words (rare) — try just the first three tokens.
    let first_three = candidate.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
    for fmt in ["%d %B %Y", "%d %b %Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(&first_three, fmt) {
            return Some(date);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Shared: previous-year comparatives for the reports (§6)
// ---------------------------------------------------------------------------

/// The most recent parsed balance sheet on file, as the previous-year
/// comparative figures for the report (CH wins when present; `None` → the
/// ledger-derived comparatives stay). Rows without a parsed document (PDFs
/// and other unparseable uploads) are skipped so comparatives stay
/// unavailable rather than zero.
pub async fn previous_year_figures(
    db: &mut toasty::Db,
    company_id: uuid::Uuid,
) -> Result<Option<PreviousYearFigures>, AppError> {
    let rows = BalanceSheet::filter_by_company_id(company_id)
        .order_by(BalanceSheet::fields().period_end().desc())
        .exec(db)
        .await?;
    Ok(rows
        .into_iter()
        .find(|bs| bs.parsed_document.is_some())
        .map(|bs| bs.figures.clone().0))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_end_from_description_parses() {
        assert_eq!(
            period_end_from_description("micro company accounts made up to 31 March 2024"),
            NaiveDate::from_ymd_opt(2024, 3, 31)
        );
        assert_eq!(
            period_end_from_description("total exemption full accounts made up to 31/12/2023"),
            NaiveDate::from_ymd_opt(2023, 12, 31)
        );
        assert_eq!(period_end_from_description("confirmation statement made on 13 November 2023"), None);
        assert_eq!(period_end_from_description(""), None);
    }

    #[test]
    fn ch_transaction_id_from_self_link() {
        let item = ct600::companies_house::FilingHistoryItem {
            date: Some("2024-03-31".into()),
            category: Some("accounts".into()),
            form_type: Some("AA".into()),
            description: Some("micro company accounts".into()),
            links: Some(ct600::companies_house::FilingHistoryLinks {
                self_link: Some("/company/00445790/filing-history/MzA1OTg4NDcwMDY5".into()),
                document_metadata: None,
            }),
        };
        assert_eq!(ch_transaction_id(&item), "MzA1OTg4NDcwMDY5");
    }

    #[test]
    fn ch_transaction_id_fallback_when_no_link() {
        let item = ct600::companies_house::FilingHistoryItem {
            date: Some("2024-03-31".into()),
            category: Some("accounts".into()),
            form_type: Some("AA".into()),
            description: Some("x".into()),
            links: None,
        };
        assert_eq!(ch_transaction_id(&item), "accounts|AA|2024-03-31");
    }
}
