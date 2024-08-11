use super::{
    static_data::{PAYE_PAID, SALES, WAGES_NET},
    tx1::Transaction1,
};
use crate::models::Account;
use rust_decimal::Decimal;
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct AccountTransactions(pub HashMap<&'static Account, Vec<Transaction1>>); // TODO use AccountMovement with variants In/Out
impl AccountTransactions {
    pub fn insert(&mut self, account: &'static Account, transaction: &Transaction1) {
        self.0.entry(account).or_default().push(transaction.clone())
    }
    // TODO if needed take in account direction of movement In/Out
    pub fn total_abs(&self) -> Decimal {
        self.0
            .iter()
            .map(|(_, v)| v.iter().map(|t| t.amount_gbp).sum::<Decimal>())
            .sum()
    }
}

#[derive(Default, Debug)]
pub struct ProfitAndLoss {
    // gross
    pub income: AccountTransactions,
    pub direct_expenses: AccountTransactions,
    // pub gross_profit_and_loss: i32,

    // net
    pub overheads: AccountTransactions,
    pub financial_expenses: AccountTransactions,
    pub taxes: AccountTransactions,
    pub net_profit_and_loss: i32,
}

impl ProfitAndLoss {
    pub fn from_transactions1(transactions: &[Transaction1]) -> Self {
        let mut pl = ProfitAndLoss::default();
        for tx in transactions.iter() {
            pl.add_transaction1(tx);
        }
        pl
    }
    pub fn add_transaction1(&mut self, transaction: &Transaction1) {
        if transaction.is_sale() {
            self.income.insert(&SALES, transaction);
        }
        if transaction.is_to_paye() {
            self.taxes.insert(&PAYE_PAID, transaction)
        }

        // TODO wages, NIC, corporation tax
    }

    // TODO rm once total is not abs
    pub fn total_income(&self) -> Decimal {
        self.income.total_abs()
    }
    // TODO rm once total is not abs
    pub fn total_direct_expenses(&self) -> Decimal {
        self.direct_expenses.total_abs()
    }
    pub fn gross_profit(&self) -> Decimal {
        self.total_income() - self.total_direct_expenses()
    }

    pub fn net_profit(&self) -> Decimal {
        self.gross_profit()
            - self.overheads.total_abs()
            - self.financial_expenses.total_abs()
            - self.taxes.total_abs()
    }
}
impl std::fmt::Display for ProfitAndLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Income: \t\t\t{:?}\n", self.income)?;
        write!(f, "Expenses: \t\t\t{:?}\n", self.direct_expenses)?;
        write!(f, "Gross profit / loss: \t\t{}\n", self.gross_profit())?;
        write!(f, "\n")?;

        write!(f, "Overheads: \t\t\t{:?}\n", self.overheads)?;
        write!(f, "Financial expenses: \t\t{:?}\n", self.financial_expenses)?;
        write!(f, "Taxes: \t\t\t\t{:?}\n", self.taxes)?;
        write!(f, "Net profit / loss: \t\t{}\n\n", self.net_profit())?;
        Ok(())
    }
}

impl Transaction1 {
    fn is_sale(&self) -> bool {
        self.from == *SALES
    }
    fn is_to_paye(&self) -> bool {
        self.to == *PAYE_PAID
    }
    fn _is_wage_net(&self) -> bool {
        self.to == *WAGES_NET // TODO figure out WHAT TO DO WITH GROSS WAGES account
    }
}
