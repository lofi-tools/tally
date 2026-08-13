//! Report generation handlers (spec §6): the four `/reports/*` endpoints.
//!
//! Every report is assembled per-request from the stored ledger rows (via
//! [`GnucashBook::from_raw_parts`]) plus the company's stored config fields,
//! mirroring the CLI's `ct600` flow exactly:
//!
//! - `Frs105Accounts::new(&book, &company, &profile, &meta).to_ixbrl()`
//! - `Frs105CorpTax::builder(&book, &company, &meta).build()`
//! - `Ct600Return::from_inputs(&accounts, &corp_tax).to_xml()`
//!
//! Nothing document-shaped is cached in the DB (spec §6).  All lib calls
//! here are infallible, so the only errors are ownership/period/validation
//! ones (see §11.3).

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use chrono::NaiveDate;
use ixbrl::company::{AccountingPeriod, AccountsMeta, Company as LibCompany, CompanyProfile};
use ixbrl::reports::uk_frs105_accounts::Frs105Accounts;
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;
use ixbrl::{GnucashBook, RawAccount, RawSplit, RawTransaction};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::companies::{
    owned_company, DEFAULT_ACCOUNTING_STANDARDS_DIMENSION,
    DEFAULT_ACCOUNTS_STATUS_DIMENSION, DEFAULT_ACCOUNTS_TYPE_DIMENSION,
};
use crate::error::{AppError, FieldIssue};
use crate::extract::{AppJson, AppPath};
use crate::models::{Account, Company, Ledger, Split, Transaction};
use crate::period::{self, PeriodRequest};

// ---------------------------------------------------------------------------
// Request body
// ---------------------------------------------------------------------------

/// The report request body: `{ ledger_id, period?: {start,end}, made_up_to?:
/// date, declaration?: {...} }` (spec §6).  Period resolution reuses the CLI
/// chain via [`PeriodRequest`].
#[derive(Debug, Deserialize)]
pub struct ReportRequest {
    pub ledger_id: uuid::Uuid,
    pub period: Option<AccountingPeriod>,
    pub made_up_to: Option<NaiveDate>,
    /// CT600 declaration boxes (975 name, 985 status); only used by the
    /// ct600 endpoint.
    pub declaration: Option<DeclarationIn>,
}

#[derive(Debug, Deserialize)]
pub struct DeclarationIn {
    pub name: Option<String>,
    pub status: Option<String>,
}

impl PeriodRequest for ReportRequest {
    fn period(&self) -> Option<AccountingPeriod> {
        self.period
    }
    fn made_up_to(&self) -> Option<NaiveDate> {
        self.made_up_to
    }
}

/// The UTR (tax reference) is the company's HMRC identity: it must be set —
/// and be exactly 10 digits — to generate the corp-tax and CT600 returns.
/// The accounts report carries no UTR and is not gated.
fn require_utr(company: &LibCompany) -> Result<(), AppError> {
    let utr = company.tax_reference.trim();
    if utr.is_empty() {
        return Err(AppError::Validation {
            fields: vec![FieldIssue {
                field: "tax_reference".into(),
                reason: "required to generate this report".into(),
            }],
        });
    }
    if utr.len() != 10 || !utr.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AppError::Validation {
            fields: vec![FieldIssue {
                field: "tax_reference".into(),
                reason: "must be a 10-digit number".into(),
            }],
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /companies/:id/reports/accounts` → FRS 105 accounts iXBRL (`text/html`).
pub async fn accounts(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(company_id): AppPath<uuid::Uuid>,
    AppJson(request): AppJson<ReportRequest>,
) -> Result<Html<String>, AppError> {
    let inputs = load_inputs(&state, user.id, company_id, &request).await?;
    let accounts = Frs105Accounts::new(&inputs.book, &inputs.company, &inputs.profile, &inputs.meta);
    Ok(Html(accounts.to_ixbrl()))
}

/// `POST /companies/:id/reports/corp-tax` → FRS 105 corp-tax iXBRL (`text/html`).
pub async fn corp_tax(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(company_id): AppPath<uuid::Uuid>,
    AppJson(request): AppJson<ReportRequest>,
) -> Result<Html<String>, AppError> {
    let inputs = load_inputs(&state, user.id, company_id, &request).await?;
    require_utr(&inputs.company)?;
    let corp_tax = Frs105CorpTax::builder(&inputs.book, &inputs.company, &inputs.meta).build();
    Ok(Html(corp_tax.to_ixbrl()))
}

/// `POST /companies/:id/reports/corp-tax.json` → the corp-tax figures as JSON
/// (the public getters of [`Frs105CorpTax`]).
pub async fn corp_tax_json(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(company_id): AppPath<uuid::Uuid>,
    AppJson(request): AppJson<ReportRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let inputs = load_inputs(&state, user.id, company_id, &request).await?;
    require_utr(&inputs.company)?;
    let tax = Frs105CorpTax::builder(&inputs.book, &inputs.company, &inputs.meta).build();
    Ok(Json(json!({
        "company_name": tax.company_name(),
        "company_number": tax.company_number(),
        "tax_reference": tax.tax_reference(),
        "type_of_company": tax.type_of_company(),
        "period_start": tax.start().to_string(),
        "period_end": tax.end().to_string(),
        "gross_profit_loss": tax.gross_profit_loss(),
        "turnover_revenue": tax.turnover_revenue(),
        "adjusted_trading_profit": tax.adjusted_trading_profit(),
        "trading_loss": tax.trading_loss(),
        "trading_losses_brought_forward": tax.trading_losses_brought_forward(),
        "net_trading_profits": tax.net_trading_profits(),
        "net_chargeable_gains": tax.net_chargeable_gains(),
        "profits_before_other_deductions_and_reliefs": tax.profits_before_other_deductions_and_reliefs(),
        "profits_before_charges_and_group_relief": tax.profits_before_charges_and_group_relief(),
        "total_profits_chargeable_to_corporation_tax": tax.total_profits_chargeable_to_corporation_tax(),
        "fy1": tax.fy1(),
        "fy2": tax.fy2(),
        "associated_companies": tax.associated_companies(),
        "fy1_profit": tax.fy1_profit(),
        "fy2_profit": tax.fy2_profit(),
        "fy1_tax_rate": tax.fy1_tax_rate(),
        "fy2_tax_rate": tax.fy2_tax_rate(),
        "fy1_tax": tax.fy1_tax(),
        "fy2_tax": tax.fy2_tax(),
        "corporation_tax_chargeable": tax.corporation_tax_chargeable(),
        "tax_chargeable": tax.tax_chargeable(),
        "tax_payable": tax.tax_payable(),
        "sme_rnd_expenditure_deduction": tax.sme_rnd_expenditure_deduction(),
        "investment_allowance": tax.investment_allowance(),
        "repayment": tax.repayment(),
        "claiming_earlier_period_relief": tax.claiming_earlier_period_relief(),
    })))
}

/// `POST /companies/:id/reports/ct600` → CT600 GovTalk XML (`application/xml`).
pub async fn ct600(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(company_id): AppPath<uuid::Uuid>,
    AppJson(request): AppJson<ReportRequest>,
) -> Result<Response, AppError> {
    let inputs = load_inputs(&state, user.id, company_id, &request).await?;
    require_utr(&inputs.company)?;
    let accounts = Frs105Accounts::new(&inputs.book, &inputs.company, &inputs.profile, &inputs.meta);
    let corp_tax = Frs105CorpTax::builder(&inputs.book, &inputs.company, &inputs.meta).build();

    let mut filing = ct600::Ct600Return::from_inputs(&accounts, &corp_tax);
    if let Some(declaration) = &request.declaration {
        let name = declaration
            .name
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| non_empty(inputs.meta.signed_by.clone()))
            .unwrap_or_default();
        let status = declaration
            .status
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Director".to_string());
        if !name.is_empty() {
            filing = filing.with_declaration(name, status);
        }
    }

    let xml = filing.to_xml();
    Ok(([(header::CONTENT_TYPE, "application/xml; charset=utf-8")], xml).into_response())
}

// ---------------------------------------------------------------------------
// Input assembly (shared by all four handlers)
// ---------------------------------------------------------------------------

/// Everything the report builders need, derived from the stored rows.
struct ReportInputs {
    book: GnucashBook,
    company: LibCompany,
    profile: CompanyProfile,
    meta: AccountsMeta,
}

/// Load + rebuild the report inputs: the owned company, the ledger (must
/// belong to the company), the book from its rows, and the resolved period.
async fn load_inputs(
    state: &Arc<AppState>,
    user_id: uuid::Uuid,
    company_id: uuid::Uuid,
    request: &ReportRequest,
) -> Result<ReportInputs, AppError> {
    let mut db = state.db.clone();
    let company = owned_company(&mut db, user_id, company_id).await?;

    // The ledger must exist and belong to this company.
    let ledger = Ledger::filter_by_id(request.ledger_id)
        .first()
        .exec(&mut db)
        .await?
        .ok_or_else(|| AppError::NotFound {
            resource: "ledger",
            id: request.ledger_id.to_string(),
        })?;
    if ledger.company_id != company_id {
        return Err(AppError::CompanyLedgerMismatch {
            company_id: company_id.to_string(),
            ledger_id: request.ledger_id.to_string(),
        });
    }

    // Rebuild the book from the stored rows.
    let accounts = Account::filter_by_ledger_id(ledger.id).exec(&mut db).await?;
    let txns = Transaction::filter_by_ledger_id(ledger.id).exec(&mut db).await?;
    let splits = Split::filter_by_ledger_id(ledger.id).exec(&mut db).await?;

    let raw_accounts = accounts
        .iter()
        .map(|a| RawAccount {
            guid: a.guid.clone(),
            name: a.name.clone(),
            r#type: a.account_type.clone(),
            parent_guid: a.parent_guid.clone(),
        })
        .collect::<Vec<_>>();
    let raw_txns = txns
        .iter()
        .map(|t| RawTransaction {
            guid: t.guid.clone(),
            post_datetime: chrono::DateTime::parse_from_rfc3339(&t.post_datetime)
                .map(|dt| dt.naive_utc())
                .unwrap_or_default(),
            description: t.description.clone(),
        })
        .collect::<Vec<_>>();
    let raw_splits = splits
        .iter()
        .map(|s| RawSplit {
            tx_guid: s.tx_guid.clone(),
            account_guid: s.account_guid.clone(),
            value: s.value,
        })
        .collect::<Vec<_>>();
    let book = GnucashBook::from_raw_parts(raw_accounts, raw_txns, raw_splits);

    // Resolve the return period (spec §7).
    let period = period::resolve_period(state, &mut db, &company, request).await?;

    let lib_company = to_lib_company(&company);
    let profile = to_profile(&company);
    let meta = to_meta(&company, period)?;

    Ok(ReportInputs { book, company: lib_company, profile, meta })
}

/// The stored company row → the ixbrl `Company` (identity + registration
/// date; the epoch is the "unknown" sentinel the libs use).
fn to_lib_company(company: &Company) -> LibCompany {
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
    lib
}

/// The stored company row → the ixbrl `CompanyProfile` (empty strings count
/// as absent for the optional facts, exactly like the CLI's `into_profile`).
fn to_profile(company: &Company) -> CompanyProfile {
    CompanyProfile {
        directors: company.directors.clone().0,
        contact_name: company.contact_name.clone().unwrap_or_default(),
        address_lines: company.address_lines.clone().0,
        county: company.county.clone().filter(|s| !s.is_empty()),
        location: company.location.clone().unwrap_or_default(),
        postcode: company.postcode.clone().unwrap_or_default(),
        email: company.email.clone().filter(|s| !s.is_empty()),
        phone_country: company.phone_country.clone().filter(|s| !s.is_empty()),
        phone_area: company.phone_area.clone().filter(|s| !s.is_empty()),
        phone_number: company.phone_number.clone().filter(|s| !s.is_empty()),
        website_url: company.website_url.clone().filter(|s| !s.is_empty()),
        website_description: company.website_description.clone().filter(|s| !s.is_empty()),
        vat_registration: company.vat_registration.clone().filter(|s| !s.is_empty()),
        sic_codes: company.sic_codes.clone().0,
        activities: company.activities.clone().filter(|s| !s.is_empty()),
        jurisdiction: company.jurisdiction.clone().unwrap_or_default(),
        accountant_name: company.accountant_name.clone().unwrap_or_default(),
        accountant_business: company.accountant_business.clone().unwrap_or_default(),
        accountant_address: company.accountant_address.clone().unwrap_or_default(),
        auditor_name: company.auditor_name.clone().unwrap_or_default(),
        auditor_business: company.auditor_business.clone().unwrap_or_default(),
        auditor_address: company.auditor_address.clone().unwrap_or_default(),
        industry_sector_dimension: company.industry_sector_dimension.clone().unwrap_or_default(),
        legal_form_dimension: company.legal_form_dimension.clone().unwrap_or_default(),
        country_dimension: company.country_dimension.clone().unwrap_or_default(),
        contact_country_dimension: company.contact_country_dimension.clone().unwrap_or_default(),
        phone_type_dimension: company.phone_type_dimension.clone().unwrap_or_default(),
        logo_b64: company.logo_b64.clone(),
    }
}

/// The stored company row + resolved period → the ixbrl `AccountsMeta`,
/// mirroring the CLI's `into_meta` (employee counts default to 1 per
/// financial year; the signatory defaults to the first director).
fn to_meta(company: &Company, period: AccountingPeriod) -> Result<AccountsMeta, AppError> {
    let report_date = required_date(company, "report_date", company.report_date.as_deref())?;
    let authorised_date = required_date(company, "authorised_date", company.authorised_date.as_deref())?;
    let incorporation_date = company
        .incorporation_date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or_default();
    let signed_by = company
        .signed_by
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| company.directors.first().cloned())
        .unwrap_or_default();

    let mut average_employees: HashMap<String, u32> = company
        .average_employees
        .clone()
        .map(|j| j.0)
        .unwrap_or_default();
    average_employees.entry(company.fy1_year.to_string()).or_insert(1);
    average_employees.entry(company.fy2_year.to_string()).or_insert(1);

    Ok(AccountsMeta {
        period: Some(period),
        accounts_made_up_to: None,
        fy1_year: company.fy1_year,
        fy2_year: company.fy2_year,
        associated_companies: company.associated_companies.unwrap_or(0) as u32,
        report_date,
        authorised_date,
        incorporation_date,
        signed_by,
        average_employees,
        accounting_standards_dimension: DEFAULT_ACCOUNTING_STANDARDS_DIMENSION.into(),
        accounts_type_dimension: DEFAULT_ACCOUNTS_TYPE_DIMENSION.into(),
        accounts_status_dimension: DEFAULT_ACCOUNTS_STATUS_DIMENSION.into(),
        signature_b64: company.signature_b64.clone().unwrap_or_default(),
    })
}

/// `Some(s)` when `s` is non-empty (for `Option<String>`-style fallbacks
/// where the string itself is the value).
fn non_empty(s: String) -> Option<String> {
    (!s.is_empty()).then_some(s)
}

/// A required `YYYY-MM-DD` date, or a field-level 422 (`validation_failed`).
fn required_date(company: &Company, field: &'static str, raw: Option<&str>) -> Result<NaiveDate, AppError> {
    raw.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .ok_or_else(|| AppError::Validation {
            fields: vec![FieldIssue {
                field: field.to_string(),
                reason: format!(
                    "required to generate this report (set it on the company first; company '{}')",
                    company.id
                ),
            }],
        })
}

