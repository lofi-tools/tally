//! Company handlers (CRUD + Companies House search/enrich) and the shared
//! ownership helpers used across the modules.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::AppState;
use crate::auth::AuthUser;
use crate::companies_house::key_missing_hint;
use crate::error::{AppError, FieldIssue};
use crate::extract::{AppJson, AppPath, AppQuery};
use crate::models::{Account, Company, Ledger, Split, Transaction};

// The CLI's fixed defaults for the accounts metadata (config.rs keeps the
// same values; keep in sync).
pub(crate) const DEFAULT_FY1_YEAR: i32 = 2019;
pub(crate) const DEFAULT_FY2_YEAR: i32 = 2020;
pub(crate) const DEFAULT_ACCOUNTING_STANDARDS_DIMENSION: &str = "uk-bus:Micro-entities";
pub(crate) const DEFAULT_ACCOUNTS_TYPE_DIMENSION: &str = "uk-bus:AbridgedAccounts";
pub(crate) const DEFAULT_ACCOUNTS_STATUS_DIMENSION: &str = "uk-bus:AuditExempt-NoAccountantsReport";

/// The update-builder type toasty generates for `Company` (GAT over the
/// borrow of the model row).  `toasty::schema::Model` is the trait; the
/// crate-root `toasty::Model` is only the derive macro.
type CompanyUpdate<'a> = <Company as toasty::schema::Model>::Update<'a>;

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/// The union of `CompanyConfig` + `AccountsConfig` (spec §5): every field
/// optional, so the same shape serves create (absent → default) and PATCH
/// (absent → unchanged).
#[derive(Debug, Default, Deserialize)]
pub struct CompanyInput {
    // identity
    pub name: Option<String>,
    pub tax_reference: Option<String>,
    pub company_number: Option<String>,
    pub registration_date: Option<String>,
    // profile
    pub directors: Option<Vec<String>>,
    pub contact_name: Option<String>,
    pub address_lines: Option<Vec<String>>,
    pub county: Option<String>,
    pub location: Option<String>,
    pub postcode: Option<String>,
    pub email: Option<String>,
    pub phone_country: Option<String>,
    pub phone_area: Option<String>,
    pub phone_number: Option<String>,
    pub website_url: Option<String>,
    pub website_description: Option<String>,
    pub vat_registration: Option<String>,
    pub sic_codes: Option<Vec<String>>,
    pub activities: Option<String>,
    pub jurisdiction: Option<String>,
    pub accountant_name: Option<String>,
    pub accountant_business: Option<String>,
    pub accountant_address: Option<String>,
    pub auditor_name: Option<String>,
    pub auditor_business: Option<String>,
    pub auditor_address: Option<String>,
    pub industry_sector_dimension: Option<String>,
    pub legal_form_dimension: Option<String>,
    pub country_dimension: Option<String>,
    pub contact_country_dimension: Option<String>,
    pub phone_type_dimension: Option<String>,
    pub logo_b64: Option<String>,
    // accounts metadata
    pub accounting_standard: Option<String>,
    pub fy1_year: Option<i32>,
    pub fy2_year: Option<i32>,
    pub associated_companies: Option<i32>,
    pub report_date: Option<String>,
    pub authorised_date: Option<String>,
    pub incorporation_date: Option<String>,
    pub signed_by: Option<String>,
    pub average_employees: Option<HashMap<String, u32>>,
    pub signature_b64: Option<String>,
}

/// The accounting standards the web add dialog offers (spec §6.1).
pub const ACCOUNTING_STANDARDS: [&str; 2] = ["FRS 105", "FRS 102"];
pub const DEFAULT_ACCOUNTING_STANDARD: &str = "FRS 105";

/// A create body must at least name the company.
fn validate_create(input: &CompanyInput) -> Result<(), AppError> {
    if input.name.as_deref().is_none_or(|s| s.trim().is_empty()) {
        return Err(AppError::Validation {
            fields: vec![FieldIssue { field: "name".into(), reason: "required".into() }],
        });
    }
    validate_accounting_standard(input)?;
    Ok(())
}

/// `accounting_standard` must be one of the supported standards when given
/// (create and PATCH).
fn validate_accounting_standard(input: &CompanyInput) -> Result<(), AppError> {
    if let Some(std) = input.accounting_standard.as_deref()
        && !ACCOUNTING_STANDARDS.contains(&std)
    {
        return Err(AppError::Validation {
            fields: vec![FieldIssue {
                field: "accounting_standard".into(),
                reason: format!("must be one of {ACCOUNTING_STANDARDS:?}"),
            }],
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /companies` — the current user's companies.
pub async fn list(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
) -> Result<Json<Vec<Company>>, AppError> {
    let mut db = state.db.clone();
    let companies = Company::filter_by_user_id(user.id)
        .order_by(Company::fields().name().asc())
        .exec(&mut db)
        .await?;
    Ok(Json(companies))
}

/// `POST /companies` — create from config fields; enrich via CH when a
/// number is given and a key is set (config values always win).
pub async fn create(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppJson(input): AppJson<CompanyInput>,
) -> Result<Json<Company>, AppError> {
    validate_create(&input)?;
    let mut input = input;
    let mut db = state.db.clone();

    let company_number = input.company_number.as_deref().unwrap_or("").trim().to_string();
    if !company_number.is_empty() {
        let dup = Company::filter_by_user_id(user.id)
            .filter(Company::fields().company_number().eq(company_number.clone()))
            .first()
            .exec(&mut db)
            .await?;
        if dup.is_some() {
            return Err(AppError::DuplicateCompany { company_number });
        }
    }

    // Enrich blanks from Companies House when a number AND a key are set.
    if !company_number.is_empty()
        && let Some(ch) = state.ch.as_ref()
    {
        let profile = ch.profile(&company_number).await?;
        let officers = ch.officers(&company_number).await.ok();
        input.enrich_from_ch(&profile, officers.as_ref());
    }

    let backfill = !company_number.is_empty() && state.ch.is_some();
    let company = create_row(&mut db, user.id, input, company_number).await?;

    // Fire-and-forget: enqueue the filings backfill without blocking the
    // request. Only when there is a number AND a key — otherwise the
    // refresh endpoint can be used once configured.
    if backfill {
        let mut db = state.db.clone();
        let id = company.id;
        tokio::spawn(async move {
            let _ = crate::jobs::enqueue(&mut db, "fetch_filings", id).await;
        });
    }

    Ok(Json(company))
}

/// `GET /companies/:id` — ownership-scoped.
pub async fn get(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(id): AppPath<uuid::Uuid>,
) -> Result<Json<Company>, AppError> {
    let mut db = state.db.clone();
    let company = owned_company(&mut db, user.id, id).await?;
    Ok(Json(company))
}

/// `PATCH /companies/:id` — partial update of config fields.
pub async fn patch(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(id): AppPath<uuid::Uuid>,
    AppJson(input): AppJson<CompanyInput>,
) -> Result<Json<Company>, AppError> {
    let mut db = state.db.clone();
    let mut company = owned_company(&mut db, user.id, id).await?;
    validate_accounting_standard(&input)?;
    let mut builder = company.update();
    apply_input(&mut builder, input);
    builder.set_updated_at(Some(now_rfc3339()));
    builder.exec(&mut db).await?;
    let company = owned_company(&mut db, user.id, id).await?;
    Ok(Json(company))
}

/// `DELETE /companies/:id` — cascades ledgers (rows + files).
pub async fn delete(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(id): AppPath<uuid::Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let mut db = state.db.clone();
    let company = owned_company(&mut db, user.id, id).await?;

    let mut tx = db.transaction().await?;
    let ledgers = delete_company_and_owned(&mut tx, &company).await?;
    tx.commit().await?;
    remove_ledger_files(&ledgers);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Delete a company and everything it owns (jobs, filings, balance sheets,
/// ledgers + their rows) inside the caller's transaction. Returns the
/// deleted ledgers so the caller can remove the stored files after commit.
/// Shared by the `DELETE` handler and the guest sweep (temp-user spec §6.3).
pub async fn delete_company_and_owned<E: toasty::Executor>(
    exec: &mut E,
    company: &Company,
) -> Result<Vec<Ledger>, AppError> {
    let ledgers = Ledger::filter_by_company_id(company.id).exec(exec).await?;
    // The filings sync tables (jobs / filings / balance sheets) cascade
    // with the company, like the ledgers.
    crate::models::Job::filter_by_company_id(company.id).delete().exec(exec).await?;
    crate::models::Filing::filter_by_company_id(company.id).delete().exec(exec).await?;
    crate::models::BalanceSheet::filter_by_company_id(company.id).delete().exec(exec).await?;
    for ledger in &ledgers {
        Split::filter_by_ledger_id(ledger.id).delete().exec(exec).await?;
        Transaction::filter_by_ledger_id(ledger.id).delete().exec(exec).await?;
        Account::filter_by_ledger_id(ledger.id).delete().exec(exec).await?;
        Ledger::filter_by_id(ledger.id).delete().exec(exec).await?;
    }
    Company::filter_by_id(company.id).delete().exec(exec).await?;
    Ok(ledgers)
}

/// Remove the stored files for deleted ledgers (best-effort, after commit).
pub fn remove_ledger_files(ledgers: &[Ledger]) {
    for ledger in ledgers {
        if !ledger.file_path.is_empty() {
            let _ = std::fs::remove_file(&ledger.file_path);
        }
    }
}

/// `GET /companies/search?q=` — Companies House search (requires a key).
///
/// Deliberately **unprotected**: the web app searches pre-login and the
/// backend holds the CH key (web-api-wiring-spec §7.2). Rate-limiting /
/// abuse is accepted at this stage.
pub async fn search(
    State(state): State<Arc<AppState>>,
    AppQuery(params): AppQuery<SearchParams>,
) -> Result<Json<Vec<crate::companies_house::SearchItem>>, AppError> {
    let q = params.q.unwrap_or_default().trim().to_string();
    if q.is_empty() {
        return Err(AppError::Validation {
            fields: vec![FieldIssue { field: "q".into(), reason: "required".into() }],
        });
    }
    let ch = state.ch.as_ref().ok_or_else(|| AppError::CompaniesHouseKeyMissing {
        hint: key_missing_hint().into(),
    })?;
    let items = ch.search(&q).await?;
    Ok(Json(items))
}

/// `POST /companies/:id/enrich` — fetch CH profile + officers and fill the
/// blank profile fields (config wins).
pub async fn enrich(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(id): AppPath<uuid::Uuid>,
) -> Result<Json<Company>, AppError> {
    let mut db = state.db.clone();
    let mut company = owned_company(&mut db, user.id, id).await?;
    if company.company_number.is_empty() {
        return Err(AppError::Validation {
            fields: vec![FieldIssue { field: "company_number".into(), reason: "required for enrichment".into() }],
        });
    }
    let ch = state.ch.as_ref().ok_or_else(|| AppError::CompaniesHouseKeyMissing {
        hint: key_missing_hint().into(),
    })?;
    let profile = ch.profile(&company.company_number).await?;
    let officers = ch.officers(&company.company_number).await.ok();

    // Preserve what the user already set (config wins); CH fills the blanks.
    let mut patch = CompanyInput {
        registration_date: company.registration_date.clone(),
        incorporation_date: company.incorporation_date.clone(),
        ..Default::default()
    };
    patch.enrich_from_ch(&profile, officers.as_ref());
    let mut builder = company.update();
    apply_input(&mut builder, patch);
    builder.set_updated_at(Some(now_rfc3339()));
    builder.exec(&mut db).await?;

    let company = owned_company(&mut db, user.id, id).await?;
    Ok(Json(company))
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
}

// ---------------------------------------------------------------------------
// Shared helpers (used by ledgers.rs / reports.rs too)
// ---------------------------------------------------------------------------

/// RFC 3339 UTC now (the app's timestamp convention; see auth.rs/jobs.rs).
fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Load a company owned by `user_id`, or 404 (never revealing other users'
/// resources).
pub async fn owned_company(
    db: &mut toasty::Db,
    user_id: uuid::Uuid,
    id: uuid::Uuid,
) -> Result<Company, AppError> {
    Company::filter_by_id(id)
        .filter(Company::fields().user_id().eq(user_id))
        .first()
        .exec(db)
        .await?
        .ok_or_else(|| AppError::NotFound { resource: "company", id: id.to_string() })
}

/// Insert a company row from an input (absent → defaults).
pub async fn create_row(
    db: &mut toasty::Db,
    user_id: uuid::Uuid,
    input: CompanyInput,
    company_number: String,
) -> Result<Company, AppError> {
    let name = input.name.unwrap_or_default();
    let tax_reference = input.tax_reference.unwrap_or_default();
    let company = toasty::create!(Company {
        user_id,
        name,
        tax_reference,
        company_number,
        registration_date: input.registration_date,
        directors: toasty::Json(input.directors.unwrap_or_default()),
        contact_name: input.contact_name,
        address_lines: toasty::Json(input.address_lines.unwrap_or_default()),
        county: input.county,
        location: input.location,
        postcode: input.postcode,
        email: input.email,
        phone_country: input.phone_country,
        phone_area: input.phone_area,
        phone_number: input.phone_number,
        website_url: input.website_url,
        website_description: input.website_description,
        vat_registration: input.vat_registration,
        sic_codes: toasty::Json(input.sic_codes.unwrap_or_default()),
        activities: input.activities,
        jurisdiction: input.jurisdiction,
        accountant_name: input.accountant_name,
        accountant_business: input.accountant_business,
        accountant_address: input.accountant_address,
        auditor_name: input.auditor_name,
        auditor_business: input.auditor_business,
        auditor_address: input.auditor_address,
        industry_sector_dimension: input
            .industry_sector_dimension
            .unwrap_or_else(|| DEFAULT_ACCOUNTING_STANDARDS_DIMENSION.into()),
        legal_form_dimension: input.legal_form_dimension.unwrap_or_default(),
        country_dimension: input.country_dimension.unwrap_or_default(),
        contact_country_dimension: input.contact_country_dimension.unwrap_or_default(),
        phone_type_dimension: input.phone_type_dimension.unwrap_or_default(),
        logo_b64: input.logo_b64,
        accounting_standard: input
            .accounting_standard
            .unwrap_or_else(|| DEFAULT_ACCOUNTING_STANDARD.into()),
        updated_at: Some(now_rfc3339()),
        fy1_year: input.fy1_year.unwrap_or(DEFAULT_FY1_YEAR),
        fy2_year: input.fy2_year.unwrap_or(DEFAULT_FY2_YEAR),
        associated_companies: input.associated_companies,
        report_date: input.report_date,
        authorised_date: input.authorised_date,
        incorporation_date: input.incorporation_date,
        signed_by: input.signed_by,
        average_employees: input.average_employees.map(toasty::Json),
        signature_b64: input.signature_b64,
    })
    .exec(db)
    .await?;
    Ok(company)
}

/// Fill the input's blank profile fields from a CH profile + officers
/// (config wins — CH only supplies what the input omitted), mirroring the
/// CLI's `CompanyConfig::enrich_from_ch`.
impl CompanyInput {
    pub fn enrich_from_ch(
        &mut self,
        profile: &ct600::CompanyProfile,
        officers: Option<&ct600::companies_house::OfficerList>,
    ) {
        if self.directors.as_deref().is_none_or(|d| d.is_empty())
            && let Some(officers) = officers
        {
            self.directors = Some(officers.directors());
        }
        if let Some(office) = &profile.registered_office_address {
            if self.address_lines.as_deref().is_none_or(|l| l.is_empty()) {
                let lines = [
                    office.premises.as_deref(),
                    office.address_line_1.as_deref(),
                    office.address_line_2.as_deref(),
                ]
                .into_iter()
                .flatten()
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
                if !lines.is_empty() {
                    self.address_lines = Some(lines);
                }
            }
            if self.county.as_deref().is_none_or(|s| s.is_empty()) {
                self.county = office.region.clone();
            }
            if self.location.as_deref().is_none_or(|s| s.is_empty()) {
                self.location = office.locality.clone();
            }
            if self.postcode.as_deref().is_none_or(|s| s.is_empty()) {
                self.postcode = office.postal_code.clone();
            }
        }
        if self.sic_codes.as_deref().is_none_or(|s| s.is_empty()) {
            self.sic_codes = profile.sic_codes.clone();
        }
        if self.jurisdiction.as_deref().is_none_or(|s| s.is_empty()) {
            self.jurisdiction = profile.jurisdiction.clone();
        }
        if self.registration_date.as_deref().is_none_or(|s| s.is_empty()) {
            self.registration_date = profile.date_of_creation.clone();
        }
        if self.incorporation_date.as_deref().is_none_or(|s| s.is_empty()) {
            self.incorporation_date = profile.date_of_creation.clone();
        }
    }
}

/// Apply a PATCH input to an update builder (only `Some` fields change).
///
/// Uses the generated `set_*` methods (`&mut self -> &mut Self`); the plain
/// `field(...)` setters consume the builder and can't be called through a
/// borrow.
pub fn apply_input<'a>(builder: &mut CompanyUpdate<'a>, input: CompanyInput) {
    macro_rules! set {
        ($setter:ident, $field:ident) => {
            if let Some(v) = input.$field {
                builder.$setter(v);
            }
        };
    }
    set!(set_name, name);
    set!(set_tax_reference, tax_reference);
    set!(set_company_number, company_number);
    set!(set_registration_date, registration_date);
    set!(set_contact_name, contact_name);
    set!(set_county, county);
    set!(set_location, location);
    set!(set_postcode, postcode);
    set!(set_email, email);
    set!(set_phone_country, phone_country);
    set!(set_phone_area, phone_area);
    set!(set_phone_number, phone_number);
    set!(set_website_url, website_url);
    set!(set_website_description, website_description);
    set!(set_vat_registration, vat_registration);
    set!(set_activities, activities);
    set!(set_jurisdiction, jurisdiction);
    set!(set_accountant_name, accountant_name);
    set!(set_accountant_business, accountant_business);
    set!(set_accountant_address, accountant_address);
    set!(set_auditor_name, auditor_name);
    set!(set_auditor_business, auditor_business);
    set!(set_auditor_address, auditor_address);
    set!(set_industry_sector_dimension, industry_sector_dimension);
    set!(set_legal_form_dimension, legal_form_dimension);
    set!(set_accounting_standard, accounting_standard);
    set!(set_country_dimension, country_dimension);
    set!(set_contact_country_dimension, contact_country_dimension);
    set!(set_phone_type_dimension, phone_type_dimension);
    set!(set_logo_b64, logo_b64);
    set!(set_fy1_year, fy1_year);
    set!(set_fy2_year, fy2_year);
    set!(set_associated_companies, associated_companies);
    set!(set_report_date, report_date);
    set!(set_authorised_date, authorised_date);
    set!(set_incorporation_date, incorporation_date);
    set!(set_signed_by, signed_by);
    set!(set_signature_b64, signature_b64);
    if let Some(v) = input.directors {
        builder.set_directors(toasty::Json(v));
    }
    if let Some(v) = input.address_lines {
        builder.set_address_lines(toasty::Json(v));
    }
    if let Some(v) = input.sic_codes {
        builder.set_sic_codes(toasty::Json(v));
    }
    if let Some(v) = input.average_employees {
        builder.set_average_employees(Some(toasty::Json(v)));
    }
}
