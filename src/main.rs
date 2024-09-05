use crate::models::profit_and_loss::ProfitAndLoss;
use crate::models::sheet3::BalanceSheet3;
use adapters::exchange_rates::models::DayPricePoint;
use adapters::exchange_rates::{AssetPair, CachedRatesApi, Currency, TimeRange};
use adapters::{banks::nordigen_client::NordigenClient, exchange_rates::RatesApi};
use anyhow::anyhow;
use chrono::{DateTime, Duration, NaiveDate};
use config::Config;
pub use config::CONFIG;
use file_cache::{FileBytes, FromFileOrNew};
use models::static_data::{DIRECTORS_LOAN, EXPENSES_TO_REPAY, NEXO_EUR};
use models::tx2::{Transaction2, TxOutput};
use models::DateAndAmount;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utils::{DateRange, DatetimeUtcExt};

pub mod config;
pub mod models;
pub mod utils;
pub mod adapters {
    pub mod banks;
    pub mod exchange_rates;
}

// TODO calculate interest on director's loan -> add interest in transaction
// TODO transaction::repay_loan_in_currency
// TODO ensure director's loan account is zero

// TODO struct ListTransactions -> get_balance_sheet_at(date): adds interest transactions at end of months
// TODO struct ListTransactions:

// TODO BalanceSheet contains Accounts, Accounts contain BalanceChanges
// -> calculating the balance sheet ? means classifying BalanceChanges, then going over loan accounts again for interest

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut state = AppState::from_config(&CONFIG)?;
    let mut transactions = state.fetch_starling_transactions().await?;
    transactions
        .add_directors_loan_repayments(&state.rates_api)
        .await?;
    transactions.add_expenses(Expenses::static_expenses_to_repay()?);

    let accounting_period = TimeRange::new(
        DateTime::from_naive_date(NaiveDate::from_ymd_opt(2022, 11, 28).unwrap()),
        DateTime::from_naive_date(NaiveDate::from_ymd_opt(2023, 11, 30).unwrap()),
    );

    // let period_end = NaiveDate::from_ymd_opt(2023, 11, 30).unwrap();
    let balance_sheet = BalanceSheet3::new(accounting_period.end.date_naive(), &state.rates_api)?
        .with_transactions(&transactions);
    println!("{balance_sheet}");

    // TODO profit and loss
    let profit_and_loss = ProfitAndLoss::new(accounting_period, &transactions)?;
    print!("{profit_and_loss}");

    // TODO task for bank account transfers without corresponding accounting
    // TODO assert invoice file per transaction (store in dir in git)

    Ok(())
}

pub struct AppState {
    pub bank_api: NordigenClient,
    pub rates_api: CachedRatesApi,
}
impl AppState {
    pub fn from_config(_config: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            bank_api: NordigenClient::new(),
            rates_api: CachedRatesApi::new()?,
        })
    }

    async fn fetch_starling_transactions(&mut self) -> anyhow::Result<ListTxs> {
        pub async fn make_new(app_state: &mut AppState) -> anyhow::Result<ListTxs> {
            let transactions = app_state.bank_api.list_starling_transactions().await?;

            let mapped_transactions: Vec<Transaction2> = transactions
                .items
                .iter()
                .map(|tx| Transaction2::from_starling(tx))
                .collect::<Result<Vec<_>, _>>()?;

            Ok(ListTxs::from_txs(mapped_transactions))
        }

        return ListTxs::from_file_or_save_new("starling_transactions.json", make_new(self)).await;
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListTxs {
    pub txs: Vec<Transaction2>,
}
impl ListTxs {
    pub fn empty() -> Self {
        Self { txs: Vec::new() }
    }
    pub fn from_txs(txs: Vec<Transaction2>) -> Self {
        Self { txs }
    }
    pub fn push(&mut self, tx: Transaction2) -> &mut Self {
        self.txs.push(tx);
        self
    }
    pub fn push_many(&mut self, txs: &[Transaction2]) -> &mut Self {
        self.txs.extend_from_slice(txs);
        self
    }

    pub fn before(&self, date: NaiveDate) -> ListTxs {
        ListTxs::from_txs(
            self.txs
                .iter()
                .filter(|tx| tx.date() < date)
                .cloned()
                .collect(),
        )
    }
    pub fn between_dates(&self, after: NaiveDate, before: NaiveDate) -> ListTxs {
        ListTxs::from_txs(
            self.txs
                .iter()
                .filter(|tx| tx.date() >= after && tx.date() <= before)
                .cloned()
                .collect(),
        )
    }
    pub fn director_borrows(&self) -> ListTxs {
        let mut borrows = self
            .txs
            .iter()
            .filter(|tx| tx.is_director_borrows())
            .cloned()
            .collect::<Vec<Transaction2>>();
        borrows.sort_by(|a, b| a.datetime.cmp(&b.datetime));
        ListTxs::from_txs(borrows)
    }

    pub async fn add_directors_loan_repayments(
        &mut self,
        rates_api: &impl RatesApi,
    ) -> anyhow::Result<&mut Self> {
        let mut repayments: Vec<Transaction2> = Vec::new();
        for borrow in self.director_borrows().iter() {
            let payment_day = borrow.datetime.with_start_of_day();
            let three_months_later = borrow.datetime + Duration::days(90);
            let possible_repay_dates = DateRange::new(payment_day, three_months_later);

            // TODO Optimize: call only once for all borrows
            let rates = rates_api
                .rate_hist(
                    &TimeRange::new(payment_day, three_months_later),
                    &AssetPair {
                        from_currency: Currency::GBP,
                        to_currency: Currency::EUR,
                    },
                )
                .await?;

            #[derive(Debug)]
            pub struct RepaymentData {
                pub amount_plus_interest_gbp: Decimal,
                pub amount_eur: Decimal,
                pub day_price_point: DayPricePoint,
            }
            let mut min_repay_opt: Option<RepaymentData> = None;
            'range_dates: for day_i in possible_repay_dates.into_iter() {
                let day_price_point = match rates.rate_at(day_i) {
                    Ok(rate) => rate,
                    Err(_) => {
                        continue 'range_dates;
                    }
                };

                let new_balance_gbp = borrow.balance_at(day_i)?;
                let potential_repay = RepaymentData {
                    amount_plus_interest_gbp: new_balance_gbp.amount,
                    amount_eur: day_price_point.rate_low * new_balance_gbp.amount,
                    day_price_point: day_price_point.clone(),
                };

                let min_repayment = min_repay_opt.as_ref().unwrap_or(&potential_repay);
                if potential_repay.amount_eur <= min_repayment.amount_eur {
                    min_repay_opt = Some(potential_repay);
                }
            }
            let min_repay = min_repay_opt.ok_or(anyhow!("No min repay price point found"))?;

            let repayment_tx = Transaction2 {
                outputs: vec![
                    TxOutput {
                        account_id: NEXO_EUR.id.clone(),
                        amount_diff: min_repay.amount_eur.trunc_with_scale(2),
                        datetime: min_repay.day_price_point.datetime,
                    },
                    TxOutput {
                        account_id: DIRECTORS_LOAN.id.clone(),
                        amount_diff: -min_repay.amount_plus_interest_gbp,
                        datetime: min_repay.day_price_point.datetime,
                    },
                ],
                datetime: min_repay.day_price_point.datetime,
            };
            repayments.push(repayment_tx);
        }

        Ok(self.push_many(&repayments))
    }
    pub fn add_expenses(&mut self, expenses: Expenses) -> &mut Self {
        self.push_many(&expenses.transactions())
    }

    // fn expense_outputs(&self) -> Vec<Expense> {
    //     self.outputs()
    //         .self
    //         .txs
    //         .iter()
    //         .filter(|tx| tx.is_expense_to_repay())
    //         .cloned()
    //         .collect()
    // }
}
impl std::ops::Deref for ListTxs {
    type Target = Vec<Transaction2>;
    fn deref(&self) -> &Self::Target {
        &self.txs
    }
}
impl FileBytes for ListTxs {
    fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
    fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug, Clone)]
pub struct Expense {
    pub desc: String,
    pub tx: Transaction2,
}
impl Expense {
    pub fn simply(dam: impl Into<DateAndAmount>) -> Self {
        let DateAndAmount { date, amount } = dam.into();
        Expense {
            desc: "Professional insurance".to_string(),
            tx: Transaction2 {
                outputs: vec![TxOutput {
                    account_id: EXPENSES_TO_REPAY.id.clone(),
                    amount_diff: amount,
                    datetime: DateTime::from_naive_date(date),
                }],
                datetime: DateTime::from_naive_date(date),
            },
        }
    }
    pub fn all_energy_bills() -> anyhow::Result<Expenses> {
        let file_str = std::fs::read_to_string(PathBuf::from("./.cache/octopus_payments.json"))?;
        let payments: Vec<DateAndAmount> = serde_json::from_str(&file_str)?;

        let expenses_energy_bills = payments
            .into_iter()
            .map(|dam| Expense::energy(dam))
            .collect::<Vec<_>>();
        // dbg!(
        //     &expenses_energy_bills,
        //     expenses_energy_bills
        //         .iter()
        //         .map(|e| e.tx.outputs[0].amount_diff)
        //         .sum::<Decimal>()
        // );
        Ok(Expenses(expenses_energy_bills))
    }
    pub fn energy_bills_between(time_range: &TimeRange) -> anyhow::Result<Expenses> {
        let all_energy_bills = Self::all_energy_bills()?;
        let expenses_energy_bills = all_energy_bills
            .0
            .into_iter()
            .filter(|ex| time_range.contains_datetime(ex.tx.datetime))
            .collect::<Vec<_>>();
        Ok(Expenses(expenses_energy_bills))
    }
    pub fn energy(dam: impl Into<DateAndAmount>) -> Self {
        let DateAndAmount { date, amount } = dam.into();
        let amount_repayable = amount / Decimal::from(4);
        Expense {
            desc: "Energy bill".to_string(),
            tx: Transaction2 {
                outputs: vec![TxOutput {
                    account_id: EXPENSES_TO_REPAY.id.clone(),
                    amount_diff: amount_repayable,
                    datetime: DateTime::from_naive_date(date),
                }],
                datetime: DateTime::from_naive_date(date),
            },
        }
    }
}
#[derive(Debug)]
pub struct Expenses(pub Vec<Expense>);
impl Expenses {
    pub fn static_expenses_to_repay() -> anyhow::Result<Self> {
        let mut expenses = vec![
            Expense::simply((135.31, (2024, 06, 21))), // simply business 135.31 on 2024 Jun 21
            Expense::simply((197.28, (2022, 12, 18))), // simply business 197.28 on 2022 Dec 18x
        ];
        expenses.extend(Expense::all_energy_bills()?.0);
        dbg!(
            &expenses,
            expenses
                .iter()
                .map(|e| e.tx.outputs[0].amount_diff)
                .sum::<Decimal>()
        );
        Ok(Expenses(expenses))
    }
    pub fn get_expenses_to_repay(&self) -> Expenses {
        Expenses(
            self.0
                .iter()
                .filter(|ex| {
                    ex.tx
                        .outputs
                        .iter()
                        .any(|o| o.account_id == EXPENSES_TO_REPAY.id)
                })
                .cloned()
                .collect(),
        )
    }
    pub fn transactions(&self) -> Vec<Transaction2> {
        self.0.iter().map(|exp| exp.tx.clone()).collect()
    }
    pub fn total(&self) -> Decimal {
        self.0.iter().map(|exp| exp.tx.outputs[0].amount_diff).sum()
    }
}

#[cfg(test)]
pub mod test_expenses {
    use super::*;

    #[test]
    fn test_octopus() -> anyhow::Result<()> {
        let all_energy_bills = Expense::all_energy_bills()?;
        Ok(())
    }
}
