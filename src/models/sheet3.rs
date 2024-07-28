use super::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct AccountBalanceChanges {
    pub account: &'static Account,
    pub balance_changes: Vec<BalanceChange>,
}
impl AccountBalanceChanges {
    pub fn new(account: &'static Account) -> Self {
        AccountBalanceChanges {
            account,
            balance_changes: Vec::new(),
        }
    }
    pub fn push(&mut self, balance_change: BalanceChange) -> &mut Self {
        self.balance_changes.push(balance_change);
        self
    }
    pub fn balance(&self) -> Decimal {
        self.balance_changes.iter().map(|bc| bc.amount).sum()
    }
}

#[derive(Default, Debug)]
pub struct BalanceSheet3 {
    pub accounts: HashMap<&'static Account, AccountBalanceChanges>,
    pub date: NaiveDate,
}
impl BalanceSheet3 {
    pub fn from_transactions(transactions: &[super::Transaction]) -> Self {
        let mut bs = BalanceSheet3::default();
        for tx in transactions.iter() {
            bs.add_transaction(tx);
        }
        bs
    }
    pub fn account_mut(&mut self, account: &'static Account) -> &mut AccountBalanceChanges {
        self.accounts
            .entry(account)
            .or_insert_with(|| AccountBalanceChanges::new(account))
    }
    pub fn add_transaction(&mut self, transaction: &super::Transaction) -> &mut Self {
        self.account_mut(transaction.from).push(BalanceChange {
            date: transaction.date,
            amount: match transaction.from.account_type() {
                AccountType::Asset => transaction.amount_gbp,
                AccountType::Liability => -transaction.amount_gbp,
                AccountType::Revenue => transaction.amount_gbp,
                AccountType::Expense => -transaction.amount_gbp,
                AccountType::Equity => transaction.amount_gbp,
            },
        });
        self.account_mut(transaction.to).push(BalanceChange {
            date: transaction.date,
            amount: match transaction.to.account_type() {
                AccountType::Asset => transaction.amount_gbp,
                AccountType::Liability => -transaction.amount_gbp,
                AccountType::Revenue => -transaction.amount_gbp,
                AccountType::Expense => transaction.amount_gbp,
                AccountType::Equity => transaction.amount_gbp,
            },
        });
        // self.mk_balanced();
        self
    }
    // pub fn mk_balanced(&mut self) -> &mut Self {
    //     todo!()
    // }
    pub fn accounts_of_type(&self, account_type: AccountType) -> Vec<AccountBalanceChanges> {
        self.accounts
            .values()
            .filter(|a| a.account.account_type() == account_type)
            .cloned()
            .collect()
    }
    pub fn total_of(&self, account_type: AccountType) -> Decimal {
        self.accounts_of_type(account_type)
            .iter()
            .map(|a| a.balance())
            .sum()
    }
}
