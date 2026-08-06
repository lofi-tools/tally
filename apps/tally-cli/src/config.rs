//! Configuration for the `tally` CLI.
//!
//! The three input sources — the environment ([`EnvVars`]), the JSON config
//! file ([`ConfigFile`]) and the subcommand's CLI values ([`CliArgs`]) — are
//! loaded by a [`ConfigBuilder`] in precedence order (CLI flags first, the
//! environment overriding the config file) and resolved into a strict
//! [`ResolvedInputs`], erroring on the first field the sources cannot
//! provide.  The config file's optional fields — the identity, the
//! descriptive profile ([`CompanyConfig`]) and the report metadata
//! ([`AccountsConfig`]) — are enriched here, from the environment /
//! Companies House or to blank defaults, into the required report types
//! ([`CompanyProfile`], [`AccountsMeta`]) before the libraries consume them.
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
//! | `accounts.fy1_year` / `fy2_year` / `fy1_rate` / `fy2_rate` | config file → defaults (2019 / 2020 at 19%) |
//! | `company.registration_date` | Companies House (only when the config carries no identity at all) → [`Company::new`] default |
//! | Companies House layer | `COMPANIES_HOUSE_API_KEY` (live) / `COMPANIES_HOUSE_SANDBOX_API_KEY` (sandbox); response cache in `CT600_CACHE_DIR` (env) |
//! | `company.*` profile fields (directors, contacts, accountant/auditor, ...) | config file only (optional — blank when absent) |
//! | `accounts.*` report metadata (dates, employees, ...) | config file only (optional — blank when absent) |
//! | `--book`, `--out` | command line only |

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{Duration, Months, NaiveDate};
use ct600::companies_house::{CompanyProfile as ChProfile, next_accounting_period_from};
use ct600::{CompaniesHouseClient, CompaniesHouseClientType};
use ixbrl::company::{AccountingPeriod, AccountsMeta, Company, CompanyProfile};
use serde::{Deserialize, Serialize};

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
/// Everything here is optional; [`ConfigBuilder::build`] enriches the
/// missing values — from the environment / Companies House (identity) or to
/// blank defaults (profile and report metadata) — into the required report
/// types before the libraries consume them.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigFile {
    /// The `company` block: the optional identity fields plus the optional
    /// descriptive profile (directors, contacts, accountant/auditor, ...).
    #[serde(default)]
    pub company: CompanyConfig,
    /// The `accounts` sub-object ([`AccountsConfig`]): the optional return
    /// period, financial-year parameters and report metadata.
    #[serde(default)]
    pub accounts: AccountsConfig,
}

impl ConfigFile {
    /// Parse the config file at `path` (`--config-path`).
    pub fn from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("read config '{}'", path.display()))?;
        serde_json::from_str(&data).with_context(|| format!("parse config '{}'", path.display()))
    }
}

/// The `company` block of the config file (`company.*`): the optional
/// identity fields plus the optional descriptive profile (directors,
/// contacts, accountant/auditor, ...).  Every field is optional and
/// serialises back as omitted when absent.  The builder enriches the
/// identity into the required [`Company`], and [`Self::into_profile`] fills
/// the profile's blanks into the required [`CompanyProfile`] the reports
/// consume.
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
    /// Fill the optional profile into the required report profile: absent
    /// fields become blank defaults (empty strings / empty lists, no logo).
    pub(crate) fn into_profile(self) -> CompanyProfile {
        CompanyProfile {
            directors: self.directors.unwrap_or_default(),
            contact_name: self.contact_name.unwrap_or_default(),
            address_lines: self.address_lines.unwrap_or_default(),
            county: self.county.unwrap_or_default(),
            location: self.location.unwrap_or_default(),
            postcode: self.postcode.unwrap_or_default(),
            email: self.email.unwrap_or_default(),
            phone_country: self.phone_country.unwrap_or_default(),
            phone_area: self.phone_area.unwrap_or_default(),
            phone_number: self.phone_number.unwrap_or_default(),
            website_url: self.website_url.unwrap_or_default(),
            website_description: self.website_description.unwrap_or_default(),
            vat_registration: self.vat_registration.unwrap_or_default(),
            sic_codes: self.sic_codes.unwrap_or_default(),
            activities: self.activities.unwrap_or_default(),
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

/// The `accounts` sub-object of the config file (`accounts.*`): the optional
/// return period, financial-year parameters and report metadata.  The fy
/// parameters default to 2019 / 2020 at 19% (matching [`AccountsMeta`]);
/// [`Self::into_meta`] fills the optional metadata into the required
/// [`AccountsMeta`] the reports consume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<AccountingPeriod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accounts_made_up_to: Option<NaiveDate>,
    #[serde(default = "default_fy1_year")]
    pub fy1_year: i32,
    #[serde(default = "default_fy2_year")]
    pub fy2_year: i32,
    #[serde(default = "default_fy1_rate")]
    pub fy1_rate: f64,
    #[serde(default = "default_fy2_rate")]
    pub fy2_rate: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorised_date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incorporation_date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_employees: Option<HashMap<String, u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accounting_standards_dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accounts_type_dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accounts_status_dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_b64: Option<String>,
}

impl Default for AccountsConfig {
    /// The default set of accounts: no period, the default financial-year
    /// tax parameters (fy1 2019, fy2 2020, both 19%) and no report
    /// metadata.  Matches the serde defaults, so a config without the
    /// `accounts` sub-object behaves like one with it.
    fn default() -> Self {
        Self {
            period: None,
            accounts_made_up_to: None,
            fy1_year: DEFAULT_FY1_YEAR,
            fy2_year: DEFAULT_FY2_YEAR,
            fy1_rate: DEFAULT_FY1_RATE,
            fy2_rate: DEFAULT_FY2_RATE,
            report_date: None,
            authorised_date: None,
            incorporation_date: None,
            signed_by: None,
            average_employees: None,
            accounting_standards_dimension: None,
            accounts_type_dimension: None,
            accounts_status_dimension: None,
            signature_b64: None,
        }
    }
}

impl AccountsConfig {
    /// Fill the optional accounts into the required report set of accounts:
    /// absent report metadata becomes blank defaults.  The period is left
    /// as given — the builder sets the resolved return period.
    pub(crate) fn into_meta(self) -> AccountsMeta {
        AccountsMeta {
            period: self.period,
            accounts_made_up_to: self.accounts_made_up_to,
            fy1_year: self.fy1_year,
            fy2_year: self.fy2_year,
            fy1_rate: self.fy1_rate,
            fy2_rate: self.fy2_rate,
            report_date: self.report_date.unwrap_or_default(),
            authorised_date: self.authorised_date.unwrap_or_default(),
            incorporation_date: self.incorporation_date.unwrap_or_default(),
            signed_by: self.signed_by.unwrap_or_default(),
            average_employees: self.average_employees.unwrap_or_default(),
            accounting_standards_dimension: self.accounting_standards_dimension.unwrap_or_default(),
            accounts_type_dimension: self.accounts_type_dimension.unwrap_or_default(),
            accounts_status_dimension: self.accounts_status_dimension.unwrap_or_default(),
            signature_b64: self.signature_b64.unwrap_or_default(),
        }
    }
}

// The default financial-year tax parameters, mirroring `AccountsMeta`'s
// defaults in libs/ixbrl (keep the two in sync).
const DEFAULT_FY1_YEAR: i32 = 2019;
const DEFAULT_FY2_YEAR: i32 = 2020;
const DEFAULT_FY1_RATE: f64 = 19.0;
const DEFAULT_FY2_RATE: f64 = 19.0;

fn default_fy1_year() -> i32 {
    DEFAULT_FY1_YEAR
}
fn default_fy2_year() -> i32 {
    DEFAULT_FY2_YEAR
}
fn default_fy1_rate() -> f64 {
    DEFAULT_FY1_RATE
}
fn default_fy2_rate() -> f64 {
    DEFAULT_FY2_RATE
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
    /// The `--out` value (required; no other source).
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
    /// sources cannot provide.
    pub async fn build(self) -> Result<ResolvedInputs> {
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

        // The config's optional fields are enriched into the required
        // report types here: the profile fills blanks, and the accounts
        // carry the resolved return period.
        let mut accounts = self.file.accounts.into_meta();
        accounts.period = Some(period);

        let mut company = Company::new(name, tax_reference, company_number);
        if let Some(date) = registration_date {
            company.registration_date = date;
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

    /// The required CLI paths, `--book` and `--out` (no alternative source).
    fn resolve_paths(&self) -> Result<(PathBuf, PathBuf)> {
        let book_path = self.cli.book_path.clone().ok_or_else(|| {
            missing(
                &self.cli.config_path,
                "--book",
                "the GnuCash ledger cannot be resolved from the environment; \
                 set the --book flag",
            )
        })?;
        let out_dir = self.cli.out_dir.clone().ok_or_else(|| {
            missing(
                &self.cli.config_path,
                "--out",
                "the output directory cannot be resolved from the environment; \
                 set the --out flag",
            )
        })?;
        Ok((book_path, out_dir))
    }
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
    /// `<out>/ct600.xml`.
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
    /// (or none) and an optional made-up-to date.
    fn accounts_config(with_period: bool, made_up_to: Option<NaiveDate>) -> AccountsConfig {
        AccountsConfig {
            period: with_period.then(|| AccountingPeriod {
                start: date(2020, 1, 1),
                end: date(2020, 12, 31),
            }),
            accounts_made_up_to: made_up_to,
            ..AccountsConfig::default()
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

    /// End-to-end through the per-source types: the example config file
    /// parses into a [`ConfigFile`], and a [`ConfigBuilder`] resolves it —
    /// erroring on the first missing CLI path, succeeding once both are
    /// present.
    #[tokio::test]
    async fn ct600_config_resolves_and_requires_paths() {
        let path = Path::new("../../libs/ixbrl/example_data/example2/input_config.json");
        let empty_env = EnvVars::default();

        let file = ConfigFile::from_file(path).expect("parse example config");
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

        // With only one path present, the error names the missing one.
        let cli = CliArgs {
            config_path: path.to_path_buf(),
            accounts_made_up_to: None,
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: None,
        };
        let err = ConfigBuilder {
            env: empty_env.clone(),
            file: file.clone(),
            cli,
        }
        .build()
        .await
        .expect_err("missing --out must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("--out"), "{msg}");
        assert!(
            !msg.contains("--book"),
            "the present path must not be reported: {msg}"
        );

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
    /// (`libs/ixbrl/example_data/example2/minimal_config.json`) parses to a
    /// blank identity, no period and no report data — everything the live
    /// test enriches from the environment and the Companies House API.
    #[test]
    fn minimal_config_parses_blank() {
        let path = Path::new("../../libs/ixbrl/example_data/example2/minimal_config.json");
        let file = ConfigFile::from_file(path).expect("parse the minimal config");
        assert_eq!(file.company.name, None);
        assert_eq!(file.company.tax_reference, None);
        assert_eq!(file.company.company_number, None);
        assert_eq!(file.accounts.period, None);
        assert_eq!(file.accounts.accounts_made_up_to, None);
        // The blank profile: no directors, no contacts, no accountant, and
        // no report metadata.
        assert_eq!(file.company.directors, None);
        assert_eq!(file.company.contact_name, None);
        assert_eq!(file.company.accountant_name, None);
        // The enrichment fills the blanks into the required report types.
        assert_eq!(
            file.company.clone().into_profile(),
            CompanyProfile::default()
        );
        assert_eq!(file.accounts.into_meta(), AccountsMeta::default());
    }

    /// The config types serialise omitting absent options: a blank company
    /// config is an empty object, and a blank accounts config carries only
    /// the defaulted financial-year parameters (the non-optional fields).
    #[test]
    fn config_serialization_omits_absent_options() {
        assert_eq!(
            serde_json::to_string(&CompanyConfig::default()).unwrap(),
            "{}"
        );
        assert_eq!(
            serde_json::to_string(&AccountsConfig::default()).unwrap(),
            "{\"fy1_year\":2019,\"fy2_year\":2020,\"fy1_rate\":19.0,\"fy2_rate\":19.0}"
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

        let mut env = env(None, None, Some("test-key"), None);
        env.cache_dir = Some(cache_dir.clone());

        let raw = company_config(
            Some("Example Biz Ltd."),
            Some("8596148860"),
            Some("12345678"),
        );
        let resolved = builder(env, raw, accounts_config(false, None))
            .build()
            .await
            .expect("resolves from the cached profile");
        let accounts = resolved.accounts;
        assert_eq!(accounts.period().start, date(2025, 1, 1));
        assert_eq!(accounts.period().end, date(2025, 12, 31));
        // The profile also supplies the registration date, anchoring the
        // registration-date schedule used as the fallback.
        assert_eq!(resolved.company.registration_date, date(2001, 1, 1));

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
    /// (`libs/ixbrl/example_data/example2/minimal_config.json` — no identity,
    /// no period, blank profile and report metadata) is enriched entirely
    /// from the live profile (name, registration date, next accounting
    /// period), and the resolved inputs are printed.  The profile lands in
    /// the ambient cache directory (idempotent: a warm cache simply skips
    /// the network); a second run with a placeholder key proves the cache
    /// serves the response.
    #[tokio::test]
    #[cfg_attr(
        not(feature = "api_tests"),
        ignore = "requires a Companies House API key, COMPANY_NUMBER and COMPANY_UNIQUE_TAXPAYER_REF"
    )]
    async fn live_minimal_config_enriched_from_api_and_cached() {
        let env = EnvVars::from_env();
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

        // The committed minimal config: no identity, no period, blank
        // profile and report metadata — nothing for resolution to use, so
        // everything comes from the environment and the live API.
        let path = Path::new("../../libs/ixbrl/example_data/example2/minimal_config.json");
        let file = ConfigFile::from_file(path).expect("parse the minimal config");
        let cli = CliArgs {
            config_path: path.to_path_buf(),
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

        // The profile is cached for the next run.
        let cache_file = env
            .ch_config()
            .cache_dir()
            .join(format!("companies-house-{number}.json"));
        assert!(cache_file.exists(), "the fetched profile is cached on disk");
        let cached = std::fs::read_to_string(&cache_file).expect("read the cache file");
        assert!(
            cached.contains("company_name"),
            "the cache holds the profile JSON"
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
    }
}
