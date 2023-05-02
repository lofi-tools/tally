use chrono::NaiveDate;
use std_accounts::*;

pub mod std_accounts {
    use crate::models::Account;

    lazy_static::lazy_static! {
        pub static ref BANK: Account = Account{code:1200, name: "Company bank account", details:"Starling bank"};
        pub static ref DIRECTORS_LOAN: Account = Account{code:2301, name: "Director's loan account", details:"Director 1: Nicolas Marshall"};
        pub static ref SALES: Account = Account{code:4010, name:"Sales - Services", details:""};

        pub static ref WAGES_GROSS: Account= Account{code:7000, name:"Gross wages", details:""};
        pub static ref WAGES_NET: Account= Account {code:2220, name:"Net wages", details:""};
        pub static ref PAYE: Account = Account{code:2210, name:"P.A.Y.E.", details:""}; // NOTE credit this account when paying PAYE taxes
        pub static ref EMPLOYERS_NI: Account = Account{code:7006, name:"Employer's N.I.",details:""}; // NOTE employer's NI debit is balanced with a PAYE credit
        // NOT USED:
        // - 2230 Pension Fund
        // - 7007 employer's pensions
    }
}

// TODO make this into trait implemented by SalesTransaction (also contains link/hash of associated invoice)
#[derive(Debug, Clone)]
pub struct Transaction {
    from: &'static Account,
    to: &'static Account,
    amount_gbp_cent: u32,
    #[allow(dead_code)]
    date: NaiveDate,
}
impl Transaction {
    pub fn sale(amount_gbp: f64, date: (i32, u32, u32)) -> Self {
        Transaction {
            from: &SALES, // money that is owed to self
            to: &BANK,    // money that is owed by self
            amount_gbp_cent: to_cents(amount_gbp),
            date: NaiveDate::ymd(date.0, date.1, date.2),
        }
    }
    pub fn withdraw_loan(amount_gbp: f64, date: (i32, u32, u32)) -> Self {
        Transaction {
            from: &BANK,         // owed to self
            to: &DIRECTORS_LOAN, // owed by self
            amount_gbp_cent: to_cents(amount_gbp),
            date: NaiveDate::ymd(date.0, date.1, date.2),
        }
    }
    pub fn repay_loan(amount_gbp: f64, date: (i32, u32, u32)) -> Self {
        Transaction {
            from: &DIRECTORS_LOAN,
            to: &BANK,
            amount_gbp_cent: to_cents(amount_gbp),
            date: NaiveDate::ymd(date.0, date.1, date.2),
        }
    }
}

pub fn to_cents(amount: f64) -> u32 {
    (amount * 1000.0).trunc() as u32 / 10 // TODO test
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Account {
    code: u16,
    name: &'static str,
    details: &'static str,
}

pub trait NaiveDateExt {
    fn ymd(year: i32, month: u32, day: u32) -> Self;
}
impl NaiveDateExt for NaiveDate {
    fn ymd(year: i32, month: u32, day: u32) -> Self {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }
}

pub mod balance_sheet {
    use crate::models::std_accounts::*;
    use crate::models::{Account, Transaction};
    use std::collections::HashMap;

    #[derive(Debug)]
    pub struct SumBalances(pub HashMap<&'static Account, i32>);
    impl SumBalances {
        pub fn from_transactions(transactions: &[Transaction]) -> Self {
            let mut sum = SumBalances(HashMap::new());
            for t in transactions.iter() {
                sum.add_transaction(t);
            }
            sum
        }
        pub fn add_transaction(&mut self, transaction: &Transaction) {
            // taking money is called CREDIT
            // TODO prevent overflow
            *self.0.entry(transaction.from).or_default() -= transaction.amount_gbp_cent as i32;
            // adding money is called DEBIT
            // TODO prevent overflow
            *self.0.entry(transaction.to).or_default() += transaction.amount_gbp_cent as i32;
        }
        // pub fn get(&self, account: &'static Account) -> Balance {
        //     Balance {
        //         account,
        //         total_gbp_cents: self.0.get(account).copied().unwrap_or_default(),
        //     }
        // }
        pub fn asset(&self, account: &'static Account) -> Balance {
            Balance {
                account,
                total_gbp_cents: self.0.get(account).copied().unwrap_or_default(),
            }
        }
        pub fn liability(&self, account: &'static Account) -> Balance {
            // a positive account balance means we've moved company money INTO a liability account => money is owed to the company.
            //  => the liability is negative (a positive liability means the company owes money)
            Balance {
                account,
                total_gbp_cents: -self.0.get(account).copied().unwrap_or_default(),
            }
        }

        pub fn balance_sheet(&self) -> BalanceSheet {
            BalanceSheet {
                fixed_assets: vec![],
                current_assets: vec![self.asset(&BANK)],
                current_liabilities: vec![self.liability(&DIRECTORS_LOAN)],
                // equity: vec![],
            }
        }
    }
    #[derive(Debug)]
    pub struct Balance {
        pub account: &'static Account,
        pub total_gbp_cents: i32,
    }

    #[derive(Debug)]
    pub struct BalanceSheet {
        // Assets
        pub fixed_assets: Vec<Balance>,
        pub current_assets: Vec<Balance>,
        // Liabilities
        pub current_liabilities: Vec<Balance>,
        // pub equity: Vec<Balance>, // TODO calculate from rest instead
    }
    impl BalanceSheet {
        pub fn total_fixed_assets(&self) -> i32 {
            self.fixed_assets.iter().map(|b| b.total_gbp_cents).sum()
        }
        pub fn total_current_assets(&self) -> i32 {
            self.current_assets.iter().map(|b| b.total_gbp_cents).sum()
        }
        pub fn total_current_liabilities(&self) -> i32 {
            self.current_liabilities
                .iter()
                .map(|b| b.total_gbp_cents)
                .sum::<i32>()
        }
        pub fn total_all_assets(&self) -> i32 {
            self.total_current_assets() + self.total_fixed_assets()
        }
        pub fn equity(&self) -> i32 {
            self.total_all_assets() - self.total_current_liabilities()
        }
    }

    pub mod maybe_rm {
        pub trait BalanceSheetEntryKind {
            type AssetOrLiability: EitherAssetOrLiability;
        }

        pub enum Asset {}
        pub enum Liability {}
        pub trait EitherAssetOrLiability {}
        impl EitherAssetOrLiability for Asset {}
        impl EitherAssetOrLiability for Liability {}

        pub trait FixedAsset {}
        pub trait CurrentAsset {}
    }
}

pub mod profit_and_loss {
    use crate::models::std_accounts::*;
    use crate::models::{Account, Transaction};
    use std::collections::HashMap;

    #[derive(Default, Debug)]
    pub struct AccountTransactions(pub HashMap<&'static Account, Vec<Transaction>>); // TODO use AccountMovement with variants In/Out
    impl AccountTransactions {
        pub fn insert(&mut self, account: &'static Account, transaction: &Transaction) {
            self.0.entry(account).or_default().push(transaction.clone())
        }
        // TODO if needed take in account direction of movement In/Out
        pub fn total_abs(&self) -> u32 {
            self.0
                .iter()
                .map(|(_, v)| v.iter().map(|t| t.amount_gbp_cent).sum::<u32>())
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
        pub fn from_transactions(transactions: &[Transaction]) -> Self {
            let mut pl = ProfitAndLoss::default();
            for tx in transactions.iter() {
                pl.add_transaction(tx);
            }
            pl
        }
        pub fn add_transaction(&mut self, transaction: &Transaction) {
            if transaction.is_sale() {
                self.income.insert(&SALES, transaction);
            }
            if transaction.is_to_paye() {
                self.taxes.insert(&PAYE, transaction)
            }

            // TODO wages, NIC, corporation tax
        }

        // TODO rm once total is not abs
        pub fn total_income(&self) -> i32 {
            self.income.total_abs() as i32
        }
        // TODO rm once total is not abs
        pub fn total_direct_expenses(&self) -> i32 {
            self.direct_expenses.total_abs() as i32
        }
        pub fn gross_profit(&self) -> i32 {
            self.total_income() - self.total_direct_expenses()
        }

        pub fn net_profit(&self) -> i32 {
            self.gross_profit()
                - self.overheads.total_abs() as i32
                - self.financial_expenses.total_abs() as i32
                - self.taxes.total_abs() as i32
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

    impl Transaction {
        fn is_sale(&self) -> bool {
            *self.from == *SALES
        }
        fn is_to_paye(&self) -> bool {
            *self.to == *PAYE
        }
        fn _is_wage_net(&self) -> bool {
            *self.to == *WAGES_NET // TODO figure out WHAT TO DO WITH GROSS WAGES account
        }
    }
}
