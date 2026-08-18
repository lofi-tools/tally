//! Shared test fixtures and repo-root resolution.
//!
//! [`REPO`] / [`repo_path`] resolve the repository root (via `git
//! rev-parse`) so tests can read committed fixtures under `example_data/`
//! and write to `.cache` from any working directory.  [`Fixtures`] holds the
//! hardcoded fictional company fixtures shared across the workspace's test
//! suites: the example company (`12345678`, "Example Biz Ltd.", the
//! `example_data/basic-1` book) and the sample company (`9876543`, "Acme
//! Ltd.") the CT600 return tests are built on.
//!
//! This crate is a leaf: it depends only on `core_model` (and chrono), so
//! any crate's test suite can depend on it without creating dependency
//! cycles.

use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;

use core_model::{AccountingPeriod, AccountsMeta, Company};

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

/// A repo-root-relative path resolved to a string (the GnuCash loader and
/// the file readers take `&str`), so tests run from any working directory.
pub fn repo_path(rel: &str) -> String {
    REPO.join(rel).to_string_lossy().into_owned()
}

/// The repo's `.cache` directory (the scratch area test suites write to).
pub fn cache_root() -> PathBuf {
    REPO.join(".cache")
}

/// The repo's `.cache/<sub>/` directory for a named test-suite scratch area.
pub fn cache_dir(sub: &str) -> PathBuf {
    cache_root().join(sub)
}

/// A path under the repo's `.cache/<sub>/` directory.
pub fn cache_path(sub: &str, file: &str) -> PathBuf {
    cache_dir(sub).join(file)
}

/// Hardcoded test data: the fictional example company and the example
/// GnuCash accounts files, so tests run with zero configuration on a fresh
/// checkout.
pub struct Fixtures;

impl Fixtures {
    /// The company number of the fictional default test company.
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

    /// The example GnuCash accounts book used by most test suites
    /// (`example_data/basic-1/input.gnucash`), resolved from the repository
    /// root.
    pub fn basic_book() -> PathBuf {
        REPO.join("example_data/basic-1/input.gnucash")
    }

    /// The path to the example GnuCash accounts file for a company number,
    /// or `None` for unknown companies.  Paths are resolved from the
    /// repository root, so tests run from any working directory.
    ///
    /// Only `example_data/basic-1/input.gnucash` is tied to a company
    /// number in the tests; `example_data/basic-2/input.gnucash` is used
    /// solely to exercise XML-format parsing, so it has no map entry.
    pub fn accounts_path(company_number: &str) -> Option<String> {
        match company_number {
            "12345678" => Some(Self::basic_book().to_string_lossy().into_owned()),
            _ => None,
        }
    }

    /// The company number of the sample company ("Acme Ltd.", the company
    /// the CT600 sample tax computation is built on).
    pub fn sample_company_number() -> &'static str {
        "9876543"
    }

    /// The sample company's set of accounts: the 2026 return period and
    /// the default financial-year parameters — the `accounts` the sample
    /// tax computation is built on.
    pub fn sample_accounts_meta() -> AccountsMeta {
        AccountsMeta {
            period: Some(AccountingPeriod {
                start: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end: chrono::NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            }),
            ..AccountsMeta::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_company_matches_fixture() {
        let company = Fixtures::default_company();
        assert_eq!(company.name, "Example Biz Ltd.");
        assert_eq!(company.tax_reference, "8596148860");
        assert_eq!(company.company_number, Fixtures::default_company_number());
        assert_eq!(
            company.registration_date,
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()
        );
        let accounts = Fixtures::default_accounts_meta();
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
            Fixtures::accounts_path(Fixtures::default_company_number()),
            Some(repo_path("example_data/basic-1/input.gnucash"))
        );
        assert_eq!(Fixtures::accounts_path("0"), None);
    }

    #[test]
    fn cache_helpers_land_under_dot_cache() {
        assert_eq!(cache_root(), REPO.join(".cache"));
        assert_eq!(cache_dir("api_responses"), REPO.join(".cache/api_responses"));
        assert_eq!(
            cache_path("ixbrl-rs-tests", "out.html"),
            REPO.join(".cache/ixbrl-rs-tests/out.html")
        );
        assert_eq!(Fixtures::basic_book(), REPO.join("example_data/basic-1/input.gnucash"));
    }

    #[test]
    fn sample_company_matches_fixture() {
        assert_eq!(Fixtures::sample_company_number(), "9876543");
        assert_eq!(
            Fixtures::sample_accounts_meta().period().start,
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
        );
        assert_eq!(
            Fixtures::sample_accounts_meta().period().end,
            chrono::NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()
        );
    }
}
