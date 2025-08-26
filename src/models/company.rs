use super::Asset2;
use super::sheet3::BalanceSheetBuilder;
use super::static_data::{AccountId, BALANCE_SHEET_2023_11_30};
use crate::adapters::exchange_rates::RatesApi;
use crate::adapters::exchange_rates::TimeRange;
use crate::adapters::starling_bank::StarlingClient;
use crate::utils::{DateExt, DatetimeUtcExt};
use crate::{Expenses, ListTxns};
use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, Utc};
use file_cache::{Cacheable, JsonFileBytes};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct Company {
    pub registration_date: NaiveDate,
}
impl Company {
    pub fn birthday_this_year(&self) -> anyhow::Result<NaiveDate> {
        let now = Utc::now();
        Ok(self.registration_date.with_year(now.year()).unwrap())
    }
    pub fn last_accounting_period_start(&self) -> anyhow::Result<NaiveDate> {
        let now = Utc::now();
        let birthday_this_year = self.birthday_this_year()?;
        let go_back_n_years = match (birthday_this_year - Months::new(3)) > now.date_naive() {
            true => 2,
            false => 1,
        };
        let period_start = first_of_next_month(birthday_this_year)
            .with_year(now.year() - go_back_n_years)
            .unwrap();
        Ok(period_start)
    }

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

    pub async fn balance_sheet_at(
        &self,
        at_date: NaiveDate,
        bank_api: &mut StarlingClient,
        rates_api: &impl RatesApi,
    ) -> anyhow::Result<BalanceSheetBuilder> {
        let accounting_period = self.last_accounting_period()?;

        let real_and_virtual_transactions = self
            .real_and_virtual_transactions(bank_api, rates_api)
            .await?
            .in_time_range(&accounting_period);

        let mut builder = BalanceSheetBuilder::new(at_date, &rates_api)
            .await?
            .with_transactions(&real_and_virtual_transactions);

        // if more than 1 year since company start, use prev balance sheet
        if accounting_period.end.date_naive() - self.registration_date > Duration::days(366) {
            builder = builder.with_prev_balance_sheet(self.prev_balance_sheet().await?.clone());
        };

        Ok(builder)
    }

    pub async fn prev_balance_sheet(&self) -> anyhow::Result<BalanceSheet4> {
        let cached = PastBalanceSheets::uniq_from_cache_or(|| async {
            Ok::<_, String>(PastBalanceSheets::default().with(BALANCE_SHEET_2023_11_30.clone()))
        })
        .await?;
        let last_accounting_period = self.last_accounting_period()?;

        let sheet = cached
            .sheets
            .get(&(last_accounting_period.start - Duration::days(1)).naive_date())
            .ok_or(anyhow::Error::msg("No balance sheet found"))?;
        Ok(sheet.clone())
    }

    pub async fn all_prev_balance_sheets() -> anyhow::Result<PastBalanceSheets> {
        let cached = PastBalanceSheets::uniq_from_cache_or(|| async {
            Ok::<_, String>(PastBalanceSheets::default().with(BALANCE_SHEET_2023_11_30.clone()))
        })
        .await?;
        Ok(cached)
    }

    pub async fn real_and_virtual_transactions(
        &self,
        bank_api: &mut StarlingClient,
        rates_api: impl RatesApi,
    ) -> anyhow::Result<ListTxns> {
        let mut transactions = bank_api.transactions().await?;
        transactions
            .add_directors_loan_repayments(&rates_api)
            .await?;
        transactions.add_expenses(Expenses::static_expenses_to_repay()?);

        Ok(transactions)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheet4 {
    pub account_balances: HashMap<AccountId, u64>,
    pub date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PastBalanceSheets {
    pub sheets: HashMap<NaiveDate, BalanceSheet4>,
}
impl PastBalanceSheets {
    pub fn get(&self, date: &NaiveDate) -> Option<&BalanceSheet4> {
        self.sheets.get(date)
    }
    pub fn insert(&mut self, sheet: BalanceSheet4) {
        self.sheets.insert(sheet.date, sheet);
    }
    pub fn with(mut self, sheet: BalanceSheet4) -> Self {
        self.insert(sheet);
        self
    }
    pub fn latest(&self) -> anyhow::Result<BalanceSheet4> {
        let latest = self
            .sheets
            .values()
            .max_by_key(|sheet| sheet.date)
            .cloned()
            .ok_or(anyhow::Error::msg("No balance sheet found"))?;

        // TODO check latest date is 1 day before

        Ok(latest)
    }
}
impl JsonFileBytes for PastBalanceSheets {}
impl Cacheable for PastBalanceSheets {}

pub struct MicroEntityBalanceSheetReport {
    pub date: NaiveDate,
    pub currency: Asset2,

    pub called_up_share_capital_not_paid: u64,
    pub total_fixed_assets: u64,
    pub total_current_assets: u64,
    pub prepayments_and_accrued_income: u64,
    pub creditors_amount_due_within_one_year: u64,
    /// negative means liabilities
    pub net_current_assets_or_liabilities: i64,
    pub total_assets_less_current_liabilities: i64,
    pub creditors_amount_due_after_more_than_one_year: u64,
    pub provision_for_liabilities: u64,
    pub accruals_and_deferred_income: u64,
    /// negative means liabilities
    pub total_net_assets_or_liabilities: i64,
    pub capital_and_reserves: i64,
    pub num_employees: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::exchange_rates::RATES_API;

    #[test]
    fn test_last_accounting_period() -> anyhow::Result<()> {
        let company = Company {
            registration_date: NaiveDate::from_ymd_opt(2022, 11, 28).unwrap(),
        };
        let _period = company.last_accounting_period();
        // dbg!(period)?;
        Ok(())
    }

    #[tokio::test]
    async fn test_prev_balance_sheet() -> anyhow::Result<()> {
        let company = Company {
            registration_date: NaiveDate::from_ymd_opt(2022, 11, 28).unwrap(),
        };
        let _sheet = company.prev_balance_sheet().await?;
        // dbg!(sheet);
        Ok(())
    }

    #[tokio::test]
    async fn test_next_balance_sheet() -> anyhow::Result<()> {
        let company = Company {
            registration_date: NaiveDate::from_ymd_opt(2022, 11, 28).unwrap(),
        };
        let mut starling = StarlingClient::new()?;
        let now = Utc::now().date_naive();

        let sheet = company
            .balance_sheet_at(now, &mut starling, &*RATES_API)
            .await?;
        dbg!(&sheet);
        println!("{sheet}");
        Ok(())
    }
}
