use crate::models::balance_sheet::SumBalances;
use crate::models::profit_and_loss::ProfitAndLoss;
use crate::models::Transaction;
pub use config::CONFIG;

pub mod config;
pub mod models;
pub mod nordigen;
pub mod utils;

fn main() {
    let transactions = [
        Transaction::sale(9240.0, (2023, 01, 13)),
        Transaction::sale(8820.0, (2023, 02, 14)),
        Transaction::withdraw_loan(1400.0, (2023, 02, 26)),
    ];

    let totals = SumBalances::from_transactions(&transactions);
    let balance_sheet = totals.balance_sheet();
    dbg!(balance_sheet);

    // TODO profit and loss
    let profit_and_loss = ProfitAndLoss::from_transactions(&transactions);
    print!("{profit_and_loss}");

    // TODO validate transaction amounts
    // TODO connect to bank account
    // TODO task for bank account transfers without corresponding accounting
    // TODO assert invoice file per transaction (store in dir in git)
}
