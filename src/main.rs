use crate::models::profit_and_loss::ProfitAndLoss;
use crate::models::sheet3::BalanceSheet3;
use adapters::banks::nordigen_client::NordigenClient;
pub use config::CONFIG;
use models::tx2::Transaction2;

pub mod config;
pub mod models;
pub mod utils;
pub mod adapters {
    pub mod banks;
    pub mod exchange_rates;
}

// TODO calculate interest on director's loan -> add interest in transaction
// TODO transaction::repay_loan_in_currency
// TODO ensure director's loan account is zero

// TODO struct ListTransactions -> get_balance_sheet_at(date): adds interest transactions at end of months
// TODO struct ListTransactions:

// TODO BalanceSheet contains Accounts, Accounts contain BalanceChanges
// -> calculating the balance sheet ? means classifying BalanceChanges, then going over loan accounts again for interest

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let starling_transactions = fetch_starling_transactions().await?;
    let extra_transactions: Vec<Transaction2> = vec![
        // Transaction::sale(9240.0, (2023, 01, 13)),
        // Transaction::sale(8820.0, (2023, 02, 14)),
        // Transaction::withdraw_loan(1400.0, (2023, 02, 26)),
    ];

    // let totals = BalanceTotals::from_transactions(&transactions);
    // let balance_sheet = totals.balance_sheet();
    let balance_sheet = BalanceSheet3::now_from_transactions2(&starling_transactions);
    println!("{balance_sheet}");

    // TODO profit and loss
    // let profit_and_loss = ProfitAndLoss::from_transactions2(&transactions);
    // print!("{profit_and_loss}");

    // TODO task for bank account transfers without corresponding accounting
    // TODO assert invoice file per transaction (store in dir in git)

    Ok(())
}

async fn fetch_starling_transactions() -> anyhow::Result<Vec<Transaction2>> {
    let mut nordigen_client = NordigenClient::new();
    let transactions = nordigen_client.list_starling_transactions().await?;

    let mapped_transactions: Vec<Transaction2> = transactions
        .iter()
        .map(|tx| Transaction2::from_starling(tx))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(mapped_transactions)
}
