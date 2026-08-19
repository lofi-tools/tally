//! Configuration for the `tally` CLI.
//!
//! The three input sources — the environment ([`EnvVars`]), the JSON config
//! file ([`ConfigFile`]) and the subcommand's CLI values ([`CliArgs`]) — are
//! loaded by a [`ConfigBuilder`] in precedence order (CLI flags first, the
//! environment overriding the config file) and resolved into a strict
//! [`ResolvedInputs`], erroring on the first field the sources cannot
//! provide.  The config file's optional fields — the identity and the
//! descriptive profile ([`CompanyConfig`]) — are enriched here, from the
//! environment / Companies House or to blank defaults, into the required
//! report types ([`CompanyProfile`], [`AccountsMeta`]) before the
//! libraries consume them.  [`AccountsConfig`] carries the accounts' return
//! period (resolvable), financial-year parameters (defaulted) and the report
//! metadata that cannot be inferred (required in the config file).
//!
//! Precedence, field by field:
//!
//! | Field | Sources (priority order) |
//! |-------|--------------------------|
//! | `company.name` | config file → Companies House (needs an API key + company number) |
//! | `company.company_number` | `COMPANY_NUMBER` (env, wins) → config file |
//! | `company.tax_reference` (UTR) | `COMPANY_UNIQUE_TAXPAYER_REF` (env, wins) → config file |
//! | `accounts.period` | config file (both dates) → deduced from the made-up-to date (`--accounts-made-up-to` wins over `accounts.accounts_made_up_to`; the 12 months ending on it) → Companies House next accounting period |
//! | `accounts.accounts_made_up_to` / `--accounts-made-up-to` | command line (flag, wins) → config file; deduces the return period as the 12 months ending on the date |
//! | `accounts.fy1_year` / `fy2_year` | config file → defaults (2019 / 2020) |
//! | `company.registration_date` | Companies House (only when the config carries no identity at all) → [`Company::new`] default |
//! | Companies House layer | `COMPANIES_HOUSE_API_KEY` (live) / `COMPANIES_HOUSE_SANDBOX_API_KEY` (sandbox); response cache in `CT600_CACHE_DIR` (env) |
//! | `company.*` profile fields (directors, contacts, accountant/auditor, ...) | config file (wins) → Companies House when absent (registered-office address, SIC codes, jurisdiction, directors) → blank defaults |
//! | `accounts.*` unguessable report metadata (report_date, authorised_date, signature_b64) | config file only (required) |
//! | `accounts.signed_by` | config file (optional) → defaults to the first director |
//! | `accounts.average_employees` | config file (optional) → defaults to 1 for each of the two financial years |
//! | `accounts.incorporation_date` | config file → Companies House profile when absent |
//! | `accounts.*` taxonomy dimensions | defaulted to the values fixed for this report |
//! | `--book` | command line only (required) |
//! | `--out` | command line (optional) → the tally repo's `.cache/tally-cli` when run from the checkout, else `~/.cache/tally-cli` |

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{Duration, Months, NaiveDate};
use ct600::companies_house::{
    CompanyProfile as ChProfile, OfficerList, next_accounting_period_from,
};
use ct600::{CompaniesHouseClient, CompaniesHouseClientType};
use reports::company::{AccountingPeriod, AccountsMeta, Company, CompanyProfile};
use serde::{Deserialize, Serialize};

/// The repository root directory, for tests that read committed fixtures
/// and write to `.cache` without depending on the working directory
/// (from the shared `test_utils` crate).
#[cfg(test)]
mod test_utils {
    pub use test_utils::{REPO, cache_dir};
}

/// The values `tally` reads from the environment, captured once via
/// [`EnvVars::from_env`] (empty variables count as unset).
#[derive(Debug, Clone, Default)]
pub struct EnvVars {
    /// `COMPANY_UNIQUE_TAXPAYER_REF` — the Corporation Tax reference (UTR); wins
    /// over the config file's `company.tax_reference`.
    pub unique_taxpayer_ref: Option<String>,
    /// `COMPANY_NUMBER` — the company number; wins over the config file's
    /// `company.company_number`.
    pub company_number: Option<String>,
    /// `COMPANIES_HOUSE_API_KEY` — the live Companies House API key.
    pub companies_house_api_key: Option<String>,
    /// `COMPANIES_HOUSE_SANDBOX_API_KEY` — the sandbox Companies House API key.
    pub companies_house_sandbox_api_key: Option<String>,
    /// `CT600_CACHE_DIR` — the Companies House response-cache directory.
    pub cache_dir: Option<PathBuf>,
}

impl EnvVars {
    /// Snapshot the environment; empty variables count as unset.
    pub fn from_env() -> Self {
        Self {
            unique_taxpayer_ref: non_empty_env("COMPANY_UNIQUE_TAXPAYER_REF"),
            company_number: non_empty_env("COMPANY_NUMBER"),
            companies_house_api_key: non_empty_env("COMPANIES_HOUSE_API_KEY"),
            companies_house_sandbox_api_key: non_empty_env("COMPANIES_HOUSE_SANDBOX_API_KEY"),
            cache_dir: non_empty_env("CT600_CACHE_DIR").map(PathBuf::from),
        }
    }

    /// Which Companies House API the captured keys configure: live preferred
    /// over sandbox; `None` when neither is set.
    pub fn companies_house_client_type(&self) -> Option<CompaniesHouseClientType> {
        if self.companies_house_api_key.is_some() {
            Some(CompaniesHouseClientType::Live)
        } else if self.companies_house_sandbox_api_key.is_some() {
            Some(CompaniesHouseClientType::Sandbox)
        } else {
            None
        }
    }

    /// Whether a Companies House API key is configured.
    pub fn api_key_configured(&self) -> bool {
        self.companies_house_client_type().is_some()
    }

    /// The Companies House client configuration from the captured values
    /// (never re-reads the environment).
    pub fn ch_config(&self) -> ct600::Config {
        // Start from a bare config so the ambient environment cannot leak in.
        let mut config = ct600::Config::default()
            .with_company_number(self.company_number.clone().unwrap_or_default());
        if let Some(cache_dir) = &self.cache_dir {
            config = config.with_cache_dir(cache_dir.clone());
        }
        if let Some(client_type) = self.companies_house_client_type() {
            let api_key = match client_type {
                CompaniesHouseClientType::Live => self.companies_house_api_key.as_deref(),
                CompaniesHouseClientType::Sandbox => {
                    self.companies_house_sandbox_api_key.as_deref()
                }
            };
            if let Some(api_key) = api_key {
                config = config.with_api(api_key, client_type == CompaniesHouseClientType::Sandbox);
            }
        }
        config
    }
}

/// A non-empty environment variable, if set.
fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// The JSON config file's contents (`--config-path`): the nested `company`
/// block and the `accounts` sub-object.
///
/// The `company` block is optional — the identity is enriched from the
/// environment / Companies House and the descriptive profile to blank
/// defaults; the `accounts` sub-object is required, because its report
/// metadata cannot be inferred.  [`ConfigBuilder::build`] resolves what is
/// still missing into the required report types before the libraries
/// consume them.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigFile {
    /// The `company` block: the optional identity fields plus the optional
    /// descriptive profile (directors, contacts, accountant/auditor, ...).
    #[serde(default)]
    pub company: CompanyConfig,
    /// The `accounts` sub-object ([`AccountsConfig`]): the return period,
    /// financial-year parameters and the required report metadata.
    pub accounts: AccountsConfig,
}

impl ConfigFile {
    /// Parse the config file at `path` (`--config-path`).
    pub fn from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("read config '{}'", path.display()))?;
        // Lenient parse (serde_json_lenient): the config files may carry
        // `//` comments and trailing commas (JSONC), so users can annotate
        // their config without breaking parsing.
        serde_json_lenient::from_str(&data)
            .with_context(|| format!("parse config '{}'", path.display()))
    }
}

/// The `company` block of the config file (`company.*`): the optional
/// identity fields plus the optional descriptive profile (directors,
/// contacts, accountant/auditor, ...).  Every field is optional and
/// serialises back as omitted when absent.  The builder enriches the
/// identity into the required [`Company`]; [`Self::enrich_from_ch`] fills
/// the descriptive fields the config left absent from the Companies House
/// profile and officers (registered-office address, SIC codes,
/// jurisdiction, directors), and [`Self::into_profile`] turns the result
/// into the required [`CompanyProfile`] the reports consume.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompanyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tax_reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directors: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address_lines: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub county: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postcode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_area: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vat_registration: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sic_codes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activities: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accountant_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accountant_business: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accountant_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auditor_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auditor_business: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auditor_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub industry_sector_dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_form_dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country_dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_country_dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_type_dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_b64: Option<String>,
}

impl CompanyConfig {
    /// Fill the descriptive profile fields the config left absent (empty
    /// counts as absent) from a resolved Companies House profile and its
    /// officers: the registered-office address (lines, county, location,
    /// postcode), the SIC codes, the jurisdiction and the current
    /// directors.  Config values always win — Companies House only supplies
    /// what the config omitted.  Fields Companies House does not hold
    /// (contacts, accountant/auditor, dimensions, logo) stay as configured.
    pub(crate) fn enrich_from_ch(
        &mut self,
        profile: Option<&ChProfile>,
        officers: Option<&OfficerList>,
    ) {
        let Some(profile) = profile else { return };
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
                .filter(|line| !line.is_empty())
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
    }

    /// Fill the optional profile into the required report profile: absent
    /// fields become blank defaults (empty strings / empty lists, no logo).
    /// The voluntary facts (registered-office county, VAT registration
    /// number, business activities, e-mail, phone, website) stay `None`
    /// when the config omitted them (empty counts as absent), so the report
    /// omits their facts entirely.
    pub(crate) fn into_profile(self) -> CompanyProfile {
        CompanyProfile {
            directors: self.directors.unwrap_or_default(),
            contact_name: self.contact_name.unwrap_or_default(),
            address_lines: self.address_lines.unwrap_or_default(),
            county: self.county.filter(|s| !s.is_empty()),
            location: self.location.unwrap_or_default(),
            postcode: self.postcode.unwrap_or_default(),
            email: self.email.filter(|s| !s.is_empty()),
            phone_country: self.phone_country.filter(|s| !s.is_empty()),
            phone_area: self.phone_area.filter(|s| !s.is_empty()),
            phone_number: self.phone_number.filter(|s| !s.is_empty()),
            website_url: self.website_url.filter(|s| !s.is_empty()),
            website_description: self.website_description.filter(|s| !s.is_empty()),
            vat_registration: self.vat_registration.filter(|s| !s.is_empty()),
            sic_codes: self.sic_codes.unwrap_or_default(),
            activities: self.activities.filter(|s| !s.is_empty()),
            jurisdiction: self.jurisdiction.unwrap_or_default(),
            accountant_name: self.accountant_name.unwrap_or_default(),
            accountant_business: self.accountant_business.unwrap_or_default(),
            accountant_address: self.accountant_address.unwrap_or_default(),
            auditor_name: self.auditor_name.unwrap_or_default(),
            auditor_business: self.auditor_business.unwrap_or_default(),
            auditor_address: self.auditor_address.unwrap_or_default(),
            industry_sector_dimension: self.industry_sector_dimension.unwrap_or_default(),
            legal_form_dimension: self.legal_form_dimension.unwrap_or_default(),
            country_dimension: self.country_dimension.unwrap_or_default(),
            contact_country_dimension: self.contact_country_dimension.unwrap_or_default(),
            phone_type_dimension: self.phone_type_dimension.unwrap_or_default(),
            logo_b64: self.logo_b64,
        }
    }
}

/// The `accounts` sub-object of the config file (`accounts.*`): the return
/// period, financial-year parameters and report metadata.
///
/// What can be guessed or inferred stays optional or defaulted: the return
/// period comes from [`Self::period`] / [`Self::accounts_made_up_to`] or the
/// company's next accounting period at Companies House, the fy years and
/// the accounts taxonomy dimensions default to the values fixed for this
/// report, the incorporation date is filled from the Companies House
/// profile when absent, the signatory defaults to the first director, and
/// the employee counts default to 1 for each of the two financial years.
/// The fields that cannot be inferred — the publication and authorisation
/// dates and the signature — are required here.  [`Self::into_meta`]
/// converts the whole thing into the required [`AccountsMeta`] the reports
/// consume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountsConfig {
    /// The return period; resolved from [`Self::accounts_made_up_to`] or the
    /// company's next accounting period at Companies House when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<AccountingPeriod>,
    /// A date at which the accounts are made; deduces the return period as
    /// the 12 months ending on it (an alternative to [`Self::period`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accounts_made_up_to: Option<NaiveDate>,
    #[serde(default = "default_fy1_year")]
    pub fy1_year: i32,
    #[serde(default = "default_fy2_year")]
    pub fy2_year: i32,
    /// Number of associated companies (excluding the company itself);
    /// divides the marginal-relief limits by this count plus one (optional;
    /// `0` — a standalone company — when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associated_companies: Option<u32>,
    /// Date the report was published / issued (required: cannot be inferred).
    pub report_date: NaiveDate,
    /// Date the financial statements were authorised for issue (required).
    pub authorised_date: NaiveDate,
    /// Date of incorporation / formation; filled from the Companies House
    /// profile when absent (so it may be left out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incorporation_date: Option<NaiveDate>,
    /// Name of the director who signed the report; defaults to the first
    /// director when absent (so it may be left out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_by: Option<String>,
    /// Average monthly number of employees, indexed by calendar year
    /// (optional; each of the two financial years defaults to 1 when not
    /// specified).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_employees: Option<HashMap<String, u32>>,
    /// Accounts taxonomy dimension values, fixed for this report.
    #[serde(default = "default_accounting_standards_dimension")]
    pub accounting_standards_dimension: String,
    #[serde(default = "default_accounts_type_dimension")]
    pub accounts_type_dimension: String,
    #[serde(default = "default_accounts_status_dimension")]
    pub accounts_status_dimension: String,
    /// Base64-encoded director's signature (required; `""` for none).
    pub signature_b64: String,
}

impl AccountsConfig {
    /// Convert into the required report set of accounts: the required
    /// metadata and the defaulted fields carry through, and the optional
    /// incorporation date and signatory fall back to blank defaults (the
    /// builder later fills the signatory from the first director when the
    /// config omitted it).  The employee counts default to 1 for each of
    /// the two financial years.  The period is left as given — the builder
    /// sets the resolved return period.
    pub(crate) fn into_meta(self) -> AccountsMeta {
        AccountsMeta {
            period: self.period,
            accounts_made_up_to: self.accounts_made_up_to,
            fy1_year: self.fy1_year,
            fy2_year: self.fy2_year,
            associated_companies: self.associated_companies.unwrap_or(0),
            report_date: self.report_date,
            authorised_date: Some(self.authorised_date),
            incorporation_date: self.incorporation_date.unwrap_or_default(),
            signed_by: self.signed_by.unwrap_or_default(),
            // The employee counts default to 1 for each of the two financial
            // years; a year explicitly given in the config wins.
            average_employees: {
                let mut employees = self.average_employees.unwrap_or_default();
                employees.entry(self.fy1_year.to_string()).or_insert(1);
                employees.entry(self.fy2_year.to_string()).or_insert(1);
                employees
            },
            accounting_standards_dimension: self.accounting_standards_dimension,
            accounts_type_dimension: self.accounts_type_dimension,
            accounts_status_dimension: self.accounts_status_dimension,
            // The config requires the field (`""` for none): normalise the
            // empty-string sentinel to an explicit `None`.
            signature_b64: (!self.signature_b64.is_empty()).then_some(self.signature_b64),
        }
    }
}

// The default financial-year tax parameters, mirroring `AccountsMeta`'s
// defaults in libs/ixbrl (keep the two in sync), and the accounts taxonomy
// dimension values fixed for the FRS 105 micro-entity accounts report
// (defaulted here in the config layer; ixbrl's `AccountsMeta::default`
// keeps them blank for its library-only paths).
const DEFAULT_FY1_YEAR: i32 = 2019;
const DEFAULT_FY2_YEAR: i32 = 2020;
const DEFAULT_ACCOUNTING_STANDARDS_DIMENSION: &str = "uk-bus:Micro-entities";
const DEFAULT_ACCOUNTS_TYPE_DIMENSION: &str = "uk-bus:AbridgedAccounts";
const DEFAULT_ACCOUNTS_STATUS_DIMENSION: &str = "uk-bus:AuditExempt-NoAccountantsReport";

fn default_fy1_year() -> i32 {
    DEFAULT_FY1_YEAR
}
fn default_fy2_year() -> i32 {
    DEFAULT_FY2_YEAR
}
fn default_accounting_standards_dimension() -> String {
    DEFAULT_ACCOUNTING_STANDARDS_DIMENSION.into()
}
fn default_accounts_type_dimension() -> String {
    DEFAULT_ACCOUNTS_TYPE_DIMENSION.into()
}
fn default_accounts_status_dimension() -> String {
    DEFAULT_ACCOUNTS_STATUS_DIMENSION.into()
}

/// The `ct600` subcommand's command-line values.
#[derive(Debug, Clone)]
pub struct CliArgs {
    /// The `--config-path` value: the JSON config file to load (required).
    pub config_path: PathBuf,
    /// The `--accounts-made-up-to` value: a date at which the accounts are
    /// made; the return period is deduced as the 12 months ending on it
    /// (wins over the config file's `accounts.accounts_made_up_to`).
    pub accounts_made_up_to: Option<NaiveDate>,
    /// The `--book` value (required; no other source).
    pub book_path: Option<PathBuf>,
    /// The `--out` value (optional; defaults to the tally repo's
    /// `.cache/tally-cli` when run from the checkout, else
    /// `~/.cache/tally-cli`).
    pub out_dir: Option<PathBuf>,
}

/// Loads the three input sources and resolves them into a strict
/// [`ResolvedInputs`].
///
/// [`ConfigBuilder::from_cli`] loads the sources in precedence order: the CLI
/// flags first, then the environment overriding the config file.
/// [`ConfigBuilder::build`] resolves what is still missing — enriching from
/// the Companies House API (cache-first) when a client is configured — and
/// errors on the first field the sources cannot provide.
#[derive(Debug)]
pub struct ConfigBuilder {
    /// The captured environment.
    pub env: EnvVars,
    /// The parsed config file.
    pub file: ConfigFile,
    /// The subcommand's CLI values.
    pub cli: CliArgs,
}

impl ConfigBuilder {
    /// Load the environment and the config file for `cli` (in precedence
    /// order: CLI flags, then the environment over the config file).
    pub fn from_cli(cli: CliArgs) -> Result<Self> {
        let env = EnvVars::from_env();
        let file = ConfigFile::from_file(&cli.config_path)?;
        Ok(Self { env, file, cli })
    }

    /// Resolve the merged inputs into a strict [`ResolvedInputs`].
    ///
    /// Each resolution concern is a sub-method — [`ConfigBuilder::resolve_company_number`],
    /// [`ConfigBuilder::resolve_identity`], [`ConfigBuilder::resolve_period`],
    /// [`ConfigBuilder::resolve_paths`] — and errors on the first input the
    /// sources cannot provide.  The descriptive profile fields the config
    /// left absent are then filled from the fetched Companies House profile
    /// and officers (best-effort), before the required report types are
    /// built.
    pub async fn build(mut self) -> Result<ResolvedInputs> {
        let ch = self.env.ch_config();
        let company_number = self.resolve_company_number()?;
        let api = self
            .env
            .api_key_configured()
            .then(|| CompaniesHouseClient::new(ch.with_company_number(company_number.clone())));
        // The profile is fetched at most once and shared across the
        // sub-methods below.
        let mut ch_profile: Option<ChProfile> = None;
        let (name, tax_reference, mut registration_date) = self
            .resolve_identity(&company_number, api.as_ref(), &mut ch_profile)
            .await?;
        let period = self
            .resolve_period(
                api.as_ref(),
                &company_number,
                &mut registration_date,
                &mut ch_profile,
            )
            .await?;
        let (book_path, out_dir) = self.resolve_paths()?;

        // The current directors are enriched from the officers list when the
        // config left them absent and a profile was fetched (best-effort: a
        // failed officers fetch is logged and the directors stay as
        // configured).
        let officers = match (&api, &ch_profile) {
            (Some(client), Some(_))
                if self
                    .file
                    .company
                    .directors
                    .as_deref()
                    .is_none_or(|d| d.is_empty()) =>
            {
                match client.get_officers(&company_number).await {
                    Ok(officers) => Some(officers),
                    Err(e) => {
                        log::warn!("failed to fetch the directors from Companies House: {e}");
                        None
                    }
                }
            }
            _ => None,
        };

        // The config's optional fields are enriched into the required report
        // types here: the profile fills blanks from the fetched profile (+ the
        // officers), and the accounts carry the resolved return period.  The
        // incorporation date is filled from the shared profile when the
        // config omits it.
        let incorporation_from_config = self.file.accounts.incorporation_date;
        let mut accounts = self.file.accounts.into_meta();
        accounts.period = Some(period);
        if incorporation_from_config.is_none()
            && let Some(profile) = ch_profile.as_ref()
            && let Some(created) = profile.date_of_creation.as_deref()
            && let Ok(date) = NaiveDate::parse_from_str(created, "%Y-%m-%d")
        {
            accounts.incorporation_date = date;
        }

        let mut company = Company::new(name, tax_reference, company_number);
        if let Some(date) = registration_date {
            company.registration_date = date;
        }
        self.file
            .company
            .enrich_from_ch(ch_profile.as_ref(), officers.as_ref());
        // The signatory defaults to the first director once the directors are
        // resolved (empty counts as absent).
        if accounts.signed_by.is_empty() {
            accounts.signed_by = self
                .file
                .company
                .directors
                .as_deref()
                .and_then(|d| d.first())
                .cloned()
                .unwrap_or_default();
        }

        Ok(ResolvedInputs {
            company,
            profile: self.file.company.into_profile(),
            accounts,
            book_path,
            out_dir,
            companies_house_client_type: self.env.companies_house_client_type(),
        })
    }

    /// The company number for the Companies House lookups: `COMPANY_NUMBER`
    /// wins over the config file.
    fn resolve_company_number(&self) -> Result<String> {
        self.env
            .company_number
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                self.file
                    .company
                    .company_number
                    .as_deref()
                    .filter(|s| !s.is_empty())
            })
            .map(str::to_string)
            .ok_or_else(|| {
                missing(
                    &self.cli.config_path,
                    "company.company_number",
                    "set the COMPANY_NUMBER environment variable (which wins) or \
                     company.company_number in the config file",
                )
            })
    }

    /// Resolve the company identity: the name, the Corporation Tax reference
    /// (UTR) and the registration date.
    ///
    /// The name is enriched from the profile when the config carries none;
    /// the registration date only when the config had no identity at all, so
    /// partial inputs don't skew the accounting periods.  The UTR cannot
    /// come from Companies House: `COMPANY_UNIQUE_TAXPAYER_REF` wins over
    /// the config file.
    async fn resolve_identity(
        &self,
        company_number: &str,
        api: Option<&CompaniesHouseClient>,
        ch_profile: &mut Option<ChProfile>,
    ) -> Result<(String, String, Option<NaiveDate>)> {
        // Empty strings count as absent (library convention).
        let name_from_config = self.file.company.name.as_deref().filter(|s| !s.is_empty());
        let number_from_config = self
            .file
            .company
            .company_number
            .as_deref()
            .filter(|s| !s.is_empty());

        let mut name = name_from_config.map(str::to_string);
        let mut registration_date = None;
        let details_fully_absent = name_from_config.is_none() && number_from_config.is_none();
        if name.is_none()
            && let Some(client) = api
        {
            let profile = fetch_profile(client, company_number, ch_profile).await?;
            name = Some(profile.company_name.clone());
            if details_fully_absent
                && let Some(created) = profile.date_of_creation.as_deref()
                && let Ok(date) = NaiveDate::parse_from_str(created, "%Y-%m-%d")
            {
                registration_date = Some(date);
            }
        }
        let name = name.ok_or_else(|| {
            missing(
                &self.cli.config_path,
                "company.name",
                "no Companies House API key is set, so it cannot be resolved from \
                 Companies House (set COMPANIES_HOUSE_API_KEY or \
                 COMPANIES_HOUSE_SANDBOX_API_KEY), or add company.name to the config file",
            )
        })?;

        let tax_reference = self
            .env
            .unique_taxpayer_ref
            .as_deref()
            .or_else(|| {
                self.file
                    .company
                    .tax_reference
                    .as_deref()
                    .filter(|s| !s.is_empty())
            })
            .map(str::to_string)
            .ok_or_else(|| {
                missing(
                    &self.cli.config_path,
                    "company.tax_reference",
                    "the Corporation Tax reference (UTR) cannot be resolved from \
                     Companies House; set the COMPANY_UNIQUE_TAXPAYER_REF environment \
                     variable or add company.tax_reference to the config file",
                )
            })?;

        Ok((name, tax_reference, registration_date))
    }

    /// Resolve the return period.
    ///
    /// An explicit `accounts.period` wins; else the made-up-to date (the
    /// `--accounts-made-up-to` flag wins over `accounts.accounts_made_up_to`)
    /// gives the 12 months ending on it; else the next accounting period is
    /// computed from the shared profile ([`next_accounting_period_from`]),
    /// which also supplies the registration date when still unknown.
    async fn resolve_period(
        &self,
        api: Option<&CompaniesHouseClient>,
        company_number: &str,
        registration_date: &mut Option<NaiveDate>,
        ch_profile: &mut Option<ChProfile>,
    ) -> Result<AccountingPeriod> {
        if let Some(period) = self.file.accounts.period {
            return Ok(period);
        }

        let made_up_to = self
            .cli
            .accounts_made_up_to
            .or(self.file.accounts.accounts_made_up_to);
        if let Some(end) = made_up_to {
            return Ok(AccountingPeriod {
                start: end - Months::new(12) + Duration::days(1),
                end,
            });
        }

        if let Some(client) = api {
            let profile = fetch_profile(client, company_number, ch_profile).await?;
            if registration_date.is_none()
                && let Some(created) = profile.date_of_creation.as_deref()
                && let Ok(created) = NaiveDate::parse_from_str(created, "%Y-%m-%d")
            {
                *registration_date = Some(created);
            }
            let today = chrono::Utc::now().date_naive();
            let mut provisional = Company::new("", "", company_number.to_string());
            provisional.registration_date = registration_date.unwrap_or(today);
            // Reuse the shared profile (no second lookup).
            let next = next_accounting_period_from(&provisional, profile.accounts.as_ref());
            return Ok(next.period);
        }

        Err(missing(
            &self.cli.config_path,
            "accounts.period",
            "the return period cannot be resolved from Companies House without an API \
             key; set COMPANIES_HOUSE_API_KEY (or COMPANIES_HOUSE_SANDBOX_API_KEY), or \
             add accounts.period (or accounts.accounts_made_up_to) to the config file",
        ))
    }

    /// The CLI paths: `--book` (required — no alternative source) and
    /// `--out` (optional; defaults to the tally repo's `.cache/tally-cli`
    /// when run from the checkout, else `~/.cache/tally-cli`).
    fn resolve_paths(&self) -> Result<(PathBuf, PathBuf)> {
        let book_path = self.cli.book_path.clone().ok_or_else(|| {
            missing(
                &self.cli.config_path,
                "--book",
                "the GnuCash ledger cannot be resolved from the environment; \
                 set the --book flag",
            )
        })?;
        let out_dir = self.cli.out_dir.clone().unwrap_or_else(default_out_dir);
        Ok((book_path, out_dir))
    }
}

/// The default output directory when `--out` is omitted: the tally repo's
/// `.cache/tally-cli` when run from inside the checkout (so the outputs
/// land where the repo's other tooling looks), else `~/.cache/tally-cli`.
fn default_out_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    default_out_dir_from(&cwd, &home_dir())
}

/// The `--out` default for a given working directory and home directory:
/// the tally workspace's `.cache/tally-cli` when `cwd` sits inside the
/// checkout, else `<home>/.cache/tally-cli`.
fn default_out_dir_from(cwd: &Path, home: &Path) -> PathBuf {
    repo_root_from(cwd)
        .map(|root| root.join(".cache/tally-cli"))
        .unwrap_or_else(|| home.join(".cache/tally-cli"))
}

/// Walk up from `dir` looking for the tally workspace root: the directory
/// holding the root `Cargo.toml` and the `apps/tally-cli` crate.
fn repo_root_from(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|d| d.join("Cargo.toml").is_file() && d.join("apps/tally-cli").is_dir())
        .map(Path::to_path_buf)
}

/// The user's home directory (`$HOME`; `.` when unset).
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Fetch the company's profile at most once per build (cache-first); later
/// callers reuse the fetched response.
async fn fetch_profile<'a>(
    api: &CompaniesHouseClient,
    company_number: &str,
    profile: &'a mut Option<ChProfile>,
) -> Result<&'a ChProfile> {
    if profile.is_none() {
        *profile = Some(
            api.get_company_profile_cached(company_number)
                .await
                .with_context(|| {
                    format!("resolve company '{company_number}' from Companies House")
                })?,
        );
    }
    Ok(profile.as_ref().expect("just filled above"))
}

/// Strict: every field is present, or resolution already errored on the
/// first missing input.
#[derive(Debug)]
pub struct ResolvedInputs {
    /// The fully-resolved company identity (name, UTR, number, registration
    /// date).
    pub company: Company,
    /// The company's descriptive profile (directors, contacts, accountant /
    /// auditor, ...) from the config's `company.*` sub-object.
    pub profile: CompanyProfile,
    /// The set of accounts being produced: the return period (resolved), the
    /// financial-year tax parameters and the report metadata.
    pub accounts: AccountsMeta,
    /// The GnuCash ledger to read.
    pub book_path: PathBuf,
    /// The output directory; the CT600 GovTalk message is written to
    /// `<out>/ct600-<company-number>.xml`.
    pub out_dir: PathBuf,
    /// Which Companies House API the config will use: live or sandbox,
    /// depending on which key is set; `None` when no key is configured.
    pub companies_house_client_type: Option<CompaniesHouseClientType>,
}

/// The error for an input no source could provide.
fn missing(config_path: &Path, field: &str, reason: impl Into<String>) -> anyhow::Error {
    anyhow!(
        "cannot resolve the config from '{}': {} is missing — {}",
        config_path.display(),
        field,
        reason.into()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn company_config(
        name: Option<&str>,
        tax_reference: Option<&str>,
        company_number: Option<&str>,
    ) -> CompanyConfig {
        CompanyConfig {
            name: name.map(str::to_string),
            tax_reference: tax_reference.map(str::to_string),
            company_number: company_number.map(str::to_string),
            // The profile is irrelevant to resolution; use the empty default.
            ..CompanyConfig::default()
        }
    }

    /// An `accounts` sub-object with the 2020 calendar-year return period
    /// (or none), an optional made-up-to date and fixed (required) report
    /// metadata — the tests don't care about the metadata values.
    fn accounts_config(with_period: bool, made_up_to: Option<NaiveDate>) -> AccountsConfig {
        AccountsConfig {
            period: with_period.then(|| AccountingPeriod {
                start: date(2020, 1, 1),
                end: date(2020, 12, 31),
            }),
            accounts_made_up_to: made_up_to,
            fy1_year: DEFAULT_FY1_YEAR,
            fy2_year: DEFAULT_FY2_YEAR,
            associated_companies: None,
            report_date: date(2021, 3, 1),
            authorised_date: date(2021, 2, 1),
            signed_by: Some("B Smith".into()),
            // The employee counts are left out: they default to 1 per
            // financial year.
            average_employees: None,
            accounting_standards_dimension: DEFAULT_ACCOUNTING_STANDARDS_DIMENSION.into(),
            accounts_type_dimension: DEFAULT_ACCOUNTS_TYPE_DIMENSION.into(),
            accounts_status_dimension: DEFAULT_ACCOUNTS_STATUS_DIMENSION.into(),
            signature_b64: String::new(),
            // The incorporation date is optional.
            incorporation_date: None,
        }
    }

    fn env(
        utr: Option<&str>,
        company_number: Option<&str>,
        api_key: Option<&str>,
        sandbox_api_key: Option<&str>,
    ) -> EnvVars {
        EnvVars {
            unique_taxpayer_ref: utr.map(str::to_string),
            company_number: company_number.map(str::to_string),
            companies_house_api_key: api_key.map(str::to_string),
            companies_house_sandbox_api_key: sandbox_api_key.map(str::to_string),
            cache_dir: None,
        }
    }

    /// A builder over the given sources with dummy CLI paths, so the tests
    /// resolve without a real config file or command line.
    fn builder(env: EnvVars, company: CompanyConfig, accounts: AccountsConfig) -> ConfigBuilder {
        ConfigBuilder {
            env,
            file: ConfigFile { company, accounts },
            cli: CliArgs {
                config_path: PathBuf::from("test.json"),
                accounts_made_up_to: None,
                book_path: Some(PathBuf::from("book.gnucash")),
                out_dir: Some(PathBuf::from("out")),
            },
        }
    }

    /// Offline resolution paths: a complete config resolves with no lookup,
    /// and a missing field errors naming it and how to resolve it.  The env
    /// is passed explicitly, so no environment mutation is involved.
    #[tokio::test]
    async fn resolve_company_offline_paths() {
        let empty_env = env(None, None, None, None);

        // Complete config: resolves with no lookup.
        let complete = company_config(
            Some("Example Biz Ltd."),
            Some("8596148860"),
            Some("12345678"),
        );
        let company = builder(
            empty_env.clone(),
            complete.clone(),
            accounts_config(true, None),
        )
        .build()
        .await
        .expect("complete config resolves")
        .company;
        assert_eq!(company.name, "Example Biz Ltd.");
        assert_eq!(company.tax_reference, "8596148860");
        assert_eq!(company.company_number, "12345678");
        // No lookup happened, so the registration date is the unset default.
        assert_eq!(company.registration_date, NaiveDate::default());

        // Incomplete config without an API key: the first missing field is
        // the company number.
        let incomplete = company_config(None, None, None);
        let err = builder(empty_env.clone(), incomplete, accounts_config(false, None))
            .build()
            .await
            .expect_err("incomplete config must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.company_number"), "{msg}");
        assert!(msg.contains("COMPANY_NUMBER"), "{msg}");

        // With a number but no name, the error names `company.name` and the
        // key needed to resolve it.
        let no_name = company_config(None, Some("8596148860"), Some("12345678"));
        let err = builder(empty_env.clone(), no_name, accounts_config(true, None))
            .build()
            .await
            .expect_err("a missing name must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.name"), "{msg}");
        assert!(msg.contains("COMPANIES_HOUSE_API_KEY"), "{msg}");

        // With a name but no UTR, the error names `company.tax_reference`
        // and the `COMPANY_UNIQUE_TAXPAYER_REF` alternative.
        let no_utr = company_config(Some("Example Biz Ltd."), None, Some("12345678"));
        let err = builder(empty_env.clone(), no_utr, accounts_config(true, None))
            .build()
            .await
            .expect_err("a missing UTR must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.tax_reference"), "{msg}");
        assert!(msg.contains("COMPANY_UNIQUE_TAXPAYER_REF"), "{msg}");

        // Empty-string fields count as absent: an empty name or UTR errors.
        let empty_name = company_config(Some(""), Some("8596148860"), Some("12345678"));
        let err = builder(empty_env.clone(), empty_name, accounts_config(true, None))
            .build()
            .await
            .expect_err("an empty company.name must error");
        assert!(format!("{err:#}").contains("company.name"));
        let empty_utr = company_config(Some("Example Biz Ltd."), Some(""), Some("12345678"));
        let err = builder(empty_env.clone(), empty_utr, accounts_config(true, None))
            .build()
            .await
            .expect_err("an empty company.tax_reference must error");
        assert!(format!("{err:#}").contains("company.tax_reference"));

        // The captured `COMPANY_UNIQUE_TAXPAYER_REF` wins over the config's.
        let company = builder(
            env(Some("1111111111"), None, None, None),
            complete.clone(),
            accounts_config(true, None),
        )
        .build()
        .await
        .expect("the env UTR wins over the config's")
        .company;
        assert_eq!(company.tax_reference, "1111111111");

        // The captured `COMPANY_NUMBER` fills an absent config number.
        let no_number = company_config(Some("Example Biz Ltd."), Some("8596148860"), None);
        let company = builder(
            env(None, Some("12345678"), None, None),
            no_number,
            accounts_config(true, None),
        )
        .build()
        .await
        .expect("the env company number fills the absent config's")
        .company;
        assert_eq!(company.company_number, "12345678");

        // The captured `COMPANY_NUMBER` wins over the config's number.
        let company = builder(
            env(None, Some("99999999"), None, None),
            complete.clone(),
            accounts_config(true, None),
        )
        .build()
        .await
        .expect("the env number wins over the config's")
        .company;
        assert_eq!(company.company_number, "99999999");

        // An API key with a *complete* config: still no lookup (the name is
        // present, so nothing needs fetching) and no network.
        let company = builder(
            env(None, None, Some("test-key"), None),
            complete,
            accounts_config(true, None),
        )
        .build()
        .await
        .expect("complete config resolves even with an API key set")
        .company;
        assert_eq!(company.name, "Example Biz Ltd.");
    }

    /// The `--out` default: inside the tally checkout the repo's
    /// `.cache/tally-cli` is used; elsewhere `<home>/.cache/tally-cli`.
    #[test]
    fn default_out_dir_prefers_repo_then_home() {
        let cwd = std::env::current_dir().unwrap();
        let repo = repo_root_from(&cwd).expect("the tests run inside the tally repo");
        assert_eq!(
            default_out_dir_from(&repo.join("apps/tally-cli"), Path::new("/tmp/home")),
            repo.join(".cache/tally-cli")
        );

        let elsewhere = Path::new("/tmp/elsewhere");
        assert_eq!(repo_root_from(elsewhere), None);
        assert_eq!(
            default_out_dir_from(elsewhere, Path::new("/tmp/home")),
            PathBuf::from("/tmp/home/.cache/tally-cli")
        );
    }

    /// End-to-end through the per-source types: the example config file
    /// parses into a [`ConfigFile`], and a [`ConfigBuilder`] resolves it —
    /// erroring on the missing `--book`, defaulting `--out` when absent and
    /// succeeding once the book is present.
    #[tokio::test]
    async fn ct600_config_resolves_and_requires_paths() {
        let path = test_utils::REPO.join("example_data/basic-1/input_config.jsonc");
        let empty_env = EnvVars::default();

        let file = ConfigFile::from_file(&path).expect("parse example config");
        assert_eq!(file.company.name.as_deref(), Some("Example Biz Ltd."));
        assert_eq!(
            file.company.directors,
            Some(vec!["A Bloggs".into(), "B Smith".into(), "C Jones".into()])
        );
        let period = file
            .accounts
            .period
            .expect("example config carries a period");
        assert_eq!(period.start, date(2020, 1, 1));
        assert_eq!(period.end, date(2020, 12, 31));
        // An explicit dimension value from the JSON overrides the default.
        assert_eq!(
            file.accounts.accounting_standards_dimension,
            "uk-bus:Micro-entities"
        );

        // Without the CLI paths, resolution errors on the first missing one.
        let cli = CliArgs {
            config_path: path.to_path_buf(),
            accounts_made_up_to: None,
            book_path: None,
            out_dir: None,
        };
        let err = ConfigBuilder {
            env: empty_env.clone(),
            file: file.clone(),
            cli,
        }
        .build()
        .await
        .expect_err("missing paths must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("--book"), "{msg}");
        assert!(
            !msg.contains("--out"),
            "resolution fails fast on the first missing path: {msg}"
        );

        // With `--book` but no `--out`, the output directory defaults to
        // the conditional default instead of erroring.
        let cli = CliArgs {
            config_path: path.to_path_buf(),
            accounts_made_up_to: None,
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: None,
        };
        let resolved = ConfigBuilder {
            env: empty_env.clone(),
            file: file.clone(),
            cli,
        }
        .build()
        .await
        .expect("resolves with the default output directory");
        assert_eq!(resolved.out_dir, default_out_dir());

        // With both, resolution succeeds and everything carries through.
        let cli = CliArgs {
            config_path: path.to_path_buf(),
            accounts_made_up_to: None,
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let resolved = ConfigBuilder {
            env: empty_env.clone(),
            file: file.clone(),
            cli,
        }
        .build()
        .await
        .expect("resolves with paths");
        assert_eq!(resolved.company.name, "Example Biz Ltd.");
        assert_eq!(resolved.company.tax_reference, "8596148860");
        assert_eq!(resolved.company.company_number, "12345678");
        assert_eq!(resolved.company.registration_date, NaiveDate::default());
        assert_eq!(resolved.accounts.period().start, date(2020, 1, 1));
        assert_eq!(resolved.accounts.period().end, date(2020, 12, 31));
        assert_eq!(resolved.accounts.fy1_year, 2019);
        // An explicit per-year map passes through unchanged.
        assert_eq!(
            resolved.accounts.average_employees,
            HashMap::from([("2019".to_string(), 1), ("2020".to_string(), 2)])
        );
        assert_eq!(resolved.book_path, PathBuf::from("input.gnucash"));
        assert_eq!(resolved.out_dir, PathBuf::from("out"));
        assert_eq!(
            resolved.profile.directors,
            vec!["A Bloggs", "B Smith", "C Jones"]
        );
        // The empty captured env configures no Companies House client.
        assert_eq!(resolved.companies_house_client_type, None);

        // With a sandbox key in the captured env, the resolved inputs report
        // it.
        let cli = CliArgs {
            config_path: path.to_path_buf(),
            accounts_made_up_to: None,
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let resolved = ConfigBuilder {
            env: env(None, None, None, Some("sandbox-key")),
            file: file.clone(),
            cli,
        }
        .build()
        .await
        .expect("resolves with paths");
        assert_eq!(
            resolved.companies_house_client_type,
            Some(CompaniesHouseClientType::Sandbox)
        );
    }

    /// The return period deduced from a made-up-to date, entirely offline:
    /// the 12 months ending on the date, with the flag winning over the
    /// config's `accounts.accounts_made_up_to`, and an explicit
    /// `accounts.period` still winning over both.
    #[tokio::test]
    async fn made_up_to_deduces_the_return_period() {
        let empty_env = env(None, None, None, None);
        let identity = company_config(
            Some("Example Biz Ltd."),
            Some("8596148860"),
            Some("12345678"),
        );

        // The config's made-up-to date: the 12 months ending on it.
        let accounts = builder(
            empty_env.clone(),
            identity.clone(),
            accounts_config(false, Some(date(2020, 12, 31))),
        )
        .build()
        .await
        .expect("resolves")
        .accounts;
        assert_eq!(accounts.period().start, date(2020, 1, 1));
        assert_eq!(accounts.period().end, date(2020, 12, 31));

        // The flag (the override) wins over the config's date.
        let file = ConfigFile {
            company: identity.clone(),
            accounts: accounts_config(false, Some(date(2020, 12, 31))),
        };
        let cli = CliArgs {
            config_path: PathBuf::from("test.json"),
            accounts_made_up_to: Some(date(2021, 3, 31)),
            book_path: Some(PathBuf::from("book.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let accounts = ConfigBuilder {
            env: empty_env.clone(),
            file,
            cli,
        }
        .build()
        .await
        .expect("resolves")
        .accounts;
        assert_eq!(accounts.period().start, date(2020, 4, 1));
        assert_eq!(accounts.period().end, date(2021, 3, 31));

        // An explicit `accounts.period` still wins over the made-up-to date
        // (config or flag).
        let file = ConfigFile {
            company: identity,
            accounts: accounts_config(true, Some(date(2020, 12, 31))),
        };
        let cli = CliArgs {
            config_path: PathBuf::from("test.json"),
            accounts_made_up_to: Some(date(2021, 3, 31)),
            book_path: Some(PathBuf::from("book.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let accounts = ConfigBuilder {
            env: empty_env,
            file,
            cli,
        }
        .build()
        .await
        .expect("resolves")
        .accounts;
        assert_eq!(accounts.period().start, date(2020, 1, 1));
        assert_eq!(accounts.period().end, date(2020, 12, 31));
    }

    /// Without any period input and no API key, the error names the period
    /// and the options for resolving it.
    #[tokio::test]
    async fn missing_period_without_api_key_errors_with_options() {
        let empty_env = env(None, None, None, None);
        let raw = company_config(
            Some("Example Biz Ltd."),
            Some("8596148860"),
            Some("12345678"),
        );
        let err = builder(empty_env, raw, accounts_config(false, None))
            .build()
            .await
            .expect_err("the missing period must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("accounts.period"), "{msg}");
        assert!(msg.contains("accounts.accounts_made_up_to"), "{msg}");
        assert!(msg.contains("COMPANIES_HOUSE_API_KEY"), "{msg}");
    }

    /// With an API key but no company number, the missing number errors
    /// first — it is needed for every Companies House lookup.
    #[tokio::test]
    async fn missing_number_with_api_key_errors() {
        let env = env(None, None, Some("test-key"), None);
        let raw = company_config(Some("Example Biz Ltd."), Some("8596148860"), None);
        let err = builder(env, raw, accounts_config(false, None))
            .build()
            .await
            .expect_err("the missing number must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.company_number"), "{msg}");
        assert!(msg.contains("COMPANY_NUMBER"), "{msg}");
    }

    /// Fail-fast order across concerns: with an incomplete identity *and*
    /// missing CLI paths, the identity field errors first.
    #[tokio::test]
    async fn identity_errors_before_paths() {
        let cli = CliArgs {
            config_path: PathBuf::from("test.json"),
            accounts_made_up_to: None,
            book_path: None,
            out_dir: None,
        };
        let err = ConfigBuilder {
            env: EnvVars::default(),
            file: ConfigFile {
                company: company_config(None, None, None),
                accounts: accounts_config(false, None),
            },
            cli,
        }
        .build()
        .await
        .expect_err("the identity error surfaces first");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.company_number"), "{msg}");
        assert!(
            !msg.contains("--book"),
            "the identity is resolved before the paths: {msg}"
        );
    }

    /// The committed minimal config
    /// (`example_data/basic-1/minimal_config.jsonc`) parses to a
    /// blank identity, no period, a blank profile and only the required
    /// report metadata — the live test enriches the identity and period
    /// from the environment and the Companies House API.
    #[test]
    fn minimal_config_parses() {
        let file = ConfigFile::from_file(
            &test_utils::REPO.join("example_data/basic-1/minimal_config.jsonc"),
        )
        .expect("parse the minimal config");
        assert_eq!(file.company.name, None);
        assert_eq!(file.company.tax_reference, None);
        assert_eq!(file.company.company_number, None);
        assert_eq!(file.accounts.period, None);
        assert_eq!(file.accounts.accounts_made_up_to, None);
        // The blank profile: no directors, no contacts, no accountant.
        assert_eq!(file.company.directors, None);
        assert_eq!(file.company.contact_name, None);
        assert_eq!(file.company.accountant_name, None);
        // The required (unguessable) report metadata comes from the config;
        // the signatory is omitted (it defaults to the first director).
        assert_eq!(file.accounts.report_date, date(2021, 3, 1));
        assert_eq!(file.accounts.authorised_date, date(2021, 2, 1));
        assert_eq!(file.accounts.signed_by, None);
        assert_eq!(file.accounts.average_employees, None);
        assert_eq!(file.accounts.signature_b64, "");
        // The enrichment fills the profile blanks and passes the metadata
        // through into the required report types; the employee counts
        // default to 1 for each of the two financial years.
        assert_eq!(
            file.company.clone().into_profile(),
            CompanyProfile::default()
        );
        let meta = file.accounts.into_meta();
        assert_eq!(meta.report_date, date(2021, 3, 1));
        assert_eq!(meta.authorised_date, Some(date(2021, 2, 1)));
        assert_eq!(meta.signed_by, "");
        assert_eq!(
            meta.average_employees,
            HashMap::from([("2019".to_string(), 1), ("2020".to_string(), 1)])
        );
        assert_eq!(meta.signature_b64, None);
    }

    /// Config files are parsed as JSONC (`serde_json_lenient`): `//`
    /// comments and trailing commas are allowed, so users can annotate
    /// their config without breaking parsing.
    #[test]
    fn config_parses_jsonc_comments_and_trailing_commas() {
        let jsonc = r#"{
            // the company block: identity + profile (all optional)
            "company": {
                "name": "Example Biz Ltd.",  // resolved from Companies House when absent
                "tax_reference": "8596148860",
            },
            "accounts": {
                "period": { "start": "2023-01-01", "end": "2023-12-31" },
                // the required (unguessable) report metadata
                "report_date": "2024-03-01",
                "authorised_date": "2024-02-01",
                "signature_b64": "",
            }
        }"#;
        let file: ConfigFile = serde_json_lenient::from_str(jsonc).expect("parse JSONC config");
        assert_eq!(file.company.name.as_deref(), Some("Example Biz Ltd."));
        assert_eq!(file.company.tax_reference.as_deref(), Some("8596148860"));
        let period = file.accounts.period.expect("period from config");
        assert_eq!(period.start, date(2023, 1, 1));
        assert_eq!(period.end, date(2023, 12, 31));
    }

    /// The employee counts default per financial year: an explicit year
    /// wins, and a year the config left out defaults to 1.
    #[test]
    fn average_employees_default_per_financial_year_overlay() {
        // Both years given: pass through unchanged.
        let both = AccountsConfig {
            average_employees: Some(HashMap::from([("2019".to_string(), 3)])),
            fy1_year: 2019,
            fy2_year: 2020,
            ..accounts_config(true, None)
        };
        assert_eq!(
            both.into_meta().average_employees,
            HashMap::from([("2019".to_string(), 3), ("2020".to_string(), 1)])
        );
        // Only one year given: the other financial year defaults to 1.
        let one = AccountsConfig {
            average_employees: Some(HashMap::from([("2020".to_string(), 2)])),
            fy1_year: 2019,
            fy2_year: 2020,
            ..accounts_config(true, None)
        };
        assert_eq!(
            one.into_meta().average_employees,
            HashMap::from([("2019".to_string(), 1), ("2020".to_string(), 2)])
        );
    }

    /// The config types serialise omitting absent options: a blank company
    /// config is an empty object, and an accounts config serialises the
    /// required metadata and the defaulted fields while omitting the absent
    /// optional fields (the period and the incorporation date).
    #[test]
    fn config_serialization_omits_absent_options() {
        assert_eq!(
            serde_json::to_string(&CompanyConfig::default()).unwrap(),
            "{}"
        );
        assert_eq!(
            serde_json::to_string(&accounts_config(false, None)).unwrap(),
            "{\"fy1_year\":2019,\"fy2_year\":2020,\"report_date\":\"2021-03-01\",\"authorised_date\":\"2021-02-01\",\"signed_by\":\"B Smith\",\"accounting_standards_dimension\":\"uk-bus:Micro-entities\",\"accounts_type_dimension\":\"uk-bus:AbridgedAccounts\",\"accounts_status_dimension\":\"uk-bus:AuditExempt-NoAccountantsReport\",\"signature_b64\":\"\"}"
        );
    }

    /// With an API key, a company number and a cached profile, the return
    /// period defaults to the profile's next accounting period — no network
    /// access (the profile is seeded into a scratch cache directory).
    #[tokio::test]
    async fn missing_period_defaults_to_companies_house_next_accounting_period() {
        let cache_dir = std::env::temp_dir().join(format!(
            "tally-cli-config-ch-default-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&cache_dir).expect("create scratch cache dir");
        std::fs::write(
            cache_dir.join("companies-house-12345678.json"),
            serde_json::json!({
                "company_number": "12345678",
                "company_name": "CACHED CORP LTD",
                "date_of_creation": "2001-01-01",
                "jurisdiction": "England and Wales",
                "registered_office_address": {
                    "address_line_1": "123 Leadbarton Street",
                    "address_line_2": "Dumpston Trading Estate",
                    "locality": "Threapminchington",
                    "region": "Minchingshire",
                    "postal_code": "QQ99 9ZZ"
                },
                "sic_codes": ["62020", "62021"],
                "accounts": {
                    "next_accounts": {
                        "period_start_on": "2025-01-01",
                        "period_end_on": "2025-12-31",
                        "due_on": "2026-09-30"
                    }
                }
            })
            .to_string(),
        )
        .expect("seed the profile cache");
        // The officers cache feeds the directors enrichment (also offline).
        std::fs::write(
            cache_dir.join("companies-house-12345678-officers.json"),
            serde_json::json!({
                "items": [
                    { "name": "A Bloggs", "officer_role": "director" },
                    { "name": "B Smith", "officer_role": "director", "resigned_on": "2020-01-01" },
                    { "name": "C Jones", "officer_role": "secretary" }
                ]
            })
            .to_string(),
        )
        .expect("seed the officers cache");

        let mut env = env(None, None, Some("test-key"), None);
        env.cache_dir = Some(cache_dir.clone());

        let raw = company_config(
            Some("Example Biz Ltd."),
            Some("8596148860"),
            Some("12345678"),
        );
        // The signatory is left out of the config: it must default to the
        // first enriched director.
        let accounts_config = AccountsConfig {
            signed_by: None,
            ..accounts_config(false, None)
        };
        let resolved = builder(env, raw, accounts_config)
            .build()
            .await
            .expect("resolves from the cached profile");
        let accounts = resolved.accounts;
        assert_eq!(accounts.period().start, date(2025, 1, 1));
        assert_eq!(accounts.period().end, date(2025, 12, 31));
        // The signatory defaults to the first director.
        assert_eq!(accounts.signed_by, "A Bloggs");
        // The employee counts default to 1 for each of the two financial
        // years (the config left them out).
        assert_eq!(
            accounts.average_employees,
            HashMap::from([("2019".to_string(), 1), ("2020".to_string(), 1)])
        );
        // The profile also supplies the registration date, anchoring the
        // registration-date schedule used as the fallback.
        assert_eq!(resolved.company.registration_date, date(2001, 1, 1));
        // The descriptive profile is enriched from the same cached profile
        // and officers: the address, SIC codes and jurisdiction from the
        // profile, and the current (non-resigned, director-role) officers.
        let profile = resolved.profile;
        assert_eq!(
            profile.address_lines,
            vec!["123 Leadbarton Street", "Dumpston Trading Estate"]
        );
        assert_eq!(profile.county.as_deref(), Some("Minchingshire"));
        assert_eq!(profile.location, "Threapminchington");
        assert_eq!(profile.postcode, "QQ99 9ZZ");
        assert_eq!(profile.sic_codes, vec!["62020", "62021"]);
        assert_eq!(profile.jurisdiction, "England and Wales");
        assert_eq!(profile.directors, vec!["A Bloggs"]);

        let _ = std::fs::remove_dir_all(&cache_dir);
    }
}

// ============================================================================
// Live enrichment tests (part of the default-enabled `api_tests` feature)
// ============================================================================

/// Live Companies House enrichment tests (part of the default-enabled
/// `api_tests` feature).
#[cfg(test)]
mod live_tests {
    use super::*;

    /// Live end-to-end enrichment of the *minimum* config from the Companies
    /// House API, and the cache-first second run.
    ///
    /// With `COMPANIES_HOUSE_API_KEY` (or `COMPANIES_HOUSE_SANDBOX_API_KEY`),
    /// `COMPANY_NUMBER` and `COMPANY_UNIQUE_TAXPAYER_REF` exported, the
    /// committed minimal config
    /// (`example_data/basic-1/minimal_config.jsonc` — no identity,
    /// no period, blank profile; the required report metadata comes from the
    /// config) is enriched from the live profile and officers (name,
    /// registration date, next accounting period, and the descriptive
    /// profile: registered-office address, SIC codes, jurisdiction and the
    /// current directors), and the resolved inputs are printed.  The profile
    /// and officers land in the repository's `.cache/api_responses` (this
    /// test exercises the CLI, not the client — the cache means repeat runs
    /// don't keep hitting the remote API), and a second run with a
    /// placeholder key proves the cache serves the response.
    #[tokio::test]
    #[cfg_attr(
        not(feature = "api_tests"),
        ignore = "requires a Companies House API key, COMPANY_NUMBER and COMPANY_UNIQUE_TAXPAYER_REF"
    )]
    async fn live_minimal_config_enriched_from_api_and_cached() {
        let mut env = EnvVars::from_env();
        assert!(
            env.companies_house_api_key.is_some() || env.companies_house_sandbox_api_key.is_some(),
            "the api_tests feature needs COMPANIES_HOUSE_API_KEY (live) or \
             COMPANIES_HOUSE_SANDBOX_API_KEY (sandbox)"
        );
        let number = env.company_number.as_deref().expect(
            "the api_tests feature needs COMPANY_NUMBER: use a real company for the \
                     live API, a sandbox test company for the sandbox API",
        );
        let utr = env.unique_taxpayer_ref.as_deref().expect(
            "the api_tests feature needs COMPANY_UNIQUE_TAXPAYER_REF (the Corporation \
                     Tax reference is never resolved from Companies House)",
        );

        // The repository's `.cache/api_responses`: the profile and officers
        // are cached there, so the second run is served from it and repeat
        // runs of the suite don't keep hitting the API.
        env.cache_dir = Some(test_utils::cache_dir("api_responses"));

        // The committed minimal config: no identity, no period, blank
        // profile — the identity and period come from the environment and
        // the live API; the required report metadata comes from the config.
        let path = test_utils::REPO.join("example_data/basic-1/minimal_config.jsonc");
        let file = ConfigFile::from_file(&path).expect("parse the minimal config");
        let cli = CliArgs {
            config_path: path,
            accounts_made_up_to: None,
            book_path: Some(PathBuf::from("book.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };

        // Run 1: everything is enriched from the live API response (or the
        // already-warm cache).
        let resolved = ConfigBuilder {
            env: env.clone(),
            file: file.clone(),
            cli: cli.clone(),
        }
        .build()
        .await
        .expect("resolve the minimum config from the live API");
        println!("resolved inputs from the live API:\n{resolved:#?}");
        let company = resolved.company;
        assert_eq!(company.company_number, number);
        assert_eq!(company.tax_reference, utr);
        assert!(
            !company.name.is_empty(),
            "the name is filled from the profile"
        );
        let today = chrono::Utc::now().date_naive();
        assert!(
            company.registration_date < today,
            "the registration date comes from the profile (created in the past), got {}",
            company.registration_date
        );
        // The return period defaults to the next accounting period to file
        // (a company behind on filings can have a past-due period, so only
        // the ordering is pinned).
        let period = resolved.accounts.period();
        assert!(period.start < period.end, "the return period is ordered");
        // The descriptive profile is enriched from the live profile and
        // officers: the registered-office address, the SIC codes and the
        // jurisdiction from the profile, and the current directors from the
        // officers list.
        let profile = resolved.profile;
        assert!(
            !profile.address_lines.is_empty() && !profile.postcode.is_empty(),
            "the registered-office address comes from the profile"
        );
        assert!(
            !profile.sic_codes.is_empty(),
            "the SIC codes come from the profile"
        );
        assert!(
            !profile.jurisdiction.is_empty(),
            "the jurisdiction comes from the profile"
        );
        assert!(
            !profile.directors.is_empty(),
            "the current directors come from the officers list"
        );
        // The county stays `None`: this company's Companies House profile
        // carries no `region` (county is a voluntary fact, filled from
        // `registered_office_address.region` only when present).
        assert_eq!(
            profile.county, None,
            "county is only filled when the profile records a region"
        );
        // The minimal config omits the signatory: it defaults to the first
        // director.
        assert_eq!(
            resolved.accounts.signed_by,
            profile
                .directors
                .first()
                .map(String::as_str)
                .unwrap_or_default()
        );

        // The profile and the officers are cached for the next run.
        let ch = env.ch_config();
        let cache_dir = ch.cache_dir().expect("the env configures a cache dir");
        let cache_file = cache_dir.join(format!("companies-house-{number}.json"));
        assert!(cache_file.exists(), "the fetched profile is cached on disk");
        let cached = std::fs::read_to_string(&cache_file).expect("read the cache file");
        assert!(
            cached.contains("company_name"),
            "the cache holds the profile JSON"
        );
        assert!(
            cache_dir
                .join(format!("companies-house-{number}-officers.json"))
                .exists(),
            "the fetched officers are cached on disk"
        );

        // Run 2: the same cache, but a placeholder key that could never
        // fetch the real profile — resolution is served entirely from the
        // cache.
        let mut cached_env = env.clone();
        cached_env.companies_house_api_key = Some("placeholder-cached-run".to_string());
        cached_env.companies_house_sandbox_api_key = None;
        let from_cache = ConfigBuilder {
            env: cached_env,
            file,
            cli,
        }
        .build()
        .await
        .expect("resolve the minimum config from the cache");
        let company2 = from_cache.company;
        assert_eq!(company2.name, company.name);
        assert_eq!(company2.registration_date, company.registration_date);
        let period2 = from_cache.accounts.period();
        assert_eq!(
            period2.start, period.start,
            "the return period is the same on the cached run"
        );
        assert_eq!(
            period2.end, period.end,
            "the return period is the same on the cached run"
        );
        // The enriched profile is served from the same caches.
        assert_eq!(from_cache.profile.directors, profile.directors);
        assert_eq!(from_cache.profile.sic_codes, profile.sic_codes);
        // County (the company-specific voluntary field) is cache-consistent
        // too: whatever the first run resolved, the cached run repeats it.
        assert_eq!(from_cache.profile.county, profile.county);
    }
}
