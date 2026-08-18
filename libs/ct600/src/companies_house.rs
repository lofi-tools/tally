//! The Companies House client and the CT600 adapters over it.
//!
//! The client itself ([`CompaniesHouseClient`], its layered [`Config`], the
//! company-resolution / next-accounting-period chain, the typed filings and
//! the iXBRL accounts parse) lives in the `companies-house` crate — this
//! module re-exports it.  On top of the client, this module adds the
//! CT600-specific derivation: the company header boxes enrichment
//! ([`CompaniesHouseFormValues`], built on [`crate::form::CompanyFormValues`]).
//!
//! The live API tests that exercise the CT600 adapters (part of the
//! default-enabled `cached_live_tests` feature) also live here.

pub use companies_house::{
    Accounts, AddressChangeFiling, ApiResult, ArdChangeFiling, ChFiling, CompaniesHouseClient,
    CompaniesHouseClientType, CompaniesHouseError, CompanyProfile, CompanyType, Config,
    ConfirmationStatement, ConfirmationStatementFiling, DocumentLinks, DocumentMetadata,
    FilingHistory, FilingHistoryItem, FilingHistoryLinks, FormType, IncorporationFiling,
    LastAccounts, NextAccounts, NextAccountingPeriod, Officer, OfficerChangeAction,
    OfficerChangeFiling, OfficerList, OtherFiling, PreviousYearFigures, RegisteredOfficeAddress,
    TypedFiling, next_accounting_period_from, parse_filed_accounts,
};

use crate::form::CompanyFormValues;
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;

/// CT600 company-header enrichment over the Companies House client.
///
/// The Companies House lookup is skipped when the caller supplied complete
/// company details (a non-empty name and registration number): the header
/// boxes are then derived from the tax computation alone.  Otherwise — when
/// a company number is configured (the `COMPANY_NUMBER` environment variable)
/// and the company details are absent — the profile is fetched (cache-first,
/// see [`CompaniesHouseClient::get_company_profile_cached`]) and boxes 1
/// (name), 2 (registration number) and 4 (type of company) are enriched from
/// it.  The tax reference (3) and the return period (30/35) always come from
/// the tax computation.
pub trait CompaniesHouseFormValues {
    /// The company header boxes for the tax computation, enriched from
    /// Companies House when the caller's company details are absent.
    fn company_form_values(
        &self,
        tax: &Frs105CorpTax,
    ) -> impl Future<Output = ApiResult<CompanyFormValues>>;
}

impl CompaniesHouseFormValues for CompaniesHouseClient {
    async fn company_form_values(&self, tax: &Frs105CorpTax) -> ApiResult<CompanyFormValues> {
        let Some(company_number) = self
            .config()
            .enrichment_number(tax.company_name(), tax.company_number())
        else {
            return Ok(CompanyFormValues::from_tax(tax));
        };
        let profile = self.get_company_profile_cached(&company_number).await?;
        Ok(CompanyFormValues::from_profile_and_tax(&profile, tax))
    }
}

// ============================================================================
// Live API tests (part of the default-enabled `cached_live_tests` feature)
// ============================================================================

/// Live Companies House API tests that exercise the CT600 adapters.
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

    use companies_house::PreviousYearFigures;

    /// The cache directory the live tests use: a scratch tempdir when
    /// `always_live_tests` is enabled, else the repository's
    /// `.cache/api_responses` (the `cached_live_tests` default).
    fn live_cache_dir() -> std::path::PathBuf {
        #[cfg(feature = "always_live_tests")]
        {
            tempfile::tempdir().unwrap().keep()
        }
        #[cfg(not(feature = "always_live_tests"))]
        {
            crate::test_utils::REPO.join(".cache/api_responses")
        }
    }

    /// A client for the live API, or the sandbox when only the sandbox key is
    /// set, pointed at the mode's cache directory (see [`live_cache_dir`]).
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

    /// The accounts filings of a real company (default `14510633`) carry the
    /// filed balance sheet: every accounts document is downloaded
    /// (cache-first) and parsed into a `Frs105Accounts`, printing the
    /// balance-sheet figures (current period | previous period) for each
    /// filed period. The filing type that holds the past balance sheet is
    /// the `accounts` category — the parse is what the tally-api sync
    /// persists into `balance_sheets`.
    #[tokio::test]
    #[cfg_attr(
        not(any(feature = "cached_live_tests", feature = "always_live_tests")),
        ignore = "requires a Companies House API key for a cold cache"
    )]
    async fn live_past_balance_sheets_print_figures() {
        let number = std::env::var("COMPANY_NUMBER").unwrap_or_else(|_| "14510633".to_string());
        let client = live_client();

        let history = client
            .get_filing_history(&number)
            .await
            .expect("fetch the filing history");

        let mut parsed = 0usize;
        for item in history.accounts() {
            let Some(tx) = item.transaction_id() else {
                continue;
            };
            let Some(filed_on) = item.filed_on() else {
                continue;
            };
            let bytes = client
                .download_filing(&number, &tx)
                .await
                .expect("download the accounts document");
            // CH serves these micro-entity accounts as raw iXBRL (XML), not
            // a zip.
            let Ok(html) = std::str::from_utf8(&bytes) else {
                println!(
                    "  filed {filed_on}  {tx}: not UTF-8 text ({} bytes) — skipping",
                    bytes.len()
                );
                continue;
            };
            // The two-pass parse: generic iXBRL → fact IR, then the
            // accounts-specific pass → the balance sheet (period recovered
            // from the document).
            match companies_house::parse_filed_accounts(html) {
                Ok(bs) => {
                    println!(
                        "balance sheet at {} (period {} → {}, filed {filed_on}):",
                        bs.period_end, bs.period_start, bs.period_end
                    );
                    print_figures(&bs.figures);
                    parsed += 1;
                }
                Err(e) => println!(
                    "  filed {filed_on}  {tx}: not a parseable accounts iXBRL document: {e}"
                ),
            }
        }
        assert!(
            parsed > 0,
            "at least one accounts filing parsed into a balance sheet"
        );
    }

    /// Print a filed balance sheet's line items (whole pounds; creditor
    /// lines negative — the reports' sign convention).
    fn print_figures(figures: &PreviousYearFigures) {
        for (label, value) in [
            (
                "called up share capital not paid",
                figures.called_up_share_capital_not_paid,
            ),
            ("fixed assets", figures.fixed_assets),
            ("current assets", figures.current_assets),
            (
                "prepayments and accrued income",
                figures.prepayments_and_accrued_income,
            ),
            ("creditors within 1 year", figures.creditors_within_1_year),
            ("net current assets", figures.net_current_assets),
            (
                "total assets less liabilities",
                figures.total_assets_less_liabilities,
            ),
            ("creditors after 1 year", figures.creditors_after_1_year),
            (
                "provisions for liabilities",
                figures.provisions_for_liabilities,
            ),
            (
                "accruals and deferred income",
                figures.accruals_and_deferred_income,
            ),
            ("net assets", figures.net_assets),
            ("capital and reserves", figures.capital_and_reserves),
        ] {
            println!("    {label:<32} {value:>12}");
        }
    }
}
