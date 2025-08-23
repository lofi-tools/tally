use crate::adapters::exchange_rates::TimeRange;
use crate::utils::DatetimeUtcExt;
use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, Utc};

pub struct Company {
    pub registration_date: NaiveDate,
}
impl Company {
    pub fn last_accounting_period(&self) -> anyhow::Result<TimeRange> {
        let now = Utc::now();

        let birthday_this_year = self.registration_date.with_year(now.year()).unwrap();
        let go_back_n_years = match (birthday_this_year - Months::new(3)) > now.date_naive() {
            true => 2,
            false => 1,
        };

        let period_start = first_of_next_month(birthday_this_year)
            .with_year(now.year() - go_back_n_years)
            .unwrap();
        let period_end =
            last_of_prev_month(period_start.checked_add_months(Months::new(12)).unwrap());

        Ok(TimeRange {
            start: DateTime::from_naive_date(period_start),
            end: DateTime::from_naive_date(period_end),
        })
    }
}

// pub fn last_of_month(any_day_of_month: NaiveDate) -> NaiveDate {
//     let first_of_month = any_day_of_month.with_day(1).unwrap();
//     let first_of_next_month = (first_of_month + Duration::weeks(6)).with_day(1).unwrap();
//     let last_of_month = first_of_next_month - Duration::days(1);
//     last_of_month
// }

pub fn first_of_next_month(any_day_of_month: NaiveDate) -> NaiveDate {
    let first_of_month = any_day_of_month.with_day(1).unwrap();
    let first_of_next_month = (first_of_month + Duration::weeks(6)).with_day(1).unwrap();
    first_of_next_month
}
pub fn last_of_prev_month(any_day_of_month: NaiveDate) -> NaiveDate {
    let first_of_month = any_day_of_month.with_day(1).unwrap();
    first_of_month - Duration::days(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_accounting_period() {
        let company = Company {
            registration_date: NaiveDate::from_ymd_opt(2022, 11, 28).unwrap(),
        };
        let period = company.last_accounting_period();
        dbg!(period);
    }
}
