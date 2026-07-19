use super::company::BalanceSheet4;
use super::profit_and_loss::{Outputs, ProfitAndLoss};
use super::static_data::HasAccountId;
use super::*;
use crate::adapters::exchange_rates::models::DayPricePoint;
use crate::adapters::exchange_rates::{GBP_EUR_PAIR, RATES_API, RatesApi, TimeRange};
use crate::utils::errors::{AnyhowResExt, ResultExt};
use crate::utils::{DateRange, DatetimeUtcExt, NumExt};
use crate::{Expenses, ListTxns};
use anyhow::anyhow;
use rust_decimal::MathematicalOps;
use static_data::{EUR, GBP};
use tx2::Transaction2;

// TODO recalculate interest on AccountBalanceChanges: go through transaction dates in order

#[derive(Debug, Clone)]
pub struct AccountBalanceChanges {
    pub account_id: AccountId,
    pub balance_changes: Vec<TxEffect>,
}
impl AccountBalanceChanges {
    pub fn new(account_id: AccountId) -> Self {
        AccountBalanceChanges {
            account_id,
            balance_changes: Vec::new(),
        }
    }
    pub fn push(&mut self, balance_change: TxEffect) -> &mut Self {
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
    pub fn balance_at(&self, datetime: DateTime<Utc>) -> anyhow::Result<AccountBalanceAt> {
        let mut balance_at_last_tx: Option<AccountBalanceAt> = None;

        for change in &self.before(datetime).sorted_chrono().balance_changes {
            let new_balance = match &balance_at_last_tx {
                Some(prev_balance) => AccountBalanceAt {
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
    pub fn balance_history(&self) -> anyhow::Result<AccountBalanceHistory> {
        let mut balance_history: Vec<AccountBalanceAt> = Vec::new();
        for change in &self.sorted_chrono().balance_changes {
            let new_balance = match &balance_history.last() {
                Some(prev_balance) => AccountBalanceAt {
                    account_id: self.account_id.clone(),
                    datetime: change.datetime,
                    amount: prev_balance.with_interest_at(change.datetime)?.amount
                        + change.amount_diff,
                },
                None => change.balance_then(),
            };
            balance_history.push(AccountBalanceAt {
                datetime: change.datetime,
                amount: new_balance.amount,
                account_id: self.account_id.clone(),
            });
        }
        Ok(AccountBalanceHistory {
            account_id: self.account_id.clone(),
            hist_points: balance_history,
        })
    }
    pub async fn repayment_at(
        &self,
        at: DateTime<Utc>,
        rates: impl RatesApi,
    ) -> Result<LoanRepayment, AnyErr> {
        let asset_pair = GBP_EUR_PAIR.clone();
        let day_price_point = rates
            .rate_at(&at.date_naive(), &asset_pair)
            .await
            .err_ctx("err getting rate_at()")?;
        let to_repay_gbp = self
            .balance_at(at)
            .err_ctx("err getting balance_at()")?
            .amount;

        Ok(LoanRepayment {
            repay_asset: Asset2::Id(EUR.clone()),
            amount_plus_interest_gbp: to_repay_gbp,
            amount_eur: day_price_point.rate_low * to_repay_gbp,
            day_price_point: day_price_point.clone(),
        })
    }
    pub fn last_txn_effect(&self) -> Result<&TxEffect, AnyErr> {
        self.balance_changes
            .iter()
            .max_by(|a, b| a.datetime.cmp(&b.datetime))
            .ok_or("No txns found".into())
    }
    pub async fn min_repayment_after_last_txn(
        &self,
        before: DateTime<Utc>,
        rates_svc: impl RatesApi,
    ) -> Result<LoanRepayment, AnyErr> {
        let last_effect = self.last_txn_effect()?;

        // let balance_after_last_txn = self
        //     .balance_at(last_effect.datetime)
        //     .err_ctx("err getting end balance balance_at()")?;

        let repay_time_range = TimeRange {
            start: last_effect.datetime,
            end: before,
        };

        // let rates_hist = rates_svc
        //     .rate_hist(&repay_time_range, &GBP_EUR_PAIR)
        //     .await
        //     .err_ctx("err getting rate_hist()")?;

        let possible_repayment_dates = DateRange::from_timerange(repay_time_range);

        // let possible_repayments = possible_repayment_dates
        //     .into_iter()
        //     .map(|repay_date| {
        //         let day_price_point = rates_hist
        //             .rate_at(repay_date)
        //             .err_ctx("err getting rate_at()")?;

        //         let new_balance_gbp = self.balance_at(datetime);
        //         let potential_repay = LoanRepayment {
        //             repay_asset: Asset2::Id(EUR.clone()),
        //             amount_plus_interest_gbp: new_balance_gbp,
        //             amount_eur: day_price_point.rate_low * new_balance_gbp,
        //             day_price_point: day_price_point.clone(),
        //         };
        //         Ok(potential_repay)
        //     })
        //     .collect::<Result<Vec<_>, AnyErr>>()?;
        // let possible_repayments = futures::stream::iter(possible_repayment_dates)
        //     .then(|date| self.repayment_at(at, rates))
        //     .collect()
        //     .await;

        let mut opt_repayment = None;
        'range_dates: for date in possible_repayment_dates.into_iter() {
            let possible_repayment = match self
                .repayment_at(DateTime::from_naive_date(date), &rates_svc)
                .await
            {
                Ok(r) => r,
                Err(_) => continue 'range_dates, // ignore error
            };
            match opt_repayment {
                None => {
                    opt_repayment = Some(possible_repayment);
                }
                Some(ref current) => {
                    if possible_repayment.amount_eur < current.amount_eur {
                        opt_repayment = Some(possible_repayment);
                    }
                }
            }
        }

        opt_repayment.ok_or("No repayment found".into())
    }
}

#[derive(Debug)]
pub struct LoanRepayment {
    pub repay_asset: Asset2,
    pub amount_plus_interest_gbp: Decimal,
    pub amount_eur: Decimal,
    pub day_price_point: DayPricePoint,
}

#[derive(Debug, Clone)]
pub struct AccountBalanceHistory {
    pub account_id: AccountId,
    pub hist_points: Vec<AccountBalanceAt>,
}
impl AccountBalanceHistory {
    pub fn account(&self) -> anyhow::Result<&'static Account> {
        self.account_id.account()
    }
    pub fn sorted_chrono(mut self) -> Self {
        self.hist_points.sort_by_key(|bc| bc.datetime);
        self
    }
    // pub fn balance_at(&self, at: DateTime<Utc>) -> Result<AccountBalanceAt, AnyErr> {
    //     // find prev balance (sorted chronologically)
    //     let balance_at_last_tx = self
    //         .hist_points
    //         .iter()
    //         .filter(|b| b.datetime <= at)
    //         .min_by(|a, b| (at - a.datetime).cmp(&(at - b.datetime)))
    //         .ok_or("No prev balance found")?;

    //     todo!()
    // }
}

#[derive(Debug, Clone)]
pub struct AccountBalanceAt {
    pub account_id: AccountId,
    pub datetime: DateTime<Utc>,
    pub amount: Decimal,
}
impl AccountBalanceAt {
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
    pub fn with_interest_at(
        &self,
        datetime_end: DateTime<Utc>,
    ) -> anyhow::Result<AccountBalanceAt> {
        let apy = match self.loan_apy()? {
            Some(apy) => apy,
            None => {
                return Ok(AccountBalanceAt {
                    datetime: datetime_end,
                    ..self.clone()
                });
            }
        };
        if datetime_end.date_naive() < self.datetime.date_naive() {
            return Err(anyhow!(
                "Trying to calculate interest at a date before the prev balance"
            ));
        }

        let num_days =
            Decimal::from_i64((datetime_end.date_naive() - self.datetime.date_naive()).num_days())
                .ok_or(anyhow!("Failed converting num_days to Decimal"))?;
        let apy_multiplier_per_year = Decimal::from(1)
            + apy
                / Decimal::from_f64(100.0)
                    .ok_or(anyhow::Error::msg("value can't be represented as Decimal"))?;
        // Regulations use 365 days as fixed basis for year / APY calculations (ignoring leap years)
        // See 12 CFR 1030.4(d) (CFPB regulations) and Regulation DD (Truth in Savings Act)
        let balance_plus_interest =
            self.amount * apy_multiplier_per_year.powd(num_days / Decimal::from(365));

        Ok(AccountBalanceAt {
            account_id: self.account_id.clone(),
            datetime: datetime_end,
            amount: balance_plus_interest.trunc_with_scale(2),
        })
    }
    pub fn is_close_to(&self, other: impl Into<Decimal>) -> bool {
        self.amount.is_close_to(other)
    }
}
impl std::ops::Add<Decimal> for AccountBalanceAt {
    type Output = AccountBalanceAt;
    fn add(self, rhs: Decimal) -> Self::Output {
        AccountBalanceAt {
            account_id: self.account_id,
            datetime: self.datetime,
            amount: self.amount + rhs,
        }
    }
}
impl PartialEq<i64> for AccountBalanceAt {
    fn eq(&self, rhs: &i64) -> bool {
        self.amount
            == match Decimal::from_i64(*rhs) {
                Some(i) => i,
                None => return false,
            }
    }
}
impl PartialEq<Decimal> for AccountBalanceAt {
    fn eq(&self, rhs: &Decimal) -> bool {
        self.amount == *rhs
    }
}
impl std::fmt::Display for AccountBalanceAt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.account_id, self.amount)
    }
}

#[derive(Debug)]
pub struct CompanyAccounting {
    // pub company: Arc<Company>,
    pub prev_balance_sheet: Option<BalanceSheet4>,
    pub time_range: TimeRange,
    pub transactions: ListTxns,
    pub accounts: EffectsByAccount,
    // pub balance_hist_by_account: BalanceHistByAccount,
    // pub date: NaiveDate,
    pub rate_gbp_eur: DayPricePoint,
}
impl CompanyAccounting {
    // pub fn now(rates_api: &CachedRatesApi) -> anyhow::Result<Self> {
    //     BalanceSheetBuilder::new(Utc::now().date_naive(), rates_api)
    // }
    pub async fn empty(times: TimeRange, rates_api: impl RatesApi) -> anyhow::Result<Self> {
        // fn const_rate_20240827() -> DayPricePoint {
        //     const GBP_TO_EUR: f64 = 1.18321;
        //     DayPricePoint {
        //         datetime: Utc.with_ymd_and_hms(2024, 08, 27, 0, 0, 0).unwrap(),
        //         rate_high: Decimal::from_f64(GBP_TO_EUR).unwrap(),
        //         rate_low: Decimal::from_f64(GBP_TO_EUR).unwrap(),
        //     }
        // }

        Ok(CompanyAccounting {
            transactions: ListTxns::empty(),
            accounts: EffectsByAccount::new(),
            // date,
            rate_gbp_eur: rates_api
                .rate_at(&times.end.date_naive(), &GBP_EUR_PAIR)
                .await?,
            prev_balance_sheet: None,
            time_range: times,
        })
    }
    // pub fn with_date(mut self, date: NaiveDate) -> Self {
    //     self.date = date;
    //     self
    // }
    pub fn date(&self) -> NaiveDate {
        self.time_range.end.date_naive()
    }
    pub fn with_prev_balance_sheet(mut self, prev_balance_sheet: BalanceSheet4) -> Self {
        self.prev_balance_sheet = Some(prev_balance_sheet);
        self
    }
    // pub fn now_from_transactions2(transactions: &[Transaction2]) -> anyhow::Result<Self> {
    //     let mut bs = BalanceSheetBuilder::new(Utc::now().date_naive(), &RATES_API)?;
    //     for tx2 in transactions.iter() {
    //         bs.add_tx2(tx2);
    //     }
    //     Ok(bs)
    // }
    // pub async fn from_txns(date: NaiveDate, transactions: &[Transaction2]) -> anyhow::Result<Self> {
    //     let mut bs = CompanyAccounts::new(date, RATES_API.deref()).await?;
    //     for tx2 in transactions.iter() {
    //         bs.add_tx2(tx2);
    //     }
    //     Ok(bs)
    // }

    pub fn for_account(&self, account: &impl HasAccountId) -> &AccountBalanceChanges {
        self.accounts.for_account(account.account_id())
    }
    pub fn account_mut(&mut self, account_id: &AccountId) -> &mut AccountBalanceChanges {
        self.accounts.account_mut(account_id)
    }
    pub fn end(&self) -> DateTime<Utc> {
        self.time_range.end
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
        self.transactions.push(tx.clone());
        for output in tx.effects.iter() {
            self.account_mut(&output.account_id).push(output.clone());
        }
        self
    }
    pub fn with_transactions(mut self, transactions: &[Transaction2]) -> Self {
        for tx in transactions.iter() {
            self.add_tx2(tx);
        }
        self
    }

    pub fn account_balance(&self, account: &'static Account) -> anyhow::Result<AccountBalanceAt> {
        Ok(self.for_account(&account.id).balance_at(self.end())?)
    }
    // pub fn account_balance_total(&mut self, account: &'static Account) -> anyhow::Result<Decimal> {
    //     Ok(self.account_balance(&account)?)
    // }
    pub fn accounts_of_type(&self, account_type: AccountType) -> Vec<AccountBalanceChanges> {
        self.accounts.accounts_of_type(account_type)
    }
    // pub fn total_of(&self, account_type: AccountType) -> anyhow::Result<Decimal> {
    //     let mut sum = Decimal::ZERO;
    //     for account in &self.accounts_of_type(account_type) {
    //         // TODO balance GBP !
    //         let balance = account.balance_at(DateTime::from_naive_date(self.date))?;
    //         sum += balance.amount;
    //     }
    //     todo!()
    // }

    fn account_balance_gbp(
        &self,
        account_id: AccountId,
        // account_balance_changes: &AccountBalanceChanges,
    ) -> anyhow::Result<Decimal> {
        let account = account_id.account()?;
        let balance = self.for_account(&account_id).balance_at(self.end())?;
        // let account = account_balance_changes
        //     .account()
        //     .map_err(|_| std::fmt::Error)?;
        // let balance = account_balance_changes
        //     .balance_at(self.datetime())
        //     .map_err(|_| std::fmt::Error)?;
        let balance_gbp = match &account.asset_id {
            a if a == &GBP => balance.amount,
            a if a == &EUR => balance.amount / self.rate_gbp_eur.rate_high,
            _ => return Err(anyhow!("Unexpected asset id: {:?}", account.asset_id)),
        };
        let bal_gbp_rounded = match account.account_type() {
            AccountType::Asset => balance_gbp.trunc_with_scale(2),
            AccountType::Liability => balance_gbp.ceil(),
            _ => {
                return Err(anyhow!(
                    "Unexpected account type: {:?}",
                    account.account_type()
                ));
            }
        };
        Ok(bal_gbp_rounded)
    }
    pub fn total_gbp_of(&self, account_type: AccountType) -> anyhow::Result<Decimal> {
        let mut sum = Decimal::ZERO;
        for balance_changes in &self.accounts_of_type(account_type) {
            let account = balance_changes.account()?;
            sum += self.account_balance_gbp(account.id.clone())?;
        }
        Ok(sum)
    }
    pub fn net_worth(&self) -> anyhow::Result<Decimal> {
        Ok(self.total_gbp_of(AccountType::Asset)? - self.total_gbp_of(AccountType::Liability)?)
    }
    fn fmt_account(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        account_balance_changes: &AccountBalanceChanges,
    ) -> std::fmt::Result {
        let account = account_balance_changes
            .account()
            .map_err(|_| std::fmt::Error)?;
        let balance = account_balance_changes
            .balance_at(self.end())
            .map_err(|_| std::fmt::Error)?;
        write!(
            f,
            "{}: {} {}",
            account.name, balance.amount, account.asset_id
        )?;
        if account.asset_id != GBP {
            write!(
                f,
                " ({} GBP)",
                self.account_balance_gbp(account.id.clone())
                    .map_err(|_| std::fmt::Error)?
            )?;
        }
        write!(f, "\n")?;
        Ok(())
    }

    pub fn balance_sheet(&self) -> Result<BalanceSheet4, AnyErr> {
        let mut balance_sheet = BalanceSheet4 {
            account_balances: match &self.prev_balance_sheet {
                Some(prev) => prev.account_balances.clone(),
                None => HashMap::new(),
            },
            date: self.end().date_naive(),
            target_currency: GBP.clone(),
        };

        for abc in self.accounts.0.values() {
            let start_bal = balance_sheet.get(&abc.account_id).unwrap_or(Decimal::ZERO);

            let end_bal = start_bal
                + self
                    .account_balance(&abc.account().unwrap())
                    .unwrap()
                    .amount;

            balance_sheet.set_value(&abc.account_id, end_bal);
        }

        Ok(balance_sheet)
    }
    pub fn profit_loss(&self) -> Result<ProfitAndLoss, AnyErr> {
        ProfitAndLoss::new(self.time_range.clone(), &self.transactions)
    }
}

impl std::fmt::Display for CompanyAccounting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "BALANCE SHEET AT DATE: {}", self.end().date_naive())?;
        writeln!(f, "ASSETS:")?;
        for account in &self.accounts_of_type(AccountType::Asset) {
            self.fmt_account(f, account)?;
        }
        writeln!(f, "LIABILITIES:")?;
        for account in &self.accounts_of_type(AccountType::Liability) {
            self.fmt_account(f, account)?;
        }
        writeln!(
            f,
            "NET WORTH: {}",
            self.net_worth().map_err(|_| std::fmt::Error)?
        )?;
        Ok(())
    }
}

pub struct BalanceHistByAccount {
    pub by_account: HashMap<AccountId, AccountBalanceHistory>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{Loader, NumExt};
    use chrono::{Days, Duration};
    use static_data::{BANK, CLIENTS, DIRECTORS_LOAN};
    use test_utils::{effect_at, effect_days_ago};

    mod test_utils {
        use super::*;
        pub fn effect_days_ago(amount: i64, days_ago: i64) -> TxEffect {
            let bank = AccountId::new(Cow::Borrowed("BANK"));
            TxEffect {
                txn_id: "fake_txn_id".into(),
                account_id: bank,
                amount_diff: Decimal::from(amount),
                datetime: Utc::now() - Duration::days(days_ago),
                tags: Loader::Loaded(vec![]),
            }
        }
        pub fn effect_at(amount: i64, datetime: DateTime<Utc>) -> TxEffect {
            let bank = AccountId::new(Cow::Borrowed("BANK"));
            TxEffect {
                txn_id: "fake_txn_id".into(),
                account_id: bank,
                amount_diff: Decimal::from(amount),
                datetime,
                tags: Loader::Loaded(vec![]),
            }
        }
    }

    #[tokio::test]
    async fn tx2_test_balance_sheet3_directors_loan_instant_repayment() -> anyhow::Result<()> {
        let yesterday = Utc::now() - Duration::days(1);
        let times = TimeRange {
            start: Utc::now() - Duration::days(2),
            end: yesterday,
        };
        let mut bs = CompanyAccounting::empty(times, &*RATES_API).await?;

        bs.add_tx2(&Transaction2::sale((1000, yesterday)));
        assert_eq!(bs.account_balance(&BANK)?, 1000);
        assert_eq!(bs.account_balance(&CLIENTS)?, (-1000));

        bs.add_tx2(&Transaction2::director_borrows_gbp((1000_f64, yesterday)));
        assert_eq!(bs.account_balance(&BANK)?, 0);
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN)?, 1000);

        bs.add_tx2(&Transaction2::director_repays_bank_gbp((
            1000_f64, yesterday,
        )));
        assert_eq!(bs.account_balance(&BANK)?, 1000);
        assert_eq!(bs.account_balance(&DIRECTORS_LOAN)?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn tx2_test_balance_sheet3_loan_out_repayment_year_later() -> anyhow::Result<()> {
        // Regulations use 365 days as fixed basis for year / APY calculations (ignoring leap years)
        let year_ago = Utc::now() - Days::new(365);
        let times = TimeRange {
            start: year_ago,
            end: Utc::now() - Duration::days(1),
        };

        let mut ca = CompanyAccounting::empty(times, &*RATES_API).await?;
        ca.add_tx2(&Transaction2::sale((1000, year_ago)));
        assert_eq!(ca.account_balance(&BANK)?, 1000);
        assert_eq!(ca.account_balance(&CLIENTS)?, (-1000));

        ca.add_tx2(&Transaction2::director_borrows_gbp((1000, year_ago)));
        assert_eq!(ca.account_balance(&BANK)?, 0);
        assert_eq!(ca.account_balance(&DIRECTORS_LOAN)?, 1000);

        // ca = ca.with_date(Utc::now().date_naive());
        assert_eq!(ca.account_balance(&BANK)?, 0);
        // one year later, loan balance should be 1000 + 2% APY
        assert!(
            ca.account_balance(&DIRECTORS_LOAN)?
                .amount
                .is_close_to(1020)
        );

        // now repay 1 year later, make sure the balance gets set to zero
        ca.add_tx2(&Transaction2::director_repays_bank_gbp((1020, Utc::now())));
        assert_eq!(ca.account_balance(&BANK)?, 1020);
        assert_eq!(ca.account_balance(&DIRECTORS_LOAN)?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_account_balance_history() -> anyhow::Result<()> {
        // let bank = AccountId::new(Cow::Borrowed("BANK"));
        // const DIRECTORS_LOAN: &str = "DIRECTORS_LOAN";

        // create list of TxEffects for account (no interest)
        // see if balance history is correct
        let abc: AccountBalanceChanges = AccountBalanceChanges {
            account_id: "BANK".into(),
            balance_changes: vec![
                effect_days_ago(1000_i64, 3),
                effect_days_ago(1000_i64, 2),
                effect_days_ago(1000_i64, 1),
            ],
        };
        // calc balance history
        let history = abc.balance_history()?;
        dbg!(&history);

        // use values:
        // - from existing test
        // - from real life with actual repayment (doesn't push balance into positive)
        // - from real life with actual repayment (pushes balance into positive)

        Ok(())
    }

    // TODO test with interest (all same 3 values)

    #[tokio::test]
    async fn test_min_repayment_after_last_txn() -> anyhow::Result<()> {
        let abc: AccountBalanceChanges = AccountBalanceChanges {
            account_id: "BANK".into(),
            balance_changes: vec![
                effect_days_ago(1000_i64, 3),
                effect_days_ago(1000_i64, 2),
                effect_days_ago(1000_i64, 1),
            ],
        };
        let now = Utc::now();
        let min_repay = abc.min_repayment_after_last_txn(now, &*RATES_API).await?;
        assert_eq!(min_repay.amount_plus_interest_gbp, 3000.into());

        // Same for account with interest (DIR_LOAN)
        let year_ago = Utc::now() - Days::new(365); // regulations use 365-day years for APY calculations
        let abc: AccountBalanceChanges = AccountBalanceChanges {
            account_id: "DIRECTORS_LOAN".into(),
            balance_changes: vec![effect_at(1000_i64, year_ago)],
        };
        let min_repay = abc.min_repayment_after_last_txn(now, &*RATES_API).await?;
        dbg!(&min_repay);
        assert_eq!(min_repay.amount_plus_interest_gbp, 3000.into());

        Ok(())
    }
}
