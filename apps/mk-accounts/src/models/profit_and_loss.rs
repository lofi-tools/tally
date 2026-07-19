use std::sync::LazyLock;

use super::static_data::{CLIENTS, CORP_TAX_PAID, WAGES_NET};
use super::tx2::TxEffect;
use crate::utils::errors::{AnyErr, AnyhowResExt};
use crate::{Expenses, ListTxns, adapters::exchange_rates::TimeRange};
use chrono::NaiveDate;
use num_traits::FromPrimitive;
use rust_decimal::Decimal;

#[derive(Debug)]
pub struct ProfitAndLoss {
    pub time_range: TimeRange,

    // gross earnings
    pub sales: Outputs,
    pub loan_interest: Outputs,
    pub capital_gain_loss: Outputs, // TODO add NEXO_EUR outputs
    // gross expenses
    pub wages: Outputs,
    pub other_expenses: Expenses,
    pub corp_tax_paid: Outputs,
    //
    // net
}
impl ProfitAndLoss {
    pub fn new(dates: TimeRange, txs: &ListTxns) -> Result<Self, AnyErr> {
        let txs = txs.between_dates(dates.start.date_naive(), dates.end.date_naive());
        Ok(ProfitAndLoss {
            time_range: dates,
            sales: txs.sale_outputs(),
            loan_interest: Outputs(Vec::new()),     // TODO
            capital_gain_loss: Outputs(Vec::new()), // TODO add NEXO_EUR outputs
            wages: txs.wage_outputs(),
            corp_tax_paid: txs.effects_corp_tax_paid(),
            other_expenses: Expenses::static_expenses_to_repay().err_ctx("err getting expenses")?,
        })
    }

    pub fn calc_sales(&self) -> Decimal {
        -self.sales.total() // minus because we sum CLIENTS account which is negative for
    }
    pub fn calc_loan_interest_earned(&self) -> Decimal {
        Decimal::ZERO // TODO
    }
    pub fn calc_capital_gain_loss(&self) -> Decimal {
        Decimal::ZERO // TODO
    }
    pub fn total_earnings(&self) -> Decimal {
        self.calc_sales() + self.calc_loan_interest_earned() + self.calc_capital_gain_loss()
    }
    pub fn calc_wages(&self) -> Decimal {
        self.wages.total()
    }
    pub fn calc_other_expenses(&self) -> Decimal {
        Decimal::from(540) // TODO
    }
    pub fn other_charges(&self) -> Decimal {
        self.calc_other_expenses() + self.corp_tax_paid()
    }
    pub fn total_expenses(&self) -> Decimal {
        self.calc_wages() + self.calc_other_expenses() + self.corp_tax_paid()
    }

    pub fn overall_profit(&self) -> Decimal {
        self.total_earnings() - self.total_expenses()
    }
    pub fn corp_tax_paid(&self) -> Decimal {
        self.corp_tax_paid.total().ceil()
    }
    pub fn next_corp_tax_estimate(&self) -> Decimal {
        self.overall_profit() * Decimal::from_f64(0.25).unwrap()
    }
    pub fn next_corp_tax_estimate_2024(&self) -> Decimal {
        const OVERALL_PROFIT_OVERRIDE: LazyLock<Decimal> = LazyLock::new(|| Decimal::from(68946));

        let num_days_old_calc = (NaiveDate::from_ymd_opt(2023, 03, 31).unwrap()
            - self.time_range.start.date_naive())
        .num_days();
        let num_days_new_calc = (self.time_range.end.date_naive()
            - NaiveDate::from_ymd_opt(2023, 04, 01).unwrap())
        .num_days();

        let proportion_old_calc =
            Decimal::from(num_days_old_calc) / Decimal::from(num_days_old_calc + num_days_new_calc);
        let proportion_new_calc =
            Decimal::from(num_days_new_calc) / Decimal::from(num_days_old_calc + num_days_new_calc);

        let tax_old_rate = (Decimal::from_f64(0.2).unwrap()
            * (*OVERALL_PROFIT_OVERRIDE)
            * Decimal::from(proportion_old_calc))
        .trunc();

        fn tax_new_rate(profit: Decimal) -> Decimal {
            let on_first_50k =
                (Decimal::from(50_000).min(profit) * Decimal::from_f64(0.19).unwrap()).trunc();
            let on_rest = match profit - Decimal::from(50_000) {
                x if x.is_sign_positive() => x * Decimal::from_f64(0.265).unwrap(),
                _ => Decimal::ZERO,
            }
            .trunc();

            (on_first_50k + on_rest).trunc()
        }
        let tax_new_rate =
            tax_new_rate((*OVERALL_PROFIT_OVERRIDE) * Decimal::from(proportion_new_calc));

        // let corp_tax = Decimal::from(50_000) * Decimal::from_f64(0.19).unwrap()
        //     + (OVERALL_PROFIT_OVERRIDE - Decimal::from(50_000)) * Decimal::from_f64(0.265).unwrap();
        tax_old_rate.trunc() + tax_new_rate.trunc()
    }

    fn fmt_other_expenses(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut expenses_by_desc = std::collections::HashMap::new();
        for expense in &self.other_expenses.0 {
            let curr_total = expenses_by_desc
                .entry(expense.desc.clone())
                .or_insert(Decimal::ZERO);
            *curr_total += expense.tx.effects[0].amount_diff;
        }
        dbg!(&expenses_by_desc);

        for (desc, amount) in &expenses_by_desc {
            writeln!(f, "{}: {}", desc, amount)?;
        }
        Ok(())
    }

    pub fn report(&self) -> PnlReport {
        use num_traits::ToPrimitive;
        PnlReport {
            turnover: self.total_earnings().floor().to_u64().unwrap(),
            staff_costs: self.calc_wages().ceil().to_u64().unwrap(),
            depreciation_and_written_off_assets: 0, // TODO
            other_charges: self.other_charges().ceil().to_u64().unwrap(),
            tax_on_profit: self.next_corp_tax_estimate().ceil().to_u64().unwrap(),
        }
    }
    // TODO calculate interest made on director's loan (total repaid - total borrowed)
    // TODO calculate capital gain/loss from accounts in other currencies (go through transaction, add up GBP amount, calculate GBP amount now, diff)
}
impl std::fmt::Display for ProfitAndLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "PROFIT AND LOSS between {} and {}",
            self.time_range.start, self.time_range.end
        )?;
        writeln!(f, "EARNINGS: {}", self.total_earnings())?;
        writeln!(f, "Sales: {}", self.calc_sales())?;
        writeln!(f, "Loan interest: {}", self.calc_loan_interest_earned())?;
        writeln!(f, "Capital gain/loss: {}", self.calc_capital_gain_loss())?;
        writeln!(f, "EXPENSES: {}", self.total_expenses())?;
        writeln!(f, "Wages: {}", self.wages.total())?;
        writeln!(f, "Corp tax paid: {}", self.corp_tax_paid())?;
        writeln!(f, "Other expenses: {}", self.calc_other_expenses())?;
        writeln!(f, "Expense details:")?;
        self.fmt_other_expenses(f)?;
        writeln!(f, "")?;
        writeln!(
            f,
            "TOTAL_EARNINGS: {}, TOTAL_EXPENSES: {}, PROFIT (loss if <0): {}, CORP_TAX: {},",
            self.total_earnings(),
            self.total_expenses(),
            self.overall_profit(),
            self.next_corp_tax_estimate()
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Outputs(pub Vec<TxEffect>);
impl Outputs {
    pub fn between_times(&self, times: TimeRange) -> Outputs {
        Outputs(
            self.0
                .iter()
                .filter(|txo| txo.datetime >= times.start && txo.datetime <= times.end)
                .cloned()
                .collect(),
        )
    }
    pub fn total(&self) -> Decimal {
        self.0.iter().map(|txo| txo.amount_diff).sum::<Decimal>()
    }

    pub fn sales(&self) -> Outputs {
        let filtered = self
            .0
            .iter()
            .filter(|o| o.account().ok() == Some(*CLIENTS))
            .cloned()
            .collect();
        Outputs(filtered)
    }
    pub fn wages(&self) -> Outputs {
        let filtered = self
            .0
            .iter()
            .filter(|o| o.account().ok() == Some(*WAGES_NET))
            .cloned()
            .collect();
        Outputs(filtered)
    }
    fn _expenses_to_repay(&self) -> Outputs {
        Outputs(
            self.0
                .iter()
                .filter(|o| o.is_expense_to_repay())
                .cloned()
                .collect(),
        )
    }

    pub(crate) fn corp_tax_paid(&self) -> Outputs {
        Outputs(
            self.0
                .iter()
                .filter(|o| o.account().ok() == Some(*CORP_TAX_PAID))
                .cloned()
                .collect(),
        )
    }
}

impl ListTxns {
    pub fn outputs(&self) -> Outputs {
        Outputs(
            self.txs
                .iter()
                .map(|tx| tx.effects.clone())
                .flatten()
                .collect::<Vec<TxEffect>>(),
        )
    }
    pub fn sale_outputs(&self) -> Outputs {
        self.outputs().sales()
    }
    pub fn wage_outputs(&self) -> Outputs {
        let wage_outputs = self.outputs().wages();
        wage_outputs
    }
    // pub fn outputs_expenses_to_repay(&self) -> Outputs {
    //     self.outputs().expenses_to_repay()
    // }
}

#[derive(Debug, Clone)]
pub struct PnlReport {
    pub turnover: u64,
    pub staff_costs: u64,
    pub depreciation_and_written_off_assets: u64,
    pub other_charges: u64,
    pub tax_on_profit: u64,
}
impl PnlReport {
    pub fn profit(&self) -> i64 {
        todo!()
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::{
//         adapters::{exchange_rates::RATES_API, starling_bank::StarlingClient},
//         models::company::EXT,
//     };

//     use super::*;

//     #[tokio::test]
//     async fn test_pnl_report() -> anyhow::Result<()> {
//         let times = EXT.last_accounting_period()?;
//         dbg!(&times);

//         let mut bank_api = StarlingClient::new()?;
//         EXT.accounting_at(times, &mut bank_api, &*RATES_API).await?;

//         Ok(())
//     }
// }
