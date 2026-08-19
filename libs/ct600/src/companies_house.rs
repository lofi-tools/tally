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
    OfficerChangeFiling, OfficerList, OtherFiling, PreviousPeriodFigures, RegisteredOfficeAddress,
    TypedFiling, next_accounting_period_from, parse_filed_accounts,
};

use crate::form::CompanyFormValues;
use reports::reports::uk_frs105_corp_tax::Frs105CorpTax;

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

    use companies_house::PreviousPeriodFigures;

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
            crate::test_utils::cache_dir("api_responses")
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
    fn print_figures(figures: &PreviousPeriodFigures) {
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

    /// The full-year pipeline for a real company (default `14510633`):
    /// fetch the filing history, take the latest accounts filing as the
    /// starting point, then generate all three output documents for the
    /// next pending period — the FRS 105 accounts, the FRS 105
    /// corporation-tax computation and the CT600 return — with an empty
    /// transaction list (the pending period has no ledger yet).  The
    /// previous-period comparative column is computed from a chart of
    /// accounts auto-generated *plausibly* from the filed balance sheet
    /// ([`crate::test_utils::plausible_previous_book`]), which must
    /// reconcile with the filing ([`Frs105Accounts::check_previous_period_matches_filing`]);
    /// with no current-period transactions the seeded current column
    /// ([`Frs105Accounts::with_prev_period_data`]) equals the previous
    /// column, so every balance reads the same in both columns.  The
    /// documents are written under `.cache`, like the other live tests'
    /// artifacts.
    #[tokio::test]
    #[cfg_attr(
        not(any(feature = "cached_live_tests", feature = "always_live_tests")),
        ignore = "requires a Companies House API key for a cold cache"
    )]
    async fn live_latest_accounts_full_year_reports() {
        use chrono::Datelike;
        use reports::GnucashBook;
        use reports::reports::uk_frs105_accounts::{Frs105Accounts, PreviousPeriodData};
        use reports::{AccountsMeta, Company, CompanyProfile};

        use crate::ct600_return::Ct600Return;

        let number = std::env::var("COMPANY_NUMBER").unwrap_or_else(|_| "14510633".to_string());
        let client = live_client();

        // The latest accounts filing: history → the accounts item covering
        // the latest period → its document downloaded (cache-first) and
        // parsed into a balance sheet.
        let filing = client
            .latest_accounts_filing(&number)
            .await
            .expect("fetch the latest accounts filing")
            .expect("the company has an accounts filing");
        let bs = filing.balance_sheet;
        println!(
            "latest accounts filing (filed {}): period {} → {}",
            filing.filed_on, bs.period_start, bs.period_end
        );
        print_figures(&bs.figures);

        // The company: registration date from the profile, so the
        // accounting-period schedule is the real one.
        let ch_profile = client
            .get_company_profile(&number)
            .await
            .expect("fetch the company profile");
        let mut company = Company::new(
            &ch_profile.company_name,
            "1234567890", // the CH profile carries no tax reference; placeholder for the artifact
            &ch_profile.company_number,
        );
        company.registration_date = ch_profile
            .date_of_creation
            .as_deref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .expect("the profile carries the registration date");

        // The reports' company profile: the CH API model has no
        // contact / accountant / auditor fields (those are config-only), so
        // the report profile carries what the API provides (address, SIC,
        // jurisdiction) and defaults the rest.
        let profile = CompanyProfile {
            address_lines: ch_profile
                .registered_office_address
                .as_ref()
                .map(|a| {
                    [a.address_line_1.as_deref(), a.address_line_2.as_deref()]
                        .into_iter()
                        .flatten()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            county: ch_profile
                .registered_office_address
                .as_ref()
                .and_then(|a| a.region.clone())
                .filter(|s| !s.is_empty()),
            location: ch_profile
                .registered_office_address
                .as_ref()
                .and_then(|a| a.locality.clone())
                .unwrap_or_default(),
            postcode: ch_profile
                .registered_office_address
                .as_ref()
                .and_then(|a| a.postal_code.clone())
                .unwrap_or_default(),
            sic_codes: ch_profile.sic_codes.clone().unwrap_or_default(),
            jurisdiction: ch_profile.jurisdiction.clone().unwrap_or_default(),
            ..Default::default()
        };

        // The pending period: the accounting period after the filed one.
        let pending = company.accounting_period_containing(
            bs.period_end
                .succ_opt()
                .expect("the filed period end has a successor"),
        );
        println!("pending period: {} → {}", pending.start, pending.end);
        let accounts_meta = AccountsMeta {
            period: Some(pending),
            fy1_year: pending.end.year() - 1,
            fy2_year: pending.end.year(),
            ..AccountsMeta::default()
        };

        // Empty ledger: no transactions in the pending period yet — the new
        // balance sheet carries the filed figures in the previous column.
        let book = GnucashBook::from_raw_parts(Vec::new(), Vec::new(), Vec::new());

        // The previous period's chart of accounts.  The filing alone cannot
        // rebuild the CoA (it carries only the aggregates), so the test
        // auto-generates a *plausible* one — a book that reproduces the
        // filed figures when run through the builder's computations (see
        // `test_utils::plausible_previous_book`) — and attaches the filing
        // for the check.
        let previous_book = crate::test_utils::plausible_previous_book(&bs);
        let prev = PreviousPeriodData {
            book: &previous_book,
            filing: Some(&bs),
        };

        // The three documents: the previous-period comparative column is
        // computed from the previous CoA, and the pending period has no
        // transactions, so the seeded current column (previous-period
        // figures + zero activity) equals the comparative column — the
        // balances are unchanged (see `Frs105Accounts::with_prev_period_data`).
        let accounts = Frs105Accounts::new(&book, &company, &profile, &accounts_meta)
            .with_prev_period_data(&prev);
        // The generated previous CoA reconciles with the filed balance
        // sheet line for line (this is what the check validates).
        accounts
            .check_previous_period_matches_filing(&bs)
            .expect("the plausible previous-period CoA must reconcile with the filed balance sheet");
        assert_eq!(
            accounts.net_assets[0], accounts.net_assets[1],
            "no transactions: net assets carried forward"
        );
        assert_eq!(
            accounts.fixed_assets[0], accounts.fixed_assets[1],
            "no transactions: fixed assets carried forward"
        );
        assert_eq!(
            accounts.current_assets[0], accounts.current_assets[1],
            "no transactions: current assets carried forward"
        );
        let corp_tax = Frs105CorpTax::builder(&book, &company, &accounts_meta).build();
        let ct600 = Ct600Return::from_inputs(&accounts, &corp_tax);

        let accounts_html = accounts.to_ixbrl();
        let corp_tax_html = corp_tax.to_ixbrl();
        let ct600_xml = ct600.to_xml();

        // Test artifacts: persisted under `.cache`, like the other live
        // tests.
        let dir = crate::test_utils::cache_dir("tally-full-year");
        std::fs::create_dir_all(&dir).expect("create the cache dir");
        for (name, doc) in [
            (format!("{number}-accounts.html"), accounts_html.as_str()),
            (format!("{number}-corp-tax.html"), corp_tax_html.as_str()),
            (format!("{number}-ct600.xml"), ct600_xml.as_str()),
        ] {
            let path = dir.join(&name);
            std::fs::write(&path, doc).expect("write the generated document");
            println!("wrote {}", path.display());
        }

        // The generated documents carry their expected markers.
        assert!(accounts_html.contains("Unaudited Micro-Entity Accounts"));
        // No logo or signature is supplied in this test — the report omits
        // both images instead of emitting empty data URIs.
        assert!(!accounts_html.contains("alt=\"Company logo\""));
        assert!(!accounts_html.contains("alt=\"Director's signature\""));
        assert!(corp_tax_html.contains("Corporation Tax Statement"));
        assert!(ct600_xml.contains("GovTalkMessage"));
        assert!(ct600_xml.contains(&ch_profile.company_number));
    }
}
