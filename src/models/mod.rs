use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::{prelude::FromPrimitive, Decimal};
use static_data::{AccountId, BANK, DIRECTORS_LOAN, PAYE_PAID, SALES};
use std::collections::HashMap;
use tx2::TxOutput;
pub mod balance_sheet_2;
pub mod profit_and_loss;
pub mod sheet3;
pub mod static_data;

pub mod tx2 {
    use super::static_data::{AccountId, DIRECTORS_LOAN, NEXO_EUR, NEXO_GBP, PAYE_PAID, WAGES_NET};
    use crate::models::static_data::{BANK, CLIENTS};
    use crate::models::Account;
    use crate::models::DateAndAmount;
    use crate::utils::DatetimeUtcExt;
    use chrono::{DateTime, Utc};
    use rust_decimal::Decimal;

    #[derive(Debug, Clone)]
    pub struct Transaction2 {
        pub outputs: Vec<TxOutput>,
        pub datetime: DateTime<Utc>,
    }
    #[derive(Debug, Clone)]
    pub struct TxOutput {
        pub account_id: AccountId,
        pub amount_diff: Decimal,
    }

    impl Transaction2 {
        fn tx_to_from(
            dam: impl Into<DateAndAmount>,
            from: &'static Account,
            to: &'static Account,
        ) -> Transaction2 {
            let DateAndAmount { date, amount } = dam.into();
            Transaction2 {
                outputs: vec![from.decrease(amount), to.increase(amount)],
                datetime: DateTime::from_naive_date(date),
            }
        }
        pub fn sale(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &CLIENTS, &BANK)
        }
        pub fn director_borrows_gbp(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &BANK, &DIRECTORS_LOAN)
        }
        pub fn director_repays_bank_gbp(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &DIRECTORS_LOAN, &BANK)
        }
        pub fn director_repays_nexo_gbp(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &DIRECTORS_LOAN, &NEXO_GBP)
        }
        pub fn director_repays_nexo_eur(arg: impl Into<DateAndAmount>) -> Self {
            // TODO convert EUR to GBP
            // increase NEXO_EUR by EUR amount
            // decrease DIRECTORS_LOAN by GBP amount
            todo!()
        }
        pub fn wage(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &BANK, &WAGES_NET)
        }
        pub fn paye(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &BANK, &PAYE_PAID)
        }
    }
}

pub mod tx1 {
    use super::*;
    use static_data::{DB, WAGES_NET};

    // TODO make this into trait implemented by SalesTransaction (also contains link/hash of associated invoice)
    #[derive(Debug, Clone)]
    pub struct Transaction1 {
        pub from: &'static Account,
        pub to: &'static Account,
        pub amount_gbp: Decimal,
        pub date: NaiveDate,
    }
    impl Transaction1 {
        pub fn sale(arg: impl Into<DateAndAmount>) -> Self {
            let arg = arg.into();
            Transaction1 {
                from: &SALES,
                to: &BANK,
                amount_gbp: arg.amount,
                date: arg.date,
            }
        }
        pub fn lend_to_director(arg: impl Into<DateAndAmount>) -> Self {
            let arg = arg.into();
            Transaction1 {
                from: &BANK,
                to: &DIRECTORS_LOAN,
                amount_gbp: arg.amount,
                date: arg.date,
            }
        }
        pub fn director_repays(arg: impl Into<DateAndAmount>) -> Self {
            let arg = arg.into();
            Transaction1 {
                from: &DIRECTORS_LOAN,
                to: &BANK,
                amount_gbp: arg.amount,
                date: arg.date,
            }
        }
        #[cfg(test)]
        pub fn company_borrows(arg: impl Into<DateAndAmount>) -> Self {
            let arg = arg.into();
            Transaction1 {
                from: &static_data::BORROW,
                to: &DB.accounts.bank(),
                amount_gbp: arg.amount,
                date: arg.date,
            }
        }
        #[cfg(test)]
        pub fn company_repays(amount_gbp: Decimal) -> Self {
            todo!()
        }
        pub fn salary(amount_gbp: Decimal, date: NaiveDate) -> Self {
            Transaction1 {
                // WAGES_GROSS is the total paid by employer (incl NIC + taxes), WAGES_NET is what is actually transferred to the employee
                from: &BANK,
                to: &WAGES_NET,
                amount_gbp,
                date,
            }
        }
        pub fn dividend(amount_gbp: Decimal, date: NaiveDate) -> Self {
            todo!()
        }
        pub fn employers_nic(amount_gbp: Decimal, date: NaiveDate) -> Self {
            todo!()
        }
        pub fn employee_nic(amount_gbp: Decimal, date: NaiveDate) -> Self {
            todo!()
        }
        pub fn to_paye(amount_gbp: Decimal, date: NaiveDate) -> Self {
            Transaction1 {
                from: &BANK,
                to: &PAYE_PAID,
                amount_gbp,
                date,
            }
        }

        pub fn to_positive_direction(self) -> Self {
            if self.amount_gbp.is_sign_negative() {
                Transaction1 {
                    from: self.to,
                    to: self.from,
                    amount_gbp: self.amount_gbp.abs(),
                    date: self.date,
                }
            } else {
                self
            }
        }
    }
}

pub struct DateAndAmount {
    pub date: NaiveDate,
    pub amount: Decimal,
}
impl From<f64> for DateAndAmount {
    fn from(amount: f64) -> Self {
        DateAndAmount {
            amount: Decimal::from_f64(amount).unwrap(),
            date: Utc::now().date_naive(),
        }
    }
}
impl From<(f64, DateTime<Utc>)> for DateAndAmount {
    fn from((amount, date): (f64, DateTime<Utc>)) -> Self {
        DateAndAmount {
            amount: Decimal::from_f64(amount).unwrap(),
            date: date.date_naive(),
        }
    }
}
impl From<(u32, DateTime<Utc>)> for DateAndAmount {
    fn from((amount, date): (u32, DateTime<Utc>)) -> Self {
        DateAndAmount {
            amount: Decimal::from_u32(amount).unwrap(),
            date: date.date_naive(),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub asset: AssetId,
    pub code: u16,
    pub name: &'static str,
    pub details: &'static str,
    pub account_tag: AccountTag,
    pub loan_apy: Option<Decimal>,
}
impl Account {
    pub fn account_type(&self) -> AccountType {
        self.account_tag.account_type()
    }
    pub fn increase(&self, amount_diff: Decimal) -> TxOutput {
        TxOutput {
            account_id: self.id,
            amount_diff,
        }
    }
    pub fn decrease(&self, amount_diff: Decimal) -> TxOutput {
        TxOutput {
            account_id: self.id,
            amount_diff: -amount_diff,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum AccountTag {
    Sales,
    Cash,
    Currency,
    AccountsReceivable,
    LoansReceivable,
    AccountsPayable,
    LoansPayable,
    TaxesPayable,
    Investments,
    Expenses,
    RetainedEarnings,
    Equity,
}
impl AccountTag {
    pub fn account_type(&self) -> AccountType {
        match self {
            AccountTag::Sales => AccountType::Revenue,
            AccountTag::Cash => AccountType::Asset,
            AccountTag::Currency => AccountType::Asset,
            AccountTag::AccountsReceivable => AccountType::Asset,
            AccountTag::LoansReceivable => AccountType::Asset,
            AccountTag::AccountsPayable => AccountType::Liability,
            AccountTag::LoansPayable => AccountType::Liability,
            AccountTag::TaxesPayable => AccountType::Liability,
            AccountTag::Investments => AccountType::Asset,
            AccountTag::Expenses => AccountType::Expense,
            AccountTag::RetainedEarnings => AccountType::Equity,
            AccountTag::Equity => AccountType::Equity,
        }
    }
    pub fn is_asset(&self) -> bool {
        return self.account_type() == AccountType::Asset;
    }
    pub fn is_liability(&self) -> bool {
        return self.account_type() == AccountType::Liability;
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum AccountType {
    Asset,
    Liability,
    Revenue,
    Expense,
    Equity,
}

#[derive(Debug, Clone)]
pub struct BalanceChange {
    pub date: NaiveDate,
    pub amount: Decimal,
}

// pub mod loan {
//     use super::{Asset, BalanceChange};
//     use rust_decimal::Decimal;

//     /// A Loan is money being lent out. It's an asset to the company
//     pub struct Loan {
//         pub asset: &'static Asset,
//         pub interest_rate: Decimal,
//         pub balance_changes: Vec<BalanceChange>,
//     }
// }

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct AssetId(pub &'static str);

#[derive(Debug, Clone)]
pub struct Asset {
    pub id: AssetId,
}

#[derive(Default, Debug, Clone)]
pub struct AllAssets {
    assets: HashMap<AssetId, Asset>,
}
impl AllAssets {
    pub fn with_asset(mut self, asset: Asset) -> Self {
        self.assets.insert(asset.id.clone(), asset);
        self
    }
    pub fn try_get(&self, id: &AssetId) -> anyhow::Result<&Asset> {
        self.assets
            .get(id)
            .ok_or(anyhow::anyhow!("Asset not found"))
    }
}
