use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::{prelude::FromPrimitive, Decimal};
use static_data::{AccountId, ALL_ACCOUNTS, BANK, DIRECTORS_LOAN, PAYE_PAID, SALES};
use std::collections::HashMap;
pub mod balance_sheet_2;
pub mod profit_and_loss;
pub mod static_data;

// TODO make this into trait implemented by SalesTransaction (also contains link/hash of associated invoice)
#[derive(Debug, Clone)]
pub struct Transaction {
    pub from: &'static Account,
    pub to: &'static Account,
    pub amount_gbp: Decimal,
    pub date: NaiveDate,
}
impl Transaction {
    pub fn sale(arg: impl Into<DateAndAmount>) -> Self {
        let arg = arg.into();
        Transaction {
            from: &SALES,
            to: &BANK,
            amount_gbp: arg.amount,
            date: arg.date,
        }
    }
    pub fn lend_to_director(arg: impl Into<DateAndAmount>) -> Self {
        let arg = arg.into();
        Transaction {
            from: &BANK,
            to: &DIRECTORS_LOAN,
            amount_gbp: arg.amount,
            date: arg.date,
        }
    }
    pub fn director_repays(arg: impl Into<DateAndAmount>) -> Self {
        let arg = arg.into();
        Transaction {
            from: &DIRECTORS_LOAN,
            to: &BANK,
            amount_gbp: arg.amount,
            date: arg.date,
        }
    }
    #[cfg(test)]
    pub fn company_borrows(arg: impl Into<DateAndAmount>) -> Self {
        let arg = arg.into();
        Transaction {
            from: &static_data::BORROW,
            to: &ALL_ACCOUNTS.bank(),
            amount_gbp: arg.amount,
            date: arg.date,
        }
    }
    #[cfg(test)]
    pub fn company_repays(amount_gbp: Decimal) -> Self {
        todo!()
    }
    pub fn salary(amount_gbp: Decimal, date: NaiveDate) -> Self {
        Transaction {
            // WAGES_GROSS is the total paid by employer (incl NIC + taxes), WAGES_NET is what is actually transferred to the employee
            from: &ALL_ACCOUNTS.bank(),
            to: &ALL_ACCOUNTS.wages_net(),
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
        Transaction {
            from: &BANK,
            to: &PAYE_PAID,
            amount_gbp,
            date,
        }
    }

    pub fn to_positive_direction(self) -> Self {
        if self.amount_gbp.is_sign_negative() {
            Transaction {
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

#[derive(PartialEq, Eq, Hash, Debug)]
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
}

#[derive(PartialEq, Eq, Hash, Debug)]
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

pub mod sheet3;

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
    pub fn with_asset(&mut self, asset: Asset) -> &mut Self {
        self.assets.insert(asset.id.clone(), asset);
        self
    }
    pub fn try_get(&self, id: &AssetId) -> anyhow::Result<&Asset> {
        self.assets
            .get(id)
            .ok_or(anyhow::anyhow!("Asset not found"))
    }
}
