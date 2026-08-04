//! Companies House public API client.
//!
//! A minimal client for the Companies House API
//! (<https://developer.company-information.service.gov.uk/>), authenticated
//! with HTTP basic access authentication.
//!
//! The API key is sent as the basic-auth *username* and the password is left
//! blank, e.g. for an API key `my_api_key`:
//!
//! ```text
//! Authorization: Basic bXlfYXBpX2tleTo=
//! ```

use std::{
    env::VarError,
    path::{Path, PathBuf},
    result::Result,
};

use chrono::{Datelike, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use snafu::Snafu;

use crate::company::{AccountingPeriod, Company};

/// Errors returned by the Companies House API client.
#[derive(Debug, Snafu)]
pub enum CompaniesHouseError {
    /// The HTTP request could not be sent.
    #[snafu(display("request failed: {source}"))]
    RequestFailed { source: reqwest::Error },

    /// The API returned a non-success status code.
    #[snafu(display("GET {url} returned HTTP {status}"))]
    HttpStatus {
        url: String,
        status: reqwest::StatusCode,
    },

    /// The response body could not be decoded as JSON.
    #[snafu(display("failed to decode response: {source}"))]
    DecodeFailed { source: reqwest::Error },

    /// No company number is configured and no full company override was
    /// supplied, so the company cannot be resolved.
    #[snafu(display("no company number configured and no full company override provided"))]
    MissingCompanyNumber,
}

pub type ApiResult<T> = Result<T, CompaniesHouseError>;

/// Base URL of the Companies House public API.
const API_BASE_URL: &str = "https://api.company-information.service.gov.uk";
/// Base URL of the Companies House sandbox API.
const API_BASE_URL_TEST: &str = "https://api-sandbox.company-information.service.gov.uk";

/// The API endpoint half of [`Config`]: base URL + key.
#[derive(Debug, Clone)]
struct ApiConfig {
    base_url: &'static str,
    api_key: String,
}

/// Which Companies House API to talk to, decided by which API key is
/// available: `COMPANIES_HOUSE_API_KEY` (live, preferred) or
/// `COMPANIES_HOUSE_API_KEY_TEST` (sandbox).  The absence of both keys is
/// represented by `Option<Self>` being `None`, so the enum itself has no
/// "none" variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompaniesHouseClientType {
    /// The live API — `COMPANIES_HOUSE_API_KEY` is set.
    Live,
    /// The sandbox API — only `COMPANIES_HOUSE_API_KEY_TEST` is set.
    Sandbox,
}

/// Layered configuration for the Companies House client and the company
/// resolution chain.
///
/// Every value is resolved once, at construction, in priority order: an
/// explicit override set via the `with_*` builder methods, then the
/// environment, then a built-in default.  The environment variables
/// consulted are:
///
/// * `COMPANY_NUMBER` — the company registration number used to resolve
///   absent company details (see [`Config::enrichment_number`] and
///   [`CompaniesHouseClient::resolve_company`]);
/// * `COMPANIES_HOUSE_API_KEY` / `COMPANIES_HOUSE_API_KEY_TEST` — the API
///   key for the live / sandbox Companies House API (live preferred by
///   [`Config::from_env`]);
/// * `CT600_CACHE_DIR` — the response-cache directory, defaulting to the
///   repository's `.cache/api_responses`.
#[derive(Debug, Clone)]
pub struct Config {
    /// The company registration number (`COMPANY_NUMBER`).
    company_number: Option<String>,
    /// The API key and its base URL (live or sandbox), if configured.
    api: Option<ApiConfig>,
    /// The response-cache directory.
    cache_dir: PathBuf,
}

impl Config {
    /// The environment-resolved base: the company number and the cache
    /// directory, without the API layer.  Shared by every constructor.
    fn base_from_env() -> Self {
        Self {
            company_number: non_empty_env("COMPANY_NUMBER"),
            api: None,
            cache_dir: cache_dir_from_env(),
        }
    }

    /// Resolve the configuration from the environment, applying the layered
    /// priority order: explicit overrides > environment > defaults.
    pub fn from_env() -> Self {
        let mut config = Self::base_from_env();
        config.api = api_config_from_env();
        config
    }

    /// Resolve the configuration for the live API, erroring when
    /// `COMPANIES_HOUSE_API_KEY` is unset.
    pub fn live_from_env() -> std::result::Result<Self, VarError> {
        Ok(Self {
            api: Some(ApiConfig {
                base_url: API_BASE_URL,
                api_key: std::env::var("COMPANIES_HOUSE_API_KEY")?,
            }),
            ..Self::base_from_env()
        })
    }

    /// Resolve the configuration for the sandbox API, erroring when
    /// `COMPANIES_HOUSE_API_KEY_TEST` is unset.
    pub fn test_from_env() -> std::result::Result<Self, VarError> {
        Ok(Self {
            api: Some(ApiConfig {
                base_url: API_BASE_URL_TEST,
                api_key: std::env::var("COMPANIES_HOUSE_API_KEY_TEST")?,
            }),
            ..Self::base_from_env()
        })
    }

    // -- explicit overrides ------------------------------------------------

    /// Override the company registration number used for resolution.
    pub fn with_company_number(mut self, company_number: impl Into<String>) -> Self {
        self.company_number = Some(company_number.into());
        self
    }

    /// Override the API key and base URL (`sandbox` selects the sandbox
    /// endpoint, live otherwise).
    pub fn with_api(mut self, api_key: impl Into<String>, sandbox: bool) -> Self {
        self.api = Some(ApiConfig {
            base_url: if sandbox { API_BASE_URL_TEST } else { API_BASE_URL },
            api_key: api_key.into(),
        });
        self
    }

    /// Override the response-cache directory.
    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = cache_dir.into();
        self
    }

    /// Point the API at an unreachable endpoint, for clients that must
    /// never reach the network (tests).
    #[cfg(test)]
    pub(crate) fn with_unreachable_api(mut self) -> Self {
        self.api = Some(ApiConfig {
            base_url: "http://127.0.0.1:1",
            api_key: "unused".to_string(),
        });
        self
    }

    // -- accessors ---------------------------------------------------------

    /// The configured company registration number, if any.
    pub fn company_number(&self) -> Option<&str> {
        self.company_number.as_deref()
    }

    /// The configured API key, if any.
    pub fn api_key(&self) -> Option<&str> {
        self.api.as_ref().map(|api| api.api_key.as_str())
    }

    /// The configured API base URL (the live endpoint by default).
    pub fn base_url(&self) -> &'static str {
        self.api
            .as_ref()
            .map(|api| api.base_url)
            .unwrap_or(API_BASE_URL)
    }

    /// The response-cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Decide whether (and with which company number) to resolve from
    /// Companies House.
    ///
    /// Resolution only ever happens when both conditions hold: a company
    /// number is configured (the `COMPANY_NUMBER` environment variable or a
    /// [`Self::with_company_number`] override) and the company details inputs
    /// are absent (an empty name or registration number).  The lookup number
    /// is always the configured value; `None` means no resolution.
    pub fn enrichment_number(&self, input_name: &str, input_number: &str) -> Option<String> {
        let company_number = self.company_number.as_deref().filter(|n| !n.is_empty())?;
        let inputs_absent = input_name.is_empty() || input_number.is_empty();
        if !inputs_absent {
            return None;
        }
        Some(company_number.to_string())
    }
}

impl Default for Config {
    /// The base configuration: no company number, no API key, and the
    /// default response-cache directory.
    fn default() -> Self {
        Self::base_from_env()
    }
}

/// A client for the Companies House public API.
///
/// All requests are authenticated with the API key using HTTP basic
/// authentication (username = API key, empty password).
///
/// All configuration — the API key / base URL, the company number used to
/// resolve absent company details, and the response-cache directory — lives in
/// a resolved [`Config`].  Company profiles fetched through
/// [`Self::get_company_profile_cached`] are cached on disk
/// (`companies-house-{number}.json`), so repeat lookups for the same company
/// never touch the network.
#[derive(Debug, Clone)]
pub struct CompaniesHouseClient {
    config: Config,
    http: reqwest::Client,
}

impl CompaniesHouseClient {
    /// Build a client from a fully-resolved [`Config`].
    ///
    /// The config must carry an API key (`live_from_env` / `test_from_env`,
    /// or `from_env` / `with_api`) for live lookups; a keyless client only
    /// serves cached profiles.  The keyed constructors below are the usual
    /// entry points.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    /// Create a client for the sandbox API, using the
    /// `COMPANIES_HOUSE_API_KEY_TEST` environment variable.
    pub fn test_client_from_env() -> Result<Self, VarError> {
        Ok(Self::new(Config::test_from_env()?))
    }

    /// Create a client for the live API, using the `COMPANIES_HOUSE_API_KEY`
    /// environment variable.
    pub fn live_from_env() -> Result<Self, VarError> {
        Ok(Self::new(Config::live_from_env()?))
    }

    /// A client pointed at an unreachable address with a placeholder API
    /// key, for tests that must never reach the network.
    #[cfg(test)]
    pub(crate) fn offline() -> Self {
        // `Config::default()` already resolves `COMPANY_NUMBER`; only the
        // unreachable endpoint is added so a cache miss can never hit the
        // network.
        Self::new(Config::default().with_unreachable_api())
    }

    /// The layered configuration this client was built with.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Fetch the company profile for the given company number.
    ///
    /// `GET /company/{companyNumber}`
    pub async fn get_company_profile(&self, company_number: &str) -> ApiResult<CompanyProfile> {
        self.get_json(&format!("/company/{company_number}")).await
    }

    /// Fetch a company's filing history.
    ///
    /// `GET /company/{companyNumber}/filing-history`
    ///
    /// Returns the first page of filings, newest first.  Each item carries the
    /// date the filing was registered (see [`FilingHistoryItem::filed_on`]) and
    /// its category, so previous *accounts* filings are the items with
    /// `category == "accounts"` (see [`FilingHistory::accounts`]).  Like the
    /// company profile, the response is cached on disk
    /// (`companies-house-{number}-filing-history.json`).
    pub async fn get_filing_history(&self, company_number: &str) -> ApiResult<FilingHistory> {
        let cache_dir = self.config.cache_dir();
        if let Some(history) = read_cached_filing_history(cache_dir, company_number) {
            return Ok(history);
        }
        let history: FilingHistory = self
            .get_json(&format!("/company/{company_number}/filing-history"))
            .await?;
        write_cached_filing_history(cache_dir, company_number, &history);
        Ok(history)
    }

    /// The next accounting period to file, with its filing deadlines.
    ///
    /// The period's start and end come from Companies House's own expectation
    /// — the `accounts.next_accounts` block of the company profile (which
    /// reflects any shortened/lengthened periods and accounting-reference-date
    /// changes) — falling back on the *default* accounting periods computed
    /// from the company's registration date ([`Company::accounting_period_n`]
    /// for the period containing today).  The caller's own
    /// `accounting_period_start` / `accounting_period_end` fields are not
    /// consulted: they describe the return period being produced, whereas the
    /// next period to file follows the registration-date schedule (or
    /// Companies House's).
    ///
    /// The Companies House accounts deadline is the profile's `due_on` (or
    /// `next_due`) when present, falling back on 9 months after the period end
    /// (the private-company rule of thumb; public companies have 6 months).
    /// The HMRC CT600 deadline is 12 months after the period end — HMRC's
    /// filing deadline for the corporation-tax return.  Note that for a
    /// company's *first* return the deadline is instead the later of 12 months
    /// after the period end and 3 months after the notice to deliver a return.
    ///
    /// `GET /company/{companyNumber}` (cache-first, see
    /// [`Self::get_company_profile_cached`]).
    pub async fn next_accounting_period(&self, company: &Company) -> ApiResult<NextAccountingPeriod> {
        let company_number = (!company.company_number.is_empty())
            .then(|| company.company_number.as_str())
            .or_else(|| self.config.company_number())
            .filter(|n| !n.is_empty())
            .ok_or(CompaniesHouseError::MissingCompanyNumber)?;
        let profile = self.get_company_profile_cached(company_number).await?;
        Ok(next_accounting_period_from(company, profile.accounts.as_ref()))
    }

    /// `GET {path}` on the configured base URL, authenticated with the API
    /// key as the basic-auth username, decoding the body as JSON.
    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> ApiResult<T> {
        let url = format!("{}{}", self.config.base_url(), path);
        let response = self
            .http
            .get(&url)
            .basic_auth(self.config.api_key().unwrap_or_default(), Some("")) //  Companies House API takes the username as the API key
            .send()
            .await
            .map_err(|source| CompaniesHouseError::RequestFailed { source })?;

        let status = response.status();
        if !status.is_success() {
            return Err(CompaniesHouseError::HttpStatus { url, status });
        }

        response
            .json::<T>()
            .await
            .map_err(|source| CompaniesHouseError::DecodeFailed { source })
    }

    /// Point this client's response cache at a specific directory, overriding
    /// `CT600_CACHE_DIR` and the repo default (`.cache/api_responses`).
    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.config = self.config.with_cache_dir(cache_dir);
        self
    }

    /// The directory cached company profiles are stored in: the per-client
    /// override, else `CT600_CACHE_DIR`, else the repository's
    /// `.cache/api_responses`.
    pub fn cache_dir(&self) -> &Path {
        self.config.cache_dir()
    }

    /// Fetch the company profile for the given company number, serving from
    /// the local response cache when available and falling back on the live
    /// API otherwise (the response is cached for next time).
    ///
    /// Cache reads are best-effort: a missing or corrupt cache entry falls
    /// back on the live API, and a failed cache write is non-fatal.
    pub async fn get_company_profile_cached(
        &self,
        company_number: &str,
    ) -> ApiResult<CompanyProfile> {
        let cache_dir = self.config.cache_dir();
        if let Some(profile) = read_cached_profile(cache_dir, company_number) {
            return Ok(profile);
        }
        let profile = self.get_company_profile(company_number).await?;
        write_cached_profile(cache_dir, company_number, &profile);
        Ok(profile)
    }

    /// Resolve the company to use for the reporting pipeline.
    ///
    /// Resolution order:
    /// 1. a full company override (non-empty name and registration number)
    ///    is returned unchanged;
    /// 2. otherwise the cached Companies House response for the configured
    ///    company number (`COMPANY_NUMBER` / [`Config::with_company_number`])
    ///    is used;
    /// 3. otherwise the profile is fetched from the live API (and cached for
    ///    next time).
    ///
    /// The returned [`Company`] keeps the caller's tax reference and
    /// accounting periods; the name and number are filled from the resolved
    /// profile, and the registration date too when the caller supplied no
    /// details at all.  Resolving fails with
    /// [`CompaniesHouseError::MissingCompanyNumber`] when neither a full
    /// override nor a configured company number is available.
    pub async fn resolve_company(&self, input: &Company) -> ApiResult<Company> {
        // 1. Full company override: nothing to resolve.
        if !input.name.is_empty() && !input.company_number.is_empty() {
            return Ok(input.clone());
        }
        // 2/3. Cache first, then the live API, by the configured number.
        let company_number = self
            .config
            .company_number()
            .filter(|n| !n.is_empty())
            .ok_or(CompaniesHouseError::MissingCompanyNumber)?;
        let profile = self.get_company_profile_cached(company_number).await?;
        Ok(Self::fill_from_profile(input, &profile))
    }

    /// Enrich the company input to the reporting pipeline from Companies
    /// House, filling in the details the caller left absent.
    ///
    /// When a company number is configured (the `COMPANY_NUMBER` environment
    /// variable) and the company's name or registration number is empty, the
    /// profile for that company is fetched (cache-first, see
    /// [`Self::get_company_profile_cached`]) and used to fill in the missing
    /// name and registration number, plus the registration date (from the
    /// profile's date of creation) when the details were fully absent.  A
    /// company with complete details is returned unchanged.
    pub async fn enrich_company(&self, input: &Company) -> ApiResult<Company> {
        let Some(company_number) =
            self.config.enrichment_number(&input.name, &input.company_number)
        else {
            return Ok(input.clone());
        };
        let profile = self.get_company_profile_cached(&company_number).await?;
        Ok(Self::fill_from_profile(input, &profile))
    }

    /// Fill the absent name / number (and registration date, when the input
    /// had no details at all) from a resolved profile.
    fn fill_from_profile(input: &Company, profile: &CompanyProfile) -> Company {
        let details_fully_absent = input.name.is_empty() && input.company_number.is_empty();
        let mut company = input.clone();
        if company.name.is_empty() {
            company.name = profile.company_name.clone();
        }
        if company.company_number.is_empty() {
            company.company_number = profile.company_number.clone();
        }
        // The registration date is only filled when the details were fully
        // absent (the caller supplied nothing but the configured number):
        // with partial inputs the caller's own period dates are left alone so
        // the accounting periods are not skewed.
        if details_fully_absent
            && let Some(created) = profile.date_of_creation.as_deref()
            && let Ok(created) = NaiveDate::parse_from_str(created, "%Y-%m-%d")
        {
            company.registration_date = created;
        }
        company
    }
}

// ============================================================================
// Caching + config-resolution helpers
// ============================================================================

/// The cache file for a company number, e.g. `companies-house-12345678.json`.
fn cache_path(cache_dir: &Path, company_number: &str) -> PathBuf {
    cache_dir.join(format!("companies-house-{company_number}.json"))
}

/// The cache file for a company number's filing history, e.g.
/// `companies-house-12345678-filing-history.json`.
fn filing_history_cache_path(cache_dir: &Path, company_number: &str) -> PathBuf {
    cache_dir.join(format!("companies-house-{company_number}-filing-history.json"))
}

/// Read a cached company profile, if present and decodable.
fn read_cached_profile(cache_dir: &Path, company_number: &str) -> Option<CompanyProfile> {
    let data = std::fs::read_to_string(cache_path(cache_dir, company_number)).ok()?;
    serde_json::from_str(&data).ok()
}

/// Write a company profile to the cache, best-effort (non-fatal on failure).
fn write_cached_profile(cache_dir: &Path, company_number: &str, profile: &CompanyProfile) {
    if let Ok(data) = serde_json::to_vec(profile) {
        write_cache_file(cache_dir, &format!("companies-house-{company_number}.json"), &data);
    }
}

/// Read a cached filing history, if present and decodable.
fn read_cached_filing_history(cache_dir: &Path, company_number: &str) -> Option<FilingHistory> {
    let data = std::fs::read_to_string(filing_history_cache_path(cache_dir, company_number)).ok()?;
    serde_json::from_str(&data).ok()
}

/// Write a filing history to the cache, best-effort (non-fatal on failure).
fn write_cached_filing_history(cache_dir: &Path, company_number: &str, history: &FilingHistory) {
    if let Ok(data) = serde_json::to_vec(history) {
        write_cache_file(
            cache_dir,
            &format!("companies-house-{company_number}-filing-history.json"),
            &data,
        );
    }
}

/// Write a cache file under the cache directory, best-effort (non-fatal on
/// failure).
fn write_cache_file(cache_dir: &Path, file_name: &str, data: &[u8]) {
    let result = std::fs::create_dir_all(cache_dir).and_then(|_| std::fs::write(cache_dir.join(file_name), data));
    if let Err(e) = result {
        log::warn!("failed to write Companies House cache: {e}");
    }
}

/// The default cache directory: `CT600_CACHE_DIR` when set, else the
/// repository's `.cache/api_responses` (the same location the offline test
/// fixtures are served from), else a relative `.cache/api_responses`.
fn cache_dir_from_env() -> PathBuf {
    if let Ok(dir) = std::env::var("CT600_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let repo_root = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| PathBuf::from(s.trim()));
    repo_root
        .map(|root| root.join(".cache/api_responses"))
        .unwrap_or_else(|| PathBuf::from(".cache/api_responses"))
}

/// A non-empty environment variable, if set.
fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The API endpoint configuration from the environment: the live key
/// (`COMPANIES_HOUSE_API_KEY`) preferred, the sandbox key
/// (`COMPANIES_HOUSE_API_KEY_TEST`) as fallback, `None` when neither is set.
fn api_config_from_env() -> Option<ApiConfig> {
    if let Some(api_key) = non_empty_env("COMPANIES_HOUSE_API_KEY") {
        return Some(ApiConfig {
            base_url: API_BASE_URL,
            api_key,
        });
    }
    if let Some(api_key) = non_empty_env("COMPANIES_HOUSE_API_KEY_TEST") {
        return Some(ApiConfig {
            base_url: API_BASE_URL_TEST,
            api_key,
        });
    }
    None
}

/// The CT600 box 4 "Type of company" options, as defined in the HMRC CT600
/// guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanyType {
    PrivateCompanyLimitedByShares,
    PrivateCompanyLimitedByGuarantee,
    PrivateUnlimitedCompany,
    PublicLimitedCompany,
    OldPublicCompany,
    PrivateCompanyLimitedBySharesExempt,
    LimitedLiabilityPartnership,
}

impl CompanyType {
    /// Parse a Companies House company type string into a [`CompanyType`].
    ///
    /// Returns `None` for types not recognised.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "ltd" => Some(Self::PrivateCompanyLimitedByShares),
            "private-limited-guarant-nsc" => Some(Self::PrivateCompanyLimitedByGuarantee),
            "private-unlimited" | "unltd" => Some(Self::PrivateUnlimitedCompany),
            "plc" => Some(Self::PublicLimitedCompany),
            "old-public-company" => Some(Self::OldPublicCompany),
            "private-limited-shares-section-30-exemption" => {
                Some(Self::PrivateCompanyLimitedBySharesExempt)
            }
            "llp" => Some(Self::LimitedLiabilityPartnership),
            _ => None,
        }
    }

    /// The CT600 box 4 code for this company type.
    pub fn code(self) -> u8 {
        match self {
            Self::PrivateCompanyLimitedByShares => 1,
            Self::PrivateCompanyLimitedByGuarantee => 2,
            Self::PrivateUnlimitedCompany => 3,
            Self::PublicLimitedCompany => 4,
            Self::OldPublicCompany => 5,
            Self::PrivateCompanyLimitedBySharesExempt => 6,
            Self::LimitedLiabilityPartnership => 9,
        }
    }
}

/// Company profile returned by `GET /company/{companyNumber}`.
///
/// Only the commonly used fields are modelled; all optional fields are
/// tolerated when absent from the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProfile {
    pub company_number: String,
    pub company_name: String,
    #[serde(default)]
    pub company_status: Option<String>,
    #[serde(default)]
    pub company_status_detail: Option<String>,
    #[serde(default)]
    pub date_of_creation: Option<String>,
    #[serde(default)]
    pub date_of_dissolution: Option<String>,
    #[serde(rename = "type", default)]
    pub company_type: Option<String>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub registered_office_address: Option<RegisteredOfficeAddress>,
    #[serde(default)]
    pub accounts: Option<Accounts>,
    #[serde(default)]
    pub confirmation_statement: Option<ConfirmationStatement>,
    #[serde(default)]
    pub sic_codes: Option<Vec<String>>,
    #[serde(default)]
    pub undeliverable_registered_office_address: Option<bool>,
    #[serde(default)]
    pub links: Option<CompanyLinks>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredOfficeAddress {
    #[serde(default)]
    pub address_line_1: Option<String>,
    #[serde(default)]
    pub address_line_2: Option<String>,
    #[serde(default)]
    pub care_of: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub locality: Option<String>,
    #[serde(default)]
    pub po_box: Option<String>,
    #[serde(default)]
    pub postal_code: Option<String>,
    #[serde(default)]
    pub premises: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Accounts {
    #[serde(default)]
    pub next_accounts: Option<NextAccounts>,
    #[serde(default)]
    pub last_accounts: Option<LastAccounts>,
    #[serde(default)]
    pub next_due: Option<String>,
    #[serde(default)]
    pub overdue: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAccounts {
    #[serde(default)]
    pub period_end_on: Option<String>,
    #[serde(default)]
    pub period_start_on: Option<String>,
    #[serde(default)]
    pub due_on: Option<String>,
}

/// The previous accounts period as reported in the company profile
/// (`accounts.last_accounts`): the period the most recently filed accounts
/// covered, when they were due, and their form type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastAccounts {
    /// The last date the accounts were made up to (the ARD).
    #[serde(default)]
    pub made_up_to: Option<String>,
    #[serde(default)]
    pub period_start_on: Option<String>,
    #[serde(default)]
    pub period_end_on: Option<String>,
    #[serde(default)]
    pub due_on: Option<String>,
    /// Form type, e.g. `AA` (full accounts) or `AA02` (micro-entity accounts).
    #[serde(rename = "type", default)]
    pub form_type: Option<String>,
}

/// A single entry in a company's filing history (`GET /company/{number}/filing-history`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilingHistoryItem {
    /// Date the filing was registered by Companies House (`YYYY-MM-DD`).
    #[serde(default)]
    pub date: Option<String>,
    /// Filing category, e.g. `accounts`, `confirmation-statement`.
    #[serde(default)]
    pub category: Option<String>,
    /// Form type code, e.g. `AA` (accounts), `AA01` (change of accounting
    /// reference date), `CS01` (confirmation statement).
    #[serde(rename = "type", default)]
    pub form_type: Option<String>,
    /// Human-readable description of the filing.
    #[serde(default)]
    pub description: Option<String>,
    /// Resource links for the filing.
    #[serde(default)]
    pub links: Option<FilingHistoryLinks>,
}

/// Resource links carried by a filing-history item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilingHistoryLinks {
    /// Link to the filing-history item itself.
    #[serde(rename = "self", default)]
    pub self_link: Option<String>,
    /// Link to the filed document's metadata (when a document exists).
    #[serde(default)]
    pub document_metadata: Option<String>,
}

impl FilingHistoryItem {
    /// The date the filing was registered with Companies House.
    pub fn filed_on(&self) -> Option<NaiveDate> {
        self.date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    }
}

/// The filing history of a company (`GET /company/{number}/filing-history`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilingHistory {
    /// Total number of filings for the company (may exceed `items.len()` when
    /// the history spans more than one page).
    #[serde(default)]
    pub total_count: Option<usize>,
    /// The (first page of) filing-history items, newest first.
    #[serde(default)]
    pub items: Vec<FilingHistoryItem>,
}

impl FilingHistory {
    /// The filings in the `accounts` category — i.e. the previous accounts
    /// filed, with their registration dates (see
    /// [`FilingHistoryItem::filed_on`]).
    pub fn accounts(&self) -> impl Iterator<Item = &FilingHistoryItem> {
        self.items
            .iter()
            .filter(|item| item.category.as_deref() == Some("accounts"))
    }
}

/// The next accounting period to file, with its filing deadlines.
///
/// See [`CompaniesHouseClient::next_accounting_period`] for how the dates are
/// resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct NextAccountingPeriod {
    /// Start of the next accounting period to file.
    pub start: NaiveDate,
    /// End of the next accounting period to file.
    pub end: NaiveDate,
    /// Deadline to file the CT600 corporation-tax return with HMRC (12 months
    /// after the period end).
    pub deadline_to_file_hmrc_ct600: NaiveDate,
    /// Deadline to file the accounts with Companies House (the profile's
    /// next-accounts due date, else 9 months after the period end).
    pub deadline_to_file_companies_house_accounts: NaiveDate,
}

/// Resolve the next accounting period from the company's registration-date
/// periods (the default), overlaid with the profile's own expectation when
/// available.
fn next_accounting_period_from(company: &Company, accounts: Option<&Accounts>) -> NextAccountingPeriod {
    let today = chrono::Utc::now().date_naive();
    let default = company.accounting_period_containing(today);

    let next = accounts.and_then(|accounts| accounts.next_accounts.as_ref());
    let parse = |date: Option<&String>| {
        date.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    };

    let profile_start = parse(next.and_then(|n| n.period_start_on.as_ref()));
    let profile_end = parse(next.and_then(|n| n.period_end_on.as_ref()));

    // Anchor the period on whichever edge the profile provides; a missing
    // edge is filled from the registration-date schedule for the anchor, so
    // the two edges always stay coherent (never a profile end mixed with a
    // today-anchored start from a different period).
    let end = profile_end.unwrap_or_else(|| {
        profile_start
            .map(|start| company.accounting_period_containing(start).end)
            .unwrap_or(default.end)
    });
    let start = profile_start
        .or_else(|| Some(schedule_period_for_end(company, end).start))
        .unwrap_or(default.start);
    let companies_house_deadline = parse(next.and_then(|n| n.due_on.as_ref()))
        .or_else(|| parse(accounts.and_then(|a| a.next_due.as_ref())))
        .unwrap_or_else(|| end + Months::new(9));

    NextAccountingPeriod {
        start,
        end,
        deadline_to_file_hmrc_ct600: end + Months::new(12),
        deadline_to_file_companies_house_accounts: companies_house_deadline,
    }
}

/// The registration-schedule period ending on (or nearest to) `end`.
///
/// `Company::accounting_period_containing` cannot invert an end date: it maps
/// any date in the accounting-reference month to the *following* period.  This
/// instead computes the candidate period index from the end date itself and
/// picks the candidate whose end is closest (exact for the schedule-aligned
/// ends the fallback produces, nearest for non-aligned ends such as shortened
/// periods).
fn schedule_period_for_end(company: &Company, end: NaiveDate) -> AccountingPeriod {
    let ard = company.first_ard();
    let months = (end.year() * 12 + end.month() as i32) - (ard.year() * 12 + ard.month() as i32);
    // For n >= 2 the schedule ends are `ard + 12*(n-1)` months.
    let candidate_n = (months.max(0) / 12) as u32 + 1;
    let candidates = [
        company.accounting_period_n(0),
        company.accounting_period_n(1),
        company.accounting_period_n(candidate_n.saturating_sub(1)),
        company.accounting_period_n(candidate_n),
        company.accounting_period_n(candidate_n + 1),
    ];
    candidates
        .into_iter()
        .min_by_key(|period| (period.end - end).num_days().abs())
        .expect("the candidate array is never empty")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationStatement {
    #[serde(default)]
    pub last_made_up_to: Option<String>,
    #[serde(default)]
    pub next_due: Option<String>,
    #[serde(default)]
    pub next_made_up_to: Option<String>,
    #[serde(default)]
    pub overdue: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyLinks {
    #[serde(default)]
    pub filing_history: Option<String>,
    #[serde(default)]
    pub officers: Option<String>,
    #[serde(default)]
    pub self_link: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_profile(company_number: &str, company_name: &str) -> CompanyProfile {
        CompanyProfile {
            company_number: company_number.to_string(),
            company_name: company_name.to_string(),
            company_status: Some("active".to_string()),
            company_status_detail: None,
            date_of_creation: Some("2001-01-01".to_string()),
            date_of_dissolution: None,
            company_type: Some("ltd".to_string()),
            jurisdiction: Some("England/Wales".to_string()),
            registered_office_address: None,
            accounts: None,
            confirmation_statement: None,
            sic_codes: None,
            undeliverable_registered_office_address: None,
            links: None,
        }
    }

    fn seed_cache(cache_dir: &Path, profile: &CompanyProfile) {
        std::fs::create_dir_all(cache_dir).unwrap();
        std::fs::write(
            cache_path(cache_dir, &profile.company_number),
            serde_json::to_vec(profile).unwrap(),
        )
        .unwrap();
    }

    /// The cached profile is served without any network access (the inner
    /// client points at an unreachable address).
    #[tokio::test]
    async fn get_company_profile_cached_serves_from_cache_without_network() {
        let cache_dir = tempfile::tempdir().unwrap();
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());

        let profile = fixture_profile("12345678", "CACHED PRODUCTS LTD");
        seed_cache(cache_dir.path(), &profile);

        let cached = client
            .get_company_profile_cached("12345678")
            .await
            .expect("serving from cache should succeed");

        assert_eq!(cached.company_name, "CACHED PRODUCTS LTD");
        assert_eq!(cached.company_type.as_deref(), Some("ltd"));
    }

    /// Resolution is gated on a configured company number and absent inputs.
    #[test]
    fn enrichment_number_gates_on_configured_number_and_absent_inputs() {
        // No company number configured: no resolution, whatever the inputs.
        let config = Config::default();
        assert_eq!(config.enrichment_number("Acme Ltd", "12345678"), None);
        assert_eq!(config.enrichment_number("", "12345678"), None);
        assert_eq!(config.enrichment_number("", ""), None);

        // Complete inputs: no resolution, even with a number configured.
        let config = Config::default().with_company_number("12345678");
        assert_eq!(config.enrichment_number("Acme Ltd", "12345678"), None);

        // Absent inputs + configured number: resolve with it.
        assert_eq!(config.enrichment_number("", ""), Some("12345678".to_string()));
        assert_eq!(config.enrichment_number("", "12345678"), Some("12345678".to_string()));

        // An empty configured number is treated as unset.
        assert_eq!(
            Config::default().with_company_number("").enrichment_number("", ""),
            None
        );
    }

    /// A full company override short-circuits the resolution chain: no cache
    /// entry, no configured number, no network access.
    #[tokio::test]
    async fn resolve_company_uses_full_override_first() {
        let cache_dir = tempfile::tempdir().unwrap();
        let config = Config::default()
            .with_company_number("12345678")
            .with_cache_dir(cache_dir.path());
        let client = CompaniesHouseClient::new(config.with_unreachable_api());

        let full = Company::new(
            "Acme Ltd",
            "1234567890",
            "9876543",
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        let resolved = client.resolve_company(&full).await.expect("full override");
        assert_eq!(resolved.name, "Acme Ltd");
        assert_eq!(resolved.company_number, "9876543");
        assert_eq!(resolved.registration_date, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
    }

    /// Otherwise the cached response for the configured company number is
    /// used (no network), keeping the caller's tax reference and period.
    #[tokio::test]
    async fn resolve_company_fills_from_cache_by_configured_number() {
        let cache_dir = tempfile::tempdir().unwrap();
        seed_cache(cache_dir.path(), &fixture_profile("12345678", "CACHED CORP LTD"));
        let config = Config::default()
            .with_company_number("12345678")
            .with_cache_dir(cache_dir.path());
        let client = CompaniesHouseClient::new(config.with_unreachable_api());

        let partial = Company::new(
            "",
            "1234567890",
            "",
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        let resolved = client.resolve_company(&partial).await.expect("cache hit");
        assert_eq!(resolved.name, "CACHED CORP LTD");
        assert_eq!(resolved.company_number, "12345678");
        assert_eq!(resolved.tax_reference, "1234567890");
        assert_eq!(resolved.registration_date, NaiveDate::from_ymd_opt(2001, 1, 1).unwrap());
    }

    /// Without a full override or a configured company number the resolution
    /// fails.
    #[tokio::test]
    async fn resolve_company_errors_without_number_or_override() {
        let cache_dir = tempfile::tempdir().unwrap();
        let config = Config::default().with_cache_dir(cache_dir.path());
        let client = CompaniesHouseClient::new(config.with_unreachable_api());

        let partial = Company::new(
            "",
            "",
            "",
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        let err = client.resolve_company(&partial).await.expect_err("cannot resolve");
        assert!(matches!(err, CompaniesHouseError::MissingCompanyNumber));
    }

    /// The `COMPANY_NUMBER`-gated paths, run together in one test so the
    /// environment variable is never contended with a parallel test.  All
    /// scenarios use an offline client, so any accidental network access (or
    /// a cache miss) would fail the test.
    #[tokio::test]
    async fn company_number_env_drives_enrichment() {
        let cache_dir = tempfile::tempdir().unwrap();
        seed_cache(cache_dir.path(), &fixture_profile("12345678", "EXAMPLE CORP LTD"));

        // 1. Config::from_env resolves COMPANY_NUMBER, and a client built
        //    from the resolved config enriches absent details from the cache.
        unsafe { std::env::set_var("COMPANY_NUMBER", "12345678") };
        let config = Config::from_env();
        assert_eq!(config.company_number(), Some("12345678"));
        assert_eq!(config.enrichment_number("", ""), Some("12345678".to_string()));
        assert_eq!(config.enrichment_number("Acme Ltd", "12345678"), None);

        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());
        let company = Company::new(
            "",
            "",
            "",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let enriched = client.enrich_company(&company).await.expect("enrich from cache");
        assert_eq!(enriched.name, "EXAMPLE CORP LTD");
        assert_eq!(enriched.company_number, "12345678");
        assert_eq!(enriched.registration_date, NaiveDate::from_ymd_opt(2001, 1, 1).unwrap());
        unsafe { std::env::remove_var("COMPANY_NUMBER") };

        // 2. Without COMPANY_NUMBER, a client resolved afterwards leaves the
        //    same absent inputs alone (no cache lookup, no network).
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());
        let company = Company::new(
            "",
            "",
            "",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let untouched = client.enrich_company(&company).await.expect("no env, no fetch");
        assert_eq!(untouched.name, "");
        assert_eq!(untouched.company_number, "");

        // 3. Complete details are never enriched, even with the env var set
        //    (the cache holds a different profile and the client is offline).
        unsafe { std::env::set_var("COMPANY_NUMBER", "12345678") };
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());
        let company = Company::new(
            "Acme Ltd",
            "1234567890",
            "12345678",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let enriched = client.enrich_company(&company).await.expect("no fetch needed");
        assert_eq!(enriched.name, "Acme Ltd");
        assert_eq!(enriched.registration_date, NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        unsafe { std::env::remove_var("COMPANY_NUMBER") };
    }

    fn seed_filing_history(cache_dir: &Path, company_number: &str, history: &FilingHistory) {
        std::fs::create_dir_all(cache_dir).unwrap();
        std::fs::write(
            filing_history_cache_path(cache_dir, company_number),
            serde_json::to_vec(history).unwrap(),
        )
        .unwrap();
    }

    /// The cached filing history is served without any network access, and the
    /// `accounts` filter surfaces the previous accounts filings with their
    /// registration dates.
    #[tokio::test]
    async fn get_filing_history_serves_from_cache_and_filters_accounts() {
        let cache_dir = tempfile::tempdir().unwrap();
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());

        let history = FilingHistory {
            total_count: Some(3),
            items: vec![
                FilingHistoryItem {
                    date: Some("2025-06-30".to_string()),
                    category: Some("accounts".to_string()),
                    form_type: Some("AA02".to_string()),
                    description: Some("micro-entity accounts".to_string()),
                    links: None,
                },
                FilingHistoryItem {
                    date: Some("2025-06-01".to_string()),
                    category: Some("confirmation-statement".to_string()),
                    form_type: Some("CS01".to_string()),
                    description: None,
                    links: None,
                },
                FilingHistoryItem {
                    date: Some("2024-06-30".to_string()),
                    category: Some("accounts".to_string()),
                    form_type: Some("AA02".to_string()),
                    description: Some("micro-entity accounts".to_string()),
                    links: None,
                },
            ],
        };
        seed_filing_history(cache_dir.path(), "12345678", &history);

        let fetched = client
            .get_filing_history("12345678")
            .await
            .expect("serving from cache should succeed");
        assert_eq!(fetched.total_count, Some(3));

        let accounts: Vec<_> = fetched.accounts().collect();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].form_type.as_deref(), Some("AA02"));
        assert_eq!(accounts[0].filed_on(), Some(NaiveDate::from_ymd_opt(2025, 6, 30).unwrap()));
        assert_eq!(accounts[1].filed_on(), Some(NaiveDate::from_ymd_opt(2024, 6, 30).unwrap()));
    }

    /// A company with registration 2024-01-01, whose cached profile announces
    /// next accounts 2025-01-01..2025-12-31 due 2026-09-30.
    fn next_accounts_profile(cache_dir: &Path) {
        let mut profile = fixture_profile("12345678", "ACCOUNTS CORP LTD");
        profile.accounts = Some(Accounts {
            next_accounts: Some(NextAccounts {
                period_start_on: Some("2025-01-01".to_string()),
                period_end_on: Some("2025-12-31".to_string()),
                due_on: Some("2026-09-30".to_string()),
            }),
            last_accounts: None,
            next_due: Some("2026-09-30".to_string()),
            overdue: Some(false),
        });
        seed_cache(cache_dir, &profile);
    }

    /// The profile's own `next_accounts` wins over the registration-date
    /// periods, and the deadlines are derived from it.
    #[tokio::test]
    async fn next_accounting_period_prefers_profile_next_accounts() {
        let cache_dir = tempfile::tempdir().unwrap();
        next_accounts_profile(cache_dir.path());
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());

        let company = Company::new(
            "",
            "1234567890",
            "12345678",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let next = client
            .next_accounting_period(&company)
            .await
            .expect("next period from profile");

        assert_eq!(next.start, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        assert_eq!(next.end, NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
        assert_eq!(
            next.deadline_to_file_companies_house_accounts,
            NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()
        );
        // CT600 deadline: 12 months after the period end.
        assert_eq!(
            next.deadline_to_file_hmrc_ct600,
            NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()
        );
    }

    /// Without `next_accounts`, the period is the registration-date default
    /// containing today, and the Companies House deadline falls back on 9
    /// months after the period end (this test pins the fallback; the
    /// registration date makes the default periods deterministic relative to
    /// `today`).
    #[tokio::test]
    async fn next_accounting_period_falls_back_to_registration_periods() {
        let cache_dir = tempfile::tempdir().unwrap();
        seed_cache(
            cache_dir.path(),
            &fixture_profile("12345678", "PLAIN CORP LTD"), // accounts: None
        );
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());

        let company = Company::new(
            "",
            "1234567890",
            "12345678",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let next = client
            .next_accounting_period(&company)
            .await
            .expect("fallback period");

        let expected = company.accounting_period_containing(chrono::Utc::now().date_naive());
        assert_eq!(next.start, expected.start);
        assert_eq!(next.end, expected.end);
        assert_eq!(
            next.deadline_to_file_companies_house_accounts,
            expected.end + Months::new(9)
        );
        assert_eq!(next.deadline_to_file_hmrc_ct600, expected.end + Months::new(12));
    }

    /// A `next_accounts` block missing `due_on` falls back on `next_due`.
    #[tokio::test]
    async fn next_accounting_period_falls_back_on_next_due() {
        let cache_dir = tempfile::tempdir().unwrap();
        let mut profile = fixture_profile("12345678", "PARTIAL CORP LTD");
        profile.accounts = Some(Accounts {
            next_accounts: Some(NextAccounts {
                period_start_on: Some("2025-01-01".to_string()),
                period_end_on: Some("2025-12-31".to_string()),
                due_on: None,
            }),
            last_accounts: None,
            next_due: Some("2026-10-15".to_string()),
            overdue: Some(false),
        });
        seed_cache(cache_dir.path(), &profile);
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());

        let company = Company::new(
            "",
            "1234567890",
            "12345678",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let next = client
            .next_accounting_period(&company)
            .await
            .expect("next period from profile");
        assert_eq!(
            next.deadline_to_file_companies_house_accounts,
            NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
        );
        assert_eq!(next.end, NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
    }

    /// A `next_accounts` block with only `period_start_on` anchors the period
    /// on that start: the end is filled from the registration-date schedule
    /// for the same period, so the two edges stay coherent.
    #[tokio::test]
    async fn next_accounting_period_anchors_on_profile_start_when_end_missing() {
        let cache_dir = tempfile::tempdir().unwrap();
        let mut profile = fixture_profile("12345678", "START ONLY CORP LTD");
        profile.accounts = Some(Accounts {
            next_accounts: Some(NextAccounts {
                period_start_on: Some("2025-01-01".to_string()),
                period_end_on: None,
                due_on: None,
            }),
            last_accounts: None,
            next_due: None,
            overdue: Some(false),
        });
        seed_cache(cache_dir.path(), &profile);
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());

        let company = Company::new(
            "",
            "1234567890",
            "12345678",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let next = client
            .next_accounting_period(&company)
            .await
            .expect("anchored period");

        assert_eq!(next.start, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        // The registration-date period containing 2025-01-01.
        let anchored = company.accounting_period_containing(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        assert_eq!(next.end, anchored.end);
        assert_eq!(next.deadline_to_file_hmrc_ct600, anchored.end + Months::new(12));
        assert_eq!(
            next.deadline_to_file_companies_house_accounts,
            anchored.end + Months::new(9)
        );
    }
}
