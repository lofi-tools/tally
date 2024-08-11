use super::*;
use rust_decimal::{prelude::Zero, MathematicalOps};
use std::collections::HashMap;
use tx1::Transaction1;
use tx2::Transaction2;

#[derive(Debug, Clone)]
pub struct AccountBalanceChanges {
    pub account_id: AccountId,
    pub balance_changes: Vec<BalanceChange>,
}
impl AccountBalanceChanges {
    pub fn new(account_id: AccountId) -> Self {
        AccountBalanceChanges {
            account_id,
            balance_changes: Vec::new(),
        }
    }
    pub fn push(&mut self, balance_change: BalanceChange) -> &mut Self {
        self.balance_changes.push(balance_change);
        self
    }

    pub fn account(&self) -> anyhow::Result<&'static Account> {
        self.account_id.account()
    }
    pub fn balance_at(&self, date: NaiveDate) -> Decimal {
        let principal: Decimal = self.balance_changes.iter().map(|bc| bc.amount).sum();
        principal + self.interest_at(date).unwrap()
        // TODO set to zero if less than 0.5 cents
    }
    pub fn interest_at(&self, balance_date: NaiveDate) -> anyhow::Result<Decimal> {
        let loan_apy = match self.account()?.loan_apy {
            Some(apy) => apy,
            None => return Ok(Decimal::zero()),
        };
        let apy_multiplier_per_year = Decimal::from(1)
            + loan_apy
                / Decimal::from_f64(100.0)
                    .ok_or(anyhow::Error::msg("value can't be represented as Decimal"))?;
        let apy_muliplier_per_day =
            apy_multiplier_per_year.powd(Decimal::from(1) / Decimal::from(366));
        let oldest_payment = match self.balance_changes.iter().min_by_key(|bc| bc.date) {
            Some(payment) => payment,
            None => return Ok(Decimal::zero()),
        };

        // fn last_of_month(date: NaiveDate) -> NaiveDate {
        //     let first_of_next_month = date.with_day(1).unwrap() + Months::new(1);
        //     first_of_next_month - Duration::days(1)
        // }

        let mut payments = self.balance_changes.clone();
        payments.sort_by_key(|bc| bc.date);

        let (mut total_interest, mut balance, mut prev_payment_date) =
            (Decimal::zero(), Decimal::zero(), oldest_payment.date);
        for payment in self.balance_changes.iter() {
            let days_since_prev_payment = (payment.date - prev_payment_date).num_days();
            let interest_since_prev_payment =
                balance * (apy_muliplier_per_day.powi(days_since_prev_payment) - Decimal::from(1));

            balance = balance + payment.amount + interest_since_prev_payment;
            total_interest = total_interest + interest_since_prev_payment;
            prev_payment_date = payment.date;
        }
        // then add interest between last payment and balance_date
        let days_since_prev_payment = (balance_date - prev_payment_date).num_days();
        let interest_since_prev_payment =
            balance * (apy_muliplier_per_day.powi(days_since_prev_payment) - Decimal::from(1));

        balance = balance + interest_since_prev_payment;
        total_interest = total_interest + interest_since_prev_payment;

        Ok(total_interest)
    }

    // not needed, interest will be minimal anyway
    // #[deprecated]
    // pub fn interest_at_2(&self, balance_date: NaiveDate) -> anyhow::Result<Decimal> {
    //     let loan_apy = match self.account()?.loan_apy {
    //         Some(apy) => apy,
    //         None => return Ok(Decimal::zero()),
    //     };
    //     todo!()
    //     // NEXT TIME re-do Transaction with Vec<TxOutput> , TxOutput has a currency and account and amount
    // }
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

    pub fn for_account(&self, account_id: AccountId) -> &AccountBalanceChanges {
        self.accounts.get(&account_id).unwrap()
    }
    pub fn account_mut(&mut self, account_id: AccountId) -> &mut AccountBalanceChanges {
        self.accounts
            .entry(account_id)
            .or_insert_with(|| AccountBalanceChanges::new(account_id))
    }
    pub fn add_tx1(&mut self, transaction: &Transaction1) -> &mut Self {
        self.account_mut(transaction.from.id).push(BalanceChange {
            date: transaction.date,
            amount: match transaction.from.account_type() {
                AccountType::Asset => -transaction.amount_gbp,
                AccountType::Liability => transaction.amount_gbp,
                AccountType::Revenue => transaction.amount_gbp,
                AccountType::Expense => -transaction.amount_gbp,
                AccountType::Equity => transaction.amount_gbp,
            },
        });
        self.account_mut(transaction.to.id).push(BalanceChange {
            date: transaction.date,
            amount: match transaction.to.account_type() {
                AccountType::Asset => transaction.amount_gbp,
                AccountType::Liability => -transaction.amount_gbp,
                AccountType::Revenue => -transaction.amount_gbp,
                AccountType::Expense => transaction.amount_gbp,
                AccountType::Equity => transaction.amount_gbp,
            },
        });
        self
    }
    pub fn add_tx2(&mut self, tx: &Transaction2) -> &mut Self {
        for output in tx.outputs.iter() {
            self.account_mut(output.account_id).push(BalanceChange {
                date: tx.datetime.date_naive(),
                amount: output.amount_diff,
            });
        }
        self
    }

    pub fn account_balance(&mut self, account: &'static Account) -> Decimal {
        self.for_account(account.id).balance_at(self.date)
    }
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
    pub fn total_of(&self, account_type: AccountType) -> Decimal {
        self.accounts_of_type(account_type)
            .iter()
            .map(|a| a.balance_at(self.date))
            .sum()
    }
    pub fn retained_earnings(&self) -> Decimal {
        self.total_of(AccountType::Asset) - self.total_of(AccountType::Liability)
    }
}
impl std::fmt::Display for BalanceSheet3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for account in self.accounts_of_type(AccountType::Asset).iter() {
            let account_name = account.account().map_err(|_| std::fmt::Error)?.name;
            let balance = account.balance_at(self.date);
            writeln!(f, "{}: {}", account_name, balance)?;
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

    #[test]
    fn tx1_test_balance_sheet3_directors_loan_instant_repayment() -> anyhow::Result<()> {
        let mut bs = BalanceSheet3::now();

        bs.add_tx1(&Transaction1::sale((1000, Utc::now())));
        assert_eq!(bs.account_balance(&BANK), 1000.into());
        assert_eq!(bs.account_balance(&SALES), 1000.into());

        bs.add_tx1(&Transaction1::lend_to_director(1000_f64));
        assert_eq!(bs.account_balance(&BANK), 0.into());
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN), 1000.into());

        bs.add_tx1(&Transaction1::director_repays(1000_f64));
        assert_eq!(bs.account_balance(&BANK), 1000.into());
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN), 0.into());

        Ok(())
    }

    #[test]
    fn tx1_test_balance_sheet3_loan_out_repayment_year_later() -> anyhow::Result<()> {
        let year_ago = Utc::now() - Months::new(12);

        let mut bs = BalanceSheet3::new(year_ago.date_naive());
        bs.add_tx1(&Transaction1::sale((1000, year_ago)));
        assert_eq!(bs.account_balance(&BANK), 1000.into());
        assert_eq!(bs.account_balance(&SALES), 1000.into());

        bs.add_tx1(&Transaction1::lend_to_director((1000, year_ago)));
        assert_eq!(bs.account_balance(&BANK), 0.into());
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN), 1000.into());

        bs = bs.with_date(Utc::now().date_naive());
        assert_eq!(bs.account_balance(&BANK), 0.into());
        // one year later, loan balance should be 1000 + 2% APY
        assert!(bs.account_balance(&DIRECTORS_LOAN).is_close_to(1020));

        Ok(())
    }

    #[test]
    fn tx2_test_balance_sheet3_directors_loan_instant_repayment() -> anyhow::Result<()> {
        let mut bs = BalanceSheet3::now();

        bs.add_tx2(&Transaction2::sale((1000, Utc::now())));
        assert_eq!(bs.account_balance(&BANK), 1000.into());
        assert_eq!(bs.account_balance(&CLIENTS), (-1000).into());

        bs.add_tx2(&Transaction2::director_borrows_gbp(1000_f64));
        assert_eq!(bs.account_balance(&BANK), 0.into());
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN), 1000.into());

        bs.add_tx2(&Transaction2::director_repays_bank_gbp(1000_f64));
        assert_eq!(bs.account_balance(&BANK), 1000.into());
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN), 0.into());

        Ok(())
    }

    #[test]
    fn tx2_test_balance_sheet3_loan_out_repayment_year_later() -> anyhow::Result<()> {
        let year_ago = Utc::now() - Months::new(12);

        let mut bs = BalanceSheet3::new(year_ago.date_naive());
        bs.add_tx2(&Transaction2::sale((1000, year_ago)));
        assert_eq!(bs.account_balance(&BANK), 1000.into());
        assert_eq!(bs.account_balance(&CLIENTS), (-1000).into());

        bs.add_tx2(&Transaction2::director_borrows_gbp((1000, year_ago)));
        assert_eq!(bs.account_balance(&BANK), 0.into());
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN), 1000.into());

        bs = bs.with_date(Utc::now().date_naive());
        assert_eq!(bs.account_balance(&BANK), 0.into());
        // one year later, loan balance should be 1000 + 2% APY
        assert!(bs.account_balance(&DIRECTORS_LOAN).is_close_to(1020));

        Ok(())
    }
}
