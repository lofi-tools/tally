use crate::models::tx1::Transaction1;
use crate::models::{Account, AccountType};
use rust_decimal::Decimal;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct AccountBalance {
    pub account: &'static Account,
    pub balance: Decimal,
}
impl AccountBalance {
    pub fn new(account: &'static Account) -> Self {
        Self {
            account,
            balance: Decimal::default(),
        }
    }
    pub fn remove_money_from(&mut self, amount: Decimal) {
        match self.account.account_type() {
            AccountType::Asset => self.balance -= amount,
            AccountType::Liability => self.balance += amount,
            AccountType::Revenue => self.balance += amount,
            AccountType::Expense => self.balance -= amount,
            AccountType::Equity => self.balance -= amount,
        }
    }
    pub fn add_money_to(&mut self, amount: Decimal) {
        match self.account.account_type() {
            AccountType::Asset => self.balance += amount,
            AccountType::Liability => self.balance -= amount,
            AccountType::Revenue => self.balance -= amount,
            AccountType::Expense => self.balance += amount,
            AccountType::Equity => self.balance += amount,
        }
    }
    pub fn set_to(&mut self, amount: Decimal) {
        self.balance = amount;
    }
}

#[derive(Default, Debug)]
pub struct BalanceSheet2 {
    pub accounts: HashMap<&'static Account, AccountBalance>,
}
impl BalanceSheet2 {
    pub fn from_transactions1(transactions: &[Transaction1]) -> Self {
        let mut bs = BalanceSheet2::default();
        for tx in transactions.iter() {
            bs.add_transaction1(tx);
        }
        bs
    }
    pub fn account_mut(&mut self, account: &'static Account) -> &mut AccountBalance {
        self.accounts
            .entry(account)
            .or_insert_with(|| AccountBalance::new(account))
    }
    pub fn add_transaction1(&mut self, transaction: &Transaction1) -> &mut Self {
        self.account_mut(transaction.from)
            .remove_money_from(transaction.amount_gbp);

        self.account_mut(transaction.to)
            .add_money_to(transaction.amount_gbp);

        self.mk_balanced();
        self
    }
    pub fn mk_balanced(&mut self) -> &mut Self {
        let total_assets = self.total_of(AccountType::Asset);
        let total_liabilities = self.total_of(AccountType::Liability);

        let _retained_earnings = total_assets - total_liabilities;
        // self.account_mut(&RETAINED_EARNINGS)
        //     .set_to(retained_earnings);

        self
    }

    pub fn accounts_of_type(&self, account_type: AccountType) -> Vec<AccountBalance> {
        self.accounts
            .values()
            .filter(|a| a.account.account_type() == account_type)
            .cloned()
            .collect()
    }
    pub fn total_of(&self, account_type: AccountType) -> Decimal {
        self.accounts_of_type(account_type)
            .iter()
            .map(|a| a.balance)
            .sum()
    }

    // pub fn total_assets(&self) -> Decimal {
    //     self.accounts_of_type(AccountType::Asset)
    //         .iter()
    //         .map(|a| a.balance)
    //         .sum()
    // }
    // pub fn total_liabilities(&self) -> Decimal {
    //     self.accounts_of_type(AccountType::Liability)
    //         .iter()
    //         .map(|a| a.balance)
    //         .sum()
    // }
    pub fn total_liabilities_and_equity(&self) -> Decimal {
        self.accounts_of_type(AccountType::Liability)
            .iter()
            .chain(self.accounts_of_type(AccountType::Equity).iter())
            .map(|a| a.balance)
            .sum()
    }
}
// impl std::fmt::Display for BalanceSheet2 {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         // TODO list assetBalances, liabilityBalances, equity
//         write!(
//             f,
//             "Assets: {}\nLiabilities: {}\nEquity: {}",
//             todo!(),
//             todo!(),
//             todo!()
//         )
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::static_data::{BANK, BORROW, DIRECTORS_LOAN, SALES};
    use crate::models::tx1::Transaction1;

    #[test]
    fn test_balance_sheet_directors_loan() -> anyhow::Result<()> {
        let mut bs = BalanceSheet2::from_transactions1(&[]);
        assert_eq!(bs.accounts.len(), 0);

        bs.add_transaction1(&Transaction1::sale(1000_f64));
        assert_eq!(bs.accounts[&*BANK].balance, 1000.into());
        assert_eq!(bs.accounts[&*SALES].balance, 1000.into());

        bs.add_transaction1(&Transaction1::lend_to_director(1000_f64));
        assert_eq!(bs.accounts[&*BANK].balance, 0.into());
        assert_eq!(bs.accounts[&*DIRECTORS_LOAN].balance, 1000.into());

        bs.add_transaction1(&Transaction1::director_repays(1000_f64));
        assert_eq!(bs.accounts[&*BANK].balance, 1000.into());
        assert_eq!(bs.accounts[&*DIRECTORS_LOAN].balance, 0.into());

        Ok(())
    }
    #[test]
    fn test_balance_sheet_company_borrows() -> anyhow::Result<()> {
        let mut bs = BalanceSheet2::default();
        bs.add_transaction1(&Transaction1::company_borrows(1000_f64));

        assert_eq!(bs.accounts[&*BANK].balance, 1000.into());
        assert_eq!(bs.accounts[&BORROW].balance, 1000.into());
        // assert_eq!(bs.accounts.try_get(&&RETAINED_EARNINGS)?.balance, 0.into()); // TODO use function to get earnings, not an account
        Ok(())
    }
}

pub mod balance_sheet_old {
    // use crate::models::std_accounts::*;
    // use crate::models::{Account, Transaction};
    // use rust_decimal::Decimal;
    // use std::collections::HashMap;

    // #[derive(Debug)]
    // pub struct BalanceTotals(pub HashMap<&'static Account, Decimal>);
    // impl BalanceTotals {
    //     pub fn from_transactions(transactions: &[Transaction]) -> Self {
    //         let mut sum = BalanceTotals(HashMap::new());
    //         for t in transactions.iter() {
    //             sum.add_transaction(t);
    //         }
    //         sum
    //     }
    //     pub fn add_transaction(&mut self, transaction: &Transaction) {
    //         // taking money is called CREDIT
    //         // TODO prevent overflow
    //         *self.0.entry(transaction.from).or_default() -= transaction.amount_gbp;
    //         // adding money is called DEBIT
    //         // TODO prevent overflow
    //         *self.0.entry(transaction.to).or_default() += transaction.amount_gbp;
    //     }
    //     // pub fn get(&self, account: &'static Account) -> Balance {
    //     //     Balance {
    //     //         account,
    //     //         total_gbp_cents: self.0.get(account).copied().unwrap_or_default(),
    //     //     }
    //     // }
    //     pub fn asset(&self, account: &'static Account) -> Balance {
    //         Balance {
    //             account,
    //             total_gbp: self.0.get(account).copied().unwrap_or_default(),
    //         }
    //     }
    //     pub fn liability(&self, account: &'static Account) -> Balance {
    //         // a positive account balance means we've moved company money INTO a liability account => money is owed to the company.
    //         //  => the liability is negative (a positive liability means the company owes money)
    //         Balance {
    //             account,
    //             total_gbp: -self.0.get(account).copied().unwrap_or_default(),
    //         }
    //     }

    //     pub fn balance_sheet(&self) -> BalanceSheet {
    //         BalanceSheet {
    //             fixed_assets: vec![],
    //             current_assets: vec![self.asset(&BANK)],
    //             current_liabilities: vec![self.liability(&DIRECTORS_LOAN)],
    //             // equity: vec![],
    //         }
    //     }
    // }
    // #[derive(Debug)]
    // pub struct Balance {
    //     pub account: &'static Account,
    //     pub total_gbp: Decimal,
    // }

    // #[derive(Debug)]
    // pub struct BalanceSheet {
    //     // Assets
    //     pub fixed_assets: Vec<Balance>,
    //     pub current_assets: Vec<Balance>,
    //     // Liabilities
    //     pub current_liabilities: Vec<Balance>,
    //     // pub equity: Vec<Balance>, // TODO calculate from rest instead
    // }
    // impl BalanceSheet {
    //     pub fn total_fixed_assets(&self) -> Decimal {
    //         self.fixed_assets.iter().map(|b| b.total_gbp).sum()
    //     }
    //     pub fn total_current_assets(&self) -> Decimal {
    //         self.current_assets.iter().map(|b| b.total_gbp).sum()
    //     }
    //     pub fn total_current_liabilities(&self) -> Decimal {
    //         self.current_liabilities
    //             .iter()
    //             .map(|b| b.total_gbp)
    //             .sum::<Decimal>()
    //     }
    //     pub fn total_all_assets(&self) -> Decimal {
    //         self.total_current_assets() + self.total_fixed_assets()
    //     }
    //     pub fn equity(&self) -> Decimal {
    //         self.total_all_assets() - self.total_current_liabilities()
    //     }
    // }
}
