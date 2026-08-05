use chrono::{Datelike, Months, NaiveDate};
use serde::{Deserialize, Serialize};

/// The company identity: who the accounts belong to.
///
/// The return period and the financial-year tax parameters live in
/// [`AccountsMeta`], not here — a company is a company whatever set of
/// accounts is being produced.
#[derive(Debug, Clone, Default)]
pub struct Company {
    pub name: String,
    pub tax_reference: String,
    pub company_number: String,
    /// The date of incorporation/registration, resolved from Companies
    /// House when the config carries no identity details.  Defaults to the
    /// Unix epoch when unknown; the registration-date accounting schedule
    /// ([`Self::first_ard`], [`Self::accounting_period_n`]) is only
    /// meaningful once it is set.
    pub registration_date: NaiveDate,
}

/// A period: `start` through `end` (inclusive).
///
/// Used both for the registration-date accounting schedule
/// ([`Company::accounting_period_n`]) and, nested in [`AccountsMeta`], for
/// the return period the accounts cover.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct AccountingPeriod {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl AccountingPeriod {
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }

    /// The corresponding period one year earlier (a crude 365-day shift).
    pub fn previous(self) -> AccountingPeriod {
        AccountingPeriod {
            start: self.start - chrono::Duration::days(365),
            end: self.end - chrono::Duration::days(365),
        }
    }
}

/// The `accounts` sub-object of the config file (`accounts.*`).
///
/// A set of accounts: the return period ([`AccountingPeriod`]) plus the
/// financial-year tax parameters (fy1/fy2 years and rates) the computation
/// runs on.  The period is optional here because it can be resolved — from
/// [`Self::accounts_made_up_to`] (the 12 months ending on that date), or
/// from the company's next accounting period to file at Companies House —
/// before the reports are built; the fy fields default to 2019/2020 at 19%.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AccountsMeta {
    /// The return period (`accounts.period.start` / `accounts.period.end`).
    #[serde(default)]
    pub period: Option<AccountingPeriod>,
    /// A date at which the accounts are made: the return period is deduced
    /// as the 12 months ending on this date when no `period` is given.
    #[serde(default)]
    pub accounts_made_up_to: Option<NaiveDate>,
    #[serde(default = "default_fy1_year")]
    pub fy1_year: i32,
    #[serde(default = "default_fy2_year")]
    pub fy2_year: i32,
    #[serde(default = "default_fy1_rate")]
    pub fy1_rate: f64,
    #[serde(default = "default_fy2_rate")]
    pub fy2_rate: f64,
}

impl Default for AccountsMeta {
    /// The default set of accounts: no period, and the default financial-year
    /// tax parameters (fy1 2019, fy2 2020, both 19%).  Matches the serde
    /// defaults, so a config without the `accounts` sub-object behaves like
    /// one with it.
    fn default() -> Self {
        Self {
            period: None,
            accounts_made_up_to: None,
            fy1_year: DEFAULT_FY1_YEAR,
            fy2_year: DEFAULT_FY2_YEAR,
            fy1_rate: DEFAULT_FY1_RATE,
            fy2_rate: DEFAULT_FY2_RATE,
        }
    }
}

const DEFAULT_FY1_YEAR: i32 = 2019;
const DEFAULT_FY2_YEAR: i32 = 2020;
const DEFAULT_FY1_RATE: f64 = 19.0;
const DEFAULT_FY2_RATE: f64 = 19.0;

fn default_fy1_year() -> i32 {
    DEFAULT_FY1_YEAR
}

fn default_fy2_year() -> i32 {
    DEFAULT_FY2_YEAR
}

fn default_fy1_rate() -> f64 {
    DEFAULT_FY1_RATE
}

fn default_fy2_rate() -> f64 {
    DEFAULT_FY2_RATE
}

impl AccountsMeta {
    /// The default financial-year tax parameters (fy1 2019, fy2 2020, both
    /// 19%), with no period.
    pub fn defaults() -> Self {
        Self::default()
    }

    /// The return period as given, falling back on the period deduced from
    /// [`Self::accounts_made_up_to`] (the 12 months ending on it).
    pub fn resolved_period(&self) -> Option<AccountingPeriod> {
        match self.period {
            Some(period) => Some(period),
            None => self.accounts_made_up_to.map(|end| AccountingPeriod {
                start: end - Months::new(12) + chrono::Duration::days(1),
                end,
            }),
        }
    }

    /// The resolved return period, filled in by resolution (config →
    /// made-up-to → Companies House) before the reports are built.
    ///
    /// Panics when absent: the reports only ever receive a resolved set of
    /// accounts.
    pub fn period(&self) -> AccountingPeriod {
        self.period
            .expect("the accounts period is resolved before the reports are built")
    }
}

impl Company {
    /// A company with only its identity: the registration date is left at
    /// its default and is filled in by resolution (Companies House) when it
    /// matters.
    pub fn new(
        name: impl Into<String>,
        tax_reference: impl Into<String>,
        company_number: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            tax_reference: tax_reference.into(),
            company_number: company_number.into(),
            registration_date: NaiveDate::default(),
        }
    }

    /// The first accounting reference date (ARD): the last day of the month
    /// following the first anniversary of registration.
    pub fn first_ard(&self) -> NaiveDate {
        let anniversary = self.registration_date + Months::new(12);
        let month = anniversary.month();
        let year = anniversary.year();
        let (y, m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
        NaiveDate::from_ymd_opt(y, m, 1).unwrap() - chrono::Duration::days(1)
    }

    /// Accounting period n:
    /// n=0: first CT period (registration to registration+12mo-1day, max 12mo)
    /// n=1: short second period (registration+12mo to first_ard, if non-empty)
    /// n>=2: subsequent full years starting from first_ard+1day
    pub fn accounting_period_n(&self, n: u32) -> AccountingPeriod {
        let reg = self.registration_date;
        let first_ard = self.first_ard();
        let first_year_end = reg + Months::new(12) - chrono::Duration::days(1);

        match n {
            0 => AccountingPeriod { start: reg, end: first_year_end },
            1 => AccountingPeriod { start: first_year_end + chrono::Duration::days(1), end: first_ard },
            _ => {
                let start = first_ard + chrono::Duration::days(1) + Months::new((n - 2) * 12);
                let end = start + Months::new(12) - chrono::Duration::days(1);
                AccountingPeriod { start, end }
            }
        }
    }

    pub fn accounting_period_containing(&self, date: NaiveDate) -> AccountingPeriod {
        let p0 = self.accounting_period_n(0);
        if p0.contains(date) {
            return p0;
        }

        let p1 = self.accounting_period_n(1);
        if p1.contains(date) {
            return p1;
        }

        let first_ard = self.first_ard();
        if date <= first_ard {
            return p1;
        }

        let years_since_ard = {
            let ard_ym = first_ard.year() * 12 + first_ard.month() as i32;
            let date_ym = date.year() * 12 + date.month() as i32;
            ((date_ym - ard_ym) / 12) as u32
        };
        self.accounting_period_n(years_since_ard + 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_company_new() {
        let c = Company::new("Acme Ltd", "1234567890", "9876543");
        assert_eq!(c.name, "Acme Ltd");
        assert_eq!(c.tax_reference, "1234567890");
        assert_eq!(c.company_number, "9876543");
    }

    #[test]
    fn test_accounts_meta_defaults() {
        let accounts = AccountsMeta::defaults();
        assert_eq!(accounts.fy1_year, 2019);
        assert_eq!(accounts.fy2_year, 2020);
        assert_eq!(accounts.fy1_rate, 19.0);
        assert_eq!(accounts.fy2_rate, 19.0);
        assert_eq!(accounts.period, None);
        assert_eq!(accounts.resolved_period(), None);
    }

    #[test]
    fn test_accounts_meta_made_up_to_deduces_period() {
        let accounts = AccountsMeta {
            accounts_made_up_to: Some(NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()),
            ..AccountsMeta::default()
        };
        let period = accounts.resolved_period().expect("deduced from made-up-to");
        assert_eq!(period.start, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());
        assert_eq!(period.end, NaiveDate::from_ymd_opt(2020, 12, 31).unwrap());
    }

    #[test]
    fn test_first_ard() {
        let cases = [
            (2015, 1, 7, 2016, 1, 31),
            (2015, 6, 23, 2016, 6, 30),
            (2015, 12, 1, 2016, 12, 31),
        ];
        for (ry, rm, rd, ay, am, ad) in cases {
            let mut c = Company::new("Co", "tax", "num");
            c.registration_date = NaiveDate::from_ymd_opt(ry, rm, rd).unwrap();
            let expected = NaiveDate::from_ymd_opt(ay, am, ad).unwrap();
            assert_eq!(c.first_ard(), expected,
                "incorp {ry}-{rm:02}-{rd:02} should give ARD {ay}-{am:02}-{ad:02}");
        }
    }

    #[test]
    fn test_accounting_periods_govuk_example() {
        // gov.uk example: incorporated 11 May 2024
        // First ARD: 31 May 2025
        // CT Period 0: 11 May 2024 - 10 May 2025 (12 months)
        // CT Period 1: 11 May 2025 - 31 May 2025 (short, ~21 days)
        let mut c = Company::new("Co", "tax", "num");
        c.registration_date = NaiveDate::from_ymd_opt(2024, 5, 11).unwrap();

        assert_eq!(c.first_ard(), NaiveDate::from_ymd_opt(2025, 5, 31).unwrap());

        let p0 = c.accounting_period_n(0);
        assert_eq!(p0.start, NaiveDate::from_ymd_opt(2024, 5, 11).unwrap());
        assert_eq!(p0.end, NaiveDate::from_ymd_opt(2025, 5, 10).unwrap());

        let p1 = c.accounting_period_n(1);
        assert_eq!(p1.start, NaiveDate::from_ymd_opt(2025, 5, 11).unwrap());
        assert_eq!(p1.end, NaiveDate::from_ymd_opt(2025, 5, 31).unwrap());

        let p2 = c.accounting_period_n(2);
        assert_eq!(p2.start, NaiveDate::from_ymd_opt(2025, 6, 1).unwrap());
        assert_eq!(p2.end, NaiveDate::from_ymd_opt(2026, 5, 31).unwrap());
    }

    #[test]
    fn test_accounting_period_containing() {
        let mut c = Company::new("Co", "tax", "num");
        c.registration_date = NaiveDate::from_ymd_opt(2024, 5, 11).unwrap();

        let p = c.accounting_period_containing(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap());
        assert_eq!(p, c.accounting_period_n(0));

        let p = c.accounting_period_containing(NaiveDate::from_ymd_opt(2025, 5, 15).unwrap());
        assert_eq!(p, c.accounting_period_n(1));

        let p = c.accounting_period_containing(NaiveDate::from_ymd_opt(2025, 12, 25).unwrap());
        assert_eq!(p, c.accounting_period_n(2));
    }
}
