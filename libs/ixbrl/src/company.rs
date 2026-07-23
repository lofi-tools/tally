use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct Company {
    pub name: String,
    pub tax_reference: String,
    pub company_number: String,
    pub accounting_period_start: NaiveDate,
    pub accounting_period_end: NaiveDate,
    pub fy1_year: i32,
    pub fy2_year: i32,
    pub fy1_rate: f64,
    pub fy2_rate: f64,
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
}
