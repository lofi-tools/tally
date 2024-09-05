use super::{
    static_data::{CLIENTS, EXPENSES_TO_REPAY, WAGES_NET},
    tx2::TxOutput,
};
use crate::{adapters::exchange_rates::TimeRange, Expense, Expenses, ListTxs};
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
    //
    // net
}
impl ProfitAndLoss {
    pub fn new(dates: TimeRange, txs: &ListTxs) -> anyhow::Result<Self> {
        let txs = txs.between_dates(dates.start.date_naive(), dates.end.date_naive());
        Ok(ProfitAndLoss {
            time_range: dates,
            sales: txs.sale_outputs(),
            loan_interest: Outputs(Vec::new()),     // TODO
            capital_gain_loss: Outputs(Vec::new()), // TODO add NEXO_EUR outputs
            wages: txs.wage_outputs(),
            other_expenses: Expenses::static_expenses_to_repay()?,
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
        Decimal::ZERO // TODO
    }
    pub fn total_expenses(&self) -> Decimal {
        self.calc_wages() + self.calc_other_expenses()
    }

    pub fn overall_profit(&self) -> Decimal {
        self.total_earnings() - self.total_expenses()
    }
    pub fn corp_tax(&self) -> Decimal {
        // self.overall_profit() * Decimal::from_f64(0.2).unwrap()
        let OVERALL_PROFIT_OVERRIDE = Decimal::from(68946);

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
            * OVERALL_PROFIT_OVERRIDE
            * Decimal::from(proportion_old_calc))
        .trunc();

        fn tax_new_rate(profit: Decimal) -> Decimal {
            let on_first_50k =
                (Decimal::from(50_000).min(profit) * Decimal::from_f64(0.19).unwrap()).trunc();
            let on_rest = match (profit - Decimal::from(50_000)) {
                x if x.is_sign_positive() => x * Decimal::from_f64(0.265).unwrap(),
                _ => Decimal::ZERO,
            }
            .trunc();

            (on_first_50k + on_rest).trunc()
        }
        let tax_new_rate =
            tax_new_rate(OVERALL_PROFIT_OVERRIDE * Decimal::from(proportion_new_calc));

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
            *curr_total += expense.tx.outputs[0].amount_diff;
        }
        dbg!(&expenses_by_desc);

        for (desc, amount) in &expenses_by_desc {
            writeln!(f, "{}: {}", desc, amount)?;
        }
        Ok(())
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
            self.corp_tax()
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Outputs(pub Vec<TxOutput>);
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
    fn expenses_to_repay(&self) -> Outputs {
        Outputs(
            self.0
                .iter()
                .filter(|o| o.is_expense_to_repay())
                .cloned()
                .collect(),
        )
    }
}

impl ListTxs {
    pub fn outputs(&self) -> Outputs {
        Outputs(
            self.txs
                .iter()
                .map(|tx| tx.outputs.clone())
                .flatten()
                .collect::<Vec<TxOutput>>(),
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

// pub struct AccountingPeriod(pub TimeRange);
