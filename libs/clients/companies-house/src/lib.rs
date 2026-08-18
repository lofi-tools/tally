//! Companies House public API client.
//!
//! The client ([`CompaniesHouseClient`]) is a minimal client for the
//! Companies House API (<https://developer.company-information.service.gov.uk/>),
//! authenticated with HTTP basic access authentication (username = API key,
//! empty password), with a layered [`Config`] and an optional disk cache for
//! fetched profiles (enabled by `CT600_CACHE_DIR` or
//! [`Config::with_cache_dir`]), and the company-resolution /
//! next-accounting-period chain.  It also parses the iXBRL accounts
//! documents Companies House files ([`parse_filed_accounts`] →
//! [`FiledBalanceSheet`]) and the typed filing kinds ([`TypedFiling`]).
//!
//! The offline test fixtures ([`test_utils`]) and the live API tests
//! ([`live_tests`], part of the default-enabled `cached_live_tests` feature) live
//! alongside the client.

use chrono::{Datelike, Months, NaiveDate};
use core_model::{AccountingPeriod, Company};
use ixbrl_ir::ixbrl_fmt::{ParsedIxBrlFacts, XmlNode, xbrl_context_dimensions};
use serde::{Deserialize, Serialize};

pub use core_model::PreviousYearFigures;
use snafu::Snafu;
use std::env::VarError;
use std::path::{Path, PathBuf};
use std::result::Result;

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

    /// A filing-history item carries no downloadable document.
    #[snafu(display("filing has no downloadable document"))]
    NoDocument,

    /// The filing id was not found in the company's filing history.
    #[snafu(display(
        "filing {filing_id} not found in the filing history of company {company_number}"
    ))]
    FilingNotFound {
        company_number: String,
        filing_id: String,
    },

    /// The filing kind is not implemented yet: its `description_values` keys
    /// are unverified (no real filing of this kind to check against), so the
    /// typed parse refuses it instead of guessing.
    #[snafu(display("filing type {filing_type} is not implemented yet"))]
    Unimplemented { filing_type: String },
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
/// `COMPANIES_HOUSE_SANDBOX_API_KEY` (sandbox).  The absence of both keys is
/// represented by `Option<Self>` being `None`, so the enum itself has no
/// "none" variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompaniesHouseClientType {
    /// The live API — `COMPANIES_HOUSE_API_KEY` is set.
    Live,
    /// The sandbox API — only `COMPANIES_HOUSE_SANDBOX_API_KEY` is set.
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
/// * `COMPANIES_HOUSE_API_KEY` / `COMPANIES_HOUSE_SANDBOX_API_KEY` — the API
///   key for the live / sandbox Companies House API (live preferred by
///   [`Config::from_env`]);
/// * `CT600_CACHE_DIR` — the response-cache directory; when unset, test
///   builds default to the repository's `.cache/api_responses` and other
///   builds have no disk cache.
#[derive(Debug, Clone)]
pub struct Config {
    /// The company registration number (`COMPANY_NUMBER`).
    company_number: Option<String>,
    /// The API key and its base URL (live or sandbox), if configured.
    api: Option<ApiConfig>,
    /// The response-cache directory, when configured (`CT600_CACHE_DIR` or an
    /// explicit [`Config::with_cache_dir`]); `None` disables the disk cache.
    cache_dir: Option<PathBuf>,
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
    /// `COMPANIES_HOUSE_SANDBOX_API_KEY` is unset.
    pub fn test_from_env() -> std::result::Result<Self, VarError> {
        Ok(Self {
            api: Some(ApiConfig {
                base_url: API_BASE_URL_TEST,
                api_key: std::env::var("COMPANIES_HOUSE_SANDBOX_API_KEY")?,
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
            base_url: if sandbox {
                API_BASE_URL_TEST
            } else {
                API_BASE_URL
            },
            api_key: api_key.into(),
        });
        self
    }

    /// Override the response-cache directory, enabling the disk cache.
    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(cache_dir.into());
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

    /// The response-cache directory, when configured.
    pub fn cache_dir(&self) -> Option<&Path> {
        self.cache_dir.as_deref()
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
    /// response cache from `CT600_CACHE_DIR` (the repository's
    /// `.cache/api_responses` in test builds, none when unset).
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
/// resolve absent company details, and the optional response-cache directory —
/// lives in a resolved [`Config`].  Company profiles fetched through
/// [`Self::get_company_profile_cached`] are cached on disk
/// (`companies-house-{number}.json`) when a cache directory is configured, so
/// repeat lookups for the same company never touch the network.
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
    /// `COMPANIES_HOUSE_SANDBOX_API_KEY` environment variable.
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
    /// (`companies-house-{number}-filing-history.json`) when a cache directory
    /// is configured; without one it is always fetched from the API.
    pub async fn get_filing_history(&self, company_number: &str) -> ApiResult<FilingHistory> {
        if let Some(cache_dir) = self.config.cache_dir()
            && let Some(history) = read_cached_filing_history(cache_dir, company_number)
        {
            return Ok(history);
        }
        let history: FilingHistory = self
            .get_json(&format!("/company/{company_number}/filing-history"))
            .await?;
        if let Some(cache_dir) = self.config.cache_dir() {
            write_cached_filing_history(cache_dir, company_number, &history);
        }
        Ok(history)
    }

    /// Fetch a company's **complete** filing history afresh, following the
    /// pagination until every filing is collected.
    ///
    /// `GET /company/{companyNumber}/filing-history?items_per_page=100&page=N`
    ///
    /// Unlike [`Self::get_filing_history`], this always hits the API — the
    /// disk cache is never consulted — and merges every page into one
    /// [`FilingHistory`] (newest first).  Callers that persist the result
    /// (e.g. the tally-api filings sync) own their own storage.  Stops early
    /// at an empty page, and caps at a sane number of pages so a malformed
    /// response can't spin forever.
    pub async fn refetch_filings(&self, company_number: &str) -> ApiResult<FilingHistory> {
        const ITEMS_PER_PAGE: usize = 100;
        const MAX_PAGES: usize = 20; // 2,000 filings is well beyond any real company

        let mut items = Vec::new();
        let mut total_count: Option<usize> = None;
        for page in 1..=MAX_PAGES {
            let history: FilingHistory = self
                .get_json(&format!(
                    "/company/{company_number}/filing-history?items_per_page={ITEMS_PER_PAGE}&page={page}"
                ))
                .await?;
            total_count = history.total_count.or(total_count);
            let fetched = history.items.len();
            items.extend(history.items);
            if fetched == 0 {
                break;
            }
            if total_count.is_some_and(|total| items.len() >= total) {
                break;
            }
        }
        Ok(FilingHistory { total_count, items })
    }

    /// Download a filed document from the document API, resolving the
    /// metadata link carried by a filing-history item to its content URL.
    ///
    /// The document API is a separate host from the main API (live
    /// `https://document-api.companieshouse.gov.uk`); the same API key is
    /// used as the basic-auth username. Returns the raw document bytes; the
    /// caller decides how to interpret them (HTML iXBRL, zipped iXBRL, PDF,
    /// ...).
    pub async fn get_filing_document(&self, document_metadata_url: &str) -> ApiResult<Vec<u8>> {
        let metadata: DocumentMetadata = self.get_url_json(document_metadata_url).await?;
        let content_path = metadata
            .links
            .document
            .ok_or(CompaniesHouseError::NoDocument)?;
        let content_url = resolve_absolute(document_metadata_url, &content_path);
        self.get_url_bytes(&content_url).await
    }

    /// Download a filing's document by its filing (transaction) ID — the
    /// trailing path segment of the filing-history item's `links.self` (e.g.
    /// `MzA1OTg4NDcwMDY5`), stable across refetches.
    ///
    /// The filing is located via the (cache-first) filing history, so a warm
    /// history cache resolves the item without touching the API.  The
    /// downloaded bytes are served from the cache first — the
    /// `filings_downloads` subdirectory of the response cache, named
    /// `{company_number}-{period_end}-{filing_id}` (the period end, falling
    /// back to the filed-on date, makes the cache queriable by company +
    /// period; the id disambiguates filings sharing a date) — and only
    /// fetched from the document API on a miss.  Without a configured cache
    /// directory every call hits the API.
    pub async fn download_filing(
        &self,
        company_number: &str,
        filing_id: &str,
    ) -> ApiResult<Vec<u8>> {
        let history = self.get_filing_history(company_number).await?;
        let item = history
            .items
            .iter()
            .find(|item| item.transaction_id().as_deref() == Some(filing_id))
            .ok_or(CompaniesHouseError::FilingNotFound {
                company_number: company_number.to_string(),
                filing_id: filing_id.to_string(),
            })?;
        let date = ChFiling::from(item).period_end.or_else(|| item.filed_on());

        if let Some(cache_dir) = self.config.cache_dir()
            && let Some(bytes) =
                read_cached_filing_download(cache_dir, company_number, date, filing_id)
        {
            return Ok(bytes);
        }

        let metadata_url = item
            .links
            .as_ref()
            .and_then(|l| l.document_metadata.as_deref())
            .ok_or(CompaniesHouseError::NoDocument)?;
        let bytes = self.get_filing_document(metadata_url).await?;
        if let Some(cache_dir) = self.config.cache_dir() {
            write_cached_filing_download(cache_dir, company_number, date, filing_id, &bytes);
        }
        Ok(bytes)
    }

    /// Fetch a company's officers.
    ///
    /// `GET /company/{companyNumber}/officers`
    ///
    /// Returns the officers of the company (directors, secretaries, ...);
    /// the current directors — the officers whose role is a director role
    /// and who have not resigned — are exposed via [`OfficerList::directors`].
    /// Like the company profile and the filing history, the response is
    /// cached on disk (`companies-house-{number}-officers.json`) when a cache
    /// directory is configured; without one it is always fetched from the API.
    pub async fn get_officers(&self, company_number: &str) -> ApiResult<OfficerList> {
        if let Some(cache_dir) = self.config.cache_dir()
            && let Some(officers) = read_cached_officers(cache_dir, company_number)
        {
            return Ok(officers);
        }
        let officers: OfficerList = self
            .get_json(&format!("/company/{company_number}/officers"))
            .await?;
        if let Some(cache_dir) = self.config.cache_dir() {
            write_cached_officers(cache_dir, company_number, &officers);
        }
        Ok(officers)
    }

    /// The next accounting period to file, with its filing deadlines.
    ///
    /// The period's start and end come from Companies House's own expectation
    /// — the `accounts.next_accounts` block of the company profile (which
    /// reflects any shortened/lengthened periods and accounting-reference-date
    /// changes) — falling back on the *default* accounting periods computed
    /// from the company's registration date ([`Company::accounting_period_n`]
    /// for the period containing today).  The caller's own return-period
    /// fields (the `accounts.period` being produced) are not consulted: the
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
    pub async fn next_accounting_period(
        &self,
        company: &Company,
    ) -> ApiResult<NextAccountingPeriod> {
        let company_number = (!company.company_number.is_empty())
            .then_some(company.company_number.as_str())
            .or_else(|| self.config.company_number())
            .filter(|n| !n.is_empty())
            .ok_or(CompaniesHouseError::MissingCompanyNumber)?;
        let profile = self.get_company_profile_cached(company_number).await?;
        Ok(next_accounting_period_from(
            company,
            profile.accounts.as_ref(),
        ))
    }

    /// `GET {path}` on the configured base URL, authenticated with the API
    /// key as the basic-auth username, decoding the body as JSON.
    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> ApiResult<T> {
        let url = format!("{}{}", self.config.base_url(), path);
        self.get_url_json(&url).await
    }

    /// `GET` an absolute URL (the document API is a separate host from the
    /// main API), authenticated the same way, decoding the body as JSON.
    async fn get_url_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> ApiResult<T> {
        let response = self
            .http
            .get(url)
            .basic_auth(self.config.api_key().unwrap_or_default(), Some("")) //  Companies House API takes the username as the API key
            .send()
            .await
            .map_err(|source| CompaniesHouseError::RequestFailed { source })?;

        let status = response.status();
        if !status.is_success() {
            return Err(CompaniesHouseError::HttpStatus {
                url: url.to_string(),
                status,
            });
        }

        response
            .json::<T>()
            .await
            .map_err(|source| CompaniesHouseError::DecodeFailed { source })
    }

    /// `GET` an absolute URL, returning the raw response bytes (document
    /// downloads).
    async fn get_url_bytes(&self, url: &str) -> ApiResult<Vec<u8>> {
        let response = self
            .http
            .get(url)
            .basic_auth(self.config.api_key().unwrap_or_default(), Some("")) //  Companies House API takes the username as the API key
            .send()
            .await
            .map_err(|source| CompaniesHouseError::RequestFailed { source })?;

        let status = response.status();
        if !status.is_success() {
            return Err(CompaniesHouseError::HttpStatus {
                url: url.to_string(),
                status,
            });
        }

        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|source| CompaniesHouseError::RequestFailed { source })
    }

    /// Point this client's response cache at a specific directory, enabling
    /// the disk cache (overriding `CT600_CACHE_DIR`).
    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.config = self.config.with_cache_dir(cache_dir);
        self
    }

    /// The configured response-cache directory: the per-client override, else
    /// `CT600_CACHE_DIR`, else (in test builds) the repository's
    /// `.cache/api_responses`; `None` when caching is disabled.
    pub fn cache_dir(&self) -> Option<&Path> {
        self.config.cache_dir()
    }

    /// Fetch the company profile for the given company number, serving from
    /// the local response cache when available and falling back on the live
    /// API otherwise (the response is cached for next time).  Without a
    /// configured cache directory the profile is always fetched from the API.
    ///
    /// Cache reads are best-effort: a missing or corrupt cache entry falls
    /// back on the live API, and a failed cache write is non-fatal.
    pub async fn get_company_profile_cached(
        &self,
        company_number: &str,
    ) -> ApiResult<CompanyProfile> {
        if let Some(cache_dir) = self.config.cache_dir()
            && let Some(profile) = read_cached_profile(cache_dir, company_number)
        {
            return Ok(profile);
        }
        let profile = self.get_company_profile(company_number).await?;
        if let Some(cache_dir) = self.config.cache_dir() {
            write_cached_profile(cache_dir, company_number, &profile);
        }
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
        let Some(company_number) = self
            .config
            .enrichment_number(&input.name, &input.company_number)
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
    cache_dir.join(format!(
        "companies-house-{company_number}-filing-history.json"
    ))
}

/// The cache file for a company number's officers, e.g.
/// `companies-house-12345678-officers.json`.
fn officers_cache_path(cache_dir: &Path, company_number: &str) -> PathBuf {
    cache_dir.join(format!("companies-house-{company_number}-officers.json"))
}

/// Read a cached company profile, if present and decodable.
fn read_cached_profile(cache_dir: &Path, company_number: &str) -> Option<CompanyProfile> {
    let data = std::fs::read_to_string(cache_path(cache_dir, company_number)).ok()?;
    serde_json::from_str(&data).ok()
}

/// Write a company profile to the cache, best-effort (non-fatal on failure).
fn write_cached_profile(cache_dir: &Path, company_number: &str, profile: &CompanyProfile) {
    if let Ok(data) = serde_json::to_vec(profile) {
        write_cache_file(
            cache_dir,
            &format!("companies-house-{company_number}.json"),
            &data,
        );
    }
}

/// Read a cached filing history, if present and decodable.
fn read_cached_filing_history(cache_dir: &Path, company_number: &str) -> Option<FilingHistory> {
    let data =
        std::fs::read_to_string(filing_history_cache_path(cache_dir, company_number)).ok()?;
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

/// Read a cached officer list, if present and decodable.
fn read_cached_officers(cache_dir: &Path, company_number: &str) -> Option<OfficerList> {
    let data = std::fs::read_to_string(officers_cache_path(cache_dir, company_number)).ok()?;
    serde_json::from_str(&data).ok()
}

/// Write an officer list to the cache, best-effort (non-fatal on failure).
fn write_cached_officers(cache_dir: &Path, company_number: &str, officers: &OfficerList) {
    if let Ok(data) = serde_json::to_vec(officers) {
        write_cache_file(
            cache_dir,
            &format!("companies-house-{company_number}-officers.json"),
            &data,
        );
    }
}

/// The cache file name for a downloaded filing document:
/// `{company_number}-{date}-{filing_id}`, where `date` is the period end
/// (falling back to the filed-on date, else `nodate` for filings without
/// either) — the company + period make the cache queriable, and the
/// transaction id disambiguates filings sharing a date.
fn filing_download_file_name(
    company_number: &str,
    date: Option<NaiveDate>,
    filing_id: &str,
) -> String {
    let date = date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "nodate".to_string());
    format!("{company_number}-{date}-{filing_id}")
}

/// The cache file for a downloaded filing document, e.g.
/// `filings_downloads/14510633-2024-11-30-MzQzMzQ3MTgwNGFkaXF6a2N4`.
fn filing_download_cache_path(
    cache_dir: &Path,
    company_number: &str,
    date: Option<NaiveDate>,
    filing_id: &str,
) -> PathBuf {
    cache_dir
        .join("filings_downloads")
        .join(filing_download_file_name(company_number, date, filing_id))
}

/// Read a cached filing download, if present.
fn read_cached_filing_download(
    cache_dir: &Path,
    company_number: &str,
    date: Option<NaiveDate>,
    filing_id: &str,
) -> Option<Vec<u8>> {
    std::fs::read(filing_download_cache_path(
        cache_dir,
        company_number,
        date,
        filing_id,
    ))
    .ok()
}

/// Write a filing download to the cache, best-effort (non-fatal on failure).
fn write_cached_filing_download(
    cache_dir: &Path,
    company_number: &str,
    date: Option<NaiveDate>,
    filing_id: &str,
    bytes: &[u8],
) {
    write_cache_file(
        &cache_dir.join("filings_downloads"),
        &filing_download_file_name(company_number, date, filing_id),
        bytes,
    );
}

/// Write a cache file under the cache directory, best-effort (non-fatal on
/// failure).
fn write_cache_file(cache_dir: &Path, file_name: &str, data: &[u8]) {
    let result = std::fs::create_dir_all(cache_dir)
        .and_then(|_| std::fs::write(cache_dir.join(file_name), data));
    if let Err(e) = result {
        log::warn!("failed to write Companies House cache: {e}");
    }
}

/// The cache directory: `CT600_CACHE_DIR` when set; in test builds, the
/// repository's `.cache/api_responses` directory (resolved through
/// [`test_utils::REPO`], so test clients share a warm cache across runs);
/// `None` disables the disk cache (the client then always fetches from the
/// API).
fn cache_dir_from_env() -> Option<PathBuf> {
    if let Some(dir) = non_empty_env("CT600_CACHE_DIR") {
        return Some(PathBuf::from(dir));
    }
    #[cfg(test)]
    {
        Some(test_utils::cache_dir("api_responses"))
    }
    #[cfg(not(test))]
    {
        None
    }
}

/// A non-empty environment variable, if set.
fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The API endpoint configuration from the environment: the live key
/// (`COMPANIES_HOUSE_API_KEY`) preferred, the sandbox key
/// (`COMPANIES_HOUSE_SANDBOX_API_KEY`) as fallback, `None` when neither is set.
fn api_config_from_env() -> Option<ApiConfig> {
    if let Some(api_key) = non_empty_env("COMPANIES_HOUSE_API_KEY") {
        return Some(ApiConfig {
            base_url: API_BASE_URL,
            api_key,
        });
    }
    if let Some(api_key) = non_empty_env("COMPANIES_HOUSE_SANDBOX_API_KEY") {
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
    /// Key/value description values from the API (e.g. an accounts filing's
    /// `made_up_date` / `period_start_on` / `period_end_on`), used to read
    /// the period a filing covers.
    #[serde(default)]
    pub description_values: std::collections::BTreeMap<String, String>,
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

    /// The Companies House transaction id: the trailing path segment of
    /// `links.self` (stable across refetches), `None` when the link is
    /// absent.
    pub fn transaction_id(&self) -> Option<String> {
        self.links
            .as_ref()
            .and_then(|l| l.self_link.as_deref())
            .and_then(|url| url.rsplit('/').next().filter(|s| !s.is_empty()))
            .map(str::to_string)
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

    /// The filings parsed into typed dates ([`ChFiling`]): each filing's
    /// registration date and, when the API reports one, the period it covers
    /// (see [`ChFiling::from`]).
    pub fn parsed(&self) -> impl Iterator<Item = ChFiling> {
        self.items.iter().map(ChFiling::from)
    }

    /// The filings parsed into their kind-specific structs
    /// ([`TypedFiling`]): each filing classified as accounts / confirmation
    /// statement / incorporation / other with the fields its kind carries
    /// (see [`TypedFiling::try_from`]). Kinds that are not implemented yet
    /// (ARD changes, officer changes) yield
    /// [`CompaniesHouseError::Unimplemented`] rather than a best-effort
    /// parse.
    pub fn typed(&self) -> impl Iterator<Item = Result<TypedFiling, CompaniesHouseError>> {
        self.items.iter().map(TypedFiling::try_from)
    }
}

/// A past filing parsed into typed dates: when it was registered and the
/// period it covers (when the API reports one — e.g. the accounts filings'
/// `made_up_date` / `period_start_on` / `period_end_on` description values).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChFiling {
    /// When the filing was registered with Companies House.
    pub filed_on: Option<NaiveDate>,
    /// The start of the period the filing covers, when reported.
    pub period_start: Option<NaiveDate>,
    /// The end of the period the filing covers, when reported.
    pub period_end: Option<NaiveDate>,
    /// Filing category, e.g. `accounts`, `confirmation-statement`.
    pub category: Option<String>,
    /// Form type code, e.g. `AA` (accounts), `CS01` (confirmation statement).
    pub form_type: Option<String>,
    /// Human-readable description of the filing.
    pub description: Option<String>,
}

impl From<&FilingHistoryItem> for ChFiling {
    fn from(item: &FilingHistoryItem) -> Self {
        let parse = |key: &str| {
            item.description_values
                .get(key)
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        };
        Self {
            filed_on: item.filed_on(),
            period_start: parse("period_start_on"),
            period_end: parse("period_end_on")
                .or_else(|| parse("made_up_to"))
                .or_else(|| parse("made_up_date")),
            category: item.category.clone(),
            form_type: item.form_type.clone(),
            description: item.description.clone(),
        }
    }
}

// ============================================================================
// Typed filing parsing (per-kind structs)
// ============================================================================

/// A Companies House form type code, classified into the kinds the product
/// understands (accounts, confirmation statements, ARD changes, officer
/// changes, incorporation); any other code is preserved verbatim in
/// [`FormType::Other`].
///
/// This is the single source of truth for the code table — the tally-api
/// storage enum (`ChFormType`) delegates its `from_code` here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormType {
    /// Full accounts (`AA`).
    Accounts,
    /// Micro-entity accounts (`AA02`).
    MicroEntityAccounts,
    /// Dormant company accounts (`AAMD`).
    DormantAccounts,
    /// Confirmation statement (`CS01`).
    ConfirmationStatement,
    /// Change of accounting reference date (`AA01`).
    ChangeAccountingReferenceDate,
    /// Change of registered office address (`AD01`).
    ChangeRegisteredOffice,
    /// Officer appointment (`AP01`–`AP04`).
    OfficerAppointed,
    /// Officer termination (`TM01`, `TM02`).
    OfficerTerminated,
    /// Change of officer details (`CH01`–`CH04`).
    OfficerDetailsChanged,
    /// Incorporation (`NEWINC`).
    Incorporation,
    /// Any other form type code, preserved verbatim.
    Other { code: String },
}

impl FormType {
    /// Classify a Companies House form type code; unmodelled codes are
    /// preserved verbatim in [`FormType::Other`].
    pub fn from_code(code: &str) -> Self {
        match code {
            "AA" => Self::Accounts,
            "AA02" => Self::MicroEntityAccounts,
            "AAMD" => Self::DormantAccounts,
            "CS01" => Self::ConfirmationStatement,
            "AA01" => Self::ChangeAccountingReferenceDate,
            "AD01" => Self::ChangeRegisteredOffice,
            "AP01" | "AP02" | "AP03" | "AP04" => Self::OfficerAppointed,
            "TM01" | "TM02" => Self::OfficerTerminated,
            "CH01" | "CH02" | "CH03" | "CH04" => Self::OfficerDetailsChanged,
            "NEWINC" => Self::Incorporation,
            _ => Self::Other {
                code: code.to_string(),
            },
        }
    }

    /// The form type code for this kind. For the multi-code kinds
    /// (officer appointments, terminations, detail changes) this returns a
    /// representative code (`AP01`, `TM01`, `CH01`) — the exact code of a
    /// filing is available from the item's `form_type` field.
    pub fn as_code(&self) -> &str {
        match self {
            Self::Accounts => "AA",
            Self::MicroEntityAccounts => "AA02",
            Self::DormantAccounts => "AAMD",
            Self::ConfirmationStatement => "CS01",
            Self::ChangeAccountingReferenceDate => "AA01",
            Self::ChangeRegisteredOffice => "AD01",
            Self::OfficerAppointed => "AP01",
            Self::OfficerTerminated => "TM01",
            Self::OfficerDetailsChanged => "CH01",
            Self::Incorporation => "NEWINC",
            Self::Other { code } => code,
        }
    }
}

/// A filing-history item parsed into a kind-specific struct
/// ([`TypedFiling`]): the common fields every filing carries (when it was
/// filed, its transaction id, its description) plus the fields specific to
/// the filing's kind (e.g. an accounts filing's period).
///
/// Dispatch is code-first for the specific kinds (ARD changes, officer
/// changes, incorporation, registered-office address changes), then by
/// category for the broad families (accounts, confirmation statements) — so
/// an accounts filing with an unknown/new form type code still parses as
/// [`TypedFiling::Accounts`].
///
/// ARD changes and officer changes are **not implemented yet**: their
/// `description_values` keys are unverified (the default test company has no
/// such filings to check against), so they return
/// [`CompaniesHouseError::Unimplemented`] instead of a best-effort parse.
impl TryFrom<&FilingHistoryItem> for TypedFiling {
    type Error = CompaniesHouseError;

    fn try_from(item: &FilingHistoryItem) -> Result<Self, Self::Error> {
        let parse = |key: &str| {
            item.description_values
                .get(key)
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        };
        let common = (
            item.filed_on(),
            item.transaction_id(),
            item.description.clone(),
        );
        let (filed_on, transaction_id, description) = common;

        let unimplemented = |fallback: &str| CompaniesHouseError::Unimplemented {
            filing_type: item
                .form_type
                .clone()
                .unwrap_or_else(|| fallback.to_string()),
        };

        match FormType::from_code(item.form_type.as_deref().unwrap_or("")) {
            FormType::ChangeAccountingReferenceDate => Err(unimplemented(
                FormType::ChangeAccountingReferenceDate.as_code(),
            )),
            kind @ (FormType::OfficerAppointed
            | FormType::OfficerTerminated
            | FormType::OfficerDetailsChanged) => Err(unimplemented(kind.as_code())),
            FormType::Incorporation => Ok(TypedFiling::Incorporation(IncorporationFiling {
                filed_on,
                transaction_id,
                description,
            })),
            // Verified against real data (14510633's 2023-12-11 AD01): the
            // `description_values` carry `change_date`, `old_address` and
            // `new_address`.
            FormType::ChangeRegisteredOffice => Ok(TypedFiling::AddressChange(
                AddressChangeFiling {
                    filed_on,
                    transaction_id,
                    description,
                    change_date: parse("change_date"),
                    old_address: item.description_values.get("old_address").cloned(),
                    new_address: item.description_values.get("new_address").cloned(),
                },
            )),
            _ => match item.category.as_deref() {
                Some("accounts") => Ok(TypedFiling::Accounts(AccountsFiling {
                    filed_on,
                    transaction_id,
                    description,
                    made_up_to: parse("made_up_to").or_else(|| parse("made_up_date")),
                    period_start: parse("period_start_on"),
                    period_end: parse("period_end_on")
                        .or_else(|| parse("made_up_to"))
                        .or_else(|| parse("made_up_date")),
                })),
                Some("confirmation-statement") => {
                    Ok(TypedFiling::ConfirmationStatement(
                        ConfirmationStatementFiling {
                            filed_on,
                            transaction_id,
                            description,
                            // CH reports the statement date as `made_up_date`.
                            made_on: parse("made_up_date").or_else(|| parse("made_on")),
                        },
                    ))
                }
                _ => Ok(TypedFiling::Other(OtherFiling {
                    filed_on,
                    transaction_id,
                    description,
                    form_type: item.form_type.clone(),
                })),
            },
        }
    }
}

/// A filing parsed into the kind-specific struct it actually is. Match on
/// the variant to read the kind's fields (see the per-kind structs); the
/// common fields (when filed, transaction id, description) are on every
/// variant.
///
/// The parse ([`TypedFiling::try_from`]) is fallible: the ARD-change and
/// officer-change variants below are **reserved** — their field mappings are
/// unverified (no real filing of those kinds to check against), so parsing
/// returns [`CompaniesHouseError::Unimplemented`] for them until they are
/// implemented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypedFiling {
    /// An accounts filing (category `accounts`, e.g. `AA`, `AA02`, `AAMD`).
    Accounts(AccountsFiling),
    /// A confirmation statement (`CS01`).
    ConfirmationStatement(ConfirmationStatementFiling),
    /// A registered-office address change (`AD01`).
    AddressChange(AddressChangeFiling),
    /// A change of accounting reference date (`AA01`). Reserved: the parse
    /// returns [`CompaniesHouseError::Unimplemented`] until the
    /// `description_values` keys are verified against real data.
    ChangeOfAccountingReferenceDate(ArdChangeFiling),
    /// An officer appointment / termination / detail change (`AP01`–`AP04`,
    /// `TM01`/`TM02`, `CH01`–`CH04`). Reserved: the parse returns
    /// [`CompaniesHouseError::Unimplemented`] until the `description_values`
    /// keys are verified against real data.
    OfficerChange(OfficerChangeFiling),
    /// The company's incorporation (`NEWINC`).
    Incorporation(IncorporationFiling),
    /// Any other filing, with its raw form type code.
    Other(OtherFiling),
}

impl TypedFiling {
    /// When the filing was registered with Companies House (on every
    /// variant).
    pub fn filed_on(&self) -> Option<NaiveDate> {
        match self {
            Self::Accounts(f) => f.filed_on,
            Self::ConfirmationStatement(f) => f.filed_on,
            Self::AddressChange(f) => f.filed_on,
            Self::ChangeOfAccountingReferenceDate(f) => f.filed_on,
            Self::OfficerChange(f) => f.filed_on,
            Self::Incorporation(f) => f.filed_on,
            Self::Other(f) => f.filed_on,
        }
    }

    /// The Companies House transaction id (on every variant).
    pub fn transaction_id(&self) -> Option<&str> {
        match self {
            Self::Accounts(f) => f.transaction_id.as_deref(),
            Self::ConfirmationStatement(f) => f.transaction_id.as_deref(),
            Self::AddressChange(f) => f.transaction_id.as_deref(),
            Self::ChangeOfAccountingReferenceDate(f) => f.transaction_id.as_deref(),
            Self::OfficerChange(f) => f.transaction_id.as_deref(),
            Self::Incorporation(f) => f.transaction_id.as_deref(),
            Self::Other(f) => f.transaction_id.as_deref(),
        }
    }

    /// CH's human-readable description (on every variant).
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Accounts(f) => f.description.as_deref(),
            Self::ConfirmationStatement(f) => f.description.as_deref(),
            Self::AddressChange(f) => f.description.as_deref(),
            Self::ChangeOfAccountingReferenceDate(f) => f.description.as_deref(),
            Self::OfficerChange(f) => f.description.as_deref(),
            Self::Incorporation(f) => f.description.as_deref(),
            Self::Other(f) => f.description.as_deref(),
        }
    }
}

/// An accounts filing: the period it covers, as CH reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsFiling {
    /// When the filing was registered with Companies House.
    pub filed_on: Option<NaiveDate>,
    /// The Companies House transaction id (`links.self`'s trailing segment).
    pub transaction_id: Option<String>,
    /// CH's human-readable description.
    pub description: Option<String>,
    /// The period end as reported (`made_up_to` / `made_up_date`).
    pub made_up_to: Option<NaiveDate>,
    /// The start of the period the accounts cover, when reported.
    pub period_start: Option<NaiveDate>,
    /// The end of the period the accounts cover, when reported.
    pub period_end: Option<NaiveDate>,
}

/// A confirmation statement (`CS01`): when it was made up to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmationStatementFiling {
    pub filed_on: Option<NaiveDate>,
    pub transaction_id: Option<String>,
    pub description: Option<String>,
    /// The date the statement was made up to (CH reports it as `made_up_date`).
    pub made_on: Option<NaiveDate>,
}

/// A change of accounting reference date (`AA01`): the new ARD, when CH
/// reports one (the `description_values` key is best-effort — some AA01
/// filings carry no date at all).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArdChangeFiling {
    pub filed_on: Option<NaiveDate>,
    pub transaction_id: Option<String>,
    pub description: Option<String>,
    /// The new accounting reference date, when reported.
    pub new_ard_date: Option<NaiveDate>,
}

/// A change of registered office address (`AD01`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressChangeFiling {
    pub filed_on: Option<NaiveDate>,
    pub transaction_id: Option<String>,
    pub description: Option<String>,
    /// The date the change took effect (CH reports it as `change_date`).
    pub change_date: Option<NaiveDate>,
    /// The previous registered office address, when reported.
    pub old_address: Option<String>,
    /// The new registered office address, when reported.
    pub new_address: Option<String>,
}

/// An officer appointment / termination / detail change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficerChangeFiling {
    pub filed_on: Option<NaiveDate>,
    pub transaction_id: Option<String>,
    pub description: Option<String>,
    /// The officer's name, as registered (e.g. `BLOGGS, A`).
    pub officer_name: Option<String>,
    /// What happened: appointed, terminated, or details changed.
    pub action: OfficerChangeAction,
    /// The date of the change (CH reports it as `appointment_date`,
    /// `termination_date`, `change_date`, or `action_date`).
    pub action_date: Option<NaiveDate>,
}

/// What an [`OfficerChangeFiling`] records about the officer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfficerChangeAction {
    /// The officer was appointed (`AP01`–`AP04`).
    Appointed,
    /// The officer was terminated (`TM01`, `TM02`).
    Terminated,
    /// The officer's details were changed (`CH01`–`CH04`).
    DetailsChanged,
}

impl OfficerChangeAction {
    /// A short label for display, e.g. `appointed`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Appointed => "appointed",
            Self::Terminated => "terminated",
            Self::DetailsChanged => "details changed",
        }
    }
}

/// The company's incorporation (`NEWINC`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncorporationFiling {
    pub filed_on: Option<NaiveDate>,
    pub transaction_id: Option<String>,
    pub description: Option<String>,
}

/// Any filing that is not one of the modelled kinds, with its raw form type
/// code preserved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtherFiling {
    pub filed_on: Option<NaiveDate>,
    pub transaction_id: Option<String>,
    pub description: Option<String>,
    /// The raw form type code (e.g. `AD01`).
    pub form_type: Option<String>,
}

// ============================================================================
// Filed document parsing (two passes: generic iXBRL -> IR -> filing-specific)
// ============================================================================

/// A balance sheet parsed from a filed accounts document (an `AA`-family
/// filing). The figures are the filed period's line items in the
/// [`PreviousYearFigures`] shape (whole pounds; creditor lines negative),
/// and the period is recovered from the document itself.
#[derive(Debug, Clone, PartialEq)]
pub struct FiledBalanceSheet {
    /// ISO-8601 — the start of the period the accounts cover.
    pub period_start: NaiveDate,
    /// ISO-8601 — the end of the period the accounts cover.
    pub period_end: NaiveDate,
    /// The balance-sheet line items for the filed period.
    pub figures: PreviousYearFigures,
}

/// Parse a filed accounts document (the iXBRL HTML/XML CH serves) into a
/// [`FiledBalanceSheet`].
///
/// Two passes, as for every filed-document type: first the **generic** iXBRL
/// pass ([`XmlNode::from_xml_string`] + [`ParsedIxBrlFacts::from_node`], the
/// shared fact IR), then this **accounts-filing-specific** pass over the IR.
/// It resolves the FRC UK-taxonomy balance-sheet facts in the current-period
/// context (the instant context at the period end; the two creditor lines
/// sit in the `WithinOneYear` / `AfterOneYear` dimension contexts) and
/// applies the reports' sign convention (creditor lines negative).  Both CH
/// renderings are tolerated: the FRC 2023 taxonomy (`uk-core:`, ISO period
/// dates, no comparatives) and the FRC 2024 one (`core:`, `DD.MM.YY` period
/// dates, comparatives present) — the figures reported are always the filed
/// (current) period.
///
/// Fails when the document is not a parseable accounts iXBRL file (PDFs,
/// zips, other taxonomies).
pub fn parse_filed_accounts(html: &str) -> Result<FiledBalanceSheet, String> {
    let node = XmlNode::from_xml_string(html)?;
    let facts = ParsedIxBrlFacts::from_node(&node);
    let dims = xbrl_context_dimensions(&node);
    let periods = context_periods(&node);

    let period_date = |key: &str| -> Result<NaiveDate, String> {
        let raw = facts
            .non_numeric
            .get(key)
            .ok_or_else(|| format!("the document carries no {key} fact"))?;
        parse_filed_date(raw).ok_or_else(|| format!("the document's {key} is not a parseable date"))
    };
    let period_start = period_date("uk-bus:StartDateForPeriodCoveredByReport")?;
    let period_end = period_date("uk-bus:EndDateForPeriodCoveredByReport")?;

    // The current-period instant contexts: those whose instant is the period
    // end (comparatives, when present, sit at the earlier instant and are
    // excluded).
    let mut at_end: Vec<&str> = periods
        .iter()
        .filter(|(_, p)| p.instant == Some(period_end))
        .map(|(id, _)| id.as_str())
        .collect();
    at_end.sort_unstable();

    // The default current context holds most lines: the no-dimension instant
    // context at the period end carrying the most balance-sheet facts (CH
    // renders exactly one — `icur1` / `CY_END` — the heuristic keeps the
    // choice deterministic).
    let figure_stems = [
        "FixedAssets",
        "CurrentAssets",
        "NetCurrentAssetsLiabilities",
        "TotalAssetsLessCurrentLiabilities",
        "NetAssetsLiabilities",
        "Equity",
    ];
    let default = at_end
        .iter()
        .copied()
        .filter(|ctx| dims.get(*ctx).map(|d| d.is_empty()).unwrap_or(true))
        .max_by_key(|ctx| {
            figure_stems
                .iter()
                .filter(|stem| has_fact(&facts, ctx, stem))
                .count()
        });

    // Fact lookups try both taxonomies' prefixes (FRC 2024 `core:`, FRC
    // 2023 `uk-core:`); dimension members likewise (`core:WithinOneYear` vs
    // `uk-core:WithinOneYear`).
    let num = |ctx: Option<&str>, stem: &str| -> f64 {
        ctx.and_then(|c| {
            ["core:", "uk-core:"].iter().find_map(|prefix| {
                facts
                    .numeric_by_ctx
                    .get(&(format!("{prefix}{stem}"), c.to_string()))
                    .copied()
            })
        })
        .unwrap_or(0.0)
    };
    let num_dim = |stem: &str, member: &str| -> f64 {
        let ctx = at_end.iter().copied().find(|ctx| {
            dims.get(*ctx).is_some_and(|d| {
                d.values()
                    .any(|v| v == member || v == &format!("uk-{member}"))
            })
        });
        num(ctx, stem)
    };
    let current = |stem: &str| num(default, stem);

    Ok(FiledBalanceSheet {
        period_start,
        period_end,
        figures: PreviousYearFigures {
            fixed_assets: current("FixedAssets"),
            called_up_share_capital_not_paid: current(
                "CalledUpShareCapitalNotPaidNotExpressedAsCurrentAsset",
            ),
            current_assets: current("CurrentAssets"),
            prepayments_and_accrued_income: current(
                "PrepaymentsAccruedIncomeNotExpressedWithinCurrentAssetSubtotal",
            ),
            // Creditor lines are stored negative (the reports' convention).
            creditors_within_1_year: -num_dim("Creditors", "core:WithinOneYear"),
            net_current_assets: current("NetCurrentAssetsLiabilities"),
            total_assets_less_liabilities: current("TotalAssetsLessCurrentLiabilities"),
            creditors_after_1_year: -num_dim("Creditors", "core:AfterOneYear"),
            provisions_for_liabilities: current("ProvisionsForLiabilitiesBalanceSheetSubtotal"),
            accruals_and_deferred_income: current(
                "AccruedLiabilitiesNotExpressedWithinCreditorsSubtotal",
            ),
            net_assets: current("NetAssetsLiabilities"),
            capital_and_reserves: current("Equity"),
        },
    })
}

/// Whether the IR holds a numeric fact for `stem` (either taxonomy prefix)
/// in the given context.
fn has_fact(facts: &ParsedIxBrlFacts, ctx: &str, stem: &str) -> bool {
    ["core:", "uk-core:"].iter().any(|prefix| {
        facts
            .numeric_by_ctx
            .contains_key(&(format!("{prefix}{stem}"), ctx.to_string()))
    })
}

/// Parse a date fact: CH renders the period dates ISO (`2023-11-30`) in the
/// 2023 taxonomy and `1.12.23` (`ixt2:datedaymonthyear`) in the 2024 one.
fn parse_filed_date(value: &str) -> Option<NaiveDate> {
    ["%Y-%m-%d", "%d.%m.%y", "%d.%m.%Y"]
        .iter()
        .find_map(|fmt| NaiveDate::parse_from_str(value.trim(), fmt).ok())
}

/// A context's period: an instant (balance-sheet dates) or a duration
/// (start → end), as the FRC taxonomy renders them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct ContextPeriod {
    instant: Option<NaiveDate>,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
}

/// Collect every `xbrli:context`'s period dates: id → instant | start..end.
fn context_periods(node: &XmlNode) -> std::collections::HashMap<String, ContextPeriod> {
    let mut out = std::collections::HashMap::new();
    collect_context_periods(node, &mut out);
    out
}

fn collect_context_periods(
    node: &XmlNode,
    out: &mut std::collections::HashMap<String, ContextPeriod>,
) {
    if let XmlNode::Elem {
        name,
        attributes,
        children,
    } = node
    {
        if name == "xbrli:context"
            && let Some(id) = attributes
                .iter()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.clone())
        {
            let mut period = ContextPeriod::default();
            collect_period_dates(children, &mut period);
            out.insert(id, period);
        }
        for child in children {
            collect_context_periods(child, out);
        }
    }
}

/// Read the `xbrli:instant` / `xbrli:startDate` / `xbrli:endDate` dates from
/// a context's subtree (the period element nests below the context).
fn collect_period_dates(nodes: &[XmlNode], period: &mut ContextPeriod) {
    for node in nodes {
        if let XmlNode::Elem { name, children, .. } = node {
            if matches!(
                name.as_str(),
                "xbrli:instant" | "xbrli:startDate" | "xbrli:endDate"
            ) {
                let value = children.iter().map(text_of).collect::<String>();
                let date = NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok();
                match name.as_str() {
                    "xbrli:instant" => period.instant = date,
                    "xbrli:startDate" => period.start = date,
                    "xbrli:endDate" => period.end = date,
                    _ => {}
                }
            }
            collect_period_dates(children, period);
        }
    }
}

/// The concatenated text of a node tree (element text or a text node).
fn text_of(node: &XmlNode) -> String {
    match node {
        XmlNode::Text(t) => t.clone(),
        XmlNode::Elem { children, .. } => children.iter().map(text_of).collect(),
    }
}

/// The metadata of a filed document (`GET` on the document API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    #[serde(default)]
    pub links: DocumentLinks,
}

/// Resource links carried by a document's metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentLinks {
    /// Link to the document's content — relative to the document API host
    /// (e.g. `/document/{id}/content`).
    #[serde(rename = "document", default)]
    pub document: Option<String>,
}

/// Resolve a possibly-relative link against an absolute base URL (the
/// metadata URL the link came from), so live and sandbox document API hosts
/// both work. Absolute links pass through unchanged.
fn resolve_absolute(base: &str, path_or_url: &str) -> String {
    if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
        return path_or_url.to_string();
    }
    let origin = base.split('/').take(3).collect::<Vec<_>>().join("/");
    format!("{origin}{path_or_url}")
}

/// A single company officer (`GET /company/{number}/officers`).
///
/// Only the commonly used fields are modelled; all optional fields are
/// tolerated when absent from the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Officer {
    /// The officer's name, as registered (e.g. `"BLOGGS, A"`).
    #[serde(default)]
    pub name: Option<String>,
    /// The officer's role, e.g. `director`, `corporate-director`, `secretary`.
    #[serde(default)]
    pub officer_role: Option<String>,
    /// The date the officer was appointed (`YYYY-MM-DD`).
    #[serde(default)]
    pub appointed_on: Option<String>,
    /// The date the officer resigned (`YYYY-MM-DD`); absent while serving.
    #[serde(default)]
    pub resigned_on: Option<String>,
}

/// The officers of a company (`GET /company/{number}/officers`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfficerList {
    /// The (first page of) officers, in the API's order.
    #[serde(default)]
    pub items: Vec<Officer>,
}

impl OfficerList {
    /// The names of the current directors — the officers whose role is a
    /// director role and who have not resigned — in the API's order.
    pub fn directors(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|officer| {
                officer.resigned_on.is_none()
                    && officer
                        .officer_role
                        .as_deref()
                        .is_some_and(|role| role.ends_with("director"))
            })
            .filter_map(|officer| officer.name.clone())
            .collect()
    }
}

/// The next accounting period to file, with its filing deadlines.
///
/// See [`CompaniesHouseClient::next_accounting_period`] for how the dates are
/// resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct NextAccountingPeriod {
    /// The next accounting period to file.
    pub period: AccountingPeriod,
    /// Deadline to file the CT600 corporation-tax return with HMRC (12 months
    /// after the period end).
    pub deadline_to_file_hmrc_ct600: NaiveDate,
    /// Deadline to file the accounts with Companies House (the profile's
    /// next-accounts due date, else 9 months after the period end).
    pub deadline_to_file_companies_house_accounts: NaiveDate,
}

/// The next accounting period to file, from an already-fetched profile: the
/// profile's `next_accounts` expectation when present, else the
/// registration-date schedule for today.
///
/// Pure (no fetch), so callers holding the profile — e.g. the `tally` CLI —
/// use it to avoid a second lookup; [`CompaniesHouseClient::next_accounting_period`]
/// is the fetch-first wrapper.
pub fn next_accounting_period_from(
    company: &Company,
    accounts: Option<&Accounts>,
) -> NextAccountingPeriod {
    let today = chrono::Utc::now().date_naive();
    let default = company.accounting_period_containing(today);

    let next = accounts.and_then(|accounts| accounts.next_accounts.as_ref());
    let parse =
        |date: Option<&String>| date.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

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
        period: AccountingPeriod { start, end },
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

// ============================================================================
// Test fixtures and offline test client
// ============================================================================

/// Test fixtures and offline test clients for the company clients.
///
/// [`TestClient`] serves company profiles from a local cache when available,
/// falling back on hardcoded default fixtures ([`TestData`]), and finally on
/// the live Companies House API when a live client is configured.  It is
/// auto-built from the environment with sensible defaults, so tests run with
/// zero configuration on a fresh checkout.
///
/// Public so downstream crates' test suites can share the fixtures (the
/// `#[cfg(test)]`-gated helpers it uses, like `offline()`/`with_unreachable_api()`,
/// only exist when this crate itself is built for tests).
pub mod test_utils {
    use std::path::{Path, PathBuf};

    use snafu::Snafu;

    use core_model::AccountsMeta;

    use super::{ApiResult, CompaniesHouseClient, CompaniesHouseError, CompanyProfile};

    /// The repository root and `.cache` helpers (from the shared
    /// `test_utils` crate).
    pub use test_utils::{REPO, cache_dir, cache_root};

    fn default_cache_dir() -> PathBuf {
        cache_dir("api_responses")
    }

    /// A client that serves company profiles from a local JSON cache, falling
    /// back on the live Companies House API when the cache misses.
    ///
    /// Cache writes are best-effort: a failure to read or write the cache is
    /// non-fatal and the live API result is used instead.
    #[derive(Debug, Clone)]
    pub struct CachedCompaniesHouseClient {
        inner: CompaniesHouseClient,
        cache_dir: PathBuf,
    }

    impl CachedCompaniesHouseClient {
        /// Create a cached client with the default cache directory (the
        /// repo's `.cache`, resolved from the repository root).
        pub fn new(inner: CompaniesHouseClient) -> Self {
            Self::with_cache_dir(inner, cache_root())
        }

        /// Create a cached client with the given cache directory.
        pub fn with_cache_dir(inner: CompaniesHouseClient, cache_dir: impl Into<PathBuf>) -> Self {
            Self {
                inner,
                cache_dir: cache_dir.into(),
            }
        }

        /// The cache file for a company number.
        fn cache_path(&self, company_number: &str) -> PathBuf {
            self.cache_dir
                .join(format!("companies-house-{company_number}.json"))
        }

        /// Fetch a company profile, loading from the local cache when available
        /// and falling back on the live API otherwise (caching the result).
        pub async fn get_company_profile(&self, company_number: &str) -> ApiResult<CompanyProfile> {
            if let Some(profile) = self.read_cache(company_number) {
                return Ok(profile);
            }

            let profile = self.inner.get_company_profile(company_number).await?;
            self.write_cache(company_number, &profile);
            Ok(profile)
        }

        fn read_cache(&self, company_number: &str) -> Option<CompanyProfile> {
            let data = std::fs::read_to_string(self.cache_path(company_number)).ok()?;
            serde_json::from_str(&data).ok()
        }

        fn write_cache(&self, company_number: &str, profile: &CompanyProfile) {
            let result = serde_json::to_vec(profile)
                .map_err(|e| std::io::Error::other(e.to_string()))
                .and_then(|data| {
                    std::fs::create_dir_all(&self.cache_dir)?;
                    std::fs::write(self.cache_path(company_number), data)
                });
            if let Err(e) = result {
                log::warn!("failed to write Companies House cache: {e}");
            }
        }
    }

    /// The profile file for a company number, e.g. `companies-house-12345678.json`
    /// (the same naming as the [`CachedCompaniesHouseClient`] cache).
    fn profile_path(dir: &Path, company_number: &str) -> PathBuf {
        dir.join(format!("companies-house-{company_number}.json"))
    }

    /// Read a cached profile for a company number, if it exists and decodes.
    fn read_profile(dir: &Path, company_number: &str) -> Option<CompanyProfile> {
        let data = std::fs::read_to_string(profile_path(dir, company_number)).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Errors returned by [`TestClient`].
    #[derive(Debug, Snafu)]
    pub enum TestClientErr {
        /// No data available for the company: no cache entry, no default
        /// fixture, and no live API client configured.
        #[snafu(display(
            "no data for company {company_number}: no cache entry, no fixture, and no live API client configured"
        ))]
        NoData { company_number: String },

        /// The live API request failed.
        #[snafu(transparent)]
        CompaniesHouse { source: CompaniesHouseError },
    }

    /// A Companies House test client.
    ///
    /// Serves from the local cache when available, then the hardcoded default
    /// fixtures, then the live API when a live client is configured.  Without a
    /// live client it can only serve cached or fixture data and returns
    /// [`TestClientErr::NoData`] on a miss.
    pub struct TestClient {
        /// Local cache directory, consulted first.
        pub cache_dir: PathBuf,
        /// Optional live API client; absent means offline-only.
        pub client: Option<CompaniesHouseClient>,
    }

    impl Default for TestClient {
        /// Build a client automatically from the environment with sensible
        /// defaults:
        ///
        /// - a local cache directory (default `{repo}/.cache/api_responses`)
        /// - the optional live client is built from `COMPANIES_HOUSE_API_KEY`
        ///   (live API) or `COMPANIES_HOUSE_SANDBOX_API_KEY` (sandbox), or `None`
        ///   when neither is set.
        ///
        /// The cache is consulted before the hardcoded fixtures, so a leftover
        /// local cache entry shadows the default fixture for the same company.
        fn default() -> Self {
            let client = CompaniesHouseClient::live_from_env()
                .or_else(|_| CompaniesHouseClient::test_client_from_env())
                .ok();
            Self {
                cache_dir: default_cache_dir(),
                client,
            }
        }
    }

    impl TestClient {
        /// Fetch a company profile, serving from the cache, then the hardcoded
        /// default fixtures, then the live API (caching the result) when
        /// configured.
        pub async fn get_company_profile(
            &self,
            company_number: &str,
        ) -> Result<CompanyProfile, TestClientErr> {
            if let Some(profile) = read_profile(&self.cache_dir, company_number) {
                return Ok(profile);
            }
            if let Some(profile) = TestData::company(company_number) {
                return Ok(profile);
            }
            if let Some(live) = &self.client {
                let profile = live.get_company_profile(company_number).await?;
                self.write_cache(company_number, &profile);
                return Ok(profile);
            }
            Err(TestClientErr::NoData {
                company_number: company_number.to_string(),
            })
        }

        fn write_cache(&self, company_number: &str, profile: &CompanyProfile) {
            let result = serde_json::to_vec(profile)
                .map_err(|e| std::io::Error::other(e.to_string()))
                .and_then(|data| {
                    std::fs::create_dir_all(&self.cache_dir)?;
                    std::fs::write(profile_path(&self.cache_dir, company_number), data)
                });
            if let Err(e) = result {
                log::warn!("failed to write Companies House test cache: {e}");
            }
        }
    }

    /// Hardcoded test data: fictional company profiles and a sample tax
    /// computation served when the local cache misses, so tests run with
    /// zero configuration on a fresh checkout.
    pub struct TestData;

    impl TestData {
        /// The company number of the fictional default test company (the
        /// shared `test_utils::Fixtures` fixture).
        pub fn default_company_number() -> &'static str {
            test_utils::Fixtures::default_company_number()
        }

        /// Build a fictional company profile with the common fixture fields
        /// filled in (active, `ltd`, England/Wales, no optional data), varying
        /// only the identifying fields.
        fn fixture_profile(
            company_number: &str,
            company_name: &str,
            date_of_creation: &str,
        ) -> CompanyProfile {
            CompanyProfile {
                company_number: company_number.to_string(),
                company_name: company_name.to_string(),
                company_status: Some("active".to_string()),
                company_status_detail: None,
                date_of_creation: Some(date_of_creation.to_string()),
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

        /// The company profile of the fictional default test company (company
        /// number [`Self::default_company_number`]).
        pub fn default_company() -> CompanyProfile {
            Self::fixture_profile(
                Self::default_company_number(),
                "EXAMPLE CORP LTD",
                "2001-01-01",
            )
        }

        /// The company number of the sample company (the shared
        /// `test_utils::Fixtures` fixture).
        pub fn sample_company_number() -> &'static str {
            test_utils::Fixtures::sample_company_number()
        }

        /// The company profile of the sample company (company number
        /// [`Self::sample_company_number`]).
        pub fn sample_company() -> CompanyProfile {
            Self::fixture_profile(Self::sample_company_number(), "Acme Ltd", "2020-01-01")
        }

        /// The sample company's set of accounts: the 2026 return period and
        /// the default financial-year parameters (the shared
        /// `test_utils::Fixtures` fixture) — the `accounts` the sample tax
        /// computation is built on.
        pub fn sample_accounts_meta() -> AccountsMeta {
            test_utils::Fixtures::sample_accounts_meta()
        }

        /// The hardcoded fixtures: the fictional company profiles for the known
        /// company numbers, or `None` for unknown companies.
        pub fn company(company_number: &str) -> Option<CompanyProfile> {
            if company_number == Self::default_company_number() {
                Some(Self::default_company())
            } else if company_number == Self::sample_company_number() {
                Some(Self::sample_company())
            } else {
                None
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Endpoint `GET /company/{companyNumber}` (company profile), served
        /// from the hardcoded default fixture via `TestClient`.  Runs with
        /// zero configuration on a fresh checkout: no API key, no network, no
        /// cached responses.
        #[tokio::test]
        async fn get_company_profile_12345678_serves_default_fixtures() {
            let client = TestClient::default();

            let profile = client
                .get_company_profile(TestData::default_company_number())
                .await
                .expect("fetching the company profile should succeed");

            assert_eq!(profile.company_number, "12345678");
            assert_eq!(profile.company_name, "EXAMPLE CORP LTD");
            assert_eq!(profile.company_status.as_deref(), Some("active"));
            assert_eq!(profile.company_type.as_deref(), Some("ltd"));
            assert_eq!(profile.date_of_creation.as_deref(), Some("2001-01-01"));
        }

        /// The sample company's profile is served from the hardcoded fixture via
        /// `TestClient`.
        #[tokio::test]
        async fn get_company_profile_sample_serves_default_fixtures() {
            let client = TestClient::default();

            let profile = client
                .get_company_profile(TestData::sample_company_number())
                .await
                .expect("fetching the sample company profile should succeed");

            let company = TestData::sample_company();
            assert_eq!(profile.company_name, company.company_name);
            assert_eq!(profile.company_number, company.company_number);
            assert_eq!(profile.company_type, company.company_type);
        }

        /// The cache is consulted before the hardcoded default fixture: with a
        /// populated cache the fixture is never read.
        #[tokio::test]
        async fn test_client_serves_from_cache_before_default_fixture() {
            let cache_dir = tempfile::tempdir().unwrap();
            let client = TestClient {
                cache_dir: cache_dir.path().to_path_buf(),
                client: None,
            };

            let profile = TestData::fixture_profile(
                TestData::default_company_number(),
                "CACHED PRODUCTS LTD",
                "2001-01-01",
            );
            let cache_file = cache_dir.path().join(format!(
                "companies-house-{}.json",
                TestData::default_company_number()
            ));
            std::fs::write(
                &cache_file,
                serde_json::to_vec(&profile).expect("serialising the profile"),
            )
            .expect("writing the cache file");

            let cached = client
                .get_company_profile(TestData::default_company_number())
                .await
                .expect("serving from cache should succeed");

            assert_eq!(cached.company_name, "CACHED PRODUCTS LTD");
            assert!(cache_file.exists());
        }

        /// Without a live client, a cache+fixture miss returns
        /// [`TestClientErr::NoData`].
        #[tokio::test]
        async fn test_client_no_data_without_live_client() {
            let dir = tempfile::tempdir().unwrap();
            let client = TestClient {
                cache_dir: dir.path().to_path_buf(),
                client: None,
            };

            let err = client
                .get_company_profile("0")
                .await
                .expect_err("a cache+fixture miss without a live client must error");

            assert!(matches!(err, TestClientErr::NoData { .. }));
        }

        /// The cache is consulted first: with a populated cache the live API is
        /// never reached (the inner client points at an unreachable address).
        #[tokio::test]
        async fn test_cached_client_serves_from_cache_without_network() {
            let dir = tempfile::tempdir().unwrap();
            let client = CachedCompaniesHouseClient::with_cache_dir(
                CompaniesHouseClient::offline(),
                dir.path(),
            );

            let profile = TestData::fixture_profile(
                TestData::default_company_number(),
                "EXAMPLE PRODUCTS LTD",
                "2001-01-01",
            );
            let cache_file = client.cache_path(TestData::default_company_number());
            std::fs::write(
                &cache_file,
                serde_json::to_vec(&profile).expect("serialising the profile"),
            )
            .expect("writing the cache file");

            let cached = client
                .get_company_profile(TestData::default_company_number())
                .await
                .expect("serving from cache should succeed");

            assert_eq!(cached.company_name, "EXAMPLE PRODUCTS LTD");
            assert_eq!(cached.company_type.as_deref(), Some("ltd"));
            assert!(cache_file.exists());
        }
    }
}

// ============================================================================
// Offline unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Build a company with the historical `Company::new` five-argument
    /// semantics: the registration date is anchored on the return-period
    /// start (the default the constructor used to apply), keeping the
    /// registration-date accounting schedule deterministic in the tests.
    fn company_with_period(
        name: &str,
        tax_reference: &str,
        company_number: &str,
        period_start: NaiveDate,
        _period_end: NaiveDate,
    ) -> Company {
        let mut company = Company::new(name, tax_reference, company_number);
        company.registration_date = period_start;
        company
    }

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
        // (`Config::default()` would resolve an ambient `COMPANY_NUMBER`, so
        // the override pins it to empty — the parallel env-mutating tests
        // never race this one.)
        let config = Config::default().with_company_number("");
        assert_eq!(config.enrichment_number("Acme Ltd", "12345678"), None);
        assert_eq!(config.enrichment_number("", "12345678"), None);
        assert_eq!(config.enrichment_number("", ""), None);

        // Complete inputs: no resolution, even with a number configured.
        let config = Config::default().with_company_number("12345678");
        assert_eq!(config.enrichment_number("Acme Ltd", "12345678"), None);

        // Absent inputs + configured number: resolve with it.
        assert_eq!(
            config.enrichment_number("", ""),
            Some("12345678".to_string())
        );
        assert_eq!(
            config.enrichment_number("", "12345678"),
            Some("12345678".to_string())
        );

        // An empty configured number is treated as unset.
        assert_eq!(
            Config::default()
                .with_company_number("")
                .enrichment_number("", ""),
            None
        );
    }

    /// In test builds the default cache dir is the repository's
    /// `.cache/api_responses`, resolved through `test_utils::REPO` (an
    /// ambient `CT600_CACHE_DIR` would override it — pinned here; no other
    /// test mutates that var, so this cannot race).
    #[test]
    fn default_cache_dir_is_repo_cache_in_tests() {
        unsafe { std::env::remove_var("CT600_CACHE_DIR") };
        let config = Config::default();
        assert_eq!(
            config.cache_dir(),
            Some(test_utils::cache_dir("api_responses").as_path())
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

        let full = company_with_period(
            "Acme Ltd",
            "1234567890",
            "9876543",
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        let resolved = client.resolve_company(&full).await.expect("full override");
        assert_eq!(resolved.name, "Acme Ltd");
        assert_eq!(resolved.company_number, "9876543");
        assert_eq!(
            resolved.registration_date,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
        );
    }

    /// Otherwise the cached response for the configured company number is
    /// used (no network), keeping the caller's tax reference and period.
    #[tokio::test]
    async fn resolve_company_fills_from_cache_by_configured_number() {
        let cache_dir = tempfile::tempdir().unwrap();
        seed_cache(
            cache_dir.path(),
            &fixture_profile("12345678", "CACHED CORP LTD"),
        );
        let config = Config::default()
            .with_company_number("12345678")
            .with_cache_dir(cache_dir.path());
        let client = CompaniesHouseClient::new(config.with_unreachable_api());

        let partial = company_with_period(
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
        assert_eq!(
            resolved.registration_date,
            NaiveDate::from_ymd_opt(2001, 1, 1).unwrap()
        );
    }

    /// Without a full override or a configured company number the resolution
    /// fails.
    #[tokio::test]
    async fn resolve_company_errors_without_number_or_override() {
        let cache_dir = tempfile::tempdir().unwrap();
        // The empty-number override pins the config against an ambient
        // `COMPANY_NUMBER` set by the parallel env-mutating tests.
        let config = Config::default()
            .with_company_number("")
            .with_cache_dir(cache_dir.path());
        let client = CompaniesHouseClient::new(config.with_unreachable_api());

        let partial = company_with_period(
            "",
            "",
            "",
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        let err = client
            .resolve_company(&partial)
            .await
            .expect_err("cannot resolve");
        assert!(matches!(err, CompaniesHouseError::MissingCompanyNumber));
    }

    /// The `COMPANY_NUMBER`-gated paths, run together in one test so the
    /// environment variable is never contended with a parallel test.  All
    /// scenarios use an offline client, so any accidental network access (or
    /// a cache miss) would fail the test.
    #[tokio::test]
    async fn company_number_env_drives_enrichment() {
        let cache_dir = tempfile::tempdir().unwrap();
        seed_cache(
            cache_dir.path(),
            &fixture_profile("12345678", "EXAMPLE CORP LTD"),
        );

        // 1. Config::from_env resolves COMPANY_NUMBER, and a client built
        //    from the resolved config enriches absent details from the cache.
        unsafe { std::env::set_var("COMPANY_NUMBER", "12345678") };
        let config = Config::from_env();
        assert_eq!(config.company_number(), Some("12345678"));
        assert_eq!(
            config.enrichment_number("", ""),
            Some("12345678".to_string())
        );
        assert_eq!(config.enrichment_number("Acme Ltd", "12345678"), None);

        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());
        let company = company_with_period(
            "",
            "",
            "",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let enriched = client
            .enrich_company(&company)
            .await
            .expect("enrich from cache");
        assert_eq!(enriched.name, "EXAMPLE CORP LTD");
        assert_eq!(enriched.company_number, "12345678");
        assert_eq!(
            enriched.registration_date,
            NaiveDate::from_ymd_opt(2001, 1, 1).unwrap()
        );
        unsafe { std::env::remove_var("COMPANY_NUMBER") };

        // 2. Without COMPANY_NUMBER, a client resolved afterwards leaves the
        //    same absent inputs alone (no cache lookup, no network).
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());
        let company = company_with_period(
            "",
            "",
            "",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let untouched = client
            .enrich_company(&company)
            .await
            .expect("no env, no fetch");
        assert_eq!(untouched.name, "");
        assert_eq!(untouched.company_number, "");

        // 3. Complete details are never enriched, even with the env var set
        //    (the cache holds a different profile and the client is offline).
        unsafe { std::env::set_var("COMPANY_NUMBER", "12345678") };
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());
        let company = company_with_period(
            "Acme Ltd",
            "1234567890",
            "12345678",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        let enriched = client
            .enrich_company(&company)
            .await
            .expect("no fetch needed");
        assert_eq!(enriched.name, "Acme Ltd");
        assert_eq!(
            enriched.registration_date,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()
        );
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

    fn seed_officers(cache_dir: &Path, company_number: &str, officers: &OfficerList) {
        std::fs::create_dir_all(cache_dir).unwrap();
        std::fs::write(
            officers_cache_path(cache_dir, company_number),
            serde_json::to_vec(officers).unwrap(),
        )
        .unwrap();
    }

    /// The cached officer list is served without any network access, and
    /// [`OfficerList::directors`] surfaces the current directors' names —
    /// excluding resigned officers and non-director roles.
    #[tokio::test]
    async fn get_officers_serves_from_cache_and_filters_directors() {
        let cache_dir = tempfile::tempdir().unwrap();
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());

        let officers = OfficerList {
            items: vec![
                Officer {
                    name: Some("A Bloggs".to_string()),
                    officer_role: Some("director".to_string()),
                    appointed_on: Some("2001-01-01".to_string()),
                    resigned_on: None,
                },
                // A resigned director is not a current director.
                Officer {
                    name: Some("B Smith".to_string()),
                    officer_role: Some("director".to_string()),
                    appointed_on: Some("2001-01-01".to_string()),
                    resigned_on: Some("2020-01-01".to_string()),
                },
                // A corporate director counts as a director role.
                Officer {
                    name: Some("C Jones Ltd".to_string()),
                    officer_role: Some("corporate-director".to_string()),
                    appointed_on: Some("2005-01-01".to_string()),
                    resigned_on: None,
                },
                // A secretary is not a director.
                Officer {
                    name: Some("D Gray".to_string()),
                    officer_role: Some("secretary".to_string()),
                    appointed_on: Some("2005-01-01".to_string()),
                    resigned_on: None,
                },
            ],
        };
        seed_officers(cache_dir.path(), "12345678", &officers);

        let fetched = client
            .get_officers("12345678")
            .await
            .expect("serving from cache should succeed");
        assert_eq!(fetched.items.len(), 4);
        assert_eq!(fetched.directors(), vec!["A Bloggs", "C Jones Ltd"]);
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
                    description_values: std::collections::BTreeMap::from([
                        ("period_start_on".to_string(), "2024-07-01".to_string()),
                        ("made_up_date".to_string(), "2025-06-30".to_string()),
                    ]),
                    links: None,
                },
                FilingHistoryItem {
                    date: Some("2025-06-01".to_string()),
                    category: Some("confirmation-statement".to_string()),
                    form_type: Some("CS01".to_string()),
                    description: None,
                    description_values: Default::default(),
                    links: None,
                },
                FilingHistoryItem {
                    date: Some("2024-06-30".to_string()),
                    category: Some("accounts".to_string()),
                    form_type: Some("AA02".to_string()),
                    description: Some("micro-entity accounts".to_string()),
                    description_values: std::collections::BTreeMap::from([
                        ("period_start_on".to_string(), "2023-07-01".to_string()),
                        ("made_up_date".to_string(), "2024-06-30".to_string()),
                    ]),
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
        assert_eq!(
            accounts[0].filed_on(),
            Some(NaiveDate::from_ymd_opt(2025, 6, 30).unwrap())
        );
        assert_eq!(
            accounts[1].filed_on(),
            Some(NaiveDate::from_ymd_opt(2024, 6, 30).unwrap())
        );
    }

    /// `download_filing` resolves the filing by its transaction id from the
    /// (cached) history and serves the document from the `filings_downloads`
    /// cache — named `{company}-{period_end}-{filing_id}` — without any
    /// network access (a miss would attempt a request and fail for the
    /// offline client).
    #[tokio::test]
    async fn download_filing_serves_from_filings_downloads_cache() {
        let cache_dir = tempfile::tempdir().unwrap();
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());

        let tx = "MzA1OTg4NDcwMDY5";
        let item = FilingHistoryItem {
            date: Some("2025-08-31".to_string()),
            category: Some("accounts".to_string()),
            form_type: Some("AA".to_string()),
            description: Some("micro-entity accounts".to_string()),
            description_values: std::collections::BTreeMap::from([
                ("period_start_on".to_string(), "2024-12-01".to_string()),
                ("made_up_date".to_string(), "2025-08-31".to_string()),
            ]),
            links: Some(FilingHistoryLinks {
                self_link: Some(format!(
                    "https://api.company-information.service.gov.uk/company/12345678/filing-history/{tx}"
                )),
                document_metadata: Some(
                    "https://document-api.company-information.service.gov.uk/document/WztYq5AM2RR80XakT3KPzoVDvWpTycq6gHuxlnSrXrs"
                        .to_string(),
                ),
            }),
        };
        assert_eq!(item.transaction_id().as_deref(), Some(tx));
        seed_filing_history(
            cache_dir.path(),
            "12345678",
            &FilingHistory {
                total_count: Some(1),
                items: vec![item],
            },
        );

        // The period end (from `made_up_date`) keys the cache filename, and
        // the download cache lives in the `filings_downloads` subdirectory.
        let period_end = NaiveDate::from_ymd_opt(2025, 8, 31).unwrap();
        assert_eq!(
            filing_download_file_name("12345678", Some(period_end), tx),
            format!("12345678-2025-08-31-{tx}")
        );
        assert_eq!(
            filing_download_cache_path(cache_dir.path(), "12345678", Some(period_end), tx),
            cache_dir
                .path()
                .join("filings_downloads")
                .join(format!("12345678-2025-08-31-{tx}"))
        );
        write_cached_filing_download(
            cache_dir.path(),
            "12345678",
            Some(period_end),
            tx,
            b"<html>cached iXBRL</html>",
        );

        let bytes = client
            .download_filing("12345678", tx)
            .await
            .expect("serving from the filings_downloads cache should succeed");
        assert_eq!(bytes, b"<html>cached iXBRL</html>");
    }

    /// The cached raw filing history parses into typed dates: each filing's
    /// registration date and, for the accounts filings, the period they cover
    /// (from the `description_values`).
    #[tokio::test]
    async fn parsed_filings_from_cached_history() {
        let cache_dir = tempfile::tempdir().unwrap();
        let client = CompaniesHouseClient::offline().with_cache_dir(cache_dir.path());

        let history = FilingHistory {
            total_count: Some(2),
            items: vec![
                FilingHistoryItem {
                    date: Some("2025-06-30".to_string()),
                    category: Some("accounts".to_string()),
                    form_type: Some("AA02".to_string()),
                    description: Some("micro-entity accounts".to_string()),
                    description_values: std::collections::BTreeMap::from([
                        ("period_start_on".to_string(), "2024-07-01".to_string()),
                        ("made_up_date".to_string(), "2025-06-30".to_string()),
                    ]),
                    links: None,
                },
                FilingHistoryItem {
                    date: Some("2025-06-01".to_string()),
                    category: Some("confirmation-statement".to_string()),
                    form_type: Some("CS01".to_string()),
                    description: None,
                    description_values: Default::default(),
                    links: None,
                },
            ],
        };
        seed_filing_history(cache_dir.path(), "12345678", &history);

        let fetched = client
            .get_filing_history("12345678")
            .await
            .expect("serving from cache should succeed");
        assert_eq!(fetched.total_count, Some(2));

        let filings: Vec<_> = fetched.parsed().collect();
        assert_eq!(filings.len(), 2);

        let accounts = &filings[0];
        assert_eq!(accounts.category.as_deref(), Some("accounts"));
        assert_eq!(
            accounts.filed_on,
            Some(NaiveDate::from_ymd_opt(2025, 6, 30).unwrap())
        );
        assert_eq!(
            accounts.period_start,
            Some(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap())
        );
        assert_eq!(
            accounts.period_end,
            Some(NaiveDate::from_ymd_opt(2025, 6, 30).unwrap())
        );

        let cs = &filings[1];
        assert_eq!(
            cs.filed_on,
            Some(NaiveDate::from_ymd_opt(2025, 6, 1).unwrap())
        );
        assert_eq!(cs.period_start, None);
        assert_eq!(cs.period_end, None);
    }

    /// Each filing kind parses into its typed struct, with the common
    /// fields (filed-on date, transaction id, description) on every variant
    /// and the kind's own fields filled from the `description_values`.
    #[test]
    fn typed_filings_parse_each_kind() {
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let tx = |id: &str| {
            Some(format!(
                "https://api.company-information.service.gov.uk/company/12345678/filing-history/{id}"
            ))
        };
        let item = |category: &str,
                    form_type: &str,
                    date: &str,
                    description: Option<&str>,
                    description_values: &[(&str, &str)]|
         -> FilingHistoryItem {
            FilingHistoryItem {
                date: Some(date.to_string()),
                category: Some(category.to_string()),
                form_type: Some(form_type.to_string()),
                description: description.map(str::to_string),
                description_values: description_values
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                links: None,
            }
        };

        // Accounts (category fallback: an unknown code still parses).
        let accounts = TypedFiling::try_from(&item(
            "accounts",
            "ZZ-NEW",
            "2025-06-30",
            Some("micro company accounts"),
            &[
                ("period_start_on", "2024-07-01"),
                ("period_end_on", "2025-06-30"),
            ],
        ))
        .expect("accounts parses");
        let TypedFiling::Accounts(accounts) = accounts else {
            panic!("expected Accounts, got {accounts:?}");
        };
        assert_eq!(accounts.filed_on, Some(d("2025-06-30")));
        assert_eq!(accounts.period_start, Some(d("2024-07-01")));
        assert_eq!(accounts.period_end, Some(d("2025-06-30")));
        assert_eq!(accounts.made_up_to, None, "no made_up key in this fixture");

        // Accounts with only `made_up_date` (the 14510633 shape).
        let accounts = TypedFiling::try_from(&item(
            "accounts",
            "AA",
            "2025-08-31",
            None,
            &[("made_up_date", "2025-08-31")],
        ))
        .expect("accounts parses");
        let TypedFiling::Accounts(accounts) = accounts else {
            panic!("expected Accounts, got {accounts:?}");
        };
        assert_eq!(accounts.period_end, Some(d("2025-08-31")));
        assert_eq!(accounts.made_up_to, Some(d("2025-08-31")));
        assert_eq!(accounts.period_start, None);

        // Confirmation statement: CH reports the statement date as
        // `made_up_date`.
        let cs = TypedFiling::try_from(&item(
            "confirmation-statement",
            "CS01",
            "2025-12-09",
            None,
            &[("made_up_date", "2025-11-27")],
        ))
        .expect("confirmation statement parses");
        let TypedFiling::ConfirmationStatement(cs) = cs else {
            panic!("expected ConfirmationStatement, got {cs:?}");
        };
        assert_eq!(cs.made_on, Some(d("2025-11-27")));

        // ARD changes and officer changes are not implemented yet: the typed
        // parse refuses them with the Unimplemented error kind.
        let ard = TypedFiling::try_from(&item(
            "accounts",
            "AA01",
            "2024-03-01",
            Some("change of accounting reference date"),
            &[("new_ard_date", "2025-06-30")],
        ))
        .expect_err("ARD change is unimplemented");
        assert!(
            matches!(&ard, CompaniesHouseError::Unimplemented { filing_type } if filing_type.as_str() == "AA01"),
            "{ard}"
        );

        for code in ["AP01", "TM01", "CH01"] {
            let err = TypedFiling::try_from(&item(
                "officers",
                code,
                "2024-01-15",
                None,
                &[
                    ("appointment_date", "2024-01-15"),
                    ("officer_name", "BLOGGS, A"),
                ],
            ))
            .expect_err("officer changes are unimplemented");
            assert!(
                matches!(&err, CompaniesHouseError::Unimplemented { filing_type } if filing_type.as_str() == code),
                "{code}: {err}"
            );
        }

        // Incorporation (empty description_values).
        let inc = TypedFiling::try_from(&item("incorporation", "NEWINC", "2022-11-28", None, &[]))
            .expect("incorporation parses");
        let TypedFiling::Incorporation(inc) = inc else {
            panic!("expected Incorporation, got {inc:?}");
        };
        assert_eq!(inc.filed_on, Some(d("2022-11-28")));

        // A registered-office address change (AD01) parses its fields (the
        // 14510633 shape: `change_date` + old/new addresses).
        let ad = TypedFiling::try_from(&item(
            "address",
            "AD01",
            "2023-12-11",
            None,
            &[
                ("change_date", "2023-12-11"),
                ("old_address", "Twincross St Thomas Close Peterlee Tyne and Wear SR8 3AR"),
                ("new_address", "52 Holly Avenue Whitley Bay NE26 1ED"),
            ],
        ))
        .expect("address change parses");
        let TypedFiling::AddressChange(ad) = ad else {
            panic!("expected AddressChange, got {ad:?}");
        };
        assert_eq!(ad.filed_on, Some(d("2023-12-11")));
        assert_eq!(ad.change_date, Some(d("2023-12-11")));
        assert_eq!(
            ad.old_address.as_deref(),
            Some("Twincross St Thomas Close Peterlee Tyne and Wear SR8 3AR")
        );
        assert_eq!(ad.new_address.as_deref(), Some("52 Holly Avenue Whitley Bay NE26 1ED"));

        // Any other kind keeps its raw form type code.
        let other = TypedFiling::try_from(&item("capital", "SH01", "2023-06-01", None, &[]))
            .expect("other parses");
        let TypedFiling::Other(other) = other else {
            panic!("expected Other, got {other:?}");
        };
        assert_eq!(other.form_type.as_deref(), Some("SH01"));
        assert_eq!(other.filed_on, Some(d("2023-06-01")));

        // The transaction id (from `links.self`) is carried on every parsed
        // kind.
        let mut tx_item = item("accounts", "AA", "2025-08-31", None, &[]);
        tx_item.links = Some(FilingHistoryLinks {
            self_link: tx("MzA1OTg4NDcwMDY5"),
            document_metadata: None,
        });
        let TypedFiling::Accounts(with_tx) =
            TypedFiling::try_from(&tx_item).expect("accounts parses")
        else {
            panic!("expected Accounts");
        };
        assert_eq!(with_tx.transaction_id.as_deref(), Some("MzA1OTg4NDcwMDY5"));
    }

    /// The `FormType` classifier maps the known codes and preserves unknown
    /// ones verbatim.
    #[test]
    fn form_type_classifies_codes() {
        assert_eq!(FormType::from_code("AA"), FormType::Accounts);
        assert_eq!(FormType::from_code("AA02"), FormType::MicroEntityAccounts);
        assert_eq!(FormType::from_code("AAMD"), FormType::DormantAccounts);
        assert_eq!(FormType::from_code("CS01"), FormType::ConfirmationStatement);
        assert_eq!(
            FormType::from_code("AA01"),
            FormType::ChangeAccountingReferenceDate
        );
        assert_eq!(
            FormType::from_code("AD01"),
            FormType::ChangeRegisteredOffice
        );
        assert_eq!(FormType::from_code("AP02"), FormType::OfficerAppointed);
        assert_eq!(FormType::from_code("TM02"), FormType::OfficerTerminated);
        assert_eq!(FormType::from_code("CH03"), FormType::OfficerDetailsChanged);
        assert_eq!(FormType::from_code("NEWINC"), FormType::Incorporation);
        assert_eq!(
            FormType::from_code("SH01"),
            FormType::Other {
                code: "SH01".to_string()
            }
        );
        assert_eq!(FormType::from_code("SH01").as_code(), "SH01");
        assert_eq!(FormType::Accounts.as_code(), "AA");
    }

    /// A minimal CH WebFiling micro-entity iXBRL document (the FRC UK
    /// taxonomy) — the shape the real 14510633 accounts filings use:
    /// `uk-bus:` period facts (ISO dates), `uk-core:` balance-sheet facts in
    /// the no-dimension instant context (`icur1`), the two `Creditors` lines
    /// in the `WithinOneYear` / `AfterOneYear` dimension contexts, `zerodash`
    /// (`-`) for blank figures, no comparatives.
    fn ch_micro_entity_ixbrl() -> String {
        let ctx = |id: &str, dim: Option<&str>| {
            let segment = dim
                .map(|m| format!(
                    "<xbrldi:segment><xbrldi:explicitMember dimension=\"uk-core:CreditorsPeriod\">{m}</xbrldi:explicitMember></xbrldi:segment>"
                ))
                .unwrap_or_default();
            format!(
                "<xbrli:context id=\"{id}\"><xbrli:entity><xbrli:identifier scheme=\"http://www.companieshouse.gov.uk/company\">12345678</xbrli:identifier></xbrli:entity><xbrli:period><xbrli:instant>2023-11-30</xbrli:instant></xbrli:period>{segment}</xbrli:context>"
            )
        };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml"
  xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:xbrldi="http://xbrl.org/2006/xbrldi"
  xmlns:uk-bus="http://xbrl.frc.org.uk/cd/2023-01-01/business"
  xmlns:uk-core="http://xbrl.frc.org.uk/fr/2023-01-01/core">
<head><title>micro-entity accounts</title></head>
<body><ix:header><ix:hidden>
  <ix:nonNumeric contextRef="icur1" name="uk-bus:StartDateForPeriodCoveredByReport">2022-11-28</ix:nonNumeric>
  <ix:nonNumeric contextRef="icur1" name="uk-bus:EndDateForPeriodCoveredByReport">2023-11-30</ix:nonNumeric>
  <ix:nonFraction contextRef="icur1" decimals="2" format="ixt2:numdotdecimal" name="uk-core:CalledUpShareCapitalNotPaidNotExpressedAsCurrentAsset" unitRef="GBP">100</ix:nonFraction>
  <ix:nonFraction contextRef="icur1" decimals="2" format="ixt2:zerodash" name="uk-core:FixedAssets" unitRef="GBP">-</ix:nonFraction>
  <ix:nonFraction contextRef="icur1" decimals="2" format="ixt2:numdotdecimal" name="uk-core:CurrentAssets" unitRef="GBP">68,946</ix:nonFraction>
  <ix:nonFraction contextRef="icur9" decimals="2" format="ixt2:numdotdecimal" name="uk-core:Creditors" unitRef="GBP">570</ix:nonFraction>
  <ix:nonFraction contextRef="icur11" decimals="2" format="ixt2:numdotdecimal" name="uk-core:Creditors" unitRef="GBP">0</ix:nonFraction>
  <ix:nonFraction contextRef="icur1" decimals="2" format="ixt2:numdotdecimal" name="uk-core:NetCurrentAssetsLiabilities" unitRef="GBP">68,376</ix:nonFraction>
  <ix:nonFraction contextRef="icur1" decimals="2" format="ixt2:numdotdecimal" name="uk-core:TotalAssetsLessCurrentLiabilities" unitRef="GBP">68,376</ix:nonFraction>
  <ix:nonFraction contextRef="icur1" decimals="2" format="ixt2:numdotdecimal" name="uk-core:ProvisionsForLiabilitiesBalanceSheetSubtotal" unitRef="GBP">0</ix:nonFraction>
  <ix:nonFraction contextRef="icur1" decimals="2" format="ixt2:numdotdecimal" name="uk-core:AccruedLiabilitiesNotExpressedWithinCreditorsSubtotal" unitRef="GBP">0</ix:nonFraction>
  <ix:nonFraction contextRef="icur1" decimals="2" format="ixt2:numdotdecimal" name="uk-core:NetAssetsLiabilities" unitRef="GBP">68,376</ix:nonFraction>
  <ix:nonFraction contextRef="icur1" decimals="2" format="ixt2:numdotdecimal" name="uk-core:Equity" unitRef="GBP">68,376</ix:nonFraction>
</ix:hidden></ix:header>
{}{}{}
<div>report body</div>
</body></html>"#,
            ctx("icur1", None),
            ctx("icur9", Some("uk-core:WithinOneYear")),
            ctx("icur11", Some("uk-core:AfterOneYear")),
        )
    }

    /// A minimal FRC 2024-taxonomy iXBRL document — the newer CH renderer:
    /// `core:` prefixes (no `uk-`), `DD.MM.YY` period dates, comparatives
    /// present (`PY_END` at the earlier instant) — to pin the tolerance of
    /// the second pass (both taxonomies' prefixes, both date formats, the
    /// comparatives excluded from the reported figures).
    fn ch_micro_entity_ixbrl_2024() -> String {
        let ctx = |id: &str, instant: &str, dim: Option<&str>| {
            let segment = dim
                .map(|m| format!(
                    "<xbrldi:segment><xbrldi:explicitMember dimension=\"core:CreditorsPeriod\">{m}</xbrldi:explicitMember></xbrldi:segment>"
                ))
                .unwrap_or_default();
            format!(
                "<xbrli:context id=\"{id}\"><xbrli:entity><xbrli:identifier scheme=\"http://www.companieshouse.gov.uk/company\">12345678</xbrli:identifier></xbrli:entity><xbrli:period><xbrli:instant>{instant}</xbrli:instant></xbrli:period>{segment}</xbrli:context>"
            )
        };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml"
  xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:xbrldi="http://xbrl.org/2006/xbrldi"
  xmlns:uk-bus="http://xbrl.frc.org.uk/cd/2024-01-01/business"
  xmlns:core="http://xbrl.frc.org.uk/fr/2024-01-01/core">
<head><title>micro-entity accounts</title></head>
<body><ix:header><ix:hidden>
  <ix:nonNumeric contextRef="CY_END" format="ixt2:datedaymonthyear" name="uk-bus:StartDateForPeriodCoveredByReport">1.12.23</ix:nonNumeric>
  <ix:nonNumeric contextRef="CY_END" format="ixt2:datedaymonthyear" name="uk-bus:EndDateForPeriodCoveredByReport">30.11.24</ix:nonNumeric>
  <ix:nonFraction contextRef="CY_END" decimals="2" name="core:CalledUpShareCapitalNotPaidNotExpressedAsCurrentAsset" unitRef="GBP">0</ix:nonFraction>
  <ix:nonFraction contextRef="CY_END" decimals="2" name="core:FixedAssets" unitRef="GBP">0</ix:nonFraction>
  <ix:nonFraction contextRef="CY_END" decimals="2" name="core:CurrentAssets" unitRef="GBP">74,991</ix:nonFraction>
  <ix:nonFraction contextRef="PY_END" decimals="2" name="core:CurrentAssets" unitRef="GBP">68,946</ix:nonFraction>
  <ix:nonFraction contextRef="CreditorsWithinOneYear_CY_END" decimals="2" name="core:Creditors" unitRef="GBP">0</ix:nonFraction>
  <ix:nonFraction contextRef="CreditorsWithinOneYear_PY_END" decimals="2" name="core:Creditors" unitRef="GBP">570</ix:nonFraction>
  <ix:nonFraction contextRef="CY_END" decimals="2" name="core:NetCurrentAssetsLiabilities" unitRef="GBP">74,991</ix:nonFraction>
  <ix:nonFraction contextRef="CY_END" decimals="2" name="core:TotalAssetsLessCurrentLiabilities" unitRef="GBP">74,991</ix:nonFraction>
  <ix:nonFraction contextRef="CY_END" decimals="2" name="core:ProvisionsForLiabilitiesBalanceSheetSubtotal" unitRef="GBP">540</ix:nonFraction>
  <ix:nonFraction contextRef="CY_END" decimals="2" name="core:NetAssetsLiabilities" unitRef="GBP">74,451</ix:nonFraction>
  <ix:nonFraction contextRef="CY_END" decimals="2" name="core:Equity" unitRef="GBP">74,451</ix:nonFraction>
</ix:hidden></ix:header>
{}{}{}{}
<div>report body</div>
</body></html>"#,
            ctx("CY_END", "2024-11-30", None),
            ctx("PY_END", "2023-11-30", None),
            ctx(
                "CreditorsWithinOneYear_CY_END",
                "2024-11-30",
                Some("core:WithinOneYear")
            ),
            ctx(
                "CreditorsWithinOneYear_PY_END",
                "2023-11-30",
                Some("core:WithinOneYear")
            ),
        )
    }

    /// The two-pass parse reads CH's micro-entity iXBRL: the generic pass
    /// collects the facts into the IR, the accounts pass resolves the
    /// current-period context and the creditor dimensions, applies the sign
    /// convention, and recovers the period from the document.
    #[test]
    fn parse_filed_accounts_reads_ch_micro_entity_ixbrl() {
        let bs = parse_filed_accounts(&ch_micro_entity_ixbrl()).expect("the fixture parses");

        assert_eq!(
            bs.period_start,
            NaiveDate::from_ymd_opt(2022, 11, 28).unwrap()
        );
        assert_eq!(
            bs.period_end,
            NaiveDate::from_ymd_opt(2023, 11, 30).unwrap()
        );

        let f = &bs.figures;
        assert_eq!(f.current_assets, 68_946.0);
        assert_eq!(
            f.creditors_within_1_year, -570.0,
            "creditors stored negative"
        );
        assert_eq!(f.creditors_after_1_year, 0.0);
        assert_eq!(f.net_current_assets, 68_376.0);
        assert_eq!(f.total_assets_less_liabilities, 68_376.0);
        assert_eq!(f.net_assets, 68_376.0);
        assert_eq!(f.capital_and_reserves, 68_376.0);
        assert_eq!(f.called_up_share_capital_not_paid, 100.0);
        assert_eq!(f.fixed_assets, 0.0, "zerodash reads as zero");
        assert_eq!(f.prepayments_and_accrued_income, 0.0);
        assert_eq!(f.provisions_for_liabilities, 0.0);
        assert_eq!(f.accruals_and_deferred_income, 0.0);
    }

    /// The FRC 2024 rendering (the real 2025-08-31 filing for 14510633):
    /// `core:` prefixes, `DD.MM.YY` period dates, comparatives present.
    #[test]
    fn parse_filed_accounts_reads_ch_2024_taxonomy() {
        let bs = parse_filed_accounts(&ch_micro_entity_ixbrl_2024()).expect("the fixture parses");

        assert_eq!(
            bs.period_start,
            NaiveDate::from_ymd_opt(2023, 12, 1).unwrap()
        );
        assert_eq!(
            bs.period_end,
            NaiveDate::from_ymd_opt(2024, 11, 30).unwrap()
        );

        let f = &bs.figures;
        assert_eq!(f.current_assets, 74_991.0);
        assert_eq!(f.creditors_within_1_year, 0.0, "current-period creditors");
        assert_eq!(f.net_current_assets, 74_991.0);
        assert_eq!(f.total_assets_less_liabilities, 74_991.0);
        assert_eq!(f.provisions_for_liabilities, 540.0);
        assert_eq!(
            f.called_up_share_capital_not_paid, 0.0,
            "present as zero in the 2024 rendering"
        );
        assert_eq!(f.net_assets, 74_451.0);
        assert_eq!(f.capital_and_reserves, 74_451.0);
        // The comparatives (PY_END: 68,946 / -570) must not leak into the
        // filed period's figures.
        assert_ne!(f.current_assets, 68_946.0);
        assert_ne!(f.creditors_within_1_year, -570.0);
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

        let company = company_with_period(
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
            next.period.start,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
        );
        assert_eq!(
            next.period.end,
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
        );
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

        let company = company_with_period(
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
        assert_eq!(next.period.start, expected.start);
        assert_eq!(next.period.end, expected.end);
        assert_eq!(
            next.deadline_to_file_companies_house_accounts,
            expected.end + Months::new(9)
        );
        assert_eq!(
            next.deadline_to_file_hmrc_ct600,
            expected.end + Months::new(12)
        );
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

        let company = company_with_period(
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
        assert_eq!(
            next.period.end,
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
        );
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

        let company = company_with_period(
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

        assert_eq!(
            next.period.start,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
        );
        // The registration-date period containing 2025-01-01.
        let anchored =
            company.accounting_period_containing(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        assert_eq!(next.period.end, anchored.end);
        assert_eq!(
            next.deadline_to_file_hmrc_ct600,
            anchored.end + Months::new(12)
        );
        assert_eq!(
            next.deadline_to_file_companies_house_accounts,
            anchored.end + Months::new(9)
        );
    }

}

// ============================================================================
// Live API tests (part of the default-enabled `cached_live_tests` feature)
// ============================================================================

/// Live Companies House API tests.
///
/// These tests exercise the real (or sandbox) Companies House API and are
/// part of the default-enabled `cached_live_tests` feature, so a plain
/// `cargo test -p ct600` runs them.  The client is pointed at the
/// repository's `.cache/api_responses` — the first (cold) run fetches from
/// the API and warms it; repeat runs are served from disk and never touch
/// the API.
/// Enabling the `always_live_tests` feature instead uses a scratch tempdir
/// cache, so every run really hits the network (e.g. to refresh the cache).
///
/// The cold run needs an API key and (for most tests) a `COMPANY_NUMBER` —
/// the period and past-filings tests default to the real company `14510633`:
///
/// ```bash
/// export COMPANIES_HOUSE_API_KEY="your-api-key"           # live API
/// # or
/// export COMPANIES_HOUSE_SANDBOX_API_KEY="your-api-key"   # sandbox API
/// export COMPANY_NUMBER="00000006"                        # a company that exists in the API you chose
/// cargo test -p ct600
/// ```
///
/// To run fully offline (a fresh clone without a key), disable the features
/// with `cargo test -p ct600 --no-default-features`: the tests are then
/// reported as ignored.
#[cfg(test)]
mod live_tests {
    use super::*;

    /// The cache directory the live tests use: a scratch tempdir when
    /// `always_live_tests` is enabled, else the repository's
    /// `.cache/api_responses` (the `cached_live_tests` default).
    fn live_cache_dir() -> PathBuf {
        #[cfg(feature = "always_live_tests")]
        {
            tempfile::tempdir().unwrap().keep()
        }
        #[cfg(not(feature = "always_live_tests"))]
        {
            test_utils::cache_dir("api_responses")
        }
    }

    /// A client for the live API, or the sandbox when only the sandbox key is
    /// set (mirroring [`test_utils::TestClient`]), pointed at the mode's cache
    /// directory (see [`live_cache_dir`]).
    ///
    /// With `cached_live_tests` (the default) a missing key is tolerated when
    /// the cache is warm — the client never reaches the network — but the
    /// first, cold run needs a key.  With `always_live_tests` a key is
    /// mandatory.
    fn live_client() -> CompaniesHouseClient {
        let client = CompaniesHouseClient::live_from_env()
            .or_else(|_| CompaniesHouseClient::test_client_from_env());
        #[cfg(feature = "always_live_tests")]
        let client = client.expect(
            "the always_live_tests feature needs COMPANIES_HOUSE_API_KEY (live) or \
             COMPANIES_HOUSE_SANDBOX_API_KEY (sandbox)",
        );
        #[cfg(not(feature = "always_live_tests"))]
        let client = client.unwrap_or_else(|_| CompaniesHouseClient::new(Config::default()));
        client.with_cache_dir(live_cache_dir())
    }

    /// The company number to look up: `COMPANY_NUMBER`.
    fn company_number() -> String {
        std::env::var("COMPANY_NUMBER").expect(
            "the live-tests features need COMPANY_NUMBER: use a real company for \
             the live API, a sandbox test company for the sandbox API",
        )
    }

    /// A real company profile round-trips through the cache-first profile
    /// fetch: a cold cache resolves from `GET /company/{number}` and warms
    /// the cache; a warm cache serves the same profile from disk.
    #[tokio::test]
    #[cfg_attr(
        not(any(feature = "cached_live_tests", feature = "always_live_tests")),
        ignore = "requires a Companies House API key (cold cache) and COMPANY_NUMBER"
    )]
    async fn live_profile_round_trip() {
        let number = company_number();
        let profile = live_client()
            .get_company_profile_cached(&number)
            .await
            .expect("fetch the company profile");

        assert_eq!(profile.company_number, number);
        assert!(
            !profile.company_name.is_empty(),
            "a real company has a name"
        );
        assert!(
            profile.date_of_creation.is_some(),
            "a real company has a date of creation"
        );
    }

    /// A real company's filing history decodes, and the `accounts` filter
    /// surfaces the previous accounts filings with their registration dates.
    #[tokio::test]
    #[cfg_attr(
        not(any(feature = "cached_live_tests", feature = "always_live_tests")),
        ignore = "requires a Companies House API key (cold cache) and COMPANY_NUMBER"
    )]
    async fn live_filing_history_decodes() {
        let number = company_number();
        let history = live_client()
            .get_filing_history(&number)
            .await
            .expect("fetch the filing history");

        // A long-established company has filings; every accounts filing carries
        // a registration date the parser can read.
        for item in history.accounts() {
            assert_eq!(item.category.as_deref(), Some("accounts"));
            assert!(
                item.filed_on().is_some(),
                "an accounts filing carries a registration date: {item:?}"
            );
        }
    }

    /// The company profile of a real company (default `14510633`, overridable
    /// with `COMPANY_NUMBER`) drives the accounting-period schedule: every
    /// period from incorporation to today is asserted coherent (ordered,
    /// contiguous, the first starting at the registration date and the last
    /// reaching today).
    #[tokio::test]
    #[cfg_attr(
        not(any(feature = "cached_live_tests", feature = "always_live_tests")),
        ignore = "requires a Companies House API key for a cold cache"
    )]
    async fn live_profile_periods() {
        let number = std::env::var("COMPANY_NUMBER").unwrap_or_else(|_| "14510633".to_string());
        let client = live_client();

        // The client's cache-first company-details method: the profile
        // carries the registration date that anchors the period schedule
        // (a warm cache serves it without touching the API).
        let profile = client
            .get_company_profile_cached(&number)
            .await
            .expect("fetch the company profile");
        assert_eq!(profile.company_number, number);
        let registration_date = profile
            .date_of_creation
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .expect("the profile carries a parseable date of creation");

        let mut company = Company::new("", "", &number);
        company.registration_date = registration_date;
        let today = chrono::Utc::now().date_naive();

        // All periods from incorporation to today.
        let mut periods = Vec::new();
        let mut n = 0u32;
        loop {
            let period = company.accounting_period_n(n);
            if period.start > today {
                break;
            }
            periods.push(period);
            n += 1;
        }

        println!(
            "company {number} ({}) — periods from {registration_date} to {today}:",
            profile.company_name
        );
        for (i, period) in periods.iter().enumerate() {
            println!("  period {i}: {} → {}", period.start, period.end);
        }

        assert!(!periods.is_empty(), "at least the period containing today");
        assert_eq!(
            periods[0].start, registration_date,
            "the first period starts at incorporation"
        );
        assert!(
            periods.last().unwrap().end >= today,
            "the last period reaches today"
        );
        for window in periods.windows(2) {
            assert_eq!(
                window[1].start,
                window[0].end + chrono::Duration::days(1),
                "periods are contiguous"
            );
            assert!(window[0].start < window[0].end, "periods are ordered");
        }
        assert_eq!(
            *periods.last().unwrap(),
            company.accounting_period_containing(today),
            "the last period is the one containing today"
        );
    }

    /// The past filings of a real company (default `14510633`, overridable
    /// with `COMPANY_NUMBER`) parse into typed dates — printed: the period
    /// each filing covers and when it was filed — and the filed documents
    /// are downloaded by filing id (cache-first, landing in the
    /// `filings_downloads` cache subdirectory, so repeat runs never touch
    /// the network).
    #[tokio::test]
    #[cfg_attr(
        not(any(feature = "cached_live_tests", feature = "always_live_tests")),
        ignore = "requires a Companies House API key for a cold cache"
    )]
    async fn live_past_filings_print_dates() {
        let number = std::env::var("COMPANY_NUMBER").unwrap_or_else(|_| "14510633".to_string());
        let client = live_client();

        let history = client
            .get_filing_history(&number)
            .await
            .expect("fetch the filing history");
        // Download the filed documents by filing id (cache-first): every
        // first-page filing carrying a document is downloaded, each
        // verified to land in the `filings_downloads` cache subdirectory,
        // and the accounts documents are parsed so the typed print below
        // can show the period start — CH's filing-history API reports only
        // the period end (`made_up_date`), never the start.
        let mut parsed_periods: std::collections::HashMap<String, (NaiveDate, NaiveDate)> =
            std::collections::HashMap::new();
        let mut n_downloaded = 0usize;
        for item in &history.items {
            let Some(tx) = item.transaction_id() else {
                continue;
            };
            let has_document = item
                .links
                .as_ref()
                .and_then(|l| l.document_metadata.as_deref())
                .is_some_and(|u| !u.is_empty());
            if !has_document {
                continue;
            }
            let bytes = client
                .download_filing(&number, &tx)
                .await
                .expect("download the filing document");
            assert!(
                !bytes.is_empty(),
                "a downloaded document is non-empty: {tx}"
            );
            n_downloaded += 1;

            // The download is cached (cache-first on the next call): the
            // cache file exists on disk with exactly these bytes.
            if let Some(cache_dir) = client.cache_dir() {
                let date = ChFiling::from(item).period_end.or_else(|| item.filed_on());
                assert_eq!(
                    read_cached_filing_download(cache_dir, &number, date, &tx).as_deref(),
                    Some(bytes.as_slice()),
                    "the download is cached under filings_downloads: {tx}"
                );
            }

            // The accounts documents are iXBRL (the other filings' are
            // PDFs, which fail to parse and are skipped): record the
            // document's own period.
            if let Ok(bs) = parse_filed_accounts(&String::from_utf8_lossy(&bytes)) {
                parsed_periods.insert(tx, (bs.period_start, bs.period_end));
            }
        }
        assert!(
            n_downloaded > 0,
            "the company has at least one downloadable filing"
        );

        // The typed parse classifies each filing into its kind — accounts
        // (with the period), confirmation statements (with the statement
        // date), incorporation, or other. ARD changes and officer changes
        // are unimplemented kinds; this company has none, so every item
        // parses.
        let typed: Vec<_> = history
            .typed()
            .collect::<Result<Vec<_>, _>>()
            .expect("no unimplemented filing kinds in this company's history");
        println!(
            "past filings for {number} ({} total):",
            history.total_count.unwrap_or(typed.len())
        );
        for (i, typed) in typed.iter().enumerate() {
            let filed = typed
                .filed_on()
                .map(|d| d.to_string())
                .unwrap_or_else(|| "—".to_string());
            let item = &history.items[i];
            let (kind, detail) = match typed {
                TypedFiling::Accounts(a) => {
                    // The typed parse knows only the period end (from
                    // `made_up_date`); prefer the document's own period
                    // (start + end), recovered from the parse above.
                    let period = a
                        .period_start
                        .zip(a.period_end)
                        .or_else(|| {
                            item.transaction_id()
                                .and_then(|tx| parsed_periods.get(&tx).copied())
                        });
                    (
                        "accounts",
                        match period {
                            Some((start, end)) => format!("period {start} → {end}"),
                            None => format!(
                                "period {} → {}",
                                a.period_start
                                    .map(|d| d.to_string())
                                    .unwrap_or_else(|| "—".to_string()),
                                a.period_end
                                    .map(|d| d.to_string())
                                    .unwrap_or_else(|| "—".to_string()),
                            ),
                        },
                    )
                }
                TypedFiling::ConfirmationStatement(cs) => (
                    "confirmation-statement",
                    cs.made_on
                        .map(|d| format!("made up to {d}"))
                        .unwrap_or_else(|| "no date".to_string()),
                ),
                // Reserved kinds (ARD changes, officer changes) never parse
                // yet — they return Unimplemented — so they don't appear here.
                TypedFiling::ChangeOfAccountingReferenceDate(_) | TypedFiling::OfficerChange(_) => {
                    unreachable!("reserved kinds are unimplemented")
                }
                TypedFiling::AddressChange(ad) => (
                    "address-change",
                    match (ad.change_date, ad.new_address.as_deref()) {
                        (Some(date), Some(address)) => {
                            format!("changed {date}  new address {address}")
                        }
                        (Some(date), None) => format!("changed {date}"),
                        (None, Some(address)) => format!("new address {address}"),
                        (None, None) => String::new(),
                    },
                ),
                TypedFiling::Incorporation(_) => ("incorporation", String::new()),
                TypedFiling::Other(o) => {
                    ("other", o.form_type.as_deref().unwrap_or("?").to_string())
                }
            };
            let mut parts = vec![format!("filed {filed}"), kind.to_string()];
            if !detail.is_empty() {
                parts.push(detail);
            }
            println!("  {}", parts.join("  "));
        }
        // Every item classified, and the accounts items carry a period end.
        assert_eq!(typed.len(), history.items.len());
        assert!(
            typed
                .iter()
                .any(|t| matches!(t, TypedFiling::Accounts(a) if a.period_end.is_some())),
            "at least one accounts filing with a period end"
        );
    }

    /// A real company's officers decode, and the current directors can be
    /// read off them.
    #[tokio::test]
    #[cfg_attr(
        not(any(feature = "cached_live_tests", feature = "always_live_tests")),
        ignore = "requires a Companies House API key (cold cache) and COMPANY_NUMBER"
    )]
    async fn live_officers_decode() {
        let number = company_number();
        let officers = live_client()
            .get_officers(&number)
            .await
            .expect("fetch the officers");

        // A company has officers, each carrying a name the parser can read,
        // and the current directors are a non-empty subset of them.
        for officer in &officers.items {
            assert!(
                officer.name.as_deref().is_some_and(|n| !n.is_empty()),
                "an officer carries a name: {officer:?}"
            );
        }
        assert!(
            !officers.directors().is_empty(),
            "a company has current directors"
        );
    }

    /// The next accounting period to file resolves from the live profile with
    /// coherent dates and deadlines.
    #[tokio::test]
    #[cfg_attr(
        not(any(feature = "cached_live_tests", feature = "always_live_tests")),
        ignore = "requires a Companies House API key (cold cache) and COMPANY_NUMBER"
    )]
    async fn live_next_accounting_period() {
        let number = company_number();
        let mut company = Company::new("", "", &number);
        company.registration_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let next = live_client()
            .next_accounting_period(&company)
            .await
            .expect("resolve the next accounting period");

        assert!(
            next.period.start < next.period.end,
            "the period is ordered start < end"
        );
        assert!(
            next.deadline_to_file_hmrc_ct600 >= next.period.end,
            "the CT600 deadline is on or after the period end"
        );
        assert!(
            next.deadline_to_file_companies_house_accounts >= next.period.end,
            "the Companies House deadline is on or after the period end"
        );
    }
}
