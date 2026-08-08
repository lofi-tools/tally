#![cfg(test)]
//! Test utilities for the ixbrl crate.
//!
//! Hosts the hardcoded default fixtures: the fictional example company
//! ([`TestData::default_company`]), its accounts
//! ([`TestData::default_accounts_meta`]) and a map from company number to
//! the example GnuCash accounts file under `example_data/`
//! ([`TestData::accounts_path`]).  Tests run with zero configuration on a
//! fresh checkout.

use crate::company::{AccountingPeriod, AccountsMeta, Company};

/// Hardcoded test data: the fictional example company and the example
/// GnuCash accounts files, so tests run with zero configuration on a fresh
/// checkout.
pub struct TestData;

impl TestData {
    /// The company number of the fictional test company.
    pub fn default_company_number() -> &'static str {
        "12345678"
    }

    /// The fictional example company: "Example Biz Ltd." with tax reference
    /// `8596148860` and company number [`Self::default_company_number`].
    pub fn default_company() -> Company {
        let mut company = Company::new(
            "Example Biz Ltd.",
            "8596148860",
            Self::default_company_number(),
        );
        company.registration_date = chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        company
    }

    /// The fictional example company's accounts: the 2020 calendar-year
    /// return period and the default financial-year tax parameters (2019 /
    /// 2020 at 19%).
    pub fn default_accounts_meta() -> AccountsMeta {
        AccountsMeta {
            period: Some(AccountingPeriod {
                start: chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                end: chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
            }),
            ..AccountsMeta::default()
        }
    }

    /// The path to the example GnuCash accounts file for a company number,
    /// or `None` for unknown companies.  Paths are relative to the crate
    /// directory (Cargo runs tests from the package root).
    ///
    /// Only `example_data/basic-1/input.gnucash` is tied to a company
    /// number in the tests; `example_data/basic-2/input.gnucash` is used
    /// solely to exercise XML-format parsing, so it has no map entry.
    pub fn accounts_path(company_number: &str) -> Option<&'static str> {
        match company_number {
            "12345678" => Some("example_data/basic-1/input.gnucash"),
            _ => None,
        }
    }
}

mod tests {
    use super::*;

    #[test]
    fn default_company_matches_fixture() {
        let company = TestData::default_company();
        assert_eq!(company.name, "Example Biz Ltd.");
        assert_eq!(company.tax_reference, "8596148860");
        assert_eq!(company.company_number, TestData::default_company_number());
        assert_eq!(
            company.registration_date,
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()
        );
        let accounts = TestData::default_accounts_meta();
        assert_eq!(
            accounts.period().start,
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()
        );
        assert_eq!(
            accounts.period().end,
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()
        );
        assert_eq!(accounts.fy1_year, 2019);
        assert_eq!(accounts.fy2_year, 2020);
    }

    #[test]
    fn accounts_path_maps_example_company() {
        assert_eq!(
            TestData::accounts_path(TestData::default_company_number()),
            Some("example_data/basic-1/input.gnucash")
        );
        assert_eq!(TestData::accounts_path("0"), None);
    }
}
