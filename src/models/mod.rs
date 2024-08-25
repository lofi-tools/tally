use chrono::{DateTime, NaiveDate, Utc};
use file_cache::FileBytes;
use rust_decimal::{prelude::FromPrimitive, Decimal};
use serde::{Deserialize, Serialize};
use static_data::{AccountId, BANK, DIRECTORS_LOAN, PAYE_PAID, SALES};
use std::collections::HashMap;
use tx2::{Transaction2, TxOutput};
pub mod balance_sheet_2;
pub mod profit_and_loss;
pub mod sheet3;
pub mod static_data;

pub mod tx2 {
    use super::sheet3::AccountBalance2;
    use super::static_data::{AccountId, DIRECTORS_LOAN, NEXO_EUR, NEXO_GBP, PAYE_PAID, WAGES_NET};
    use crate::adapters::exchange_rates::{AssetPair, Currency, RatesApi, TimeRange};
    use crate::models::static_data::{BANK, CLIENTS};
    use crate::models::Account;
    use crate::models::DateAndAmount;
    use crate::utils::DatetimeUtcExt;
    use anyhow::anyhow;
    use chrono::{DateTime, Days, NaiveDate, Utc};
    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Transaction2 {
        pub outputs: Vec<TxOutput>,
        pub datetime: DateTime<Utc>,
    }
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct TxOutput {
        pub account_id: AccountId,
        pub amount_diff: Decimal,
        pub datetime: DateTime<Utc>,
    }
    impl Transaction2 {
        fn tx_to_from(
            dam: impl Into<DateAndAmount>,
            from: &'static Account,
            to: &'static Account,
        ) -> Transaction2 {
            let DateAndAmount { date, amount } = dam.into();
            Transaction2 {
                outputs: vec![
                    TxOutput {
                        account_id: from.id.clone(),
                        amount_diff: -amount,
                        datetime: DateTime::from_naive_date(date),
                    },
                    TxOutput {
                        account_id: to.id.clone(),
                        amount_diff: amount,
                        datetime: DateTime::from_naive_date(date),
                    },
                ],
                datetime: DateTime::from_naive_date(date),
            }
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
        pub async fn director_repays_nexo_eur(
            dam: impl Into<DateAndAmount>,
            rates_api: impl RatesApi,
        ) -> anyhow::Result<Transaction2> {
            let DateAndAmount {
                date,
                amount: amount_eur,
            } = dam.into();
            let datetime = DateTime::from_naive_date(date);

            const EUR_TO_GBP: AssetPair = AssetPair {
                from_currency: Currency::EUR,
                to_currency: Currency::GBP,
            };
            let rates_hist = rates_api
                .rate_hist(
                    &TimeRange::new(
                        datetime
                            .checked_sub_days(Days::new(1))
                            .unwrap()
                            .with_start_of_day(),
                        datetime
                            .checked_add_days(Days::new(3))
                            .unwrap()
                            .with_end_of_day(),
                    ),
                    &EUR_TO_GBP,
                )
                .await?;
            let pricepoint = rates_hist.max_rate()?;

            let amount_gbp = (amount_eur * pricepoint.rate).round_dp(2);

            // increase NEXO_EUR by EUR amount
            // decrease DIRECTORS_LOAN by GBP amount
            Ok(Transaction2 {
                outputs: vec![
                    TxOutput {
                        account_id: NEXO_EUR.id.clone(),
                        amount_diff: amount_eur,
                        datetime: datetime.clone(),
                    },
                    TxOutput {
                        account_id: DIRECTORS_LOAN.id.clone(),
                        amount_diff: -amount_gbp,
                        datetime: datetime.clone(),
                    },
                ],
                datetime,
            })
        }
        pub fn wage(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &BANK, &WAGES_NET)
        }
        pub fn paye(arg: impl Into<DateAndAmount>) -> Self {
            Self::tx_to_from(arg, &BANK, &PAYE_PAID)
        }

        pub fn date(&self) -> NaiveDate {
            self.datetime.date_naive()
        }
        pub fn loan_output(&self) -> anyhow::Result<&TxOutput> {
            let outputs_with_apy = self
                .outputs
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
        // pub fn interest_only_at(&self, date: NaiveDate) -> anyhow::Result<Decimal> {
        //     self.loan_output()?.interest_only_at(date)
        // }
        // pub fn balance_plus_interest_at(&self, date: NaiveDate) -> anyhow::Result<Decimal> {
        //     self.loan_output()?.balance_plus_interest_at(date)
        // }
        pub fn balance_at(&self, date: NaiveDate) -> anyhow::Result<AccountBalance2> {
            self.loan_output()?
                .balance_at(DateTime::from_naive_date(date))
        }
    }

    impl TxOutput {
        pub fn account(&self) -> anyhow::Result<&'static Account> {
            self.account_id.account()
        }
        pub fn loan_apy(&self) -> anyhow::Result<Decimal> {
            self.account()?
                .loan_apy
                .ok_or(anyhow!("Expected output account to have a loan APY"))
        }
        // #[deprecated]
        // pub fn balance_plus_interest_at(&self, date: NaiveDate) -> anyhow::Result<Decimal> {
        //     // TODO use AccountBalance to compute interest
        //     let apy = self.loan_apy()?;
        //     let apy_multiplier_per_year = Decimal::from(1)
        //         + apy
        //             / Decimal::from_f64(100.0)
        //                 .ok_or(anyhow::Error::msg("value can't be represented as Decimal"))?;
        //     let num_days = (date - self.datetime.date_naive()).num_days();
        //     if num_days.is_negative() {
        //         anyhow::bail!("trying to calculate interest before borrow date");
        //     }

        //     let num_days_d = Decimal::from_i64(num_days)
        //         .ok_or(anyhow!("num_days can't be represented as Decimal"))?;
        //     let balance_plus_interest =
        //         self.amount_diff * apy_multiplier_per_year.powd(num_days_d / Decimal::from(366));
        //     Ok(balance_plus_interest.trunc_with_scale(2))
        // }
        // pub fn interest_only_at(&self, date: NaiveDate) -> anyhow::Result<Decimal> {
        //     let interest = self.balance_plus_interest_at(date)? - self.amount_diff;
        //     Ok(interest.trunc_with_scale(2))
        // }
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

// #[derive(Debug, Clone)]
// pub struct BalanceChange {
//     pub date: NaiveDate,
//     pub amount: Decimal,
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

pub mod tx1 {
    use super::*;
    use static_data::WAGES_NET;

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
            use static_data::DB;

            let arg = arg.into();
            Transaction1 {
                from: &static_data::BORROW,
                to: &DB.accounts.bank(),
                amount_gbp: arg.amount,
                date: arg.date,
            }
        }
        // #[cfg(test)]
        // pub fn company_repays(amount_gbp: Decimal) -> Self {
        //     todo!()
        // }
        pub fn salary(amount_gbp: Decimal, date: NaiveDate) -> Self {
            Transaction1 {
                // WAGES_GROSS is the total paid by employer (incl NIC + taxes), WAGES_NET is what is actually transferred to the employee
                from: &BANK,
                to: &WAGES_NET,
                amount_gbp,
                date,
            }
        }
        // pub fn dividend(amount_gbp: Decimal, date: NaiveDate) -> Self {
        //     todo!()
        // }
        // pub fn employers_nic(amount_gbp: Decimal, date: NaiveDate) -> Self {
        //     todo!()
        // }
        // pub fn employee_nic(amount_gbp: Decimal, date: NaiveDate) -> Self {
        //     todo!()
        // }
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
