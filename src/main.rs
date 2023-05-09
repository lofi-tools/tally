use crate::models::balance_sheet::SumBalances;
use crate::models::profit_and_loss::ProfitAndLoss;
use crate::models::Transaction;
use anyhow::anyhow as err;
pub use config::CONFIG;
use models::std_accounts::*;
use nordigen::{BookedTransaction, NordigenClient};

pub mod config;
pub mod models;
pub mod nordigen;
pub mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let transactions = fetch_starling_transactions().await?;

    // let transactions = [
    // Transaction::sale(9240.0, (2023, 01, 13)),
    // Transaction::sale(8820.0, (2023, 02, 14)),
    // Transaction::withdraw_loan(1400.0, (2023, 02, 26)),
    // ];

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

    Ok(())
}

async fn fetch_starling_transactions() -> anyhow::Result<Vec<Transaction>> {
    let mut nordigen_client = NordigenClient::new();
    let transactions = nordigen_client.list_starling_transactions().await?;

    let mapped_transactions: Vec<Transaction> = transactions
        .iter()
        .map(|tx| Transaction::from_starling(tx))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(mapped_transactions)
}

impl Transaction {
    fn from_starling(starling_tx: &BookedTransaction) -> anyhow::Result<Transaction> {
        match starling_tx {
            tx if tx.debtor_name == Some("Nigel Frank Intern".to_string()) => Ok(
                Transaction::sale(tx.transaction_amount.amount, tx.booking_date),
            ),
            tx if tx.remittance_information_unstructured.contains("Salary") => Ok(
                Transaction::salary(tx.transaction_amount.amount, tx.booking_date),
            ),
            tx if tx
                .remittance_information_unstructured
                .contains("Director's loan") =>
            {
                Ok(Transaction {
                    from: &DIRECTORS_LOAN,
                    to: &BANK,
                    amount_gbp: tx.transaction_amount.amount,
                    date: tx.booking_date,
                })
            }
            tx if tx
                .remittance_information_unstructured
                .contains("120PZ028811752312") =>
            {
                Ok(Transaction::to_paye(
                    tx.transaction_amount.amount,
                    tx.booking_date,
                ))
            }
            other_tx => return Err(err!("no match for tx: {other_tx:?}")),
        }
    }
}
