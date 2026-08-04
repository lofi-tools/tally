//! Test fixtures and offline test clients for the company clients.
//!
//! [`TestClient`] serves company profiles from a local cache when available,
//! falling back on hardcoded default fixtures ([`TestData`]), and finally on
//! the live Companies House API when a live client is configured.  It is
//! auto-built from the environment with sensible defaults, so tests run with
//! zero configuration on a fresh checkout.
//!
//! This module is compiled in normal builds so downstream crates' tests can
//! share the fixtures.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use snafu::Snafu;

use super::companies_house::{
    ApiResult, CompaniesHouseClient, CompaniesHouseError, CompanyProfile,
};
use crate::reports::uk_frs105_corp_tax::Frs105CorpTax;

/// The repository root directory.
pub static REPO: LazyLock<PathBuf> = LazyLock::new(|| {
    let path_bytes = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .unwrap()
        .stdout;
    let path_str = std::str::from_utf8(&path_bytes).unwrap().trim();
    PathBuf::from(path_str)
});

fn default_cache_dir() -> PathBuf {
    REPO.join(".cache/api_responses")
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
    /// - `CT600_CACHE_DIR` (default `{repo}/.cache/api_responses`)
    /// - the optional live client is built from `COMPANIES_HOUSE_API_KEY`
    ///   (live API) or `COMPANIES_HOUSE_API_KEY_TEST` (sandbox), or `None`
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
    /// The company number of the fictional default test company.
    pub fn default_company_number() -> &'static str {
        "12345678"
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
        Self::fixture_profile(Self::default_company_number(), "EXAMPLE CORP LTD", "2001-01-01")
    }

    /// The company number of the sample company (the company used by
    /// [`Self::sample_tax`]).
    pub fn sample_company_number() -> &'static str {
        "9876543"
    }

    /// The company profile of the sample company (company number
    /// [`Self::sample_company_number`]), the same company as
    /// [`Self::sample_tax`].
    pub fn sample_company() -> CompanyProfile {
        Self::fixture_profile(Self::sample_company_number(), "Acme Ltd", "2020-01-01")
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

    /// A sample FRS 105 tax computation for a fictional company (Acme Ltd,
    /// company number `9876543`, period 2025), including the numeric facts
    /// (profits, tax rates, allowances, R&D) so the derived form values are
    /// fully populated.
    pub fn sample_tax() -> Frs105CorpTax {
        let mut facts = crate::ixbrl_fmt::ParsedIxBrlFacts::default();
        facts
            .non_numeric
            .insert("ct-comp:CompanyName".to_string(), "Acme Ltd".to_string());
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
        for (name, ctx, v) in [
            ("ct-comp:NetTradingProfits", "ctxt-3", 12345.0),
            ("ct-comp:FY1AmountOfProfitChargeableAtFirstRate", "ctxt-3", 6000.0),
            ("ct-comp:FY2AmountOfProfitChargeableAtFirstRate", "ctxt-3", 6345.0),
            ("ct-comp:FY1FirstRateOfTax", "ctxt-1", 19.0),
            ("ct-comp:FY2FirstRateOfTax", "ctxt-1", 19.0),
            ("ct-comp:FY1TaxAtFirstRate", "ctxt-3", 1140.0),
            ("ct-comp:FY2TaxAtFirstRate", "ctxt-3", 1205.55),
            ("ct-comp:CorporationTaxChargeable", "ctxt-3", 2345.55),
            ("ct-comp:TaxChargeable", "ctxt-3", 2345.55),
            ("ct-comp:TaxPayable", "ctxt-3", 2345.55),
            ("ct-comp:MainPoolAnnualInvestmentAllowance", "ctxt-2", 1000.0),
            (
                "ct-comp:AdjustmentsAdditionalDeductionForQualifyingRDExpenditureSME",
                "ctxt-4",
                5000.0,
            ),
        ] {
            facts
                .numeric_by_ctx
                .insert((name.to_string(), ctx.to_string()), v);
        }

        // The company is built from [`Self::sample_company`] so the profile
        // served for it (by `tax.company_number()`) can never disagree with
        // the tax computation.
        let sample = Self::sample_company();
        let company = crate::company::Company::new(
            &sample.company_name,
            "1234567890",
            &sample.company_number,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        Frs105CorpTax::from_parsed_facts(&facts, &company)
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
    /// `TestClient` (the company `TestData::sample_tax` is built for).
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
        let client =
            CachedCompaniesHouseClient::with_cache_dir(CompaniesHouseClient::offline(), dir.path());

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
