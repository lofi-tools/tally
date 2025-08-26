use chrono::{DateTime, NaiveDate, Utc};
use file_cache::FileBytes;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use serde::{Deserialize, Serialize};
use static_data::AccountId;
use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, LazyLock},
};
use tx2::{Transaction2, TxEffect};

use crate::utils::{MapExt, errors::AnyErr};

pub mod company;
pub mod profit_and_loss;
pub mod sheet3;
pub mod static_data;

pub mod tx2 {
    use super::sheet3::AccountBalance2;
    use super::static_data::{AccountId, DIRECTORS_LOAN, NEXO_GBP, PAYE_PAID, WAGES_NET};
    use super::{HasTxnDetails, TxnTag};
    use crate::models::Account;
    use crate::models::DateAndAmount;
    use crate::models::static_data::{BANK, CLIENTS, EXPENSES_TO_REPAY};
    use crate::utils::{DatetimeUtcExt, Loader};
    use anyhow::anyhow;
    use chrono::{DateTime, NaiveDate, Utc};
    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TxnId(pub String);

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Transaction2 {
        pub id: TxnId,
        pub effects: Vec<TxEffect>,
        pub datetime: DateTime<Utc>,
        pub tags: Vec<TxnTag>,
    }
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct TxEffect {
        pub txn_id: TxnId,
        pub account_id: AccountId,
        pub amount_diff: Decimal,
        pub datetime: DateTime<Utc>,
        pub tags: Loader<Vec<TxnTag>>,
    }
    impl Transaction2 {
        fn tx_to_from(
            dam: impl Into<DateAndAmount>,
            from: &'static Account,
            to: &'static Account,
        ) -> Transaction2 {
            let DateAndAmount { date, amount } = dam.into();
            let txn_id = TxnId(format!("{}->{}_{}_{}", from.id, to.id, date, amount));
            Transaction2 {
                id: txn_id.clone(),
                effects: vec![
                    TxEffect {
                        txn_id: txn_id.clone(),
                        account_id: from.id.clone(),
                        amount_diff: -amount,
                        datetime: DateTime::from_naive_date(date),
                        tags: Loader::None,
                    },
                    TxEffect {
                        txn_id: txn_id.clone(),
                        account_id: to.id.clone(),
                        amount_diff: amount,
                        datetime: DateTime::from_naive_date(date),
                        tags: Loader::None,
                    },
                ],
                datetime: DateTime::from_naive_date(date),
                tags: vec![],
            }
        }
        pub fn from_details(txn: impl HasTxnDetails) -> anyhow::Result<Transaction2> {
            let datetime = txn
                .datetime()
                .map_err(|e| anyhow!("Failed getting datetime: {e}"))?;
            let amount_pos = txn
                .asset_amount_positive()
                .map_err(|e| anyhow!("Failed getting asset amount positive: {e}"))?
                .amount_decimal()
                .map_err(|e| anyhow!("Failed getting asset amount positive: {e}"))?;
            let from_to = txn
                .from_to()
                .map_err(|e| anyhow!("Failed getting from/to accounts: {e}"))?;
            let tags = txn.tags()?;
            let txn_id = TxnId(format!(
                "{}->{}_{}_{}",
                from_to.0.id, from_to.1.id, datetime, amount_pos
            ));
            Ok(Transaction2 {
                id: txn_id.clone(),
                effects: vec![
                    TxEffect {
                        txn_id: txn_id.clone(),
                        account_id: from_to.0.id.clone(),
                        amount_diff: -amount_pos,
                        datetime: datetime.clone(),
                        tags: Loader::Loaded(tags.clone()),
                    },
                    TxEffect {
                        txn_id: txn_id.clone(),
                        account_id: from_to.1.id.clone(),
                        amount_diff: amount_pos,
                        datetime: datetime.clone(),
                        tags: Loader::Loaded(tags.clone()),
                    },
                ],
                datetime,
                tags,
            })
        }

        pub fn tx_to_bank_from(
            from: &'static Account,
            dam: impl Into<DateAndAmount>,
        ) -> Transaction2 {
            Self::tx_to_from(dam, from, &BANK)
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
        // pub async fn director_repays_nexo_eur(
        //     dam: impl Into<DateAndAmount>,
        //     rates_api: impl RatesApi,
        // ) -> anyhow::Result<Transaction2> {
        //     let DateAndAmount {
        //         date,
        //         amount: amount_eur,
        //     } = dam.into();
        //     let datetime = DateTime::from_naive_date(date);

        //     const EUR_TO_GBP: AssetPair = AssetPair {
        //         from_currency: Currency::EUR,
        //         to_currency: Currency::GBP,
        //     };
        //     let rates_hist = rates_api
        //         .rate_hist(
        //             &TimeRange::new(
        //                 datetime
        //                     .checked_sub_days(Days::new(1))
        //                     .unwrap()
        //                     .with_start_of_day(),
        //                 datetime
        //                     .checked_add_days(Days::new(3))
        //                     .unwrap()
        //                     .with_end_of_day(),
        //             ),
        //             &EUR_TO_GBP,
        //         )
        //         .await?;
        //     let pricepoint = rates_hist.max_rate()?;

        //     let amount_gbp = (amount_eur * pricepoint.rate).round_dp(2);

        //     Ok(Transaction2 {
        //         outputs: vec![
        //             TxOutput {
        //                 account_id: NEXO_EUR.id.clone(),
        //                 amount_diff: amount_eur,
        //                 datetime: datetime.clone(),
        //             },
        //             TxOutput {
        //                 account_id: DIRECTORS_LOAN.id.clone(),
        //                 amount_diff: -amount_gbp,
        //                 datetime: datetime.clone(),
        //             },
        //         ],
        //         datetime,
        //     })
        // }
        pub fn wage(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &BANK, &WAGES_NET)
        }
        pub fn paye(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &BANK, &PAYE_PAID)
        }

        pub fn date(&self) -> NaiveDate {
            self.datetime.date_naive()
        }
        pub fn loan_output(&self) -> anyhow::Result<&TxEffect> {
            let outputs_with_apy = self
                .effects
                .iter()
                .filter(|output| {
                    output
                        .account()
                        .ok()
                        .and_then(|account| account.loan_apy)
                        .is_some()
                })
                .collect::<Vec<_>>();
            if outputs_with_apy.len() != 1 {
                anyhow::bail!("Expected exactly one transaction output account to have a loan APY");
            }
            let account_with_apy = outputs_with_apy
                .into_iter()
                .next()
                .ok_or(anyhow!("Expected outputs to have 1 output with loan_apy"))?;
            Ok(account_with_apy)
        }
        pub fn loan_account(&self) -> anyhow::Result<&'static Account> {
            self.loan_output()?.account()
        }
        pub fn loan_apy(&self) -> anyhow::Result<Decimal> {
            self.loan_account()?
                .loan_apy
                .ok_or(anyhow!("Expected output account to have a loan APY"))
        }
        pub fn balance_at(&self, date: NaiveDate) -> anyhow::Result<AccountBalance2> {
            self.loan_output()?
                .balance_at(DateTime::from_naive_date(date))
        }

        // pub fn is_expense_or_future_expense(&self) -> bool {
        //     fn is_expense_account(account_id: &AccountId) -> anyhow::Result<bool> {
        //         Ok(account_id.account().account_type() == AccountType::Expense
        //             || account_id == &EXPENSES_TO_REPAY.id)
        //     }
        //     self.outputs.iter().any(|output| {
        //         output
        //             .account()
        //             .map(|account| is_expense_account(account)?)
        //             .unwrap_or(false)
        //     })
        // }
        pub fn is_expense_to_repay(&self) -> bool {
            self.effects.iter().any(|output| {
                output
                    .account()
                    .map(|account| account.id == EXPENSES_TO_REPAY.id)
                    .unwrap_or(false)
            })
        }
    }

    impl TxEffect {
        pub fn account(&self) -> anyhow::Result<&'static Account> {
            self.account_id.account()
        }
        pub fn loan_apy(&self) -> anyhow::Result<Decimal> {
            self.account()?
                .loan_apy
                .ok_or(anyhow!("Expected output account to have a loan APY"))
        }
        pub fn balance_at(&self, datetime: DateTime<Utc>) -> anyhow::Result<AccountBalance2> {
            self.balance_then().with_interest_at(datetime)
        }
        pub fn balance_then(&self) -> AccountBalance2 {
            AccountBalance2 {
                account_id: self.account_id.clone(),
                datetime: self.datetime,
                amount: self.amount_diff,
            }
        }
        pub fn is_expense_to_repay(&self) -> bool {
            self.account_id == EXPENSES_TO_REPAY.id
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct List<T> {
    pub items: Vec<T>,
}
impl<T> List<T> {
    pub fn new(items: Vec<T>) -> Self {
        List { items }
    }
}
impl std::ops::Deref for List<Transaction2> {
    type Target = Vec<Transaction2>;
    fn deref(&self) -> &Self::Target {
        &self.items
    }
}
impl FileBytes for List<Transaction2> {
    fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
    fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Deserialize, Debug)]
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
impl<F: Into<f64>> From<(F, (i32, u32, u32))> for DateAndAmount {
    fn from((amount, (y, m, d)): (F, (i32, u32, u32))) -> Self {
        DateAndAmount {
            amount: Decimal::from_f64(amount.into()).unwrap(),
            date: NaiveDate::from_ymd_opt(y, m, d).unwrap(),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub asset_id: AssetId,
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
    pub fn is_loan(&self) -> bool {
        self.loan_apy.is_some()
    }
    // pub fn increase(&self, amount_diff: Decimal) -> TxOutput {
    //     TxOutput {
    //         account_id: self.id.clone(),
    //         amount_diff,
    //     }
    // }
    // pub fn decrease(&self, amount_diff: Decimal) -> TxOutput {
    //     TxOutput {
    //         account_id: self.id.clone(),
    //         amount_diff: -amount_diff,
    //     }
    // }
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
    ExpensesToRepay,
    ExpensesPaid,
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
            AccountTag::ExpensesToRepay => AccountType::Liability,
            AccountTag::ExpensesPaid => AccountType::Expense,
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

// #[derive(Debug, Clone)]
// pub struct BalanceChange {
//     pub date: NaiveDate,
//     pub amount: Decimal,
// }

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct AssetId(pub Cow<'static, str>);
impl std::fmt::Display for AssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl From<&str> for AssetId {
    fn from(s: &str) -> Self {
        AssetId(Cow::Owned(s.to_string()))
    }
}

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

#[derive(Debug, Clone)]
pub enum Asset2 {
    Id(AssetId),
    Val {
        id: AssetId,
        decimals: u32,
        symbol: char,
        ticker: String,
    },
}
impl Asset2 {
    pub fn new(id: &str, decimals: u32, symbol: char, ticker: &str) -> Self {
        Asset2::Val {
            id: AssetId(Cow::Owned(id.to_string())),
            decimals,
            symbol,
            ticker: ticker.to_string(),
        }
    }
}
pub static ALL_ASSET2S: LazyLock<HashMap<AssetId, Asset2>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("USD".into(), Asset2::new("USD", 2, '$', "USD"));
    map.insert("EUR".into(), Asset2::new("EUR", 2, '€', "EUR"));
    map.insert("GBP".into(), Asset2::new("GBP", 2, '£', "GBP"));
    map
});
impl Asset2 {
    pub fn load(self) -> Result<Asset2, String> {
        match self {
            Asset2::Id(id) => Ok(ALL_ASSET2S.try_get(&id).unwrap().clone()),
            Asset2::Val { .. } => Ok(self),
        }
    }
    pub fn decimals(&self) -> Result<u32, String> {
        match self {
            Asset2::Id(_) => self.clone().load()?.decimals(),
            Asset2::Val { decimals, .. } => Ok(*decimals),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AssetAmount2 {
    pub asset: Arc<Asset2>,
    amount: u64,
}
impl AssetAmount2 {
    pub fn new(asset: Arc<Asset2>, amount: u64) -> Self {
        AssetAmount2 { asset, amount }
    }
    pub fn amount_decimal(&self) -> Result<Decimal, String> {
        Ok(Decimal::from(self.amount) / Decimal::from(10u64.pow(self.asset.decimals()?)))
    }
}

pub trait HasDatetime {
    fn datetime(&self) -> Result<DateTime<Utc>, String>;
}
pub trait HasAssetAmount {
    fn asset_amount_positive(&self) -> Result<AssetAmount2, String>;
}

/// Implement either from_to or from_account/to_account. TODO define 2 traits inherited by a third trait
pub trait HasFromTo {
    fn from_to(&self) -> Result<(&Account, &Account), String> {
        self.__inner_from_to(0)
    }
    fn from_account(&self) -> Result<&Account, String> {
        self.__inner_from_account(0)
    }
    fn to_account(&self) -> Result<&Account, String> {
        self.__inner_to_account(0)
    }

    fn __inner_from_account(&self, counter: u8) -> Result<&Account, String> {
        self.__inner_check_counter(counter)?;

        self.from_to().map(|(a, _)| a)
    }
    fn __inner_to_account(&self, counter: u8) -> Result<&Account, String> {
        self.__inner_check_counter(counter)?;

        self.__inner_from_to(counter + 1).map(|(_, a)| a)
    }
    fn __inner_from_to(&self, counter: u8) -> Result<(&Account, &Account), String> {
        self.__inner_check_counter(counter)?;

        let from = self.__inner_from_account(counter + 1)?;
        let to = self.__inner_to_account(counter + 1)?;
        Ok((from, to))
    }
    fn __inner_check_counter(&self, counter: u8) -> Result<(), String> {
        if counter > 1 {
            return Err(format!(
                "Bad implementation of HasFromTo: Too many levels of indirection"
            ));
        }
        Ok(())
    }
}
pub trait HasTxnDetails: HasDatetime + HasAssetAmount + HasFromTo + AddTags {}
impl<T> HasTxnDetails for T where T: HasDatetime + HasAssetAmount + HasFromTo + AddTags {} // auto-implement for all

pub trait AddTags {
    fn tags(&self) -> Result<Vec<TxnTag>, AnyErr>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnTag {
    pub name: String,
}
impl TxnTag {
    pub fn arc(name: &str) -> Arc<Self> {
        Arc::new(Self {
            name: name.to_string(),
        })
    }
    pub fn from_str(s: &str) -> Result<Arc<Self>, AnyErr> {
        TXN_TAGS.try_get(s)
    }
}

pub struct AllStaticTxnTags(pub LazyLock<HashMap<&'static str, Arc<TxnTag>>>);
impl AllStaticTxnTags {
    pub fn try_get<'a>(&'a self, s: &str) -> Result<Arc<TxnTag>, AnyErr> {
        let res = self
            .0
            .get(&s)
            .ok_or(format!("No txn tag found for {s}").into());
        res.map(|t| t.clone())
    }
    pub fn refc(&self, name: &str) -> Result<TxnTag, AnyErr> {
        self.try_get(name).map(|t| (*t).clone())
    }
}
pub const TXN_TAGS: AllStaticTxnTags = AllStaticTxnTags(LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("Income", TxnTag::arc("Income"));
    map.insert("DirectorBorrows", TxnTag::arc("DirectorBorrows"));
    map.insert("DirectorRepays", TxnTag::arc("DirectorRepays"));
    map.insert("PayWagesNet", TxnTag::arc("PayWagesNet"));
    map.insert("PayPaye", TxnTag::arc("PayPaye"));
    map.insert("ExpenseToReimburse", TxnTag::arc("ExpenseToReimburse"));
    map.insert("ReimburseExpense", TxnTag::arc("ReimburseExpense"));
    map.insert("PayCorporateTax", TxnTag::arc("PayCorporateTax"));
    map
}));

// pub enum EnumTxnTags {
//     Income,
//     DirectorsBorrows,
//     DirectorRepays,
//     PayPaye,
//     ReimburseExpense,
//     PayCorporateTax,
// }
// impl EnumTxnTags {

// }

// pub mod _old {
//     pub mod tx1 {
//         use crate::adapters::banks::nordigen_client::BookedTransaction;
//         use crate::models::static_data::{BANK, DIRECTORS_LOAN, PAYE_PAID, SALES, WAGES_NET};
//         use crate::models::{Account, DateAndAmount};
//         use chrono::NaiveDate;
//         use rust_decimal::Decimal;

//         // TODO make this into trait implemented by SalesTransaction (also contains link/hash of associated invoice)
//         #[derive(Debug, Clone)]
//         pub struct Transaction1 {
//             pub from: &'static Account,
//             pub to: &'static Account,
//             pub amount_gbp: Decimal,
//             pub date: NaiveDate,
//         }
//         impl Transaction1 {
//             pub fn sale(arg: impl Into<DateAndAmount>) -> Self {
//                 let arg = arg.into();
//                 Transaction1 {
//                     from: &SALES,
//                     to: &BANK,
//                     amount_gbp: arg.amount,
//                     date: arg.date,
//                 }
//             }
//             pub fn lend_to_director(arg: impl Into<DateAndAmount>) -> Self {
//                 let arg = arg.into();
//                 Transaction1 {
//                     from: &BANK,
//                     to: &DIRECTORS_LOAN,
//                     amount_gbp: arg.amount,
//                     date: arg.date,
//                 }
//             }
//             pub fn director_repays(arg: impl Into<DateAndAmount>) -> Self {
//                 let arg = arg.into();
//                 Transaction1 {
//                     from: &DIRECTORS_LOAN,
//                     to: &BANK,
//                     amount_gbp: arg.amount,
//                     date: arg.date,
//                 }
//             }
//             #[cfg(test)]
//             pub fn company_borrows(arg: impl Into<DateAndAmount>) -> Self {
//                 use crate::models::static_data::{self, DB};

//                 let arg = arg.into();
//                 Transaction1 {
//                     from: &static_data::BORROW,
//                     to: &DB.accounts.bank(),
//                     amount_gbp: arg.amount,
//                     date: arg.date,
//                 }
//             }
//             // #[cfg(test)]
//             // pub fn company_repays(amount_gbp: Decimal) -> Self {
//             //     todo!()
//             // }
//             pub fn salary(amount_gbp: Decimal, date: NaiveDate) -> Self {
//                 Transaction1 {
//                     // WAGES_GROSS is the total paid by employer (incl NIC + taxes), WAGES_NET is what is actually transferred to the employee
//                     from: &BANK,
//                     to: &WAGES_NET,
//                     amount_gbp,
//                     date,
//                 }
//             }
//             // pub fn dividend(amount_gbp: Decimal, date: NaiveDate) -> Self {
//             //     todo!()
//             // }
//             // pub fn employers_nic(amount_gbp: Decimal, date: NaiveDate) -> Self {
//             //     todo!()
//             // }
//             // pub fn employee_nic(amount_gbp: Decimal, date: NaiveDate) -> Self {
//             //     todo!()
//             // }
//             pub fn to_paye(amount_gbp: Decimal, date: NaiveDate) -> Self {
//                 Transaction1 {
//                     from: &BANK,
//                     to: &PAYE_PAID,
//                     amount_gbp,
//                     date,
//                 }
//             }

//             pub fn to_positive_direction(self) -> Self {
//                 if self.amount_gbp.is_sign_negative() {
//                     Transaction1 {
//                         from: self.to,
//                         to: self.from,
//                         amount_gbp: self.amount_gbp.abs(),
//                         date: self.date,
//                     }
//                 } else {
//                     self
//                 }
//             }
//         }
//         // impl Transaction1 {
//         //     fn _from_starling(starling_tx: &BookedTransaction) -> anyhow::Result<Transaction1> {
//         //         match starling_tx {
//         //             tx if tx.debtor_name == Some("Nigel Frank Intern".to_string()) => {
//         //                 Ok(Transaction1::to_bank(starling_tx, &SALES))
//         //             }
//         //             tx if tx.remittance_information_unstructured.contains("Salary") => {
//         //                 Ok(Transaction1::to_bank(starling_tx, &WAGES_NET))
//         //             }
//         //             tx if tx
//         //                 .remittance_information_unstructured
//         //                 .contains("Director's loan") =>
//         //             {
//         //                 Ok(Transaction1::to_bank(starling_tx, &DIRECTORS_LOAN))
//         //             }
//         //             tx if tx
//         //                 .remittance_information_unstructured
//         //                 .contains("120PZ028811752312") =>
//         //             {
//         //                 Ok(Transaction1::to_bank(starling_tx, &PAYE_PAID))
//         //             }
//         //             other_tx => return Err(anyhow::anyhow!("no match for tx: {other_tx:?}")),
//         //         }
//         //     }

//         //     fn to_bank(
//         //         nordigen_tx: &BookedTransaction,
//         //         from_account: &'static Account,
//         //     ) -> Transaction1 {
//         //         Transaction1 {
//         //             from: from_account,
//         //             to: &BANK, // all nordigen transactions are to the bank: positive amounts for incoming, negative amounts for outgoing
//         //             amount_gbp: nordigen_tx.amount.amount,
//         //             date: nordigen_tx.booking_date,
//         //         }
//         //         .to_positive_direction() // this reverses the direction of the transaction so that amounts are positive
//         //     }
//         // }
//     }

//     pub mod balance_sheet_2 {
//         use super::tx1::Transaction1;
//         use crate::models::{Account, AccountType};
//         use rust_decimal::Decimal;
//         use std::collections::HashMap;

//         #[derive(Debug, Clone, Copy)]
//         pub struct AccountBalance {
//             pub account: &'static Account,
//             pub balance: Decimal,
//         }
//         impl AccountBalance {
//             pub fn new(account: &'static Account) -> Self {
//                 Self {
//                     account,
//                     balance: Decimal::default(),
//                 }
//             }
//             pub fn remove_money_from(&mut self, amount: Decimal) {
//                 match self.account.account_type() {
//                     AccountType::Asset => self.balance -= amount,
//                     AccountType::Liability => self.balance += amount,
//                     AccountType::Revenue => self.balance += amount,
//                     AccountType::Expense => self.balance -= amount,
//                     AccountType::Equity => self.balance -= amount,
//                 }
//             }
//             pub fn add_money_to(&mut self, amount: Decimal) {
//                 match self.account.account_type() {
//                     AccountType::Asset => self.balance += amount,
//                     AccountType::Liability => self.balance -= amount,
//                     AccountType::Revenue => self.balance -= amount,
//                     AccountType::Expense => self.balance += amount,
//                     AccountType::Equity => self.balance += amount,
//                 }
//             }
//             pub fn set_to(&mut self, amount: Decimal) {
//                 self.balance = amount;
//             }
//         }

//         #[derive(Default, Debug)]
//         pub struct BalanceSheet2 {
//             pub accounts: HashMap<&'static Account, AccountBalance>,
//         }
//         impl BalanceSheet2 {
//             pub fn from_transactions1(transactions: &[Transaction1]) -> Self {
//                 let mut bs = BalanceSheet2::default();
//                 for tx in transactions.iter() {
//                     bs.add_transaction1(tx);
//                 }
//                 bs
//             }
//             pub fn account_mut(&mut self, account: &'static Account) -> &mut AccountBalance {
//                 self.accounts
//                     .entry(account)
//                     .or_insert_with(|| AccountBalance::new(account))
//             }
//             pub fn add_transaction1(&mut self, transaction: &Transaction1) -> &mut Self {
//                 self.account_mut(transaction.from)
//                     .remove_money_from(transaction.amount_gbp);

//                 self.account_mut(transaction.to)
//                     .add_money_to(transaction.amount_gbp);

//                 self.mk_balanced();
//                 self
//             }
//             pub fn mk_balanced(&mut self) -> &mut Self {
//                 let total_assets = self.total_of(AccountType::Asset);
//                 let total_liabilities = self.total_of(AccountType::Liability);

//                 let _retained_earnings = total_assets - total_liabilities;
//                 // self.account_mut(&RETAINED_EARNINGS)
//                 //     .set_to(retained_earnings);

//                 self
//             }

//             pub fn accounts_of_type(&self, account_type: AccountType) -> Vec<AccountBalance> {
//                 self.accounts
//                     .values()
//                     .filter(|a| a.account.account_type() == account_type)
//                     .cloned()
//                     .collect()
//             }
//             pub fn total_of(&self, account_type: AccountType) -> Decimal {
//                 self.accounts_of_type(account_type)
//                     .iter()
//                     .map(|a| a.balance)
//                     .sum()
//             }

//             // pub fn total_assets(&self) -> Decimal {
//             //     self.accounts_of_type(AccountType::Asset)
//             //         .iter()
//             //         .map(|a| a.balance)
//             //         .sum()
//             // }
//             // pub fn total_liabilities(&self) -> Decimal {
//             //     self.accounts_of_type(AccountType::Liability)
//             //         .iter()
//             //         .map(|a| a.balance)
//             //         .sum()
//             // }
//             pub fn total_liabilities_and_equity(&self) -> Decimal {
//                 self.accounts_of_type(AccountType::Liability)
//                     .iter()
//                     .chain(self.accounts_of_type(AccountType::Equity).iter())
//                     .map(|a| a.balance)
//                     .sum()
//             }
//         }
//         // impl std::fmt::Display for BalanceSheet2 {
//         //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         //         // TODO list assetBalances, liabilityBalances, equity
//         //         write!(
//         //             f,
//         //             "Assets: {}\nLiabilities: {}\nEquity: {}",
//         //             todo!(),
//         //             todo!(),
//         //             todo!()
//         //         )
//         //     }
//         // }

//         #[cfg(test)]
//         mod tests {
//             use super::*;
//             use crate::models::static_data::{BANK, BORROW, DIRECTORS_LOAN, SALES};

//             #[test]
//             fn test_balance_sheet_directors_loan() -> anyhow::Result<()> {
//                 let mut bs = BalanceSheet2::from_transactions1(&[]);
//                 assert_eq!(bs.accounts.len(), 0);

//                 bs.add_transaction1(&Transaction1::sale(1000_f64));
//                 assert_eq!(bs.accounts[&*BANK].balance, 1000.into());
//                 assert_eq!(bs.accounts[&*SALES].balance, 1000.into());

//                 bs.add_transaction1(&Transaction1::lend_to_director(1000_f64));
//                 assert_eq!(bs.accounts[&*BANK].balance, 0.into());
//                 assert_eq!(bs.accounts[&*DIRECTORS_LOAN].balance, 1000.into());

//                 bs.add_transaction1(&Transaction1::director_repays(1000_f64));
//                 assert_eq!(bs.accounts[&*BANK].balance, 1000.into());
//                 assert_eq!(bs.accounts[&*DIRECTORS_LOAN].balance, 0.into());

//                 Ok(())
//             }
//             #[test]
//             fn test_balance_sheet_company_borrows() -> anyhow::Result<()> {
//                 let mut bs = BalanceSheet2::default();
//                 bs.add_transaction1(&Transaction1::company_borrows(1000_f64));

//                 assert_eq!(bs.accounts[&*BANK].balance, 1000.into());
//                 assert_eq!(bs.accounts[&*BORROW].balance, 1000.into());
//                 // assert_eq!(bs.accounts.try_get(&&RETAINED_EARNINGS)?.balance, 0.into()); // TODO use function to get earnings, not an account
//                 Ok(())
//             }
//         }

//         pub mod balance_sheet_old {
//             // use crate::models::std_accounts::*;
//             // use crate::models::{Account, Transaction};
//             // use rust_decimal::Decimal;
//             // use std::collections::HashMap;

//             // #[derive(Debug)]
//             // pub struct BalanceTotals(pub HashMap<&'static Account, Decimal>);
//             // impl BalanceTotals {
//             //     pub fn from_transactions(transactions: &[Transaction]) -> Self {
//             //         let mut sum = BalanceTotals(HashMap::new());
//             //         for t in transactions.iter() {
//             //             sum.add_transaction(t);
//             //         }
//             //         sum
//             //     }
//             //     pub fn add_transaction(&mut self, transaction: &Transaction) {
//             //         // taking money is called CREDIT
//             //         // TODO prevent overflow
//             //         *self.0.entry(transaction.from).or_default() -= transaction.amount_gbp;
//             //         // adding money is called DEBIT
//             //         // TODO prevent overflow
//             //         *self.0.entry(transaction.to).or_default() += transaction.amount_gbp;
//             //     }
//             //     // pub fn get(&self, account: &'static Account) -> Balance {
//             //     //     Balance {
//             //     //         account,
//             //     //         total_gbp_cents: self.0.get(account).copied().unwrap_or_default(),
//             //     //     }
//             //     // }
//             //     pub fn asset(&self, account: &'static Account) -> Balance {
//             //         Balance {
//             //             account,
//             //             total_gbp: self.0.get(account).copied().unwrap_or_default(),
//             //         }
//             //     }
//             //     pub fn liability(&self, account: &'static Account) -> Balance {
//             //         // a positive account balance means we've moved company money INTO a liability account => money is owed to the company.
//             //         //  => the liability is negative (a positive liability means the company owes money)
//             //         Balance {
//             //             account,
//             //             total_gbp: -self.0.get(account).copied().unwrap_or_default(),
//             //         }
//             //     }

//             //     pub fn balance_sheet(&self) -> BalanceSheet {
//             //         BalanceSheet {
//             //             fixed_assets: vec![],
//             //             current_assets: vec![self.asset(&BANK)],
//             //             current_liabilities: vec![self.liability(&DIRECTORS_LOAN)],
//             //             // equity: vec![],
//             //         }
//             //     }
//             // }
//             // #[derive(Debug)]
//             // pub struct Balance {
//             //     pub account: &'static Account,
//             //     pub total_gbp: Decimal,
//             // }

//             // #[derive(Debug)]
//             // pub struct BalanceSheet {
//             //     // Assets
//             //     pub fixed_assets: Vec<Balance>,
//             //     pub current_assets: Vec<Balance>,
//             //     // Liabilities
//             //     pub current_liabilities: Vec<Balance>,
//             //     // pub equity: Vec<Balance>, // TODO calculate from rest instead
//             // }
//             // impl BalanceSheet {
//             //     pub fn total_fixed_assets(&self) -> Decimal {
//             //         self.fixed_assets.iter().map(|b| b.total_gbp).sum()
//             //     }
//             //     pub fn total_current_assets(&self) -> Decimal {
//             //         self.current_assets.iter().map(|b| b.total_gbp).sum()
//             //     }
//             //     pub fn total_current_liabilities(&self) -> Decimal {
//             //         self.current_liabilities
//             //             .iter()
//             //             .map(|b| b.total_gbp)
//             //             .sum::<Decimal>()
//             //     }
//             //     pub fn total_all_assets(&self) -> Decimal {
//             //         self.total_current_assets() + self.total_fixed_assets()
//             //     }
//             //     pub fn equity(&self) -> Decimal {
//             //         self.total_all_assets() - self.total_current_liabilities()
//             //     }
//             // }
//         }
//     }

//     mod profit_and_loss_1 {
//         use super::tx1::Transaction1;
//         use crate::models::{
//             static_data::{PAYE_PAID, SALES, WAGES_NET},
//             Account,
//         };
//         use rust_decimal::Decimal;
//         use std::collections::HashMap;

//         #[derive(Default, Debug)]
//         pub struct AccountTransactions(pub HashMap<&'static Account, Vec<Transaction1>>); // TODO use AccountMovement with variants In/Out
//         impl AccountTransactions {
//             pub fn insert(&mut self, account: &'static Account, transaction: &Transaction1) {
//                 self.0.entry(account).or_default().push(transaction.clone())
//             }
//             // TODO if needed take in account direction of movement In/Out
//             pub fn total_abs(&self) -> Decimal {
//                 self.0
//                     .iter()
//                     .map(|(_, v)| v.iter().map(|t| t.amount_gbp).sum::<Decimal>())
//                     .sum()
//             }
//         }

//         #[derive(Default, Debug)]
//         pub struct ProfitAndLoss {
//             // gross
//             pub income: AccountTransactions,
//             pub direct_expenses: AccountTransactions,
//             // pub gross_profit_and_loss: i32,

//             // net
//             pub overheads: AccountTransactions,
//             pub financial_expenses: AccountTransactions,
//             pub taxes: AccountTransactions,
//             pub net_profit_and_loss: i32,
//         }

//         impl ProfitAndLoss {
//             pub fn from_transactions1(transactions: &[Transaction1]) -> Self {
//                 let mut pl = ProfitAndLoss::default();
//                 for tx in transactions.iter() {
//                     pl.add_transaction1(tx);
//                 }
//                 pl
//             }
//             pub fn add_transaction1(&mut self, transaction: &Transaction1) {
//                 if transaction.is_sale() {
//                     self.income.insert(&SALES, transaction);
//                 }
//                 if transaction.is_to_paye() {
//                     self.taxes.insert(&PAYE_PAID, transaction)
//                 }
//             }

//             pub fn total_income(&self) -> Decimal {
//                 self.income.total_abs()
//             }
//             pub fn total_direct_expenses(&self) -> Decimal {
//                 self.direct_expenses.total_abs()
//             }
//             pub fn gross_profit(&self) -> Decimal {
//                 self.total_income() - self.total_direct_expenses()
//             }

//             pub fn net_profit(&self) -> Decimal {
//                 self.gross_profit()
//                     - self.overheads.total_abs()
//                     - self.financial_expenses.total_abs()
//                     - self.taxes.total_abs()
//             }
//         }
//         impl std::fmt::Display for ProfitAndLoss {
//             fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//                 write!(f, "Income: \t\t\t{:?}\n", self.income)?;
//                 write!(f, "Expenses: \t\t\t{:?}\n", self.direct_expenses)?;
//                 write!(f, "Gross profit / loss: \t\t{}\n", self.gross_profit())?;
//                 write!(f, "\n")?;

//                 write!(f, "Overheads: \t\t\t{:?}\n", self.overheads)?;
//                 write!(f, "Financial expenses: \t\t{:?}\n", self.financial_expenses)?;
//                 write!(f, "Taxes: \t\t\t\t{:?}\n", self.taxes)?;
//                 write!(f, "Net profit / loss: \t\t{}\n\n", self.net_profit())?;
//                 Ok(())
//             }
//         }

//         impl Transaction1 {
//             fn is_sale(&self) -> bool {
//                 self.from == *SALES
//             }
//             fn is_to_paye(&self) -> bool {
//                 self.to == *PAYE_PAID
//             }
//             fn _is_wage_net(&self) -> bool {
//                 self.to == *WAGES_NET // TODO figure out WHAT TO DO WITH GROSS WAGES account
//             }
//         }
//     }
// }
