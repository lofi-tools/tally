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

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use snafu::Snafu;

use crate::company::Company;

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
        let url = format!("{}/company/{company_number}", self.config.base_url());
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
            .json::<CompanyProfile>()
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

/// Read a cached company profile, if present and decodable.
fn read_cached_profile(cache_dir: &Path, company_number: &str) -> Option<CompanyProfile> {
    let data = std::fs::read_to_string(cache_path(cache_dir, company_number)).ok()?;
    serde_json::from_str(&data).ok()
}

/// Write a company profile to the cache, best-effort (non-fatal on failure).
fn write_cached_profile(cache_dir: &Path, company_number: &str, profile: &CompanyProfile) {
    let result = serde_json::to_vec(profile)
        .map_err(|e| std::io::Error::other(e.to_string()))
        .and_then(|data| {
            std::fs::create_dir_all(cache_dir)?;
            std::fs::write(cache_path(cache_dir, company_number), data)
        });
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
}
