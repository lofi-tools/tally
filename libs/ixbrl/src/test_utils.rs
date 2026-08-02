#![cfg(test)]
//! Test utilities for the ixbrl crate.
//!
//! Hosts the hardcoded default fixtures: the fictional example company
//! ([`TestData::default_company`]) and a map from company number to the
//! example GnuCash accounts file under `example_data/`
//! ([`TestData::accounts_path`]).  Tests run with zero configuration on a
//! fresh checkout.

use crate::company::Company;

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
    /// `8596148860` and accounting period 1 Jan - 31 Dec 2020 (company
    /// number [`Self::default_company_number`]).
    pub fn default_company() -> Company {
        Company::new(
            "Example Biz Ltd.",
            "8596148860",
            Self::default_company_number(),
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap(),
        )
    }

    /// The path to the example GnuCash accounts file for a company number,
    /// or `None` for unknown companies.  Paths are relative to the crate
    /// directory (Cargo runs tests from the package root).
    ///
    /// Only `example_data/example2/input.gnucash` is tied to a company
    /// number in the tests; `example_data/example1/example.gnucash` is used
    /// solely to exercise XML-format parsing, so it has no map entry.
    pub fn accounts_path(company_number: &str) -> Option<&'static str> {
        match company_number {
            "12345678" => Some("example_data/example2/input.gnucash"),
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
            company.accounting_period_start,
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()
        );
        assert_eq!(
            company.accounting_period_end,
            chrono::NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()
        );
    }

    #[test]
    fn accounts_path_maps_example_company() {
        assert_eq!(
            TestData::accounts_path(TestData::default_company_number()),
            Some("example_data/example2/input.gnucash")
        );
        assert_eq!(TestData::accounts_path("0"), None);
    }
}
