use super::*;
use crate::adapters::exchange_rates::models::DayPricePoint;
use crate::adapters::exchange_rates::{AssetPair, Currency, RatesApi};
use crate::utils::{DateRange, DatetimeUtcExt, NumExt};
use anyhow::anyhow;
use chrono::Duration;
use rust_decimal::{prelude::Zero, MathematicalOps};
use static_data::NEXO_EUR;
use std::collections::HashMap;
use std::ops::Add;
use tx1::Transaction1;
use tx2::Transaction2;

// TODO recalculate interest on AccountBalanceChanges: go through transaction dates in order

#[derive(Debug, Clone)]
pub struct AccountBalanceChanges {
    pub account_id: AccountId,
    pub balance_changes: Vec<TxOutput>,
}
impl AccountBalanceChanges {
    pub fn new(account_id: AccountId) -> Self {
        AccountBalanceChanges {
            account_id,
            balance_changes: Vec::new(),
        }
    }
    pub fn push(&mut self, balance_change: TxOutput) -> &mut Self {
        self.balance_changes.push(balance_change);
        self
    }
    pub fn account(&self) -> anyhow::Result<&'static Account> {
        self.account_id.account()
    }

    pub fn sorted_chrono(&self) -> Self {
        let mut balance_changes = self.balance_changes.clone();
        balance_changes.sort_by_key(|bc| bc.datetime);
        Self {
            account_id: self.account_id.clone(),
            balance_changes,
        }
    }
    pub fn before(&self, datetime: DateTime<Utc>) -> Self {
        Self {
            account_id: self.account_id.clone(),
            balance_changes: self
                .balance_changes
                .iter()
                .filter(|bc| bc.datetime <= datetime)
                .cloned()
                .collect(),
        }
    }
    pub fn balance_at(&self, datetime: DateTime<Utc>) -> anyhow::Result<AccountBalance2> {
        let mut balance_at_last_tx: Option<AccountBalance2> = None;

        for change in &self.before(datetime).sorted_chrono().balance_changes {
            let new_balance = match &balance_at_last_tx {
                Some(prev_balance) => AccountBalance2 {
                    account_id: self.account_id.clone(),
                    datetime: change.datetime,
                    amount: prev_balance.with_interest_at(change.datetime)?.amount
                        + change.amount_diff,
                },
                None => change.balance_then(),
            };

            balance_at_last_tx = Some(new_balance);
        }

        let balance_at_last_tx =
            balance_at_last_tx.ok_or(anyhow!("Balance should be Some() by now"))?;
        Ok(balance_at_last_tx.with_interest_at(datetime)?)
    }
}

#[derive(Debug, Clone)]
pub struct AccountBalance2 {
    pub account_id: AccountId,
    pub datetime: DateTime<Utc>,
    pub amount: Decimal,
}
impl AccountBalance2 {
    pub fn account(&self) -> anyhow::Result<&'static Account> {
        self.account_id.account()
    }
    pub fn loan_apy(&self) -> anyhow::Result<Option<Decimal>> {
        Ok(self.account()?.loan_apy)
    }
    pub fn interest_only_at(&self, datetime: DateTime<Utc>) -> anyhow::Result<Option<Decimal>> {
        if self.loan_apy()?.is_none() {
            return Ok(None);
        }
        let balance_at = self.with_interest_at(datetime)?.amount;
        Ok(Some(balance_at - self.amount))
    }
    pub fn with_interest_at(&self, datetime: DateTime<Utc>) -> anyhow::Result<AccountBalance2> {
        let apy = match self.loan_apy()? {
            Some(apy) => apy,
            None => {
                return Ok(AccountBalance2 {
                    datetime,
                    ..self.clone()
                })
            }
        };
        if datetime.date_naive() < self.datetime.date_naive() {
            return Err(anyhow!(
                "Trying to calculate interest at a date before the prev balance"
            ));
        }

        let num_days =
            Decimal::from_i64((datetime.date_naive() - self.datetime.date_naive()).num_days())
                .ok_or(anyhow!("Failed converting num_days to Decimal"))?;
        let apy_multiplier_per_year = Decimal::from(1)
            + apy
                / Decimal::from_f64(100.0)
                    .ok_or(anyhow::Error::msg("value can't be represented as Decimal"))?;
        let balance_plus_interest =
            self.amount * apy_multiplier_per_year.powd(num_days / Decimal::from(366));

        Ok(AccountBalance2 {
            account_id: self.account_id.clone(),
            datetime,
            amount: balance_plus_interest.trunc_with_scale(2),
        })
    }
    pub fn is_close_to(&self, other: impl Into<Decimal>) -> bool {
        self.amount.is_close_to(other)
    }
}
impl std::ops::Add<Decimal> for AccountBalance2 {
    type Output = AccountBalance2;
    fn add(self, rhs: Decimal) -> Self::Output {
        AccountBalance2 {
            account_id: self.account_id,
            datetime: self.datetime,
            amount: self.amount + rhs,
        }
    }
}
impl PartialEq<i64> for AccountBalance2 {
    fn eq(&self, rhs: &i64) -> bool {
        self.amount
            == match Decimal::from_i64(*rhs) {
                Some(i) => i,
                None => return false,
            }
    }
}
impl PartialEq<Decimal> for AccountBalance2 {
    fn eq(&self, rhs: &Decimal) -> bool {
        self.amount == *rhs
    }
}
impl std::fmt::Display for AccountBalance2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.account_id, self.amount)
    }
}

#[derive(Debug)]
pub struct BalanceSheet3 {
    pub accounts: HashMap<AccountId, AccountBalanceChanges>,
    pub date: NaiveDate,
}
impl BalanceSheet3 {
    pub fn now() -> Self {
        BalanceSheet3::new(Utc::now().date_naive())
    }
    pub fn new(date: NaiveDate) -> Self {
        BalanceSheet3 {
            accounts: HashMap::new(),
            date,
        }
    }
    pub fn with_date(mut self, date: NaiveDate) -> Self {
        self.date = date;
        self
    }
    pub fn now_from_transactions2(transactions: &[Transaction2]) -> Self {
        let mut bs = BalanceSheet3::new(Utc::now().date_naive());
        for tx2 in transactions.iter() {
            bs.add_tx2(tx2);
        }
        bs
    }

    pub fn for_account(&self, account_id: &AccountId) -> &AccountBalanceChanges {
        self.accounts.get(account_id).unwrap()
    }
    pub fn account_mut(&mut self, account_id: &AccountId) -> &mut AccountBalanceChanges {
        self.accounts
            .entry(account_id.clone())
            .or_insert_with(|| AccountBalanceChanges::new(account_id.clone()))
    }
    pub fn datetime(&self) -> DateTime<Utc> {
        DateTime::from_naive_date(self.date)
    }
    // pub fn add_tx1(&mut self, transaction: &Transaction1) -> &mut Self {
    //     self.account_mut(&transaction.from.id).push(BalanceChange {
    //         date: transaction.date,
    //         amount: match transaction.from.account_type() {
    //             AccountType::Asset => -transaction.amount_gbp,
    //             AccountType::Liability => transaction.amount_gbp,
    //             AccountType::Revenue => transaction.amount_gbp,
    //             AccountType::Expense => -transaction.amount_gbp,
    //             AccountType::Equity => transaction.amount_gbp,
    //         },
    //     });
    //     self.account_mut(&transaction.to.id).push(BalanceChange {
    //         date: transaction.date,
    //         amount: match transaction.to.account_type() {
    //             AccountType::Asset => transaction.amount_gbp,
    //             AccountType::Liability => -transaction.amount_gbp,
    //             AccountType::Revenue => -transaction.amount_gbp,
    //             AccountType::Expense => transaction.amount_gbp,
    //             AccountType::Equity => transaction.amount_gbp,
    //         },
    //     });
    //     self
    // }

    pub fn add_tx2(&mut self, tx: &Transaction2) -> &mut Self {
        for output in tx.outputs.iter() {
            self.account_mut(&output.account_id).push(output.clone());
        }
        self
    }
    pub fn add_many_tx2(&mut self, transactions: &[Transaction2]) -> &mut Self {
        for tx in transactions.iter() {
            self.add_tx2(tx);
        }
        self
    }

    pub fn account_balance(
        &mut self,
        account: &'static Account,
    ) -> anyhow::Result<AccountBalance2> {
        Ok(self.for_account(&account.id).balance_at(self.datetime())?)
    }
    // pub fn account_balance_total(&mut self, account: &'static Account) -> anyhow::Result<Decimal> {
    //     Ok(self.account_balance(&account)?)
    // }
    pub fn accounts_of_type(&self, account_type: AccountType) -> Vec<AccountBalanceChanges> {
        self.accounts
            .values()
            .filter(|a| {
                a.account()
                    .map(|acc| acc.account_type() == account_type)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }
    pub fn total_of(&self, account_type: AccountType) -> anyhow::Result<Decimal> {
        let mut sum = Decimal::ZERO;
        for account in &self.accounts_of_type(account_type) {
            let balance = account.balance_at(DateTime::from_naive_date(self.date))?;
            sum += balance.amount;
        }
        // self.accounts_of_type(account_type)
        //     .iter()
        //     .try_fold(Decimal::zero(), |sum, a| Ok(sum + a.balance_at(self.date)?))
        todo!()
    }
    pub fn retained_earnings(&self) -> anyhow::Result<Decimal> {
        Ok(self.total_of(AccountType::Asset)? - self.total_of(AccountType::Liability)?)
    }
}
impl std::fmt::Display for BalanceSheet3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for account in self.accounts_of_type(AccountType::Asset).iter() {
            let account_name = account.account().map_err(|_| std::fmt::Error)?.name;
            let balance = account
                .balance_at(self.datetime())
                .map_err(|_| std::fmt::Error)?;
            writeln!(f, "{}: {}", account_name, balance.amount)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::NumExt;
    use chrono::Months;
    use static_data::CLIENTS;

    // #[test]
    // fn tx1_test_balance_sheet3_directors_loan_instant_repayment() -> anyhow::Result<()> {
    //     let mut bs = BalanceSheet3::now();

    //     bs.add_tx1(&Transaction1::sale((1000, Utc::now())));
    //     assert_eq!(bs.account_balance(&BANK)?, 1000.into());
    //     assert_eq!(bs.account_balance(&SALES)?, 1000.into());

    //     bs.add_tx1(&Transaction1::lend_to_director(1000_f64));
    //     assert_eq!(bs.account_balance(&BANK)?, 0.into());
    //     assert_eq!(bs.account_balance(&DIRECTORS_LOAN)?, 1000.into());

    //     bs.add_tx1(&Transaction1::director_repays(1000_f64));
    //     assert_eq!(bs.account_balance(&BANK)?, 1000.into());
    //     assert_eq!(bs.account_balance(&DIRECTORS_LOAN)?, 0.into());

    //     Ok(())
    // }

    // #[test]
    // fn tx1_test_balance_sheet3_loan_out_repayment_year_later() -> anyhow::Result<()> {
    //     let year_ago = Utc::now() - Months::new(12);

    //     let mut bs = BalanceSheet3::new(year_ago.date_naive());
    //     bs.add_tx1(&Transaction1::sale((1000, year_ago)));
    //     assert_eq!(bs.account_balance(&BANK), 1000.into());
    //     assert_eq!(bs.account_balance(&SALES), 1000.into());

    //     bs.add_tx1(&Transaction1::lend_to_director((1000, year_ago)));
    //     assert_eq!(bs.account_balance(&BANK), 0.into());
    //     assert_eq!(bs.account_balance(&DIRECTORS_LOAN), 1000.into());

    //     bs = bs.with_date(Utc::now().date_naive());
    //     assert_eq!(bs.account_balance(&BANK), 0.into());
    //     // one year later, loan balance should be 1000 + 2% APY
    //     assert!(bs.account_balance(&DIRECTORS_LOAN).is_close_to(1020));

    //     Ok(())
    // }

    #[test]
    fn tx2_test_balance_sheet3_directors_loan_instant_repayment() -> anyhow::Result<()> {
        let mut bs = BalanceSheet3::now();

        bs.add_tx2(&Transaction2::sale((1000, Utc::now())));
        assert_eq!(bs.account_balance(&BANK)?, 1000);
        assert_eq!(bs.account_balance(&CLIENTS)?, (-1000));

        bs.add_tx2(&Transaction2::director_borrows_gbp(1000_f64));
        assert_eq!(bs.account_balance(&BANK)?, 0);
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN)?, 1000);

        bs.add_tx2(&Transaction2::director_repays_bank_gbp(1000_f64));
        assert_eq!(bs.account_balance(&BANK)?, 1000);
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN)?, 0);

        Ok(())
    }

    #[test]
    fn tx2_test_balance_sheet3_loan_out_repayment_year_later() -> anyhow::Result<()> {
        let year_ago = Utc::now() - Months::new(12);

        let mut bs = BalanceSheet3::new(year_ago.date_naive());
        bs.add_tx2(&Transaction2::sale((1000, year_ago)));
        assert_eq!(bs.account_balance(&BANK)?, 1000);
        assert_eq!(bs.account_balance(&CLIENTS)?, (-1000));

        bs.add_tx2(&Transaction2::director_borrows_gbp((1000, year_ago)));
        assert_eq!(bs.account_balance(&BANK)?, 0);
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN)?, 1000);

        bs = bs.with_date(Utc::now().date_naive());
        assert_eq!(bs.account_balance(&BANK)?, 0);
        // one year later, loan balance should be 1000 + 2% APY
        assert!(bs
            .account_balance(&DIRECTORS_LOAN)?
            .amount
            .is_close_to(1020));

        // now repay 1 year later, make sure the balance gets set to zero
        bs.add_tx2(&Transaction2::director_repays_bank_gbp((1020, Utc::now())));
        assert_eq!(bs.account_balance(&BANK)?, 1020);
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN)?, 0);
        dbg!(bs.account_balance(&DIRECTORS_LOAN)?);

        Ok(())
    }
}
