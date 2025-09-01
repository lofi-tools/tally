use super::sheet3::CompanyAccounting;
use super::static_data::{
    AccountId, BALANCE_SHEET_2023_11_30, BANK, DIRECTORS_LOAN, GBP, HasAccountId, NEXO_EUR,
    NEXO_GBP,
};
use super::{Account, Asset2, AssetId};
use crate::adapters::exchange_rates::RatesApi;
use crate::adapters::exchange_rates::{GBP_EUR_PAIR, RATES_API, TimeRange};
use crate::adapters::starling_bank::StarlingClient;
use crate::utils::errors::AnyErr;
use crate::utils::{DateExt, DatetimeUtcExt};
use crate::{Expenses, ListTxns};
use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, Utc};
use file_cache::{Cacheable, JsonFileBytes};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

pub const EXT: LazyLock<Company> = LazyLock::new(|| Company {
    registration_date: NaiveDate::from_ymd_opt(2022, 11, 28).unwrap(),
});

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
        let go_back_n_years =
            match (first_of_next_month(birthday_this_year) - Months::new(3)) > now.date_naive() {
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

    pub async fn accounting_at(
        &self,
        times: TimeRange,
        bank_api: &mut StarlingClient,
        rates_api: &impl RatesApi,
    ) -> anyhow::Result<CompanyAccounting> {
        let accounting_period = self.last_accounting_period()?;

        let real_and_virtual_transactions = self
            .real_and_virtual_transactions(bank_api, rates_api)
            .await?
            .in_time_range(&accounting_period);

        let mut builder = CompanyAccounting::empty(times, &rates_api)
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
            .ok_or(anyhow::Error::msg("No prev balance sheet found"))?;
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
            .add_cached_directors_loan_repayments(&rates_api)
            .await;
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
    pub target_currency: AssetId,
    pub account_balances: HashMap<AccountId, Decimal>,
    pub date: NaiveDate,
    // TODO include notes (include gbp/eur rate at time)
}
impl BalanceSheet4 {
    pub fn get(&self, account_id: &AccountId) -> Result<Decimal, AnyErr> {
        self.account_balances
            .get(account_id)
            .cloned()
            .ok_or("No balance found".into())
    }
    pub fn set_value(&mut self, account_id: &AccountId, value: Decimal) {
        self.account_balances.insert(account_id.clone(), value);
    }
    pub async fn total_from_accounts(
        &self,
        accounts: &[&Account],
        rates: impl RatesApi,
    ) -> Decimal {
        let mut sum = Decimal::ZERO;
        for account in accounts {
            // convert if different currency
            let balance_converted = match self.target_currency == account.asset_id {
                true => self.get(account.account_id()).unwrap(),
                false => {
                    let asset_pair = GBP_EUR_PAIR.clone(); // TODO
                    let rate_then = rates.rate_at(&self.date, &asset_pair).await.unwrap();
                    let converted = self.get(account.account_id()).unwrap() / rate_then.rate_high;
                    converted
                }
            };
            sum += balance_converted;
        }

        sum
    }
    pub async fn total_current_assets(&self) -> Result<u64, AnyErr> {
        let dec = self
            .total_from_accounts(&[*BANK, *DIRECTORS_LOAN, *NEXO_GBP, *NEXO_EUR], &*RATES_API)
            .await;
        use num_traits::ToPrimitive;
        let u: u64 = dec
            .floor()
            .to_u64()
            .ok_or(AnyErr::from("Failed converting to u64"))?;
        Ok(u)
    }
    pub async fn report(&self) -> Result<MicroEntityBalanceSheetReport, AnyErr> {
        let total_current_assets = self.total_current_assets().await?;
        let provision_for_liabilities = 540;
        let net_current_assets_or_liabilities: i64 =
            (total_current_assets as i64) - (provision_for_liabilities as i64);
        let total_net_assets_or_liabilities: i64 = net_current_assets_or_liabilities;

        Ok(MicroEntityBalanceSheetReport {
            date: self.date,
            currency: Asset2::Id(GBP.clone()),
            called_up_share_capital_not_paid: 0,
            total_fixed_assets: 0,
            total_current_assets,
            prepayments_and_accrued_income: 0,
            creditors_amount_due_within_one_year: 0,
            net_current_assets_or_liabilities: 0,
            total_assets_less_current_liabilities: 0,
            creditors_amount_due_after_more_than_one_year: 0,
            provision_for_liabilities,
            accruals_and_deferred_income: 0,
            total_net_assets_or_liabilities,
            capital_and_reserves: 0,
            num_employees: 1,
        })
    }
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

#[derive(Debug, Clone)]
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

        let period = company.last_accounting_period()?;

        let sheet = company
            .accounting_at(period, &mut starling, &*RATES_API)
            .await?;
        // dbg!(&sheet.for_account(*DIRECTORS_LOAN));
        println!("{sheet}");

        let report = sheet.balance_sheet()?.report().await;
        dbg!(&report);
        Ok(())
    }

    #[tokio::test]
    async fn test_company_pnl() -> anyhow::Result<()> {
        let company = Company {
            registration_date: NaiveDate::from_ymd_opt(2022, 11, 28).unwrap(),
        };
        let mut starling = StarlingClient::new()?;
        let period = company.last_accounting_period()?;

        let accounting = company
            .accounting_at(period, &mut starling, &*RATES_API)
            .await?;

        let pnl = accounting.profit_loss()?;
        // dbg!(&pnl);
        println!("{pnl}");

        let report = pnl.report();
        dbg!(&report);

        Ok(())
    }
}
