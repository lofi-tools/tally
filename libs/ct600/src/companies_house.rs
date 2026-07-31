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

use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;
use std::{env::VarError, result::Result};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::form::CompanyFormValues;
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;

/// Errors returned by the Companies House API client.
#[derive(Debug, Error)]
pub enum CompaniesHouseError {
    /// The HTTP request could not be sent.
    #[error("request failed: {0}")]
    RequestFailed(reqwest::Error),

    /// The API returned a non-success status code.
    #[error("GET {url} returned HTTP {status}")]
    HttpStatus {
        url: String,
        status: reqwest::StatusCode,
    },

    /// The response body could not be decoded as JSON.
    #[error("failed to decode response: {0}")]
    DecodeFailed(reqwest::Error),
}

pub type ApiResult<T> = Result<T, CompaniesHouseError>;

/// The repository root directory, resolved via `git rev-parse
/// --show-toplevel`, falling back on the `CARGO_MANIFEST_DIR` build-time path.
pub static REPO: LazyLock<PathBuf> = LazyLock::new(|| {
    let git_root = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()
        .and_then(|out| {
            if !out.status.success() {
                return None;
            }
            let path = std::str::from_utf8(&out.stdout).ok()?.trim();
            (!path.is_empty()).then(|| PathBuf::from(path))
        });
    git_root.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
});

/// A client for the Companies House public API.
///
/// All requests are authenticated with the API key using HTTP basic
/// authentication (username = API key, empty password).
#[derive(Debug, Clone)]
pub struct CompaniesHouseClient {
    base_url: &'static str,
    http: reqwest::Client,
    api_key: String,
}

impl CompaniesHouseClient {
    /// Create a new client using the given API key.
    pub fn test_client_from_env() -> Result<Self, VarError> {
        const API_BASE_URL_TEST: &str = "https://api-sandbox.company-information.service.gov.uk";
        Ok(Self {
            base_url: API_BASE_URL_TEST,
            http: reqwest::Client::new(),
            api_key: std::env::var("COMPANIES_HOUSE_API_KEY_TEST")?,
        })
    }
    pub fn live_from_env() -> Result<Self, VarError> {
        /// Base URL of the Companies House public API.
        const API_BASE_URL: &str = "https://api.company-information.service.gov.uk";

        Ok(Self {
            base_url: API_BASE_URL,
            http: reqwest::Client::new(),
            api_key: std::env::var("COMPANIES_HOUSE_API_KEY")?,
        })
    }

    /// Fetch the company profile for the given company number.
    ///
    /// `GET /company/{companyNumber}`
    pub async fn get_company_profile(&self, company_number: &str) -> ApiResult<CompanyProfile> {
        let url = format!("{}/company/{company_number}", self.base_url);
        let response = self
            .http
            .get(&url)
            .basic_auth(&self.api_key, Some("")) //  Companies House API takes the username as the API key
            .send()
            .await
            .map_err(CompaniesHouseError::RequestFailed)?;

        let status = response.status();
        if !status.is_success() {
            return Err(CompaniesHouseError::HttpStatus { url, status });
        }

        response
            .json::<CompanyProfile>()
            .await
            .map_err(CompaniesHouseError::DecodeFailed)
    }

    /// Fetch the company profile for the tax computation's company number and
    /// combine it with the tax-derived values into the company header boxes
    /// (1, 2, 3, 4, 30 and 35).
    ///
    /// Companies House supplies boxes 1 (name), 2 (registration number) and 4
    /// (type of company, mapped from the register's company type); the tax
    /// reference (3) and the return period (30/35) come from the tax
    /// computation.
    pub async fn company_form_values(&self, tax: &Frs105CorpTax) -> ApiResult<CompanyFormValues> {
        let profile = self.get_company_profile(tax.company_number()).await?;
        Ok(company_form_values_from_profile(&profile, tax))
    }
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
    /// Create a cached client with the default cache directory (`.cache`).
    pub fn new(inner: CompaniesHouseClient) -> Self {
        Self::with_cache_dir(inner, PathBuf::from(".cache"))
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

    /// Create a cached client for tests whose cache lives in
    /// `{REPO}/.cache/api_responses`.
    #[cfg(test)]
    pub fn test_instance(inner: CompaniesHouseClient) -> Self {
        Self::with_cache_dir(inner, REPO.join(".cache/api_responses"))
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

    /// Fetch the company header boxes, using the cached profile when
    /// available.
    pub async fn company_form_values(&self, tax: &Frs105CorpTax) -> ApiResult<CompanyFormValues> {
        let profile = self.get_company_profile(tax.company_number()).await?;
        Ok(company_form_values_from_profile(&profile, tax))
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

/// Build the CT600 company header boxes from a Companies House profile and
/// the tax computation.
fn company_form_values_from_profile(
    profile: &CompanyProfile,
    tax: &Frs105CorpTax,
) -> CompanyFormValues {
    CompanyFormValues {
        company_name: profile.company_name.clone(),
        company_number: profile.company_number.clone(),
        tax_reference: tax.tax_reference().to_string(),
        type_of_company: profile
            .company_type
            .as_deref()
            .and_then(CompanyType::parse_str)
            .map(CompanyType::code)
            .unwrap_or(0),
        start: tax.start(),
        end: tax.end(),
    }
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

    /// EXT SOFTWARE SERVICES LTD, an active private limited company
    /// incorporated on 28 November 2022.
    const TEST_COMPANY_NUMBER: &str = "14510633";

    #[test]
    fn test_company_form_values_from_profile() {
        let profile = CompanyProfile {
            company_number: "14510633".to_string(),
            company_name: "EXT SOFTWARE SERVICES LTD".to_string(),
            company_status: Some("active".to_string()),
            company_status_detail: None,
            date_of_creation: Some("2022-11-28".to_string()),
            date_of_dissolution: None,
            company_type: Some("ltd".to_string()),
            jurisdiction: Some("England/Wales".to_string()),
            registered_office_address: None,
            accounts: None,
            confirmation_statement: None,
            sic_codes: None,
            undeliverable_registered_office_address: None,
            links: None,
        };

        let mut facts = ixbrl::ixbrl_fmt::ParsedIxBrlFacts::default();
        facts.non_numeric.insert(
            "ct-comp:CompanyName".to_string(),
            "EXT SOFTWARE SERVICES LTD".to_string(),
        );
        facts
            .non_numeric
            .insert("ct-comp:TaxReference".to_string(), "1234567890".to_string());
        facts.non_numeric.insert(
            "ct-comp:FinancialYear1CoveredByTheReturn".to_string(),
            "2025".to_string(),
        );
        facts.non_numeric.insert(
            "ct-comp:FinancialYear2CoveredByTheReturn".to_string(),
            "2026".to_string(),
        );
        facts.non_numeric.insert(
            "ct-comp:PeriodOfAccountStartDate".to_string(),
            "1 January 2025".to_string(),
        );
        facts.non_numeric.insert(
            "ct-comp:PeriodOfAccountEndDate".to_string(),
            "31 December 2025".to_string(),
        );
        let company = ixbrl::company::Company::new(
            "EXT SOFTWARE SERVICES LTD",
            "1234567890",
            "14510633",
            chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        let tax = Frs105CorpTax::from_parsed_facts(&facts, &company);

        let values = company_form_values_from_profile(&profile, &tax);

        assert_eq!(values.company_name, "EXT SOFTWARE SERVICES LTD");
        assert_eq!(values.company_number, "14510633");
        assert_eq!(values.tax_reference, "1234567890");
        assert_eq!(values.type_of_company, 1);
        assert_eq!(values.start.to_string(), "2025-01-01");
        assert_eq!(values.end.to_string(), "2025-12-31");
    }

    /// Live integration test against the Companies House API, caching the
    /// response in the repo `.cache/api_responses` directory so repeat runs
    /// don't need the network or an API key.
    ///
    /// Requires a valid API key in the `COMPANIES_HOUSE_API_KEY` or
    /// `COMPANIES_HOUSE_API_KEY_TEST` environment variable and network access
    /// on the first (uncached) run, so it is ignored by default:
    ///
    /// ```text
    /// COMPANIES_HOUSE_API_KEY=<key> cargo test --package ct600 -- --ignored
    /// ```
    #[tokio::test]
    #[ignore = "requires an API key and network access on the first run"]
    async fn get_company_profile_14510633() {
        let inner = CompaniesHouseClient::live_from_env().expect("set COMPANIES_HOUSE_API_KEY");
        let client = CachedCompaniesHouseClient::test_instance(inner);

        let profile = client
            .get_company_profile(TEST_COMPANY_NUMBER)
            .await
            .expect("fetching the company profile should succeed");

        assert_eq!(profile.company_number, "14510633");
        assert_eq!(profile.company_name, "EXT SOFTWARE SERVICES LTD");
        assert_eq!(profile.company_status.as_deref(), Some("active"));
        assert_eq!(profile.company_type.as_deref(), Some("ltd"));
        assert_eq!(profile.date_of_creation.as_deref(), Some("2022-11-28"));
    }

    /// The cache is consulted first: with a populated cache the live API is
    /// never reached (the inner client points at an unreachable address).
    #[tokio::test]
    async fn test_cached_client_serves_from_cache_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let inner = CompaniesHouseClient {
            base_url: "http://127.0.0.1:1",
            http: reqwest::Client::new(),
            api_key: "unused".to_string(),
        };
        let client = CachedCompaniesHouseClient::with_cache_dir(inner, dir.path());

        let profile = CompanyProfile {
            company_number: TEST_COMPANY_NUMBER.to_string(),
            company_name: "EXT SOFTWARE SERVICES LTD".to_string(),
            company_status: Some("active".to_string()),
            company_status_detail: None,
            date_of_creation: Some("2022-11-28".to_string()),
            date_of_dissolution: None,
            company_type: Some("ltd".to_string()),
            jurisdiction: Some("England/Wales".to_string()),
            registered_office_address: None,
            accounts: None,
            confirmation_statement: None,
            sic_codes: None,
            undeliverable_registered_office_address: None,
            links: None,
        };
        let cache_file = client.cache_path(TEST_COMPANY_NUMBER);
        std::fs::write(
            &cache_file,
            serde_json::to_vec(&profile).expect("serialising the profile"),
        )
        .expect("writing the cache file");

        let cached = client
            .get_company_profile(TEST_COMPANY_NUMBER)
            .await
            .expect("serving from cache should succeed");

        assert_eq!(cached.company_name, "EXT SOFTWARE SERVICES LTD");
        assert_eq!(cached.company_type.as_deref(), Some("ltd"));
        assert!(cache_file.exists());
    }
}
