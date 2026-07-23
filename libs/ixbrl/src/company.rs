use chrono::{Datelike, Months, NaiveDate};

#[derive(Debug, Clone)]
pub struct Company {
    pub name: String,
    pub tax_reference: String,
    pub company_number: String,
    pub registration_date: NaiveDate,
    pub accounting_period_start: NaiveDate,
    pub accounting_period_end: NaiveDate,
    pub fy1_year: i32,
    pub fy2_year: i32,
    pub fy1_rate: f64,
    pub fy2_rate: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountingPeriod {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl AccountingPeriod {
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }
}

impl Company {
    pub fn new(
        name: impl Into<String>,
        tax_reference: impl Into<String>,
        company_number: impl Into<String>,
        accounting_period_start: NaiveDate,
        accounting_period_end: NaiveDate,
    ) -> Self {
        Self {
            name: name.into(),
            tax_reference: tax_reference.into(),
            company_number: company_number.into(),
            registration_date: accounting_period_start,
            accounting_period_start,
            accounting_period_end,
            fy1_year: 2019,
            fy2_year: 2020,
            fy1_rate: 19.0,
            fy2_rate: 19.0,
        }
    }

    pub fn return_period_start(&self) -> NaiveDate {
        self.accounting_period_start
    }

    pub fn return_period_end(&self) -> NaiveDate {
        self.accounting_period_end
    }

    pub fn prev_period_start(&self) -> NaiveDate {
        self.accounting_period_start - chrono::Duration::days(365)
    }

    pub fn prev_period_end(&self) -> NaiveDate {
        self.accounting_period_end - chrono::Duration::days(365)
    }

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
        let c = Company::new(
            "Acme Ltd",
            "1234567890",
            "09876543",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        assert_eq!(c.name, "Acme Ltd");
        assert_eq!(c.fy1_year, 2019);
        assert_eq!(c.fy1_rate, 19.0);
        assert_eq!(c.prev_period_start(), NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
    }

    #[test]
    fn test_first_ard() {
        let cases = [
            (2015, 1, 7, 2016, 1, 31),
            (2015, 6, 23, 2016, 6, 30),
            (2015, 12, 1, 2016, 12, 31),
        ];
        for (ry, rm, rd, ay, am, ad) in cases {
            let c = Company::new("Co", "tax", "num",
                NaiveDate::from_ymd_opt(ry, rm, rd).unwrap(),
                NaiveDate::from_ymd_opt(ry, rm, rd).unwrap(),
            );
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
        let c = Company::new("Co", "tax", "num",
            NaiveDate::from_ymd_opt(2024, 5, 11).unwrap(),
            NaiveDate::from_ymd_opt(2024, 5, 11).unwrap(),
        );

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
        let c = Company::new("Co", "tax", "num",
            NaiveDate::from_ymd_opt(2024, 5, 11).unwrap(),
            NaiveDate::from_ymd_opt(2024, 5, 11).unwrap(),
        );

        let p = c.accounting_period_containing(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap());
        assert_eq!(p, c.accounting_period_n(0));

        let p = c.accounting_period_containing(NaiveDate::from_ymd_opt(2025, 5, 15).unwrap());
        assert_eq!(p, c.accounting_period_n(1));

        let p = c.accounting_period_containing(NaiveDate::from_ymd_opt(2025, 12, 25).unwrap());
        assert_eq!(p, c.accounting_period_n(2));
    }
}
